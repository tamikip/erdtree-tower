use erdtree_tower::{Class,NodeKind,Phase,Run};

#[test]
fn bosses_restore_resources_only_between_acts_and_keep_progression() {
    let mut run=Run::new(26,Class::Samurai);
    run.weapon_level=7;
    run.claimed_items.push(erdtree_tower::rewards::CATALOG.iter().find(|i|i.kind==erdtree_tower::rewards::ItemKind::Weapon).unwrap().id);
    for act in 1..=3 {
        assert_eq!(run.act,act);
        let old_seed=run.seed;
        while run.visited.len()+1<run.floors() {
            let id=run.available()[0];run.enter(id).unwrap();
            match run.nodes[id].kind {
                NodeKind::Monster|NodeKind::Elite=>{run.win_combat().unwrap();run.decline_reward().unwrap();},
                NodeKind::Camp=>run.leave_camp().unwrap(),
                NodeKind::Shop=>run.leave_shop().unwrap(),
                NodeKind::Chest=>run.finish_chest().unwrap(),
                NodeKind::Event=>{run.resolve_event(false).unwrap();},
                _=>unreachable!(),
            }
        }
        run.enter(run.available()[0]).unwrap();run.win_combat().unwrap();
        run.hp=27;run.flasks=1;
        let gold=run.gold;let loot=run.claimed_items.clone();
        if act==2 {run.claim_reward(0).unwrap();} else {run.decline_reward().unwrap();}
        assert_eq!(run.gold,gold);
        assert_eq!(run.initial_seed,Some(26));
        assert_eq!(run.completed_nodes,u32::from(act)*run.floors() as u32);
        assert!(run.completed_battles>=u32::from(act));
        assert_eq!(run.weapon_level,7);assert!(run.claimed_items.starts_with(&loot));
        if act<3 {
            assert_eq!((run.hp,run.flasks),(run.max_hp,run.flask_limit));
            assert_eq!(run.phase,Phase::Map);assert!(run.visited.is_empty());
            assert_eq!(run.available(),run.start_nodes());assert_ne!(run.seed,old_seed);
        } else {assert_eq!(run.phase,Phase::Won);assert_eq!((run.hp,run.flasks),(27,1));}
        assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
    }
}

#[test]
fn existing_active_saves_start_in_act_one_and_invalid_acts_are_rejected() {
    let run=Run::new(1,Class::Astrologer);
    let mut value=serde_json::to_value(&run).unwrap();
    value.as_object_mut().unwrap().remove("act");
    assert_eq!(Run::decode(&value.to_string()).unwrap(),run);
    for act in [0,4,255] {
        value["act"]=serde_json::json!(act);
        assert!(Run::decode(&value.to_string()).is_err());
    }
}
