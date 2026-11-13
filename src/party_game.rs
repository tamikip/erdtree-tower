use serde::{Deserialize,Serialize};
use crate::{Run,Phase,RuleResult};
pub fn act_seed(initial:u64,act:u8)->RuleResult<u64> {
    if !(1..=3).contains(&act){return Err("幕数无效".into());}
    Ok((2..=act).fold(initial,|seed,act|crate::mix(seed^u64::from(act))))
}

#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub session:u64, pub seed:u64, pub player:u64, pub host:u64,
    pub address:std::net::SocketAddr, pub token:String, pub peers:Vec<Peer>,
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Peer {pub player:u64,pub token:String}
impl Config {
    pub fn decode(text:&str)->RuleResult<Self> {
        if text.len()>8192{return Err("队伍配置过大".into());}
        let c:Self=serde_json::from_str(text).map_err(|_|"队伍配置格式无效")?;
        if c.session==0||c.player==0||c.host==0||c.address.port()==0||c.peers.len()>=crate::party_roster::MAX_PLAYERS
            ||c.token.len()<32||c.token.len()>256 {return Err("队伍配置无效".into());}
        if c.player==c.host {
            if c.peers.is_empty(){return Err("房主必须配置队员".into());}
            for (i,p) in c.peers.iter().enumerate(){
                if p.player==0||p.player==c.host||p.token.len()<32||p.token.len()>256
                    ||c.peers[..i].iter().any(|q|q.player==p.player||q.token==p.token){return Err("队员凭据无效或重复".into());}
            }
        }else if !c.peers.is_empty(){return Err("队友配置不能包含其他队员凭据".into());}
        Ok(c)
    }
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Report {
    #[serde(default)] pub pause_saved:Option<u64>,
    #[serde(default)] pub leaving:bool,
    #[serde(default)] pub resume:Option<crate::suspension::Point>,
    #[serde(default)] pub merchant_bound:bool,
    #[serde(default)] pub testing:bool,
    #[serde(default)] pub event_testing:bool,
    #[serde(default)] pub mausoleum_open:Option<bool>,
    #[serde(default)] pub scarab_claim:Option<ScarabClaim>,
    #[serde(default)] pub law:crate::law::Law,
    #[serde(default)] pub flask_uses:u64,
    #[serde(default)] pub drawing:Option<crate::map_drawing::Request>,
    #[serde(default)] pub trial:bool,
    #[serde(default)] pub trial_choice:Option<crate::regional_events::Trial>,
    #[serde(default)] pub has_stonesword_key:bool,
    #[serde(default)] pub rest_end:bool,
    /// 战斗胜利后已交互奖励传送门（全员都进入后房主才发奖励）
    #[serde(default)] pub portal:bool,
    /// 选用的称号（titles::TITLES 的编号，0 为不戴），房主汇总后用 Order::Titles 发给全队
    #[serde(default)] pub title:u8,
    pub present:bool,
    pub at_hub:bool,
    pub event_request:Option<crate::party_events::Kind>,
    pub event_bound:Option<u8>,
    pub event_applied:Option<u8>,
    #[serde(default)]
    pub selected:Option<usize>,
    #[serde(default)] pub stormveil:Option<crate::stormveil::Progress>,
    #[serde(default)] pub storm_choice:Option<bool>,
    #[serde(default)] pub storm_finish:bool,
    #[serde(default)] pub chariot_ready:bool,
    #[serde(default)] pub chariot:bool,
    /// 成员带着的余烬：房主据此核对代为执行的敌人侧效果
    #[serde(default)] pub embers:Vec<crate::embers::Ember>,
    /// 成员算出的敌人侧余烬效果（最近 STRIKE_WINDOW 条，带序号），由房主执行没执行过的
    #[serde(default)] pub strikes:Vec<crate::embers::StrikeRequest>,
    pub epoch:u64,pub seed:u64,pub act:u8,pub visited:Vec<usize>,pub map_ready:bool,
    pub arrived:bool,pub bound:bool,pub alive:bool,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct ScarabClaim {pub wave:u8,pub killed:u8,pub owned:u8,pub finished:bool}
impl ScarabClaim {
    fn valid(self)->bool {self.wave>0&&self.killed<=7&&self.owned&!self.killed==0}
}
impl Report {
    pub fn new(run:&Run,epoch:u64,arrived:bool,bound:bool,alive:bool)->Self {
        Self{pause_saved:None,leaving:false,resume:None,merchant_bound:false,testing:false,event_testing:false,mausoleum_open:None,scarab_claim:None,law:run.law,flask_uses:0,drawing:None,trial:run.regional_event.trial,trial_choice:None,has_stonesword_key:false,chariot_ready:false,rest_end:false,portal:false,title:0,present:true,at_hub:false,event_request:None,event_bound:None,event_applied:None,selected:None,chariot:run.regional_event.chariot,stormveil:run.regional_event.stormveil,storm_choice:None,storm_finish:false,embers:run.embers.clone(),strikes:vec![],epoch,seed:run.seed,act:run.act,visited:run.visited.clone(),map_ready:run.phase==Phase::Map&&!run.contract_pending(),arrived,bound,alive}
    }
    pub fn unavailable(epoch:u64)->Self {
        Self{pause_saved:None,leaving:false,resume:None,merchant_bound:false,testing:false,event_testing:false,mausoleum_open:None,scarab_claim:None,law:Default::default(),flask_uses:0,drawing:None,trial:false,trial_choice:None,has_stonesword_key:false,chariot_ready:false,rest_end:false,portal:false,title:0,present:false,at_hub:false,event_request:None,event_bound:None,event_applied:None,selected:None,chariot:false,stormveil:None,storm_choice:None,storm_finish:false,embers:vec![],strikes:vec![],epoch,seed:0,act:1,visited:vec![],map_ready:false,arrived:false,bound:false,alive:false}
    }
    pub fn at_hub(epoch:u64)->Self {Self{present:true,at_hub:true,alive:true,..Self::unavailable(epoch)}}
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub enum Order {
    PausePrepare{epoch:u64,ticket:u64},
    PauseCommit{epoch:u64,ticket:u64},
    PauseAbort{epoch:u64,ticket:u64},
    LeaveReady{epoch:u64,player:u64},
    ResumeReady{epoch:u64,point:crate::suspension::Point},
    DescentComplete{epoch:u64},
    MausoleumOpen{epoch:u64},
    Merchant{epoch:u64,phase:u8},
    BossTest{epoch:u64,seed:u64,test:crate::debug::BossTest},
    EventTest{epoch:u64,seed:u64,act:u8,visited:Vec<usize>,draw:crate::events::Draw,has_stonesword_key:bool},
    Law{epoch:u64,seed:u64,law:crate::law::Law},
    MapDrawing(crate::map_drawing::Canvas),
    ForceEnd,
    SkipNode{epoch:u64,target:crate::skip::Target},
    ChariotPose{epoch:u64,sequence:u64,pose:[f32;3]},
    Trial{epoch:u64,choice:crate::regional_events::Trial},
    Stormveil{epoch:u64,decision:crate::stormveil::Decision},
    NewRun{epoch:u64,seed:u64},
    /// 房主在大赐福用固定种子开局：换成这个队伍种子（同一轮次，不重置队伍）；还没出发的新旅程按它重排路线
    Reseed{epoch:u64,seed:u64},
    Votes{epoch:u64,ballots:Vec<crate::party_vote::Ballot>},
    Enter{epoch:u64,act:u8,visited:Vec<usize>,node:usize,has_stonesword_key:bool},
    EventStart(crate::party_events::Job),
    EventHunt{epoch:u64,wave:u8},
    EventAborted{epoch:u64,wave:u8},
    EventDone{epoch:u64,wave:u8,killed:u8,owners:[u64;3]},
    Spawn{epoch:u64},Engage{epoch:u64},Victory{epoch:u64},Rewards{epoch:u64},Defeat{epoch:u64},Stop(String),
    /// 全队的称号：(Steam ID, 称号编号)；不分轮次，随时覆盖
    Titles(Vec<(u64,u8)>),
    FrostShackles{epoch:u64,remaining:Vec<(u8,u16)>},
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Stage {Map,Travel,Combat,Fighting,Service,Reward,Stopped}
pub struct Gate {trial:Option<crate::regional_events::Trial>,pub epoch:u64,pub stage:Stage,peers:Vec<(u64,Option<Report>)>,events:Option<crate::party_events::Queue>,stormveil:Option<crate::stormveil::Decision>}
impl Gate {
    pub fn skip_node(&mut self,rewards:bool) {
        self.stage=if rewards{Stage::Reward}else{Stage::Service};
        self.events=None;self.trial=None;self.stormveil=None;
        for peer in &mut self.peers {peer.1=None;}
    }
    pub fn merchant_present(&self)->bool {self.stage==Stage::Service&&self.all(|r|r.arrived&&!r.map_ready)}
    pub fn merchant_ready(&self)->bool {self.merchant_present()&&self.all(|r|!r.alive||r.merchant_bound)&&self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.alive))}
    pub fn boss_test(&mut self,run:&Run,test:crate::debug::BossTest)->RuleResult<Order> {
        if self.events_unsettled()||!matches!(self.stage,Stage::Map|Stage::Reward|Stage::Service)
            ||run.phase!=Phase::Map||!self.all(|r|r.testing&&r.alive&&r.map_ready&&r.law==run.law&&r.seed==run.seed&&r.act==run.act&&r.visited==run.visited) {
            return Err("Boss 测试需全员开启调试、存活，并完成领奖与誓约选择后回到路线图".into());
        }
        let mut next=run.clone();next.debug_boss_test(test.encounter,test.act)?;
        self.epoch=self.epoch.checked_add(1).ok_or("队伍序号耗尽")?;self.stage=Stage::Travel;
        self.events=None;self.stormveil=None;self.trial=None;
        for p in &mut self.peers {p.1=None;}
        Ok(Order::BossTest{epoch:self.epoch,seed:run.seed,test})
    }
    pub fn event_test(&mut self,run:&Run,draw:crate::events::Draw)->RuleResult<Order> {
        if self.events_unsettled()||!matches!(self.stage,Stage::Map|Stage::Reward|Stage::Service)
            ||run.phase!=Phase::Map||!self.all(|r|r.event_testing&&r.testing&&r.alive&&r.map_ready&&r.law==run.law&&r.seed==run.seed&&r.act==run.act&&r.visited==run.visited) {
            return Err("事件测试需全员更新版本、开启调试、存活，并完成领奖与誓约选择后回到路线图".into());
        }
        let mut next=run.clone();next.debug_event_test(draw)?;
        let has_stonesword_key=self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.has_stonesword_key));
        self.epoch=self.epoch.checked_add(1).ok_or("队伍序号耗尽")?;self.stage=Stage::Service;
        self.events=None;self.stormveil=None;self.trial=None;
        for p in &mut self.peers {p.1=None;}
        Ok(Order::EventTest{epoch:self.epoch,seed:run.seed,act:run.act,visited:run.visited.clone(),draw,has_stonesword_key})
    }
    pub fn law_order(&self,run:&Run)->RuleResult<Option<Order>> {
        if !self.all(|r|r.seed==run.seed&&r.act==run.act&&r.visited==run.visited){return Ok(None);}
        if self.all(|r|r.law==run.law){return Ok(None);}
        if run.act!=1||!run.visited.is_empty()||run.phase!=Phase::Map {return Err("队伍苦难律法不一致，请重新开始共同旅程".into());}
        Ok(Some(Order::Law{epoch:self.epoch,seed:run.seed,law:run.law}))
    }
    pub fn flask_uses(&self)->Vec<(u64,u64)> {
        self.peers.iter().filter_map(|(id,r)|r.as_ref().filter(|r|r.present&&r.epoch==self.epoch).map(|r|(*id,r.flask_uses))).collect()
    }
    pub fn drawing_requests(&self)->Vec<(u64,crate::map_drawing::Request)> {
        self.peers.iter().filter_map(|(id,r)|r.as_ref().filter(|r|r.present&&r.alive)
            .and_then(|r|r.drawing.as_ref().filter(|d|d.scope.seed==r.seed&&d.scope.act==r.act)).map(|d|(*id,d.clone()))).collect()
    }
    pub fn rest_end(&mut self)->Option<Order> {
        if self.stage==Stage::Stopped||!self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.epoch==self.epoch&&r.rest_end)){return None;}
        self.stage=Stage::Stopped;Some(Order::Defeat{epoch:self.epoch})
    }
    pub fn ballots(&self)->Vec<crate::party_vote::Ballot> {
        self.peers.iter().map(|(player,report)|crate::party_vote::Ballot{player:*player,node:report.as_ref().filter(|r|r.present&&r.map_ready&&r.alive).and_then(|r|r.selected)}).collect()
    }
    pub fn new(ids:Vec<u64>)->Self {Self{trial:None,epoch:0,stage:Stage::Map,peers:ids.into_iter().map(|id|(id,None)).collect(),events:None,stormveil:None}}
    pub fn trial_order(&mut self)->Option<Order> {
        use crate::regional_events::Trial;
        if self.stage!=Stage::Service||self.trial.is_some()||!self.all(|r|r.trial&&r.arrived&&r.alive&&r.trial_choice.is_some()){return None;}
        let first=self.peers.first()?.1.as_ref()?;
        if !self.all(|r|r.seed==first.seed&&r.act==first.act&&r.visited==first.visited){return None;}
        let options=[Trial::Descent,Trial::Stormveil,Trial::Chariot,Trial::Skip,Trial::Mausoleum,Trial::HereticalRise];
        let counts=options.map(|c|self.peers.iter().filter(|(_,r)|r.as_ref().is_some_and(|r|r.trial_choice==Some(c))).count());
        let best=*counts.iter().max()?;
        let tied:Vec<_>=options.into_iter().zip(counts).filter_map(|(c,n)|(n==best).then_some(c)).collect();
        let choice=tied[crate::mix(first.seed^self.epoch^0x747269616c) as usize%tied.len()];
        self.trial=Some(choice);
        if matches!(choice,Trial::Stormveil|Trial::Chariot|Trial::HereticalRise){self.stormveil=Some(crate::stormveil::Decision::Challenge);}
        Some(Order::Trial{epoch:self.epoch,choice})
    }
    pub fn stormveil_order(&mut self)->Option<Order> {
        use crate::stormveil::{Progress,Decision};
        if self.stage!=Stage::Service{return None;}
        let chariot=self.peers.first()?.1.as_ref()?.chariot;
        if !self.all(|r|r.chariot==chariot){return None;}
        let decision=if self.stormveil.is_none()&&self.all(|r|r.arrived&&r.alive&&r.stormveil==Some(Progress::Choosing)) {
            if self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.storm_choice==Some(false))) {Decision::Skip}
            else if self.all(|r|r.storm_choice==Some(true)){Decision::Challenge}else{return None;}
        }else if chariot&&self.stormveil==Some(Decision::Challenge)&&self.all(|r|!r.alive||r.chariot_ready&&r.stormveil==Some(Progress::Running))
            &&self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.alive&&r.chariot_ready)){Decision::Engage}
        else if self.stormveil==Some(if chariot{Decision::Engage}else{Decision::Challenge})&&self.all(|r|r.stormveil==Some(Progress::Running)&&(!r.alive||r.storm_finish))
            &&self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.alive&&r.storm_finish)){Decision::Complete}
        else{return None;};
        self.stormveil=Some(decision);Some(Order::Stormveil{epoch:self.epoch,decision})
    }
    pub fn descent_order(&mut self)->Option<Order> {
        if self.stage!=Stage::Service||self.trial!=Some(crate::regional_events::Trial::Descent)
            ||self.stormveil.is_some()||!self.all(|r|!r.alive||r.storm_finish)
            ||!self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.alive&&r.storm_finish)){return None;}
        self.stormveil=Some(crate::stormveil::Decision::Complete);
        Some(Order::DescentComplete{epoch:self.epoch})
    }
    pub fn mausoleum_order(&mut self)->Option<Order> {
        if self.stage!=Stage::Service||self.trial!=Some(crate::regional_events::Trial::Mausoleum)
            ||self.stormveil.is_some()||!self.all(|r|r.mausoleum_open.is_some())
            ||!self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.alive&&r.mausoleum_open==Some(true))){return None;}
        self.stormveil=Some(crate::stormveil::Decision::Complete);
        Some(Order::MausoleumOpen{epoch:self.epoch})
    }
    pub fn restart(&mut self,seed:u64)->RuleResult<Option<Order>> {
        if self.epoch==0||!matches!(self.stage,Stage::Stopped|Stage::Reward)||!self.all(|r|r.at_hub&&r.alive){return Ok(None);}
        if seed==0{return Err("新局种子无效".into());}
        self.epoch=self.epoch.checked_add(1).ok_or("队伍序号耗尽")?;
        self.stage=Stage::Map;self.events=None;self.stormveil=None;self.trial=None;for p in &mut self.peers {p.1=None;}
        Ok(Some(Order::NewRun{epoch:self.epoch,seed}))
    }
    /// 房主用固定种子开局：只在地图阶段、每个人都还在大赐福或刚开的新旅程（第一幕、没走过任何节点）时可以换种子
    pub fn reseed(&self,seed:u64)->RuleResult<Order> {
        if seed==0 {return Err("种子无效".into());}
        if self.stage!=Stage::Map {return Err("队伍正在进行中，不能更换种子".into());}
        let fresh=|r:&Report|!r.present||r.epoch!=self.epoch||r.at_hub||(r.act==1&&r.visited.is_empty());
        if !self.peers.iter().all(|(_,r)|r.as_ref().is_none_or(fresh)) {return Err("有队友已经出发，不能更换种子".into());}
        Ok(Order::Reseed{epoch:self.epoch,seed})
    }
    pub fn begin_events(&mut self)->RuleResult<()> {
        if self.stage!=Stage::Service||self.events.is_some(){return Err("事件房尚未进入或已经初始化".into());}
        self.events=Some(crate::party_events::Queue::new(self.epoch,self.peers.iter().map(|p|p.0).collect())?);Ok(())
    }
    pub fn event_start(&mut self)->RuleResult<Option<Order>> {
        if self.stage!=Stage::Service||!self.all(|r|r.arrived&&r.alive){return Ok(None);}
        if self.events.as_ref().and_then(|q|q.completed()).is_some_and(|j|!self.all(|r|r.event_applied==Some(j.wave))){return Ok(None);}
        let Some(events)=&mut self.events else{return Ok(None);};
        for (player,report) in &self.peers {
            if let Some(kind)=report.as_ref().and_then(|r|r.event_request){let _=events.request(self.epoch,*player,kind);}
        }
        Ok(events.start().map(Order::EventStart))
    }
    pub fn scarab_owners(&self,wave:u8,killed:u8,known:[u64;3])->Option<[u64;3]> {
        let job=self.events.as_ref()?.active()?;
        if self.stage!=Stage::Service||job.wave!=wave||killed>7||!matches!(job.kind,crate::party_events::Kind::Scarabs(_))
            ||!self.all(|r|r.event_bound==Some(wave)&&(!r.alive||r.scarab_claim.is_some_and(|c|c.valid()&&c.wave==wave&&c.finished))){return None;}
        Some(std::array::from_fn(|slot|{
            if killed&(1<<slot)==0{return 0;}
            if known[slot]!=0&&self.peers.iter().any(|(id,_)|*id==known[slot]){return known[slot];}
            let mut claims=self.peers.iter().filter_map(|(id,r)|r.as_ref().and_then(|r|r.scarab_claim)
                .filter(|c|c.valid()&&c.wave==wave&&c.owned&(1<<slot)!=0).map(|_|*id));
            let owner=claims.next().unwrap_or(0);
            if claims.next().is_some(){0}else{owner}
        }))
    }
    pub fn event_done(&mut self,wave:u8,killed:u8)->RuleResult<Order> {
        self.event_done_owned(wave,killed,[0;3])
    }
    pub fn event_done_owned(&mut self,wave:u8,killed:u8,owners:[u64;3])->RuleResult<Order> {
        let job=self.events.as_ref().and_then(|q|q.active()).ok_or("没有活动事件波次")?;
        if self.stage!=Stage::Service||wave!=job.wave||killed>7||!self.all(|r|r.event_bound==Some(wave))
            ||(matches!(job.kind,crate::party_events::Kind::Guards(_))&&(killed!=0||owners!=[0;3]))
            ||owners.iter().enumerate().any(|(slot,owner)|*owner!=0&&(killed&(1<<slot)==0||!self.peers.iter().any(|(id,_)|id==owner))){return Err("事件波次尚未同步或结算无效".into());}
        self.events.as_mut().unwrap().finish(self.epoch,wave)?;
        Ok(Order::EventDone{epoch:self.epoch,wave,killed,owners})
    }
    pub fn event_bound(&self,wave:u8)->bool {
        self.stage==Stage::Service&&self.events.as_ref().and_then(|q|q.active()).is_some_and(|j|j.wave==wave)
            &&self.all(|r|r.event_bound==Some(wave))
    }
    pub fn event_abort(&mut self,wave:u8)->RuleResult<Order> {
        let job=self.events.as_ref().and_then(|q|q.active()).ok_or("没有活动事件")?;
        if self.stage!=Stage::Service||job.wave!=wave||!matches!(job.kind,crate::party_events::Kind::Scarabs(_)){return Err("不能取消此事件".into());}
        self.events.as_mut().unwrap().cancel(self.epoch,wave)?;
        Ok(Order::EventAborted{epoch:self.epoch,wave})
    }
    fn events_unsettled(&self)->bool {
        self.events.as_ref().is_some_and(|q|q.busy()||q.completed().is_some_and(|j|!self.all(|r|r.event_applied==Some(j.wave))))
    }
    pub fn disconnect(&mut self,id:u64){if let Some(p)=self.peers.iter_mut().find(|p|p.0==id){p.1=None;}}
    /// 队员退出了无缝会话（不是房主）：从队伍里去掉，之后的“全员”判断（出发、开战、胜利、领奖、投票）不再等他
    pub fn remove(&mut self,id:u64)->bool {let before=self.peers.len();self.peers.retain(|p|p.0!=id);self.peers.len()<before}
    pub fn report(&mut self,id:u64,report:Report)->RuleResult<()> {
        if report.resume.as_ref().is_some_and(|p|p.visited.len()>64||!(1..=3).contains(&p.act)||p.law.len()>16){return Err("暂存旅程信息无效".into());}
        if report.visited.len()>64||report.act==0||report.act>3||report.epoch!=self.epoch||!report.law.valid(){return Err("队员旅程进度或律法不匹配".into());}
        if report.event_request.is_some_and(|k|!k.valid()){return Err("队员事件请求无效".into());}
        if report.scarab_claim.is_some_and(|c|!c.valid()){return Err("粪金龟击杀汇报无效".into());}
        if report.embers.len()>crate::embers::Ember::ALL.len()||report.strikes.len()>crate::embers::STRIKE_WINDOW{return Err("队员余烬汇报过长".into());}
        self.peers.iter_mut().find(|p|p.0==id).ok_or("未知队员")?.1=Some(report);Ok(())
    }
    fn all(&self,f:impl Fn(&Report)->bool)->bool {
        !self.peers.is_empty()&&self.peers.iter().all(|p|p.1.as_ref().is_some_and(|r|r.present&&r.epoch==self.epoch&&f(r)))
    }
    pub fn enter(&mut self,run:&Run,node:usize)->RuleResult<Order> {
        if self.events_unsettled()||!matches!(self.stage,Stage::Map|Stage::Reward|Stage::Service)||!run.available().contains(&node)
            ||!self.all(|r|r.alive&&r.map_ready&&r.law==run.law&&r.seed==run.seed&&r.act==run.act&&r.visited==run.visited){return Err("等待全员同步律法并完成奖励与契约选择".into());}
        let has_stonesword_key=self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.has_stonesword_key));
        self.epoch=self.epoch.checked_add(1).ok_or("队伍序号耗尽")?;
        self.stage=if run.nodes[node].kind.combat(){Stage::Travel}else{Stage::Service};
        self.events=None;self.stormveil=None;self.trial=None;
        for p in &mut self.peers {p.1=None;}
        Ok(Order::Enter{epoch:self.epoch,act:run.act,visited:run.visited.clone(),node,has_stonesword_key})
    }
    pub fn resolve_votes(&mut self,run:&Run,entropy:u64)->RuleResult<Option<Order>> {
        if self.events_unsettled()||!matches!(self.stage,Stage::Map|Stage::Reward|Stage::Service)
            ||!self.all(|r|r.alive&&r.map_ready&&r.law==run.law&&r.seed==run.seed&&r.act==run.act&&r.visited==run.visited){return Ok(None);}
        let ids:Vec<_>=self.peers.iter().map(|p|p.0).collect();let available=run.available();
        let mut vote=crate::party_vote::Vote::new(self.epoch,&ids,&available)?;
        for (id,report) in &self.peers {
            let Some(node)=report.as_ref().and_then(|r|r.selected).filter(|n|available.contains(n)) else{return Ok(None);};
            vote.ready(self.epoch,*id,true)?;vote.cast(self.epoch,*id,node)?;
        }
        vote.draw(entropy).map(|node|self.enter(run,node)).transpose()
    }
    pub fn spawn(&mut self)->Option<Order> {
        if self.stage==Stage::Travel&&self.all(|r|r.arrived&&r.alive){self.stage=Stage::Combat;Some(Order::Spawn{epoch:self.epoch})}else{None}
    }
    pub fn victory(&mut self)->RuleResult<Order> {
        if self.stage!=Stage::Fighting||!self.all(|r|r.bound){return Err("等待队友确认敌人复制".into());}
        self.stage=Stage::Reward;Ok(Order::Victory{epoch:self.epoch})
    }
    pub fn defeat(&mut self)->Option<Order> {
        if matches!(self.stage,Stage::Travel|Stage::Combat|Stage::Fighting|Stage::Service)&&self.all(|r|!r.alive){self.stage=Stage::Stopped;Some(Order::Defeat{epoch:self.epoch})}else{None}
    }
    pub fn members(&self)->Vec<u64> {self.peers.iter().map(|p|p.0).collect()}
    pub fn pause_ready(&self)->bool {
        let Some(first)=self.peers.first().and_then(|p|p.1.as_ref()) else{return false};
        self.all(|r|r.alive&&r.map_ready&&r.seed==first.seed&&r.act==first.act&&r.visited==first.visited&&r.law==first.law)
    }
    pub fn pause_saved(&self,ticket:u64)->bool {
        let Some(point)=self.peers.first().and_then(|p|p.1.as_ref()).and_then(|r|r.resume.as_ref()) else{return false};
        ticket!=0&&self.all(|r|r.alive&&r.pause_saved==Some(ticket)&&r.resume.as_ref()==Some(point))
    }
    pub fn leaving(&self)->Vec<u64> {self.peers.iter().filter_map(|(id,r)|r.as_ref().filter(|r|r.epoch==self.epoch&&r.present&&r.alive&&r.leaving).map(|_|*id)).collect()}
    pub fn resume_ready(&self,point:&crate::suspension::Point)->bool {self.all(|r|r.epoch==self.epoch&&r.at_hub&&r.alive&&r.resume.as_ref()==Some(point))}
    /// 成员的敌人侧余烬效果（只取本场战斗、在场的成员），以及核对用的余烬
    /// 本场在场队员持有的余烬（房主据此代为执行只作用在敌人身上、又不走汇报的余烬，如鲜血骑士的破势）
    pub fn held_embers(&self)->Vec<crate::embers::Ember> {
        self.peers.iter().filter_map(|(_,r)|r.as_ref().filter(|r|r.present&&r.epoch==self.epoch)).flat_map(|r|r.embers.iter().copied()).collect()
    }
    pub fn strikes(&self)->Vec<(u64,Vec<crate::embers::Ember>,Vec<crate::embers::StrikeRequest>)> {
        self.peers.iter().filter_map(|(id,r)|r.as_ref().filter(|r|r.present&&r.epoch==self.epoch&&!r.strikes.is_empty()).map(|r|(*id,r.embers.clone(),r.strikes.clone()))).collect()
    }
    /// 全队当前汇报的称号（没汇报过的队员不列出）
    pub fn titles(&self)->Vec<(u64,u8)> {
        self.peers.iter().filter_map(|(id,r)|r.as_ref().filter(|r|r.present).map(|r|(*id,r.title))).collect()
    }
    /// 奖励传送门：每个活着的队员都交互过才发奖励（倒下的队员不用等），至少一人进入
    pub fn rewards(&self)->Option<Order> {
        (self.stage==Stage::Reward&&self.all(|r|r.bound&&(!r.alive||r.portal))
            &&self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.alive&&r.portal))).then_some(Order::Rewards{epoch:self.epoch})
    }
    pub fn engage(&mut self)->Option<Order> {self.engage_within(None)}
    /// 开战屏障：全员确认敌人复制后开战（敌人此前在各自的游戏里不动、无敌）。
    pub fn engage_within(&mut self,_host_ready_ms:Option<u64>)->Option<Order> {
        if self.stage!=Stage::Combat {return None;}
        let ready=self.all(|r|r.bound);
        if ready {self.stage=Stage::Fighting;Some(Order::Engage{epoch:self.epoch})}else{None}
    }
    /// 还没确认敌人复制的队员
    pub fn unbound(&self)->Vec<u64> {
        self.peers.iter().filter(|(_,r)|!r.as_ref().is_some_and(|r|r.present&&r.epoch==self.epoch&&r.bound)).map(|(id,_)|*id).collect()
    }
    pub fn stop(&mut self){self.stage=Stage::Stopped;}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descent_completion_waits_for_survivors_and_never_rewards_a_wipe() {
        for count in 2..=8 {
            let run=Run::new(42,crate::Class::Samurai);
            let mut gate=Gate::new((1..=count).collect());gate.stage=Stage::Service;
            gate.trial=Some(crate::regional_events::Trial::Descent);
            let mut r=Report::new(&run,0,true,false,false);
            for id in 1..=count {gate.report(id,r.clone()).unwrap();}
            assert!(gate.descent_order().is_none());
            r.alive=true;gate.report(count,r.clone()).unwrap();
            assert!(gate.descent_order().is_none());
            r.storm_finish=true;gate.report(count,r.clone()).unwrap();
            gate.disconnect(1);assert!(gate.descent_order().is_none());
            let mut dead=r.clone();dead.alive=false;dead.storm_finish=false;gate.report(1,dead).unwrap();
            assert!(matches!(gate.descent_order(),Some(Order::DescentComplete{epoch:0})));
            assert!(gate.descent_order().is_none());
        }
    }
    #[test]
    fn chariot_start_ignores_dead_peers_but_waits_for_living_peers() {
        use crate::stormveil::{Progress,Decision};
        let run=Run::new(42,crate::Class::Samurai);
        let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Service;gate.stormveil=Some(Decision::Challenge);
        let mut r=Report::new(&run,0,true,false,true);r.chariot=true;r.chariot_ready=true;r.stormveil=Some(Progress::Running);
        gate.report(1,r.clone()).unwrap();r.chariot_ready=false;gate.report(2,r.clone()).unwrap();
        assert!(gate.stormveil_order().is_none());
        r.alive=false;gate.report(2,r).unwrap();
        assert!(matches!(gate.stormveil_order(),Some(Order::Stormveil{decision:Decision::Engage,..})));
    }
    #[test]
    fn merchant_waits_for_all_surviving_peers_and_rejects_departures() {
        let run=Run::new(42,crate::Class::Samurai);let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Service;
        let mut r=Report::new(&run,0,true,false,true);r.map_ready=false;r.merchant_bound=true;
        gate.report(1,r.clone()).unwrap();assert!(!gate.merchant_ready());
        let mut other=r.clone();other.merchant_bound=false;gate.report(2,other.clone()).unwrap();assert!(!gate.merchant_ready());
        other.alive=false;gate.report(2,other.clone()).unwrap();assert!(gate.merchant_ready());
        other.map_ready=true;gate.report(2,other).unwrap();assert!(!gate.merchant_present());
        r.alive=false;r.map_ready=false;gate.report(1,r.clone()).unwrap();gate.report(2,r).unwrap();assert!(!gate.merchant_ready());
    }
    #[test]
    fn mausoleum_open_sync_accepts_either_survivor_and_never_a_wipe() {
        for survivor in [1,2] {
            let run=Run::new(42,crate::Class::Samurai);
            let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Service;
            gate.trial=Some(crate::regional_events::Trial::Mausoleum);
            let mut report=Report::new(&run,0,true,false,true);report.mausoleum_open=Some(false);
            gate.report(1,report.clone()).unwrap();gate.report(2,report.clone()).unwrap();
            assert!(gate.mausoleum_order().is_none());
            report.alive=false;report.mausoleum_open=Some(true);
            gate.report(1,report.clone()).unwrap();gate.report(2,report.clone()).unwrap();
            assert!(gate.mausoleum_order().is_none(),"no revival after a party wipe");
            report.alive=true;gate.report(survivor,report.clone()).unwrap();
            let mut old=report.clone();old.alive=false;old.mausoleum_open=None;
            gate.report(3-survivor,old.clone()).unwrap();
            assert!(gate.mausoleum_order().is_none(),"old clients lack the message handler");
            old.mausoleum_open=Some(false);gate.report(3-survivor,old).unwrap();
            gate.trial=Some(crate::regional_events::Trial::Skip);assert!(gate.mausoleum_order().is_none());
            gate.trial=Some(crate::regional_events::Trial::Mausoleum);
            let order=gate.mausoleum_order().unwrap();
            assert!(matches!(order,Order::MausoleumOpen{epoch:0}));
            assert!(matches!(serde_json::from_str::<Order>(&serde_json::to_string(&order).unwrap()).unwrap(),Order::MausoleumOpen{epoch:0}));
            assert!(gate.mausoleum_order().is_none(),"shared once per visit");
        }
    }
    #[test]
    fn event_test_requires_capability_consent_and_matching_route() {
        let run=Run::new(42,crate::Class::Samurai);let draw=crate::events::Draw::Mausoleum;
        let mut ready=Report::new(&run,0,false,false,true);ready.testing=true;ready.event_testing=true;
        let mut gate=Gate::new(vec![1,2]);gate.report(1,ready.clone()).unwrap();
        assert!(gate.event_test(&run,draw).is_err());
        for field in ["event_testing","testing","alive","map_ready","seed","act","visited"] {
            let mut other=serde_json::to_value(&ready).unwrap();
            other[field]=match field {"seed"=>43.into(),"act"=>2.into(),"visited"=>serde_json::json!([0]),_=>false.into()};
            gate.report(2,serde_json::from_value(other).unwrap()).unwrap();
            assert!(gate.event_test(&run,draw).is_err(),"{field}");assert_eq!(gate.epoch,0);
        }
        ready.has_stonesword_key=true;gate.report(2,ready.clone()).unwrap();
        let order=gate.event_test(&run,draw).unwrap();
        let wire=serde_json::to_string(&order).unwrap();
        assert!(matches!(serde_json::from_str::<Order>(&wire).unwrap(),Order::EventTest{epoch:1,seed:42,act:1,draw:crate::events::Draw::Mausoleum,has_stonesword_key:true,..}));
        assert_eq!(gate.stage,Stage::Service);assert!(gate.event_test(&run,draw).is_err());
        gate.begin_events().unwrap();assert_eq!(gate.stage,Stage::Service);
        let mut old=serde_json::to_value(ready).unwrap();old.as_object_mut().unwrap().remove("event_testing");
        assert!(!serde_json::from_value::<Report>(old).unwrap().event_testing);
        let mut host=run.clone();let mut guest=Run::new(42,crate::Class::Astrologer);
        let node=host.debug_event_test(draw).unwrap();assert_eq!(guest.debug_event_test(draw).unwrap(),node);
        host.enter(node).unwrap();guest.enter(node).unwrap();
        assert!(host.debug&&guest.debug);assert_eq!(host.event_draw(node),guest.event_draw(node));
        assert_eq!(Run::decode(&guest.encode().unwrap()).unwrap().event_draw(node),draw);
    }
    #[test]
    fn boss_test_requires_all_peers_to_opt_in_and_be_ready() {
        let run=Run::new(42,crate::Class::Samurai);let test=crate::debug::BossTest{encounter:3,act:2};
        let mut gate=Gate::new(vec![1,2]);
        let mut ready=Report::new(&run,0,false,false,true);ready.testing=true;
        gate.report(1,ready.clone()).unwrap();
        assert!(gate.boss_test(&run,test).is_err());assert_eq!(gate.epoch,0);
        let mut other=ready.clone();other.testing=false;gate.report(2,other).unwrap();
        assert!(gate.boss_test(&run,test).is_err());assert_eq!(gate.epoch,0);
        gate.report(2,ready).unwrap();
        assert!(matches!(gate.boss_test(&run,test).unwrap(),Order::BossTest{epoch:1,seed:42,test:t} if t==test));
        assert_eq!(gate.stage,Stage::Travel);
        assert!(gate.boss_test(&run,test).is_err());assert_eq!(gate.epoch,1);
    }
    #[test]
    fn suspension_barriers_require_every_member_and_matching_progress() {
        let run=Run::new(42,crate::Class::Samurai);let point=crate::suspension::Point::of(&run).unwrap();
        let mut gate=Gate::new(vec![1,2]);let mut report=Report::new(&run,0,false,false,true);
        gate.report(1,report.clone()).unwrap();assert!(!gate.pause_ready());
        gate.report(2,report.clone()).unwrap();assert!(gate.pause_ready());
        report.pause_saved=Some(9);report.resume=Some(point.clone());gate.report(1,report.clone()).unwrap();assert!(!gate.pause_saved(9));
        let mut wrong=report.clone();wrong.resume.as_mut().unwrap().seed=43;gate.report(2,wrong).unwrap();assert!(!gate.pause_saved(9));
        gate.report(2,report).unwrap();assert!(gate.pause_saved(9));assert!(!gate.pause_saved(10));
        let mut hub=Report::at_hub(0);hub.resume=Some(point.clone());gate.report(1,hub.clone()).unwrap();assert!(!gate.resume_ready(&point));
        gate.report(2,hub.clone()).unwrap();assert!(gate.resume_ready(&point));
        hub.alive=false;gate.report(2,hub).unwrap();assert!(!gate.resume_ready(&point));
    }
    #[test]
    fn titles_are_collected_from_every_present_member() {
        let run=Run::new(42,crate::Class::Samurai);
        let mut gate=Gate::new(vec![3,1,2]);
        assert!(gate.titles().is_empty());
        let mut a=Report::new(&run,0,false,false,true);a.title=2;gate.report(1,a).unwrap();
        let mut b=Report::at_hub(0);b.title=7;gate.report(3,b).unwrap();
        gate.report(2,Report::unavailable(0)).unwrap();
        assert_eq!(gate.titles(),vec![(3,7),(1,2)],"members that are loading are left out");
        assert_eq!(gate.members(),vec![3,1,2]);
    }
    #[test]
    fn member_strikes_reach_the_host_with_the_embers_to_check_them() {
        use crate::embers::{Ember,Strike,StrikeRequest,STRIKE_WINDOW};
        let mut run=Run::new(42,crate::Class::Samurai);run.embers=vec![Ember::Bloodlord];
        let mut gate=Gate::new(vec![1,2,3]);
        let strike=StrikeRequest{seq:1,strike:Strike::Buildup{slot:0,ember:Ember::Bloodlord}};
        let mut a=Report::new(&run,0,true,true,true);a.strikes=vec![strike];gate.report(2,a.clone()).unwrap();
        gate.report(3,Report::new(&run,0,true,true,true)).unwrap();
        assert_eq!(gate.strikes(),vec![(2,vec![Ember::Bloodlord],vec![strike])],"members without strikes are left out");
        let mut stale=a.clone();stale.epoch=1;assert!(gate.report(2,stale).is_err(),"another fight");
        let mut long=a.clone();long.strikes=vec![strike;STRIKE_WINDOW+1];assert!(gate.report(2,long).is_err());
        let mut away=a;away.present=false;gate.report(2,away).unwrap();assert!(gate.strikes().is_empty());
    }
    #[test]
    fn reward_portal_waits_for_every_living_member_two_to_eight_players() {
        for count in 2..=crate::party_roster::MAX_PLAYERS as u64 {
            let run=Run::new(42,crate::Class::Samurai);
            let mut gate=Gate::new((1..=count).collect());gate.stage=Stage::Fighting;
            for id in 1..=count {gate.report(id,Report::new(&run,0,true,true,true)).unwrap();}
            gate.victory().unwrap();
            assert!(gate.rewards().is_none());
            let mut touched=Report::new(&run,0,true,true,true);touched.portal=true;
            for id in 1..count {gate.report(id,touched.clone()).unwrap();assert!(gate.rewards().is_none());}
            let mut dead=Report::new(&run,0,true,true,false);
            gate.report(count,dead.clone()).unwrap();
            assert!(matches!(gate.rewards(),Some(Order::Rewards{epoch:0})));
            gate.report(count,touched.clone()).unwrap();assert!(gate.rewards().is_some());
            gate.disconnect(count);assert!(gate.rewards().is_none());
            for id in 1..=count {gate.report(id,dead.clone()).unwrap();}
            assert!(gate.rewards().is_none());
            dead.portal=true;gate.stage=Stage::Fighting;
            for id in 1..=count {gate.report(id,touched.clone()).unwrap();}
            assert!(gate.rewards().is_none(),"rewards only after victory");
        }
    }
    #[test]
    fn key_eligibility_is_committed_from_any_current_member() {
        for count in 2..=8 {for holder in 0..=count {
            let run=Run::new(42,crate::Class::Samurai);let node=run.available()[0];
            let mut gate=Gate::new((1..=count).collect());
            for id in 1..=count {
                let mut report=Report::new(&run,0,false,false,true);
                report.has_stonesword_key=id==holder;gate.report(id,report).unwrap();
            }
            let order=gate.enter(&run,node).unwrap();
            assert!(matches!(order,Order::Enter{has_stonesword_key,..} if has_stonesword_key==(holder!=0)));
        }}
    }
    #[test]
    fn any_member_rest_ends_two_to_eight_players_once_in_every_active_stage() {
        for count in 2..=8 {for member in 1..=count {for stage in [Stage::Map,Stage::Travel,Stage::Combat,Stage::Fighting,Stage::Service,Stage::Reward] {
            let mut gate=Gate::new((1..=count).collect());gate.stage=stage;
            let mut report=Report::unavailable(0);report.rest_end=true;
            assert!(gate.report(count+1,report.clone()).is_err());
            let mut stale=report.clone();stale.epoch=1;assert!(gate.report(member,stale).is_err());
            assert!(gate.rest_end().is_none());
            gate.report(member,report).unwrap();
            assert!(matches!(gate.rest_end(),Some(Order::Defeat{epoch:0})));
            assert!(gate.rest_end().is_none());assert_eq!(gate.stage,Stage::Stopped);
        }}}
    }
    #[test]
    fn mausoleum_votes_wait_for_all_peers_and_emit_one_shared_decision(){
        use crate::regional_events::Trial;
        for count in 2..=8 {for choice in [Trial::Mausoleum,Trial::Skip] {
            let mut run=Run::new(42,crate::Class::Samurai);run.regional_event.trial=true;
            let mut gate=Gate::new((1..=count).collect());gate.stage=Stage::Service;
            let mut report=Report::new(&run,0,true,false,true);report.trial_choice=Some(choice);
            for id in 1..count {gate.report(id,report.clone()).unwrap();}
            assert!(gate.trial_order().is_none());
            let mut loading=report.clone();loading.arrived=false;gate.report(count,loading).unwrap();assert!(gate.trial_order().is_none());
            gate.report(count,report).unwrap();assert!(matches!(gate.trial_order(),Some(Order::Trial{choice:c,..}) if c==choice));
            assert!(gate.trial_order().is_none());
        }}
    }
    #[test]
    fn chariot_vote_requires_same_trial_for_two_to_eight_players() {
        use crate::stormveil::{Progress,Decision};
        for count in 2..=8 {
            let mut run=Run::new(42,crate::Class::Samurai);
            run.regional_event.stormveil=Some(crate::stormveil::Progress::Choosing);
            run.regional_event.chariot=true;
            let mut gate=Gate::new((1..=count).collect());gate.stage=Stage::Service;
            let mut report=Report::new(&run,0,true,false,true);report.storm_choice=Some(true);
            for id in 1..count {gate.report(id,report.clone()).unwrap();}
            let mut wrong=report.clone();wrong.chariot=false;
            gate.report(count,wrong).unwrap();assert!(gate.stormveil_order().is_none());
            gate.report(count,report.clone()).unwrap();
            assert!(matches!(gate.stormveil_order(),Some(Order::Stormveil{decision:crate::stormveil::Decision::Challenge,..})));
            assert!(gate.stormveil_order().is_none());
            report.stormveil=Some(Progress::Running);report.chariot_ready=true;
            for id in 1..count {gate.report(id,report.clone()).unwrap();}
            assert!(gate.stormveil_order().is_none());
            gate.report(count,report.clone()).unwrap();
            assert!(matches!(gate.stormveil_order(),Some(Order::Stormveil{decision:Decision::Engage,..})));
            assert!(gate.stormveil_order().is_none());
            report.storm_finish=true;
            for id in 1..count {gate.report(id,report.clone()).unwrap();}
            assert!(gate.stormveil_order().is_none());
            gate.report(count,report).unwrap();
            assert!(matches!(gate.stormveil_order(),Some(Order::Stormveil{decision:Decision::Complete,..})));
            assert!(gate.stormveil_order().is_none());
        }
    }
    #[test]
    fn stormveil_two_to_eight_require_unanimity_skip_veto_and_all_survivors_at_finish() {
        use crate::stormveil::{Progress,Decision};
        for count in 2..=8 {
            for skip in [false,true] {
                let run=Run::new(42,crate::Class::Samurai);
                let mut gate=Gate::new((1..=count).collect());gate.stage=Stage::Service;
                let mut r=Report::new(&run,0,true,false,true);r.stormveil=Some(Progress::Choosing);r.storm_choice=Some(true);
                for id in 1..count {gate.report(id,r.clone()).unwrap();}
                assert!(gate.stormveil_order().is_none());
                let mut last=r.clone();last.storm_choice=None;gate.report(count,last.clone()).unwrap();
                assert!(gate.stormveil_order().is_none());
                last.storm_choice=Some(!skip);gate.report(count,last).unwrap();
                assert!(matches!(gate.stormveil_order(),Some(Order::Stormveil{decision,..}) if decision==if skip{Decision::Skip}else{Decision::Challenge}));
                assert!(gate.stormveil_order().is_none());
                if skip{continue;}
                r.stormveil=Some(Progress::Running);r.storm_finish=true;
                for id in 1..count {gate.report(id,r.clone()).unwrap();}
                assert!(gate.stormveil_order().is_none());
                let mut last=r.clone();last.storm_finish=false;gate.report(count,last.clone()).unwrap();
                assert!(gate.stormveil_order().is_none());
                gate.disconnect(count);assert!(gate.stormveil_order().is_none());
                last.alive=false;gate.report(count,last).unwrap();
                assert!(matches!(gate.stormveil_order(),Some(Order::Stormveil{decision:Decision::Complete,..})));
                assert!(gate.stormveil_order().is_none());
            }
        }
    }
    #[test]
    fn two_to_eight_players_wait_for_last_vote_arrival_replica_and_restart() {
        for count in 2..=crate::party_roster::MAX_PLAYERS as u64 {
            let run=Run::new(42,crate::Class::Samurai);let node=run.available()[0];
            let mut gate=Gate::new((1..=count).collect());
            let mut ready=Report::new(&run,0,false,false,true);ready.selected=Some(node);
            for id in (1..count).rev() {gate.report(id,ready.clone()).unwrap();}
            assert!(gate.resolve_votes(&run,100).unwrap().is_none());
            gate.report(count,ready).unwrap();
            assert!(matches!(gate.resolve_votes(&run,100).unwrap(),Some(Order::Enter{epoch:1,..})));
            for id in 1..count {gate.report(id,Report::new(&run,1,true,true,true)).unwrap();}
            gate.report(count,Report::unavailable(1)).unwrap();assert!(gate.spawn().is_none());
            gate.report(count,Report::new(&run,1,true,false,true)).unwrap();
            assert!(gate.spawn().is_some());assert!(gate.spawn().is_none());
            assert!(gate.engage().is_none());assert!(gate.victory().is_err());
            gate.report(count,Report::new(&run,1,true,true,true)).unwrap();
            assert!(gate.engage().is_some());assert!(gate.engage().is_none());
            for id in 1..count {gate.report(id,Report::new(&run,1,true,true,false)).unwrap();}
            assert!(gate.defeat().is_none());
            assert!(gate.victory().is_ok());assert!(gate.victory().is_err());
            for id in 1..count {gate.report(id,Report::at_hub(1)).unwrap();}
            assert!(gate.restart(43).unwrap().is_none());
            gate.report(count,Report::at_hub(1)).unwrap();
            assert!(matches!(gate.restart(43).unwrap(),Some(Order::NewRun{epoch:2,seed:43})));
        }
    }
    #[test]
    fn ended_party_restarts_only_after_every_player_returns_to_hub() {
        let mut gate=Gate::new(vec![1,2]);gate.epoch=4;gate.stage=Stage::Stopped;
        let hub=Report::at_hub(4);
        gate.report(1,hub.clone()).unwrap();assert!(gate.restart(99).unwrap().is_none());
        gate.report(2,Report::unavailable(4)).unwrap();assert!(gate.restart(99).unwrap().is_none());
        gate.report(2,hub).unwrap();
        assert!(matches!(gate.restart(99).unwrap(),Some(Order::NewRun{epoch:5,seed:99})));
        assert_eq!(gate.stage,Stage::Map);assert!(gate.restart(100).unwrap().is_none());
        let run=Run::new(99,crate::Class::Samurai);let mut report=Report::new(&run,5,false,false,true);
        report.selected=Some(run.available()[0]);
        gate.report(1,report.clone()).unwrap();assert!(gate.resolve_votes(&run,0).unwrap().is_none());
        gate.report(2,report).unwrap();assert!(matches!(gate.resolve_votes(&run,0).unwrap(),Some(Order::Enter{epoch:6,..})));
        let mut won=Gate::new(vec![1]);won.epoch=8;won.stage=Stage::Reward;
        won.report(1,Report::at_hub(8)).unwrap();assert!(won.restart(101).unwrap().is_some());
    }
    #[test]
    fn guest_guard_request_waits_for_both_replicas_and_both_completion_receipts() {
        let run=Run::new(42,crate::Class::Samurai);
        let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Service;gate.begin_events().unwrap();
        let host=Report::new(&run,0,true,false,true);
        let mut guest=host.clone();guest.event_request=Some(crate::party_events::Kind::Guards(1));
        gate.report(1,host.clone()).unwrap();gate.report(2,guest.clone()).unwrap();
        let Some(Order::EventStart(job))=gate.event_start().unwrap() else{panic!("guest event missing")};
        assert_eq!(job.player,2);
        let mut host=host;host.event_bound=Some(job.wave);gate.report(1,host.clone()).unwrap();
        assert!(!gate.event_bound(job.wave));assert!(gate.event_done(job.wave,0).is_err());
        guest.event_bound=Some(job.wave);gate.report(2,guest.clone()).unwrap();assert!(gate.event_bound(job.wave));
        assert!(gate.event_done(job.wave,1).is_err());gate.event_done(job.wave,0).unwrap();
        host.event_applied=Some(job.wave);gate.report(1,host.clone()).unwrap();
        assert!(gate.enter(&run,run.available()[0]).is_err());
        guest.event_applied=Some(job.wave);guest.event_request=None;gate.report(2,guest).unwrap();
        assert!(!gate.events_unsettled());assert!(gate.event_start().unwrap().is_none());
    }
    #[test]
    fn combat_engages_once_only_after_every_present_peer_binds() {
        let run=Run::new(42,crate::Class::Samurai);let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Combat;
        gate.report(1,Report::new(&run,0,true,true,true)).unwrap();
        gate.report(2,Report::new(&run,0,true,false,true)).unwrap();
        assert!(gate.engage().is_none());assert!(gate.victory().is_err());
        gate.report(2,Report::unavailable(0)).unwrap();assert!(gate.engage().is_none());
        gate.report(2,Report::new(&run,0,true,true,true)).unwrap();
        assert!(gate.victory().is_err());
        assert!(matches!(gate.engage(),Some(Order::Engage{epoch:0})));
        assert!(gate.engage().is_none());assert!(gate.victory().is_ok());
    }
    #[test]
    fn a_member_who_quits_mid_fight_no_longer_blocks_the_party() {
        let run=Run::new(42,crate::Class::Samurai);let mut gate=Gate::new(vec![1,2,3]);gate.stage=Stage::Combat;
        gate.report(1,Report::new(&run,0,true,true,true)).unwrap();
        gate.report(2,Report::new(&run,0,true,true,true)).unwrap();
        assert!(gate.engage().is_none(),"member 3 never reported");
        assert!(gate.remove(3));assert!(!gate.remove(3));assert_eq!(gate.members(),vec![1,2]);
        assert!(matches!(gate.engage(),Some(Order::Engage{epoch:0})));
        assert!(gate.victory().is_ok(),"victory no longer waits for the player who left");
    }
    #[test]
    fn slow_members_must_bind_before_any_fight_starts() {
        let run=Run::new(42,crate::Class::Samurai);let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Combat;
        gate.report(1,Report::new(&run,0,true,true,true)).unwrap();
        gate.report(2,Report::new(&run,0,true,false,true)).unwrap();
        assert_eq!(gate.unbound(),vec![2]);
        assert!(gate.engage_within(None).is_none(),"host enemies not ready yet");
        for elapsed in [1499,1500,10_000,21_500,90_000] {
            assert!(gate.engage_within(Some(elapsed)).is_none(),"unbound guest must not be bypassed after {elapsed} ms");
        }
        gate.report(2,Report::new(&run,0,true,true,true)).unwrap();
        assert!(matches!(gate.engage_within(Some(21_501)),Some(Order::Engage{epoch:0})));
        assert!(gate.victory().is_ok());
        // 队员掉线（没有汇报）时不靠宽限开战
        let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Combat;
        gate.report(1,Report::new(&run,0,true,true,true)).unwrap();
        assert!(gate.engage_within(Some(10_000)).is_none());
    }
    #[test]
    fn failed_hunt_refund_waits_for_ack_before_retry() {
        let run=Run::new(42,crate::Class::Samurai);
        let mut gate=Gate::new(vec![1]);gate.stage=Stage::Service;gate.begin_events().unwrap();
        let mut report=Report::new(&run,0,true,false,true);report.event_request=Some(crate::party_events::Kind::Scarabs(1));
        gate.report(1,report.clone()).unwrap();
        let Some(Order::EventStart(first))=gate.event_start().unwrap() else{panic!()};
        assert!(matches!(gate.event_abort(first.wave).unwrap(),Order::EventAborted{..}));
        assert!(gate.event_abort(first.wave).is_err());assert!(gate.event_start().unwrap().is_none());
        report.event_applied=Some(first.wave);report.event_request=None;gate.report(1,report.clone()).unwrap();
        assert!(gate.event_start().unwrap().is_none());
        report.event_request=Some(crate::party_events::Kind::Scarabs(1));gate.report(1,report).unwrap();
        let Some(Order::EventStart(retry))=gate.event_start().unwrap() else{panic!()};
        assert_eq!(retry.wave,first.wave+1);
    }
    #[test]
    fn event_waves_wait_for_arrival_and_binding_of_every_member() {
        let run=Run::new(42,crate::Class::Samurai);
        let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Service;gate.begin_events().unwrap();
        let mut report=Report::new(&run,0,true,false,true);
        report.event_request=Some(crate::party_events::Kind::Scarabs(1));
        gate.report(1,report.clone()).unwrap();assert!(gate.event_start().unwrap().is_none());
        gate.report(2,report.clone()).unwrap();
        let Some(Order::EventStart(first))=gate.event_start().unwrap() else{panic!("missing wave")};
        assert_eq!(first.player,1);assert!(gate.event_start().unwrap().is_none());
        assert!(gate.event_done(first.wave,5).is_err());
        report.event_bound=Some(first.wave);gate.report(1,report.clone()).unwrap();
        assert!(!gate.event_bound(first.wave));
        assert!(gate.event_done(first.wave,5).is_err());
        gate.report(2,report.clone()).unwrap();assert!(gate.event_done(first.wave,8).is_err());
        assert!(gate.event_bound(first.wave));assert!(!gate.event_bound(first.wave+1));
        assert!(matches!(gate.event_done(first.wave,5).unwrap(),Order::EventDone{killed:5,..}));
        assert!(!gate.event_bound(first.wave));
        assert!(gate.event_start().unwrap().is_none());
        report.event_applied=Some(first.wave);gate.report(1,report.clone()).unwrap();
        assert!(gate.event_start().unwrap().is_none());
        gate.report(2,report.clone()).unwrap();
        let Some(Order::EventStart(second))=gate.event_start().unwrap() else{panic!("missing second wave")};
        assert_eq!(second.player,2);assert_ne!(first.wave,second.wave);
        assert!(gate.event_done(second.wave,0).is_err());
        for id in [1,2] {report.event_bound=Some(second.wave);gate.report(id,report.clone()).unwrap();}
        gate.event_done(second.wave,0).unwrap();
        report.selected=Some(run.available()[0]);
        for id in [1,2] {gate.report(id,report.clone()).unwrap();}
        assert!(gate.resolve_votes(&run,0).unwrap().is_none());
        assert!(gate.enter(&run,run.available()[0]).is_err());
        for id in [1,2] {report.event_applied=Some(second.wave);gate.report(id,report.clone()).unwrap();}
        assert!(gate.event_start().unwrap().is_none());
        assert!(gate.resolve_votes(&run,0).unwrap().is_some());
    }
    #[test]
    fn scarab_reports_fill_missing_host_owners_independently_of_payment() {
        for payer in [1,2] {
            let run=Run::new(42,crate::Class::Samurai);
            let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Service;gate.begin_events().unwrap();
            let mut host=Report::new(&run,0,true,false,true);let mut guest=host.clone();
            if payer==1{host.event_request=Some(crate::party_events::Kind::Scarabs(1));}else{guest.event_request=Some(crate::party_events::Kind::Scarabs(1));}
            gate.report(1,host.clone()).unwrap();gate.report(2,guest.clone()).unwrap();
            let Some(Order::EventStart(job))=gate.event_start().unwrap() else{panic!()};assert_eq!(job.player,payer);
            host.event_bound=Some(job.wave);guest.event_bound=Some(job.wave);
            host.scarab_claim=Some(ScarabClaim{wave:job.wave,killed:7,owned:0,finished:true});
            gate.report(1,host.clone()).unwrap();gate.report(2,guest.clone()).unwrap();
            assert_eq!(gate.scarab_owners(job.wave,7,[0;3]),None,"wait for the member's result");
            guest.scarab_claim=Some(ScarabClaim{wave:job.wave+1,killed:7,owned:7,finished:true});gate.report(2,guest.clone()).unwrap();
            assert_eq!(gate.scarab_owners(job.wave,7,[0;3]),None,"reject another wave");
            guest.scarab_claim.as_mut().unwrap().wave=job.wave;guest.scarab_claim.as_mut().unwrap().finished=false;gate.report(2,guest.clone()).unwrap();
            assert_eq!(gate.scarab_owners(job.wave,7,[0;3]),None,"do not settle before late kills arrive");
            guest.scarab_claim.as_mut().unwrap().finished=true;gate.report(2,guest.clone()).unwrap();
            assert_eq!(gate.scarab_owners(job.wave,7,[0;3]),Some([2;3]));
            assert_eq!(gate.scarab_owners(job.wave,7,[0,1,0]),Some([2,1,2]),"host-confirmed last hits take precedence");
            assert_eq!(gate.scarab_owners(job.wave,5,[0;3]),Some([2,0,2]),"only host-confirmed deaths receive rewards");
            host.scarab_claim.as_mut().unwrap().owned=1;gate.report(1,host.clone()).unwrap();
            assert_eq!(gate.scarab_owners(job.wave,7,[0;3]),Some([0,2,2]),"conflicting claims must not duplicate rewards");
            guest.scarab_claim.as_mut().unwrap().owned=8;assert!(gate.report(2,guest).is_err());
            host.scarab_claim.as_mut().unwrap().owned=0;gate.report(1,host).unwrap();
            let owners=gate.scarab_owners(job.wave,7,[0;3]).unwrap();
            assert!(matches!(gate.event_done_owned(job.wave,7,owners).unwrap(),Order::EventDone{owners:[2,2,2],..}));
            assert!(gate.event_done_owned(job.wave,7,owners).is_err());
        }
    }
    #[test]
    fn scarab_killer_can_be_nonpayer_and_invalid_owners_do_not_settle() {
        let run=Run::new(42,crate::Class::Samurai);
        let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Service;gate.begin_events().unwrap();
        let mut payer=Report::new(&run,0,true,false,true);
        payer.event_request=Some(crate::party_events::Kind::Scarabs(1));
        let mut guest=Report::new(&run,0,true,false,true);
        gate.report(1,payer.clone()).unwrap();gate.report(2,guest.clone()).unwrap();
        let Some(Order::EventStart(job))=gate.event_start().unwrap() else{panic!()};assert_eq!(job.player,1);
        payer.event_bound=Some(job.wave);guest.event_bound=Some(job.wave);
        gate.report(1,payer).unwrap();gate.report(2,guest).unwrap();
        assert!(gate.event_done_owned(job.wave,5,[9,0,1]).is_err());
        assert!(gate.event_done_owned(job.wave,5,[2,2,1]).is_err());
        let order=gate.event_done_owned(job.wave,5,[2,0,1]).unwrap();
        let restored:Order=serde_json::from_str(&serde_json::to_string(&order).unwrap()).unwrap();
        assert!(matches!(restored,Order::EventDone{killed:5,owners:[2,0,1],..}));
        assert!(gate.event_done_owned(job.wave,5,[2,0,1]).is_err());
    }
    #[test]
    fn host_can_reseed_only_before_anyone_sets_out() {
        let run=Run::new(42,crate::Class::Samurai);let mut gate=Gate::new(vec![1,2]);
        assert!(matches!(gate.reseed(77),Ok(Order::Reseed{epoch:0,seed:77})),"nobody has reported yet");
        gate.report(1,Report::at_hub(0)).unwrap();gate.report(2,Report::new(&run,0,false,false,true)).unwrap();
        assert!(gate.reseed(77).is_ok(),"one at the hub, one on a fresh route");
        assert!(gate.reseed(0).is_err());
        let mut moved=run.clone();let id=moved.available()[0];moved.finish(id);
        gate.report(2,Report::new(&moved,0,false,false,true)).unwrap();
        assert!(gate.reseed(77).is_err(),"a teammate has already set out");
        gate.report(2,Report::new(&run,0,false,false,true)).unwrap();gate.stage=Stage::Travel;
        assert!(gate.reseed(77).is_err(),"not while travelling");
        let mut aligned=run.clone();aligned.align_party_start(77).unwrap();assert_eq!((aligned.seed,aligned.nodes.clone()),(77,Run::new(77,crate::Class::Samurai).nodes));
    }
    #[test]
    fn services_never_spawn_enemies_and_wait_for_both_personal_exits() {
        for kind in [crate::NodeKind::Camp,crate::NodeKind::Shop,crate::NodeKind::Chest,crate::NodeKind::Event] {
            let (mut run,node)=(0..100).find_map(|seed|{
                let mut run=Run::new(seed,crate::Class::Samurai);
                for _ in 0..15 {
                    let available=run.available();
                    if let Some(&id)=available.iter().find(|&&id|run.nodes[id].kind==kind&&(kind!=crate::NodeKind::Event||run.event_draw(id)!=crate::events::Draw::Trial)){return Some((run,id));}
                    run.finish(available[0]);
                }
                None
            }).unwrap();
            run.validate().unwrap();let mut gate=Gate::new(vec![1,2]);
            let mut vote=Report::new(&run,0,false,false,true);vote.selected=Some(node);
            for id in [1,2] {gate.report(id,vote.clone()).unwrap();}
            assert!(gate.resolve_votes(&run,0).unwrap().is_some());assert_eq!(gate.stage,Stage::Service);
            run.enter(node).unwrap();
            for id in [1,2] {gate.report(id,Report::new(&run,1,true,false,true)).unwrap();}
            assert!(gate.spawn().is_none());assert!(gate.victory().is_err());
            match kind {crate::NodeKind::Camp=>run.leave_camp().unwrap(),crate::NodeKind::Shop=>run.leave_shop().unwrap(),crate::NodeKind::Event=>run.leave_regional_event().unwrap(),_=>run.finish_chest().unwrap()}
            let mut next=Report::new(&run,1,true,false,true);next.selected=Some(run.available()[0]);
            gate.report(1,next.clone()).unwrap();assert!(gate.resolve_votes(&run,0).unwrap().is_none());
            gate.report(2,Report::new(&run,1,true,false,true)).unwrap();assert!(gate.resolve_votes(&run,0).unwrap().is_none());
            gate.report(2,next).unwrap();
            assert!(gate.resolve_votes(&run,0).unwrap().is_some());
        }
    }
    #[test]
    fn party_seed_tracks_actual_boss_act_transition() {
        let mut run=Run::new(42,crate::Class::Samurai);
        for act in 1..=3 {
            assert_eq!(act_seed(42,act).unwrap(),run.seed);
            if act<3 {let boss=run.nodes.iter().position(|n|n.kind==crate::NodeKind::Boss).unwrap();run.finish(boss);}
        }
        assert!(act_seed(42,0).is_err());assert!(act_seed(42,4).is_err());
    }
    #[test]
    fn aligning_first_route_preserves_personal_contract_and_resources() {
        let mut run=Run::new(17,crate::Class::Astrologer);run.enable_contracts();
        run.contract.as_mut().unwrap().choose(0,run.seed,run.class).unwrap();
        run.contract.as_mut().unwrap().skip_gift().unwrap();
        run.hp=83;run.flasks=2;run.physick_empty=true;run.gold=812;
        let original=run.clone();run.align_party_start(42).unwrap();
        assert_eq!(run.nodes,Run::new(42,run.class).nodes);
        assert_eq!(run.initial_seed,Some(42));
        let mut personal=run.clone();personal.seed=original.seed;personal.initial_seed=original.initial_seed;personal.nodes=original.nodes.clone();
        assert_eq!(personal,original);run.validate().unwrap();
        let node=run.available()[0];run.enter(node).unwrap();
        let fighting=run.clone();assert!(run.align_party_start(99).is_err());assert_eq!(run,fighting);
    }
    #[test]
    fn unloading_revokes_vote_and_is_neither_arrival_nor_death() {
        let run=Run::new(42,crate::Class::Samurai);let node=run.available()[0];
        let mut gate=Gate::new(vec![1,2]);
        let mut ready=Report::new(&run,0,true,true,true);ready.selected=Some(node);
        for id in [1,2] {gate.report(id,ready.clone()).unwrap();}
        gate.report(2,Report::unavailable(0)).unwrap();
        assert_eq!(gate.ballots()[1].node,None);
        assert!(gate.resolve_votes(&run,0).unwrap().is_none());
        gate.stage=Stage::Travel;assert!(gate.spawn().is_none());
        gate.stage=Stage::Combat;
        gate.report(1,Report::new(&run,0,true,true,false)).unwrap();
        assert!(gate.defeat().is_none());assert!(gate.victory().is_err());
        gate.stage=Stage::Map;
        for id in [1,2] {gate.report(id,Report::new(&run,0,false,false,true)).unwrap();}
        assert!(gate.resolve_votes(&run,0).unwrap().is_none());
        for id in [1,2] {gate.report(id,ready.clone()).unwrap();}
        assert!(matches!(gate.resolve_votes(&run,0).unwrap(),Some(Order::Enter{..})));
    }
    #[test]
    fn host_cannot_travel_until_guest_votes_and_result_is_shared_once() {
        let run=Run::new(42,crate::Class::Samurai);let nodes=run.available();let mut gate=Gate::new(vec![1,2]);
        let mut host=Report::new(&run,0,false,false,true);host.selected=Some(nodes[0]);
        gate.report(1,host).unwrap();gate.report(2,Report::new(&run,0,false,false,true)).unwrap();
        assert!(gate.resolve_votes(&run,1).unwrap().is_none());
        let mut guest=Report::new(&run,0,false,false,true);guest.selected=Some(*nodes.last().unwrap());
        gate.report(2,guest).unwrap();
        assert!(matches!(gate.resolve_votes(&run,1).unwrap(),Some(Order::Enter{node,epoch:1,..}) if node==*nodes.last().unwrap()));
        assert!(gate.resolve_votes(&run,1).unwrap().is_none());
    }
    #[test]
    fn travel_waits_for_all_players_and_victory_requires_replica_ack() {
        let run=Run::new(42,crate::Class::Samurai);let mut g=Gate::new(vec![1,2]);let node=run.available()[0];
        g.report(1,Report::new(&run,0,false,false,true)).unwrap();assert!(g.enter(&run,node).is_err());
        g.report(2,Report::new(&run,0,false,false,true)).unwrap();g.enter(&run,node).unwrap();
        g.report(1,Report::new(&run,1,true,true,true)).unwrap();assert!(g.spawn().is_none());
        assert!(g.report(2,Report::new(&run,0,true,true,true)).is_err());
        g.report(2,Report::new(&run,1,true,false,true)).unwrap();assert!(g.spawn().is_some());assert!(g.victory().is_err());
        g.report(2,Report::new(&run,1,true,true,true)).unwrap();assert!(g.engage().is_some());assert!(g.victory().is_ok());assert!(g.victory().is_err());
    }
    #[test]
    fn one_death_or_disconnect_never_ends_the_run_for_everyone() {
        let run=Run::new(42,crate::Class::Samurai);let mut g=Gate::new(vec![1,2]);g.stage=Stage::Combat;
        g.report(1,Report::new(&run,0,true,true,false)).unwrap();g.report(2,Report::new(&run,0,true,true,true)).unwrap();assert!(g.defeat().is_none());
        g.disconnect(2);assert!(g.defeat().is_none());
        g.report(2,Report::new(&run,0,true,true,false)).unwrap();assert!(g.defeat().is_some());
    }
    #[test]
    fn unclaimed_reward_wrong_route_and_unknown_player_cannot_unlock_travel() {
        let run=Run::new(42,crate::Class::Samurai);let mut gate=Gate::new(vec![1,2]);let node=run.available()[0];
        let report=Report::new(&run,0,false,false,true);
        assert!(gate.report(3,report.clone()).is_err());
        gate.report(1,report.clone()).unwrap();let mut other=report.clone();other.map_ready=false;
        gate.report(2,other).unwrap();assert!(gate.enter(&run,node).is_err());
        let mut other=report.clone();other.seed=123;gate.report(2,other).unwrap();assert!(gate.enter(&run,node).is_err());
        let mut other=report.clone();other.visited=vec![7];gate.report(2,other).unwrap();assert!(gate.enter(&run,node).is_err());
        gate.report(2,report).unwrap();assert!(gate.enter(&run,node).is_ok());assert!(gate.enter(&run,node).is_err());
    }
    #[test]
    fn invalid_guest_event_request_or_unreachable_vote_never_errors_the_host() {
        use crate::party_events::Kind;
        let run=Run::new(42,crate::Class::Samurai);let node=run.available()[0];
        let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Service;gate.begin_events().unwrap();
        let report=Report::new(&run,0,true,false,true);gate.report(1,report.clone()).unwrap();
        for kind in [Kind::Guards(3),Kind::Scarabs(0),Kind::Scarabs(4)] {
            let mut bad=report.clone();bad.event_request=Some(kind);assert!(gate.report(2,bad).is_err());
        }
        gate.report(2,report.clone()).unwrap();
        gate.peers[1].1.as_mut().unwrap().event_request=Some(Kind::Guards(3));
        assert!(gate.event_start().unwrap().is_none());
        let mut gate=Gate::new(vec![1,2]);let mut host=report.clone();host.arrived=false;host.selected=Some(node);
        gate.report(1,host.clone()).unwrap();
        let mut guest=host.clone();guest.selected=Some(usize::MAX);gate.report(2,guest).unwrap();
        assert!(gate.resolve_votes(&run,0).unwrap().is_none());assert_eq!(gate.epoch,0);
        gate.report(2,host).unwrap();assert!(matches!(gate.resolve_votes(&run,0).unwrap(),Some(Order::Enter{epoch:1,..})));
    }
    #[test]
    fn generated_config_is_bounded_and_separates_peer_credentials() {
        let text=r#"{"session":9,"seed":42,"player":1,"host":1,"address":"127.0.0.1:47931","token":"00000000000000000000000000000001","peers":[{"player":2,"token":"00000000000000000000000000000002"}]}"#;
        assert!(Config::decode(text).is_ok());
        assert!(Config::decode(&text.replace("\"player\":2","\"player\":1")).is_err());
        assert!(Config::decode(&text.replace("\"player\":1","\"player\":3")).is_err());
        assert!(Config::decode(&text.replace(":47931",":0")).is_err());
        assert!(Config::decode(&"x".repeat(8193)).is_err());
    }
}
