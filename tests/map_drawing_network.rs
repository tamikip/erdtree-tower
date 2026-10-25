use erdtree_tower::{Class,Run,map_drawing::{self,Edit,Point,Scope},party_game::{Gate,Report,Order},party_link::*};
use std::{thread,time::{Duration,Instant}};
#[test]
fn tcp_round_trip_delivers_guest_ink_and_shared_erasure_without_route_votes() {
    let token="drawing-credential-0000000000000000";
    let mut host=Host::bind("127.0.0.1:0".parse().unwrap(),91,vec![Credential{player:2,token:token.into()}]).unwrap();
    let mut guest=Client::connect(host.address().unwrap(),91,token.into()).unwrap();
    let start=Instant::now();
    loop {
        host.poll();if guest.poll().iter().any(|e|matches!(e,ClientEvent::Connected(2))){break;}
        assert!(start.elapsed()<Duration::from_secs(3));thread::sleep(Duration::from_millis(2));
    }
    let run=Run::new(42,Class::Samurai);let mut gate=Gate::new(vec![1,2]);
    gate.report(1,Report::new(&run,0,false,false,true)).unwrap();
    let mut authority=map_drawing::Sync::default();let mut replica=map_drawing::Sync::default();
    authority.set_scope(Some(Scope::of(&run)));replica.set_scope(Some(Scope::of(&run)));
    for erase in [false,true] {
        let edit=if erase {Edit::Erase(vec![replica.canvas.as_ref().unwrap().strokes[0].id])}
            else{Edit::Add(vec![Point{x:100,y:200},Point{x:300,y:400}])};
        assert!(replica.submit(2,edit,false));
        let mut report=Report::new(&run,0,false,false,true);report.drawing=replica.request();
        guest.send(ClientMessage::Game(report)).unwrap();
        let start=Instant::now();let mut delivered=false;
        while !delivered {
            guest.poll();
            for event in host.poll(){if let HostEvent::Message{player,message:ClientMessage::Game(report)}=event {
                gate.report(player,report).unwrap();
                for (player,request) in gate.drawing_requests(){delivered|=authority.apply(player,&request);}
            }}
            assert!(start.elapsed()<Duration::from_secs(3));thread::sleep(Duration::from_millis(2));
        }
        assert!(gate.ballots().iter().all(|b|b.node.is_none()));
        host.send(2,HostMessage::Game(Order::MapDrawing(authority.canvas.clone().unwrap()))).unwrap();
        let start=Instant::now();let mut acknowledged=false;
        while !acknowledged {
            host.poll();
            for event in guest.poll(){if let ClientEvent::Message(HostMessage::Game(Order::MapDrawing(canvas)))=event {
                acknowledged=replica.receive(canvas,2);
            }}
            assert!(start.elapsed()<Duration::from_secs(3));thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(authority.canvas,replica.canvas);assert!(replica.request().is_none());
    }
    assert!(replica.canvas.unwrap().strokes.is_empty());
}
