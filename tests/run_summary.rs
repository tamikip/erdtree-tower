use erdtree_tower::{Class,Run};

#[test]
fn seeded_runs_reproduce_routes_without_sharing_personal_rewards() {
    let a=Run::new_personal(u64::MAX,Class::Samurai,10);
    let b=Run::new_personal(u64::MAX,Class::Samurai,20);
    assert_eq!(a.nodes,b.nodes);
    assert_eq!(a.initial_seed,Some(u64::MAX));
    assert_ne!(a.reward_seed(),b.reward_seed());
    assert_eq!(Run::decode(&a.encode().unwrap()).unwrap(),a);
}

#[test]
fn summary_counts_completed_nodes_and_retains_seed_after_death() {
    let mut run=Run::new(42,Class::Samurai);
    let id=run.available()[0];run.enter(id).unwrap();
    assert_eq!(run.reached_layer,1);
    run.win_combat().unwrap();run.claim_reward(0).unwrap();
    assert_eq!((run.completed_nodes,run.completed_battles),(1,1));
    run.end_on_death();
    let loaded=Run::decode(&run.encode().unwrap()).unwrap();
    assert_eq!(loaded.initial_seed,Some(42));
    assert_eq!((loaded.completed_nodes,loaded.completed_battles),(1,1));
}

#[test]
fn older_saves_load_without_inventing_original_seed_or_statistics() {
    let run=Run::new(42,Class::Samurai);
    let mut json=serde_json::to_value(run).unwrap();
    for key in ["initial_seed","completed_nodes","completed_battles","reached_layer"] {json.as_object_mut().unwrap().remove(key);}
    let loaded=Run::decode(&json.to_string()).unwrap();
    assert_eq!(loaded.initial_seed,None);
    assert_eq!(loaded.completed_nodes,0);
}
