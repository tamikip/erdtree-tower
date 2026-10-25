use erdtree_tower::{Class,NodeKind,Phase,Reward,Run,elite,rewards};

#[test]
fn boss_always_offers_three_distinct_legendaries_for_every_class() {
    let mut seen=std::collections::HashSet::new();
    for class in Class::ALL {for seed in 0..100 {
        let run=Run::new(seed,class);
        let node=run.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap();
        let offers=rewards::offers(&run,node);
        assert_eq!(offers.len(),3);assert_eq!(offers,rewards::offers(&run,node));
        let mut ids=std::collections::HashSet::new();
        for reward in offers {
            let Reward::Item(id)=reward else {panic!("Boss reward must be a legendary item")};
            let base=erdtree_tower::affixes::base(id);
            assert!(elite::LEGENDARY.contains(&base));assert!(ids.insert(base));seen.insert(base);
        }
    }}
    assert_eq!(seen.len(),elite::LEGENDARY.len());
}

#[test]
fn collected_boss_pool_still_offers_three_usable_weapons() {
    let mut run=Run::new(99,Class::Samurai);
    run.claimed_items=elite::LEGENDARY.to_vec();
    let node=run.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap();
    let offers=rewards::offers(&run,node);
    assert_eq!(offers.len(),3);
    for reward in offers {
        let Reward::Item(id)=reward else {panic!("Expected weapon")};
        assert_eq!(rewards::lookup(id).unwrap().kind,rewards::ItemKind::Weapon);
    }
}

#[test]
fn boss_rewards_can_be_claimed_and_saved_in_all_three_acts() {
    for seed in 0..30 {
        let mut run=Run::new(seed,Class::Samurai);
        while !matches!(run.phase,Phase::Won) {
            match run.phase.clone() {
                Phase::Map=>{let node=run.available()[0];run.enter(node).unwrap();},
                Phase::Combat{..}=>run.win_combat().unwrap(),
                Phase::Reward{id,offers}=>{
                    let index=if run.nodes[id].kind==NodeKind::Boss {
                        offers.iter().position(|r|matches!(r,Reward::Item(id) if *id>>28==4)).unwrap_or(0)
                    }else{0};
                    run.claim_reward(index).unwrap();
                    run=Run::decode(&run.encode().unwrap()).unwrap();
                },
                Phase::Node{id}=>match run.nodes[id].kind {
                    NodeKind::Camp=>run.leave_camp().unwrap(),
                    NodeKind::Shop=>run.leave_shop().unwrap(),
                    NodeKind::Chest=>run.finish_chest().unwrap(),
                    NodeKind::Event=>run.resolve_event(false).unwrap(),
                    _=>unreachable!(),
                },
                _=>panic!("Unexpected phase"),
            }
        }
    }
}
