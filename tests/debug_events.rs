use erdtree_tower::{Run,Class,Phase,events::Draw};
#[test]
fn every_event_can_be_forced_and_survives_reload() {
    for draw in [Draw::Trial,Draw::Chests,Draw::Altar,Draw::Locked,Draw::Scarabs,Draw::Spring,Draw::Dice,Draw::Mystery,Draw::Reforge,Draw::Mausoleum,Draw::Furnace] {
        let mut run=Run::new(42,Class::Samurai);let gold=run.gold;let seed=run.seed;
        let node=run.debug_event_test(draw).unwrap();
        assert!(run.debug);assert_eq!(run.gold,gold);assert_eq!(run.seed,seed);
        assert_eq!(run.event_draw(node),draw);
        run=Run::decode(&run.encode().unwrap()).unwrap();
        run.enter(node).unwrap();assert_eq!(run.event_draw(node),draw);
        assert_eq!(run.phase,Phase::Node{id:node});
        assert!(Run::decode(&run.encode().unwrap()).is_ok());
        let before=run.clone();assert!(run.debug_event_test(Draw::Spring).is_err());assert_eq!(run,before);
    }
}
#[test]
fn event_override_cannot_be_used_in_a_regular_run_or_override_mausoleum_test() {
    let mut run=Run::new(42,Class::Samurai);run.debug_event_test(Draw::Dice).unwrap();
    run.debug_mausoleum_test().unwrap();assert!(run.event_test.is_none());
    let node=run.debug_event_test(Draw::Trial).unwrap();assert!(run.mausoleum_test.is_none());
    run.debug=false;assert!(Run::decode(&run.encode().unwrap()).is_err());
    run.debug=true;assert_eq!(run.event_draw(node),Draw::Trial);
}
