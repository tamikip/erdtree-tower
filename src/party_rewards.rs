use crate::{Phase,RuleResult,Run};
use serde::{Deserialize,Serialize};

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum Choice {Contract(usize),Gear(usize),SkipGear,Reward(usize),SkipReward,SkipContract}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {pub session:u64,pub sequence:u64,pub action:Choice}
#[derive(Debug,PartialEq,Eq)]
pub enum Applied {New,Duplicate}

pub trait PersonalBridge {
    // Apply only to the authenticated owner's character; rollback effects on error.
    // Gear completion and reward claims must not succeed before inventory delivery.
    fn apply_atomic(&mut self,owner:u64,before:&Run,after:&Run)->RuleResult<()>;
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Player {
    id:u64,run:Run,last:Option<Request>,
    #[serde(skip)] connected:bool,
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyRewards {session:u64,host:u64,players:Vec<Player>}
impl PartyRewards {
    pub fn new(session:u64,host:u64,players:Vec<(u64,Run)>)->RuleResult<Self> {
        let party=Self {session,host,players:players.into_iter().map(|(id,run)|Player{id,run,last:None,connected:true}).collect()};
        party.validate()?;Ok(party)
    }
    pub fn run(&self,owner:u64)->Option<&Run> {self.players.iter().find(|p|p.id==owner).map(|p|&p.run)}
    // Called by the verified session adapter, never from a client-supplied owner field.
    pub fn set_connected(&mut self,owner:u64,connected:bool)->RuleResult<()> {
        self.players.iter_mut().find(|p|p.id==owner).ok_or("未知队员")?.connected=connected;Ok(())
    }
    pub fn route_ready(&self)->bool {
        let Some(first)=self.players.first() else{return false;};
        self.players.iter().all(|p|p.connected&&p.run.phase==Phase::Map&&!p.run.contract_pending()
            &&p.run.act==first.run.act&&p.run.visited==first.run.visited)
    }
    pub fn enter(&mut self,sender:u64,node:usize)->RuleResult<()> {
        self.authorize_host(sender)?;
        if !self.route_ready(){return Err("等待所有队员完成奖励与契约选择".into());}
        let mut next=self.clone();
        for player in &mut next.players {player.run.enter(node)?;}
        *self=next;Ok(())
    }
    // The host calls this only after its authoritative encounter confirms victory.
    pub fn victory(&mut self,sender:u64,node:usize)->RuleResult<()> {
        self.authorize_host(sender)?;
        let mut next=self.clone();
        for player in &mut next.players {
            if player.run.phase!=(Phase::Combat{id:node}) {return Err("战斗结算与当前节点不符".into());}
            player.run.win_combat()?;
        }
        *self=next;Ok(())
    }
    pub fn apply(&mut self,sender:u64,request:Request,bridge:&mut impl PersonalBridge)->RuleResult<Applied> {
        if request.session!=self.session {return Err("请求不属于当前队伍旅程".into());}
        let player=self.players.iter_mut().find(|p|p.id==sender&&p.connected).ok_or("队员未连接")?;
        if player.last.as_ref()==Some(&request) {return Ok(Applied::Duplicate);}
        let expected=player.last.as_ref().map_or(Some(1),|r|r.sequence.checked_add(1)).ok_or("请求序号已耗尽")?;
        if request.sequence!=expected {return Err("请求重复、过期或乱序".into());}
        let mut next=player.run.clone();
        match request.action {
            Choice::Reward(index)=>next.claim_reward(index)?,
            Choice::SkipReward=>next.decline_reward()?,
            Choice::Contract(index)=>{
                if next.phase!=Phase::Map{return Err("当前不能选择契约".into());}
                let seed=next.reward_seed();
                next.contract.as_mut().ok_or("契约未开启")?.choose(index,seed,next.class)?;
            },
            Choice::SkipContract=>{
                if next.phase!=Phase::Map{return Err("当前不能跳过契约".into());}
                next.contract.as_mut().ok_or("契约未开启")?.skip()?;
            },
            Choice::Gear(index)=>{
                if next.phase!=Phase::Map{return Err("当前不能领取适配装备".into());}
                let contract=next.contract.as_mut().ok_or("契约未开启")?;
                contract.begin_gift(index)?;
                contract.gift.as_mut().unwrap().delivered=[true;2];
                contract.finish_gift()?;
            },
            Choice::SkipGear=>{
                if next.phase!=Phase::Map{return Err("当前不能跳过适配装备".into());}
                next.contract.as_mut().ok_or("契约未开启")?.skip_gift()?;
            },
        }
        next.validate()?;
        bridge.apply_atomic(sender,&player.run,&next)?;
        player.run=next;player.last=Some(request);Ok(Applied::New)
    }
    pub fn encode(&self)->RuleResult<String> {self.validate()?;serde_json::to_string(self).map_err(|e|e.to_string())}
    pub fn decode(text:&str)->RuleResult<Self> {
        if text.len()>2*1024*1024{return Err("队伍存档过大".into());}
        let party:Self=serde_json::from_str(text).map_err(|e|e.to_string())?;
        party.validate()?;Ok(party)
    }
    fn authorize_host(&self,sender:u64)->RuleResult<()> {
        if sender!=self.host||!self.players.iter().any(|p|p.id==sender&&p.connected){return Err("只有在线房主能推进路线或结算战斗".into());}Ok(())
    }
    fn validate(&self)->RuleResult<()> {
        if self.session==0||self.players.is_empty()||self.players.len()>crate::party_roster::MAX_PLAYERS||!self.players.iter().any(|p|p.id==self.host){return Err("无效队伍".into());}
        let first=&self.players[0].run;
        for (index,player) in self.players.iter().enumerate() {
            if player.id==0||self.players[..index].iter().any(|p|p.id==player.id)
                ||player.run.seed!=first.seed||player.run.nodes!=first.nodes
                ||player.last.as_ref().is_some_and(|r|r.session!=self.session||r.sequence==0){return Err("队员身份或旅程不一致".into());}
            player.run.validate()?;
        }
        Ok(())
    }
}
