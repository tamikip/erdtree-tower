use erdtree_tower::*;
#[test]
fn interrupted_combat_returns_to_same_available_encounter() {
    let mut run=Run::new(42,Class::Samurai);
    run.enter(0).unwrap();run.use_flask().unwrap();run.observe_hp(25).unwrap();
    let mut loaded=Run::decode(&run.encode().unwrap()).unwrap();
    loaded.resume_from_hub().unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded.available(),loaded.start_nodes());
    assert!(loaded.visited.is_empty());
    loaded.enter(0).unwrap();loaded.validate().unwrap();
}
#[test]
fn pending_reward_survives_lobby_without_reroll_and_death_cannot_resume() {
    let mut run=Run::new(42,Class::Samurai);
    run.enter(0).unwrap();run.win_combat().unwrap();
    let original=run.clone();run.resume_from_hub().unwrap();assert_eq!(run,original);
    let mut dead=Run::new(1,Class::Samurai);dead.enter(0).unwrap();dead.observe_hp(0).unwrap();
    assert!(dead.resume_from_hub().is_err());
}
#[test]
fn death_in_lobby_transition_or_reward_is_terminal_and_serializable() {
    let mut run=Run::new(1,Class::Samurai);run.enter(0).unwrap();
    run.end_on_death();let loaded=Run::decode(&run.encode().unwrap()).unwrap();
    assert_eq!(loaded.phase,Phase::Dead);assert_eq!(loaded.hp,0);
}
