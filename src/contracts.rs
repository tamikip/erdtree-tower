use serde::{Deserialize,Serialize};
use crate::{Class,RuleResult};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum Kind {Steel,Breaker,Combo,Guard,Mystic,Hunter,Execution,Feather,Blood,Ascetic}
impl Kind {
    pub const ALL:[Self;10]=[Self::Steel,Self::Breaker,Self::Combo,Self::Guard,Self::Mystic,Self::Hunter,Self::Execution,Self::Feather,Self::Blood,Self::Ascetic];
    pub fn name(self)->&'static str {match self {
        Self::Steel=>"蓄力之誓",Self::Breaker=>"凌空之誓",Self::Combo=>"连击之誓",
        Self::Guard=>"坚盾之誓",Self::Mystic=>"秘法之誓",Self::Hunter=>"猎手之誓",
        Self::Execution=>"处刑之誓",Self::Feather=>"轻羽之誓",Self::Blood=>"血契之誓",Self::Ascetic=>"苦修之誓",
    }}
    pub fn boon(self)->&'static str {match self {
        Self::Steel=>"蓄力重攻击伤害 +20%",Self::Breaker=>"跳跃攻击伤害 +25%",
        Self::Combo=>"连段末击伤害 +30%",Self::Guard=>"防御反击伤害 +30%",
        Self::Mystic=>"魔法／祷告伤害 +20%",Self::Hunter=>"弓弩伤害 +30%",
        Self::Execution=>"致命一击伤害 +30%",
        Self::Feather=>"轻负重时伤害 +15%\n精力恢复 +5 点/秒",
        Self::Blood=>"生命低于 50%\n伤害 +25%",
        Self::Ascetic=>"胜利恢复最大生命 10%\n最大专注 15%",
    }}
    pub fn bane(self)->&'static str {match self {
        Self::Steel|Self::Hunter=>"战技伤害 -20%",Self::Breaker=>"战技专注消耗 +25%",
        Self::Combo=>"受到伤害 +10%",Self::Guard=>"动作精力消耗 +20%",
        Self::Mystic=>"法术专注消耗 +20%",
        Self::Execution=>"非致命攻击伤害 -10%",
        Self::Feather=>"受到物理伤害 +10%",
        Self::Blood=>"红圣杯瓶恢复量 -20%",
        Self::Ascetic=>"红、蓝圣杯瓶恢复量 -35%",
    }}
    pub fn hint(self)->&'static str {match self {
        Self::Steel=>"抓住敌人硬直，以蓄力重击换取爆发。",
        Self::Breaker=>"以跳跃攻击接近敌人，减少使用战技。",
        Self::Combo=>"完整打出连段才获得增益；普通连击中途没有加成。",
        Self::Guard=>"格挡后重击反击，注意为后续闪避保留精力。",
        Self::Mystic=>"适配装备附送基础攻击法术；自行装备并记忆。",
        Self::Hunter=>"适配装备附送 30 支对应箭矢或弩箭。",
        Self::Execution=>"以背刺和弹反处决敌人。",
        Self::Feather=>"装备重量低于最大负重的 30% 时获得增益。",
        Self::Blood=>"以低血量换取更高伤害。",
        Self::Ascetic=>"胜利恢复按最大生命和专注计算，不超过上限。",
    }}
    pub fn index(self)->usize {Self::ALL.iter().position(|k|*k==self).unwrap()}
}

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Gift {pub index:usize,pub delivered:[bool;2]}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct State {
    pub act:u8,pub offers:[Kind;2],pub chosen:Option<Kind>,pub gear:[u32;3],
    pub gear_done:bool,pub peak:u8,pub gift:Option<Gift>,
}
impl State {
    pub fn new(seed:u64,class:Class,act:u8,previous:Option<Kind>,peak:u8)->Self {
        let roll=crate::mix(seed^0xc071ac7);
        let preferred=if class.sorcerer()||class.cleric(){Kind::Mystic}else{
            [Kind::Steel,Kind::Breaker,Kind::Combo,Kind::Guard][roll as usize%4]
        };
        let mut pool:Vec<_>=Kind::ALL.into_iter().filter(|k|Some(*k)!=previous).collect();
        pool.sort_by_key(|k|crate::mix(roll^k.index() as u64));
        let first=if Some(preferred)!=previous{preferred}else{pool[0]};
        let second=*pool.iter().find(|k|**k!=first).unwrap();
        Self{act,offers:[first,second],chosen:None,gear:[0;3],gear_done:false,peak:peak.min(50),gift:None}
    }
    pub fn selection_pending(&self)->bool {self.chosen.is_none()&&!self.gear_done}
    pub fn pending(&self)->bool {!self.gear_done}
    pub fn skip(&mut self)->RuleResult<()> {
        if !self.selection_pending(){return Err("本幕契约已经确定".into());}
        self.gear_done=true;Ok(())
    }
    pub fn choose(&mut self,index:usize,seed:u64,class:Class)->RuleResult<()> {
        if !self.selection_pending(){return Err("本幕契约已经确定".into());}
        let kind=*self.offers.get(index).ok_or("请选择两份契约中的一份")?;
        self.chosen=Some(kind);self.gear=gear(kind,class,seed);Ok(())
    }
    pub fn begin_gift(&mut self,index:usize)->RuleResult<()> {
        if self.chosen.is_none()||self.gear_done||index>=3 {return Err("当前不能领取适配装备".into());}
        if self.gift.as_ref().is_some_and(|g|g.index!=index){return Err("装备已开始发放，请重试原选择".into());}
        if self.gift.is_none(){self.gift=Some(Gift{index,delivered:[false;2]});}Ok(())
    }
    pub fn finish_gift(&mut self)->RuleResult<()> {
        if !self.gift.as_ref().is_some_and(|g|g.delivered==[true;2]){return Err("装备尚未发放完成".into());}
        self.gear_done=true;Ok(())
    }
    pub fn skip_gift(&mut self)->RuleResult<()> {
        if self.chosen.is_none()||self.gear_done||self.gift.is_some(){return Err("当前不能跳过装备".into());}
        self.gear_done=true;Ok(())
    }
    pub fn valid(&self,act:u8)->bool {
        self.act==act&&self.peak<=50&&self.offers[0]!=self.offers[1]
            &&self.chosen.is_none_or(|k|self.offers.contains(&k))
            &&if self.chosen.is_none(){self.gift.is_none()&&self.gear==[0;3]}else{
                self.gear.iter().enumerate().all(|(i,id)|!self.gear[..i].contains(id)&&crate::rewards::lookup(*id).is_some_and(|r|r.kind==crate::rewards::ItemKind::Weapon))
                    &&self.gift.as_ref().is_none_or(|g|g.index<3&&(!self.gear_done||g.delivered==[true;2]))
            }
    }
}
pub fn reinforcement_level(peak:u8,cap:u8)->u8 {((u16::from(peak.min(50))*u16::from(cap))/50) as u8}
fn gear(kind:Kind,_class:Class,seed:u64)->[u32;3] {
    let mut pool:Vec<u32>=include_str!("../scene-assets/reward-rarities.tsv").lines().filter_map(|line| {
        let (id,tier)=line.split_once('\t')?;let id:u32=id.parse().ok()?;
        if tier.parse::<u8>().ok()?>1||!crate::rewards::lookup(id).is_some_and(|i|i.kind==crate::rewards::ItemKind::Weapon){return None;}
        let family=id/1_000_000;
        let matches=match kind {
            Kind::Steel=>(1..=23).contains(&family),
            Kind::Breaker=>matches!(family,3|4|8|12|15|17|18|19|23),
            Kind::Combo=>matches!(family,1|2|5|7|9|10|21|22),
            Kind::Guard=>matches!(family,2|3|11|16|18|31|32),
            Kind::Mystic=>matches!(family,33|34),
            Kind::Hunter=>(40..=44).contains(&family),
            Kind::Execution=>matches!(family,1|5|30),
            Kind::Feather=>matches!(family,1|5|7|21|22),
            Kind::Blood=>(1..=23).contains(&family),
            Kind::Ascetic=>(1..=23).contains(&family)||matches!(family,33|34),
        };matches.then_some(id)
    }).collect();
    pool.sort_by_key(|id|crate::mix(seed^u64::from(*id)^0x9ea7));
    if kind==Kind::Guard {
        let shield=*pool.iter().find(|id|matches!(**id/1_000_000,31|32)).expect("Missing common/rare shields");
        pool.retain(|id|*id/1_000_000<30);return [shield,pool[0],pool[1]];
    }
    [pool[0],pool[1],pool[2]]
}
pub fn hunter_ammo(weapon:u32)->Option<u32> {
    match weapon/1_000_000 {40|41=>Some(50000000),42=>Some(51000000),43=>Some(52000000),44=>Some(53000000),_=>None}
}

impl crate::Run {
    pub fn enable_contracts(&mut self) {
        if self.contract.is_none() {self.contract=Some(State::new(self.reward_seed(),self.class,self.act,None,0));}
    }
    pub fn contract_pending(&self)->bool {self.contract.as_ref().is_some_and(State::pending)}
}
