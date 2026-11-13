use std::{collections::HashMap,sync::LazyLock};
use crate::{Run,rewards::{Item,ItemKind}};

struct Variant {id:u32,tier:u8,kind:&'static str,name:&'static str}
static VARIANTS:LazyLock<Vec<Variant>>=LazyLock::new(||include_str!("../scene-assets/talisman-definitions.tsv").lines().map(|line|{
    let p:Vec<_>=line.split('\t').collect();Variant{id:0x20000000|p[0].parse::<u32>().unwrap(),tier:p[3].parse().unwrap(),kind:p[4],name:p[5]}
}).collect());
pub static CATALOG:LazyLock<Vec<Item>>=LazyLock::new(||VARIANTS.iter().map(|v|Item{id:v.id,name:v.name,kind:ItemKind::Talisman}).collect());
static INDEX:LazyLock<HashMap<u32,usize>>=LazyLock::new(||VARIANTS.iter().enumerate().map(|(i,v)|(v.id,i)).collect());
static GROUPS:LazyLock<Vec<Vec<usize>>>=LazyLock::new(||{
    let mut map=std::collections::BTreeMap::<&str,Vec<usize>>::new();
    for (i,v) in VARIANTS.iter().enumerate(){
        if matches!(v.kind,"experienced rider"|"finding"){continue;}
        map.entry(v.kind).or_default().push(i);
    }
    map.into_values().collect()
});
pub fn lookup(id:u32)->Option<&'static Item>{crate::talisman_passives::variant(id).map(|v|&v.item).or_else(||INDEX.get(&id).map(|i|&CATALOG[*i]))}
pub fn tier(id:u32)->Option<u8>{crate::talisman_passives::variant(id).map(|v|v.tier).or_else(||INDEX.get(&id).map(|i|VARIANTS[*i].tier))}
pub fn roll(run:&Run,original:u32,entropy:u64,repeat:bool)->u32 {
    roll_min(run,original,entropy,repeat,0)
}
pub fn roll_min(run:&Run,original:u32,entropy:u64,repeat:bool,min:u8)->u32 {
    if !run.talisman_affixes||original>>28!=2||lookup(original).is_some(){return original;}
    if run.talisman_passives&&let Some(id)=crate::talisman_passives::roll(run,original,entropy,repeat,min){return id;}
    let seed=crate::mix(run.reward_seed()^entropy^u64::from(original)^0x74616c69736d616e);
    if seed%2==0{return original;}
    let start=(seed>>8) as usize%GROUPS.len();
    let tier=match crate::mix(seed^0x726172697479)%100 {0..=44=>0,45..=74=>1,75..=92=>2,_=>3};
    for offset in 0..GROUPS.len() {
        let group=&GROUPS[(start+offset)%GROUPS.len()];
        let picked=group.iter().position(|i|VARIANTS[*i].tier>=tier).unwrap_or(group.len()-1);
        for n in 0..group.len(){let id=VARIANTS[group[(picked+n)%group.len()]].id;if repeat||!run.claimed_items.contains(&id){return id;}}
    }
    original
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn riding_and_finding_variants_remain_readable_but_never_roll() {
        let excluded:Vec<_>=VARIANTS.iter().enumerate().filter(|(_,v)|matches!(v.kind,"experienced rider"|"finding")).collect();
        assert_eq!(excluded.len(),8);
        for (index,v) in excluded {
            assert!(lookup(v.id).is_some());
            assert!(!GROUPS.iter().any(|g|g.contains(&index)),"{}",v.name);
        }
        assert_eq!(GROUPS.iter().map(Vec::len).sum::<usize>(),VARIANTS.len()-8);
    }
}
