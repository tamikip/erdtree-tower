use erdtree_tower::{Class, NodeKind, Phase, Run};

#[test]
fn leaving_camp_grants_nothing_and_cannot_skip_other_nodes() {
    let mut run = Run::new(42, Class::Samurai);
    assert!(run.leave_camp().is_err());
    let id = run
        .nodes
        .iter()
        .position(|node| node.kind == NodeKind::Camp)
        .unwrap();
    run.phase = Phase::Node { id };
    let resources = (
        run.flasks,
        run.gold,
        run.hp,
        run.weapon_level,
        run.blessings.clone(),
    );
    run.leave_camp().unwrap();
    assert_eq!(run.phase, Phase::Map);
    assert_eq!(run.visited.last(), Some(&id));
    assert_eq!(
        resources,
        (
            run.flasks,
            run.gold,
            run.hp,
            run.weapon_level,
            run.blessings
        )
    );
}
