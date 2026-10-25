use erdtree_tower::map_drawing::*;
fn scope()->Scope {Scope{seed:42,act:1}}
fn line()->Edit {Edit::Add(vec![Point{x:100,y:200},Point{x:9000,y:8000}])}
#[test]
fn guest_edits_are_acknowledged_once_and_erase_is_shared() {
    let mut host=Sync::default();let mut guest=Sync::default();
    host.set_scope(Some(scope()));guest.set_scope(Some(scope()));
    assert!(guest.submit(2,line(),false));
    assert_eq!(guest.preview(2).unwrap().strokes.len(),1);
    assert!(guest.canvas.as_ref().unwrap().strokes.is_empty());
    let payload=erdtree_tower::party_handshake::Payload::Report{session:1,report:{
        let mut r=erdtree_tower::party_game::Report::new(&erdtree_tower::Run::new(42,erdtree_tower::Class::Samurai),0,false,false,true);
        r.drawing=guest.request();r
    }};
    let erdtree_tower::party_handshake::Payload::Report{report,..}=erdtree_tower::party_handshake::Payload::decode(&payload.encode().unwrap()).unwrap() else{panic!("wrong payload");};
    let request=report.drawing.unwrap();
    assert!(host.apply(2,&request));assert!(!host.apply(2,&request));
    assert!(guest.receive(host.canvas.clone().unwrap(),2));
    assert!(guest.request().is_none());assert_eq!(host.canvas,guest.canvas);
    let id=guest.canvas.as_ref().unwrap().strokes[0].id;
    assert_eq!(id.player,2);
    assert!(guest.submit(2,Edit::Erase(vec![id]),false));
    assert!(guest.preview(2).unwrap().strokes.is_empty());
    assert!(host.apply(2,&guest.request().unwrap()));
    guest.receive(host.canvas.clone().unwrap(),2);
    assert!(guest.canvas.unwrap().strokes.is_empty());
}
#[test]
fn scope_reset_and_out_of_order_snapshots_cannot_restore_old_ink() {
    let mut host=Sync::default();host.set_scope(Some(scope()));host.submit(1,line(),true);
    let older=host.canvas.clone().unwrap();
    host.submit(1,Edit::Erase(vec![older.strokes[0].id]),true);
    let mut guest=Sync::default();guest.set_scope(Some(scope()));
    assert!(guest.receive(host.canvas.clone().unwrap(),2));assert!(!guest.receive(older.clone(),2));
    guest.set_scope(Some(Scope{seed:42,act:2}));
    assert!(!guest.receive(older,2));assert!(guest.canvas.unwrap().strokes.is_empty());
}
#[test]
fn bounded_requests_and_maximum_snapshot_fit_the_transport() {
    let mut host=Sync::default();host.set_scope(Some(scope()));
    assert!(!host.submit(1,Edit::Add(vec![Point{x:10001,y:0};2]),true));
    assert!(!host.submit(1,Edit::Add(vec![Point{x:0,y:0};MAX_POINTS+1]),true));
    for _ in 0..MAX_STROKES {assert!(host.submit(1,Edit::Add(vec![Point{x:10000,y:10000};MAX_POINTS]),true));}
    let payload=erdtree_tower::party_handshake::Payload::Order{session:1,order:erdtree_tower::party_game::Order::MapDrawing(host.canvas.clone().unwrap())};
    assert!(payload.encode().unwrap().len()<256*1024);
    let mut guest=Sync::default();guest.set_scope(Some(scope()));
    for _ in 0..MAX_PENDING {assert!(guest.submit(2,line(),false));}
    assert!(!guest.submit(2,line(),false));
    assert!(host.apply(2,&guest.request().unwrap()));
    guest.receive(host.canvas.clone().unwrap(),2);
    assert!(guest.request().is_some()); // Full canvas still acknowledges the rejected edit.
}
#[test]
fn eraser_hits_the_middle_of_segments() {
    assert!(near_segment([50.0,4.0],[0.0,0.0],[100.0,0.0],6.0));
    assert!(!near_segment([50.0,8.0],[0.0,0.0],[100.0,0.0],6.0));
    assert!(near_segment([1.0,1.0],[0.0,0.0],[0.0,0.0],2.0));
}
#[test]
fn invalid_snapshot_and_mismatched_report_scope_cannot_change_shared_ink() {
    let run=erdtree_tower::Run::new(42,erdtree_tower::Class::Samurai);
    let mut guest=Sync::default();guest.set_scope(Some(scope()));guest.submit(2,line(),false);
    let mut gate=erdtree_tower::party_game::Gate::new(vec![1,2]);
    let mut report=erdtree_tower::party_game::Report::new(&run,0,false,false,true);
    report.drawing=guest.request();report.drawing.as_mut().unwrap().scope.act=2;
    gate.report(2,report).unwrap();assert!(gate.drawing_requests().is_empty());
    let mut host=Sync::default();host.set_scope(Some(scope()));host.submit(1,line(),true);
    let mut bad=host.canvas.unwrap();bad.strokes.push(bad.strokes[0].clone());
    assert!(!guest.receive(bad,2));assert!(guest.request().is_some());assert!(guest.canvas.unwrap().strokes.is_empty());
}
