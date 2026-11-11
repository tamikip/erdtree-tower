use erdtree_tower::{Class,NodeKind,Run,law::Law};

#[test]
fn elite_pressure_only_replaces_later_monsters_and_survives_reload() {
    let law=Law::default().toggle(34);assert_eq!(law.score(),20);assert_eq!(law.0,1<<34);
    let (mut eligible,mut converted)=(0,0);
    for seed in 0..100 {
        let plain=Run::new(seed,Class::Samurai);let mut run=plain.clone();run.set_law(law).unwrap();
        for (old,new) in plain.nodes.iter().zip(&run.nodes) {
            assert_eq!((&old.next,old.layer,old.lane),(&new.next,new.layer,new.lane));
            if old.kind==NodeKind::Monster&&old.layer>=5 {
                eligible+=1;converted+=usize::from(new.kind==NodeKind::Elite);
                assert!(matches!(new.kind,NodeKind::Monster|NodeKind::Elite));
            } else {assert_eq!(old.kind,new.kind);}
        }
        assert_eq!(Run::decode(&run.encode().unwrap()).unwrap().nodes,run.nodes);
        let nodes=run.nodes.clone();run.set_law(law).unwrap();assert_eq!(run.nodes,nodes);
        run.set_law(Law::default()).unwrap();assert_eq!(run.nodes,plain.nodes);
    }
    assert!(converted*100>eligible*20&&converted*100<eligible*30,"{converted}/{eligible}");
    assert_eq!(Law::parse(&law.code()),Some(law));
}

#[test]
fn elite_pressure_matches_party_members_after_seed_alignment_and_debug_act_changes() {
    let law=Law::default().toggle(34);
    let mut host=Run::new_personal(42,Class::Samurai,111);
    let mut guest=Run::new_personal(99,Class::Hero,222);
    host.set_law(law).unwrap();guest.set_law(law).unwrap();guest.align_party_start(42).unwrap();
    assert_eq!(host.nodes,guest.nodes);
    for act in 1..=3 {
        host.debug_boss_test(0,act).unwrap();guest.debug_boss_test(0,act).unwrap();
        assert_eq!(host.nodes,guest.nodes);
        assert!(host.nodes.iter().filter(|n|n.layer<5).all(|n|n.kind!=NodeKind::Elite));
        host.validate().unwrap();guest.validate().unwrap();
    }
}
