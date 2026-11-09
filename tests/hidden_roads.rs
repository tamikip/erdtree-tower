use erdtree_tower::{Run,Class,NodeKind,Phase,regional_events::{Trial,Progress}};
#[test]
fn retired_hidden_path_saves_migrate_to_the_remaining_route() {
    let choice:Trial=serde_json::from_str("\"HiddenPath\"").unwrap();
    assert_eq!(choice,Trial::HereticalRise);
    assert_eq!(serde_json::to_string(&choice).unwrap(),"\"HereticalRise\"");
}
#[test]
fn hidden_routes_save_and_complete_without_changing_existing_trials() {
    for choice in [Trial::HereticalRise] {
        let mut run=Run::new(42,Class::Samurai);
        let id=run.nodes.iter().position(|n|n.kind==NodeKind::Event).unwrap();
        run.phase=Phase::Node{id};run.regional_event=Progress{trial:true,..Default::default()};
        run.choose_trial(choice).unwrap();
        assert_eq!(run.regional_event.hidden_road,Some(choice));
        let mut loaded:Run=serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
        loaded.stormveil_decide(erdtree_tower::stormveil::Decision::Complete).unwrap();
        assert!(loaded.stormveil_decide(erdtree_tower::stormveil::Decision::Complete).is_err());
        loaded.leave_regional_event().unwrap();
        assert!(loaded.regional_event.hidden_road.is_none());
    }
}
