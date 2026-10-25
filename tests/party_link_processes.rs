use erdtree_tower::{Class, Run, RuleResult, party_link::*, party_rewards::*};
use std::{process::{Command, Child}, time::{Duration, Instant}, thread};

const TOKEN_A: &str = "process-test-player-a-0000000000000000";
const TOKEN_B: &str = "process-test-player-b-0000000000000000";
struct Children(Vec<Child>);
impl Drop for Children {
    fn drop(&mut self) { for child in &mut self.0 { child.kill().ok(); child.wait().ok(); } }
}
#[derive(Default)]
struct InventoryStub(Vec<u64>);
impl PersonalBridge for InventoryStub {
    fn apply_atomic(&mut self, owner: u64, _: &Run, _: &Run) -> RuleResult<()> { self.0.push(owner); Ok(()) }
}
fn request(sequence: u64, action: Choice) -> ClientMessage {
    ClientMessage::Choice(Request { session: 900, sequence, action })
}

#[test]
#[ignore = "child process entry point; invoked by two_process_reward_barrier"]
fn party_child() {
    let Ok(address) = std::env::var("TOWER_TEST_ADDRESS") else { return; };
    let mut link = Client::connect(address.parse().unwrap(),900,std::env::var("TOWER_TEST_TOKEN").unwrap()).unwrap();
    let start = Instant::now();
    let mut owner = 0;
    loop {
        for event in link.poll() {
            match event {
                ClientEvent::Connected(id) => { owner = id; link.send(request(1,Choice::Contract(if id == 10 {0} else {1}))).unwrap(); }
                ClientEvent::Message(HostMessage::Receipt { sequence: 1 }) => link.send(request(2,Choice::SkipGear)).unwrap(),
                ClientEvent::Message(HostMessage::Victory { .. }) => {
                    if owner == 20 { thread::sleep(Duration::from_millis(80)); }
                    let choice = request(3,Choice::Reward(0));
                    link.send(choice.clone()).unwrap();
                    link.send(choice).unwrap();
                }
                ClientEvent::Message(HostMessage::Enter { epoch: 2, .. }) => { link.send(ClientMessage::Ready { epoch: 2 }).unwrap(); }
                ClientEvent::Message(HostMessage::Paused(reason)) if reason == "test complete" => return,
                ClientEvent::Disconnected => panic!("unexpected disconnect"),
                _ => {},
            }
        }
        assert!(start.elapsed() < Duration::from_secs(10));
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn two_process_reward_barrier() {
    let mut host = Host::bind("127.0.0.1:0".parse().unwrap(),900,vec![
        Credential { player: 10, token: TOKEN_A.into() }, Credential { player: 20, token: TOKEN_B.into() },
    ]).unwrap();
    let mut run = Run::new(42,Class::Samurai); run.enable_contracts();
    let mut party = PartyRewards::new(900,10,vec![(10,run.clone()),(20,run)]).unwrap();
    party.set_connected(10,false).unwrap(); party.set_connected(20,false).unwrap();
    let mut children = Children(vec![]);
    for token in [TOKEN_A,TOKEN_B] {
        children.0.push(Command::new(std::env::current_exe().unwrap())
            .args(["--exact","party_child","--ignored","--nocapture"])
            .env("TOWER_TEST_ADDRESS",host.address().unwrap().to_string())
            .env("TOWER_TEST_TOKEN",token).spawn().unwrap());
    }
    let start = Instant::now(); let mut stage = 0;
    let mut inventory = InventoryStub::default(); let mut ready = vec![];
    let mut observed_barrier = false; let mut duplicates = 0;
    loop {
        for event in host.poll() {
            match event {
                HostEvent::Connected(id) => party.set_connected(id,true).unwrap(),
                HostEvent::Disconnected(id) => { party.set_connected(id,false).unwrap(); assert!(stage == 3); }
                HostEvent::Message { player, message: ClientMessage::Choice(req) } => {
                    let seq = req.sequence;
                    if party.apply(player,req,&mut inventory).unwrap() == Applied::Duplicate { duplicates += 1; }
                    host.send(player,HostMessage::Receipt { sequence: seq }).unwrap();
                    if stage == 1 && seq == 3 && !party.route_ready() { observed_barrier = true; }
                }
                HostEvent::Message { player, message: ClientMessage::Ready { epoch: 2 } } => { if !ready.contains(&player) { ready.push(player); } }
                _ => {},
            }
        }
        if stage == 0 && party.route_ready() {
            assert_ne!(party.run(10).unwrap().contract.as_ref().unwrap().chosen,party.run(20).unwrap().contract.as_ref().unwrap().chosen);
            let node = party.run(10).unwrap().available()[0];
            party.enter(10,node).unwrap(); party.victory(10,node).unwrap();
            for id in [10,20] { host.send(id,HostMessage::Victory { epoch: 1, node }).unwrap(); }
            stage = 1;
        } else if stage == 1 && party.route_ready() {
            assert!(observed_barrier);
            let node = party.run(10).unwrap().available()[0]; party.enter(10,node).unwrap();
            for id in [10,20] { host.send(id,HostMessage::Enter { epoch: 2, node }).unwrap(); }
            stage = 2;
        } else if stage == 2 && ready.len() == 2 {
            assert_eq!(duplicates,2);
            assert_eq!(inventory.0.iter().filter(|id| **id == 10).count(),3);
            assert_eq!(inventory.0.iter().filter(|id| **id == 20).count(),3);
            for id in [10,20] { host.send(id,HostMessage::Paused("test complete".into())).unwrap(); }
            stage = 3;
        } else if stage == 3 {
            let statuses: Vec<_> = children.0.iter_mut().map(|p| p.try_wait().unwrap()).collect();
            if statuses.iter().all(Option::is_some) { assert!(statuses.iter().all(|s|s.unwrap().success())); break; }
        }
        assert!(start.elapsed() < Duration::from_secs(15),"process test timed out at stage {stage}");
        thread::sleep(Duration::from_millis(2));
    }
}
