use eldenring::{cs::{BlockId,ChrInsExt,TalkScript,WorldChrMan},ez_state::EzStateValue};
use fromsoftware_shared::FromStatic;
use std::{ffi::c_void,sync::{Mutex,atomic::{AtomicBool,AtomicUsize,Ordering}}};
use hudhook::mh::{MhHook,MH_EnableHook};
mod seamless_hook;
pub(crate) mod refund;

static ACTIVE:AtomicBool=AtomicBool::new(false);
static ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static USE_STATE:AtomicUsize=AtomicUsize::new(0);
static CLEAR:AtomicUsize=AtomicUsize::new(0);
static REGISTER:AtomicUsize=AtomicUsize::new(0);
static BOUND:AtomicUsize=AtomicUsize::new(0);
pub fn begin_frame(){ACTIVE.store(false,Ordering::Release);BOUND.store(0,Ordering::Release);TWIN_ARMED.store(false,Ordering::Release);}
pub fn set_active(active:bool){ACTIVE.store(active,Ordering::Release);if !active {BOUND.store(0,Ordering::Release);reset_twin();}}
/// 双生的灵唤铃（余烬，单人与联机）：第一只召唤物在场时重新开放召唤点，再召唤一次。房主走 Pending 的召唤点、成员走 party_spirits，都经过 Spirits::poll。
/// 原版只记一个“当前召唤”，场上有召唤物时不让用骨灰，所以在“使用前检查”的阶段把“有存活召唤物”暂时清掉（ARMED）。
/// 第二次请求接受后，仅跳过更新函数中新召唤替换旧组的那一次清除。
static TWIN:AtomicBool=AtomicBool::new(false);
static TWIN_ARMED:AtomicBool=AtomicBool::new(false);
static SECOND_ACCEPTED:AtomicBool=AtomicBool::new(false);
static REPLACE_ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static REPLACE_CALLER:AtomicUsize=AtomicUsize::new(0);
static REPLACE_READY:AtomicBool=AtomicBool::new(false);
static REPLACEMENT:Mutex<Option<(usize,i32)>>=Mutex::new(None);
fn reset_twin() {
    TWIN_ARMED.store(false,Ordering::Release);SECOND_ACCEPTED.store(false,Ordering::Release);
    if let Ok(mut gate)=REPLACEMENT.lock(){*gate=None;}
}
fn preserve_replacement(ticket:Option<(usize,i32)>,manager:usize,effect:i32,at_replacement:bool,enabled:bool,cancel:bool)->bool {
    at_replacement&&enabled&&!cancel&&ticket==Some((manager,effect))
}
#[unsafe(naked)]
unsafe extern "C" fn replacement_entry(_manager:*mut u8,_owner:i32) {
    core::arch::naked_asm!("mov r8, [rsp]", "jmp {}",sym replacement_clear);
}
unsafe extern "C" fn replacement_clear(manager:*mut u8,owner:i32,caller:usize) {
    let original:unsafe extern "C" fn(*mut u8,i32)=unsafe {std::mem::transmute(REPLACE_ORIGINAL.load(Ordering::Acquire))};
    let expected=REPLACE_CALLER.load(Ordering::Acquire);
    if ACTIVE.load(Ordering::Acquire)&&TWIN.load(Ordering::Acquire) {
        let buddy=unsafe {&*manager.cast::<eldenring::cs::SummonBuddyManager>()};
        let ticket=REPLACEMENT.lock().ok().and_then(|g|*g);
        super::log(format!("TOWER_TWIN clear audit: caller_match={} owner={owner} pending={} cancel={} second={} bound={} ticket={ticket:?}",caller==expected,buddy.request_summon_speffect_id,buddy.disappear_requested,SECOND_ACCEPTED.load(Ordering::Acquire),BOUND.load(Ordering::Acquire)==manager as usize));
    }
    if expected!=0&&caller==expected&&ACTIVE.load(Ordering::Acquire)&&TWIN.load(Ordering::Acquire)
        &&SECOND_ACCEPTED.load(Ordering::Acquire)&&BOUND.load(Ordering::Acquire)==manager as usize {
        let buddy=unsafe {&*manager.cast::<eldenring::cs::SummonBuddyManager>()};
        let living_player=unsafe {eldenring::cs::PlayerIns::local_player()}.is_ok_and(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()&&unsafe {p.player_game_data.as_ref()}.character_event_id as i32==owner&&super::arenas::for_block(i32::from(p.current_block_id)).is_some());
        let mut preserved=false;
        if let Ok(gate)=REPLACEMENT.lock() {
            preserved=preserve_replacement(*gate,manager as usize,buddy.request_summon_speffect_id,true,living_player,buddy.disappear_requested);
        }
        if preserved {
            super::log(format!("TOWER_TWIN preserved first group: owner={owner} pending={} caller=4b8a95",buddy.request_summon_speffect_id));return;
        }
    }
    unsafe {original(manager,owner);}
}
/// 第二只的召唤请求被原版拒绝时记下骨灰编号：扣骨灰与发召唤请求在使用动画里同时发生，被拒就会白扣一个，由运行时补回
/// 召唤师「群灵」：自己的召唤物出场后全部阵亡（或消失），重新开放召唤点，可以再召唤新的（骨灰照常消耗，胜利时还活着照样返还）
static RESUMMON:AtomicBool=AtomicBool::new(false);
pub fn set_resummon(on:bool){if RESUMMON.swap(on,Ordering::AcqRel)!=on {super::log(format!("TOWER_RESUMMON enabled={on}"));}}
/// 召唤点已用过、这一轮召唤的召唤物出场过、现在场上一只都不剩
fn resummon_allowed(enabled:bool,used:bool,seen:bool,buddies:usize)->bool {enabled&&used&&seen&&buddies==0}
static TWIN_REFUND:std::sync::atomic::AtomicI32=std::sync::atomic::AtomicI32::new(0);
pub fn take_twin_refund()->Option<u32> {let id=TWIN_REFUND.swap(0,Ordering::AcqRel);(id>0).then_some(id as u32&0x0fffffff)}
pub fn set_twin(on:bool){let on=on&&REPLACE_READY.load(Ordering::Acquire);if TWIN.swap(on,Ordering::AcqRel)!=on {super::log(format!("TOWER_TWIN enabled={on}"));if !on{reset_twin();}}}
fn refresh_allowed(active:bool,bound:usize,manager:usize,block:i32,alive:bool,used:bool)->bool {
    active&&bound!=0&&bound==manager&&eligible(block,alive,used)
}
fn second_use_allowed(armed:bool,spent:bool,pending:i32,cooldown:f32,in_range:bool,cancel:bool)->bool {
    armed&&!spent&&pending<0&&cooldown<=0.0&&in_range&&!cancel
}
unsafe extern "C" fn use_state(manager:*mut u8,goods:i32)->i32 {
    let original:unsafe extern "C" fn(*mut u8,i32)->i32=unsafe {std::mem::transmute(USE_STATE.load(Ordering::Acquire))};
    let state=unsafe {original(manager,goods)};
    if ACTIVE.load(Ordering::Acquire)&&TWIN.load(Ordering::Acquire)&&BOUND.load(Ordering::Acquire)==manager as usize {
        let buddy=unsafe {&*manager.cast::<eldenring::cs::SummonBuddyManager>()};
        let range=buddy.is_within_activation_range&&buddy.buddy_stone_entity_id!=0;
        if second_use_allowed(TWIN_ARMED.load(Ordering::Acquire),SECOND_ACCEPTED.load(Ordering::Acquire),buddy.request_summon_speffect_id,buddy.item_use_cooldown_timer,range,buddy.disappear_requested)
            &&super::native_talk::flag(SUMMONED_FLAG)==Ok(false)
            &&(goods==-1||erdtree_tower::rewards::spirit_level(0x40000000|goods as u32).is_some())
            &&unsafe {eldenring::cs::PlayerIns::local_player()}.is_ok_and(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()&&super::arenas::for_block(i32::from(p.current_block_id)).is_some()) {
            return 0;
        }
    }
    state
}
unsafe extern "C" fn clear_frame(manager:*mut u8) {
    let original:unsafe extern "C" fn(*mut u8)=unsafe {std::mem::transmute(CLEAR.load(Ordering::Acquire))};
    unsafe {original(manager);}
    if !ACTIVE.load(Ordering::Acquire)||BOUND.load(Ordering::Acquire)!=manager as usize {return;}
    // 双生的灵唤铃：第一只还在场时，让骨灰在这一阶段的使用检查里可用
    if TWIN_ARMED.load(Ordering::Acquire) {unsafe {(*manager.cast::<eldenring::cs::SummonBuddyManager>()).player_has_alive_summon=false;}}
    let Ok(world)=(unsafe {WorldChrMan::instance_mut()}) else {return;};
    let Some(player)=world.main_player.as_mut() else {return;};
    let used=super::native_talk::flag(SUMMONED_FLAG).unwrap_or(true);
    if !refresh_allowed(ACTIVE.load(Ordering::Acquire),BOUND.load(Ordering::Acquire),manager as usize,i32::from(player.current_block_id),player.chr_ins.modules.data.hp>0&&!player.chr_ins.chr_flags1c5.death_flag(),used){return;}
    // TalkMan clears the point before character-use checks; restore in that same phase.
    for effect in [9530,84] {player.chr_ins.apply_speffect(effect,false);}
    let register:unsafe extern "C" fn(*mut u8,u32)=unsafe {std::mem::transmute(REGISTER.load(Ordering::Acquire))};
    let Some(arena)=super::arenas::for_block(i32::from(player.current_block_id)) else {return;};
    unsafe {register(manager,arena.stone);}
}
unsafe extern "C" fn request(manager:*mut u8,effect:i32)->bool {
    let original:unsafe extern "C" fn(*mut u8,i32)->bool=unsafe {std::mem::transmute(ORIGINAL.load(Ordering::Acquire))};
    if !ACTIVE.load(Ordering::Acquire){return unsafe {original(manager,effect)};}
    let query:unsafe extern "C" fn(*mut u8,i32)->i32=unsafe {std::mem::transmute(USE_STATE.load(Ordering::Acquire))};
    let state=unsafe {query(manager,-1)};
    let field=|offset|unsafe {manager.add(offset).cast::<i32>().read()};
    super::log(format!("Spirit use request: effect={effect} state={state} stone={} active_stone={} goods={} active_goods={} twin_armed={}",field(0x38),field(0x3c),field(0xe0),field(0xe4),TWIN_ARMED.load(Ordering::Acquire)));
    let twin=TWIN_ARMED.load(Ordering::Acquire)&&BOUND.load(Ordering::Acquire)==manager as usize;let goods=field(0xe0);
    let usage=(BOUND.load(Ordering::Acquire)==manager as usize).then(||refund::capture(unsafe {&*manager.cast::<eldenring::cs::SummonBuddyManager>()},goods,effect)).flatten();
    if let Ok(mut gate)=REPLACEMENT.lock(){*gate=None;}
    let accepted=unsafe {original(manager,effect)};
    if accepted&&let Some(usage)=usage {refund::accepted(usage);}
    TWIN_ARMED.store(false,Ordering::Release);
    if twin&&accepted {
        SECOND_ACCEPTED.store(true,Ordering::Release);
        if let Ok(mut gate)=REPLACEMENT.lock(){*gate=Some((manager as usize,effect));}
    }
    super::log(format!("Spirit use result: accepted={accepted} pending={} active={} stone={}",field(0x20),field(0x24),field(0x3c)));
    if twin&&!accepted&&goods>0 {TWIN_REFUND.store(goods,Ordering::Release);super::log(format!("TOWER_TWIN second summon rejected; ash {goods} will be returned"));}
    accepted
}
pub unsafe fn install()->Result<(),String> {
    let register=super::items::checked_function(0x4b8130,&[0x48,0x8b,0xc4,0x89,0x50,0x10,0x57,0x48,0x83,0xec,0x60])?;
    let clear=super::items::checked_function(0x4b6f60,&[0x48,0x89,0x5c,0x24,0x08,0x48,0x89,0x74,0x24,0x10,0x57,0x48,0x83,0xec,0x20])? as *mut c_void;
    let phase=unsafe {MhHook::new(clear,clear_frame as *mut c_void)}.map_err(|e|format!("Spirit phase hook: {e:?}"))?;
    REGISTER.store(register,Ordering::Release);
    CLEAR.store(phase.trampoline() as usize,Ordering::Release);
    unsafe {MH_EnableHook(clear)}.ok().map_err(|e|format!("Spirit phase enable: {e:?}"))?;
    super::log("Spirit summon-point refresh installed");
    match unsafe {install_trace()} {
        Ok(())=>if let Err(error)=unsafe {install_replacement_guard()} {super::log(format!("Twin replacement guard unavailable; second summon disabled: {error}"));},
        Err(error)=>super::log(format!("Spirit usage tracing skipped; summon-point refresh remains active: {error}")),
    }
    Ok(())
}
unsafe fn install_replacement_guard()->Result<(),String> {
    let caller=super::items::checked_function(0x4b8a90,&[0xe8,0xeb,0x2f,0,0])?+5;
    let address=super::items::checked_function(0x4bba80,&[0x89,0x54,0x24,0x10,0x55,0x57,0x48,0x83,0xec,0x28,0x48,0x8b,0x41,0x78])? as *mut c_void;
    let hook=unsafe {MhHook::new(address,replacement_entry as *mut c_void)}.map_err(|e|format!("Twin replacement hook: {e:?}"))?;
    REPLACE_ORIGINAL.store(hook.trampoline() as usize,Ordering::Release);REPLACE_CALLER.store(caller,Ordering::Release);
    unsafe {MH_EnableHook(address)}.ok().map_err(|e|format!("Twin replacement hook enable: {e:?}"))?;
    REPLACE_READY.store(true,Ordering::Release);super::log("Twin replacement guard installed: 4b8a90 -> 4bba80");Ok(())
}

unsafe fn install_trace()->Result<(),String> {
    let expected=&[0x48,0x89,0x5c,0x24,0x08,0x48,0x89,0x74,0x24,0x10,0x57,0x48,0x83,0xec,0x20];
    let query=unsafe {seamless_hook::checked_function(0x4b73d0,expected)}?;
    let address=unsafe {seamless_hook::checked_function(0x4b8690,expected)}? as *mut c_void;
    let query_hook=unsafe {MhHook::new(query as *mut c_void,use_state as *mut c_void)}.map_err(|e|format!("Twin use-state hook: {e:?}"))?;
    let hook=unsafe {MhHook::new(address,request as *mut c_void)}.map_err(|e|format!("Spirit request trace: {e:?}"))?;
    USE_STATE.store(query_hook.trampoline() as usize,Ordering::Release);
    ORIGINAL.store(hook.trampoline() as usize,Ordering::Release);
    unsafe {MH_EnableHook(query as *mut c_void)}.ok().map_err(|e|format!("Twin use-state enable: {e:?}"))?;
    if let Err(error)=unsafe {MH_EnableHook(address)}.ok() {
        let rollback=unsafe {hudhook::mh::MH_DisableHook(query as *mut c_void)};
        return Err(format!("Spirit request trace enable: {error:?}; use-state rollback: {rollback:?}"));
    }
    super::log("Twin use-state check installed: 4b73d0");
    super::log("Spirit request trace installed: 4b8690");
    Ok(())
}

pub const SUMMONED_FLAG:u32=45020968;
#[derive(Default)]
pub struct Spirits {script:Option<TalkScript>,reported:bool,failed:bool,status:Option<(u32,bool,bool,bool)>,registration:Option<u32>,
    /// 双生的灵唤铃：这场战斗已经重新开放过召唤点；上一次记录的存活召唤物数量
    twin_reopened:bool,buddies:Option<usize>,
    /// 召唤师再召唤：这一轮召唤的召唤物已经出场过（出场前召唤物数为 0，不能当成阵亡）
    spirit_seen:bool}
// Accessed only from the serialized game task, like the native NPC dialogs.
unsafe impl Send for Spirits {}

/// 第二只还没召：已经重新开放、召唤点没再用掉、场上有第一只
fn twin_armed(twin:bool,reopened:bool,used:bool,buddies:usize)->bool {twin&&reopened&&!used&&buddies>0}
fn eligible(block:i32,combat:bool,used:bool)->bool {super::arenas::for_block(block).is_some()&&combat&&!used}
impl Spirits {
    pub fn poll(&mut self,world:&WorldChrMan,block:i32,combat:bool)->Result<(),String> {
        if self.failed||!eligible(block,combat,false) {return Ok(());}
        let stone=super::arenas::for_block(block).unwrap().stone;
        if self.script.is_none() {
            let handle=world.chr_sets.iter().flatten().flat_map(|set|set.characters())
                .find(|chr|chr.event_entity_id==stone).map(|chr|chr.field_ins_handle);
            let Some(handle)=handle else {
                if !self.reported {super::log(format!("Spirit stone entity {stone} not loaded during combat"));self.reported=true;}
                return Ok(());
            };
            reset_twin();
            // Initialize only after the destination stone has loaded.
            super::native_talk::set_flag(SUMMONED_FLAG,false)?;
            self.script=Some(TalkScript::new(BlockId::from(block),2000,handle));
            self.reported=false;
            super::log(format!("Spirit stone bound: {handle:?}"));
        }
        let mut used=super::native_talk::flag(SUMMONED_FLAG)?;
        let manager=&world.summon_buddy_manager;
        if let Ok(mut gate)=REPLACEMENT.lock() {
            if gate.is_some_and(|(address,effect)|address!=std::ptr::from_ref(&**manager) as usize||effect!=manager.request_summon_speffect_id) {*gate=None;}
        }
        let buddies=manager.groups.iter().flat_map(|pair|pair.second.iter()).filter(|group| {
            let chr=unsafe {group.chr_ins.as_ref()};
            !group.is_remote&&!group.disappear_requested&&chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag()
        }).count();
        if self.buddies!=Some(buddies) {
            super::log(format!("TOWER_TWIN alive spirits={buddies} (was {:?}) active_goods={} active_effect={} alive_flag={} reopened={}",self.buddies,manager.active_summon_goods_id,manager.active_summon_speffect_id,manager.player_has_alive_summon,self.twin_reopened));
            self.buddies=Some(buddies);
        }
        if buddies>0 {self.spirit_seen=true;}
        if resummon_allowed(RESUMMON.load(Ordering::Acquire),used,self.spirit_seen,buddies) {
            super::native_talk::set_flag(SUMMONED_FLAG,false)?;
            used=false;self.spirit_seen=false;self.twin_reopened=false;reset_twin();
            super::log("TOWER_RESUMMON all spirits are gone; summon point reopened");
        }
        let twin=TWIN.load(Ordering::Acquire);
        if used&&twin&&!self.twin_reopened&&buddies>0 {
            super::native_talk::set_flag(SUMMONED_FLAG,false)?;
            self.twin_reopened=true;used=false;
            super::log("TOWER_TWIN first spirit is out; summon point reopened for a second spirit");
        }
        TWIN_ARMED.store(twin_armed(twin&&!SECOND_ACCEPTED.load(Ordering::Acquire),self.twin_reopened,used,buddies),Ordering::Release);
        let status=(manager.buddy_stone_entity_id,manager.is_within_activation_range,manager.player_has_alive_summon,used);
        if self.status!=Some(status) {
            super::log(format!("Spirit state: stone={} range={} alive={} used={} cooldown={}",status.0,status.1,status.2,status.3,manager.item_use_cooldown_timer));
            self.status=Some(status);
        }
        if used {return Ok(());}
        BOUND.store(std::ptr::from_ref(&**manager) as usize,Ordering::Release);
        let script=self.script.as_mut().unwrap();
        // Native t000002000 refreshes the short-lived summon permission before registration.
        for id in [9530,84] {
            if let Err(error)=script.event((62,vec![EzStateValue::Int32(id)])) {self.failed=true;return Err(error.to_string());}
        }
        if let Err(error)=script.event((122,Vec::<EzStateValue>::new())) {self.failed=true;return Err(error.to_string());}
        let registered=world.summon_buddy_manager.buddy_stone_entity_id;
        if self.registration!=Some(registered) {
            super::log(format!("Spirit registration result: expected={stone} actual={registered}, entity_valid={}",world.chr_ins_by_handle(&script.npc_talk.base.field_ins_handle).is_some()));
            self.registration=Some(registered);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn frame_pause_keeps_accepted_second_request_until_encounter_cleanup() {
        super::ACTIVE.store(true,super::Ordering::Release);
        super::BOUND.store(10,super::Ordering::Release);
        super::SECOND_ACCEPTED.store(true,super::Ordering::Release);
        *super::REPLACEMENT.lock().unwrap()=Some((10,232000));
        super::begin_frame();
        assert!(!super::ACTIVE.load(super::Ordering::Acquire));
        assert!(super::SECOND_ACCEPTED.load(super::Ordering::Acquire));
        assert_eq!(*super::REPLACEMENT.lock().unwrap(),Some((10,232000)));
        super::set_active(true);
        assert!(super::SECOND_ACCEPTED.load(super::Ordering::Acquire));
        super::set_active(false);
        assert!(!super::SECOND_ACCEPTED.load(super::Ordering::Acquire));
        assert_eq!(*super::REPLACEMENT.lock().unwrap(),None);
    }
    #[test]
    fn second_use_preserves_cooldown_range_and_single_extra_use() {
        use super::second_use_allowed as allowed;
        assert!(allowed(true,false,-1,0.0,true,false));
        assert!(!allowed(false,false,-1,0.0,true,false));
        assert!(!allowed(true,true,-1,0.0,true,false));
        assert!(!allowed(true,false,212000,0.0,true,false));
        assert!(!allowed(true,false,-1,0.01,true,false));
        assert!(!allowed(true,false,-1,f32::NAN,true,false));
        assert!(!allowed(true,false,-1,0.0,false,false));
        assert!(!allowed(true,false,-1,0.0,true,true));
    }
    #[test]
    fn replacement_guard_only_protects_the_matching_pending_second_summon() {
        let ticket=Some((10usize,217001));
        assert!(!super::preserve_replacement(ticket,10,217001,false,true,false));
        assert!(!super::preserve_replacement(ticket,20,217001,true,true,false));
        assert!(!super::preserve_replacement(ticket,10,232000,true,true,false));
        assert!(!super::preserve_replacement(ticket,10,217001,true,false,false));
        assert!(!super::preserve_replacement(ticket,10,217001,true,true,true));
        for _ in 0..3 {assert!(super::preserve_replacement(ticket,10,217001,true,true,false),"pending request can span frames");}
        assert!(!super::preserve_replacement(ticket,10,-1,true,true,false),"spawn consumed the request");
        assert!(!super::preserve_replacement(None,10,217001,true,true,false));
    }
    #[test]
    fn frame_refresh_never_leaks_to_other_managers_maps_or_finished_encounters() {
        use super::refresh_allowed;
        assert!(refresh_allowed(true,10,10,0x2d020000,true,false));
        assert!(!refresh_allowed(false,10,10,0x2d020000,true,false));
        assert!(!refresh_allowed(true,0,0,0x2d020000,true,false));
        assert!(!refresh_allowed(true,10,20,0x2d020000,true,false));
        assert!(!refresh_allowed(true,10,10,0x0b0a0000,true,false));
        assert!(!refresh_allowed(true,10,10,0x2d020000,false,false));
        assert!(!refresh_allowed(true,10,10,0x2d020000,true,true));
    }
    #[test]
    fn the_twin_summon_only_lifts_the_alive_flag_between_the_first_and_second_spirit() {
        use super::twin_armed;
        assert!(twin_armed(true,true,false,1));
        assert!(!twin_armed(false,true,false,1),"no ember");
        assert!(!twin_armed(true,false,false,1),"first summon not yet made");
        assert!(!twin_armed(true,true,true,2),"second spirit already summoned");
        assert!(!twin_armed(true,true,false,0),"the first spirit is gone: the native rules apply again");
    }
    #[test]
    fn summoners_can_call_again_only_after_their_spirits_are_gone() {
        use super::resummon_allowed as allowed;
        assert!(allowed(true,true,true,0),"the spirit came out and died");
        assert!(!allowed(true,true,false,0),"used the bell, spirit not out yet");
        assert!(!allowed(true,true,true,1),"a spirit is still alive");
        assert!(!allowed(true,false,true,0),"the summon point is already open");
        assert!(!allowed(false,true,true,0),"other classes keep the native rule");
    }
    #[test]
    fn summons_are_only_registered_for_an_unused_combat_in_the_arena() {
        for act in 1..=3 {
            let block=super::super::arenas::for_act(act).block;
            assert!(super::eligible(block,true,false));
            assert!(!super::eligible(block,true,true));
            assert!(!super::eligible(block,false,false));
        }
        for block in [0x2d000000,0x2d010000,0x2d020000] {
            assert!(super::eligible(block,true,false));
            assert!(!super::eligible(block,false,false));
            assert!(!super::eligible(block,true,true));
        }
        assert!(super::eligible(0x2d020000,true,false));
        assert!(!super::eligible(0x2d020000,false,false));
        assert!(!super::eligible(0x2d020000,true,true));
        assert!(!super::eligible(0x0b0a0000,true,false));
    }
}
