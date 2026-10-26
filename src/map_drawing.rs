use serde::{Deserialize,Serialize};
use std::collections::VecDeque;

pub const MAX_STROKES:usize=64;
pub const MAX_POINTS:usize=128;
pub const MAX_PENDING:usize=8;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Scope {pub seed:u64,pub act:u8}
impl Scope {pub fn of(run:&crate::Run)->Self {Self{seed:run.seed,act:run.act}}}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Point {pub x:u16,pub y:u16}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct StrokeId {pub player:u64,pub sequence:u64}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Stroke {pub id:StrokeId,pub points:Vec<Point>}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum Edit {Add(Vec<Point>),Erase(Vec<StrokeId>)}
impl Edit {
    pub fn valid(&self)->bool {match self {
        Self::Add(points)=>valid_points(points),
        Self::Erase(ids)=>!ids.is_empty()&&ids.len()<=MAX_STROKES&&ids.iter().all(|id|id.player!=0&&id.sequence!=0),
    }}
}
fn valid_points(points:&[Point])->bool {(2..=MAX_POINTS).contains(&points.len())&&points.iter().all(|p|p.x<=10000&&p.y<=10000)}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Request {pub scope:Scope,pub sequence:u64,pub edit:Edit}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Canvas {pub scope:Scope,pub revision:u64,pub strokes:Vec<Stroke>,pub acknowledged:Vec<(u64,u64)>}
impl Canvas {
    pub fn valid(&self)->bool {
        (1..=3).contains(&self.scope.act)&&self.strokes.len()<=MAX_STROKES
            &&self.acknowledged.len()<=crate::party_roster::MAX_PLAYERS
            &&self.acknowledged.iter().enumerate().all(|(i,(id,n))|*id!=0&&*n!=0&&!self.acknowledged[..i].iter().any(|p|p.0==*id))
            &&self.strokes.iter().enumerate().all(|(i,s)|s.id.player!=0&&s.id.sequence!=0
                &&valid_points(&s.points)&&!self.strokes[..i].iter().any(|p|p.id==s.id))
    }
    fn ack(&self,player:u64)->u64 {self.acknowledged.iter().find(|p|p.0==player).map_or(0,|p|p.1)}
}
#[derive(Default)]
pub struct Sync {pub canvas:Option<Canvas>,pending:VecDeque<Request>,sequence:u64}
impl Sync {
    pub fn set_scope(&mut self,scope:Option<Scope>) {
        if self.canvas.as_ref().map(|c|c.scope)==scope {return;}
        self.pending.clear();
        self.canvas=scope.map(|scope|Canvas{scope,revision:0,strokes:vec![],acknowledged:vec![]});
    }
    pub fn request(&self)->Option<Request> {self.pending.front().cloned()}
    pub fn preview(&self,player:u64)->Option<Canvas> {
        let mut canvas=self.canvas.clone()?;
        for request in &self.pending {match &request.edit {
            Edit::Add(points) if canvas.strokes.len()<MAX_STROKES=>canvas.strokes.push(Stroke{id:StrokeId{player,sequence:request.sequence},points:points.clone()}),
            Edit::Erase(ids)=>canvas.strokes.retain(|s|!ids.contains(&s.id)),
            _=>{},
        }}
        Some(canvas)
    }
    pub fn submit(&mut self,player:u64,edit:Edit,host:bool)->bool {
        let Some(canvas)=&self.canvas else{return false;};
        if !edit.valid()||player==0||self.pending.len()>=MAX_PENDING
            ||matches!(&edit,Edit::Add(_))&&canvas.strokes.len()>=MAX_STROKES {return false;}
        let Some(sequence)=self.sequence.checked_add(1) else{return false;};self.sequence=sequence;
        let request=Request{scope:canvas.scope,sequence,edit};
        if host {self.apply(player,&request)}else{self.pending.push_back(request);true}
    }
    pub fn apply(&mut self,player:u64,request:&Request)->bool {
        let Some(canvas)=&mut self.canvas else{return false;};
        if player==0||request.sequence==0||request.scope!=canvas.scope||!request.edit.valid()
            ||request.sequence<=canvas.ack(player)||canvas.revision==u64::MAX {return false;}
        if let Some(ack)=canvas.acknowledged.iter_mut().find(|p|p.0==player){ack.1=request.sequence;}
        else if canvas.acknowledged.len()<crate::party_roster::MAX_PLAYERS {canvas.acknowledged.push((player,request.sequence));}
        else{return false;}
        match &request.edit {
            Edit::Add(points) if canvas.strokes.len()<MAX_STROKES=>canvas.strokes.push(Stroke{id:StrokeId{player,sequence:request.sequence},points:points.clone()}),
            Edit::Erase(ids)=>canvas.strokes.retain(|s|!ids.contains(&s.id)),
            _=>{},
        }
        canvas.revision+=1;true
    }
    pub fn receive(&mut self,canvas:Canvas,player:u64)->bool {
        if !canvas.valid()||self.canvas.as_ref().is_none_or(|current|current.scope!=canvas.scope||current.revision>=canvas.revision){return false;}
        let ack=canvas.ack(player);
        self.pending.retain(|r|r.sequence>ack);self.sequence=self.sequence.max(ack);self.canvas=Some(canvas);true
    }
}
pub fn near_segment(p:[f32;2],a:[f32;2],b:[f32;2],radius:f32)->bool {
    let d=[b[0]-a[0],b[1]-a[1]];let length=d[0]*d[0]+d[1]*d[1];
    let t=if length>0.0 {((p[0]-a[0])*d[0]+(p[1]-a[1])*d[1])/length}else{0.0}.clamp(0.0,1.0);
    (p[0]-a[0]-t*d[0]).hypot(p[1]-a[1]-t*d[1])<=radius
}
