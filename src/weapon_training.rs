use serde::{Deserialize,Serialize};
use crate::Class;

pub const COSTS:[u32;5]=[100,200,350,550,800];
pub fn weight_percent(level:u8)->u64 {100+80*u64::from(level.min(5))}
pub const TYPES:&[(u8,&str)]=&[
    (1,"短剑"),(2,"直剑"),(3,"大剑"),(4,"特大剑"),(5,"刺剑"),(6,"重刺剑"),
    (7,"曲剑"),(8,"大曲剑"),(9,"刀"),(10,"双头剑"),(11,"槌"),(12,"大槌"),(13,"连枷"),
    (14,"斧"),(15,"大斧"),(16,"矛"),(17,"大矛"),(18,"戟"),(19,"镰刀"),(20,"软鞭"),
    (21,"拳头"),(22,"钩爪"),(23,"特大武器"),(24,"火把"),(30,"小盾"),(31,"中盾"),(32,"大盾"),
    (33,"法杖"),(34,"圣印记"),(40,"小弓"),(41,"长弓"),(42,"大弓"),(43,"弩"),(44,"弩炮"),
    (60,"格斗术"),(61,"调香瓶"),(62,"突刺盾"),(63,"投掷剑"),(64,"反手剑"),(66,"大刀"),(67,"轻大剑"),(68,"兽爪"),
];
pub fn known(kind:u8)->bool {TYPES.iter().any(|&(id,_)|id==kind)}
/// 武器种类的名称（直剑、大剑……）：按基础武器编号的百万位判断，词条武器先换回基础武器；不是已知种类返回 None
pub fn type_name(id:u32)->Option<&'static str> {
    let kind=(crate::affixes::base(id)&0x0fffffff)/1_000_000;
    TYPES.iter().find(|&&(k,_)|u32::from(k)==kind).map(|&(_,name)|name)
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Preference {pub kind:u8,pub level:u8}
impl Preference {
    pub fn valid(self)->bool {known(self.kind)&&(1..=5).contains(&self.level)}
    pub fn weight(self,id:u32)->u64 {if self.valid()&&(id&0x0fffffff)/1_000_000==u32::from(self.kind) {weight_percent(self.level)}else{100}}
}
#[derive(Clone,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(default)]
pub struct Training {pub selected:Option<u8>,pub level:u8,pub levels:Vec<Preference>,pub enabled:Vec<u8>}
impl Training {
    pub fn valid(&self)->bool {self.level<=5&&self.selected.is_none_or(known)&&valid_preferences(&self.levels)
        &&self.enabled.iter().enumerate().all(|(i,k)|known(*k)&&!self.enabled[..i].contains(k))}
    pub fn migrate(&mut self) {
        if self.level>0&&self.levels.is_empty() {
            if let Some(kind)=self.selected {self.levels.push(Preference{kind,level:self.level});self.enabled=vec![kind];}
        }
        self.level=0;self.selected=None;
    }
    pub fn level_for(&self,kind:u8)->u8 {self.levels.iter().find(|p|p.kind==kind).map_or(0,|p|p.level)}
    pub fn cost_for(&self,kind:u8)->Option<u32> {COSTS.get(usize::from(self.level_for(kind))).copied()}
    pub fn choose(&mut self,kind:Option<u8>)->Result<(),&'static str> {
        if kind.is_some_and(|k|!known(k)) {return Err("无效的武器类型");}
        if let Some(kind)=kind {
            if self.enabled.contains(&kind){self.enabled.retain(|k|*k!=kind);}else{self.enabled.push(kind);}
        }else{self.enabled.clear();}Ok(())
    }
    pub fn spent(&self)->u32 {self.levels.iter().map(|p|COSTS.iter().take(usize::from(p.level)).sum::<u32>()).sum()}
    pub fn preferences(&self)->Vec<Preference> {self.levels.iter().copied().filter(|p|self.enabled.contains(&p.kind)).collect()}
}
pub fn valid_preferences(prefs:&[Preference])->bool {prefs.len()<=TYPES.len()&&prefs.iter().enumerate().all(|(i,p)|p.valid()&&!prefs[..i].iter().any(|old|old.kind==p.kind))}
/// Preserve the 70% class / 30% global mixture, then multiply the selected type's weight.
pub fn pick(ids:&[u32],roll:u64,preference:Preference,class:Option<Class>)->usize {
    pick_many(ids,roll,&[preference],class)
}
pub fn pick_many(ids:&[u32],roll:u64,preferences:&[Preference],class:Option<Class>)->usize {
    assert!(!ids.is_empty());
    let favored=|id:u32|class.is_some_and(|c|c.weapon_types().contains(&((id&0x0fffffff)/1_000_000)));
    let count=ids.iter().filter(|&&id|favored(id)).count() as u64;
    let weight=|id|preferences.iter().map(|p|p.weight(id)).max().unwrap_or(100)*if count==0 {1}else{3*count+if favored(id){7*ids.len() as u64}else{0}};
    let mut ticket=roll%ids.iter().map(|&id|weight(id)).sum::<u64>();
    for (i,&id) in ids.iter().enumerate() {let w=weight(id);if ticket<w{return i;}ticket-=w;}
    unreachable!()
}
pub fn reward(run:&crate::Run,ids:&[u32],roll:u64)->u32 {
    let original=ids[roll as usize%ids.len()];
    if run.preferences().is_empty()||original>>28!=0 {return original;}
    let legendary=crate::elite::LEGENDARY.contains(&original);
    let weapons:Vec<_>=ids.iter().copied().filter(|id|id>>28==0&&crate::elite::LEGENDARY.contains(id)==legendary).collect();
    weapons[pick_many(&weapons,crate::mix(roll^0x70726566),run.preferences(),None)]
}

#[cfg(test)]
mod type_name_tests {
    #[test]
    fn weapon_cards_name_their_weapon_type() {
        assert_eq!(super::type_name(2000000),Some("直剑"));assert_eq!(super::type_name(3000000),Some("大剑"));
        assert_eq!(super::type_name(33000000),Some("法杖"));assert_eq!(super::type_name(66000000),Some("大刀"));
        assert_eq!(super::type_name(99000000),None);
        // 词条武器按基础武器判断
        let weapons:Vec<_>=crate::affixes::CATALOG.iter().filter(|i|i.kind==crate::rewards::ItemKind::Weapon).collect();
        assert!(weapons.iter().filter(|i|super::type_name(i.id).is_some()).count()*10>=weapons.len()*9,"almost every affix weapon has a known type");
        for item in weapons.iter().take(50) {assert_eq!(super::type_name(item.id),super::type_name(crate::affixes::base(item.id)));}
    }
}
