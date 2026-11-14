use serde::{Deserialize,Serialize};
use crate::{party_roster::Roster,RuleResult};
pub const VERSION:u32=93;
#[derive(Clone,Debug,Serialize,Deserialize)]
pub enum Payload {
    Handshake(Message),
    Report{session:u64,report:crate::party_game::Report},
    Order{session:u64,order:crate::party_game::Order},
    Fault{session:u64},
    Latency{session:u64,snapshot:crate::party_latency::Snapshot},
}
#[derive(Serialize,Deserialize)]
struct Envelope {magic:String,version:u32,payload:Payload}
impl Payload {
    pub fn encode(&self)->RuleResult<Vec<u8>> {
        let bytes=serde_json::to_vec(&Envelope{magic:"ERDTREE-PARTY".into(),version:VERSION,payload:self.clone()}).map_err(|_|"消息序列化失败")?;
        if bytes.len()>256*1024 {return Err("队伍消息过大".into());}Ok(bytes)
    }
    pub fn decode(bytes:&[u8])->RuleResult<Self> {
        if bytes.len()>256*1024{return Err("队伍消息过大".into());}
        let header:serde_json::Value=serde_json::from_slice(bytes).map_err(|_|"队伍消息格式无效")?;
        if let Some(version)=header.get("version").and_then(|v|v.as_u64()) {
            if version!=u64::from(VERSION){return Err(format!("爬塔协议版本不匹配：本机 {VERSION}，对方 {version}。请所有玩家更新同一版爬塔补丁。"));}
        }
        let envelope:Envelope=serde_json::from_value(header).map_err(|_|"队伍消息格式无效")?;
        if envelope.magic!="ERDTREE-PARTY"||envelope.version!=VERSION{return Err("队伍协议不匹配".into());}
        Ok(envelope.payload)
    }
}
fn yes()->bool {true}
/// Hello 的 dlc：本机是否按「有 DLC」运行（启动器里 DLC 设为「有 DLC」）；Commit 的 dlc：房主定下的整队模式，
/// 所有人都有 DLC 才按有 DLC 运行，有一个人没有就整队按「没有 DLC」运行（DLC 敌人、物品都不出）
#[derive(Clone,Debug,Serialize,Deserialize)]
pub enum Message {
    Hello{version:u32,nonce:u64,members:Vec<u64>,#[serde(default="yes")]dlc:bool,#[serde(default)]compatibility:Option<crate::party_compat::Compatibility>},
    Offer{nonce:u64,session:u64,seed:u64,members:Vec<u64>,#[serde(default)]compatibility:Option<crate::party_compat::Compatibility>},
    Ack{nonce:u64,session:u64},
    Commit{nonce:u64,session:u64,#[serde(default="yes")]dlc:bool},
    Reject{nonce:u64,reason:String},
}
pub struct Handshake {
    roster:Roster,nonce:u64,session:Option<(u64,u64)>,
    peers:Vec<(u64,Option<u64>,bool)>,committed:bool,
    /// 本机是否有 DLC；房主另记每个队员 Hello 里报的；party_dlc 是提交后整队用的模式
    dlc:bool,peer_dlc:Vec<(u64,bool)>,party_dlc:Option<bool>,
    compatibility:Option<crate::party_compat::Compatibility>,
}
impl Handshake {
    pub fn new(roster:Roster,nonce:u64,seed:u64)->RuleResult<Self> {
        if nonce==0||roster.members.len()<2||roster.members.len()>crate::party_roster::MAX_PLAYERS
            ||!roster.members.contains(&roster.local)||!roster.members.contains(&roster.host)
            ||roster.members.contains(&0)||roster.members.windows(2).any(|p|p[0]>=p[1]) {return Err("队伍握手身份无效".into());}
        let peers=roster.members.iter().filter(|&&id|id!=roster.local).map(|&id|(id,None,false)).collect();
        let session=(roster.local==roster.host).then_some((nonce,seed));
        Ok(Self{roster,nonce,session,peers,committed:false,dlc:true,peer_dlc:vec![],party_dlc:None,compatibility:None})
    }
    /// 本机是否按「有 DLC」运行，默认有
    pub fn with_dlc(mut self,dlc:bool)->Self {self.dlc=dlc;self}
    pub fn with_compatibility(mut self,profile:crate::party_compat::Compatibility)->RuleResult<Self> {
        profile.validate()?;self.compatibility=Some(profile);Ok(self)
    }
    pub fn hello(&self)->Option<(u64,Message)> {
        (self.roster.local!=self.roster.host&&!self.committed).then(||(self.roster.host,Message::Hello{version:VERSION,nonce:self.nonce,members:self.roster.members.clone(),dlc:self.dlc,compatibility:self.compatibility.clone()}))
    }
    pub fn ready(&self)->Option<(u64,u64)> {self.committed.then_some(self.session).flatten()}
    /// 提交后整队是否按「有 DLC」运行
    pub fn party_dlc(&self)->Option<bool> {self.committed.then_some(self.party_dlc).flatten()}
    pub fn waiting_reason(&self)->String {
        if self.roster.local!=self.roster.host {
            return format!("等待房主 {} 的{}。尚不能确定是网络延迟还是对方未加载爬塔模块。",self.roster.host,if self.session.is_some(){"握手提交"}else{"握手回应"});
        }
        let pending:Vec<_>=self.peers.iter().filter(|p|!p.2).map(|p|format!("队员 {}：{}",p.0,if p.1.is_none(){"尚未收到握手消息"}else{"尚未确认房主握手"})).collect();
        format!("爬塔握手未完成（队伍 {} 人）：\n{}\n尚未收到响应不等于版本不匹配；请检查对应队员是否从爬塔入口启动。",self.roster.members.len(),pending.join("\n"))
    }
    pub fn receive_lobby(&mut self,sender:u64,message:Message)->RuleResult<Vec<(u64,Message)>> {
        if matches!(&message,Message::Hello{version,..} if *version!=VERSION){return Err("队伍协议不匹配".into());}
        self.preflight(sender,&message)?;
        if matches!(&message,Message::Reject{nonce,..} if self.reject_matches(sender,*nonce)){return self.receive(sender,message);}
        if let Message::Hello{version,nonce,members,..}=&message {
            if self.roster.local==self.roster.host&&*version==VERSION&&*nonce!=0&&members==&self.roster.members {
                if let Some(peer)=self.peers.iter_mut().find(|p|p.0==sender) {
                    if peer.1.is_some_and(|old|old!=*nonce) {
                        if self.committed {return Err("队员已重新连接，需要重新建立会话".into());}
                        peer.1=Some(*nonce);peer.2=false;
                    }
                }
            }
        }
        // Joining peers can still have the previous roster or nonce; await their next hello.
        Ok(self.receive(sender,message).unwrap_or_default())
    }
    pub fn receive(&mut self,sender:u64,message:Message)->RuleResult<Vec<(u64,Message)>> {
        if sender==self.roster.local||!self.roster.members.contains(&sender){return Err("消息不属于当前队伍".into());}
        self.preflight(sender,&message)?;
        let host=self.roster.local==self.roster.host;
        match message {
            Message::Hello{version,nonce,members,dlc,..} if host=>{
                if version!=VERSION||nonce==0||members!=self.roster.members{return Err("队伍版本或成员不一致".into());}
                let peer=self.peers.iter_mut().find(|p|p.0==sender).unwrap();
                if peer.1.is_some_and(|old|old!=nonce){return Err("队员已重新连接，需要重新建立会话".into());}
                peer.1=Some(nonce);
                self.peer_dlc.retain(|p|p.0!=sender);self.peer_dlc.push((sender,dlc));
                let (session,seed)=self.session.unwrap();
                Ok(vec![(sender,Message::Offer{nonce,session,seed,members:self.roster.members.clone(),compatibility:self.compatibility.clone()})])
            },
            Message::Offer{nonce,session,seed,members,..} if !host&&sender==self.roster.host=>{
                if nonce!=self.nonce||session==0||members!=self.roster.members
                    ||self.session.is_some_and(|s|s!=(session,seed)){return Err("收到过期或冲突的队伍邀请".into());}
                self.session=Some((session,seed));
                Ok(vec![(sender,Message::Ack{nonce:self.nonce,session})])
            },
            Message::Ack{nonce,session} if host=>{
                let peer=self.peers.iter_mut().find(|p|p.0==sender).unwrap();
                if peer.1!=Some(nonce)||self.session.unwrap().0!=session{return Err("队伍确认不属于当前会话".into());}
                peer.2=true;
                if !self.peers.iter().all(|p|p.2){return Ok(vec![]);}
                self.committed=true;
                let dlc=self.dlc&&self.peers.iter().all(|p|self.peer_dlc.iter().find(|d|d.0==p.0).is_none_or(|d|d.1));
                self.party_dlc=Some(dlc);
                Ok(self.peers.iter().map(|p|(p.0,Message::Commit{nonce:p.1.unwrap(),session,dlc})).collect())
            },
            Message::Commit{nonce,session,dlc} if !host&&sender==self.roster.host=>{
                if nonce!=self.nonce||self.session.is_none_or(|s|s.0!=session){return Err("会话提交不匹配".into());}
                self.committed=true;self.party_dlc=Some(dlc);Ok(vec![])
            },
            Message::Reject{nonce,reason} if self.reject_matches(sender,nonce)=>{
                if reason.len()>512||reason.is_empty(){return Err("对端兼容检查拒绝信息无效".into());}
                Err(format!("队员 {sender} 拒绝开局：{reason}"))
            },
            _=>Err("消息发送者没有对应会话权限".into()),
        }
    }
    fn reject_matches(&self,sender:u64,nonce:u64)->bool {
        sender!=self.roster.local&&self.roster.members.contains(&sender)&&if self.roster.local==self.roster.host {
            self.peers.iter().any(|p|p.0==sender&&p.1==Some(nonce))
        }else{sender==self.roster.host&&nonce==self.nonce}
    }
    pub fn rejections(&self,sender:u64,message:&Message,reason:&str)->Vec<(u64,Message)> {
        if sender==self.roster.local||!self.roster.members.contains(&sender){return vec![];}
        let reason=reason.chars().take(120).collect::<String>();
        match message {
            Message::Hello{nonce,..} if self.roster.local==self.roster.host&&*nonce!=0=>self.peers.iter().filter_map(|peer|{
                let nonce=if peer.0==sender{Some(*nonce)}else{peer.1}?;
                Some((peer.0,Message::Reject{nonce,reason:reason.clone()}))
            }).collect(),
            Message::Offer{nonce,..} if self.roster.local!=self.roster.host&&sender==self.roster.host&&*nonce==self.nonce=>vec![(sender,Message::Reject{nonce:*nonce,reason})],
            _=>vec![],
        }
    }
    fn preflight(&self,sender:u64,message:&Message)->RuleResult<()> {
        if sender==self.roster.local||!self.roster.members.contains(&sender){return Ok(());}
        let remote=match message {
            Message::Hello{nonce,members,compatibility,..} if self.roster.local==self.roster.host&&*nonce!=0&&members==&self.roster.members=>compatibility,
            Message::Offer{nonce,members,compatibility,..} if self.roster.local!=self.roster.host&&sender==self.roster.host&&*nonce==self.nonce&&members==&self.roster.members=>compatibility,
            _=>return Ok(()),
        };
        match (&self.compatibility,remote) {
            (Some(local),Some(remote))=>local.check(remote),
            (None,None)=>Ok(()),
            _=>Err("缺少开局兼容信息，请全队更新同一构建后重启游戏".into()),
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn pending_peer_restart_replaces_nonce_and_rejects_old_ack() {
        let mut host=state(1);
        let mut first=state(2);
        let hello=first.hello().unwrap().1;
        let offer=host.receive_lobby(2,hello).unwrap().remove(0).1;
        let stale_ack=first.receive_lobby(1,offer).unwrap().remove(0).1;
        host.receive_lobby(2,stale_ack.clone()).unwrap();
        let mut restarted=Handshake::new(Roster{local:2,host:1,members:vec![1,2,3]},202,99).unwrap();
        let offer=host.receive_lobby(2,restarted.hello().unwrap().1).unwrap().pop().expect("restarted guest must receive a fresh offer").1;
        assert!(host.receive_lobby(2,stale_ack).unwrap().is_empty());
        assert!(host.ready().is_none());
        let ack=restarted.receive_lobby(1,offer).unwrap().remove(0).1;
        assert!(host.receive_lobby(2,ack).unwrap().is_empty());
        let mut third=state(3);
        let offer=host.receive_lobby(3,third.hello().unwrap().1).unwrap().remove(0).1;
        let ack=third.receive_lobby(1,offer).unwrap().remove(0).1;
        for (id,commit) in host.receive_lobby(3,ack).unwrap() {
            if id==2 {restarted.receive_lobby(1,commit).unwrap();}else{third.receive_lobby(1,commit).unwrap();}
        }
        assert_eq!(host.ready(),Some((101,42)));
        assert_eq!(restarted.ready(),host.ready());assert_eq!(third.ready(),host.ready());
    }
    #[test]
    fn committed_peer_restart_requests_a_new_lobby_session() {
        let mut host=Handshake::new(Roster{local:1,host:1,members:vec![1,2]},101,42).unwrap();
        let mut guest=Handshake::new(Roster{local:2,host:1,members:vec![1,2]},102,42).unwrap();
        let offer=host.receive_lobby(2,guest.hello().unwrap().1).unwrap().remove(0).1;
        let ack=guest.receive_lobby(1,offer).unwrap().remove(0).1;
        host.receive_lobby(2,ack).unwrap();
        let error=host.receive_lobby(2,Message::Hello{version:VERSION,nonce:202,members:vec![1,2],dlc:true,compatibility:None}).unwrap_err();
        assert!(error.contains("重新连接"));assert_eq!(host.ready(),Some((101,42)));
    }
    #[test]
    fn diagnostics_separate_missing_response_from_version_conflict() {
        let mut host=state(1);
        assert!(host.waiting_reason().contains("队员 2：尚未收到握手消息"));
        host.receive(2,super::Message::Hello{version:super::VERSION,nonce:102,members:vec![1,2,3],dlc:true,compatibility:None}).unwrap();
        assert!(host.waiting_reason().contains("队员 2：尚未确认房主握手"));
        assert!(host.waiting_reason().contains("队员 3：尚未收到握手消息"));
        let bytes=br#"{"magic":"ERDTREE-PARTY","version":1,"payload":{"UnknownFutureMessage":{}}}"#;
        let error=super::Payload::decode(bytes).unwrap_err();
        assert!(error.contains(&format!("本机 {}，对方 1",super::VERSION)));
        assert!(state(2).waiting_reason().contains("等待房主 1"));
    }
    use super::*;
    fn state(local:u64)->Handshake {Handshake::new(Roster{local,host:1,members:vec![1,2,3]},100+local,42).unwrap()}
    #[test]
    fn growing_and_shrinking_lobby_recommits_up_to_eight_players() {
        for count in (2..=crate::party_roster::MAX_PLAYERS as u64).chain((2..8).rev()) {
            let members=(1..=count).collect::<Vec<_>>();
            let mut states=members.iter().map(|&local|Handshake::new(Roster{local,host:1,members:members.clone()},count*100+local,42).unwrap()).collect::<Vec<_>>();
            assert!(states[0].receive_lobby(2,Message::Hello{version:VERSION,nonce:102,members:vec![1,2,99],dlc:true,compatibility:None}).unwrap().is_empty());
            assert!(states[0].ready().is_none());
            for id in 2..=count {
                let guest=id as usize-1;
                assert!(states[guest].receive_lobby(1,Message::Commit{nonce:102,session:101,dlc:true}).unwrap().is_empty());
                let hello=states[guest].hello().unwrap().1;
                let offer=states[0].receive_lobby(id,hello).unwrap().remove(0).1;
                let ack=states[guest].receive_lobby(1,offer).unwrap().remove(0).1;
                let commits=states[0].receive_lobby(id,ack).unwrap();
                if id<count {assert!(commits.is_empty());assert!(states[0].ready().is_none());}
                for (target,message) in commits {states[target as usize-1].receive_lobby(1,message).unwrap();}
            }
            for state in &states {assert_eq!(state.ready(),Some((count*100+1,42)));}
            assert!(states[0].receive_lobby(2,Message::Hello{version:VERSION-1,nonce:102,members,dlc:true,compatibility:None}).is_err());
        }
    }
    #[test]
    fn frames_reject_wrong_protocol_and_excessive_payloads() {
        let frame=Payload::Handshake(state(2).hello().unwrap().1);
        let bytes=frame.encode().unwrap();assert!(Payload::decode(&bytes).is_ok());
        let mut value:serde_json::Value=serde_json::from_slice(&bytes).unwrap();
        value["version"]=4.into();assert!(Payload::decode(&serde_json::to_vec(&value).unwrap()).is_err());
        value["version"]=VERSION.into();value["magic"]="other-mod".into();
        assert!(Payload::decode(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(Payload::decode(&vec![0;256*1024+1]).is_err());
    }
    #[test]
    fn all_members_ack_before_commit_and_share_seed() {
        let mut host=state(1);let mut a=state(2);let mut b=state(3);
        for guest in [&mut a,&mut b] {
            let id=guest.roster.local;
            let hello=guest.hello().unwrap().1;
            let offer=host.receive(id,hello).unwrap().remove(0).1;
            let ack=guest.receive(1,offer).unwrap().remove(0).1;
            assert!(guest.ready().is_none());
            let commits=host.receive(id,ack).unwrap();
            if id==2 {assert!(commits.is_empty());assert!(host.ready().is_none());}
        }
        let commits=host.receive(3,Message::Ack{nonce:103,session:101}).unwrap();
        for (id,message) in commits {if id==2 {a.receive(1,message).unwrap();}else{b.receive(1,message).unwrap();}}
        assert_eq!(host.ready(),Some((101,42)));assert_eq!(a.ready(),host.ready());assert_eq!(b.ready(),host.ready());
        assert!(a.hello().is_none());
    }
    #[test]
    fn party_runs_without_dlc_when_any_member_lacks_it() {
        for (host_dlc,guest_dlc,expected) in [(true,true,true),(false,false,false),(true,false,false),(false,true,false)] {
            let mut host=Handshake::new(Roster{local:1,host:1,members:vec![1,2,3]},101,42).unwrap().with_dlc(host_dlc);
            let mut a=Handshake::new(Roster{local:2,host:1,members:vec![1,2,3]},102,42).unwrap().with_dlc(guest_dlc);
            let mut b=Handshake::new(Roster{local:3,host:1,members:vec![1,2,3]},103,42).unwrap().with_dlc(true);
            assert!(host.party_dlc().is_none());
            let mut commits=vec![];
            for guest in [&mut a,&mut b] {
                let id=guest.roster.local;
                let offer=host.receive(id,guest.hello().unwrap().1).unwrap().remove(0).1;
                let ack=guest.receive(1,offer).unwrap().remove(0).1;
                commits.extend(host.receive(id,ack).unwrap());
            }
            for (id,message) in commits {if id==2 {a.receive(1,message).unwrap();}else{b.receive(1,message).unwrap();}}
            for state in [&host,&a,&b] {assert_eq!(state.party_dlc(),Some(expected),"host={host_dlc} guest={guest_dlc}");}
        }
    }
    #[test]
    fn stale_spoofed_or_different_roster_messages_do_not_unlock() {
        let mut guest=state(2);let mut host=state(1);
        assert!(guest.receive(1,Message::Commit{nonce:102,session:101,dlc:true}).is_err());
        for (sender,nonce,members) in [(3,102,vec![1,2,3]),(1,99,vec![1,2,3]),(1,102,vec![1,2])] {
            assert!(guest.receive(sender,Message::Offer{nonce,session:101,seed:42,members,compatibility:None}).is_err());
        }
        assert!(host.receive(2,Message::Ack{nonce:102,session:101}).is_err());
        assert!(host.receive(9,Message::Hello{version:VERSION,nonce:109,members:vec![1,2,3],dlc:true,compatibility:None}).is_err());
        assert!(host.ready().is_none());assert!(guest.ready().is_none());
    }
}
