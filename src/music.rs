use std::sync::LazyLock;
use crate::{NodeKind,Run};

pub const FLAG_BASE:u32=45020400;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Pool {Explore,Calm,Battle,Boss}
pub struct Track {pub id:u32,pub flag:u32,pub pool:Pool,pub name:&'static str}
pub static CATALOG:LazyLock<Vec<Track>>=LazyLock::new(||include_str!("../scene-assets/bgm-catalog.txt").lines().enumerate().map(|(i,line)| {
    let p:Vec<_>=line.split('|').collect();
    Track{id:p[1].parse().unwrap(),flag:FLAG_BASE+i as u32,pool:match p[0]{"explore"=>Pool::Explore,"calm"=>Pool::Calm,"battle"=>Pool::Battle,"boss"=>Pool::Boss,_=>panic!("Invalid music pool")},name:p[2]}
}).collect());
static DECKS:LazyLock<[Vec<usize>;4]>=LazyLock::new(||std::array::from_fn(|index|CATALOG.iter().enumerate().filter(|(_,t)|t.pool as usize==index).map(|(i,_)|i).collect()));
pub fn choose(seed:u64,ordinal:usize,pool:Pool)->&'static Track {
    let deck=&DECKS[pool as usize];
    let start=crate::mix(seed^(pool as u64).wrapping_mul(0x9e3779b97f4a7c15)) as usize%deck.len();
    let mut step=(crate::mix(seed^0x62676d5f6465636b) as usize%deck.len()).max(1);
    fn gcd(mut a:usize,mut b:usize)->usize {while b!=0{(a,b)=(b,a%b);}a}
    while gcd(step,deck.len())!=1 {step=(step+1)%deck.len();}
    &CATALOG[deck[(start+(ordinal%deck.len())*step)%deck.len()]]
}
pub fn for_node(run:&Run,node:usize,combat:bool,wave:u8)->&'static Track {
    let kind=run.nodes.get(node).map(|n|n.kind);
    let pool=if combat {if matches!(kind,Some(NodeKind::Boss|NodeKind::Elite)){Pool::Boss}else{Pool::Battle}}
        else if matches!(kind,Some(NodeKind::Camp|NodeKind::Shop)){Pool::Calm}else{Pool::Explore};
    let ordinal=usize::from(run.act.saturating_sub(1))*12+run.nodes.get(node).map_or(node,|n|n.layer)+usize::from(wave)*37;
    choose(run.initial_seed.unwrap_or(run.seed),ordinal,pool)
}
pub fn hub()->&'static Track {CATALOG.iter().find(|t|t.id==11000026).unwrap()}
pub fn supported(block:i32)->bool {
    static BLOCKS:LazyLock<Vec<i32>>=LazyLock::new(||include_str!("../scene-assets/bgm-maps.txt").lines().map(|map| {
        let p:Vec<u8>=map[1..].split('_').map(|v|v.parse().unwrap()).collect();i32::from_be_bytes([p[0],p[1],p[2],p[3]])
    }).collect());BLOCKS.contains(&block)
}
