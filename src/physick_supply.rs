use serde::{Deserialize,Serialize};
use crate::{Run,NodeKind};

pub const STARTING_TEAR:u32=0x40000000|11018;
pub const BOTTLE:u32=250;
pub const CAPACITY:u64=999;

#[derive(Clone,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
pub struct Supply {
    pub claimed:Vec<(u8,usize)>,
    pub pending:Option<Receipt>,
    #[serde(default)]
    pub reserve:u16,
    #[serde(default)]
    pub issued:bool,
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Receipt {
    pub act:u8,
    pub node:usize,
    pub target:Option<u64>,
}
impl Supply {
    pub fn valid(&self)->bool {
        let valid_key=|(act,node):(u8,usize)|(1..=3).contains(&act)&&node<256;
        u64::from(self.reserve)<=CAPACITY&&(!self.issued||self.reserve>0)&&self.claimed.len()<=768&&self.claimed.iter().copied().all(valid_key)
            &&self.claimed.iter().collect::<std::collections::BTreeSet<_>>().len()==self.claimed.len()
            &&self.pending.as_ref().is_none_or(|p|valid_key((p.act,p.node))&&!self.claimed.contains(&(p.act,p.node))&&p.target.is_none_or(|t|(1..=CAPACITY).contains(&t)))
    }
    pub fn queue(&mut self,act:u8,node:usize)->bool {
        if !(1..=3).contains(&act)||node>=256||self.pending.is_some()||self.claimed.contains(&(act,node)){return false;}
        self.pending=Some(Receipt{act,node,target:None});true
    }
    pub fn prepare(&mut self,count:u64) {
        if count<CAPACITY&&let Some(receipt)=self.pending.as_mut() {
            receipt.target.get_or_insert(count+1);
        }
    }
    pub fn complete(&mut self) {
        if let Some(receipt)=self.pending.take(){self.claimed.push((receipt.act,receipt.node));}
    }
    pub fn credit(&mut self)->bool {
        if self.pending.is_none()||u64::from(self.reserve)>=CAPACITY{return false;}
        self.reserve+=1;self.complete();true
    }
    pub fn confirm_load(&mut self,held:u64)->bool {
        if !self.issued||held==0{return false;}
        self.reserve=self.reserve.saturating_sub(1);self.issued=false;true
    }
    pub fn begin_load(&mut self,held:u64)->bool {
        if held>0||self.reserve==0||self.issued{return false;}
        self.issued=true;true
    }
}
impl Run {
    pub fn physick_drop(&self,node:usize,victory:bool)->bool {
        self.nodes.get(node).is_some_and(|n|n.kind==NodeKind::Camp ||
            victory&&n.kind.combat()&&crate::mix(self.seed^(u64::from(self.act)<<48)^(node as u64).wrapping_mul(7919)^0x7068797369636b)%100<15)
    }
}
