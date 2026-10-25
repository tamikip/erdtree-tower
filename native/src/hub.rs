use std::{path::PathBuf, time::{Duration,Instant}};
use eldenring::cs::{BlockId,GameMan,WorldChrMan};
use erdtree_tower::{Class,Phase,Run};
pub const BLOCK: i32=0x0b0a0000;
fn seeded_start_allowed()->bool {
    use fromsoftware_shared::FromStatic;
    if super::party::manual_config().is_some(){return false;}
    if !super::seamless::active(){return true;}
    unsafe {eldenring::cs::CSSessionManager::instance()}.ok()
        .is_some_and(|session|session.lobby_state==eldenring::cs::LobbyState::None||super::seamless::roster(session).is_ok_and(|r|r.members.len()==1&&r.local==r.host))
}
#[derive(Debug,PartialEq,Eq)]
enum WarpStep {Request,Wait,Ready,Failed}
fn choice_allowed(npc:usize,choice:i32,party_ready:bool)->bool {
    party_ready||npc!=0||!matches!(choice,1|2|3)
}
fn follows_host(remote:bool,paused:bool,settled:bool)->bool {remote&&!paused&&!settled}
#[derive(Default)]
enum WarpGate {
    #[default]
    Idle,
    Loading {requested:Duration,stable:Option<Duration>},
    Ready,
    Failed,
}
impl WarpGate {
    fn follow(&mut self,now:Duration,arrived:bool,grounded:bool)->WarpStep {
        if !arrived||!grounded {*self=Self::Idle;return WarpStep::Wait;}
        if matches!(self,Self::Idle|Self::Failed) {*self=Self::Loading{requested:now,stable:Some(now)};}
        self.poll(now,arrived,grounded)
    }
    fn poll(&mut self,now:Duration,arrived:bool,grounded:bool)->WarpStep {
        match self {
            Self::Idle=>{
                *self=Self::Loading {requested:now,stable:None};
                WarpStep::Request
            },
            Self::Loading {requested,stable}=>{
                if now.saturating_sub(*requested)>=Duration::from_secs(60) {
                    *self=Self::Failed;
                    return WarpStep::Failed;
                }
                if !arrived || !grounded {*stable=None;return WarpStep::Wait;}
                let since=*stable.get_or_insert(now);
                if now.saturating_sub(since)<Duration::from_secs(2) {return WarpStep::Wait;}
                *self=Self::Ready;
                WarpStep::Ready
            },
            Self::Ready=>WarpStep::Ready,
            Self::Failed=>WarpStep::Failed,
        }
    }
}
pub fn path(owner:(u32,i32), suffix:&str)->PathBuf {
    super::LOG_PATH.with_file_name(format!("tower-{}-{}.{suffix}",owner.1,owner.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sec(n:u64)->Duration {Duration::from_secs(n)}
    #[test]
    fn settled_guest_returns_independently_instead_of_waiting_forever() {
        assert!(follows_host(true,false,false));
        assert!(!follows_host(true,false,true));
        assert!(!follows_host(true,true,false));
        assert!(!follows_host(false,false,false));
        let mut gate=WarpGate::default();
        let step=if follows_host(true,false,true){gate.follow(sec(0),false,true)}else{gate.poll(sec(0),false,true)};
        assert_eq!(step,WarpStep::Request);
        assert_eq!(gate.poll(sec(1),false,true),WarpStep::Wait);
        assert_eq!(gate.poll(sec(2),true,true),WarpStep::Wait);
        assert_eq!(gate.poll(sec(4),true,true),WarpStep::Ready);
    }
    #[test]
    fn pending_party_keeps_class_service_but_blocks_start_and_resume() {
        assert!(!choice_allowed(0,1,false));
        assert!(!choice_allowed(0,2,false));
        assert!(choice_allowed(0,1,true));
        assert!(choice_allowed(0,2,true));
        for choice in 1..=10 {assert!(choice_allowed(1,choice,false));}
        assert!(choice_allowed(0,0,false));
    }
    #[test]
    fn guest_waits_for_host_destination_without_issuing_a_warp() {
        let mut gate=WarpGate::default();
        for t in 0..120 {assert_eq!(gate.follow(sec(t),false,true),WarpStep::Wait);}
        assert_eq!(gate.follow(sec(120),true,true),WarpStep::Wait);
        assert_eq!(gate.follow(sec(122),true,true),WarpStep::Ready);
        assert_eq!(gate.follow(sec(123),true,false),WarpStep::Wait);
        assert_eq!(gate.follow(sec(124),true,true),WarpStep::Wait);
        assert_eq!(gate.follow(sec(126),true,true),WarpStep::Ready);
    }
    #[test]
    fn old_map_frames_never_resubmit_warp() {
        let mut gate=WarpGate::default();
        assert_eq!(gate.poll(sec(0),false,false),WarpStep::Request);
        for t in 1..20 {assert_eq!(gate.poll(sec(t),false,false),WarpStep::Wait);}
        assert_eq!(gate.poll(sec(20),true,true),WarpStep::Wait);
        assert_eq!(gate.poll(sec(21),true,true),WarpStep::Wait);
        assert_eq!(gate.poll(sec(22),true,true),WarpStep::Ready);
        assert_eq!(gate.poll(sec(90),true,true),WarpStep::Ready);
    }
    #[test]
    fn arrival_must_stay_grounded_before_reset() {
        let mut gate=WarpGate::default();
        gate.poll(sec(0),false,false);
        gate.poll(sec(3),true,true);
        gate.poll(sec(4),true,false);
        assert_eq!(gate.poll(sec(5),true,true),WarpStep::Wait);
        assert_eq!(gate.poll(sec(6),false,true),WarpStep::Wait);
        assert_eq!(gate.poll(sec(7),true,true),WarpStep::Wait);
        assert_eq!(gate.poll(sec(9),true,true),WarpStep::Ready);
    }
    #[test]
    fn timeout_is_terminal_until_explicit_new_trip() {
        let mut gate=WarpGate::default();
        gate.poll(sec(0),false,false);
        assert_eq!(gate.poll(sec(60),false,false),WarpStep::Failed);
        assert_eq!(gate.poll(sec(61),true,true),WarpStep::Failed);
        assert_eq!(gate.poll(sec(90),false,false),WarpStep::Failed);
        gate=WarpGate::default();
        assert_eq!(gate.poll(sec(91),true,true),WarpStep::Request);
    }
    #[test]
    fn same_map_reload_sends_one_request_and_waits_for_completion() {
        let mut gate=WarpGate::default();
        assert_eq!(gate.poll(sec(0),true,true),WarpStep::Request);
        for t in 1..5 {assert_eq!(gate.poll(sec(t),false,true),WarpStep::Wait);}
        assert_eq!(gate.poll(sec(5),true,true),WarpStep::Wait);
        assert_eq!(gate.poll(sec(7),true,true),WarpStep::Ready);
    }
}
pub struct Hub {
    pub active: bool,
    pub paused: bool,
    pub owner: (u32,i32),
    room: Option<super::rooms::Room>,
    clock: Instant,
    warp: WarpGate,
    reset: bool,
    start: bool,
    chosen: u8,
    resume: Option<Run>,
    seed:Option<u64>,
    settled:bool,
}
impl Hub {
    pub fn dialog_active(&self)->bool {self.active && self.room.as_ref().is_some_and(|r|r.dialog_active())}
    pub fn new(owner:(u32,i32), archetype:u8)->Result<Self,String> {
        let save=path(owner,"json");
        let resume=if save.exists() {Some(Run::decode(&std::fs::read_to_string(save).map_err(|e|e.to_string())?)?)} else {None};
        let chosen=std::fs::read_to_string(path(owner,"reset")).or_else(|_|std::fs::read_to_string(path(owner,"class"))).ok().and_then(|s|s.parse().ok()).filter(|a|super::class_from_archetype(*a).is_some()).unwrap_or(if super::class_from_archetype(archetype).is_some(){archetype}else{0});
        let reset=resume.as_ref().is_none_or(|r| matches!(r.phase,Phase::Dead|Phase::Won)) || path(owner,"reset").exists();
        let paused=!reset&&path(owner,"paused").exists();
        if paused {if let Some(run)=&resume {super::run_screen::summary(run);}}
        let settled=resume.as_ref().is_some_and(|r|matches!(r.phase,Phase::Dead|Phase::Won));
        Ok(Self {active:true,paused,owner,room:None,clock:Instant::now(),warp:WarpGate::default(),reset,start:false,chosen,resume,seed:None,settled})
    }
    pub fn suspend(&mut self,run:Run) {self.active=true;self.paused=true;self.reset=false;self.start=false;self.resume=Some(run);self.seed=None;self.warp=WarpGate::default();self.clock=Instant::now();}
    pub fn finish(&mut self) { self.active=true;self.paused=false;self.settled=true;self.reset=true;self.start=false;self.resume=None;self.seed=None;self.warp=WarpGate::default(); }
    pub fn ready_for_party_restart(&self)->bool {self.active&&!self.reset&&!self.start&&matches!(self.warp,WarpGate::Ready)}
    pub fn settling(&self)->bool {self.active&&self.settled}
    pub fn view(&self)->super::rooms::RoomView {
        self.room.as_ref().map(|r|r.view.clone()).unwrap_or(super::rooms::RoomView{message:"正在返回大赐福并准备角色……".into(),loading:true,completed:false})
    }
    pub fn poll(&mut self,world:&mut WorldChrMan,game:&mut GameMan,blocked:Option<&str>)->Result<Option<Run>,String> {
        let party_ready=blocked.is_none();
        let player=world.main_player.as_ref().ok_or("等待角色加载")?;
        let guest=follows_host(game.event_world_type==eldenring::cs::EventWorldType::Remote&&super::seamless::active(),self.paused,self.settled);
        if guest&&(i32::from(player.current_block_id)!=BLOCK||game.warp_requested){return Ok(None);}
        if player.chr_ins.modules.data.hp<=0 || player.chr_ins.chr_flags1c5.death_flag() {return Ok(None);}
        if (self.reset || matches!(self.warp,WarpGate::Idle)) && (game.warp_requested || i32::from(player.current_block_id)==-1) {return Ok(None);}
        if self.reset && (!player.chr_ins.modules.physics.standing_on_solid_ground || player.chr_ins.chr_flags1c5.is_invincible()) {return Ok(None);}
        if self.reset {
            if path(self.owner,"paused").exists(){std::fs::remove_file(path(self.owner,"paused")).map_err(|e|e.to_string())?;}self.paused=false;
            std::fs::write(path(self.owner,"reset"),self.chosen.to_string()).map_err(|e|e.to_string())?;
            let player=world.main_player.as_mut().unwrap();
            super::blessings::Blessings::clear_run(&mut player.chr_ins);
            super::loadout::reset(unsafe {player.player_game_data.as_mut()},self.chosen)?;
            super::native_talk::set_flag(45000880,false)?;
            std::fs::write(path(self.owner,"class"),self.chosen.to_string()).map_err(|e|e.to_string())?;
            let save=path(self.owner,"json");if save.exists(){std::fs::remove_file(save).map_err(|e|e.to_string())?;}
            std::fs::remove_file(path(self.owner,"reset")).map_err(|e|e.to_string())?;
            self.resume=None;self.reset=false;
            if let Some(room)=self.room.take(){room.retire(world);}
            self.warp=WarpGate::default();
            super::log("Hub class reset complete before the single return warp");

        }
        let player=world.main_player.as_ref().unwrap();
        let block=i32::from(player.current_block_id);
        let was_loading=matches!(self.warp,WarpGate::Loading {..});
        let grounded=player.chr_ins.modules.physics.standing_on_solid_ground
                && !player.chr_ins.modules.physics.is_falling
                && !player.chr_ins.chr_flags1c5.is_invincible();
        let step=if guest {self.warp.follow(self.clock.elapsed(),block==BLOCK&&!game.warp_requested,grounded)}
            else {self.warp.poll(self.clock.elapsed(),block==BLOCK&&!game.warp_requested,grounded)};
        match step {
            WarpStep::Request=>{
                super::bloodstain::clear()?;
                super::log(format!("Hub warp requested once: from={block:#010x} target={BLOCK:#010x} entry=11102691"));
                game.move_map_target=BlockId::from(BLOCK);game.initial_area_entity_id=11102691;game.warp_requested=true;
                return Ok(None);
            },
            WarpStep::Wait=>return Ok(None),
            WarpStep::Failed=>{
                if was_loading {super::log(format!("Hub warp timed out: block={block:#010x} pending={}",game.warp_requested));}
                return Err("返回大赐福超时，已停止重复传送。请退出游戏后重新启动，并保留日志。".into());
            },
            WarpStep::Ready=>{self.settled=false;if was_loading {super::log("Hub warp completed; player grounded");}},
        }
        super::run_screen::hub_ready();
        if self.start && party_ready {
            self.active=false;self.start=false;
            if let Some(room)=self.room.take(){room.retire(world);}
            let class=super::class_from_archetype(self.chosen).unwrap_or(Class::Vagabond);
            let seed=self.seed.take().unwrap_or_else(||std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(42,|t|t.as_nanos() as u64));
            let player=world.main_player.as_mut().unwrap();
            player.chr_ins.modules.data.hp=player.chr_ins.modules.data.max_hp;
            player.chr_ins.modules.data.fp=player.chr_ins.modules.data.max_fp;
            let mut run=Run::new_personal(seed,class,super::party::random_entropy()?);run.enable_contracts();
            return Ok(Some(run));
        }
        if self.room.is_none() {
            let player=world.main_player.as_mut().unwrap();
            let class=super::class_from_archetype(self.chosen).unwrap();
            if self.resume.is_none(){super::flasks::Flasks::bind(unsafe {player.player_game_data.as_mut()},class)?;}
            self.room=Some(super::rooms::Room::hub(player.chr_ins.modules.data.hp,player.chr_ins.modules.data.fp)?);
        }
        let room=self.room.as_mut().unwrap();
        if let Some(seed)=super::run_screen::take_seed() {
            if seeded_start_allowed() {self.seed=Some(seed);self.reset=true;self.start=true;}else{room.notice(9500105)?;}
            return Ok(None);
        }
        if super::run_screen::active(){return Ok(None);}
        room.poll(world,game)?;
        if let Some((npc,choice))=room.hub_choice() {
            if !choice_allowed(npc,choice,party_ready) {
                let reason=blocked.unwrap_or("尚未收到队伍就绪确认，原因待核实。");
                room.view.message=reason.into();
                super::run_screen::party_notice(reason.into());
                super::log(format!("Hub start/resume blocked: {reason}"));
                return Ok(None);
            }
            match (npc,choice) {
                (0,1)=>{self.seed=None;self.reset=true;self.start=true;},
                (0,3)=>{if seeded_start_allowed(){super::run_screen::seed_entry();}else{room.notice(9500105)?;}},
                (0,2)=>{
                    if let Some(mut run)=self.resume.take().filter(|r|!matches!(r.phase,Phase::Dead|Phase::Won)) {
                        run.resume_from_hub()?;
                        if path(self.owner,"paused").exists(){std::fs::remove_file(path(self.owner,"paused")).map_err(|e|e.to_string())?;}self.paused=false;
                        room.retire(world);self.room=None;self.active=false;
                        let player=world.main_player.as_mut().unwrap();
                        player.chr_ins.modules.data.hp=(i64::from(player.chr_ins.modules.data.max_hp)*i64::from(run.hp)/i64::from(run.max_hp.max(1))).clamp(1,i64::from(player.chr_ins.modules.data.max_hp)) as i32;
                        if let Some(fp)=std::fs::read_to_string(path(self.owner,"fp")).ok().and_then(|s|s.parse::<i32>().ok()) {player.chr_ins.modules.data.fp=fp.clamp(0,player.chr_ins.modules.data.max_fp);}
                        return Ok(Some(run));
                    }
                    room.notice(9500030)?;
                },
                (1,1..=10)=>{self.chosen=[0,4,5,7,1,2,3,8,6,9][choice as usize-1];self.reset=true;},
                _=>{},
            }
        }
        Ok(None)
    }
}
