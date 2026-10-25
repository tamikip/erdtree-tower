use crate::RuleResult;
use serde::{Deserialize,Serialize};

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Ballot {pub player:u64,pub node:Option<usize>}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Snapshot {pub epoch:u64,pub revision:u64,pub ballots:Vec<Ballot>,pub chosen:Option<usize>}
pub struct Vote {state:Snapshot,available:Vec<usize>,ready:Vec<bool>}
impl Vote {
    pub fn new(epoch:u64,players:&[u64],available:&[usize])->RuleResult<Self> {
        if players.is_empty()||players.len()>crate::party_roster::MAX_PLAYERS||players.iter().enumerate().any(|(i,p)|*p==0||players[..i].contains(p))||available.is_empty(){return Err("无效队伍或路线".into());}
        Ok(Self{state:Snapshot{epoch,revision:0,ballots:players.iter().map(|&player|Ballot{player,node:None}).collect(),chosen:None},available:available.to_vec(),ready:vec![false;players.len()]})
    }
    pub fn snapshot(&self)->&Snapshot {&self.state}
    fn index(&self,epoch:u64,player:u64)->RuleResult<usize> {
        if epoch!=self.state.epoch||self.state.chosen.is_some(){return Err("选路已结束或序号过期".into());}
        self.state.ballots.iter().position(|b|b.player==player).ok_or("未知队员".into())
    }
    pub fn ready(&mut self,epoch:u64,player:u64,ready:bool)->RuleResult<()> {
        let i=self.index(epoch,player)?;
        if self.ready[i]!=ready {
            self.ready[i]=ready;
            if !ready {self.state.ballots[i].node=None;}
            self.state.revision+=1;
        }
        Ok(())
    }
    pub fn cast(&mut self,epoch:u64,player:u64,node:usize)->RuleResult<()> {
        let i=self.index(epoch,player)?;
        if !self.ready[i]||!self.available.contains(&node){return Err("请完成当前节点并选择可达路线".into());}
        if self.state.ballots[i].node!=Some(node){self.state.ballots[i].node=Some(node);self.state.revision+=1;}
        Ok(())
    }
    // Caller supplies uniform entropy. Reject the short remainder to avoid modulo bias.
    pub fn draw(&mut self,entropy:u64)->Option<usize> {
        if self.state.chosen.is_some()||!self.ready.iter().all(|r|*r)||self.state.ballots.iter().any(|b|b.node.is_none()){return None;}
        let count=self.state.ballots.len() as u64;
        if entropy<count.wrapping_neg()%count{return None;}
        let node=self.state.ballots[(entropy%count) as usize].node?;
        self.state.chosen=Some(node);self.state.revision+=1;Some(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn vote()->Vote {Vote::new(7,&[1,2,3],&[4,5,6]).unwrap()}
    #[test]
    fn all_players_must_choose_even_when_everyone_is_ready() {
        let mut v=vote();for p in 1..=3{v.ready(7,p,true).unwrap();}
        v.cast(7,1,4).unwrap();v.cast(7,2,5).unwrap();assert_eq!(v.draw(5),None);
        v.cast(7,3,6).unwrap();assert_eq!(v.draw(5),Some(6));assert_eq!(v.draw(4),None);
        assert!(v.cast(7,1,5).is_err());
    }
    #[test]
    fn votes_are_per_person_not_per_distinct_destination() {
        for (entropy,expected) in [(3,4),(4,4),(5,6)] {
            let mut v=vote();for p in 1..=3{v.ready(7,p,true).unwrap();v.cast(7,p,if p==3{6}else{4}).unwrap();}
            assert_eq!(v.draw(entropy),Some(expected));
        }
    }
    #[test]
    fn disconnect_or_unready_invalidates_vote_and_requires_reselection() {
        let mut v=vote();for p in 1..=3{v.ready(7,p,true).unwrap();v.cast(7,p,4).unwrap();}
        v.ready(7,2,false).unwrap();assert_eq!(v.draw(3),None);
        v.ready(7,2,true).unwrap();assert_eq!(v.draw(3),None);
        v.cast(7,2,6).unwrap();assert_eq!(v.draw(4),Some(6));
    }
    #[test]
    fn stale_unknown_unready_and_unreachable_votes_cannot_change_state() {
        let mut v=vote();let before=v.snapshot().clone();
        for (epoch,player,node) in [(6,1,4),(7,9,4),(7,1,4)]{assert!(v.cast(epoch,player,node).is_err());}
        assert_eq!(*v.snapshot(),before);v.ready(7,1,true).unwrap();assert!(v.cast(7,1,99).is_err());
        v.cast(7,1,4).unwrap();v.cast(7,1,5).unwrap();let revision=v.snapshot().revision;
        v.cast(7,1,5).unwrap();assert_eq!(v.snapshot().revision,revision);
        assert_eq!(v.snapshot().ballots[0].node,Some(5));
    }
}
