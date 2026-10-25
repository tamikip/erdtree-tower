use erdtree_tower::{Run,Class,party_game::{Gate,Report,Order},party_link::*};
use std::{time::{Duration,Instant},thread};

#[test]
fn legacy_host_only_route_protocol_cannot_join_voting_session() {
    use std::{io::Write,net::TcpStream};
    let token="vote-test-credential-0000000000000000";
    let mut host=Host::bind("127.0.0.1:0".parse().unwrap(),91,vec![Credential{player:2,token:token.into()}]).unwrap();
    let mut raw=TcpStream::connect(host.address().unwrap()).unwrap();
    let message=serde_json::to_vec(&serde_json::json!({"Hello":{"version":3,"session":91,"token":token}})).unwrap();
    raw.write_all(&(message.len() as u32).to_be_bytes()).unwrap();raw.write_all(&message).unwrap();
    for _ in 0..20 {assert!(!host.poll().iter().any(|e|matches!(e,HostEvent::Connected(_))));thread::sleep(Duration::from_millis(2));}
    let mut guest=Client::connect(host.address().unwrap(),91,token.into()).unwrap();let start=Instant::now();
    loop {host.poll();if guest.poll().iter().any(|e|matches!(e,ClientEvent::Connected(2))){break;}
        assert!(start.elapsed()<Duration::from_secs(3));thread::sleep(Duration::from_millis(2));}
}

#[test]
fn remote_ballot_reaches_host_before_shared_travel_and_stale_reports_are_rejected() {
    let token="vote-test-credential-0000000000000000";
    let mut host=Host::bind("127.0.0.1:0".parse().unwrap(),91,vec![Credential{player:2,token:token.into()}]).unwrap();
    let mut guest=Client::connect(host.address().unwrap(),91,token.into()).unwrap();
    let start=Instant::now();let mut connected=false;
    while !connected {
        host.poll();connected=guest.poll().iter().any(|e|matches!(e,ClientEvent::Connected(2)));
        assert!(start.elapsed()<Duration::from_secs(3));thread::sleep(Duration::from_millis(2));
    }
    let run=Run::new(42,Class::Samurai);let nodes=run.available();let mut gate=Gate::new(vec![1,2]);
    let mut local=Report::new(&run,0,false,false,true);local.selected=Some(nodes[0]);gate.report(1,local).unwrap();
    assert!(gate.resolve_votes(&run,1).unwrap().is_none());
    let mut remote=Report::new(&run,0,false,false,true);remote.selected=Some(*nodes.last().unwrap());
    guest.send(ClientMessage::Game(remote.clone())).unwrap();
    let mut delivered=false;
    while !delivered {
        guest.poll();
        for event in host.poll(){if let HostEvent::Message{player,message:ClientMessage::Game(report)}=event {
            assert_eq!(player,2);gate.report(player,report).unwrap();delivered=true;
        }}
        assert!(start.elapsed()<Duration::from_secs(3));thread::sleep(Duration::from_millis(2));
    }
    let ballots=gate.ballots();
    host.send(2,HostMessage::Game(Order::Votes{epoch:0,ballots:ballots.clone()})).unwrap();
    let order=gate.resolve_votes(&run,1).unwrap().unwrap();
    assert!(matches!(order,Order::Enter{epoch:1,node,..} if node==*nodes.last().unwrap()));
    host.send(2,HostMessage::Game(order)).unwrap();
    let mut seen=vec![];
    while seen.len()<2 {
        host.poll();
        for event in guest.poll(){if let ClientEvent::Message(HostMessage::Game(order))=event {seen.push(order);}}
        assert!(start.elapsed()<Duration::from_secs(3));thread::sleep(Duration::from_millis(2));
    }
    assert!(matches!(&seen[0],Order::Votes{epoch:0,ballots:b} if b==&ballots));
    assert!(matches!(seen[1],Order::Enter{epoch:1,node,..} if node==*nodes.last().unwrap()));
    assert!(gate.report(2,remote).is_err());
    assert!(gate.resolve_votes(&run,0).unwrap().is_none());
}
