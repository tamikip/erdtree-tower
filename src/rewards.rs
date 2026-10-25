use crate::{Class,Reward,Run};
use std::sync::LazyLock;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ItemKind {Weapon,Armor,Sorcery,Incantation,Spirit,Talisman,Crystal}
impl ItemKind {
    pub fn label(self)->&'static str {
        match self {Self::Weapon=>"武器",Self::Armor=>"防具",Self::Sorcery=>"魔法",Self::Incantation=>"祷告",Self::Spirit=>"骨灰",Self::Talisman=>"护符",Self::Crystal=>"结晶露滴"}
    }
    pub fn allowed(self,class:Class)->bool {
        match self {Self::Sorcery=>class.sorcerer(),Self::Incantation=>class.cleric(),_=>true}
    }
}
pub struct Item {pub id:u32,pub name:&'static str,pub kind:ItemKind}
impl Item {
    pub fn random_eligible(&self)->bool {
        self.id!=(0x40000000|11019)&&self.kind!=ItemKind::Armor&&(self.kind!=ItemKind::Talisman||(!self.name.contains(['+','＋'])
            &&!matches!(self.id&0x0fffffff,1100|2140|4060|6040|6060|6070|6080|6090)))
    }
}
pub static CATALOG:LazyLock<Vec<Item>>=LazyLock::new(||parse(include_str!("../scene-assets/reward-catalog.txt")).into_iter().chain(parse(include_str!("../scene-assets/crystal-tears.txt"))).chain(parse(include_str!("../scene-assets/ruby-spells-catalog.txt"))).collect());
static RUBY_SPELL_IDS:LazyLock<std::collections::HashSet<u32>>=LazyLock::new(||parse(include_str!("../scene-assets/ruby-spells-catalog.txt")).iter().map(|i|i.id).collect());
static OLD_CATALOG:LazyLock<Vec<Item>>=LazyLock::new(||parse(include_str!("../scene-assets/reward-catalog-v29.txt")));
fn parse(text:&'static str)->Vec<Item> {text.lines().map(|line| {
    let mut parts=line.split('|');
    let (kind,tag)=match parts.next().unwrap() {
        "weapon"=>(ItemKind::Weapon,0),"armor"=>(ItemKind::Armor,0x10000000),
        "talisman"=>(ItemKind::Talisman,0x20000000),
        "crystal"=>(ItemKind::Crystal,0x40000000),
        "sorcery"=>(ItemKind::Sorcery,0x40000000),"incantation"=>(ItemKind::Incantation,0x40000000),"spirit"=>(ItemKind::Spirit,0x40000000),_=>panic!("Invalid reward catalog"),
    };
    Item{id:tag|parts.next().unwrap().parse::<u32>().unwrap(),name:parts.next().unwrap(),kind}
}).collect()}
static SPIRIT_LEVELS:LazyLock<Vec<Item>>=LazyLock::new(||CATALOG.iter().filter(|i|i.kind==ItemKind::Spirit).flat_map(|i|(1..=10).map(move |level|Item{
    id:i.id+level,name:Box::leak(format!("{} +{level}",i.name).into_boxed_str()),kind:ItemKind::Spirit,
})).collect());
pub fn lookup(id:u32)->Option<&'static Item> {crate::affixes::lookup(id).or_else(||CATALOG.iter().find(|item|item.id==id)).or_else(||SPIRIT_LEVELS.iter().find(|item|item.id==id))}
pub fn spirit_base(id:u32)->u32 {
    let base=id-(id&0x0fffffff)%1000;
    if id-base<=10&&CATALOG.iter().any(|i|i.id==base&&i.kind==ItemKind::Spirit){base}else{id}
}
pub fn random_spirit_level(id:u32,entropy:u64)->u32 {
    let base=spirit_base(id);
    if !CATALOG.iter().any(|i|i.id==base&&i.kind==ItemKind::Spirit){return id;}
    base+(crate::mix(entropy^u64::from(base)^0xa541_e7e1)%11) as u32
}

pub fn offers(run:&Run,node:usize)->Vec<Reward> {
    crate::affixes::rewards(run,node,before_affixes(run,node))
}
pub(crate) fn before_affixes(run:&Run,node:usize)->Vec<Reward> {
    let mut result=before_spirit_levels(run,node);
    for reward in &mut result {if let Reward::Item(id)=reward {
        *id=random_spirit_level(*id,run.reward_seed()^(node as u64).wrapping_mul(7919));
        if lookup(*id).is_some_and(|i|i.kind==ItemKind::Spirit)&&run.claimed_items.contains(id) {
            let base=spirit_base(*id);let level=*id-base;
            *reward=(1..=10).map(|offset|base+(level+offset)%11).find(|id|!run.claimed_items.contains(id))
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
                result[index]=if pool.is_empty(){Reward::Gold(1500)}else{Reward::Item(pool[crate::mix(run.reward_seed()^node as u64^index as u64) as usize%pool.len()].id)};
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
        if !pool.is_empty(){result[0]=Reward::Item(pool[crate::mix(roll^3) as usize%pool.len()]);}
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
fn weighted(run:&Run,node:usize,base_only:bool)->Vec<Reward> {
    let mut offers=ranked_pool(run,node,true,&CATALOG,base_only);
    if crate::mix(run.reward_seed() ^ node as u64 ^ 0xa549) % 4 != 0 {
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
    for (index,kind) in [ItemKind::Weapon,ItemKind::Armor,ItemKind::Sorcery,ItemKind::Incantation,ItemKind::Spirit,ItemKind::Talisman].into_iter().enumerate() {
        if kind==ItemKind::Spirit&&!spirits {continue;}
        let pool:Vec<_>=catalog.iter().filter(|item|item.kind==kind&&kind.allowed(run.class)&&!run.claimed_items.contains(&item.id)&&(!base_only||item.random_eligible())&&(run.ruby_spells||!RUBY_SPELL_IDS.contains(&item.id))).collect();
        if !pool.is_empty() {
            let roll=crate::mix(run.reward_seed()^(node as u64*97+index as u64+1234));
            categories.push(Reward::Item(pool[roll as usize%pool.len()].id));
        }
    }
    let mut indexed:Vec<_>=categories.into_iter().enumerate().collect();
    indexed.sort_by_key(|(index,_)|crate::mix(run.reward_seed()^(node as u64*167+*index as u64+9876)));
    indexed.into_iter().map(|(_,reward)|reward).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut run=(0..100).map(|seed|Run::new(seed,Class::Samurai)).find(|run|offers(run,run.start_nodes()[0]).iter().any(|r|matches!(r,Reward::Gold(_)))).unwrap();
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
            assert_eq!(a.contract,Some(crate::contracts::State::new(a.reward_seed(),a.class,act,None,0)));
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
            let mut run=Run::new(seed,Class::Samurai);run.claimed_items.push(0x20000000|1001);
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
            let mut run=Run::new(seed,Class::Samurai);
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
    fn previous_version_pending_rewards_still_load() {
        for seed in 0..30 {
            let mut run=Run::new(seed,Class::Samurai);run.enter(0).unwrap();run.win_combat().unwrap();
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
