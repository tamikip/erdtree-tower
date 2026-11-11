use std::time::{Duration,Instant};
use eldenring::cs::{CSSessionManager,CSNowLoadingHelper,FieldArea,GameMan,WorldChrMan};
use fromsoftware_shared::FromStatic;

#[derive(Default)]
pub struct Trace {
    departure:Option<(usize,Instant)>,last:Option<Instant>,signature:String,finished:bool,
    fade_handled:bool,
}
impl Trace {
    fn observe(&mut self,now:Instant,departure:Option<(usize,Instant)>,signature:&str,grounded:bool)->Option<String> {
        if departure.is_none() {
            let old=self.departure.take()?;
            if self.finished{return None;}
            self.finished=true;
            return Some(format!("LANDING_PROBE node={} elapsed_ms={} end=cancelled",old.0,now.saturating_duration_since(old.1).as_millis()));
        }
        if self.departure!=departure {*self=Self{departure,..Default::default()};}
        if self.finished{return None;}
        let (node,start)=departure?;let elapsed=now.saturating_duration_since(start);
        let end=if grounded {"grounded"}else if elapsed>=Duration::from_secs(120){"window_limit"}else{"false"};
        if end=="false"&&self.signature==signature&&self.last.is_some_and(|last|now.saturating_duration_since(last)<Duration::from_secs(1)){return None;}
        self.last=Some(now);self.signature=signature.into();self.finished=end!="false";
        Some(format!("LANDING_PROBE node={node} elapsed_ms={} end={end} {signature}",elapsed.as_millis()))
    }
    pub fn tick(&mut self,now:Instant,departure:Option<(usize,Instant)>,world:&WorldChrMan,game:&GameMan,host:Option<bool>) {
        if departure.is_none()||self.departure==departure&&self.finished {
            if let Some(line)=self.observe(now,departure,"",false){crate::log(line);}
            return;
        }
        let (signature,grounded)=snapshot(world,game,host);
        if let Some(line)=self.observe(now,departure,&signature,grounded){crate::log(line);}
        if !self.finished&&!self.fade_handled {
            match finish_guest_fade(world,game,host) {
                Ok(Some(elapsed))=>{self.fade_handled=true;crate::log(format!("DRAGONLORD_FADE immediate requested elapsed={elapsed:.3}s; native loading checks retained"));}
                Ok(None)=>{}
                Err(error)=>{self.fade_handled=true;crate::log(format!("DRAGONLORD_FADE skipped: {error}"));}
            }
        }
    }
}
fn snapshot(world:&WorldChrMan,game:&GameMan,host:Option<bool>)->(String,bool) {
    let Some(player)=world.main_player.as_ref() else{return (format!("host={host:?} warp={} player=unloaded",game.warp_requested),false);};
    let chr=&player.chr_ins;let physics=&chr.modules.physics;
    let block=i32::from(player.current_block_id);
    let pos=[physics.position.0,physics.position.1,physics.position.2];
    let block_pos=[player.block_position.x,player.block_position.y,player.block_position.z];
    let origin:[f32;3]=std::array::from_fn(|i|pos[i]-block_pos[i]);
    let center=if block==0x0d000000 {
        unsafe {FieldArea::instance()}.ok().and_then(|area|{
            let info=&area.world_info_owner.world_res.world_info;
            if info.world_block_info_count>192 {return None;}
            info.world_block_info_by_map(&player.current_block_id).map(|b|{
                let p=&b.physics_center;[p.0,p.1,p.2]
            })
        })
    }else{None};
    let session=host.and_then(|_|unsafe {CSSessionManager::instance()}.ok()).map(|s|{
        let local=s.players.iter().find(|p|p.is_local_player).map(|p|(p.join_wait,p.rebreak_in,p.disable_emk_sync));
        let warp=s.stay_in_multiplay_area_warp_data.as_ptr();
        let warp=unsafe {warp.as_ref()}.map(|w|(w.warp_request_delay,w.disable_multiplay_restriction,w.is_warp_possible,i32::from(w.saved_block_id)));
        format!("lobby={:?} protocol={:?} local_join_reentry_emk={local:?} area_warp={warp:?}",s.lobby_state,s.protocol_state)
    });
    let counter=unsafe {CSNowLoadingHelper::instance()}.ok().map(|h|h.menu_load_counter);
    let proxy=chr.chr_ctrl.chr_proxy_flags;
    let grounded=block==0x0d000000&&!game.warp_requested&&chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag()&&physics.standing_on_solid_ground&&!physics.is_falling;
    (format!("host={host:?} warp={} block={block:08x} chr_block={:08x} origin_block={:08x} origin_override={:08x} block_center={center:?} observed_origin={origin:.3?} position={pos:.3?} block_position={block_pos:.3?} grounded={grounded} proxy_update={} ctrl_position_sync={} ctrl_rotation_sync={} load={:?} backread={} omission={:?} alpha={:.3} invincible={} enter_anim_disabled={} load_counter={counter:?} session={session:?}",
        game.warp_requested,i32::from(chr.block_id),i32::from(chr.block_origin),i32::from(chr.block_origin_override),physics.chr_proxy_pos_update_requested,
        proxy.position_sync_requested(),proxy.rotation_sync_requested(),chr.load_state,chr.backread_state,chr.omission_mode,chr.tint_alpha_multiplier,
        chr.chr_flags1c5.is_invincible(),game.disable_map_enter_anim),grounded)
}

fn can_finish_fade(guest_reloading:bool,block:i32,stage:i32,elapsed:f32,immediate:u8,queued:i16)->bool {
    guest_reloading&&block==0x0d000000&&stage==3&&(0.0..15.0).contains(&elapsed)&&immediate==0&&queued==-1
}
fn finish_guest_fade(world:&WorldChrMan,game:&GameMan,host:Option<bool>)->Result<Option<f32>,String> {
    use eldenring::cs::{CSWorldGeomMan,CSWorldGeomManBlockData,LobbyState,ProtocolState};
    if host!=Some(false)||game.warp_requested{return Ok(None);}
    let Some(player)=world.main_player.as_ref().filter(|p|i32::from(p.current_block_id)==0x0d000000&&p.chr_ins.modules.data.hp>0) else{return Ok(None);};
    let Ok(session)= (unsafe {CSSessionManager::instance()}) else{return Ok(None);};
    if session.lobby_state!=LobbyState::Client||session.protocol_state!=ProtocolState::WaitReload2{return Ok(None);}
    let Ok(geometry)= (unsafe {CSWorldGeomMan::instance_mut()}) else{return Ok(None);};
    let Some(block)=geometry.geom_block_data_by_id_mut(&player.current_block_id) else{return Ok(None);};
    let block_id=i32::from(block.block_id);
    let ptr=(block as *mut CSWorldGeomManBlockData).cast::<u8>();
    // Verified 2.7.2.0: stage 3 fades geometry for 15 s; +0x235 requests native immediate completion.
    super::items::checked_function(0x6b22c5,&[0x8b,0x89,0x30,0x02,0x00,0x00])?;
    super::items::checked_function(0x6b27be,&[0x45,0x38,0xbe,0x35,0x02,0x00,0x00,0x75,0x08,0x41,0x0f,0x2e,0xc8,0x7a,0x0b,0x75,0x09,0xf3,0x45,0x0f,0x11,0x8e,0x40,0x02,0x00,0x00])?;
    super::items::checked_function(0x6b242c,&[0x45,0x88,0xbe,0x35,0x02,0x00,0x00])?;
    unsafe {
        let stage=ptr.add(0x230).cast::<i32>().read();
        let elapsed=ptr.add(0x240).cast::<f32>().read();
        let immediate=ptr.add(0x235).read();
        let queued=ptr.add(0x23e).cast::<i16>().read();
        if !can_finish_fade(true,block_id,stage,elapsed,immediate,queued){return Ok(None);}
        ptr.add(0x235).write(1);
        Ok(Some(elapsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration,Instant};
    #[test]
    fn only_dragonlord_guest_reload_can_finish_the_active_fade() {
        assert!(can_finish_fade(true,0x0d000000,3,0.25,0,-1));
        assert!(!can_finish_fade(false,0x0d000000,3,0.25,0,-1));
        assert!(!can_finish_fade(true,0x2d000000,3,0.25,0,-1));
        for stage in [0,1,2,4,5] {assert!(!can_finish_fade(true,0x0d000000,stage,0.25,0,-1));}
        for elapsed in [-1.0,15.0,f32::NAN,f32::INFINITY] {assert!(!can_finish_fade(true,0x0d000000,3,elapsed,0,-1));}
        assert!(!can_finish_fade(true,0x0d000000,3,0.25,1,-1));
        assert!(!can_finish_fade(true,0x0d000000,3,0.25,0,1));
    }
    #[test]
    fn stalled_guest_emits_heartbeats_and_finishes_on_landing() {
        let start=Instant::now();let key=Some((1,start));let mut trace=Trace::default();
        assert!(trace.observe(start,key,"unloaded",false).unwrap().contains("elapsed_ms=0"));
        assert!(trace.observe(start+Duration::from_millis(250),key,"unloaded",false).is_none());
        assert!(trace.observe(start+Duration::from_secs(1),key,"unloaded",false).is_some());
        assert!(trace.observe(start+Duration::from_secs(5),key,"proxy=true origin=[0,0,0]",false).is_some());
        let end=trace.observe(start+Duration::from_secs(22),key,"proxy=false origin=[-16,-1008,-296]",true).unwrap();
        assert!(end.contains("elapsed_ms=22000")&&end.contains("end=grounded"));
        assert!(trace.observe(start+Duration::from_secs(23),key,"moved",true).is_none());
    }
    #[test]
    fn new_attempt_rearms_even_when_the_same_node_is_retried() {
        let start=Instant::now();let mut trace=Trace::default();
        trace.observe(start,Some((1,start)),"grounded",true).unwrap();
        let retry=start+Duration::from_secs(30);
        let line=trace.observe(retry,Some((1,retry)),"unloaded",false).unwrap();
        assert!(line.contains("elapsed_ms=0")&&line.contains("end=false"));
    }
    #[test]
    fn cancellation_and_time_limit_close_the_trace_once() {
        let start=Instant::now();let mut trace=Trace::default();let key=Some((1,start));
        assert!(trace.observe(start,None,"idle",false).is_none());
        trace.observe(start,key,"unloaded",false).unwrap();
        assert!(trace.observe(start+Duration::from_secs(2),None,"",false).unwrap().contains("end=cancelled"));
        assert!(trace.observe(start+Duration::from_secs(3),None,"",false).is_none());
        trace.observe(start,key,"unloaded",false).unwrap();
        assert!(trace.observe(start+Duration::from_secs(120),key,"unloaded",false).unwrap().contains("end=window_limit"));
        assert!(trace.observe(start+Duration::from_secs(121),key,"changed",false).is_none());
    }
    #[test]
    fn state_changes_are_logged_before_the_heartbeat() {
        let start=Instant::now();let mut trace=Trace::default();let key=Some((1,start));
        trace.observe(start,key,"no_player",false).unwrap();
        assert!(trace.observe(start+Duration::from_millis(250),key,"map_assigned",false).is_some());
        assert!(trace.observe(start+Duration::from_millis(500),key,"map_assigned",false).is_none());
    }
}
