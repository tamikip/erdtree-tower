use std::{sync::{Arc,atomic::{AtomicBool,Ordering},mpsc::{self,SyncSender,Receiver}},thread,time::{Duration,Instant}};
use erdtree_tower::{Run,party_game::{Config,Gate,Order,Report},party_link::*};
fn config_path(value:Option<std::ffi::OsString>)->Option<std::ffi::OsString> {value.filter(|path|!path.is_empty())}
pub fn manual_config()->Option<std::ffi::OsString> {config_path(std::env::var_os("ERDTREE_TOWER_PARTY"))}
const QUEUE:usize=1024;
fn ignored(peer:u64,error:&str) {
    static SEEN:std::sync::Mutex<Vec<String>>=std::sync::Mutex::new(vec![]);
    let key=format!("player={peer} {error}");let mut seen=SEEN.lock().unwrap_or_else(|e|e.into_inner());
    if seen.len()<32&&!seen.contains(&key){super::log(format!("PARTY_IGNORED {key}"));seen.push(key);}
}
enum Send {Broadcast(Order),Report(Report),Fault(String)}
enum Receive {Order(Order),Report(u64,Report),Disconnected(u64),Fault(String)}
struct Steam {
    transport:super::steam_messages::Transport,
    handshake:erdtree_tower::party_handshake::Handshake,
    roster:erdtree_tower::party_roster::Roster,
}
#[derive(Default)]
pub struct EventSync {
    pub chariot_pose:Option<(u64,[f32;3],Instant)>,
    pub trial_choice:Option<erdtree_tower::regional_events::Trial>,
    pub trial_decision:Option<erdtree_tower::regional_events::Trial>,
    pub has_stonesword_key:bool,
    pub storm_choice:Option<bool>,
    pub storm_finish:bool,
    pub chariot_ready:bool,
    pub storm_decision:Option<erdtree_tower::stormveil::Decision>,
    scarab_slots:Vec<(u64,u8)>,
    pub request:Option<erdtree_tower::party_events::Kind>,
    pub bound:Option<u8>,
    pub job:Option<erdtree_tower::party_events::Job>,
    pub done:Option<u8>,
    pub applied:Option<u8>,
    pub hunting:bool,
    pub aborted:bool,
}
impl EventSync {
    pub fn victory(&self)->bool {
        self.job.is_some_and(|j|matches!(j.kind,erdtree_tower::party_events::Kind::Guards(_)))
            &&self.hunting&&self.done==Some(0)&&!self.aborted
    }
    pub fn pending(&self)->bool {self.request.is_some()||self.job.is_some_and(|j|self.applied!=Some(j.wave))}
    pub fn scarab_base(&mut self,player:u64,attempt:u8)->Result<u32,String> {
        if player==0||!(1..=3).contains(&attempt){return Err("粪金龟请求无效".into());}
        let key=(player,attempt);
        let slot=if let Some(slot)=self.scarab_slots.iter().position(|p|*p==key){slot}else{
            if self.scarab_slots.len()>=erdtree_tower::party_roster::MAX_PLAYERS*3{return Err("粪金龟场景容量不足".into());}
            self.scarab_slots.push(key);self.scarab_slots.len()-1
        };
        Ok(48040000+slot as u32*3)
    }
    pub fn combat_active(&self)->bool {
        self.job.is_some_and(|j|matches!(j.kind,erdtree_tower::party_events::Kind::Guards(_))
            &&self.bound==Some(j.wave)&&self.applied!=Some(j.wave))&&self.hunting&&self.done.is_none()&&!self.aborted
    }
}
pub struct Link {
    pub drawing:erdtree_tower::map_drawing::Sync,
    drawing_sent:Option<(erdtree_tower::map_drawing::Scope,u64,Instant)>,
    pub event:EventSync,
    steam:Option<Steam>,
    queued:std::collections::VecDeque<Receive>,
    selected:Option<usize>,
    pub ballots:Vec<erdtree_tower::party_vote::Ballot>,
    pub config:Config,pub gate:Gate,pub epoch:u64,pub arrived:bool,pub bound:bool,
    pub spawn:bool,pub fighting:bool,pub victory:bool,pub rewards:bool,pub defeated:bool,pub stopped:Option<String>,
    tx:SyncSender<Send>,rx:Receiver<Receive>,running:Arc<AtomicBool>,
    last_report:Instant,last_seen:Vec<(u64,Instant)>,pub encounter:Option<Instant>,
}
impl Drop for Link {fn drop(&mut self){self.running.store(false,Ordering::Release);}}
pub struct Departure {link:Link,started:Instant,sent:Instant}
impl Departure {
    pub fn new(link:Link)->Self {
        let now=Instant::now();
        Self{link,started:now,sent:now-Duration::from_secs(1)}
    }
    pub fn tick(&mut self)->bool {
        if self.started.elapsed()>=Duration::from_secs(5){return true;}
        if self.sent.elapsed()>=Duration::from_millis(250) {
            self.sent=Instant::now();
            if let Err(error)=self.link.send(Send::Broadcast(Order::ForceEnd)){super::log(format!("HOME_SYNC send failed: {error}"));}
        }
        false
    }
}
impl Link {
    pub fn poll_force_end(&mut self,lobby:bool)->bool {
        if self.host(){return false;}
        if let Err(error)=self.receive_steam(lobby){if !self.queued.iter().any(|e|matches!(e,Receive::Fault(_))) {self.queued.push_back(Receive::Fault(error));}}
        if self.steam.is_none() {
            while self.queued.len()<QUEUE {
                match self.rx.try_recv(){
                    Ok(Receive::Order(Order::ForceEnd))=>{self.queued.clear();return true;},
                    Ok(event)=>self.queued.push_back(event),
                    Err(_)=>break,
                }
            }
        }
        let requested=self.queued.iter().any(|event|matches!(event,Receive::Order(Order::ForceEnd)));
        if requested {self.queued.clear();}
        requested
    }
    pub fn load()->Result<Option<Self>,String> {
        let Some(path)=manual_config() else{return Ok(None);};
        let config=Config::decode(&std::fs::read_to_string(path).map_err(|_|"无法读取队伍配置")?)?;
        if std::env::var("ERDTREE_TOWER_SEAMLESS_ENTRY").ok().as_deref()!=Some("1") {return Err("队伍同步必须从无缝启动入口运行".into());}
        let (tx,commands)=mpsc::sync_channel(64);let (events,rx)=mpsc::sync_channel(64);
        let running=Arc::new(AtomicBool::new(true));let flag=running.clone();let c=config.clone();
        thread::Builder::new().name("tower-party".into()).spawn(move||{
            if let Err(e)=worker(c,commands,&events,flag){let _=events.try_send(Receive::Fault(e));}
        }).map_err(|_|"无法启动队伍同步线程")?;
        let ids=std::iter::once(config.player).chain(config.peers.iter().map(|p|p.player)).collect();
        Ok(Some(Self{config,gate:Gate::new(ids),epoch:0,arrived:false,bound:false,spawn:false,fighting:false,victory:false,rewards:false,defeated:false,stopped:None,
            drawing:Default::default(),drawing_sent:None,event:Default::default(),steam:None,queued:Default::default(),selected:None,ballots:vec![],tx,rx,running,last_report:Instant::now()-Duration::from_secs(1),last_seen:vec![],encounter:None}))
    }
    pub fn from_steam(pending:super::steam_messages::Pending,session:u64,seed:u64)->Result<Self,String> {
        let roster=pending.roster.clone();
        let (transport,handshake,packets)=pending.finish();
        if handshake.ready()!=Some((session,seed)){return Err("队伍握手尚未提交".into());}
        let peers=if roster.local==roster.host {roster.members.iter().filter(|&&id|id!=roster.local).map(|&player|erdtree_tower::party_game::Peer{player,token:String::new()}).collect()}else{vec![]};
        let config=Config{session,seed,player:roster.local,host:roster.host,address:"127.0.0.1:1".parse().unwrap(),token:String::new(),peers};
        let (tx,_commands)=mpsc::sync_channel(1);let (_events,rx)=mpsc::sync_channel(1);
        let mut link=Self{gate:Gate::new(roster.members.clone()),config,epoch:0,arrived:false,bound:false,spawn:false,fighting:false,victory:false,rewards:false,defeated:false,stopped:None,
            drawing:Default::default(),drawing_sent:None,event:Default::default(),steam:Some(Steam{transport,handshake,roster}),queued:Default::default(),selected:None,ballots:vec![],tx,rx,running:Arc::new(AtomicBool::new(false)),last_report:Instant::now()-Duration::from_secs(1),last_seen:vec![],encounter:None};
        for (peer,packet) in packets {match steam_event(&link.config,peer,packet){Ok(Some(event))=>link.queued.push_back(event),Ok(None)=>{},Err(e)=>ignored(peer,&e)}}
        Ok(link)
    }
    pub fn roster(&self)->Option<&erdtree_tower::party_roster::Roster> {self.steam.as_ref().map(|s|&s.roster)}
    pub fn host(&self)->bool {self.config.player==self.config.host}
    pub fn chariot_pose(&mut self,pose:[f32;3])->Result<(),String> {
        if !self.host()||self.event.storm_decision!=Some(erdtree_tower::stormveil::Decision::Engage){return Ok(());}
        if self.event.chariot_pose.is_some_and(|(_,_,at)|at.elapsed()<Duration::from_millis(100)){return Ok(());}
        let sequence=self.event.chariot_pose.map_or(1,|(n,_,_)|n+1);
        self.send(Send::Broadcast(Order::ChariotPose{epoch:self.epoch,sequence,pose}))?;
        self.event.chariot_pose=Some((sequence,pose,Instant::now()));Ok(())
    }
    pub fn end_run(&mut self)->Result<(),String> {
        if !self.host()||self.stopped.is_some()||self.defeated{return Err("当前不能结束队伍旅程".into());}
        self.send(Send::Broadcast(Order::Defeat{epoch:self.epoch}))?;
        self.accept(&Order::Defeat{epoch:self.epoch})?;Ok(())
    }
    fn send(&mut self,command:Send)->Result<(),String> {
        use erdtree_tower::party_handshake::Payload;
        let Some(steam)=&self.steam else{return self.tx.try_send(command).map_err(|_|"队伍同步队列已满或通道已断开".into());};
        let session=self.config.session;
        match command {
            Send::Broadcast(order) if self.host()=>{
                let suspend=matches!(order,Order::ForceEnd|Order::MapDrawing(_));
                let bytes=Payload::Order{session,order}.encode()?;
                let mut failure=None;
                for peer in &self.config.peers {
                    if let Err(error)=steam.transport.send(peer.player,&bytes) {
                        if !suspend{return Err(error);}
                        failure=Some(format!("player={} {error}",peer.player));
                    }
                }
                failure.map_or(Ok(()),Err)
            },
            Send::Report(report) if !self.host()=>steam.transport.send(self.config.host,&Payload::Report{session,report}.encode()?),
            Send::Fault(_) if !self.host()=>steam.transport.send(self.config.host,&Payload::Fault{session}.encode()?),
            _=>Err("队伍发送权限不匹配".into()),
        }
    }
    pub fn stop(&mut self,reason:String) {
        if self.stopped.is_some(){return;}
        self.gate.stop();self.stopped=Some(reason.clone());
        self.selected=None;self.ballots.clear();self.queued.clear();
        let _=self.send(if self.host(){Send::Broadcast(Order::Stop(reason))}else{Send::Fault(reason)});
    }
    pub fn enter(&mut self,run:&Run,node:usize)->Result<(),String> {
        if let Some(error)=&self.stopped{return Err(error.clone());}
        if run.phase!=erdtree_tower::Phase::Map||run.contract_pending()||!run.available().contains(&node){return Err("请选择可达路线".into());}
        self.selected=Some(node);self.last_report=Instant::now()-Duration::from_secs(1);Ok(())
    }
    pub fn result(&mut self)->Result<(),String> {
        let order=self.gate.victory()?;self.send(Send::Broadcast(order))?;self.victory=true;Ok(())
    }
    pub fn event_result(&mut self,wave:u8,killed:u8)->Result<(),String> {
        if !self.host()||self.stopped.is_some(){return Err("当前不能提交事件结算".into());}
        let order=self.gate.event_done(wave,killed)?;
        self.send(Send::Broadcast(order.clone()))?;self.accept(&order)?;Ok(())
    }
    pub fn event_hunt(&mut self,wave:u8)->Result<(),String> {
        if !self.host()||self.stopped.is_some()||!self.gate.event_bound(wave)||self.event.bound!=Some(wave)
            ||self.event.done.is_some()||self.event.aborted||self.event.hunting
            ||!self.event.job.is_some_and(|j|j.wave==wave) {return Err("事件尚未全员就绪或已结束".into());}
        let order=Order::EventHunt{epoch:self.epoch,wave};
        self.send(Send::Broadcast(order.clone()))?;self.accept(&order)?;Ok(())
    }
    pub fn event_abort(&mut self,wave:u8)->Result<(),String> {
        if !self.host()||self.stopped.is_some(){return Err("当前不能取消事件".into());}
        let order=self.gate.event_abort(wave)?;
        self.send(Send::Broadcast(order.clone()))?;self.accept(&order)?;Ok(())
    }
    pub fn open_rewards(&mut self)->Result<(),String>{
        if let Some(error)=&self.stopped{return Err(error.clone());}
        if self.host()&&!self.rewards {self.send(Send::Broadcast(Order::Rewards{epoch:self.epoch}))?;self.rewards=true;}Ok(())
    }
    pub fn accept(&mut self,order:&Order)->Result<bool,String> {
        if let Order::MapDrawing(canvas)=order {return Ok(!self.host()&&self.drawing.receive(canvas.clone(),self.config.player));}
        if matches!(order,Order::ForceEnd){return Ok(!self.host());}
        if let Some(error)=&self.stopped {return Err(error.clone());}
        if self.defeated&&!matches!(order,Order::NewRun{..}|Order::Stop(_)){return Ok(false);}
        match order {
            Order::ChariotPose{epoch,sequence,pose}=>{
                if *epoch!=self.epoch||self.event.storm_decision!=Some(erdtree_tower::stormveil::Decision::Engage){return Ok(false);}
                if *sequence==0||!super::chariot::valid_pose(*pose){return Err("战车位置同步数据无效".into());}
                if self.event.chariot_pose.is_some_and(|(n,_,_)|*sequence<=n){return Ok(false);}
                self.event.chariot_pose=Some((*sequence,*pose,Instant::now()));
            },
            Order::Trial{epoch,choice} if *epoch==self.epoch=>{
                if self.event.trial_decision==Some(*choice){return Ok(false);}
                if self.event.trial_decision.is_some(){return Err("试炼选择已经锁定".into());}
                self.event.trial_decision=Some(*choice);
                if matches!(choice,erdtree_tower::regional_events::Trial::Stormveil|erdtree_tower::regional_events::Trial::Chariot){
                    self.event.storm_decision=Some(erdtree_tower::stormveil::Decision::Challenge);
                }
                super::log(format!("Party trial committed: epoch={epoch} choice={choice:?}"));
            },
            Order::Stormveil{epoch,decision} if *epoch==self.epoch=>{
                use erdtree_tower::stormveil::Decision;
                if self.event.storm_decision==Some(*decision){return Ok(false);}
                if !matches!((self.event.storm_decision,*decision),(None,Decision::Challenge|Decision::Skip)|(Some(Decision::Challenge),Decision::Engage|Decision::Complete)|(Some(Decision::Engage),Decision::Complete)) {return Err("跑城事件同步次序无效".into());}
                self.event.storm_decision=Some(*decision);
            },
            Order::NewRun{epoch,seed}=>{
                if *epoch==self.epoch&&*seed==self.config.seed{return Ok(false);}
                if self.epoch.checked_add(1)!=Some(*epoch)||*seed==0||*seed==self.config.seed{return Err("新局同步序号或种子无效".into());}
                self.config.seed=*seed;self.epoch=*epoch;
                self.event=Default::default();self.ballots.clear();self.selected=None;
                self.arrived=false;self.bound=false;self.spawn=false;self.fighting=false;self.victory=false;self.rewards=false;self.defeated=false;self.encounter=None;
                super::log(format!("Party new run committed: epoch={epoch} seed={seed}"));
            },
            Order::Votes{epoch,ballots} if *epoch==self.epoch=>{
                if ballots.is_empty()||ballots.len()>erdtree_tower::party_roster::MAX_PLAYERS||ballots.iter().enumerate().any(|(i,b)|b.player==0||ballots[..i].iter().any(|p|p.player==b.player)){return Err("无效队伍选路状态".into());}
                self.ballots=ballots.clone();
            },
            Order::Enter{epoch,has_stonesword_key,..}=>{
                if *epoch==self.epoch{return Ok(false);}
                if *epoch!=self.epoch+1{return Err("节点同步序号不连续，请重新组队".into());}
                self.event=Default::default();self.ballots.clear();self.selected=None;self.event.has_stonesword_key=*has_stonesword_key;self.epoch=*epoch;self.arrived=false;self.bound=false;self.spawn=false;self.fighting=false;self.victory=false;self.rewards=false;self.defeated=false;self.encounter=Some(Instant::now());
            }
            Order::EventStart(job) if job.epoch==self.epoch=>{
                if self.event.job==Some(*job){return Ok(false);}
                let next=self.event.job.map_or(1,|j|j.wave.saturating_add(1));
                if job.wave!=next||(self.event.job.is_some()&&self.event.done.is_none()){return Err("事件波次不连续或上一波未结束".into());}
                self.event.job=Some(*job);self.event.done=None;self.event.bound=None;self.event.hunting=false;self.event.aborted=false;
                if job.player==self.config.player&&self.event.request==Some(job.kind){self.event.request=None;}
            },
            Order::EventHunt{epoch,wave} if *epoch==self.epoch&&self.event.job.is_some_and(|j|j.wave==*wave)=>{
                if self.event.hunting{return Ok(false);}
                if self.event.bound!=Some(*wave){return Err("本轮事件尚未加载，拒绝提前计时".into());}
                if self.event.done.is_some(){return Err("事件已经结束".into());}
                self.event.hunting=true;
            },
            Order::EventAborted{epoch,wave} if *epoch==self.epoch&&self.event.job.is_some_and(|j|j.wave==*wave&&matches!(j.kind,erdtree_tower::party_events::Kind::Scarabs(_)))=>{
                if self.event.aborted{return Ok(false);}
                if self.event.done.is_some(){return Err("已结算的事件不能退款".into());}
                self.event.aborted=true;self.event.done=Some(0);
            },
            Order::EventDone{epoch,wave,killed} if *epoch==self.epoch&&*killed<=7&&!self.event.aborted&&self.event.job.is_some_and(|j|j.wave==*wave)=>{
                if let Some(previous)=self.event.done {if previous==*killed{return Ok(false);}return Err("事件结算重复且结果不一致".into());}
                self.event.done=Some(*killed);
            },
            Order::Spawn{epoch} if *epoch==self.epoch=>{self.spawn=true;self.encounter=Some(Instant::now());},
            Order::Engage{epoch} if *epoch==self.epoch&&self.spawn&&self.bound&&!self.victory=>self.fighting=true,
            Order::Victory{epoch} if *epoch==self.epoch&&self.bound=>self.victory=true,
            Order::Rewards{epoch} if *epoch==self.epoch&&self.victory=>self.rewards=true,
            Order::Defeat{epoch} if *epoch==self.epoch=>{
                self.defeated=true;self.gate.stop();self.event=Default::default();
                self.spawn=false;self.fighting=false;self.victory=false;self.rewards=false;
                self.selected=None;self.ballots.clear();
            },
            Order::Stop(error)=>{self.stopped=Some(error.clone());self.gate.stop();},
            _=>return Err("收到过期或不适用的战斗指令".into()),
        }
        Ok(true)
    }
    pub fn needs_new_run(&self)->bool {self.defeated||(self.victory&&self.rewards)}
    pub fn poll(&mut self,run:Option<&Run>,alive:bool,at_hub:bool,rest_end:bool)->Vec<Order> {
        if self.poll_force_end(at_hub&&run.is_none()){return vec![Order::ForceEnd];}
        if let Some(error)=&self.stopped{return vec![Order::Stop(error.clone())];}
        let mut orders=vec![];
        if let Err(error)=self.receive_steam(at_hub&&run.is_none()){self.stop(error);}
        for _ in 0..64 {
            let event=self.queued.pop_front().map(Ok).unwrap_or_else(||self.rx.try_recv());
            match event {
                Ok(Receive::Order(Order::ForceEnd)) if !self.host()=>return vec![Order::ForceEnd],
                Ok(Receive::Order(Order::Stop(error)))=>self.stop(error),
                Ok(Receive::Order(order))=>orders.push(order),
                Ok(Receive::Report(id,report))=>{
                    if report.epoch==self.gate.epoch {
                        match self.gate.report(id,report) {
                            Ok(())=>if let Some(p)=self.last_seen.iter_mut().find(|p|p.0==id){p.1=Instant::now();}else{self.last_seen.push((id,Instant::now()));},
                            Err(e)=>ignored(id,&e),
                        }
                    }
                },
                Ok(Receive::Disconnected(id))=>{self.gate.disconnect(id);self.stop("队员断线，同步已暂停；请退出本次验证后重新组队".into());},
                Ok(Receive::Fault(error))=>self.stop(error),
                Err(mpsc::TryRecvError::Disconnected) if self.steam.is_none()=>{self.stop("队伍网络线程已退出".into());break;},
                Err(_)=>break,
            }
        }
        self.finish_poll(run,alive,at_hub,rest_end,orders)
    }
    fn receive_steam(&mut self,lobby:bool)->Result<(),String> {
        if let Some(steam)=&mut self.steam {
                use erdtree_tower::party_handshake::Payload;
                let host=self.config.host;let guest=self.config.player!=host;
                for (peer,bytes) in steam.transport.receive()? {
                    if guest&&peer!=host {ignored(peer,"非房主队员向队友发送消息");continue;}
                    let packet=match Payload::decode(&bytes){Ok(packet)=>packet,Err(e)=>{ignored(peer,&e);continue;}};
                    match packet {
                        Payload::Handshake(message)=>for (target,response) in if lobby{steam.handshake.receive_lobby(peer,message)?}else{steam.handshake.receive(peer,message)?} {
                            steam.transport.send(target,&Payload::Handshake(response).encode()?)?;
                        },
                        packet=>if let Some(event)=steam_event(&self.config,peer,packet).unwrap_or_else(|e|{ignored(peer,&e);None}) {
                            if matches!(event,Receive::Order(Order::ForceEnd)) {
                                self.queued.clear();self.queued.push_back(event);continue;
                            }
                            if self.queued.iter().any(|e|matches!(e,Receive::Order(Order::ForceEnd))){continue;}
                            if self.queued.len()>=QUEUE {return Err("队伍接收队列已满".into());}
                            self.queued.push_back(event);
                        },
                    }
                }
        }
        Ok(())
    }
    fn finish_poll(&mut self,run:Option<&Run>,alive:bool,at_hub:bool,rest_end:bool,mut orders:Vec<Order>)->Vec<Order> {
        if let Some(error)=&self.stopped{return vec![Order::Stop(error.clone())];}
        self.drawing.set_scope(run.map(erdtree_tower::map_drawing::Scope::of));
        if self.last_report.elapsed()>=Duration::from_millis(200) {
            self.last_report=Instant::now();
            if let Some(run)=run {
                let mut report=Report::new(run,self.epoch,self.arrived,self.bound,alive);
                report.drawing=self.drawing.request();
                report.has_stonesword_key=unsafe {eldenring::cs::PlayerIns::local_player()}.is_ok_and(|p|super::loot::count(unsafe {p.player_game_data.as_ref()},0x40001f40)>0);
                report.rest_end=rest_end;
                report.event_request=self.event.request;report.event_bound=self.event.bound;report.event_applied=self.event.applied;
                report.trial_choice=self.event.trial_choice;report.storm_choice=self.event.storm_choice;report.storm_finish=self.event.storm_finish;
                report.chariot_ready=self.event.chariot_ready;
                if super::run_screen::trial_rewards_active(){report.map_ready=false;}
                if self.event.job.is_some_and(|job|self.event.applied!=Some(job.wave)){report.map_ready=false;}
                if report.map_ready&&alive {report.selected=self.selected;} else {self.selected=None;}
                if self.host(){let _=self.gate.report(self.config.player,report);}else if let Err(e)=self.send(Send::Report(report)){self.stop(e);}
            }else {
                self.selected=None;
                let mut report=if at_hub&&alive {Report::at_hub(self.epoch)}else{Report::unavailable(self.epoch)};
                report.rest_end=rest_end;
                if self.host(){let _=self.gate.report(self.config.player,report);}else if let Err(e)=self.send(Send::Report(report)){self.stop(e);}
            }
        }
        if self.host()&&self.stopped.is_none() {
            let mut stale=false;
            for (player,request) in self.gate.drawing_requests() {
                if !self.drawing.apply(player,&request) {stale|=self.drawing.canvas.as_ref().is_some_and(|c|c.scope==request.scope&&c.acknowledged.iter().any(|a|a.0==player&&a.1>=request.sequence));}
            }
            if let Some(canvas)=self.drawing.canvas.clone().filter(|c|c.revision>0) {
                let (scope,revision)=(canvas.scope,canvas.revision);
                if drawing_due(self.drawing_sent,scope,revision,stale) {
                    match self.send(Send::Broadcast(Order::MapDrawing(canvas))) {
                        Ok(())=>self.drawing_sent=Some((scope,revision,Instant::now())),
                        Err(e)=>{ignored(0,&format!("地图标记发送失败，稍后重试：{e}"));self.drawing_sent=Some(self.drawing_sent.map_or((scope,0,Instant::now()),|(s,r,_)|(s,r,Instant::now())));},
                    }
                }
            }

            if let Some(order)=self.gate.rest_end() {
                super::log(format!("PARTY_REST_END epoch={}; settling all players",self.epoch));
                if let Err(error)=self.send(Send::Broadcast(order.clone())){self.stop(error);return vec![Order::Stop(self.stopped.clone().unwrap())];}
                orders.push(order);return orders;
            }
            for &(id,seen) in &self.last_seen {
                if seen.elapsed()>Duration::from_secs(2) {self.gate.disconnect(id);}
            }
            let stale=self.last_seen.iter().any(|(_,t)|t.elapsed()>Duration::from_secs(120));
            if stale {self.stop("队员游戏状态超时，同步已暂停".into());}
            else {
                if self.needs_new_run() {
                    let next=random_entropy().map(|seed|if seed==0||seed==self.config.seed {self.config.seed.wrapping_add(1).max(1)}else{seed}).and_then(|seed|self.gate.restart(seed));
                    match next {
                        Ok(Some(order))=>{
                            if let Err(e)=self.send(Send::Broadcast(order.clone())){self.stop(e);return vec![Order::Stop(self.stopped.clone().unwrap())];}
                            orders.push(order);return orders;
                        },
                        Ok(None)=>{},Err(e)=>{self.stop(e);return vec![Order::Stop(self.stopped.clone().unwrap())];},
                    }
                }
                if let Some(run)=run {
                    let ballots=self.gate.ballots();
                    if ballots!=self.ballots {
                        self.ballots=ballots.clone();
                        if let Err(e)=self.send(Send::Broadcast(Order::Votes{epoch:self.epoch,ballots})){self.stop(e);}
                    }
                    match random_entropy().and_then(|entropy|self.gate.resolve_votes(run,entropy)) {
                        Ok(Some(order))=>if let Err(e)=self.send(Send::Broadcast(order.clone())){self.stop(e);}else{orders.push(order);},
                        Ok(None)=>{},Err(e)=>self.stop(e),
                    }
                }
                match self.gate.event_start() {
                    Ok(Some(order))=>if let Err(e)=self.send(Send::Broadcast(order.clone())){self.stop(e);}else{orders.push(order);},
                    Ok(None)=>{},Err(e)=>self.stop(e),
                }
                for order in [self.gate.defeat(),self.gate.trial_order(),self.gate.stormveil_order(),self.gate.spawn(),self.gate.engage()].into_iter().flatten() {
                    if let Err(e)=self.send(Send::Broadcast(order.clone())){self.stop(e);}else{orders.push(order);}
                }
            }
        }
        if let Some(error)=&self.stopped {vec![Order::Stop(error.clone())]}else{orders}
    }
}
fn drawing_due(sent:Option<(erdtree_tower::map_drawing::Scope,u64,Instant)>,scope:erdtree_tower::map_drawing::Scope,revision:u64,stale:bool)->bool {
    let Some((s,r,at))=sent else{return true;};
    at.elapsed()>=Duration::from_millis(if s!=scope||r!=revision{500}else if stale{1000}else{10_000})
}
fn steam_event(config:&Config,peer:u64,packet:erdtree_tower::party_handshake::Payload)->Result<Option<Receive>,String> {
    use erdtree_tower::party_handshake::Payload;
    let session=match &packet {Payload::Report{session,..}|Payload::Order{session,..}|Payload::Fault{session}=>*session,_=>return Err("握手消息未处理".into())};
    if session!=config.session {return Ok(None);}
    match packet {
        Payload::Order{order,..} if config.player!=config.host&&peer==config.host=>Ok(Some(Receive::Order(order))),
        Payload::Report{report,..} if config.player==config.host&&config.peers.iter().any(|p|p.player==peer)=>Ok(Some(Receive::Report(peer,report))),
        Payload::Fault{..} if config.player==config.host&&config.peers.iter().any(|p|p.player==peer)=>Ok(Some(Receive::Fault("队友报告游戏执行失败，请查看队友日志".into()))),
        _=>Err("Steam 队伍消息权限不匹配".into()),
    }
}
pub fn random_entropy()->Result<u64,String> {
    #[link(name="bcrypt")]
    unsafe extern "system" {fn BCryptGenRandom(handle:*mut std::ffi::c_void,buffer:*mut u8,length:u32,flags:u32)->i32;}
    let mut value=0u64;
    let status=unsafe {BCryptGenRandom(std::ptr::null_mut(),(&mut value as *mut u64).cast(),8,2)};
    if status<0 {Err("队伍选路随机数不可用".into())}else{Ok(value)}
}
#[cfg(test)]
mod tests {
    #[test]
    fn guard_victory_uses_completion_not_wave_as_kill_bits() {
        use super::*;
        use erdtree_tower::party_events::{Job,Kind};
        for wave in 1..=24 {
            let mut event=EventSync{job:Some(Job{epoch:1,wave,player:2,kind:Kind::Guards(0)}),hunting:true,done:Some(0),..Default::default()};
            assert!(event.victory());
            event.aborted=true;assert!(!event.victory());event.aborted=false;
            event.job.as_mut().unwrap().kind=Kind::Scarabs(1);event.done=Some(wave);
            assert!(!event.victory());
        }
        assert!(!EventSync::default().victory());
    }
    #[test]
    fn map_edits_from_authenticated_guest_are_broadcast_and_acknowledged() {
        use super::*;
        use erdtree_tower::map_drawing::*;
        let run=Run::new(42,erdtree_tower::Class::Samurai);
        let (mut host,events,commands)=test_link(1);host.epoch=0;
        let (mut guest,_events,_commands)=test_link(2);guest.epoch=0;
        guest.drawing.set_scope(Some(Scope::of(&run)));
        guest.drawing.submit(2,Edit::Add(vec![Point{x:100,y:100},Point{x:9000,y:9000}]),false);
        let mut report=Report::new(&run,0,false,false,true);report.drawing=guest.drawing.request();
        events.send(Receive::Report(2,report)).unwrap();
        host.poll(Some(&run),true,false,false);
        let order=commands.try_iter().find_map(|s|match s {Send::Broadcast(o@Order::MapDrawing(_))=>Some(o),_=>None}).unwrap();
        assert!(guest.accept(&order).unwrap());assert!(guest.drawing.request().is_none());
        assert_eq!(host.drawing.canvas,guest.drawing.canvas);
        assert!(!host.accept(&order).unwrap());
        let payload=erdtree_tower::party_handshake::Payload::Order{session:17,order};
        assert!(steam_event(&guest.config,3,payload.clone()).is_err());
        assert!(matches!(steam_event(&guest.config,1,payload).unwrap(),Some(Receive::Order(Order::MapDrawing(_)))));
    }
    fn test_link(player:u64)->(super::Link,super::SyncSender<super::Receive>,super::Receiver<super::Send>) {
        use super::*;
        let config=Config{session:17,seed:42,player,host:1,address:"127.0.0.1:1".parse().unwrap(),token:String::new(),peers:vec![]};
        let (tx,commands)=mpsc::sync_channel(64);let (events,rx)=mpsc::sync_channel(64);
        (Link{config,gate:Gate::new(vec![1,2]),epoch:4,arrived:true,bound:true,spawn:true,fighting:true,victory:false,rewards:false,defeated:false,stopped:None,
            drawing:Default::default(),drawing_sent:None,event:Default::default(),steam:None,queued:Default::default(),selected:None,ballots:vec![],tx,rx,running:Arc::new(AtomicBool::new(true)),last_report:Instant::now(),last_seen:vec![],encounter:None},events,commands)
    }
    #[test]
    fn malformed_guest_report_is_ignored_without_stopping_session() {
        use super::*;
        let run=Run::new(42,erdtree_tower::Class::Samurai);
        let (mut host,events,_commands)=test_link(1);
        let mut report=Report::new(&run,0,false,false,true);report.event_request=Some(erdtree_tower::party_events::Kind::Guards(3));
        events.send(Receive::Report(2,report)).unwrap();
        assert!(!host.poll(Some(&run),true,false,false).iter().any(|o|matches!(o,Order::Stop(_))));
        assert!(host.stopped.is_none());
    }
    #[test]
    fn unchanged_canvas_is_resent_only_as_slow_keepalive() {
        use super::*;
        let scope=erdtree_tower::map_drawing::Scope{seed:1,act:1};let at=|ms|Some((scope,3,Instant::now()-Duration::from_millis(ms)));
        assert!(drawing_due(None,scope,3,false));
        assert!(!drawing_due(at(9_000),scope,3,false));assert!(drawing_due(at(10_000),scope,3,false));
        assert!(!drawing_due(at(400),scope,4,false));assert!(drawing_due(at(500),scope,4,false));
        assert!(!drawing_due(at(900),scope,3,true));assert!(drawing_due(at(1000),scope,3,true));
        assert!(drawing_due(at(500),erdtree_tower::map_drawing::Scope{seed:1,act:2},3,false));
    }
    #[test]
    fn home_suspend_overrides_stopped_and_stale_room_orders() {
        use super::*;
        let (mut link,events,_commands)=test_link(2);
        link.stopped=Some("room failed".into());link.defeated=true;
        events.send(Receive::Fault("disconnected".into())).unwrap();
        events.send(Receive::Order(Order::Spawn{epoch:999})).unwrap();
        events.send(Receive::Order(Order::ForceEnd)).unwrap();
        assert!(matches!(link.poll(None,false,false,false).as_slice(),[Order::ForceEnd]));
        assert!(link.accept(&Order::ForceEnd).unwrap());
        assert!(!link.poll_force_end(false));
        let config=link.config.clone();
        use erdtree_tower::party_handshake::Payload;
        assert!(steam_event(&config,3,Payload::Order{session:17,order:Order::ForceEnd}).is_err());
        assert!(steam_event(&config,1,Payload::Order{session:16,order:Order::ForceEnd}).unwrap().is_none());
        assert!(matches!(steam_event(&config,1,Payload::Order{session:17,order:Order::ForceEnd}).unwrap(),Some(Receive::Order(Order::ForceEnd))));
        let (mut host,_,_)=test_link(1);assert!(!host.accept(&Order::ForceEnd).unwrap());
    }
    #[test]
    fn home_departure_retains_transport_and_retries_without_waiting() {
        use super::*;
        let (link,_events,commands)=test_link(1);let running=link.running.clone();
        let mut departure=Departure::new(link);
        assert!(!departure.tick());assert!(running.load(Ordering::Acquire));
        assert!(matches!(commands.try_recv(),Ok(Send::Broadcast(Order::ForceEnd))));
        assert!(!departure.tick());assert!(commands.try_recv().is_err());
        departure.sent-=Duration::from_secs(1);assert!(!departure.tick());
        assert!(matches!(commands.try_recv(),Ok(Send::Broadcast(Order::ForceEnd))));
        departure.started-=Duration::from_secs(6);assert!(departure.tick());
        drop(departure);assert!(!running.load(Ordering::Acquire));
    }
    #[test]
    fn scarab_slots_are_unique_per_player_attempt_and_stable_for_retry() {
        let mut event=super::EventSync::default();let mut ids=std::collections::HashSet::new();
        for player in 1..=8 {for attempt in 1..=3 {
            let base=event.scarab_base(player,attempt).unwrap();
            for slot in 0..3 {assert!(ids.insert(base+slot));}
            assert_eq!(event.scarab_base(player,attempt).unwrap(),base);
        }}
        assert_eq!(ids.len(),72);assert!(event.scarab_base(9,1).is_err());
        assert!(event.scarab_base(1,0).is_err());
    }
    #[test]
    fn paid_event_blocks_exit_until_result_is_applied() {
        let mut event=super::EventSync::default();assert!(!event.pending());
        event.request=Some(erdtree_tower::party_events::Kind::Scarabs(1));assert!(event.pending());
        event.job=Some(erdtree_tower::party_events::Job{epoch:1,wave:1,player:1,kind:event.request.take().unwrap()});
        assert!(event.pending());event.done=Some(0);assert!(event.pending());event.applied=Some(1);assert!(!event.pending());
    }
    #[test]
    fn missing_and_empty_config_select_automatic_party_detection() {
        assert!(super::config_path(None).is_none());
        assert!(super::config_path(Some(std::ffi::OsString::new())).is_none());
        let path=std::ffi::OsString::from("C:/test folder/party.json");
        assert_eq!(super::config_path(Some(path.clone())),Some(path));
    }
    use super::*;
    #[test]
    fn defeat_clears_event_and_ignores_late_same_epoch_orders() {
        let config=Config{session:17,seed:42,player:2,host:1,address:"127.0.0.1:1".parse().unwrap(),token:String::new(),peers:vec![]};
        let (tx,_commands)=mpsc::sync_channel(64);let (_events,rx)=mpsc::sync_channel(64);
        let mut link=Link{config,gate:Gate::new(vec![1,2]),epoch:4,arrived:true,bound:true,spawn:true,fighting:true,victory:false,rewards:false,defeated:false,stopped:None,
            drawing:Default::default(),drawing_sent:None,event:Default::default(),steam:None,queued:Default::default(),selected:None,ballots:vec![],tx,rx,running:Arc::new(AtomicBool::new(false)),last_report:Instant::now(),last_seen:vec![],encounter:None};
        let job=erdtree_tower::party_events::Job{epoch:4,wave:1,player:2,kind:erdtree_tower::party_events::Kind::Guards(0)};
        link.accept(&Order::EventStart(job)).unwrap();link.event.bound=Some(1);
        link.accept(&Order::EventHunt{epoch:4,wave:1}).unwrap();assert!(link.event.combat_active());
        assert!(link.accept(&Order::Defeat{epoch:4}).unwrap());
        assert!(!link.event.combat_active());assert!(!link.fighting&&!link.spawn&&!link.rewards);
        for order in [Order::Defeat{epoch:4},Order::EventStart(job),Order::EventHunt{epoch:4,wave:1},Order::EventDone{epoch:4,wave:1,killed:0},Order::Spawn{epoch:4},Order::Victory{epoch:4}] {
            assert!(!link.accept(&order).unwrap());
        }
        assert!(link.event.job.is_none());
        assert!(link.accept(&Order::NewRun{epoch:5,seed:99}).unwrap());
        assert!(!link.defeated);
    }
    #[test]
    fn new_run_clears_previous_vote_combat_and_event_state_once() {
        let config=Config{session:17,seed:42,player:2,host:1,address:"127.0.0.1:1".parse().unwrap(),token:String::new(),peers:vec![]};
        let (tx,_commands)=mpsc::sync_channel(64);let (_events,rx)=mpsc::sync_channel(64);
        let mut link=Link{config,gate:Gate::new(vec![1,2]),epoch:4,arrived:true,bound:true,spawn:true,fighting:true,victory:true,rewards:true,defeated:true,stopped:None,
            drawing:Default::default(),drawing_sent:None,event:Default::default(),steam:None,queued:Default::default(),selected:Some(3),ballots:vec![erdtree_tower::party_vote::Ballot{player:2,node:Some(3)}],tx,rx,running:Arc::new(AtomicBool::new(false)),last_report:Instant::now(),last_seen:vec![],encounter:Some(Instant::now())};
        link.event.job=Some(erdtree_tower::party_events::Job{epoch:4,wave:2,player:2,kind:erdtree_tower::party_events::Kind::Guards(1)});
        let next=Order::NewRun{epoch:5,seed:99};assert!(link.accept(&next).unwrap());
        assert_eq!((link.epoch,link.config.seed),(5,99));assert!(!link.needs_new_run());
        assert!(!link.arrived&&!link.bound&&!link.spawn&&!link.fighting&&!link.victory&&!link.rewards&&!link.defeated);
        assert!(link.event.job.is_none()&&link.selected.is_none()&&link.ballots.is_empty()&&link.encounter.is_none());
        link.selected=Some(1);assert!(!link.accept(&next).unwrap());assert_eq!(link.selected,Some(1));
        assert!(link.accept(&Order::Victory{epoch:4}).is_err());
        assert!(link.accept(&Order::NewRun{epoch:6,seed:99}).is_err());
    }
    #[test]
    fn only_bound_unfinished_guard_wave_opens_combat_services() {
        let job=erdtree_tower::party_events::Job{epoch:1,wave:2,player:1,kind:erdtree_tower::party_events::Kind::Guards(0)};
        let mut event=EventSync{job:Some(job),..Default::default()};
        assert!(!event.combat_active());event.bound=Some(1);assert!(!event.combat_active());
        event.bound=Some(2);assert!(!event.combat_active());event.hunting=true;assert!(event.combat_active());
        event.done=Some(0);assert!(!event.combat_active());event.done=None;
        event.applied=Some(2);assert!(!event.combat_active());event.applied=None;
        event.aborted=true;assert!(!event.combat_active());event.aborted=false;
        event.job=Some(erdtree_tower::party_events::Job{kind:erdtree_tower::party_events::Kind::Scarabs(1),..job});
        assert!(!event.combat_active());
    }
    #[test]
    fn fault_discards_travel_already_queued_in_the_same_poll() {
        for signal in 0..3 {
        let config=Config{session:17,seed:42,player:2,host:1,address:"127.0.0.1:1".parse().unwrap(),token:String::new(),peers:vec![]};
        let (tx,_commands)=mpsc::sync_channel(64);let (events,rx)=mpsc::sync_channel(64);
        let mut link=Link{config,gate:Gate::new(vec![1,2]),epoch:0,arrived:false,bound:false,spawn:false,fighting:false,victory:false,rewards:false,defeated:false,stopped:None,
            drawing:Default::default(),drawing_sent:None,event:Default::default(),steam:None,queued:Default::default(),selected:Some(0),ballots:vec![],tx,rx,running:Arc::new(AtomicBool::new(false)),last_report:Instant::now(),last_seen:vec![],encounter:None};
        let job=erdtree_tower::party_events::Job{epoch:0,wave:1,player:2,kind:erdtree_tower::party_events::Kind::Scarabs(1)};
        use erdtree_tower::stormveil::Decision;
        assert!(link.accept(&Order::Stormveil{epoch:1,decision:Decision::Challenge}).is_err());
        assert!(link.accept(&Order::Stormveil{epoch:0,decision:Decision::Complete}).is_err());
        assert!(link.accept(&Order::Stormveil{epoch:0,decision:Decision::Challenge}).unwrap());
        assert!(!link.accept(&Order::Stormveil{epoch:0,decision:Decision::Challenge}).unwrap());
        assert!(link.accept(&Order::Stormveil{epoch:0,decision:Decision::Skip}).is_err());
        assert!(link.accept(&Order::Stormveil{epoch:0,decision:Decision::Complete}).unwrap());
        assert!(!link.accept(&Order::Stormveil{epoch:0,decision:Decision::Complete}).unwrap());
        link.event=Default::default();
        use erdtree_tower::regional_events::Trial;
        assert!(link.accept(&Order::Trial{epoch:1,choice:Trial::Chariot}).is_err());
        assert!(link.accept(&Order::Trial{epoch:0,choice:Trial::Chariot}).unwrap());
        assert!(!link.accept(&Order::Trial{epoch:0,choice:Trial::Chariot}).unwrap());
        assert!(link.accept(&Order::Trial{epoch:0,choice:Trial::Skip}).is_err());
        assert_eq!(link.event.storm_decision,Some(Decision::Challenge));
        assert!(link.accept(&Order::Stormveil{epoch:0,decision:Decision::Engage}).unwrap());
        let pose=Order::ChariotPose{epoch:0,sequence:1,pose:[1.,2.,0.]};
        assert!(link.accept(&pose).unwrap());assert!(!link.accept(&pose).unwrap());
        assert!(!link.accept(&Order::ChariotPose{epoch:1,sequence:2,pose:[0.;3]}).unwrap());
        assert!(link.accept(&Order::ChariotPose{epoch:0,sequence:2,pose:[f32::NAN,0.,0.]}).is_err());
        assert!(link.accept(&Order::ChariotPose{epoch:0,sequence:2,pose:[90.,0.,0.]}).is_err());
        assert_eq!(link.event.chariot_pose.unwrap().0,1);
        link.event=Default::default();
        assert!(!link.accept(&pose).unwrap());assert!(link.event.chariot_pose.is_none());
        link.event.request=Some(job.kind);
        assert!(link.accept(&Order::EventStart(job)).unwrap());
        assert!(link.event.request.is_none());
        assert!(!link.accept(&Order::EventStart(job)).unwrap());
        assert!(link.accept(&Order::EventStart(erdtree_tower::party_events::Job{wave:2,..job})).is_err());
        let hunt=Order::EventHunt{epoch:0,wave:1};
        assert!(link.accept(&hunt).is_err());assert!(!link.event.hunting);
        link.event.bound=Some(2);assert!(link.accept(&hunt).is_err());assert!(!link.event.hunting);
        link.event.bound=Some(1);
        assert!(link.accept(&hunt).unwrap());assert!(!link.accept(&hunt).unwrap());
        let done=Order::EventDone{epoch:0,wave:1,killed:5};
        assert!(link.accept(&done).unwrap());assert!(!link.accept(&done).unwrap());
        assert!(link.accept(&Order::EventDone{epoch:0,wave:1,killed:3}).is_err());
        assert!(link.accept(&Order::EventStart(erdtree_tower::party_events::Job{wave:2,..job})).unwrap());
        assert!(link.event.done.is_none());assert!(link.event.bound.is_none());assert!(!link.event.hunting);
        let abort=Order::EventAborted{epoch:0,wave:2};
        assert!(link.accept(&abort).unwrap());assert!(!link.accept(&abort).unwrap());
        assert!(link.event.aborted);
        assert!(link.accept(&Order::EventDone{epoch:0,wave:2,killed:0}).is_err());
        assert!(link.accept(&Order::Engage{epoch:0}).is_err());
        link.accept(&Order::Spawn{epoch:0}).unwrap();
        assert!(link.accept(&Order::Engage{epoch:0}).is_err());
        link.bound=true;link.accept(&Order::Engage{epoch:0}).unwrap();assert!(link.fighting);
        assert!(link.accept(&Order::Engage{epoch:1}).is_err());
        events.send(Receive::Order(Order::Enter{epoch:1,act:1,visited:vec![],node:0,has_stonesword_key:false})).unwrap();
        events.send(match signal {
            0=>Receive::Fault("lost peer".into()),
            1=>Receive::Order(Order::Stop("host stopped".into())),
            _=>Receive::Disconnected(1),
        }).unwrap();
        events.send(Receive::Order(Order::Spawn{epoch:1})).unwrap();
        for _ in 0..2 {
            let orders=link.poll(None,true,false,false);
            assert!(matches!(orders.as_slice(),[Order::Stop(_)]));
            assert_eq!(link.epoch,0);
        }
        assert!(link.accept(&Order::Enter{epoch:1,act:1,visited:vec![],node:0,has_stonesword_key:false}).is_err());
        assert!(link.open_rewards().is_err());
        }
    }
    #[test]
    fn steam_messages_enforce_role_and_ignore_previous_session() {
        use erdtree_tower::party_handshake::Payload;
        let mut config=Config{session:17,seed:42,player:1,host:1,address:"127.0.0.1:1".parse().unwrap(),token:String::new(),peers:vec![erdtree_tower::party_game::Peer{player:2,token:String::new()}]};
        let report=Report::unavailable(0);
        assert!(matches!(steam_event(&config,2,Payload::Report{session:17,report:report.clone()}).unwrap(),Some(Receive::Report(2,_))));
        assert!(steam_event(&config,3,Payload::Report{session:17,report:report.clone()}).is_err());
        assert!(steam_event(&config,2,Payload::Order{session:17,order:Order::Spawn{epoch:1}}).is_err());
        assert!(steam_event(&config,2,Payload::Report{session:16,report}).unwrap().is_none());
        config.player=2;config.peers.clear();
        assert!(matches!(steam_event(&config,1,Payload::Order{session:17,order:Order::Spawn{epoch:1}}).unwrap(),Some(Receive::Order(Order::Spawn{epoch:1}))));
        assert!(steam_event(&config,3,Payload::Order{session:17,order:Order::Spawn{epoch:1}}).is_err());
        assert!(steam_event(&config,1,Payload::Order{session:16,order:Order::Spawn{epoch:1}}).unwrap().is_none());
    }
    struct WorkerGuard(Arc<AtomicBool>);
    impl Drop for WorkerGuard {fn drop(&mut self){self.0.store(false,Ordering::Release);}}
    #[test]
    fn home_broadcast_reaches_multiple_clients_despite_missing_peer() {
        let reservation=std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address=reservation.local_addr().unwrap();drop(reservation);
        let token="0123456789abcdef0123456789abcdef".to_string();
        let config=Config{session:17,seed:42,player:1,host:1,address,token:token.clone(),peers:(2..=4).map(|player|erdtree_tower::party_game::Peer{player,token:format!("{token}{player}")}).collect()};
        let (tx,commands)=mpsc::sync_channel(64);let (events,_rx)=mpsc::sync_channel(64);
        let running=Arc::new(AtomicBool::new(true));let guard=WorkerGuard(running.clone());
        let task=thread::spawn(move||worker(config,commands,&events,running));
        let deadline=Instant::now()+Duration::from_secs(3);
        let mut clients=vec![];
        for player in [3,4] {
            loop {
                if let Ok(client)=Client::connect(address,17,format!("{token}{player}")){clients.push(client);break;}
                assert!(Instant::now()<deadline);thread::sleep(Duration::from_millis(10));
            }
        }
        let mut connected=[false;2];let mut received=[false;2];
        while Instant::now()<deadline&&!received.iter().all(|r|*r) {
            for (i,client) in clients.iter_mut().enumerate() {for event in client.poll() {match event {
                ClientEvent::Connected(_)=>connected[i]=true,
                ClientEvent::Message(HostMessage::Game(Order::ForceEnd))=>received[i]=true,
                ClientEvent::Disconnected=>panic!("host dropped before suspend delivery"),
                _=>{},
            }}}
            if connected.iter().all(|r|*r){tx.send(Send::Broadcast(Order::ForceEnd)).unwrap();}
            thread::sleep(Duration::from_millis(10));
        }
        drop(guard);assert!(task.join().unwrap().is_ok());
        assert!(received.iter().all(|r|*r));
    }
    #[test]
    fn votes_before_connection_are_replayed_and_worker_keeps_delivering() {
        let reservation=std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address=reservation.local_addr().unwrap();drop(reservation);
        let token="0123456789abcdef0123456789abcdef".to_string();
        let config=Config{session:17,seed:42,player:1,host:1,address,token:token.clone(),peers:vec![erdtree_tower::party_game::Peer{player:2,token:token.clone()}]};
        let (tx,commands)=mpsc::sync_channel(64);let (events,_rx)=mpsc::sync_channel(64);
        let running=Arc::new(AtomicBool::new(true));let guard=WorkerGuard(running.clone());
        let task=thread::spawn(move||worker(config,commands,&events,running));
        tx.send(Send::Broadcast(Order::Votes{epoch:0,ballots:vec![erdtree_tower::party_vote::Ballot{player:1,node:Some(3)}]})).unwrap();
        thread::sleep(Duration::from_millis(100));
        let deadline=Instant::now()+Duration::from_secs(3);
        let mut client=loop {
            if let Ok(client)=Client::connect(address,17,token.clone()) {break client;}
            assert!(Instant::now()<deadline,"host never accepted a connection");
            thread::sleep(Duration::from_millis(10));
        };
        let mut replayed=false;let mut entered=false;
        while Instant::now()<deadline&&!entered {
            for event in client.poll() {
                match event {
                    ClientEvent::Message(HostMessage::Game(Order::Votes{epoch:0,ballots}))=>{
                        assert_eq!(ballots[0].node,Some(3));replayed=true;
                        tx.send(Send::Broadcast(Order::Enter{epoch:1,act:1,visited:vec![],node:3,has_stonesword_key:false})).unwrap();
                    },
                    ClientEvent::Message(HostMessage::Game(Order::Enter{epoch:1,node:3,..}))=>entered=true,
                    ClientEvent::Disconnected=>panic!("worker disconnected after pre-connection votes"),
                    _=>{},
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        drop(guard);assert!(task.join().unwrap().is_ok());
        assert!(replayed&&entered,"snapshot or subsequent travel was lost");
    }
}
fn emit(backlog:&mut Backlog<Receive>,event:Receive)->Result<(),String>{if backlog.push(event){Ok(())}else{Err("游戏线程未及时处理队伍消息".into())}}
fn deliver(backlog:&mut Backlog<Receive>,events:&SyncSender<Receive>) {
    backlog.flush(|event|events.try_send(event).map_err(|e|match e{mpsc::TrySendError::Full(v)|mpsc::TrySendError::Disconnected(v)=>v}));
}
fn worker(config:Config,commands:Receiver<Send>,events:&SyncSender<Receive>,running:Arc<AtomicBool>)->Result<(),String> {
    if config.player==config.host {
        let mut host=Host::bind(config.address,config.session,config.peers.iter().map(|p|Credential{player:p.player,token:p.token.clone()}).collect()).map_err(|_|"队伍监听端口无法打开")?;
        let mut connected=std::collections::HashSet::new();
        let mut votes:Option<Order>=None;let mut backlog=Backlog::new(QUEUE);
        while running.load(Ordering::Acquire) {
            for event in host.poll(){match event{
                HostEvent::Message{player,message:ClientMessage::Game(report)}=>backlog.latest(player,Receive::Report(player,report)),
                HostEvent::Message{message:ClientMessage::Fault(_),..}=>emit(&mut backlog,Receive::Fault("队友报告游戏执行失败，请查看队友日志".into()))?,
                HostEvent::Message{player,..}=>ignored(player,"队伍协议不匹配"),
                HostEvent::Disconnected(id)=>{connected.remove(&id);emit(&mut backlog,Receive::Disconnected(id))?;},
                HostEvent::Connected(id)=>{
                    connected.insert(id);
                    if let Some(order)=&votes {host.send(id,HostMessage::Game(order.clone())).map_err(|_|"无法补发队伍选路状态")?;}
                },
            }}
            for command in commands.try_iter(){if let Send::Broadcast(order)=command {
                let snapshot=matches!(order,Order::Votes{..}|Order::MapDrawing(_));
                if matches!(order,Order::Votes{..}) {votes=Some(order.clone());}
                if matches!(order,Order::Enter{..}|Order::NewRun{..}|Order::Stop(_)) {votes=None;}
                for p in &config.peers{
                    if snapshot&&!connected.contains(&p.player) {continue;}
                    if let Err(error)=host.send(p.player,HostMessage::Game(order.clone())) {
                        if matches!(order,Order::MapDrawing(_)){ignored(p.player,&format!("地图标记发送失败，稍后重试：{error}"));continue;}
                        if !matches!(order,Order::ForceEnd){return Err("队员未连接，指令未送达".into());}
                        super::log(format!("HOME_SYNC player={} send failed: {error}",p.player));
                    }
                }
            }}
            deliver(&mut backlog,events);
            thread::sleep(Duration::from_millis(10));
        }
    }else {
        let mut attempt=0;
        let connect=|attempt:&mut u32|->Result<Client,String>{loop{
            if *attempt>0{thread::sleep(Duration::from_millis(500<<(*attempt-1)));}
            *attempt+=1;
            match Client::connect(config.address,config.session,config.token.clone()) {Ok(client)=>return Ok(client),Err(_) if *attempt<3=>{},Err(_)=>return Err("无法连接爬塔房主（已重试 3 次）".into())}
        }};
        let mut client=connect(&mut attempt)?;
        let mut connected=false;let mut backlog=Backlog::new(QUEUE);
        while running.load(Ordering::Acquire) {
            for event in client.poll(){match event{
                ClientEvent::Connected(id) if id==config.player=>connected=true,
                ClientEvent::Message(HostMessage::Game(order@Order::MapDrawing(_)))=>backlog.latest(0,Receive::Order(order)),
                ClientEvent::Message(HostMessage::Game(order))=>emit(&mut backlog,Receive::Order(order))?,
                ClientEvent::Mismatch(version)=>return Err(format!("爬塔协议版本不匹配：本机 {VERSION}，房主 {version}。请所有玩家更新同一版爬塔补丁。")),
                ClientEvent::Disconnected if !connected&&attempt<3=>{client=connect(&mut attempt)?;break;},
                ClientEvent::Disconnected if !connected=>return Err("爬塔房主拒绝连接或未响应（已重试 3 次）".into()),
                ClientEvent::Disconnected=>return Err("爬塔房主已断开".into()),
                _=>return Err("房主协议或队员身份不匹配".into()),
            }}
            deliver(&mut backlog,events);
            for command in commands.try_iter(){if connected{
                let message=match command{Send::Report(r)=>ClientMessage::Game(r),Send::Fault(e)=>ClientMessage::Fault(e),_=>return Err("队友不能发送房主指令".into())};
                client.send(message).map_err(|_|"无法发送队伍状态")?;
            }}
            thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(())
}
