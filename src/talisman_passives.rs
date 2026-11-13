use std::{collections::HashMap,sync::LazyLock};
use crate::{Run,rewards::{Item,ItemKind}};
pub struct Variant {pub item:Item,pub base:u32,pub tier:u8,pub effects:Vec<i32>}
pub static VARIANTS:LazyLock<Vec<Variant>>=LazyLock::new(||include_str!("../scene-assets/talisman-passives.tsv").lines().map(|line|{
    let p:Vec<_>=line.split('\t').collect();Variant{item:Item{id:0x20000000|p[0].parse::<u32>().unwrap(),name:p[4],kind:ItemKind::Talisman},
        base:0x20000000|p[1].parse::<u32>().unwrap(),tier:p[2].parse().unwrap(),effects:p[3].split(',').map(|s|s.parse().unwrap()).collect()}
}).collect());
static INDEX:LazyLock<HashMap<u32,usize>>=LazyLock::new(||VARIANTS.iter().enumerate().map(|(i,v)|(v.item.id,i)).collect());
static BASES:LazyLock<HashMap<u32,[u32;4]>>=LazyLock::new(||{
    let mut map=HashMap::new();for v in VARIANTS.iter(){map.entry(v.base).or_insert([0;4])[usize::from(v.tier)]=v.item.id;}map
});
pub fn variant(id:u32)->Option<&'static Variant>{INDEX.get(&id).map(|i|&VARIANTS[*i])}
pub fn base(id:u32)->u32{variant(id).map_or(id,|v|v.base)}
pub fn effects(id:u32)->&'static [i32]{variant(id).map_or(&[],|v|v.effects.as_slice())}
pub fn roll(run:&Run,id:u32,entropy:u64,repeat:bool,min:u8)->Option<u32> {
    let variants=BASES.get(&id)?;
    let mut value=(crate::mix(run.reward_seed()^entropy^u64::from(id)^0x7a115a)%100) as u32;let mut tier=3;
    for (i,odds) in crate::affixes::quality_odds(run).into_iter().enumerate(){if value<odds{tier=i as u8;break;}value-=odds;}
    let floor=min.min(3);tier=tier.max(floor);
    for n in 0..4 {let t=(tier+n)%4;if t<floor{continue;}let picked=variants[usize::from(t)];if repeat||!run.claimed_items.contains(&picked){return Some(picked);}}
    Some(id)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_original_talismans_keep_their_identity_and_gain_tier_passives() {
        assert_eq!(INDEX.len(),VARIANTS.len());
        for item in crate::rewards::CATALOG.iter().filter(|i|i.kind==ItemKind::Talisman) {
            for (tier,&id) in BASES[&item.id].iter().enumerate(){let v=variant(id).unwrap();
                assert_eq!(v.base,item.id);assert_eq!(v.effects.len(),[1,1,2,3][tier]);
                assert_eq!(v.effects.iter().collect::<std::collections::HashSet<_>>().len(),v.effects.len());
                assert!(v.effects.iter().all(|e|(9850000..9850210).contains(e)&&e%10==tier as i32));
                assert_eq!(crate::dlc::is_dlc_item(id),crate::dlc::is_dlc_item(item.id));
            }
        }
    }
    #[test]
    fn new_rolls_keep_the_base_and_floor_while_old_runs_stay_unchanged() {
        let mut run=Run::new(8,crate::Class::Samurai);let id=0x20000000|1000;
        let high=crate::affixes::roll_min(&run,id,5,false,3).unwrap();assert_eq!(variant(high).unwrap().tier,3);assert_eq!(base(high),id);
        assert_eq!(high,crate::affixes::roll_min(&Run::decode(&run.encode().unwrap()).unwrap(),id,5,false,3).unwrap());
        run.talisman_passives=false;assert!(variant(crate::talismans::roll(&run,id,5,false)).is_none());
    }
}
