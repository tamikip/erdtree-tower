use serde::{Deserialize,Serialize};

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LarvalGift {
    delivered_act:u8,
    target:Option<u64>,
}
impl Default for LarvalGift {
    fn default()->Self {Self{delivered_act:1,target:None}}
}
impl LarvalGift {
    pub fn pending(&self,act:u8)->bool {(2..=3).contains(&act)&&self.delivered_act<act}
    pub fn begin(&mut self,count:u64)->u64 {*self.target.get_or_insert(count.saturating_add(1))}
    pub fn complete(&mut self,act:u8,count:u64)->bool {
        if !self.pending(act)||!self.target.is_some_and(|target|count>=target){return false;}
        self.delivered_act=act;self.target=None;true
    }
    pub fn valid(&self,act:u8)->bool {
        (1..=act).contains(&self.delivered_act)&&self.target.is_none_or(|t|t>0&&t<=u32::MAX as u64&&self.pending(act))
    }
}
