use erdtree_tower::{Class, NodeKind, Phase, Run};

#[test]
fn physical_chest_finishes_once_without_granting_another_core_reward() {
    let mut run = Run::new(42, Class::Samurai);
    assert!(run.finish_chest().is_err());
    let id = run
        .nodes
        .iter()
        .position(|n| n.kind == NodeKind::Chest)
        .unwrap();
    run.phase = Phase::Node { id };
    let before = (
        run.gold,
        run.flasks,
        run.weapon_level,
        run.blessings.clone(),
    );
    run.finish_chest().unwrap();
    assert_eq!(run.phase, Phase::Map);
    assert_eq!(run.visited.last(), Some(&id));
    assert_eq!(
        before,
        (
            run.gold,
            run.flasks,
            run.weapon_level,
            run.blessings.clone()
        )
    );
    assert!(run.finish_chest().is_err());
}
