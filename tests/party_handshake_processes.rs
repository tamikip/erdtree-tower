use erdtree_tower::{party_handshake::{Handshake, Message, Payload}, party_roster::Roster};
use std::{io::{Read, Write}, net::{TcpListener, TcpStream}, process::{Child, Command}, time::Duration};

fn state(local:u64, nonce:u64)->Handshake {
    Handshake::new(Roster{local,host:1,members:vec![1,2]},nonce,42).unwrap()
}
fn send(stream:&mut TcpStream, message:Message) {
    let data=Payload::Handshake(message).encode().unwrap();
    stream.write_all(&(data.len() as u32).to_be_bytes()).unwrap();
    stream.write_all(&data).unwrap();
}
fn receive(stream:&mut TcpStream)->Message {
    let mut size=[0;4];stream.read_exact(&mut size).unwrap();
    let size=u32::from_be_bytes(size) as usize;assert!(size<=256*1024);
    let mut data=vec![0;size];stream.read_exact(&mut data).unwrap();
    match Payload::decode(&data).unwrap() {Payload::Handshake(message)=>message,_=>panic!("unexpected payload")}
}
fn configure(stream:&TcpStream) {
    stream.set_nonblocking(false).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
}
struct ChildGuard(Child);
impl Drop for ChildGuard {fn drop(&mut self) {let _=self.0.kill();let _=self.0.wait();}}

#[test]
fn restarted_guest_completes_wire_handshake_and_rejects_stale_ack() {
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut child=ChildGuard(Command::new(std::env::current_exe().unwrap())
        .args(["--ignored","--exact","handshake_guest_process"])
        .env("TOWER_HANDSHAKE_TEST_ADDR",listener.local_addr().unwrap().to_string()).spawn().unwrap());
    let start=std::time::Instant::now();
    let mut stream=loop {
        match listener.accept() {
            Ok((stream,_))=>break stream,
            Err(error) if error.kind()==std::io::ErrorKind::WouldBlock=>{
                assert!(start.elapsed()<Duration::from_secs(10),"guest connection timed out");
                assert!(child.0.try_wait().unwrap().is_none(),"guest exited before connecting");
                std::thread::sleep(Duration::from_millis(10));
            },
            Err(error)=>panic!("accept failed: {error}"),
        }
    };
    configure(&stream);
    let mut host=state(1,101);
    let hello=receive(&mut stream);
    send(&mut stream,host.receive_lobby(2,hello).unwrap().remove(0).1);
    let old_ack=receive(&mut stream);
    let restarted_hello=receive(&mut stream);
    send(&mut stream,host.receive_lobby(2,restarted_hello).unwrap().remove(0).1);
    assert!(host.receive_lobby(2,old_ack).unwrap().is_empty());
    assert!(host.ready().is_none());
    let new_ack=receive(&mut stream);
    send(&mut stream,host.receive_lobby(2,new_ack).unwrap().remove(0).1);
    assert_eq!(host.ready(),Some((101,42)));
    let reconnect=receive(&mut stream);
    assert!(host.receive_lobby(2,reconnect).unwrap_err().contains("重新连接"));
    while child.0.try_wait().unwrap().is_none() {
        assert!(start.elapsed()<Duration::from_secs(15),"guest exit timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(child.0.wait().unwrap().success());
}

#[test]
#[ignore]
fn handshake_guest_process() {
    let address=std::env::var("TOWER_HANDSHAKE_TEST_ADDR").expect("spawned only by parent test");
    let mut stream=TcpStream::connect(address).unwrap();configure(&stream);
    let mut original=state(2,102);
    send(&mut stream,original.hello().unwrap().1);
    let offer=receive(&mut stream);
    send(&mut stream,original.receive_lobby(1,offer).unwrap().remove(0).1);
    let mut restarted=state(2,202);
    send(&mut stream,restarted.hello().unwrap().1);
    let offer=receive(&mut stream);
    send(&mut stream,restarted.receive_lobby(1,offer).unwrap().remove(0).1);
    let commit=receive(&mut stream);restarted.receive_lobby(1,commit).unwrap();
    assert_eq!(restarted.ready(),Some((101,42)));
    send(&mut stream,state(2,302).hello().unwrap().1);
}
