use erdtree_tower::{Class, Phase, Run};

#[test]
fn combat_does_not_mint_a_second_currency_when_reward_is_declined() {
    let mut run = Run::new(42, Class::Vagabond);
    assert!(run.decline_reward().is_err());
    run.enter(0).unwrap();
    assert!(run.decline_reward().is_err());
    run.win_combat().unwrap();
    run.decline_reward().unwrap();
    assert_eq!(run.gold, 0);
    assert_eq!(run.visited, [0]);
    assert_eq!(run.phase, Phase::Map);
    assert!(!run.available().is_empty());
    assert!(run.decline_reward().is_err());
    assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(), run);
}
