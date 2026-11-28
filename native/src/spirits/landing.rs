use std::{collections::{HashMap,HashSet},sync::{LazyLock,Mutex},time::{Duration,Instant}};
use eldenring::cs::WorldChrMan;

struct Sample {
    radius:f32,y:f32,player_y:f32,alive:bool,remote:bool,dismissing:bool,
    player_grounded:bool,grounded:bool,proxy_pending:bool,animation:Option<i32>,
}
fn initial_height(s:&Sample)->Option<f32> {
    if !s.alive||s.remote||s.dismissing||!s.player_grounded||s.grounded||!s.proxy_pending
        ||s.animation.is_some_and(|id|id>=0)||!s.radius.is_finite()||!(1.25..=20.0).contains(&s.radius)
        ||!s.y.is_finite()||!s.player_y.is_finite()||(s.y-s.player_y).abs()>0.75 {return None;}
    Some(s.y+s.radius+0.25)
}
#[derive(Default)]
struct Guard {scope:Option<(usize,i32)>,seen:HashSet<u64>}
impl Guard {
    fn begin(&mut self,scope:(usize,i32),present:&HashSet<u64>) {
        if self.scope!=Some(scope){self.seen.clear();self.scope=Some(scope);}
        self.seen.retain(|id|present.contains(id));
    }
    fn observe(&mut self,key:u64,s:&Sample)->Option<f32> {if self.seen.insert(key){initial_height(s)}else{None}}
}
static GUARD:LazyLock<Mutex<Guard>>=LazyLock::new(||Mutex::new(Guard::default()));
static PENDING:LazyLock<Mutex<HashMap<u64,(Instant,[f32;3])>>>=LazyLock::new(||Mutex::new(HashMap::new()));
fn recovery_target(start:[f32;3],current:[f32;3],s:&Sample,elapsed:Duration)->Option<[f32;3]> {
    if elapsed>Duration::from_secs(2)||!s.alive||s.remote||s.dismissing||!s.player_grounded||s.grounded
        ||s.proxy_pending||s.animation.is_none_or(|id|id<0)||!current.iter().all(|v|v.is_finite())
        ||(start[1]-s.player_y).abs()>0.75||start[1]-current[1]<s.radius*0.75{return None;}
    Some([start[0],start[1]+s.radius+0.25,start[2]])
}
pub(crate) fn move_native(physics:&mut eldenring::cs::CSChrPhysicsModule,target:[f32;3])->Result<(),String> {
    static MOVE:std::sync::OnceLock<Result<usize,String>>=std::sync::OnceLock::new();
    let address=MOVE.get_or_init(||{
        crate::items::checked_function(0x45c99e,&[0x66,0xc7,0x83,0x90,0,0,0,0x01,0x01])?;
        crate::items::checked_function(0x45c910,&[0x40,0x53,0x48,0x83,0xec,0x60])
    }).as_ref().map_err(Clone::clone)?;
    #[repr(C,align(16))]
    struct Delta([f32;4]);
    let delta=Delta([target[0]-physics.position.0,target[1]-physics.position.1,target[2]-physics.position.2,0.0]);
    // Verified game 2.7.2.0 native translation sets both physics update flags and updates proxy state.
    let translate:unsafe extern "C" fn(*mut eldenring::cs::CSChrPhysicsModule,*const Delta,bool)=unsafe {std::mem::transmute(*address)};
    unsafe {translate(physics,&delta,false);}
    Ok(())
}

pub(crate) fn tick(world:&mut WorldChrMan,warping:bool) {
    if warping{if let Ok(mut pending)=PENDING.try_lock(){pending.clear();}return;}
    let Some(player)=world.main_player.as_ref() else{return};
    let owner=unsafe {player.player_game_data.as_ref()}.character_event_id as i32;
    let p=&player.chr_ins.modules.physics;
    let player_y=p.position.1;let player_grounded=p.standing_on_solid_ground&&!p.is_falling&&player.chr_ins.modules.data.hp>0&&!player.chr_ins.chr_flags1c5.death_flag();
    let manager=&world.summon_buddy_manager;
    let scope=(std::ptr::from_ref(&**manager) as usize,owner);
    let samples:Vec<_>=manager.groups.iter().filter(|pair|pair.first==owner).flat_map(|pair|pair.second.iter()).filter_map(|group|{
        let offset=group.buddy_param_id.checked_sub(1600000000)?;
        if offset<0||offset/40000>=erdtree_tower::rewards::CUSTOM_SPIRITS.len() as i32||offset%1000>=3{return None;}
        let chr=super::live_chr(group)?;let physics=&chr.modules.physics;
        let handle=chr.field_ins_handle;let key=(u64::from(handle.block_id.0 as u32)<<32)|u64::from(handle.selector.0);
        let time=&chr.modules.time_act;
        Some((key,handle,[physics.position.0,physics.position.1,physics.position.2],Sample{radius:physics.hit_radius.max(physics.chr_hit_radius),y:physics.position.1,player_y,
            alive:chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag(),remote:group.is_remote,
            dismissing:group.disappear_requested||manager.disappear_requested,player_grounded,
            grounded:physics.standing_on_solid_ground||physics.is_touching_ground,
            proxy_pending:physics.chr_proxy_pos_update_requested,animation:time.anim_queue.get(time.read_idx as usize).map(|a|a.anim_id)}))
    }).collect();
    let Ok(mut guard)=GUARD.try_lock() else{return};
    let Ok(mut pending)=PENDING.try_lock() else{return};
    if guard.scope!=Some(scope){pending.clear();}
    let present=samples.iter().map(|(key,_,_,_)|*key).collect();
    guard.begin(scope,&present);
    pending.retain(|key,(at,_)|present.contains(key)&&at.elapsed()<=Duration::from_secs(2));
    for (key,handle,position,sample) in samples {
        if guard.observe(key,&sample).is_some(){pending.insert(key,(Instant::now(),position));continue;}
        let Some((at,start))=pending.get(&key) else{continue;};
        if !sample.alive||sample.dismissing{pending.remove(&key);continue;}
        let Some(target)=recovery_target(*start,position,&sample,at.elapsed()) else{continue;};
        pending.remove(&key);
        let Some(chr)=world.chr_ins_by_handle_mut(&handle) else{continue;};
        match move_native(&mut chr.modules.physics,target) {
            Ok(())=>crate::log(format!("ASH_SPAWN_RECOVER handle={handle} npc={} before={position:.3?} target={target:.3?}; native translation after initialization",chr.npc_param_id)),
            Err(error)=>crate::log(format!("ASH_SPAWN_RECOVER skipped: {error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample()->Sample {Sample{radius:12.0,y:1.631,player_y:1.635,alive:true,remote:false,dismissing:false,player_grounded:true,grounded:false,proxy_pending:true,animation:Some(-1)}}
    #[test]
    fn recovery_waits_for_initialization_and_rejects_late_or_normal_falls() {
        let start=[5.584,1.631,22.124];let fallen=[5.584,-10.670,22.124];
        let ready=Sample{proxy_pending:false,animation:Some(7000),..sample()};
        assert_eq!(recovery_target(start,fallen,&sample(),Duration::from_millis(500)),None);
        let target=recovery_target(start,fallen,&ready,Duration::from_millis(500)).unwrap();
        assert_eq!(target[0],start[0]);assert_eq!(target[2],start[2]);assert!(target[1]>start[1]);
        assert_eq!(recovery_target(start,fallen,&ready,Duration::from_secs(3)),None);
        assert_eq!(recovery_target(start,start,&ready,Duration::from_millis(500)),None);
        assert_eq!(recovery_target(start,fallen,&Sample{grounded:true,..ready},Duration::from_millis(500)),None);
    }
    #[test]
    fn observed_large_spawn_is_lifted_once_without_height_lock() {
        let mut guard=Guard::default();guard.begin((1,2),&HashSet::from([3]));
        assert!((guard.observe(3,&sample()).unwrap()-13.881).abs()<0.00001);
        assert_eq!(guard.observe(3,&sample()),None);
        guard.begin((1,2),&HashSet::from([3]));assert_eq!(guard.observe(3,&sample()),None);
        guard.begin((1,2),&HashSet::new());guard.begin((1,2),&HashSet::from([3]));assert!(guard.observe(3,&sample()).is_some());
    }
    #[test]
    fn normal_remote_dead_falling_and_active_animations_are_untouched() {
        for s in [Sample{radius:0.4,..sample()},Sample{remote:true,..sample()},Sample{alive:false,..sample()},
            Sample{dismissing:true,..sample()},Sample{player_grounded:false,..sample()},Sample{grounded:true,..sample()},
            Sample{proxy_pending:false,..sample()},Sample{animation:Some(7000),..sample()},Sample{y:-10.0,..sample()},Sample{radius:f32::NAN,..sample()}] {
            assert_eq!(initial_height(&s),None);
        }
    }
    #[test]
    fn previously_seen_spirit_cannot_be_lifted_during_later_warp() {
        let mut guard=Guard::default();guard.begin((1,2),&HashSet::from([3]));
        assert_eq!(guard.observe(3,&Sample{grounded:true,..sample()}),None);
        assert_eq!(guard.observe(3,&sample()),None);
        guard.begin((2,2),&HashSet::from([3]));assert!(guard.observe(3,&sample()).is_some());
    }
}
