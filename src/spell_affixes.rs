use std::{collections::HashMap,sync::LazyLock};
use crate::{Run,rewards::{Item,ItemKind}};

pub struct Variant {pub item:Item,pub base:u32,pub tier:u8,pub kind:u8}
pub static VARIANTS:LazyLock<Vec<Variant>>=LazyLock::new(||include_str!("../scene-assets/spell-affix-catalog.txt").lines().map(|line|{
    let p:Vec<_>=line.split('|').collect();
    Variant{item:Item{id:0x40000000|p[1].parse::<u32>().unwrap(),name:p[4],kind:match p[0]{"sorcery"=>ItemKind::Sorcery,"incantation"=>ItemKind::Incantation,_=>panic!("Invalid spell category")}},
        base:0x40000000|p[2].parse::<u32>().unwrap(),tier:p[3].parse().unwrap(),kind:p[5].parse().unwrap()}
}).collect());
static BY_ID:LazyLock<HashMap<u32,usize>>=LazyLock::new(||VARIANTS.iter().enumerate().map(|(i,v)|(v.item.id,i)).collect());
static BY_BASE:LazyLock<HashMap<u32,Vec<usize>>>=LazyLock::new(||{
    let mut rows:HashMap<u32,Vec<usize>>=HashMap::new();
    for (i,v) in VARIANTS.iter().enumerate(){rows.entry(v.base).or_default().push(i);}rows
});
pub fn variant(id:u32)->Option<&'static Variant>{BY_ID.get(&id).map(|i|&VARIANTS[*i])}
pub fn base(id:u32)->u32{variant(id).map_or(id,|v|v.base)}
pub fn supported(id:u32)->bool {BY_BASE.contains_key(&base(id))}
pub fn roll(run:&Run,id:u32,entropy:u64,repeat:bool,min:u8)->Option<u32> {
    let Some(rows)=BY_BASE.get(&base(id)) else{return Some(id);};
    if !run.spell_affixes{return Some(id);}
    let value=crate::mix(run.reward_seed()^entropy^u64::from(id)^0x5ae11)%100;
    let mut remaining=value as u32;let mut tier=3;
    for (i,odds) in crate::affixes::quality_odds(run).into_iter().enumerate(){if remaining<odds{tier=i as u8;break;}remaining-=odds;}
    tier=tier.max(min.min(3));
    let choices:Vec<_>=rows.iter().map(|i|&VARIANTS[*i]).filter(|v|v.tier==tier).collect();
    let start=crate::mix(entropy^run.reward_seed()^0x5ae12) as usize%choices.len();
    (0..choices.len()).map(|n|choices[(start+n)%choices.len()].item.id).find(|id|!run.claimed_items.contains(id))
        .or_else(||repeat.then_some(choices[start].item.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_spell_has_four_tiers_without_duplicate_ids() {
        assert_eq!(BY_ID.len(),VARIANTS.len());
        for item in crate::rewards::CATALOG.iter().filter(|i|matches!(i.kind,ItemKind::Sorcery|ItemKind::Incantation)) {
            let rows=&BY_BASE[&item.id];
            for i in rows {let v=&VARIANTS[*i];assert_eq!(v.item.kind,item.kind);assert!(v.kind<8&&v.tier<4);
                for tier in 0..4{assert_eq!(rows.iter().filter(|j|VARIANTS[**j].kind==v.kind&&VARIANTS[**j].tier==tier).count(),1);}}
        }
    }
    #[test]
    fn spell_rolls_preserve_base_minimum_dlc_and_save_determinism() {
        let run=Run::new(91,crate::Class::Astrologer);
        for &id in BY_BASE.keys(){for min in 0..4 {
            let selected=roll(&run,id,42,false,min).unwrap();let v=variant(selected).unwrap();
            assert_eq!(v.base,id);assert!(v.tier>=min);
            assert_eq!(crate::dlc::is_dlc_item(selected),crate::dlc::is_dlc_item(id));
            assert_eq!(roll(&Run::decode(&run.encode().unwrap()).unwrap(),id,42,false,min),Some(selected));
        }}
    }
    #[test]
    fn old_runs_keep_original_spells_and_claimed_variants_are_not_repeated() {
        let mut run=Run::new(4,crate::Class::Astrologer);let id=0x40000000|4000;
        run.spell_affixes=false;assert_eq!(roll(&run,id,3,false,3),Some(id));
        run.spell_affixes=true;run.claimed_items.extend(BY_BASE[&id].iter().map(|i|VARIANTS[*i].item.id));
        assert_eq!(roll(&run,id,3,false,0),None);assert!(roll(&run,id,3,true,0).is_some());
    }
}
