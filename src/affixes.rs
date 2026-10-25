use std::{collections::HashMap,sync::LazyLock};
use crate::{Run,Reward,rewards::{Item,ItemKind}};

struct Variant {item:Item,base:u32,tier:u8,profile:Option<u8>}
static VARIANTS:LazyLock<Vec<Variant>>=LazyLock::new(||include_str!("../scene-assets/affix-catalog.txt").lines().chain(include_str!("../scene-assets/affix-v2-catalog.txt").lines()).map(|line| {
    let p:Vec<_>=line.split('|').collect();let tag=if p[0]=="weapon"{0}else{0x10000000};
    Variant{item:Item{id:tag|p[1].parse::<u32>().unwrap(),kind:if tag==0{ItemKind::Weapon}else{ItemKind::Armor},name:p[4]},base:tag|p[2].parse::<u32>().unwrap(),tier:p[3].parse().unwrap(),profile:p.get(5).map(|v|v.parse().unwrap())}
}).collect());
pub static CATALOG:LazyLock<Vec<Item>>=LazyLock::new(||VARIANTS.iter().map(|v|Item{id:v.item.id,kind:v.item.kind,name:v.item.name}).collect());
static BY_ID:LazyLock<HashMap<u32,usize>>=LazyLock::new(||VARIANTS.iter().enumerate().map(|(i,v)|(v.item.id,i)).collect());
static BY_BASE:LazyLock<HashMap<u32,[u32;4]>>=LazyLock::new(|| {
    let mut map=HashMap::new();for v in VARIANTS.iter().filter(|v|v.profile.is_none()){map.entry(v.base).or_insert([0;4])[v.tier as usize]=v.item.id;}map
});
static ADVANCED:LazyLock<HashMap<u32,Vec<[u32;4]>>>=LazyLock::new(|| {
    let mut map=HashMap::new();for v in VARIANTS.iter(){if let Some(profile)=v.profile {
        let groups=map.entry(v.base).or_insert_with(Vec::new);groups.resize(groups.len().max(profile as usize+1),[0;4]);groups[profile as usize][v.tier as usize]=v.item.id;
    }}map
});
static BASE_RARITY:LazyLock<HashMap<u32,u8>>=LazyLock::new(||include_str!("../scene-assets/reward-rarities.tsv").lines().map(|l|{let (id,tier)=l.split_once('\t').unwrap();(id.parse().unwrap(),tier.parse().unwrap())}).collect());
pub fn lookup(id:u32)->Option<&'static Item>{BY_ID.get(&id).map(|i|&CATALOG[*i])}
pub fn base(id:u32)->u32 {BY_ID.get(&id).map_or(id,|i|VARIANTS[*i].base)}
pub fn tier(id:u32)->Option<u8>{BY_ID.get(&id).map(|i|VARIANTS[*i].tier)}
pub fn profile(id:u32)->Option<u8>{BY_ID.get(&id).and_then(|i|VARIANTS[*i].profile)}
pub fn roll(run:&Run,id:u32,entropy:u64,repeat:bool)->Option<u32> {
    if !run.affix_equipment{return Some(id);}
    let groups=if run.affix_rules>=2 {ADVANCED.get(&id).map(Vec::as_slice)}else{BY_BASE.get(&id).map(std::slice::from_ref)};
    let Some(groups)=groups else{return Some(id)};
    let start=if run.affix_rules>=2{crate::mix(run.reward_seed()^entropy^u64::from(id)^0xa4c4_7e99) as usize%groups.len()}else{0};
    let floor=BASE_RARITY.get(&id).copied().unwrap_or(0).min(3);
    let value=crate::mix(run.reward_seed()^entropy^u64::from(id)^0xaff1_600d)%100;
    let selected=(match value {0..=44=>0,45..=74=>1,75..=92=>2,_=>3}).max(floor);
    for n in 0..groups.len() {
        let variants=&groups[(start+n)%groups.len()];
        if let Some(id)=(0..4).map(|offset|(selected+offset)%4).filter(|t|*t>=floor).map(|t|variants[t as usize]).find(|variant|*variant!=0&&!run.claimed_items.contains(variant)){return Some(id);}
    }
    repeat.then_some(groups[start][selected as usize])
}
pub fn rewards(run:&Run,node:usize,mut offers:Vec<Reward>)->Vec<Reward> {
    if !run.affix_equipment{return offers;}
    let entropy=(node as u64).wrapping_mul(7919);
    let normal=run.nodes.get(node).is_some_and(|n|matches!(n.kind,crate::NodeKind::Monster|crate::NodeKind::Chest));
    if normal&&crate::mix(run.reward_seed()^entropy^0xa420)%4==0 {
        if let Some(slot)=offers.iter().position(|r|matches!(r,Reward::Item(id) if crate::rewards::lookup(*id).is_some_and(|i|i.kind==ItemKind::Weapon))) {
            let armor:Vec<_>=crate::rewards::CATALOG.iter().filter(|i|i.kind==ItemKind::Armor).collect();
            let start=crate::mix(entropy^run.reward_seed()) as usize%armor.len();
            if let Some(item)=(0..armor.len()).map(|n|armor[(start+n)%armor.len()]).find(|i|roll(run,i.id,entropy^slot as u64,false).is_some()){offers[slot]=Reward::Item(item.id);}
        }
    }
    let repeat=run.nodes.get(node).is_some_and(|n|n.kind==crate::NodeKind::Boss);
    for (slot,reward) in offers.iter_mut().enumerate(){if let Reward::Item(id)=*reward {
        *reward=roll(run,id,entropy^slot as u64,repeat).map_or(Reward::Gold(1500),Reward::Item);
    }}
    offers
}
