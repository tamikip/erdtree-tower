use erdtree_tower::{Class,Run,rewards::{self,ItemKind}};

fn definitions()->Vec<Vec<&'static str>> {
    include_str!("../scene-assets/custom-spirit-definitions.tsv").lines().map(|l|l.split('|').collect()).collect()
}
#[test]
fn all_twelve_custom_spirits_have_names_quality_and_forty_levels() {
    let defs=definitions();assert_eq!(defs.len(),12);
    let rarities:std::collections::HashMap<u32,u8>=include_str!("../scene-assets/reward-rarities.tsv").lines().map(|l|{
        let (id,tier)=l.split_once('\t').unwrap();(id.parse().unwrap(),tier.parse().unwrap())
    }).collect();
    let mut ids=std::collections::HashSet::new();
    for d in defs {
        let base=0x40000000|d[0].parse::<u32>().unwrap();assert!(ids.insert(base));
        let item=rewards::lookup(base).expect("custom spirit in reward catalog");
        assert_eq!(item.kind,ItemKind::Spirit);assert_eq!(item.name,d[7]);assert!(item.random_eligible());
        assert_eq!(rarities[&base],d[4].parse::<u8>().unwrap());
        for level in 0..=39 {
            assert_eq!(rewards::spirit_level(base+level),Some(level as u8));
            assert!(rewards::lookup(base+level).is_some());
        }
        assert_eq!(rewards::spirit_tune_target(base+39,39),None);
    }
}

#[test]
fn custom_ashes_persist_and_can_roll_high_levels() {
    let mut run=Run::new(42,Class::Confessor);run.spirit_peak=Some(39);
    for d in definitions() {run.claimed_items.push(0x40000000|d[0].parse::<u32>().unwrap()+39);}
    let loaded=Run::decode(&run.encode().unwrap()).unwrap();assert_eq!(loaded.claimed_items,run.claimed_items);
    for id in loaded.claimed_items {
        let found:std::collections::HashSet<_>=(0..1000).map(|seed|rewards::random_spirit_level(id,seed,39)-rewards::spirit_base(id)).collect();
        assert!(found.contains(&39));
    }
}
