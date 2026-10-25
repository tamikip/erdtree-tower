use erdtree_tower::{Run,Class,Phase,NodeKind,regional_events::Trial,party_game::{Gate,Report,Stage,Order}};

fn trial()->Run {
    let mut r=Run::new(26,Class::Samurai);
    loop {
        let id=r.available()[0];r.enter(id).unwrap();
        match r.nodes[id].kind {
            NodeKind::Event=>break,
            NodeKind::Monster|NodeKind::Elite|NodeKind::Boss=>{r.win_combat().unwrap();r.decline_reward().unwrap();},
            NodeKind::Camp=>r.leave_camp().unwrap(),NodeKind::Shop=>r.leave_shop().unwrap(),NodeKind::Chest=>r.finish_chest().unwrap(),
        }
    }
    r.regional_event=Default::default();r.regional_event.trial=true;r
}
#[test]
fn four_choices_persist_and_cannot_repeat() {
    for choice in [Trial::Descent,Trial::Stormveil,Trial::Chariot,Trial::Skip] {
        let mut r=trial();let gold=r.gold;
        assert!(r.leave_regional_event().is_err());
        assert!(r.open_regional_event(0).is_err());
        r.choose_trial(choice).unwrap();
        assert!(r.choose_trial(choice).is_err());
        assert_eq!(r.gold,gold+if choice==Trial::Skip{800}else{0});
        assert_eq!(Run::decode(&r.encode().unwrap()).unwrap(),r);
        match choice {
            Trial::Descent=>assert!(r.regional_event.descent_started),
            Trial::Stormveil|Trial::Chariot=>assert_eq!(r.regional_event.stormveil,Some(erdtree_tower::stormveil::Progress::Running)),
            Trial::Skip=>{r.leave_regional_event().unwrap();assert_eq!(r.phase,Phase::Map);}
        }
    }
}
#[test]
fn two_to_eight_votes_wait_for_everyone_and_commit_once() {
    for count in 2..=8 {
        let r=trial();let mut g=Gate::new((1..=count).collect());g.stage=Stage::Service;
        for id in 1..=count {
            let mut report=Report::new(&r,0,true,false,true);
            report.trial_choice=Some(if id==count{Trial::Skip}else{Trial::Chariot});
            g.report(id,report).unwrap();
            if id<count {assert!(g.trial_order().is_none());}
        }
        let Some(Order::Trial{choice,..})=g.trial_order() else{panic!("missing result")};
        if count>2 {assert_eq!(choice,Trial::Chariot);}
        assert!(g.trial_order().is_none());
    }
}
#[test]
fn missing_or_unready_members_block_and_ties_ignore_report_order() {
    let r=trial();let mut results=Vec::new();
    for ids in [vec![1,2],vec![2,1]] {
        let mut g=Gate::new(ids);g.stage=Stage::Service;
        for id in 1..=2 {
            let mut report=Report::new(&r,0,true,false,true);
            report.trial_choice=Some(if id==1{Trial::Descent}else{Trial::Skip});
            g.report(id,report).unwrap();
        }
        g.disconnect(2);assert!(g.trial_order().is_none());
        let mut report=Report::new(&r,0,false,false,true);report.trial_choice=Some(Trial::Skip);
        g.report(2,report.clone()).unwrap();assert!(g.trial_order().is_none());
        report.arrived=true;report.alive=false;
        g.report(2,report.clone()).unwrap();assert!(g.trial_order().is_none());
        report.alive=true;g.report(2,report).unwrap();
        let Some(Order::Trial{choice,..})=g.trial_order() else{panic!()};results.push(choice);
    }
    assert_eq!(results[0],results[1]);
}
#[test]
fn selected_chariot_retains_arrival_barrier_and_completion_order() {
    use erdtree_tower::stormveil::Decision;
    for choice in [Trial::Stormveil,Trial::Chariot] {
        let mut r=trial();let mut g=Gate::new(vec![1,2]);g.stage=Stage::Service;
        let mut report=Report::new(&r,0,true,false,true);report.trial_choice=Some(choice);
        for id in 1..=2 {g.report(id,report.clone()).unwrap();}
        assert!(g.trial_order().is_some());r.choose_trial(choice).unwrap();
        let mut report=Report::new(&r,0,true,false,true);
        for id in 1..=2 {g.report(id,report.clone()).unwrap();}
        assert!(g.stormveil_order().is_none());
        if choice==Trial::Chariot {
            report.chariot_ready=true;
            g.report(1,report.clone()).unwrap();assert!(g.stormveil_order().is_none());
            g.report(2,report.clone()).unwrap();
            assert!(matches!(g.stormveil_order(),Some(Order::Stormveil{decision:Decision::Engage,..})));
        }
        report.storm_finish=true;
        for id in 1..=2 {g.report(id,report.clone()).unwrap();}
        assert!(matches!(g.stormveil_order(),Some(Order::Stormveil{decision:Decision::Complete,..})));
    }
}
