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
#[test]
fn version_mismatch_is_reported_to_client_instead_of_silent_drop() {
    let mut host=host();let mut raw=TcpStream::connect(host.address().unwrap()).unwrap();
    raw.write_all(&frame(serde_json::json!({"Hello":{"version":VERSION-1,"session":900,"token":TOKEN}}))).unwrap();
    let start=Instant::now();let mut reply=vec![];raw.set_read_timeout(Some(Duration::from_millis(5))).unwrap();
    loop {
        assert!(host.poll().is_empty());
        let mut buffer=[0;256];match std::io::Read::read(&mut raw,&mut buffer) {Ok(0)=>break,Ok(n)=>reply.extend_from_slice(&buffer[..n]),Err(_)=>{}}
        assert!(start.elapsed()<Duration::from_secs(2));
    }
    let message:serde_json::Value=serde_json::from_slice(&reply[4..]).unwrap();
    assert_eq!(message["Welcome"]["version"],VERSION);assert_eq!(message["Welcome"]["player"],0);
}
#[test]
fn client_surfaces_host_protocol_version() {
    let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut client=Client::connect(listener.local_addr().unwrap(),900,TOKEN.into()).unwrap();
    let (mut stream,_)=listener.accept().unwrap();
    stream.write_all(&frame(serde_json::json!({"Welcome":{"version":VERSION+1,"session":900,"player":20}}))).unwrap();
    let start=Instant::now();let mut events=vec![];
    while !events.iter().any(|e|matches!(e,ClientEvent::Disconnected)) {events.extend(client.poll());assert!(start.elapsed()<Duration::from_secs(2));thread::sleep(Duration::from_millis(2));}
    assert!(matches!(events.as_slice(),[ClientEvent::Mismatch(v),ClientEvent::Disconnected] if *v==VERSION+1));
}
#[test]
fn silent_unauthenticated_sockets_are_dropped_within_two_seconds() {
    let mut host=host();let mut idle:Vec<_>=(0..16).map(|_|TcpStream::connect(host.address().unwrap()).unwrap()).collect();
    for stream in &idle {stream.set_read_timeout(Some(Duration::from_millis(1))).unwrap();}
    let start=Instant::now();let mut closed=0;
    while closed<idle.len() {
        assert!(host.poll().is_empty());closed=0;
        for stream in &mut idle {if matches!(std::io::Read::read(stream,&mut [0;8]),Ok(0)) {closed+=1;}}
        assert!(start.elapsed()<Duration::from_millis(2600),"idle sockets held past hello timeout");
        thread::sleep(Duration::from_millis(5));
    }
    assert!(start.elapsed()>=Duration::from_millis(1900));
    let mut client=client(&host,TOKEN);
    pump(&mut host,&mut client,|_,ce|ce.iter().any(|e|matches!(e,ClientEvent::Connected(20))));
}
#[test]
fn backlog_coalesces_snapshots_keeps_ordered_items_and_retries_when_full() {
    let mut backlog=Backlog::new(3);
    backlog.latest(2,"r2a");assert!(backlog.push("o1"));backlog.latest(3,"r3");backlog.latest(2,"r2b");
    assert!(backlog.push("o2"));assert!(backlog.push("o3"));assert!(!backlog.push("o4"));assert_eq!(backlog.len(),5);
    let mut sent=vec![];backlog.flush(|item|if sent.len()<2 {sent.push(item);Ok(())}else{Err(item)});
    assert_eq!(sent,["o1","o2"]);assert_eq!(backlog.len(),3);
    let mut rest=vec![];backlog.flush(|item|{rest.push(item);Ok(())});
    assert_eq!(rest,["o3","r2b","r3"]);assert_eq!(backlog.len(),0);
}
