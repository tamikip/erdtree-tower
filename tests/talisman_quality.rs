use std::collections::BTreeMap;

fn tiers() -> BTreeMap<u32,u8> {
    include_str!("../scene-assets/reward-rarities.tsv").lines().map(|l| {
        let (id,tier)=l.split_once('\t').unwrap();
        (id.parse().unwrap(),tier.parse().unwrap())
    }).collect()
}

#[test]
fn vanilla_talismans_reflect_tower_utility() {
    let tiers=tiers();
    for (id,tier) in [(1000,0),(1160,0),(1191,0),(1140,1),(1051,2),
        (1231,3),(1250,3),(2081,3),(2160,3),(4003,3),(7030,3),(8040,3),(8190,1)] {
        assert_eq!(tiers[&(0x20000000|id)],tier,"talisman {id}");
    }
}

#[test]
fn balance_table_covers_every_vanilla_talisman() {
    let tiers=tiers();
    let mut expected=BTreeMap::new();
    for line in include_str!("../scene-assets/vanilla-talisman-quality.tsv").lines() {
        let fields:Vec<_>=line.split('\t').collect();
        let id=0x20000000|fields[0].parse::<u32>().unwrap();
        let tier=fields[1].parse::<u8>().unwrap();
        assert!(tier<=3);
        assert!(expected.insert(id,tier).is_none(),"duplicate {id}");
        assert_eq!(erdtree_tower::rewards::lookup(id).unwrap().name,fields[2]);
    }
    let actual: BTreeMap<_,_>=tiers.into_iter().filter(|(id,_)|id>>28==2).collect();
    assert_eq!(actual,expected);
}

#[test]
fn upgraded_talismans_never_have_lower_quality() {
    let tiers=tiers();
    for family in [&[1000,1001,1002,7000][..],&[1010,1011,1012,7010],
        &[1020,1021,1022,7020],&[1030,1031,1032],&[1040,1041,1042],
        &[4000,4001,4002,4003],&[4010,4011,4012,7090],&[4020,4021,4022,7100],
        &[4030,4031,4032,7110],&[4040,4041,4042,7120],&[4050,4051,4052,7130],
        &[1150,7030],&[1230,1231],&[2080,2081]] {
        for pair in family.windows(2) {
            assert!(tiers[&(0x20000000|pair[0])]<=tiers[&(0x20000000|pair[1])],"{pair:?}");
        }
    }
}
