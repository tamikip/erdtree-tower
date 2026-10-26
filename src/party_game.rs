use serde::{Deserialize,Serialize};
use crate::{Run,Phase,RuleResult};
pub fn act_seed(initial:u64,act:u8)->RuleResult<u64> {
    if !(1..=3).contains(&act){return Err("幕数无效".into());}
    Ok((2..=act).fold(initial,|seed,act|crate::mix(seed^u64::from(act))))
}

#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub session:u64, pub seed:u64, pub player:u64, pub host:u64,
    pub address:std::net::SocketAddr, pub token:String, pub peers:Vec<Peer>,
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Peer {pub player:u64,pub token:String}
impl Config {
    pub fn decode(text:&str)->RuleResult<Self> {
        if text.len()>8192{return Err("队伍配置过大".into());}
        let c:Self=serde_json::from_str(text).map_err(|_|"队伍配置格式无效")?;
        if c.session==0||c.player==0||c.host==0||c.address.port()==0||c.peers.len()>=crate::party_roster::MAX_PLAYERS
            ||c.token.len()<32||c.token.len()>256 {return Err("队伍配置无效".into());}
        if c.player==c.host {
            if c.peers.is_empty(){return Err("房主必须配置队员".into());}
            for (i,p) in c.peers.iter().enumerate(){
                if p.player==0||p.player==c.host||p.token.len()<32||p.token.len()>256
                    ||c.peers[..i].iter().any(|q|q.player==p.player||q.token==p.token){return Err("队员凭据无效或重复".into());}
            }
        }else if !c.peers.is_empty(){return Err("队友配置不能包含其他队员凭据".into());}
        Ok(c)
    }
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Report {
    #[serde(default)] pub drawing:Option<crate::map_drawing::Request>,
    #[serde(default)] pub trial:bool,
    #[serde(default)] pub trial_choice:Option<crate::regional_events::Trial>,
    #[serde(default)] pub has_stonesword_key:bool,
    #[serde(default)] pub rest_end:bool,
    pub present:bool,
    pub at_hub:bool,
    pub event_request:Option<crate::party_events::Kind>,
    pub event_bound:Option<u8>,
    pub event_applied:Option<u8>,
    #[serde(default)]
    pub selected:Option<usize>,
    #[serde(default)] pub stormveil:Option<crate::stormveil::Progress>,
    #[serde(default)] pub storm_choice:Option<bool>,
    #[serde(default)] pub storm_finish:bool,
    #[serde(default)] pub chariot_ready:bool,
    #[serde(default)] pub chariot:bool,
    pub epoch:u64,pub seed:u64,pub act:u8,pub visited:Vec<usize>,pub map_ready:bool,
    pub arrived:bool,pub bound:bool,pub alive:bool,
}
impl Report {
    pub fn new(run:&Run,epoch:u64,arrived:bool,bound:bool,alive:bool)->Self {
        Self{drawing:None,trial:run.regional_event.trial,trial_choice:None,has_stonesword_key:false,chariot_ready:false,rest_end:false,present:true,at_hub:false,event_request:None,event_bound:None,event_applied:None,selected:None,chariot:run.regional_event.chariot,stormveil:run.regional_event.stormveil,storm_choice:None,storm_finish:false,epoch,seed:run.seed,act:run.act,visited:run.visited.clone(),map_ready:run.phase==Phase::Map&&!run.contract_pending(),arrived,bound,alive}
    }
    pub fn unavailable(epoch:u64)->Self {
        Self{drawing:None,trial:false,trial_choice:None,has_stonesword_key:false,chariot_ready:false,rest_end:false,present:false,at_hub:false,event_request:None,event_bound:None,event_applied:None,selected:None,chariot:false,stormveil:None,storm_choice:None,storm_finish:false,epoch,seed:0,act:1,visited:vec![],map_ready:false,arrived:false,bound:false,alive:false}
    }
    pub fn at_hub(epoch:u64)->Self {Self{present:true,at_hub:true,alive:true,..Self::unavailable(epoch)}}
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub enum Order {
    MapDrawing(crate::map_drawing::Canvas),
    ForceEnd,
    ChariotPose{epoch:u64,sequence:u64,pose:[f32;3]},
    Trial{epoch:u64,choice:crate::regional_events::Trial},
    Stormveil{epoch:u64,decision:crate::stormveil::Decision},
    NewRun{epoch:u64,seed:u64},
    Votes{epoch:u64,ballots:Vec<crate::party_vote::Ballot>},
    Enter{epoch:u64,act:u8,visited:Vec<usize>,node:usize,has_stonesword_key:bool},
    EventStart(crate::party_events::Job),
    EventHunt{epoch:u64,wave:u8},
    EventAborted{epoch:u64,wave:u8},
    EventDone{epoch:u64,wave:u8,killed:u8},
    Spawn{epoch:u64},Engage{epoch:u64},Victory{epoch:u64},Rewards{epoch:u64},Defeat{epoch:u64},Stop(String),
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Stage {Map,Travel,Combat,Fighting,Service,Reward,Stopped}
pub struct Gate {trial:Option<crate::regional_events::Trial>,pub epoch:u64,pub stage:Stage,peers:Vec<(u64,Option<Report>)>,events:Option<crate::party_events::Queue>,stormveil:Option<crate::stormveil::Decision>}
impl Gate {
    pub fn drawing_requests(&self)->Vec<(u64,crate::map_drawing::Request)> {
        self.peers.iter().filter_map(|(id,r)|r.as_ref().filter(|r|r.present&&r.alive)
            .and_then(|r|r.drawing.as_ref().filter(|d|d.scope.seed==r.seed&&d.scope.act==r.act)).map(|d|(*id,d.clone()))).collect()
    }
    pub fn rest_end(&mut self)->Option<Order> {
        if self.stage==Stage::Stopped||!self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.epoch==self.epoch&&r.rest_end)){return None;}
        self.stage=Stage::Stopped;Some(Order::Defeat{epoch:self.epoch})
    }
    pub fn ballots(&self)->Vec<crate::party_vote::Ballot> {
        self.peers.iter().map(|(player,report)|crate::party_vote::Ballot{player:*player,node:report.as_ref().filter(|r|r.present&&r.map_ready&&r.alive).and_then(|r|r.selected)}).collect()
    }
    pub fn new(ids:Vec<u64>)->Self {Self{trial:None,epoch:0,stage:Stage::Map,peers:ids.into_iter().map(|id|(id,None)).collect(),events:None,stormveil:None}}
    pub fn trial_order(&mut self)->Option<Order> {
        use crate::regional_events::Trial;
        if self.stage!=Stage::Service||self.trial.is_some()||!self.all(|r|r.trial&&r.arrived&&r.alive&&r.trial_choice.is_some()){return None;}
        let first=self.peers.first()?.1.as_ref()?;
        if !self.all(|r|r.seed==first.seed&&r.act==first.act&&r.visited==first.visited){return None;}
        let options=[Trial::Descent,Trial::Stormveil,Trial::Chariot,Trial::Skip];
        let counts=options.map(|c|self.peers.iter().filter(|(_,r)|r.as_ref().is_some_and(|r|r.trial_choice==Some(c))).count());
        let best=*counts.iter().max()?;
        let tied:Vec<_>=options.into_iter().zip(counts).filter_map(|(c,n)|(n==best).then_some(c)).collect();
        let choice=tied[crate::mix(first.seed^self.epoch^0x747269616c) as usize%tied.len()];
        self.trial=Some(choice);
        if matches!(choice,Trial::Stormveil|Trial::Chariot){self.stormveil=Some(crate::stormveil::Decision::Challenge);}
        Some(Order::Trial{epoch:self.epoch,choice})
    }
    pub fn stormveil_order(&mut self)->Option<Order> {
        use crate::stormveil::{Progress,Decision};
        if self.stage!=Stage::Service{return None;}
        let chariot=self.peers.first()?.1.as_ref()?.chariot;
        if !self.all(|r|r.chariot==chariot){return None;}
        let decision=if self.stormveil.is_none()&&self.all(|r|r.arrived&&r.alive&&r.stormveil==Some(Progress::Choosing)) {
            if self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.storm_choice==Some(false))) {Decision::Skip}
            else if self.all(|r|r.storm_choice==Some(true)){Decision::Challenge}else{return None;}
        }else if chariot&&self.stormveil==Some(Decision::Challenge)&&self.all(|r|r.alive&&r.chariot_ready&&r.stormveil==Some(Progress::Running)){Decision::Engage}
        else if self.stormveil==Some(if chariot{Decision::Engage}else{Decision::Challenge})&&self.all(|r|r.stormveil==Some(Progress::Running)&&(!r.alive||r.storm_finish))
            &&self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.alive&&r.storm_finish)){Decision::Complete}
        else{return None;};
        self.stormveil=Some(decision);Some(Order::Stormveil{epoch:self.epoch,decision})
    }
    pub fn restart(&mut self,seed:u64)->RuleResult<Option<Order>> {
        if self.epoch==0||!matches!(self.stage,Stage::Stopped|Stage::Reward)||!self.all(|r|r.at_hub&&r.alive){return Ok(None);}
        if seed==0{return Err("新局种子无效".into());}
        self.epoch=self.epoch.checked_add(1).ok_or("队伍序号耗尽")?;
        self.stage=Stage::Map;self.events=None;self.stormveil=None;self.trial=None;for p in &mut self.peers {p.1=None;}
        Ok(Some(Order::NewRun{epoch:self.epoch,seed}))
    }
    pub fn begin_events(&mut self)->RuleResult<()> {
        if self.stage!=Stage::Service||self.events.is_some(){return Err("事件房尚未进入或已经初始化".into());}
        self.events=Some(crate::party_events::Queue::new(self.epoch,self.peers.iter().map(|p|p.0).collect())?);Ok(())
    }
    pub fn event_start(&mut self)->RuleResult<Option<Order>> {
        if self.stage!=Stage::Service||!self.all(|r|r.arrived&&r.alive){return Ok(None);}
        if self.events.as_ref().and_then(|q|q.completed()).is_some_and(|j|!self.all(|r|r.event_applied==Some(j.wave))){return Ok(None);}
        let Some(events)=&mut self.events else{return Ok(None);};
        for (player,report) in &self.peers {
            if let Some(kind)=report.as_ref().and_then(|r|r.event_request){let _=events.request(self.epoch,*player,kind);}
        }
        Ok(events.start().map(Order::EventStart))
    }
    pub fn event_done(&mut self,wave:u8,killed:u8)->RuleResult<Order> {
        let job=self.events.as_ref().and_then(|q|q.active()).ok_or("没有活动事件波次")?;
        if self.stage!=Stage::Service||wave!=job.wave||killed>7||!self.all(|r|r.event_bound==Some(wave))
            ||(matches!(job.kind,crate::party_events::Kind::Guards(_))&&killed!=0){return Err("事件波次尚未同步或结算无效".into());}
        self.events.as_mut().unwrap().finish(self.epoch,wave)?;
        Ok(Order::EventDone{epoch:self.epoch,wave,killed})
    }
    pub fn event_bound(&self,wave:u8)->bool {
        self.stage==Stage::Service&&self.events.as_ref().and_then(|q|q.active()).is_some_and(|j|j.wave==wave)
            &&self.all(|r|r.event_bound==Some(wave))
    }
    pub fn event_abort(&mut self,wave:u8)->RuleResult<Order> {
        let job=self.events.as_ref().and_then(|q|q.active()).ok_or("没有活动事件")?;
        if self.stage!=Stage::Service||job.wave!=wave||!matches!(job.kind,crate::party_events::Kind::Scarabs(_)){return Err("不能取消此事件".into());}
        self.events.as_mut().unwrap().cancel(self.epoch,wave)?;
        Ok(Order::EventAborted{epoch:self.epoch,wave})
    }
    fn events_unsettled(&self)->bool {
        self.events.as_ref().is_some_and(|q|q.busy()||q.completed().is_some_and(|j|!self.all(|r|r.event_applied==Some(j.wave))))
    }
    pub fn disconnect(&mut self,id:u64){if let Some(p)=self.peers.iter_mut().find(|p|p.0==id){p.1=None;}}
    pub fn report(&mut self,id:u64,report:Report)->RuleResult<()> {
        if report.visited.len()>64||report.act==0||report.act>3||report.epoch!=self.epoch{return Err("队员旅程进度不匹配".into());}
        if report.event_request.is_some_and(|k|!k.valid()){return Err("队员事件请求无效".into());}
        self.peers.iter_mut().find(|p|p.0==id).ok_or("未知队员")?.1=Some(report);Ok(())
    }
    fn all(&self,f:impl Fn(&Report)->bool)->bool {
        !self.peers.is_empty()&&self.peers.iter().all(|p|p.1.as_ref().is_some_and(|r|r.present&&r.epoch==self.epoch&&f(r)))
    }
    pub fn enter(&mut self,run:&Run,node:usize)->RuleResult<Order> {
        if self.events_unsettled()||!matches!(self.stage,Stage::Map|Stage::Reward|Stage::Service)||!run.available().contains(&node)
            ||!self.all(|r|r.alive&&r.map_ready&&r.seed==run.seed&&r.act==run.act&&r.visited==run.visited){return Err("等待全员完成奖励与契约选择".into());}
        let has_stonesword_key=self.peers.iter().any(|(_,r)|r.as_ref().is_some_and(|r|r.has_stonesword_key));
        self.epoch=self.epoch.checked_add(1).ok_or("队伍序号耗尽")?;
        self.stage=if run.nodes[node].kind.combat(){Stage::Travel}else{Stage::Service};
        self.events=None;self.stormveil=None;self.trial=None;
        for p in &mut self.peers {p.1=None;}
        Ok(Order::Enter{epoch:self.epoch,act:run.act,visited:run.visited.clone(),node,has_stonesword_key})
    }
    pub fn resolve_votes(&mut self,run:&Run,entropy:u64)->RuleResult<Option<Order>> {
        if self.events_unsettled()||!matches!(self.stage,Stage::Map|Stage::Reward|Stage::Service)
            ||!self.all(|r|r.alive&&r.map_ready&&r.seed==run.seed&&r.act==run.act&&r.visited==run.visited){return Ok(None);}
        let ids:Vec<_>=self.peers.iter().map(|p|p.0).collect();let available=run.available();
        let mut vote=crate::party_vote::Vote::new(self.epoch,&ids,&available)?;
        for (id,report) in &self.peers {
            let Some(node)=report.as_ref().and_then(|r|r.selected).filter(|n|available.contains(n)) else{return Ok(None);};
            vote.ready(self.epoch,*id,true)?;vote.cast(self.epoch,*id,node)?;
        }
        vote.draw(entropy).map(|node|self.enter(run,node)).transpose()
    }
    pub fn spawn(&mut self)->Option<Order> {
        if self.stage==Stage::Travel&&self.all(|r|r.arrived&&r.alive){self.stage=Stage::Combat;Some(Order::Spawn{epoch:self.epoch})}else{None}
    }
    pub fn victory(&mut self)->RuleResult<Order> {
        if self.stage!=Stage::Fighting||!self.all(|r|r.bound){return Err("等待队友确认敌人复制".into());}
        self.stage=Stage::Reward;Ok(Order::Victory{epoch:self.epoch})
    }
    pub fn defeat(&mut self)->Option<Order> {
        if matches!(self.stage,Stage::Travel|Stage::Combat|Stage::Fighting|Stage::Service)&&self.all(|r|!r.alive){self.stage=Stage::Stopped;Some(Order::Defeat{epoch:self.epoch})}else{None}
    }
    pub fn engage(&mut self)->Option<Order> {
        if self.stage==Stage::Combat&&self.all(|r|r.bound) {self.stage=Stage::Fighting;Some(Order::Engage{epoch:self.epoch})}else{None}
    }
    pub fn stop(&mut self){self.stage=Stage::Stopped;}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn key_eligibility_is_committed_from_any_current_member() {
        for count in 2..=8 {for holder in 0..=count {
            let run=Run::new(42,crate::Class::Samurai);let node=run.available()[0];
            let mut gate=Gate::new((1..=count).collect());
            for id in 1..=count {
                let mut report=Report::new(&run,0,false,false,true);
                report.has_stonesword_key=id==holder;gate.report(id,report).unwrap();
            }
            let order=gate.enter(&run,node).unwrap();
            assert!(matches!(order,Order::Enter{has_stonesword_key,..} if has_stonesword_key==(holder!=0)));
        }}
    }
    #[test]
    fn any_member_rest_ends_two_to_eight_players_once_in_every_active_stage() {
        for count in 2..=8 {for member in 1..=count {for stage in [Stage::Map,Stage::Travel,Stage::Combat,Stage::Fighting,Stage::Service,Stage::Reward] {
            let mut gate=Gate::new((1..=count).collect());gate.stage=stage;
            let mut report=Report::unavailable(0);report.rest_end=true;
            assert!(gate.report(count+1,report.clone()).is_err());
            let mut stale=report.clone();stale.epoch=1;assert!(gate.report(member,stale).is_err());
            assert!(gate.rest_end().is_none());
            gate.report(member,report).unwrap();
            assert!(matches!(gate.rest_end(),Some(Order::Defeat{epoch:0})));
            assert!(gate.rest_end().is_none());assert_eq!(gate.stage,Stage::Stopped);
        }}}
    }
    #[test]
    fn chariot_vote_requires_same_trial_for_two_to_eight_players() {
        use crate::stormveil::{Progress,Decision};
        for count in 2..=8 {
            let mut run=Run::new(42,crate::Class::Samurai);
            run.regional_event.stormveil=Some(crate::stormveil::Progress::Choosing);
            run.regional_event.chariot=true;
            let mut gate=Gate::new((1..=count).collect());gate.stage=Stage::Service;
            let mut report=Report::new(&run,0,true,false,true);report.storm_choice=Some(true);
            for id in 1..count {gate.report(id,report.clone()).unwrap();}
            let mut wrong=report.clone();wrong.chariot=false;
            gate.report(count,wrong).unwrap();assert!(gate.stormveil_order().is_none());
            gate.report(count,report.clone()).unwrap();
            assert!(matches!(gate.stormveil_order(),Some(Order::Stormveil{decision:crate::stormveil::Decision::Challenge,..})));
            assert!(gate.stormveil_order().is_none());
            report.stormveil=Some(Progress::Running);report.chariot_ready=true;
            for id in 1..count {gate.report(id,report.clone()).unwrap();}
            assert!(gate.stormveil_order().is_none());
            gate.report(count,report.clone()).unwrap();
            assert!(matches!(gate.stormveil_order(),Some(Order::Stormveil{decision:Decision::Engage,..})));
            assert!(gate.stormveil_order().is_none());
            report.storm_finish=true;
            for id in 1..count {gate.report(id,report.clone()).unwrap();}
            assert!(gate.stormveil_order().is_none());
            gate.report(count,report).unwrap();
            assert!(matches!(gate.stormveil_order(),Some(Order::Stormveil{decision:Decision::Complete,..})));
            assert!(gate.stormveil_order().is_none());
        }
    }
    #[test]
    fn stormveil_two_to_eight_require_unanimity_skip_veto_and_all_survivors_at_finish() {
        use crate::stormveil::{Progress,Decision};
        for count in 2..=8 {
            for skip in [false,true] {
                let run=Run::new(42,crate::Class::Samurai);
                let mut gate=Gate::new((1..=count).collect());gate.stage=Stage::Service;
                let mut r=Report::new(&run,0,true,false,true);r.stormveil=Some(Progress::Choosing);r.storm_choice=Some(true);
                for id in 1..count {gate.report(id,r.clone()).unwrap();}
                assert!(gate.stormveil_order().is_none());
                let mut last=r.clone();last.storm_choice=None;gate.report(count,last.clone()).unwrap();
                assert!(gate.stormveil_order().is_none());
                last.storm_choice=Some(!skip);gate.report(count,last).unwrap();
                assert!(matches!(gate.stormveil_order(),Some(Order::Stormveil{decision,..}) if decision==if skip{Decision::Skip}else{Decision::Challenge}));
                assert!(gate.stormveil_order().is_none());
                if skip{continue;}
                r.stormveil=Some(Progress::Running);r.storm_finish=true;
                for id in 1..count {gate.report(id,r.clone()).unwrap();}
                assert!(gate.stormveil_order().is_none());
                let mut last=r.clone();last.storm_finish=false;gate.report(count,last.clone()).unwrap();
                assert!(gate.stormveil_order().is_none());
                gate.disconnect(count);assert!(gate.stormveil_order().is_none());
                last.alive=false;gate.report(count,last).unwrap();
                assert!(matches!(gate.stormveil_order(),Some(Order::Stormveil{decision:Decision::Complete,..})));
                assert!(gate.stormveil_order().is_none());
            }
        }
    }
    #[test]
    fn two_to_eight_players_wait_for_last_vote_arrival_replica_and_restart() {
        for count in 2..=crate::party_roster::MAX_PLAYERS as u64 {
            let run=Run::new(42,crate::Class::Samurai);let node=run.available()[0];
            let mut gate=Gate::new((1..=count).collect());
            let mut ready=Report::new(&run,0,false,false,true);ready.selected=Some(node);
            for id in (1..count).rev() {gate.report(id,ready.clone()).unwrap();}
            assert!(gate.resolve_votes(&run,100).unwrap().is_none());
            gate.report(count,ready).unwrap();
            assert!(matches!(gate.resolve_votes(&run,100).unwrap(),Some(Order::Enter{epoch:1,..})));
            for id in 1..count {gate.report(id,Report::new(&run,1,true,true,true)).unwrap();}
            gate.report(count,Report::unavailable(1)).unwrap();assert!(gate.spawn().is_none());
            gate.report(count,Report::new(&run,1,true,false,true)).unwrap();
            assert!(gate.spawn().is_some());assert!(gate.spawn().is_none());
            assert!(gate.engage().is_none());assert!(gate.victory().is_err());
            gate.report(count,Report::new(&run,1,true,true,true)).unwrap();
            assert!(gate.engage().is_some());assert!(gate.engage().is_none());
            for id in 1..count {gate.report(id,Report::new(&run,1,true,true,false)).unwrap();}
            assert!(gate.defeat().is_none());
            assert!(gate.victory().is_ok());assert!(gate.victory().is_err());
            for id in 1..count {gate.report(id,Report::at_hub(1)).unwrap();}
            assert!(gate.restart(43).unwrap().is_none());
            gate.report(count,Report::at_hub(1)).unwrap();
            assert!(matches!(gate.restart(43).unwrap(),Some(Order::NewRun{epoch:2,seed:43})));
        }
    }
    #[test]
    fn ended_party_restarts_only_after_every_player_returns_to_hub() {
        let mut gate=Gate::new(vec![1,2]);gate.epoch=4;gate.stage=Stage::Stopped;
        let hub=Report::at_hub(4);
        gate.report(1,hub.clone()).unwrap();assert!(gate.restart(99).unwrap().is_none());
        gate.report(2,Report::unavailable(4)).unwrap();assert!(gate.restart(99).unwrap().is_none());
        gate.report(2,hub).unwrap();
        assert!(matches!(gate.restart(99).unwrap(),Some(Order::NewRun{epoch:5,seed:99})));
        assert_eq!(gate.stage,Stage::Map);assert!(gate.restart(100).unwrap().is_none());
        let run=Run::new(99,crate::Class::Samurai);let mut report=Report::new(&run,5,false,false,true);
        report.selected=Some(run.available()[0]);
        gate.report(1,report.clone()).unwrap();assert!(gate.resolve_votes(&run,0).unwrap().is_none());
        gate.report(2,report).unwrap();assert!(matches!(gate.resolve_votes(&run,0).unwrap(),Some(Order::Enter{epoch:6,..})));
        let mut won=Gate::new(vec![1]);won.epoch=8;won.stage=Stage::Reward;
        won.report(1,Report::at_hub(8)).unwrap();assert!(won.restart(101).unwrap().is_some());
    }
    #[test]
    fn guest_guard_request_waits_for_both_replicas_and_both_completion_receipts() {
        let run=Run::new(42,crate::Class::Samurai);
        let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Service;gate.begin_events().unwrap();
        let host=Report::new(&run,0,true,false,true);
        let mut guest=host.clone();guest.event_request=Some(crate::party_events::Kind::Guards(1));
        gate.report(1,host.clone()).unwrap();gate.report(2,guest.clone()).unwrap();
        let Some(Order::EventStart(job))=gate.event_start().unwrap() else{panic!("guest event missing")};
        assert_eq!(job.player,2);
        let mut host=host;host.event_bound=Some(job.wave);gate.report(1,host.clone()).unwrap();
        assert!(!gate.event_bound(job.wave));assert!(gate.event_done(job.wave,0).is_err());
        guest.event_bound=Some(job.wave);gate.report(2,guest.clone()).unwrap();assert!(gate.event_bound(job.wave));
        assert!(gate.event_done(job.wave,1).is_err());gate.event_done(job.wave,0).unwrap();
        host.event_applied=Some(job.wave);gate.report(1,host.clone()).unwrap();
        assert!(gate.enter(&run,run.available()[0]).is_err());
        guest.event_applied=Some(job.wave);guest.event_request=None;gate.report(2,guest).unwrap();
        assert!(!gate.events_unsettled());assert!(gate.event_start().unwrap().is_none());
    }
    #[test]
    fn combat_engages_once_only_after_every_present_peer_binds() {
        let run=Run::new(42,crate::Class::Samurai);let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Combat;
        gate.report(1,Report::new(&run,0,true,true,true)).unwrap();
        gate.report(2,Report::new(&run,0,true,false,true)).unwrap();
        assert!(gate.engage().is_none());assert!(gate.victory().is_err());
        gate.report(2,Report::unavailable(0)).unwrap();assert!(gate.engage().is_none());
        gate.report(2,Report::new(&run,0,true,true,true)).unwrap();
        assert!(gate.victory().is_err());
        assert!(matches!(gate.engage(),Some(Order::Engage{epoch:0})));
        assert!(gate.engage().is_none());assert!(gate.victory().is_ok());
    }
    #[test]
    fn failed_hunt_refund_waits_for_ack_before_retry() {
        let run=Run::new(42,crate::Class::Samurai);
        let mut gate=Gate::new(vec![1]);gate.stage=Stage::Service;gate.begin_events().unwrap();
        let mut report=Report::new(&run,0,true,false,true);report.event_request=Some(crate::party_events::Kind::Scarabs(1));
        gate.report(1,report.clone()).unwrap();
        let Some(Order::EventStart(first))=gate.event_start().unwrap() else{panic!()};
        assert!(matches!(gate.event_abort(first.wave).unwrap(),Order::EventAborted{..}));
        assert!(gate.event_abort(first.wave).is_err());assert!(gate.event_start().unwrap().is_none());
        report.event_applied=Some(first.wave);report.event_request=None;gate.report(1,report.clone()).unwrap();
        assert!(gate.event_start().unwrap().is_none());
        report.event_request=Some(crate::party_events::Kind::Scarabs(1));gate.report(1,report).unwrap();
        let Some(Order::EventStart(retry))=gate.event_start().unwrap() else{panic!()};
        assert_eq!(retry.wave,first.wave+1);
    }
    #[test]
    fn event_waves_wait_for_arrival_and_binding_of_every_member() {
        let run=Run::new(42,crate::Class::Samurai);
        let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Service;gate.begin_events().unwrap();
        let mut report=Report::new(&run,0,true,false,true);
        report.event_request=Some(crate::party_events::Kind::Scarabs(1));
        gate.report(1,report.clone()).unwrap();assert!(gate.event_start().unwrap().is_none());
        gate.report(2,report.clone()).unwrap();
        let Some(Order::EventStart(first))=gate.event_start().unwrap() else{panic!("missing wave")};
        assert_eq!(first.player,1);assert!(gate.event_start().unwrap().is_none());
        assert!(gate.event_done(first.wave,5).is_err());
        report.event_bound=Some(first.wave);gate.report(1,report.clone()).unwrap();
        assert!(!gate.event_bound(first.wave));
        assert!(gate.event_done(first.wave,5).is_err());
        gate.report(2,report.clone()).unwrap();assert!(gate.event_done(first.wave,8).is_err());
        assert!(gate.event_bound(first.wave));assert!(!gate.event_bound(first.wave+1));
        assert!(matches!(gate.event_done(first.wave,5).unwrap(),Order::EventDone{killed:5,..}));
        assert!(!gate.event_bound(first.wave));
        assert!(gate.event_start().unwrap().is_none());
        report.event_applied=Some(first.wave);gate.report(1,report.clone()).unwrap();
        assert!(gate.event_start().unwrap().is_none());
        gate.report(2,report.clone()).unwrap();
        let Some(Order::EventStart(second))=gate.event_start().unwrap() else{panic!("missing second wave")};
        assert_eq!(second.player,2);assert_ne!(first.wave,second.wave);
        assert!(gate.event_done(second.wave,0).is_err());
        for id in [1,2] {report.event_bound=Some(second.wave);gate.report(id,report.clone()).unwrap();}
        gate.event_done(second.wave,0).unwrap();
        report.selected=Some(run.available()[0]);
        for id in [1,2] {gate.report(id,report.clone()).unwrap();}
        assert!(gate.resolve_votes(&run,0).unwrap().is_none());
        assert!(gate.enter(&run,run.available()[0]).is_err());
        for id in [1,2] {report.event_applied=Some(second.wave);gate.report(id,report.clone()).unwrap();}
        assert!(gate.event_start().unwrap().is_none());
        assert!(gate.resolve_votes(&run,0).unwrap().is_some());
    }
    #[test]
    fn services_never_spawn_enemies_and_wait_for_both_personal_exits() {
        for kind in [crate::NodeKind::Camp,crate::NodeKind::Shop,crate::NodeKind::Chest,crate::NodeKind::Event] {
            let (mut run,node)=(0..100).find_map(|seed|{
                let mut run=Run::new(seed,crate::Class::Samurai);
                for _ in 0..15 {
                    let available=run.available();
                    if let Some(&id)=available.iter().find(|&&id|run.nodes[id].kind==kind){return Some((run,id));}
                    run.finish(available[0]);
                }
                None
            }).unwrap();
            run.validate().unwrap();let mut gate=Gate::new(vec![1,2]);
            let mut vote=Report::new(&run,0,false,false,true);vote.selected=Some(node);
            for id in [1,2] {gate.report(id,vote.clone()).unwrap();}
            assert!(gate.resolve_votes(&run,0).unwrap().is_some());assert_eq!(gate.stage,Stage::Service);
            run.enter(node).unwrap();
            for id in [1,2] {gate.report(id,Report::new(&run,1,true,false,true)).unwrap();}
            assert!(gate.spawn().is_none());assert!(gate.victory().is_err());
            match kind {crate::NodeKind::Camp=>run.leave_camp().unwrap(),crate::NodeKind::Shop=>run.leave_shop().unwrap(),crate::NodeKind::Event=>run.leave_regional_event().unwrap(),_=>run.finish_chest().unwrap()}
            let mut next=Report::new(&run,1,true,false,true);next.selected=Some(run.available()[0]);
            gate.report(1,next.clone()).unwrap();assert!(gate.resolve_votes(&run,0).unwrap().is_none());
            gate.report(2,Report::new(&run,1,true,false,true)).unwrap();assert!(gate.resolve_votes(&run,0).unwrap().is_none());
            gate.report(2,next).unwrap();
            assert!(gate.resolve_votes(&run,0).unwrap().is_some());
        }
    }
    #[test]
    fn party_seed_tracks_actual_boss_act_transition() {
        let mut run=Run::new(42,crate::Class::Samurai);
        for act in 1..=3 {
            assert_eq!(act_seed(42,act).unwrap(),run.seed);
            if act<3 {let boss=run.nodes.iter().position(|n|n.kind==crate::NodeKind::Boss).unwrap();run.finish(boss);}
        }
        assert!(act_seed(42,0).is_err());assert!(act_seed(42,4).is_err());
    }
    #[test]
    fn aligning_first_route_preserves_personal_contract_and_resources() {
        let mut run=Run::new(17,crate::Class::Astrologer);run.enable_contracts();
        run.contract.as_mut().unwrap().choose(0,run.seed,run.class).unwrap();
        run.contract.as_mut().unwrap().skip_gift().unwrap();
        run.hp=83;run.flasks=2;run.physick_empty=true;run.gold=812;
        let original=run.clone();run.align_party_start(42).unwrap();
        assert_eq!(run.nodes,Run::new(42,run.class).nodes);
        assert_eq!(run.initial_seed,Some(42));
        let mut personal=run.clone();personal.seed=original.seed;personal.initial_seed=original.initial_seed;personal.nodes=original.nodes.clone();
        assert_eq!(personal,original);run.validate().unwrap();
        let node=run.available()[0];run.enter(node).unwrap();
        let fighting=run.clone();assert!(run.align_party_start(99).is_err());assert_eq!(run,fighting);
    }
    #[test]
    fn unloading_revokes_vote_and_is_neither_arrival_nor_death() {
        let run=Run::new(42,crate::Class::Samurai);let node=run.available()[0];
        let mut gate=Gate::new(vec![1,2]);
        let mut ready=Report::new(&run,0,true,true,true);ready.selected=Some(node);
        for id in [1,2] {gate.report(id,ready.clone()).unwrap();}
        gate.report(2,Report::unavailable(0)).unwrap();
        assert_eq!(gate.ballots()[1].node,None);
        assert!(gate.resolve_votes(&run,0).unwrap().is_none());
        gate.stage=Stage::Travel;assert!(gate.spawn().is_none());
        gate.stage=Stage::Combat;
        gate.report(1,Report::new(&run,0,true,true,false)).unwrap();
        assert!(gate.defeat().is_none());assert!(gate.victory().is_err());
        gate.stage=Stage::Map;
        for id in [1,2] {gate.report(id,Report::new(&run,0,false,false,true)).unwrap();}
        assert!(gate.resolve_votes(&run,0).unwrap().is_none());
        for id in [1,2] {gate.report(id,ready.clone()).unwrap();}
        assert!(matches!(gate.resolve_votes(&run,0).unwrap(),Some(Order::Enter{..})));
    }
    #[test]
    fn host_cannot_travel_until_guest_votes_and_result_is_shared_once() {
        let run=Run::new(42,crate::Class::Samurai);let nodes=run.available();let mut gate=Gate::new(vec![1,2]);
        let mut host=Report::new(&run,0,false,false,true);host.selected=Some(nodes[0]);
        gate.report(1,host).unwrap();gate.report(2,Report::new(&run,0,false,false,true)).unwrap();
        assert!(gate.resolve_votes(&run,1).unwrap().is_none());
        let mut guest=Report::new(&run,0,false,false,true);guest.selected=Some(*nodes.last().unwrap());
        gate.report(2,guest).unwrap();
        assert!(matches!(gate.resolve_votes(&run,1).unwrap(),Some(Order::Enter{node,epoch:1,..}) if node==*nodes.last().unwrap()));
        assert!(gate.resolve_votes(&run,1).unwrap().is_none());
    }
    #[test]
    fn travel_waits_for_all_players_and_victory_requires_replica_ack() {
        let run=Run::new(42,crate::Class::Samurai);let mut g=Gate::new(vec![1,2]);let node=run.available()[0];
        g.report(1,Report::new(&run,0,false,false,true)).unwrap();assert!(g.enter(&run,node).is_err());
        g.report(2,Report::new(&run,0,false,false,true)).unwrap();g.enter(&run,node).unwrap();
        g.report(1,Report::new(&run,1,true,true,true)).unwrap();assert!(g.spawn().is_none());
        assert!(g.report(2,Report::new(&run,0,true,true,true)).is_err());
        g.report(2,Report::new(&run,1,true,false,true)).unwrap();assert!(g.spawn().is_some());assert!(g.victory().is_err());
        g.report(2,Report::new(&run,1,true,true,true)).unwrap();assert!(g.engage().is_some());assert!(g.victory().is_ok());assert!(g.victory().is_err());
    }
    #[test]
    fn one_death_or_disconnect_never_ends_the_run_for_everyone() {
        let run=Run::new(42,crate::Class::Samurai);let mut g=Gate::new(vec![1,2]);g.stage=Stage::Combat;
        g.report(1,Report::new(&run,0,true,true,false)).unwrap();g.report(2,Report::new(&run,0,true,true,true)).unwrap();assert!(g.defeat().is_none());
        g.disconnect(2);assert!(g.defeat().is_none());
        g.report(2,Report::new(&run,0,true,true,false)).unwrap();assert!(g.defeat().is_some());
    }
    #[test]
    fn unclaimed_reward_wrong_route_and_unknown_player_cannot_unlock_travel() {
        let run=Run::new(42,crate::Class::Samurai);let mut gate=Gate::new(vec![1,2]);let node=run.available()[0];
        let report=Report::new(&run,0,false,false,true);
        assert!(gate.report(3,report.clone()).is_err());
        gate.report(1,report.clone()).unwrap();let mut other=report.clone();other.map_ready=false;
        gate.report(2,other).unwrap();assert!(gate.enter(&run,node).is_err());
        let mut other=report.clone();other.seed=123;gate.report(2,other).unwrap();assert!(gate.enter(&run,node).is_err());
        let mut other=report.clone();other.visited=vec![7];gate.report(2,other).unwrap();assert!(gate.enter(&run,node).is_err());
        gate.report(2,report).unwrap();assert!(gate.enter(&run,node).is_ok());assert!(gate.enter(&run,node).is_err());
    }
    #[test]
    fn invalid_guest_event_request_or_unreachable_vote_never_errors_the_host() {
        use crate::party_events::Kind;
        let run=Run::new(42,crate::Class::Samurai);let node=run.available()[0];
        let mut gate=Gate::new(vec![1,2]);gate.stage=Stage::Service;gate.begin_events().unwrap();
        let report=Report::new(&run,0,true,false,true);gate.report(1,report.clone()).unwrap();
        for kind in [Kind::Guards(3),Kind::Scarabs(0),Kind::Scarabs(4)] {
            let mut bad=report.clone();bad.event_request=Some(kind);assert!(gate.report(2,bad).is_err());
        }
        gate.report(2,report.clone()).unwrap();
        gate.peers[1].1.as_mut().unwrap().event_request=Some(Kind::Guards(3));
        assert!(gate.event_start().unwrap().is_none());
        let mut gate=Gate::new(vec![1,2]);let mut host=report.clone();host.arrived=false;host.selected=Some(node);
        gate.report(1,host.clone()).unwrap();
        let mut guest=host.clone();guest.selected=Some(usize::MAX);gate.report(2,guest).unwrap();
        assert!(gate.resolve_votes(&run,0).unwrap().is_none());assert_eq!(gate.epoch,0);
        gate.report(2,host).unwrap();assert!(matches!(gate.resolve_votes(&run,0).unwrap(),Some(Order::Enter{epoch:1,..})));
    }
    #[test]
    fn generated_config_is_bounded_and_separates_peer_credentials() {
        let text=r#"{"session":9,"seed":42,"player":1,"host":1,"address":"127.0.0.1:47931","token":"00000000000000000000000000000001","peers":[{"player":2,"token":"00000000000000000000000000000002"}]}"#;
        assert!(Config::decode(text).is_ok());
        assert!(Config::decode(&text.replace("\"player\":2","\"player\":1")).is_err());
        assert!(Config::decode(&text.replace("\"player\":1","\"player\":3")).is_err());
        assert!(Config::decode(&text.replace(":47931",":0")).is_err());
        assert!(Config::decode(&"x".repeat(8193)).is_err());
    }
}
