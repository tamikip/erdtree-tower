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
    /// 新规则下各等级的增益（1–3 级）
    pub fn boon_at(self,level:u8)->String {
        let l=usize::from(level.clamp(1,3)-1);
        let narrow=NARROW[l];
        match self {
            Self::Steel=>format!("蓄力重攻击伤害 +{narrow}%"),Self::Breaker=>format!("跳跃攻击伤害 +{narrow}%"),
            Self::Combo=>format!("连段末击伤害 +{narrow}%"),Self::Guard=>format!("防御反击伤害 +{narrow}%"),
            Self::Hunter=>format!("弓弩伤害 +{narrow}%"),Self::Execution=>format!("致命一击伤害 +{narrow}%"),
            Self::Mystic=>format!("魔法／祷告伤害 +{}%",MYSTIC[l]),
            Self::Feather=>format!("轻负重时伤害 +{}%\n精力恢复 +{} 点/秒",FEATHER[l],FEATHER_STAMINA[l]),
            Self::Blood=>format!("生命低于 50%\n伤害 +{}%",BLOOD[l]),
            Self::Ascetic=>{let (hp,fp)=ASCETIC[l];format!("胜利恢复最大生命 {hp}%\n最大专注 {fp}%")},
        }
    }
    /// 新规则下的代价：不随等级加重
    pub fn bane_at(self)->&'static str {match self {
        Self::Combo=>"受到伤害 +5%",Self::Guard=>"动作精力消耗 +10%",_=>self.bane(),
    }}
}
// 誓约 I–III 级的增益（%）。I 级不变，II、III 级削弱过一次（原来 II、III 级分别约为 I 级的 1.6 倍、2.3 倍）；
// 参数里的倍率在 scene-builder/OathAssets.cs，两边一起改
/// 蓄力、凌空、连击、坚盾、猎手、处刑
pub const NARROW:[u32;3]=[35,45,55];
pub const MYSTIC:[u32;3]=[25,32,40];
pub const FEATHER:[u32;3]=[20,25,30];
/// 轻羽之誓的精力恢复（点/秒）
pub const FEATHER_STAMINA:[u32;3]=[5,6,7];
pub const BLOOD:[u32;3]=[35,45,55];
/// 苦修之誓各等级的胜利恢复（最大生命 %、最大专注 %）
pub const ASCETIC:[(u8,u8);3]=[(15,20),(18,23),(20,26)];
pub const MAX_LEVEL:u8=3;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Oath {pub kind:Kind,pub level:u8}
/// 誓约界面上的一张卡
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Card {Oath(Kind),Deepen(Kind)}

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Gift {pub index:usize,pub delivered:[bool;2]}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct State {
    pub act:u8,pub offers:Vec<Kind>,pub chosen:Option<Kind>,pub gear:[u32;3],
    pub gear_done:bool,pub peak:u8,pub gift:Option<Gift>,
    /// 0：旧规则（每幕一份、下一幕作废）；1：誓约整局保留，可以深化
    #[serde(default)] pub rules:u8,
    /// 新规则下本局保留的誓约，含本幕刚立下或深化的
    #[serde(default,skip_serializing_if="Vec::is_empty")] pub held:Vec<Oath>,
    /// 第三张卡：深化已有的誓约
    #[serde(default,skip_serializing_if="Option::is_none")] pub deepen:Option<Kind>,
    /// 本幕选择了深化：不送装备，改送一个圣杯滴露
    #[serde(default,skip_serializing_if="std::ops::Not::not")] pub deepened:bool,
}
/// 每幕第一张誓约卡的职业倾向
fn preferred(class:Class,roll:u64)->Kind {
    let kinds:&[Kind]=match class {
        Class::Vagabond=>&[Kind::Guard,Kind::Steel],
        Class::Hero=>&[Kind::Breaker,Kind::Blood],
        Class::Samurai=>&[Kind::Execution,Kind::Combo],
        Class::Astrologer=>&[Kind::Mystic],
        Class::Prophet=>&[Kind::Mystic,Kind::Guard],
        Class::Confessor=>&[Kind::Mystic],
    };
    kinds[roll as usize%kinds.len()]
}
impl State {
    pub fn new(seed:u64,class:Class,act:u8,previous:Option<Kind>,peak:u8)->Self {
        let roll=crate::mix(seed^0xc071ac7);
        let preferred=if class.sorcerer()||class.cleric(){Kind::Mystic}else{
            [Kind::Steel,Kind::Breaker,Kind::Combo,Kind::Guard][roll as usize%4]
        };
        let mut pool:Vec<_>=Kind::ALL.into_iter().filter(|k|*k!=Kind::Hunter&&Some(*k)!=previous).collect();
        pool.sort_by_key(|k|crate::mix(roll^k.index() as u64));
        let first=if Some(preferred)!=previous{preferred}else{pool[0]};
        let second=*pool.iter().find(|k|**k!=first).unwrap();
        Self{act,offers:vec![first,second],chosen:None,gear:[0;3],gear_done:false,peak:peak.min(MAX_PEAK),gift:None,rules:0,held:vec![],deepen:None,deepened:false}
    }
    /// 开局三份新誓约；后续两份新誓约，并提供未满级誓约的深化。
    pub fn start(seed:u64,class:Class,act:u8,held:Vec<Oath>,peak:u8)->Self {
        let roll=crate::mix(seed^0xc071ac7);
        let preferred=preferred(class,roll);
        let mut pool:Vec<_>=Kind::ALL.into_iter().filter(|k|*k!=Kind::Hunter&&!held.iter().any(|o|o.kind==*k)).collect();
        pool.sort_by_key(|k|crate::mix(roll^k.index() as u64));
        let first=if pool.contains(&preferred){preferred}else{pool[0]};
        let second=*pool.iter().find(|k|**k!=first).unwrap();
        let deepen=held.iter().rev().filter(|o|o.kind!=Kind::Hunter&&o.level<MAX_LEVEL).max_by_key(|o|o.level).map(|o|o.kind);
        let mut offers=vec![first,second];
        if act==1&&held.is_empty() {offers.push(*pool.iter().find(|k|!offers.contains(k)).unwrap());}
        Self{act,offers,chosen:None,gear:[0;3],gear_done:false,peak:peak.min(MAX_PEAK),gift:None,rules:1,held,deepen,deepened:false}
    }
    /// 进入下一幕：旧规则换一份新契约，新规则保留已有誓约
    pub fn next_act(&self,seed:u64,class:Class,act:u8)->Self {
        if self.rules==0 {Self::new(seed,class,act,self.chosen,self.peak)}else{Self::start(seed,class,act,self.held.clone(),self.peak)}
    }
    /// 当前候选誓约及深化卡。
    pub fn cards(&self)->Vec<Card> {
        let mut cards:Vec<_>=self.offers.iter().map(|k|Card::Oath(*k)).collect();
        if self.rules==1 {if let Some(kind)=self.deepen {cards.push(Card::Deepen(kind));}}
        cards
    }
    pub fn options(&self)->usize {self.cards().len()}
    pub fn retire_ranged_offers(&mut self) {
        if !self.selection_pending(){return;}
        for slot in 0..self.offers.len() {
            if self.offers[slot]==Kind::Hunter {
                if let Some(kind)=Kind::ALL.into_iter().find(|k|*k!=Kind::Hunter&&!self.offers.contains(k)&&!self.held.iter().any(|o|o.kind==*k)) {
                    self.offers[slot]=kind;
                }
            }
        }
        if self.deepen==Some(Kind::Hunter){self.deepen=None;}
    }
    /// 当前生效的誓约及等级；旧规则返回等级 0，表示沿用旧数值
    pub fn active(&self)->Vec<Oath> {
        if self.rules==0 {self.chosen.map(|kind|Oath{kind,level:0}).into_iter().collect()}else{self.held.clone()}
    }
    pub fn level(&self,kind:Kind)->Option<u8> {self.active().into_iter().find(|o|o.kind==kind).map(|o|o.level)}
    pub fn finish_deepen(&mut self)->RuleResult<()> {
        if !self.deepened||self.gear_done {return Err("当前没有待完成的深化".into());}
        self.gear_done=true;Ok(())
    }
    pub fn selection_pending(&self)->bool {self.chosen.is_none()&&!self.gear_done}
    pub fn pending(&self)->bool {!self.gear_done}
    pub fn skip(&mut self)->RuleResult<()> {
        if !self.selection_pending(){return Err("本幕契约已经确定".into());}
        self.gear_done=true;Ok(())
    }
    pub fn choose(&mut self,index:usize,seed:u64,class:Class)->RuleResult<()> {
        if !self.selection_pending(){return Err("本幕契约已经确定".into());}
        let kind=match *self.cards().get(index).ok_or("请选择一份誓约")? {
            Card::Deepen(kind)=>{
                let oath=self.held.iter_mut().find(|o|o.kind==kind&&o.level<MAX_LEVEL).ok_or("誓约已达到最高等级")?;
                oath.level+=1;self.chosen=Some(kind);self.deepened=true;return Ok(());
            },
            Card::Oath(kind)=>kind,
        };
        self.chosen=Some(kind);self.gear=gear(kind,class,seed,self.rules==1);
        if self.rules==1 {
            for (slot,id) in self.gear.iter_mut().enumerate() {
                *id=crate::affixes::common_weapon(*id,seed^u64::from(self.act)<<48^slot as u64^u64::from(*id)).ok_or("誓约武器缺少普通词条参数")?;
            }
        }
        if self.rules==1 {self.held.push(Oath{kind,level:1});}
        Ok(())
    }
    pub fn begin_gift(&mut self,index:usize)->RuleResult<()> {
        if self.chosen.is_none()||self.deepened||self.gear_done||index>=3 {return Err("当前不能领取适配装备".into());}
        if self.gift.as_ref().is_some_and(|g|g.index!=index){return Err("装备已开始发放，请重试原选择".into());}
        if self.gift.is_none(){self.gift=Some(Gift{index,delivered:[false;2]});}Ok(())
    }
    pub fn finish_gift(&mut self)->RuleResult<()> {
        if !self.gift.as_ref().is_some_and(|g|g.delivered==[true;2]){return Err("装备尚未发放完成".into());}
        self.gear_done=true;Ok(())
    }
    pub fn skip_gift(&mut self)->RuleResult<()> {
        if self.chosen.is_none()||self.deepened||self.gear_done||self.gift.is_some(){return Err("当前不能跳过装备".into());}
        self.gear_done=true;Ok(())
    }
    pub fn valid(&self,act:u8)->bool {
        let levels:u32=self.held.iter().map(|o|u32::from(o.level)).sum();
        let held_ok=match self.rules {
            0=>self.held.is_empty()&&self.deepen.is_none()&&!self.deepened,
            1=>self.held.iter().enumerate().all(|(i,o)|(1..=MAX_LEVEL).contains(&o.level)&&!self.held[..i].iter().any(|p|p.kind==o.kind))
                &&levels<=u32::from(act)-u32::from(self.chosen.is_none())
                &&self.deepen.is_none_or(|k|self.held.iter().any(|o|o.kind==k&&(o.level<MAX_LEVEL||self.deepened)))
                &&self.offers.iter().all(|k|Some(*k)==self.chosen&&!self.deepened||!self.held.iter().any(|o|o.kind==*k))
                &&self.chosen.is_none_or(|k|self.held.iter().any(|o|o.kind==k))
                &&(!self.deepened||self.chosen.is_some()&&self.chosen==self.deepen),
            _=>false,
        };
        let count_ok=self.offers.len()==2||(self.rules==1&&act==1&&self.offers.len()==3&&self.deepen.is_none());
        self.act==act&&self.peak<=MAX_PEAK&&count_ok&&self.offers.iter().enumerate().all(|(i,k)|!self.offers[..i].contains(k))&&held_ok
            &&self.chosen.is_none_or(|k|self.offers.contains(&k)||self.deepened)
            &&if self.chosen.is_none()||self.deepened{self.gift.is_none()&&self.gear==[0;3]}else{
                self.gear.iter().enumerate().all(|(i,id)|!self.gear[..i].contains(id)&&crate::rewards::lookup(*id).is_some_and(|r|r.kind==crate::rewards::ItemKind::Weapon))
                    &&self.gift.as_ref().is_none_or(|g|g.index<3&&(!self.gear_done||g.delivered==[true;2]))
            }
    }
}
/// 武器强化上限（scene-builder weapon-level-patch 补了等级行）：普通 +99（武器编号能表示的最高等级）、失色 +39（每级相当于普通 2.5 级，取同一水平）。
/// 原版是 +25、+10。等效等级 peak = 普通等级×2 = 失色等级×5（原版满级都是 50），最高 MAX_PEAK
pub const NORMAL_MAX:u8=99;
pub const SOMBER_MAX:u8=39;
pub const MAX_PEAK:u8=198;
/// 等效等级换算成某种武器的强化等级：cap 为原版满级（普通 25、失色 10），超过原版满级照比例继续
pub fn reinforcement_level(peak:u8,cap:u8)->u8 {((u16::from(peak.min(MAX_PEAK))*u16::from(cap))/50) as u8}
fn gear(kind:Kind,_class:Class,seed:u64,epic:bool)->[u32;3] {
    let mut pool:Vec<u32>=include_str!("../scene-assets/reward-rarities.tsv").lines().filter_map(|line| {
        let (id,tier)=line.split_once('\t')?;let id:u32=id.parse().ok()?;
        let tier=tier.trim().parse::<u8>().ok()?;
        if (if epic{tier!=2}else{tier>1})||!crate::rewards::lookup(id).is_some_and(|i|i.kind==crate::rewards::ItemKind::Weapon){return None;}
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
    match crate::affixes::base(weapon)/1_000_000 {40|41=>Some(50000000),42=>Some(51000000),43=>Some(52000000),44=>Some(53000000),_=>None}
}

impl crate::Run {
    pub fn enable_contracts(&mut self) {
        if self.contract.is_none() {self.contract=Some(State::start(self.reward_seed(),self.class,self.act,vec![],0));}
    }
    pub fn contract_pending(&self)->bool {self.contract.as_ref().is_some_and(State::pending)}
}
