use erdtree_tower::{Class,Run};

#[test]
fn old_saves_load_with_empty_delivery_history() {
    let run=Run::new(42,Class::Samurai);
    let mut json:serde_json::Value=serde_json::from_str(&run.encode().unwrap()).unwrap();
    json.as_object_mut().unwrap().remove("delivered_talismans");
    assert!(Run::decode(&json.to_string()).unwrap().delivered_talismans.is_empty());
}

#[test]
fn delivered_talismans_persist_but_new_runs_start_empty() {
    let mut run=Run::new(42,Class::Samurai);
    run.delivered_talismans=vec![1000,1150];
    assert_eq!(Run::decode(&run.encode().unwrap()).unwrap().delivered_talismans,vec![1000,1150]);
    assert!(Run::new(43,Class::Samurai).delivered_talismans.is_empty());
    run.delivered_talismans.push(1000);
    assert!(run.validate().is_err());
}
