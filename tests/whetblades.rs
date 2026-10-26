use erdtree_tower::{Class,Run,NodeKind,Reward,rewards};

#[test]
fn all_six_whetblades_are_obtainable_and_claimed_blades_do_not_repeat() {
    let ids=[8590,8970,8971,8972,8973,8974].map(|id|0x40000000|id);
    for id in ids {assert!(rewards::lookup(id).is_some(),"missing whetblade {id}");assert!(rewards::progression_good(id));}
    let mut seen=std::collections::HashSet::new();
    for seed in 0..1000 {
        let mut run=Run::new(seed,Class::Samurai);
        let node=run.nodes.iter().position(|n|n.kind==NodeKind::Monster).unwrap();
        for reward in rewards::offers(&run,node) {if let Reward::Item(id)=reward {if ids.contains(&id){seen.insert(id);}}}
        run.claimed_items.extend(ids);
        assert!(!rewards::offers(&run,node).iter().any(|r|matches!(r,Reward::Item(id) if ids.contains(id))));
    }
    assert_eq!(seen.len(),6);
}
