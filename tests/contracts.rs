use erdtree_tower::{Class,contracts::{State,Kind,reinforcement_level}};

#[test]
fn skipping_oath_unlocks_routes_persists_and_grants_nothing() {
    let mut run=erdtree_tower::Run::new(42,Class::Samurai);run.enable_contracts();
    let before=run.clone();
    let state=run.contract.as_mut().unwrap();
    state.skip().unwrap();
    assert!(!state.pending());assert!(!state.selection_pending());assert!(state.chosen.is_none());
    assert_eq!(state.gear,[0;3]);assert!(state.gift.is_none());
    assert!(state.skip().is_err());assert!(state.choose(0,42,Class::Samurai).is_err());
    assert!(state.begin_gift(0).is_err());assert!(state.skip_gift().is_err());
    assert_eq!(run.gold,before.gold);
    assert!(!run.available().is_empty());
    assert_eq!(erdtree_tower::Run::decode(&run.encode().unwrap()).unwrap(),run);
    assert!(State::new(42,Class::Samurai,2,None,0).selection_pending());
    let mut chosen=State::new(42,Class::Samurai,1,None,0);
    chosen.choose(0,42,Class::Samurai).unwrap();assert!(chosen.skip().is_err());
}

#[test]
fn expanded_oaths_keep_existing_ids_and_all_enter_the_offer_pool() {
    for (index,kind) in [Kind::Steel,Kind::Breaker,Kind::Combo,Kind::Guard,Kind::Mystic,Kind::Hunter].into_iter().enumerate() {assert_eq!(kind.index(),index);}
    let mut seen=std::collections::HashSet::new();
    for seed in 0..256 {for kind in State::new(seed,Class::Samurai,1,None,0).offers {seen.insert(kind.index());}}
    assert_eq!(seen.len(),10);
    assert!(Kind::Ascetic.bane().contains("35%"));
}

#[test]
fn two_distinct_contracts_persist_and_do_not_repeat_previous_act() {
    for class in Class::ALL { for seed in 0..128 {
        let mut state=State::new(seed,class,1,None,0);
        assert_ne!(state.offers[0],state.offers[1]);
        assert!(state.choose(2,seed,class).is_err());
        state.choose(0,seed,class).unwrap();
        assert!(state.choose(1,seed,class).is_err());
        let restored:State=serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(state,restored);
        let next=State::new(seed,class,2,state.chosen,24);
        assert!(!next.offers.contains(&state.chosen.unwrap()));
        assert!(next.chosen.is_none());
        assert_eq!(next.peak,24);
    }}
}

#[test]
fn gift_is_once_only_and_delivery_locks_choice() {
    let mut state=State::new(42,Class::Samurai,1,None,0);
    assert!(state.begin_gift(0).is_err());
    state.choose(0,42,Class::Samurai).unwrap();
    state.begin_gift(0).unwrap();
    assert!(state.begin_gift(1).is_err());
    assert!(state.skip_gift().is_err());
    state.gift.as_mut().unwrap().delivered=[true,true];
    state.finish_gift().unwrap();
    assert!(!state.pending());
    assert!(state.begin_gift(0).is_err());
}

#[test]
fn catch_up_never_exceeds_recorded_progress() {
    for peak in 0..=50 {for cap in [10,25] {
        let level=reinforcement_level(peak,cap);
        assert!(level<=cap);
        assert!(u16::from(level)*50/u16::from(cap)<=u16::from(peak));
    }}
    assert_eq!(reinforcement_level(10,25),5);
    assert_eq!(reinforcement_level(10,10),2);
    assert_eq!(reinforcement_level(9,10),1);
}

#[test]
fn every_contract_has_three_valid_distinct_weapons() {
    for kind in Kind::ALL {for class in Class::ALL {
        let mut s=State::new(9,class,1,None,0);s.offers[0]=kind;
        s.choose(0,9,class).unwrap();
        assert_eq!(s.gear.len(),3);
        for (i,id) in s.gear.iter().enumerate() {
            assert!(!s.gear[..i].contains(id));
            assert!(erdtree_tower::rewards::lookup(*id).is_some());
        }
    }}
}

#[test]
fn contract_pool_is_white_or_blue_varied_and_keeps_matching_ammunition() {
    let tiers:std::collections::HashMap<u32,u8>=include_str!("../scene-assets/reward-rarities.tsv").lines().map(|l| {
        let (id,tier)=l.split_once('\t').unwrap();(id.parse().unwrap(),tier.parse().unwrap())
    }).collect();
    for kind in Kind::ALL {
        let mut seen=std::collections::HashSet::new();
        for seed in 0..128 {
            let mut s=State::new(seed,Class::Prophet,1,None,0);s.offers[0]=kind;
            s.offers[1]=*Kind::ALL.iter().find(|k|**k!=kind).unwrap();
            s.choose(0,seed,Class::Prophet).unwrap();
            assert!(s.valid(1));
            for id in s.gear {assert!(tiers[&id]<=1);seen.insert(id);}
            if kind==Kind::Guard {assert!(matches!(s.gear[0]/1_000_000,31|32));}
            if kind==Kind::Mystic {assert!(s.gear.iter().all(|id|matches!(id/1_000_000,33|34)));}
            if kind==Kind::Hunter {assert!(s.gear.iter().all(|id|erdtree_tower::contracts::hunter_ammo(*id).is_some()));}
            assert_eq!(s,serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap());
        }
        assert!(seen.len()>5,"Pool still too small: {kind:?}");
    }
    for (id,ammo) in [(40000000,50000000),(41000000,50000000),(42000000,51000000),(43020000,52000000),(44000000,53000000)] {
        assert_eq!(erdtree_tower::contracts::hunter_ammo(id),Some(ammo));
    }
}

#[test]
fn act_changes_replace_contract_and_selection_blocks_travel() {
    use erdtree_tower::{Run,Phase,NodeKind};
    for mut run in [Run::new(26,Class::Samurai),Run::new_personal(26,Class::Samurai,101)] {
    run.enable_contracts();
    let mut previous=None;
    for act in 1..=3 {
        assert!(run.contract_pending());assert!(run.available().is_empty());assert!(run.enter(0).is_err());
        let seed=run.reward_seed();
        let state=run.contract.as_mut().unwrap();
        assert_eq!(state.act,act);assert!(!previous.is_some_and(|k|state.offers.contains(&k)));
        state.choose(0,seed,run.class).unwrap();previous=state.chosen;state.peak=24;
        assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
        assert!(run.available().is_empty());
        run.contract.as_mut().unwrap().skip_gift().unwrap();
        assert!(!run.available().is_empty());
        for _ in 0..run.floors() {
            let id=run.available()[0];run.enter(id).unwrap();
            match run.nodes[id].kind {
                NodeKind::Monster|NodeKind::Elite|NodeKind::Boss=>{run.win_combat().unwrap();run.decline_reward().unwrap();},
                NodeKind::Camp=>run.leave_camp().unwrap(),NodeKind::Shop=>run.leave_shop().unwrap(),
                NodeKind::Chest=>run.finish_chest().unwrap(),NodeKind::Event=>{run.resolve_event(false).unwrap();},
            }
            assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
        }
        assert_eq!(run.contract.as_ref().unwrap().peak,24);
    }
    assert_eq!(run.phase,Phase::Won);
    }
    let mut legacy=serde_json::to_value(Run::new(9,Class::Samurai)).unwrap();
    legacy.as_object_mut().unwrap().remove("contract");
    assert!(Run::decode(&legacy.to_string()).unwrap().contract.is_none());
}
