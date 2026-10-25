use erdtree_tower::party_link::*;
use erdtree_tower::party_rewards::{Choice,Request};
use std::{net::{TcpStream,Shutdown},io::Write,time::{Instant,Duration},thread};

fn pump(host:&mut Host,client:&mut Client,until:impl Fn(&[HostEvent],&[ClientEvent])->bool) {
    let start=Instant::now();let mut he=vec![];let mut ce=vec![];
    loop {
        he.extend(host.poll());ce.extend(client.poll());
        if until(&he,&ce){return;}
        assert!(start.elapsed()<Duration::from_secs(3),"link timed out");
        thread::sleep(Duration::from_millis(2));
    }
}
fn host()->Host {Host::bind("127.0.0.1:0".parse().unwrap(),900,vec![Credential{player:20,token:"test-credential-20-0000000000000000".into()}]).unwrap()}
fn client(host:&Host,token:&str)->Client {Client::connect(host.address().unwrap(),900,token.into()).unwrap()}
const TOKEN:&str="test-credential-20-0000000000000000";
#[test]
fn real_tcp_assigns_identity_and_delivers_typed_messages() {
    let mut host=host();let mut client=client(&host,TOKEN);
    pump(&mut host,&mut client,|he,ce|he.iter().any(|e|matches!(e,HostEvent::Connected(20)))&&ce.iter().any(|e|matches!(e,ClientEvent::Connected(20))));
    let request=Request{session:900,sequence:1,action:Choice::Contract(0)};
    client.send(ClientMessage::Choice(request.clone())).unwrap();
    pump(&mut host,&mut client,|he,_|he.iter().any(|e|matches!(e,HostEvent::Message{player:20,message:ClientMessage::Choice(r)} if r==&request)));
    host.send(20,HostMessage::Paused("waiting for teammate".into())).unwrap();
    pump(&mut host,&mut client,|_,ce|ce.iter().any(|e|matches!(e,ClientEvent::Message(HostMessage::Paused(_)))));
}
#[test]
fn wrong_session_wrong_token_and_duplicate_connection_are_rejected() {
    let mut host=host();let mut bad=client(&host,"wrong-token-000000000000000000000");
    pump(&mut host,&mut bad,|he,ce|{assert!(!he.iter().any(|e|matches!(e,HostEvent::Connected(_))));ce.iter().any(|e|matches!(e,ClientEvent::Disconnected))});
    let mut bad=Client::connect(host.address().unwrap(),901,TOKEN.into()).unwrap();
    pump(&mut host,&mut bad,|he,ce|{assert!(!he.iter().any(|e|matches!(e,HostEvent::Connected(_))));ce.iter().any(|e|matches!(e,ClientEvent::Disconnected))});
    let mut first=client(&host,TOKEN);pump(&mut host,&mut first,|_,ce|ce.iter().any(|e|matches!(e,ClientEvent::Connected(20))));
    let mut duplicate=client(&host,TOKEN);pump(&mut host,&mut duplicate,|he,ce|{assert!(!he.iter().any(|e|matches!(e,HostEvent::Disconnected(20))));ce.iter().any(|e|matches!(e,ClientEvent::Disconnected))});
    first.send(ClientMessage::Ready{epoch:1}).unwrap();
    pump(&mut host,&mut first,|he,_|he.iter().any(|e|matches!(e,HostEvent::Message{player:20,..})));
}
#[test]
fn malformed_length_closes_socket_and_reconnect_keeps_identity() {
    let mut host=host();let mut raw=TcpStream::connect(host.address().unwrap()).unwrap();
    raw.write_all(&u32::MAX.to_be_bytes()).unwrap();
    for _ in 0..4 {assert!(host.poll().is_empty());thread::sleep(Duration::from_millis(2));}
    raw.shutdown(Shutdown::Both).ok();
    let mut first=client(&host,TOKEN);pump(&mut host,&mut first,|_,ce|ce.iter().any(|e|matches!(e,ClientEvent::Connected(20))));
    drop(first);
    let start=Instant::now();while !host.poll().iter().any(|e|matches!(e,HostEvent::Disconnected(20))) {assert!(start.elapsed()<Duration::from_secs(2));thread::sleep(Duration::from_millis(2));}
    let mut again=client(&host,TOKEN);pump(&mut host,&mut again,|_,ce|ce.iter().any(|e|matches!(e,ClientEvent::Connected(20))));
}

fn frame(value:serde_json::Value)->Vec<u8> {
    let bytes=serde_json::to_vec(&value).unwrap();let mut result=(bytes.len() as u32).to_be_bytes().to_vec();result.extend(bytes);result
}
#[test]
fn fragmented_handshake_and_coalesced_requests_keep_order() {
    let mut host=host();let mut raw=TcpStream::connect(host.address().unwrap()).unwrap();
    let hello=frame(serde_json::json!({"Hello":{"version":VERSION,"session":900,"token":TOKEN}}));
    for byte in &hello[..hello.len()-1] {raw.write_all(&[*byte]).unwrap();assert!(host.poll().is_empty());}
    raw.write_all(&hello[hello.len()-1..]).unwrap();
    let start=Instant::now();loop {if host.poll().iter().any(|e|matches!(e,HostEvent::Connected(20))){break;}assert!(start.elapsed()<Duration::from_secs(2));thread::sleep(Duration::from_millis(2));}
    let mut bytes=frame(serde_json::json!({"Client":{"Ready":{"epoch":1}}}));
    bytes.extend(frame(serde_json::json!({"Client":{"Ready":{"epoch":2}}})));
    raw.write_all(&bytes).unwrap();let mut epochs=vec![];
    let start=Instant::now();while epochs.len()<2 {
        for e in host.poll(){if let HostEvent::Message{player:20,message:ClientMessage::Ready{epoch}}=e {epochs.push(epoch);}}
        assert!(start.elapsed()<Duration::from_secs(2));thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(epochs,vec![1,2]);
    raw.write_all(&frame(serde_json::json!({"Host":{"Victory":{"epoch":2,"node":0}}}))).unwrap();
    let start=Instant::now();loop {
        let events=host.poll();assert!(!events.iter().any(|e|matches!(e,HostEvent::Message{..})));
        if events.iter().any(|e|matches!(e,HostEvent::Disconnected(20))){break;}
        assert!(start.elapsed()<Duration::from_secs(2));thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn bounded_output_rejects_oversized_messages_without_losing_connection() {
    let mut host=host();let mut client=client(&host,TOKEN);
    pump(&mut host,&mut client,|_,ce|ce.iter().any(|e|matches!(e,ClientEvent::Connected(20))));
    assert!(host.send(20,HostMessage::Paused("x".repeat(300_000))).is_err());
    host.send(20,HostMessage::Paused("still connected".into())).unwrap();
    pump(&mut host,&mut client,|_,ce|ce.iter().any(|e|matches!(e,ClientEvent::Message(HostMessage::Paused(s)) if s=="still connected")));
}
