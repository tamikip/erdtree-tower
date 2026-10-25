use erdtree_tower::{Class,Phase,Run,RuleResult};
use erdtree_tower::party_rewards::*;

#[derive(Default)]
struct Bridge { owners:Vec<u64>, fail:bool }
impl PersonalBridge for Bridge {
    fn apply_atomic(&mut self,owner:u64,_before:&Run,_after:&Run)->RuleResult<()> {
        if self.fail{return Err("inventory unavailable".into());}
        self.owners.push(owner);Ok(())
    }
}
fn party(contracts:bool)->PartyRewards {
    let mut a=Run::new(42,Class::Samurai);if contracts{a.enable_contracts();}
    PartyRewards::new(900,10,vec![(10,a.clone()),(20,a)]).unwrap()
}
fn request(sequence:u64,action:Choice)->Request {Request{session:900,sequence,action}}

#[test]
fn one_player_skips_oath_while_another_selects() {
    let mut party=party(true);let mut bridge=Bridge::default();
    let skip=request(1,Choice::SkipContract);
    assert_eq!(party.apply(10,skip.clone(),&mut bridge).unwrap(),Applied::New);
    assert_eq!(party.apply(10,skip,&mut bridge).unwrap(),Applied::Duplicate);
    assert!(!party.route_ready());assert!(party.run(10).unwrap().contract.as_ref().unwrap().chosen.is_none());
    party.apply(20,request(1,Choice::Contract(0)),&mut bridge).unwrap();
    party.apply(20,request(2,Choice::SkipGear),&mut bridge).unwrap();
    assert!(party.route_ready());
    let mut restored=PartyRewards::decode(&party.encode().unwrap()).unwrap();
    for id in [10,20]{restored.set_connected(id,true).unwrap();}
    let node=restored.run(10).unwrap().available()[0];restored.enter(10,node).unwrap();
}

#[test]
fn eight_independent_draws_survive_party_settlement_and_reconnect() {
    let owners=(1..=erdtree_tower::party_roster::MAX_PLAYERS as u64).map(|id|id*10).collect::<Vec<_>>();
    let mut players=owners.iter().map(|&id|(id,Run::new_personal(42,Class::Samurai,id*101/10))).collect::<Vec<_>>();
    for (_,run) in &mut players {run.enable_contracts();}
    let mut party=PartyRewards::new(900,10,players).unwrap();
    let mut bridge=Bridge::default();
    for &owner in &owners {
        let run=party.run(owner).unwrap();
        let mut expected=run.contract.clone().unwrap();
        expected.choose(0,run.reward_seed(),run.class).unwrap();
        party.apply(owner,request(1,Choice::Contract(0)),&mut bridge).unwrap();
        assert_eq!(party.run(owner).unwrap().contract.as_ref(),Some(&expected));
        party.apply(owner,request(2,Choice::SkipGear),&mut bridge).unwrap();
    }
    let node=party.run(10).unwrap().available()[0];
    party.enter(10,node).unwrap();party.victory(10,node).unwrap();
    assert_ne!(party.run(10).unwrap().phase,party.run(20).unwrap().phase);
    let saved=party.encode().unwrap();let mut restored=PartyRewards::decode(&saved).unwrap();
    for &owner in &owners {
        assert_eq!(restored.run(owner),party.run(owner));
        restored.set_connected(owner,true).unwrap();
        restored.apply(owner,request(3,Choice::Reward(0)),&mut bridge).unwrap();
    }
    assert!(restored.route_ready());
    assert_eq!(bridge.owners.len(),owners.len()*3);
}

#[test]
fn each_player_chooses_their_own_contract_and_equipment() {
    let mut party=party(true);let mut bridge=Bridge::default();
    let before=party.run(20).unwrap().clone();
    party.apply(10,request(1,Choice::Contract(0)),&mut bridge).unwrap();
    assert_eq!(party.run(20).unwrap(),&before);
    party.apply(20,request(1,Choice::Contract(1)),&mut bridge).unwrap();
    assert_ne!(party.run(10).unwrap().contract.as_ref().unwrap().chosen,party.run(20).unwrap().contract.as_ref().unwrap().chosen);
    party.apply(10,request(2,Choice::Gear(0)),&mut bridge).unwrap();
    assert!(party.run(10).unwrap().contract.as_ref().unwrap().gear_done);
    assert!(!party.run(20).unwrap().contract.as_ref().unwrap().gear_done);
    assert!(!party.route_ready());
    party.apply(20,request(2,Choice::SkipGear),&mut bridge).unwrap();
    assert!(party.route_ready());assert_eq!(bridge.owners,vec![10,20,10,20]);
}
#[test]
fn one_claim_does_not_consume_teammates_reward_or_allow_early_travel() {
    let mut party=party(false);let mut bridge=Bridge::default();
    let node=party.run(10).unwrap().available()[0];
    assert!(party.enter(20,node).is_err());party.enter(10,node).unwrap();
    assert!(party.victory(20,node).is_err());party.victory(10,node).unwrap();
    let before=party.run(20).unwrap().clone();
    party.apply(10,request(1,Choice::Reward(0)),&mut bridge).unwrap();
    assert_eq!(party.run(20).unwrap(),&before);assert!(matches!(before.phase,Phase::Reward{..}));
    assert!(!party.route_ready());assert!(party.enter(10,node).is_err());
    party.apply(20,request(1,Choice::SkipReward),&mut bridge).unwrap();assert!(party.route_ready());
}
#[test]
fn replay_foreign_session_and_unknown_sender_never_grant_items() {
    let mut party=party(true);let mut bridge=Bridge::default();let command=request(1,Choice::Contract(0));
    assert!(party.apply(30,command.clone(),&mut bridge).is_err());
    assert!(party.apply(10,Request{session:901,..command.clone()},&mut bridge).is_err());
    assert_eq!(party.apply(10,command.clone(),&mut bridge).unwrap(),Applied::New);
    assert_eq!(party.apply(10,command,&mut bridge).unwrap(),Applied::Duplicate);
    assert!(party.apply(10,request(1,Choice::Contract(1)),&mut bridge).is_err());
    assert!(party.apply(10,request(3,Choice::SkipGear),&mut bridge).is_err());
    assert_eq!(bridge.owners,vec![10]);
}
#[test]
fn failed_delivery_can_retry_and_reconnect_preserves_pending_pages() {
    let mut party=party(true);let mut bridge=Bridge::default();
    party.apply(10,request(1,Choice::Contract(0)),&mut bridge).unwrap();
    let before=party.run(10).unwrap().clone();bridge.fail=true;
    assert!(party.apply(10,request(2,Choice::Gear(1)),&mut bridge).is_err());assert_eq!(party.run(10).unwrap(),&before);
    let saved=party.encode().unwrap();let mut restored=PartyRewards::decode(&saved).unwrap();bridge.fail=false;
    assert!(!restored.route_ready());restored.set_connected(10,true).unwrap();
    assert_eq!(restored.apply(10,request(2,Choice::Gear(1)),&mut bridge).unwrap(),Applied::New);
    assert_eq!(restored.apply(10,request(2,Choice::Gear(1)),&mut bridge).unwrap(),Applied::Duplicate);
    assert_eq!(restored.run(20).unwrap(),party.run(20).unwrap());
    assert!(PartyRewards::new(1,10,vec![(10,before.clone()),(10,before)]).is_err());
}

#[test]
fn identical_reward_choices_are_personal_and_disconnect_blocks_host() {
    let mut party=party(false);let mut bridge=Bridge::default();
    let node=party.run(10).unwrap().available()[0];party.enter(10,node).unwrap();party.victory(10,node).unwrap();
    party.apply(10,request(1,Choice::Reward(0)),&mut bridge).unwrap();
    party.apply(20,request(1,Choice::Reward(0)),&mut bridge).unwrap();
    assert_eq!(party.run(10).unwrap().claimed_items,party.run(20).unwrap().claimed_items);
    assert_eq!(bridge.owners,vec![10,20]);assert!(party.route_ready());
    let next=party.run(10).unwrap().available()[0];party.set_connected(20,false).unwrap();
    assert!(!party.route_ready());assert!(party.enter(10,next).is_err());
    let saved=party.encode().unwrap();let mut restored=PartyRewards::decode(&saved).unwrap();
    restored.set_connected(10,true).unwrap();assert!(!restored.route_ready());
    restored.set_connected(20,true).unwrap();restored.enter(10,next).unwrap();
}

#[test]
fn invalid_actions_and_stale_victory_leave_all_players_unchanged() {
    let mut party=party(false);let mut bridge=Bridge::default();
    let before=party.encode().unwrap();
    assert!(party.apply(10,request(1,Choice::Reward(99)),&mut bridge).is_err());
    assert!(party.victory(10,0).is_err());assert_eq!(party.encode().unwrap(),before);assert!(bridge.owners.is_empty());
    let node=party.run(10).unwrap().available()[0];party.enter(10,node).unwrap();party.victory(10,node).unwrap();
    let before=party.encode().unwrap();assert!(party.victory(10,node).is_err());assert_eq!(party.encode().unwrap(),before);
}
