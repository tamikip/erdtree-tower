use erdtree_tower::{Run,Class,rewards::*};

#[test]
fn every_spirit_supports_39_levels_and_stops_at_the_boundary() {
    for ash in CATALOG.iter().filter(|i|i.kind==ItemKind::Spirit) {
        assert_eq!(spirit_tune_target(ash.id+10,10),Some(ash.id+11));
        assert_eq!(spirit_tune_target(ash.id,38),Some(ash.id+39));
        assert_eq!(spirit_tune_target(ash.id+39,39),None);
        for level in 0..=39 {
            assert_eq!(spirit_level(ash.id+level),Some(level as u8));
            assert_eq!(spirit_base(ash.id+level),ash.id);
            assert!(lookup(ash.id+level).is_some());
        }
        assert_eq!(spirit_level(ash.id+40),None);
    }
}

#[test]
fn high_level_ashes_survive_save_and_reward_generation() {
    let mut run=Run::new(42,Class::Confessor);run.spirit_peak=Some(39);
    assert_eq!(spirit_cap(&run),39);
    assert_eq!(Run::decode(&run.encode().unwrap()).unwrap().spirit_peak,Some(39));
    let ash=CATALOG.iter().find(|i|i.kind==ItemKind::Spirit).unwrap().id;
    let levels:std::collections::BTreeSet<_>=(0..10000).map(|seed|random_spirit_level(ash,seed,39)-ash).collect();
    assert_eq!(levels,(0..=39).collect());
    run.spirit_peak=Some(40);assert!(run.validate().is_err());
}
