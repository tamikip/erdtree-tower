use crate::{Class,Reward,Run};
use std::sync::LazyLock;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ItemKind {Weapon,Armor,Sorcery,Incantation,Spirit,Talisman,Crystal,Tool}
impl ItemKind {
    pub fn label(self)->&'static str {
        match self {Self::Weapon=>"武器",Self::Armor=>"防具",Self::Sorcery=>"魔法",Self::Incantation=>"祷告",Self::Spirit=>"骨灰",Self::Talisman=>"护符",Self::Crystal=>"结晶露滴",Self::Tool=>"砥石刀"}
    }
    pub fn allowed(self,class:Class)->bool {
        match self {Self::Sorcery=>class.sorcerer(),Self::Incantation=>class.cleric(),_=>true}
    }
}
pub struct Item {pub id:u32,pub name:&'static str,pub kind:ItemKind}
impl Item {
    pub fn random_eligible(&self)->bool {
        crate::dlc::item_allowed(self.id)&&self.id!=(0x40000000|11019)&&self.kind!=ItemKind::Armor&&(self.kind!=ItemKind::Talisman||(!self.name.contains(['+','＋'])
            &&!matches!(self.id&0x0fffffff,1100|2140|4060|6040|6060|6070|6080|6090)))
    }
}
pub static CATALOG:LazyLock<Vec<Item>>=LazyLock::new(||parse(include_str!("../scene-assets/reward-catalog.txt")).into_iter().chain(parse(include_str!("../scene-assets/crystal-tears.txt"))).collect());
pub fn progression_good(id:u32)->bool {
    static IDS:LazyLock<std::collections::HashSet<u32>>=LazyLock::new(||CATALOG.iter().filter(|i|matches!(i.kind,ItemKind::Tool|ItemKind::Crystal)).map(|i|i.id).collect());
    IDS.contains(&id)
}
/// 公开版移除的 RNG Lands - Ruby 法术，只保留编号用于读取旧存档
pub static RETIRED_SPELLS:LazyLock<std::collections::HashSet<u32>>=LazyLock::new(||include_str!("../scene-assets/retired-spell-rows.txt").lines().filter_map(|l|l.strip_prefix("EquipParamGoods|")).map(|id|0x40000000|id.parse::<u32>().unwrap()).collect());
static OLD_CATALOG:LazyLock<Vec<Item>>=LazyLock::new(||parse(include_str!("../scene-assets/reward-catalog-v29.txt")));
fn parse(text:&'static str)->Vec<Item> {text.lines().map(|line| {
    let mut parts=line.split('|');
    let (kind,tag)=match parts.next().unwrap() {
        "weapon"=>(ItemKind::Weapon,0),"armor"=>(ItemKind::Armor,0x10000000),
        "talisman"=>(ItemKind::Talisman,0x20000000),
        "crystal"=>(ItemKind::Crystal,0x40000000),
        "tool"=>(ItemKind::Tool,0x40000000),
        "sorcery"=>(ItemKind::Sorcery,0x40000000),"incantation"=>(ItemKind::Incantation,0x40000000),"spirit"=>(ItemKind::Spirit,0x40000000),_=>panic!("Invalid reward catalog"),
    };
    Item{id:tag|parts.next().unwrap().parse::<u32>().unwrap(),name:parts.next().unwrap(),kind}
}).collect()}
static SPIRIT_LEVELS:LazyLock<Vec<Item>>=LazyLock::new(||CATALOG.iter().filter(|i|i.kind==ItemKind::Spirit).flat_map(|i|(1..=10).map(move |level|Item{
    id:i.id+level,name:Box::leak(format!("{} +{level}",i.name).into_boxed_str()),kind:ItemKind::Spirit,
})).collect());
pub fn lookup(id:u32)->Option<&'static Item> {crate::affixes::lookup(id).or_else(||crate::talismans::lookup(id)).or_else(||CATALOG.iter().find(|item|item.id==id)).or_else(||SPIRIT_LEVELS.iter().find(|item|item.id==id))}
pub fn spirit_base(id:u32)->u32 {
    let base=id-(id&0x0fffffff)%1000;
    if id-base<=10&&CATALOG.iter().any(|i|i.id==base&&i.kind==ItemKind::Spirit){base}else{id}
}
/// 刷出的骨灰最高等级：规则 1 为当前骨灰等级（持有过的骨灰最高等级与武器等价强化中较高的）；规则 0 为持有过的骨灰最高等级 +1，最多 +10。
/// 旧存档没有骨灰记录时沿用旧规则：武器的等价强化等级（换算成 +0～+10）+1，也没有武器记录时不限
pub fn spirit_cap(run:&Run)->u32 {
    match run.spirit_peak {
        // 骨灰规则 1（新旅程）：上限就是当前骨灰等级，取持有过的骨灰最高等级与武器等价强化等级中较大的，不再 +1
        Some(_) if run.spirit_rules>=1=>u32::from(spirit_level_now(run)),
        Some(peak)=>(u32::from(peak)+1).min(10),
        None=>run.weapon_peak.map_or(10,|peak|(u32::from(crate::contracts::reinforcement_level(peak,10))+1).min(10)),
    }
}
/// 当前骨灰等级：持有过的骨灰最高等级（只升不降）与武器的等价强化等级（换算成 +0～+10）中较大的
pub fn spirit_level_now(run:&Run)->u8 {
    let weapon=run.weapon_peak.map_or(0,|peak|crate::contracts::reinforcement_level(peak,10));
    run.spirit_peak.unwrap_or(0).max(weapon).min(10)
}
/// 骨灰等级（+0～+10）；不是骨灰时为 None
pub fn spirit_level(id:u32)->Option<u8> {
    let base=spirit_base(id);
    (CATALOG.iter().any(|i|i.id==base&&i.kind==ItemKind::Spirit)).then(||(id-base) as u8)
}
/// 调灵：把一个骨灰升到 max(自身, 当前骨灰等级) + 1（最多 +10），低级骨灰一次追上来；已经 +10 时为 None
pub fn spirit_tune_target(id:u32,peak:u8)->Option<u32> {
    let level=spirit_level(id)?;
    (level<10).then(||spirit_base(id)+u32::from(level.max(peak.min(10))+1).min(10))
}
/// 随机骨灰等级 +0～+cap（cap 为 10 时与原来的 +0～+10 相同）
pub fn random_spirit_level(id:u32,entropy:u64,cap:u32)->u32 {
    let base=spirit_base(id);
    if !CATALOG.iter().any(|i|i.id==base&&i.kind==ItemKind::Spirit){return id;}
    base+(crate::mix(entropy^u64::from(base)^0xa541_e7e1)%u64::from(cap.min(10)+1)) as u32
}

pub fn offers(run:&Run,node:usize)->Vec<Reward> {
    if crate::mix(run.reward_seed()^(node as u64).wrapping_mul(7919)^0x746f6f6c)%100<10{return before_tool_rate(run,node);}
    let mut limited=run.clone();limited.claimed_items.extend(CATALOG.iter().filter(|i|i.kind==ItemKind::Tool).map(|i|i.id));
    before_tool_rate(&limited,node)
}
pub(crate) fn before_tool_rate(run:&Run,node:usize)->Vec<Reward> {
    let mut owned:Vec<_>=run.claimed_items.iter().map(|id|crate::affixes::base(*id)).filter(|id|!run.claimed_items.contains(id)).collect();
    // 职业传承选用的开局护符已经在身上，不再作为奖励出现
    if run.start_talisman!=0&&!run.claimed_items.contains(&(0x20000000|run.start_talisman)) {owned.push(0x20000000|run.start_talisman);}
    if owned.is_empty(){return crate::affixes::rewards(run,node,casting_replaced(run,node,true));}
    let mut run=run.clone();run.claimed_items.extend(owned);
    crate::affixes::rewards(&run,node,casting_replaced(&run,node,true))
}
pub(crate) fn before_whetblades(run:&Run,node:usize)->Vec<Reward> {
    let mut run=run.clone();
    run.claimed_items.extend(CATALOG.iter().filter(|i|i.kind==ItemKind::Tool).map(|i|i.id));
    offers(&run,node)
}
pub(crate) fn before_claimed_bases(run:&Run,node:usize)->Vec<Reward> {
    crate::affixes::rewards(run,node,casting_replaced(run,node,false))
}
fn casting_replaced(run:&Run,node:usize,spread:bool)->Vec<Reward> {
    let mut result=before_spirit_levels(run,node);let cap=spirit_cap(run);
    for reward in &mut result {if let Reward::Item(id)=reward {
        *id=random_spirit_level(*id,run.reward_seed()^(node as u64).wrapping_mul(7919),cap);
        if lookup(*id).is_some_and(|i|i.kind==ItemKind::Spirit)&&run.claimed_items.contains(id) {
            let base=spirit_base(*id);let level=*id-base;
            *reward=(1..=cap).map(|offset|base+(level+offset)%(cap+1)).find(|id|!run.claimed_items.contains(id))
                .map_or(Reward::Gold(1500),Reward::Item);
        }
    }}
    if let Some(tools)=run.casting_tools {
        for index in 0..result.len() {
            if let Reward::Item(id)=result[index] {if !castable(id,tools) {
                let pool:Vec<_>=CATALOG.iter().filter(|i|i.kind==ItemKind::Weapon&&i.random_eligible()
                    &&!run.claimed_items.contains(&i.id)&&!result.contains(&Reward::Item(i.id))
                    &&(!crate::elite::LEGENDARY.contains(&id)||crate::elite::LEGENDARY.contains(&i.id))
                    &&(!crate::elite::EPIC.contains(&id)||crate::elite::EPIC.contains(&i.id))).collect();
                let roll=if spread{crate::mix(run.reward_seed()^(node as u64).wrapping_mul(7919)^index as u64^0xca57)}else{crate::mix(run.reward_seed()^node as u64^index as u64)};
                result[index]=if pool.is_empty(){Reward::Gold(1500)}else{Reward::Item(pool[roll as usize%pool.len()].id)};
            }}
        }
    }
    result
}
pub fn castable(id:u32,tools:[bool;2])->bool {
    match lookup(id).map(|i|i.kind) {Some(ItemKind::Sorcery)=>tools[0],Some(ItemKind::Incantation)=>tools[1],_=>true}
}
pub(crate) fn before_spirit_levels(run:&Run,node:usize)->Vec<Reward> {offers_currency(run,node,1500)}
pub(crate) fn before_currency_1500(run:&Run,node:usize)->Vec<Reward> {offers_currency(run,node,600)}
fn offers_currency(run:&Run,node:usize,currency:u32)->Vec<Reward> {
    if run.nodes.get(node).is_some_and(|n|n.kind==crate::NodeKind::Boss){return crate::elite::boss_offers(run,node);}
    if run.nodes.get(node).is_some_and(|n|n.kind==crate::NodeKind::Elite){return crate::elite::offers(run,node);}
    let mut result=if run.nodes.get(node).is_some_and(|n|n.kind==crate::NodeKind::Chest){chest_offers(run,node)}else{before_bosses(run,node)};
    if run.nodes.get(node).is_some_and(|n|n.kind==crate::NodeKind::Monster) {
        let roll=crate::mix(run.reward_seed() ^ node as u64 ^ 0x67077);
        match roll%100 {
            0..=9 if run.flask_bonus<4=>result[1]=Reward::FlaskCapacity,
            10..=19 if run.memory_bonus<6=>result[1]=Reward::MemorySlot,
            20..=29 if run.class.sorcerer()&&!run.star_stream=>result[1]=Reward::StarStream,
            _=>{},
        }
    }
    for reward in &mut result {if *reward==Reward::Gold(75){*reward=Reward::Gold(currency);}}
    result
}
fn chest_offers(run:&Run,node:usize)->Vec<Reward> {
    let mut result=weighted(run,node,true);
    let roll=crate::mix(run.reward_seed()^(node as u64).wrapping_mul(7919)^0xc4e57);
    if roll%100<25 {
        let buffs:Vec<_>=crate::elite::BUFFS.iter().copied().filter(|id|!run.elite_buffs.contains(id)).collect();
        if !buffs.is_empty(){result[1]=Reward::TalismanBuff(buffs[crate::mix(roll^1) as usize%buffs.len()]);}
    }
    if let Some(id)=crystal_offer_with_chance(run,node as u64,0xc715,8){result[2]=Reward::Item(id);}
    if crate::mix(roll^2)%100<20 {
        let pool:Vec<_>=crate::elite::EPIC.iter().chain(crate::elite::LEGENDARY).copied()
            .filter(|id|!run.claimed_items.contains(id)&&!result[1..].contains(&Reward::Item(*id))
                &&lookup(*id).is_some_and(|i|i.kind.allowed(run.class)&&i.random_eligible())).collect();
        if !pool.is_empty(){result[0]=Reward::Item(crate::weapon_training::reward(run,&pool,crate::mix(roll^3)));}
    }
    result
}
pub(crate) fn before_chest_boosts(run:&Run,node:usize)->Vec<Reward> {
    let mut result=before_bosses(run,node);
    for reward in &mut result {if *reward==Reward::Gold(75){*reward=Reward::Gold(600);}}
    result
}
pub(crate) fn before_bosses(run:&Run,node:usize)->Vec<Reward> {
    if run.nodes.get(node).is_some_and(|n|n.kind==crate::NodeKind::Elite){return crate::elite::legendary_offers(run,node);}
    let mut offers=weighted(run,node,true);
    if let Some(id)=crystal_offer(run,node as u64,0xc715) {offers[2]=Reward::Item(id);}
    offers
}
pub fn crystal_offer(run:&Run,visit:u64,salt:u64)->Option<u32> {
    crystal_offer_with_chance(run,visit,salt,5)
}
fn crystal_offer_with_chance(run:&Run,visit:u64,salt:u64,chance:u64)->Option<u32> {
    let roll=crate::mix(run.reward_seed() ^ (u64::from(run.act)<<48) ^ visit.wrapping_mul(7919) ^ salt);
    if roll%100>=chance {return None;}
    let pool:Vec<_>=CATALOG.iter().filter(|i|i.kind==ItemKind::Crystal&&i.random_eligible()&&!run.claimed_items.contains(&i.id)).collect();
    (!pool.is_empty()).then(||pool[crate::mix(roll) as usize%pool.len()].id)
}
pub(crate) fn before_elites(run:&Run,node:usize)->Vec<Reward> {weighted(run,node,true)}
pub(crate) fn before_base_talismans(run:&Run,node:usize)->Vec<Reward> {weighted(run,node,false)}
pub(crate) fn boss_regular_offers(run:&Run,node:usize)->Vec<Reward> {weighted(run,node,true)}
fn weighted(run:&Run,node:usize,base_only:bool)->Vec<Reward> {
    let mut offers=ranked_pool(run,node,true,&CATALOG,base_only);
    if run.law.lone_wolf()||!run.summoner()&&crate::mix(run.reward_seed() ^ node as u64 ^ 0xa549) % 4 != 0 {
        offers.retain(|r|!matches!(r,Reward::Item(id) if lookup(*id).is_some_and(|i|i.kind==ItemKind::Spirit)));
    }
    offers.truncate(3);
    while offers.len()<3 {offers.push(Reward::Gold(75));}
    offers
}
pub(crate) fn before_spirit_weight(run:&Run,node:usize)->Vec<Reward> {
    let mut offers=ranked(run,node,true,&CATALOG);offers.truncate(3);
    while offers.len()<3 {offers.push(Reward::Gold(75));} offers
}
pub(crate) fn previous_offers(run:&Run,node:usize)->Vec<Reward> {
    let mut offers=ranked(run,node,false,&OLD_CATALOG);
    offers.truncate(2);
    while offers.len()<2 {offers.push(Reward::Gold(75));}
    offers.push(if run.weapon_level<25 {Reward::UpgradeWeapon}else{Reward::Gold(100)});
    offers
}
pub(crate) fn before_spirits(run:&Run,node:usize)->Vec<Reward> {
    let mut offers=ranked(run,node,false,&OLD_CATALOG);offers.truncate(3);
    while offers.len()<3 {offers.push(Reward::Gold(75));}
    offers
}
pub(crate) fn before_expansion(run:&Run,node:usize)->Vec<Reward> {
    let mut offers=ranked(run,node,true,&OLD_CATALOG);offers.truncate(3);
    while offers.len()<3 {offers.push(Reward::Gold(75));} offers
}
fn ranked(run:&Run,node:usize,spirits:bool,catalog:&[Item])->Vec<Reward> {
    ranked_pool(run,node,spirits,catalog,false)
}
fn ranked_pool(run:&Run,node:usize,spirits:bool,catalog:&[Item],base_only:bool)->Vec<Reward> {
    let mut categories=if catalog.iter().any(|i|i.kind==ItemKind::Talisman) {vec![]}else{vec![run.legacy_offers(node)[0].clone()]};
    for (index,kind) in [ItemKind::Weapon,ItemKind::Armor,ItemKind::Sorcery,ItemKind::Incantation,ItemKind::Spirit,ItemKind::Talisman,ItemKind::Tool].into_iter().enumerate() {
        if kind==ItemKind::Tool&&!base_only {continue;}
        if kind==ItemKind::Spirit&&!spirits {continue;}
        let mut pool:Vec<_>=catalog.iter().filter(|item|item.kind==kind&&kind.allowed(run.class)&&!run.claimed_items.contains(&item.id)&&crate::dlc::item_allowed(item.id)&&(!base_only||item.random_eligible())).collect();
        // 职业规则 1：武器约七成从职业擅长的类型里出
        if kind==ItemKind::Weapon&&run.preferences().is_empty()&&crate::mix(run.reward_seed()^(node as u64*131)^0xc1a5_5e5)%10<7 {
            let favored:Vec<_>=pool.iter().copied().filter(|i|run.class.weapon_types().contains(&((i.id&0x0fffffff)/1_000_000))).collect();
            if !favored.is_empty() {pool=favored;}
        }
        if !pool.is_empty() {
            let roll=crate::mix(run.reward_seed()^(node as u64*97+index as u64+1234));
            let choice=if kind==ItemKind::Weapon&&!run.preferences().is_empty() {Some(crate::weapon_training::pick_many(&pool.iter().map(|i|i.id).collect::<Vec<_>>(),roll,run.preferences(),Some(run.class)))}else{None};
            categories.push(Reward::Item(pool[choice.unwrap_or(roll as usize%pool.len())].id));
        }
    }
    let mut indexed:Vec<_>=categories.into_iter().enumerate().collect();
    indexed.sort_by_key(|(index,_)|crate::mix(run.reward_seed()^(node as u64*167+*index as u64+9876)));
    let mut result:Vec<_>=indexed.into_iter().map(|(_,reward)|reward).collect();
    // 职业规则 1：术士一半的奖励页必有魔法、祷誓者必有祷告，召唤师每页都有骨灰
    if let Some((kind,always))=favored_kind(run.class) {
        if always||crate::mix(run.reward_seed()^(node as u64*173)^0xfa70)%2==0 {
            if let Some(index)=result.iter().position(|r|matches!(r,Reward::Item(id) if lookup(*id).is_some_and(|i|i.kind==kind))) {let reward=result.remove(index);result.insert(0,reward);}
        }
    }
    result
}
/// 职业偏爱的奖励类型，以及是否每页都保证出现
fn favored_kind(class:Class)->Option<(ItemKind,bool)> {
    match class {Class::Astrologer=>Some((ItemKind::Sorcery,false)),Class::Prophet=>Some((ItemKind::Incantation,false)),Class::Confessor=>Some((ItemKind::Spirit,true)),_=>None}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spirit_rewards_stay_within_the_spirit_peak_plus_one() {
        let run=|spirit:Option<u8>,weapon:Option<u8>,rules:u8|{let mut r=crate::Run::new(1,crate::Class::Confessor);r.spirit_peak=spirit;r.weapon_peak=weapon;r.spirit_rules=rules;r};
        assert_eq!(spirit_cap(&run(None,None,0)),10);
        assert_eq!([0u8,4,5,24,45,50].map(|p|spirit_cap(&run(None,Some(p),0))),[1,1,2,5,10,10],"saves without a spirit record keep the weapon rule");
        assert_eq!([0u8,1,5,9,10].map(|p|spirit_cap(&run(Some(p),Some(50),0))),[1,2,6,10,10],"rule 0: the spirit record plus one");
        // 规则 1：上限就是当前骨灰等级 = max(持有过的骨灰最高等级, 武器等价强化等级)，不再 +1
        assert_eq!([0u8,1,5,9,10].map(|p|spirit_cap(&run(Some(p),None,1))),[0,1,5,9,10]);
        assert_eq!([0u8,3,7].map(|p|spirit_cap(&run(Some(p),Some(25),1))),[5,5,7],"the higher of the ash record and the weapon level");
        assert_eq!(spirit_cap(&run(Some(2),Some(50),1)),10);
        let fresh=crate::Run::new(7,crate::Class::Hero);assert_eq!((fresh.spirit_peak,fresh.spirit_rules),(Some(0),1));
        let mut seen=[false;11];
        for seed in 0..3000 {for peak in [Some(0u8),Some(3),Some(9),None] {
            let mut run=crate::Run::new(seed,crate::Class::Confessor);run.spirit_peak=peak;run.spirit_rules=0;
            let cap=spirit_cap(&run);
            for node in 0..run.nodes.len().min(12) {for reward in offers(&run,node) {
                if let Reward::Item(id)=reward {if lookup(id).is_some_and(|i|i.kind==ItemKind::Spirit) {
                    let level=id-spirit_base(id);assert!(level<=cap,"+{level} over +{cap}");
                    if peak.is_none(){seen[level as usize]=true;}
                }}
            }}
        }}
        assert!(seen.iter().all(|s|*s),"without any record every level still appears");
    }
    #[test]
    fn upgrade_stones_appear_in_combat_rewards_of_new_runs_only() {
        let (mut seen,mut total)=(0,0);
        for seed in 0..3000 {
            let mut run=Run::new(seed,Class::Samurai);
            let node=run.start_nodes()[0];run.enter(node).unwrap();run.win_combat().unwrap();
            let crate::Phase::Reward{ref offers,..}=run.phase else {panic!()};
            total+=1;if offers.contains(&Reward::UpgradeStone) {seen+=1;}
            assert!(offers.iter().filter(|r|**r==Reward::UpgradeStone).count()<=1);
            if offers.contains(&Reward::UpgradeStone) {
                let index=offers.iter().position(|r|*r==Reward::UpgradeStone).unwrap();
                let before=run.clone();run.claim_reward(index).unwrap();
                assert_eq!((run.weapon_level,run.gold,&run.claimed_items),(before.weapon_level,before.gold,&before.claimed_items),"the stone itself is delivered by the DLL");
            }
            let mut old=Run::new(seed,Class::Samurai);old.stone_rewards=false;
            let node=old.start_nodes()[0];old.enter(node).unwrap();old.win_combat().unwrap();
            let crate::Phase::Reward{ref offers,..}=old.phase else {panic!()};
            assert!(!offers.contains(&Reward::UpgradeStone),"saves from before keep their offers");
        }
        assert!((300..600).contains(&seen),"about {}% of {total}: {seen}",crate::STONE_CHANCE);
        for seed in 0..200 {
            let run=Run::new(seed,Class::Samurai);
            let boss=run.nodes.iter().position(|n|n.kind==crate::NodeKind::Boss).unwrap();
            assert!(!run.offers_for_test(boss).contains(&Reward::UpgradeStone),"no stones from bosses");
        }
    }
    #[test]
    fn tuning_catches_a_low_ash_up_to_the_peak_plus_one() {
        let ash=CATALOG.iter().find(|i|i.kind==ItemKind::Spirit).unwrap().id;
        assert_eq!(spirit_level(ash),Some(0));assert_eq!(spirit_level(ash+7),Some(7));
        assert_eq!(spirit_tune_target(ash,0),Some(ash+1));
        assert_eq!(spirit_tune_target(ash,4),Some(ash+5),"a +0 ash jumps past the best ash held");
        assert_eq!(spirit_tune_target(ash+6,4),Some(ash+7),"a better ash still gains one level");
        assert_eq!(spirit_tune_target(ash+9,10),Some(ash+10));
        assert_eq!(spirit_tune_target(ash+10,3),None);
        let weapon=CATALOG.iter().find(|i|i.kind==ItemKind::Weapon).unwrap().id;
        assert_eq!(spirit_tune_target(weapon,0),None);assert_eq!(spirit_level(weapon),None);
    }
    #[test]
    fn pending_rewards_from_before_whetblades_still_load() {
        for seed in 0..60 {
            let mut run=Run::new(seed,Class::Samurai);run.ember_rewards=false;run.stone_rewards=false;
            let node=run.start_nodes()[0];run.enter(node).unwrap();run.win_combat().unwrap();
            run.phase=crate::Phase::Reward{id:node,offers:before_whetblades(&run,node)};
            let loaded=Run::decode(&run.encode().unwrap()).unwrap();
            assert_eq!(loaded.phase,crate::Phase::Reward{id:node,offers:offers(&loaded,node)});
        }
    }
    #[test]
    fn casting_requirements_are_independent() {
        for tools in [[false,false],[true,false],[false,true],[true,true]] {
            for item in CATALOG.iter() {
                assert_eq!(castable(item.id,tools),match item.kind {ItemKind::Sorcery=>tools[0],ItemKind::Incantation=>tools[1],_=>true});
            }
        }
    }
    #[test]
    fn catalyst_snapshot_filters_all_reward_nodes_and_survives_save() {
        for class in [Class::Astrologer,Class::Prophet] {
            for seed in 0..50 {
                let mut run=Run::new(seed,class);
                run.casting_tools=Some([false,false]);
                for node in 0..run.nodes.len() {
                    assert!(offers(&run,node).iter().all(|r|!matches!(r,Reward::Item(id) if lookup(*id).is_some_and(|i|matches!(i.kind,ItemKind::Sorcery|ItemKind::Incantation)))));
                }
                let node=run.available()[0];run.enter(node).unwrap();run.win_combat().unwrap();
                assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
            }
        }
    }
    #[test]
    fn chest_boosts_are_stable_personal_and_exclude_owned_buffs() {
        let mut counts=[0;3];let mut old_rare=0;
        for seed in 0..1000 {
            let mut run=Run::new_personal(seed,Class::Astrologer,seed+11);
            let Some(node)=run.nodes.iter().position(|n|n.kind==crate::NodeKind::Chest) else{continue;};
            let rewards=offers(&run,node);
            old_rare+=before_chest_boosts(&run,node).iter().filter(|r|matches!(r,Reward::Item(id) if crate::elite::EPIC.contains(id)||crate::elite::LEGENDARY.contains(id))).count();
            assert_eq!(rewards,offers(&run,node));assert_eq!(rewards.len(),3);
            assert!(rewards.iter().enumerate().all(|(i,r)|!rewards[..i].contains(r)));
            for r in &rewards {match r {
                Reward::TalismanBuff(_)=>counts[0]+=1,
                Reward::Item(id) if crate::elite::EPIC.contains(&crate::affixes::base(*id))||crate::elite::LEGENDARY.contains(&crate::affixes::base(*id))=>counts[1]+=1,
                Reward::Item(id) if lookup(*id).unwrap().kind==ItemKind::Crystal=>counts[2]+=1,
                _=>{},
            }}
            run.elite_buffs=crate::elite::BUFFS.to_vec();
            assert!(!offers(&run,node).iter().any(|r|matches!(r,Reward::TalismanBuff(_))));
        }
        assert!((190..310).contains(&counts[0]),"{counts:?}");
        assert!(counts[1]>old_rare&&counts[1]<old_rare+250,"{counts:?}, old={old_rare}");
        assert!((45..115).contains(&counts[2]),"{counts:?}");
    }
    #[test]
    fn old_chest_pages_migrate_and_buff_claims_persist_once() {
        use crate::{NodeKind,Phase};
        let mut claimed=false;
        for seed in 0..60 {
            let mut run=Run::new(seed,Class::Samurai);
            for _ in 0..run.floors()-1 {
                let available=run.available();
                let id=available.iter().copied().find(|id|run.nodes[*id].kind==NodeKind::Chest).unwrap_or(available[0]);
                run.enter(id).unwrap();
                match run.nodes[id].kind {
                    NodeKind::Chest=>{
                        run.phase=Phase::Reward{id,offers:before_chest_boosts(&run,id)};
                        let mut loaded=Run::decode(&run.encode().unwrap()).unwrap();
                        let expected=offers(&loaded,id);
                        assert_eq!(loaded.phase,Phase::Reward{id,offers:expected.clone()});
                        if let Some(index)=expected.iter().position(|r|matches!(r,Reward::TalismanBuff(_))) {
                            let Reward::TalismanBuff(buff)=expected[index] else{unreachable!()};
                            loaded.claim_reward(index).unwrap();assert!(loaded.elite_buffs.contains(&buff));
                            assert!(loaded.claim_reward(index).is_err());
                            assert_eq!(Run::decode(&loaded.encode().unwrap()).unwrap(),loaded);claimed=true;
                        }
                        break;
                    },
                    NodeKind::Monster|NodeKind::Elite=>{run.win_combat().unwrap();run.decline_reward().unwrap();},
                    NodeKind::Camp=>run.leave_camp().unwrap(),NodeKind::Shop=>run.leave_shop().unwrap(),
                    NodeKind::Event=>run.resolve_event(false).unwrap(),_=>unreachable!(),
                }
            }
        }
        assert!(claimed);
    }
    #[test]
    fn current_currency_fallback_is_1500_and_old_rewards_migrate() {
        let mut run=(0..100).map(|seed|{let mut run=Run::new(seed,Class::Samurai);run.ember_rewards=false;run.stone_rewards=false;run.claimed_items.extend(CATALOG.iter().filter(|i|i.kind==ItemKind::Tool).map(|i|i.id));run}).find(|run|offers(run,run.start_nodes()[0]).iter().any(|r|matches!(r,Reward::Gold(_)))).unwrap();
        let node=run.start_nodes()[0];
        run.enter(node).unwrap();run.win_combat().unwrap();
        let old=before_bosses(&run,node);
        assert!(old.contains(&Reward::Gold(75)));
        let expected=offers(&run,node);
        assert!(expected.contains(&Reward::Gold(1500)));
        assert!(!expected.contains(&Reward::Gold(75)));
        run.phase=crate::Phase::Reward{id:node,offers:old};
        let restored=Run::decode(&run.encode().unwrap()).unwrap();
        assert_eq!(restored.phase,crate::Phase::Reward{id:node,offers:expected});
    }
    #[test]
    fn legacy_reward_cards_exclude_all_armor() {
        assert!(CATALOG.iter().any(|i|i.kind==ItemKind::Armor));
        for seed in 0..80 {
            let mut run=Run::new(seed,Class::Samurai);run.affix_equipment=false;
            for node in 0..run.nodes.len() {
                assert!(offers(&run,node).iter().all(|r|!matches!(r,Reward::Item(id) if lookup(*id).is_some_and(|i|i.kind==ItemKind::Armor))));
            }
        }
    }
    #[test]
    fn random_talismans_exclude_ineffective_rewards_but_keep_conditional_benefits() {
        for id in [1100,2140,4060,6040,6060,6070,6080,6090] {
            assert!(!lookup(0x20000000|id).unwrap().random_eligible(),"{id}");
        }
        for id in [1110,1140,1190,2040,2160,2210,6050,6110] {
            assert!(lookup(0x20000000|id).unwrap().random_eligible(),"{id}");
        }
    }
    #[test]
    fn fixed_seed_runs_draw_the_same_rewards() {
        // 大厅里输入固定种子开局时，奖励随机数也用这个种子（native/src/hub.rs）
        for seed in 1..20 {
            let (mut a,mut b)=(Run::new_personal(seed,Class::Samurai,seed),Run::new_personal(seed,Class::Samurai,seed));
            for step in 0..3 {
                assert_eq!(a.available(),b.available());
                for node in 0..a.nodes.len() {assert_eq!(offers(&a,node),offers(&b,node),"seed {seed} step {step} node {node}");}
                let id=a.available()[0];
                if a.nodes[id].kind!=crate::NodeKind::Monster {break;}
                for r in [&mut a,&mut b] {r.enter(id).unwrap();r.win_combat().unwrap();}
                assert_eq!(a.phase,b.phase);
                for r in [&mut a,&mut b] {r.claim_reward(0).unwrap();}
            }
        }
    }
    #[test]
    fn personal_draws_share_routes_but_not_reward_rng() {
        for seed in 1..40 {
            let mut a=Run::new_personal(seed,Class::Samurai,101);
            let b=Run::new_personal(seed,Class::Samurai,202);
            assert_eq!(a.nodes,b.nodes);
            assert_ne!(offers(&a,0),offers(&b,0));
            assert_ne!(crate::elite::boss_offers(&a,50),crate::elite::boss_offers(&b,50));
            assert_ne!(crate::elite::offers(&a,20),crate::elite::offers(&b,20));
            let before=offers(&a,0);
            a.align_party_start(seed+40).unwrap();
            assert_eq!(offers(&a,0),before);
            a.enter(0).unwrap();a.win_combat().unwrap();
            assert_eq!(Run::decode(&a.encode().unwrap()).unwrap(),a);
        }
    }
    #[test]
    fn legacy_saves_keep_their_draws_and_personal_seed_survives_acts() {
        let legacy=Run::new(42,Class::Samurai);
        let mut json=serde_json::to_value(&legacy).unwrap();
        json.as_object_mut().unwrap().remove("personal_reward_seed");
        let loaded=Run::decode(&json.to_string()).unwrap();
        assert_eq!(offers(&legacy,0),offers(&loaded,0));
        let mut a=Run::new_personal(42,Class::Samurai,101);
        let mut b=Run::new_personal(42,Class::Samurai,202);
        let first=a.reward_seed();
        for act in 1..=3 {
            a.act=act;b.act=act;
            assert_ne!(a.reward_seed(),b.reward_seed());
            a.enable_contracts();b.enable_contracts();
            assert_eq!(a.contract,Some(crate::contracts::State::start(a.reward_seed(),a.class,act,vec![],0)));
            a.contract=None;b.contract=None;
        }
        assert_ne!(first,a.reward_seed());
    }
    #[test]
    fn old_boss_reward_pages_migrate_to_three_legendaries() {
        use crate::{NodeKind,Phase};
        for seed in 0..40 {
            let mut run=Run::new(seed,Class::Samurai);
            while run.visited.len()+1<run.floors() {
                let id=run.available()[0];run.enter(id).unwrap();
                match run.nodes[id].kind {
                    NodeKind::Monster|NodeKind::Elite=>{run.win_combat().unwrap();run.decline_reward().unwrap();},
                    NodeKind::Camp=>run.leave_camp().unwrap(),NodeKind::Shop=>run.leave_shop().unwrap(),
                    NodeKind::Chest=>run.finish_chest().unwrap(),NodeKind::Event=>run.resolve_event(false).unwrap(),_=>unreachable!(),
                }
            }
            let id=run.available()[0];run.enter(id).unwrap();run.win_combat().unwrap();
            run.phase=Phase::Reward{id,offers:before_bosses(&run,id)};
            let loaded=Run::decode(&run.encode().unwrap()).unwrap();
            assert_eq!(loaded.phase,Phase::Reward{id,offers:offers(&run,id)});
        }
    }
    #[test]
    fn crystal_rewards_are_rare_unique_and_legacy_offers_migrate() {
        let mut count=0;
        for seed in 0..1000 {
            let mut run=Run::new(seed,Class::Samurai);
            let is_crystal=|r:&Reward|matches!(r,Reward::Item(id) if lookup(*id).unwrap().kind==ItemKind::Crystal);
            let offered=offers(&run,0);assert_eq!(offered.len(),3);
            count+=usize::from(offered.iter().any(is_crystal));
            if let Some(Reward::Item(id))=offered.iter().find(|r|is_crystal(r)) {
                run.claimed_items.push(*id);assert!(!offers(&run,0).contains(&Reward::Item(*id)));
            }
            run.enter(0).unwrap();run.win_combat().unwrap();
            run.phase=crate::Phase::Reward{id:0,offers:before_elites(&run,0)};
            assert!(Run::decode(&run.encode().unwrap()).is_ok());
        }
        assert!((25..=75).contains(&count));
        let mut run=Run::new(1,Class::Samurai);
        run.claimed_items=CATALOG.iter().filter(|i|i.kind==ItemKind::Crystal).map(|i|i.id).collect();
        assert!((0..1000).all(|v|crystal_offer(&run,v,0xc715).is_none()));
    }
    #[test]
    fn old_numbered_talisman_offers_migrate_without_removing_owned_items() {
        let mut migrated=0;
        for seed in 0..500 {
            let mut run=Run::new(seed,Class::Samurai);run.ember_rewards=false;run.stone_rewards=false;run.claimed_items.push(0x20000000|1001);
            run.enter(0).unwrap();run.win_combat().unwrap();
            let old=before_base_talismans(&run,0);
            if !old.iter().any(|r|matches!(r,Reward::Item(id) if !lookup(*id).unwrap().random_eligible())){continue;}
            run.phase=crate::Phase::Reward{id:0,offers:old};
            let restored=Run::decode(&run.encode().unwrap()).unwrap();
            assert_eq!(restored.claimed_items,run.claimed_items);
            assert_eq!(restored.phase,crate::Phase::Reward{id:0,offers:offers(&run,0)});migrated+=1;
        }
        assert!(migrated>0);
    }
    #[test]
    fn spirit_offers_are_rarer_and_old_pending_rewards_remain_loadable() {
        let mut old=0;let mut new=0;
        let contains=|rewards:&[Reward]|rewards.iter().any(|r|matches!(r,Reward::Item(id) if lookup(*id).is_some_and(|i|i.kind==ItemKind::Spirit)));
        for seed in 0..1000 {
            let mut run=Run::new(seed,Class::Samurai);run.ember_rewards=false;run.stone_rewards=false;
            run.claimed_items.extend(CATALOG.iter().filter(|i|i.kind==ItemKind::Tool).map(|i|i.id));
            let mut unweighted=ranked_pool(&run,0,true,&CATALOG,true);
            unweighted.truncate(3);
            old+=usize::from(contains(&unweighted));
            new+=usize::from(contains(&offers(&run,0)));
            if seed<30 {
                run.enter(0).unwrap();run.win_combat().unwrap();
                run.phase=crate::Phase::Reward{id:0,offers:before_spirit_weight(&run,0)};
                assert!(Run::decode(&run.encode().unwrap()).is_ok());
            }
        }
        assert!(new>0&&new*3<old&&new*5>old);
    }
    #[test]
    fn rewards_saved_before_tool_rate_change_still_load() {
        for seed in 0..60 {
            let mut run=Run::new(seed,Class::Samurai);run.ember_rewards=false;run.stone_rewards=false;
            let node=run.start_nodes()[0];run.enter(node).unwrap();run.win_combat().unwrap();
            run.phase=crate::Phase::Reward{id:node,offers:before_tool_rate(&run,node)};
            let restored=Run::decode(&run.encode().unwrap()).unwrap();
            assert_eq!(restored.phase,crate::Phase::Reward{id:node,offers:offers(&restored,node)});
        }
    }
    #[test]
    fn previous_version_pending_rewards_still_load() {
        for seed in 0..30 {
            let mut run=Run::new(seed,Class::Samurai);run.ember_rewards=false;run.stone_rewards=false;run.enter(0).unwrap();run.win_combat().unwrap();
            run.phase=crate::Phase::Reward{id:0,offers:run.legacy_offers(0)};
            let mut json:serde_json::Value=serde_json::from_str(&run.encode().unwrap()).unwrap();
            json.as_object_mut().unwrap().remove("claimed_items");
            let loaded=Run::decode(&json.to_string()).unwrap();
            assert_eq!(loaded.phase,crate::Phase::Reward{id:0,offers:offers(&run,0)});
            run.phase=crate::Phase::Reward{id:0,offers:previous_offers(&run,0)};
            let loaded=Run::decode(&run.encode().unwrap()).unwrap();
            assert_eq!(loaded.phase,crate::Phase::Reward{id:0,offers:offers(&run,0)});
            run.phase=crate::Phase::Reward{id:0,offers:before_spirits(&run,0)};
            let loaded=Run::decode(&run.encode().unwrap()).unwrap();
            assert_eq!(loaded.phase,crate::Phase::Reward{id:0,offers:offers(&run,0)});
            run.phase=crate::Phase::Reward{id:0,offers:before_expansion(&run,0)};
            let loaded=Run::decode(&run.encode().unwrap()).unwrap();
            assert_eq!(loaded.phase,crate::Phase::Reward{id:0,offers:offers(&run,0)});
        }
    }
}
#[cfg(test)]
mod claimed_base_tests {
    use crate::{Class,Phase,Run};
    #[test]
    fn pending_page_from_variant_blind_rules_still_loads_and_migrates() {
        let mut run=Run::new(7,Class::Samurai);run.ember_rewards=false;run.stone_rewards=false;
        let id=run.available()[0];run.enter(id).unwrap();run.win_combat().unwrap();
        let variant=crate::affixes::roll(&run,3150000,0,true).unwrap();run.claimed_items.push(variant);
        let old=super::before_claimed_bases(&run,id);run.phase=Phase::Reward{id,offers:old};
        let loaded=Run::decode(&run.encode().unwrap()).unwrap();
        assert_eq!(loaded.phase,Phase::Reward{id,offers:super::offers(&run,id)});
    }
}
