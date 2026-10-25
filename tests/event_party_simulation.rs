use erdtree_tower::{Run,Class,party_game::{Gate,Report,Order,Stage},party_events::Kind};

#[test]
fn cancelled_scarabs_wait_for_all_refund_receipts_before_retry() {
    for count in 2..=8 {
        let run=Run::new(42,Class::Samurai);
        let mut gate=Gate::new((1..=count).collect());gate.stage=Stage::Service;gate.begin_events().unwrap();
        let mut reports=vec![Report::new(&run,0,true,false,true);count as usize];
        reports[count as usize-1].event_request=Some(Kind::Scarabs(1));
        for (i,r) in reports.iter().enumerate(){gate.report(i as u64+1,r.clone()).unwrap();}
        let Some(Order::EventStart(job))=gate.event_start().unwrap() else{panic!()};
        assert_eq!(job.player,count);
        assert!(matches!(gate.event_abort(job.wave).unwrap(),Order::EventAborted{..}));
        assert!(gate.event_abort(job.wave).is_err());assert!(gate.event_done(job.wave,7).is_err());
        for i in (0..count as usize).rev() {
            assert!(gate.event_start().unwrap().is_none());
            reports[i].event_request=None;reports[i].event_applied=Some(job.wave);
            gate.report(i as u64+1,reports[i].clone()).unwrap();
        }
        assert!(gate.event_start().unwrap().is_none());
        reports[count as usize-1].event_request=Some(Kind::Scarabs(1));
        gate.report(count,reports[count as usize-1].clone()).unwrap();
        let Some(Order::EventStart(retry))=gate.event_start().unwrap() else{panic!()};
        assert_eq!(retry.player,count);assert_eq!(retry.wave,job.wave+1);
        assert!(gate.event_done(job.wave,7).is_err());
    }
}

#[test]
fn simultaneous_requests_wait_for_every_replica_and_every_receipt() {
    for count in 2..=8 {
        let run=Run::new(42,Class::Samurai);
        let mut gate=Gate::new((1..=count).collect());gate.stage=Stage::Service;gate.begin_events().unwrap();
        let mut reports=vec![Report::new(&run,0,true,false,true);count as usize];
        let mut completed=std::collections::HashSet::new();
        for kind in [Kind::Guards(0),Kind::Scarabs(1),Kind::Guards(1),Kind::Scarabs(2),Kind::Guards(2),Kind::Scarabs(3)] {
            for (i,r) in reports.iter_mut().enumerate(){
                r.event_request=Some(kind);
                gate.report(i as u64+1,r.clone()).unwrap();gate.report(i as u64+1,r.clone()).unwrap();
            }
            for owner in 1..=count {
                let Some(Order::EventStart(job))=gate.event_start().unwrap() else {panic!("missing queued request")};
                assert_eq!(job.player,owner);assert_eq!(job.kind,kind);
                assert!(completed.insert((owner,job.wave)));
                assert!(gate.event_start().unwrap().is_none());
                assert!(gate.event_done(job.wave,0).is_err());
                for i in (0..count as usize).rev() {
                    assert!(!gate.event_bound(job.wave));
                    reports[i].event_bound=Some(job.wave);
                    gate.report(i as u64+1,reports[i].clone()).unwrap();
                }
                assert!(gate.event_bound(job.wave));
                let kills=if matches!(kind,Kind::Scarabs(_)){5}else{0};
                assert!(matches!(gate.event_done(job.wave,kills).unwrap(),Order::EventDone{..}));
                assert!(gate.event_done(job.wave,kills).is_err());
                assert!(gate.event_done(job.wave.wrapping_sub(1),kills).is_err());
                for i in 0..count as usize {
                    assert!(gate.event_start().unwrap().is_none());
                    reports[i].event_applied=Some(job.wave);
                    gate.report(i as u64+1,reports[i].clone()).unwrap();
                }
            }
            assert!(gate.event_start().unwrap().is_none());
        }
        assert_eq!(completed.len(),count as usize*6);
    }
}

#[test]
fn interrupted_events_never_start_another_wave_or_accept_late_rewards() {
    for count in 2..=8 {for interruption in 0..3 {
        let run=Run::new(42,Class::Samurai);
        let mut gate=Gate::new((1..=count).collect());gate.stage=Stage::Service;gate.begin_events().unwrap();
        let mut report=Report::new(&run,0,true,false,true);report.event_request=Some(Kind::Guards(0));
        for id in 1..=count {gate.report(id,report.clone()).unwrap();}
        let Some(Order::EventStart(job))=gate.event_start().unwrap() else{panic!()};
        report.event_bound=Some(job.wave);
        for id in 1..=count {gate.report(id,report.clone()).unwrap();}
        match interruption {
            0=>{report.rest_end=true;gate.report(count,report.clone()).unwrap();assert!(gate.rest_end().is_some());},
            1=>{report.alive=false;for id in 1..=count {gate.report(id,report.clone()).unwrap();}assert!(gate.defeat().is_some());},
            _=>{gate.disconnect(count);assert!(!gate.event_bound(job.wave));assert!(gate.event_done(job.wave,0).is_err());gate.stop();},
        }
        assert!(gate.event_done(job.wave,0).is_err());assert!(gate.event_start().unwrap().is_none());
        assert!(!gate.event_bound(job.wave));assert!(gate.spawn().is_none());
    }}
}
