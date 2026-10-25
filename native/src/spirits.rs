use eldenring::{cs::{BlockId,ChrInsExt,TalkScript,WorldChrMan},ez_state::EzStateValue};
use fromsoftware_shared::FromStatic;
use std::{ffi::c_void,sync::atomic::{AtomicBool,AtomicUsize,Ordering}};
use hudhook::mh::{MhHook,MH_EnableHook};

static ACTIVE:AtomicBool=AtomicBool::new(false);
static ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static USE_STATE:AtomicUsize=AtomicUsize::new(0);
static CLEAR:AtomicUsize=AtomicUsize::new(0);
static REGISTER:AtomicUsize=AtomicUsize::new(0);
static BOUND:AtomicUsize=AtomicUsize::new(0);
pub fn set_active(active:bool){ACTIVE.store(active,Ordering::Release);if !active {BOUND.store(0,Ordering::Release);}}
fn refresh_allowed(active:bool,bound:usize,manager:usize,block:i32,alive:bool,used:bool)->bool {
    active&&bound!=0&&bound==manager&&eligible(block,alive,used)
}
unsafe extern "C" fn clear_frame(manager:*mut u8) {
    let original:unsafe extern "C" fn(*mut u8)=unsafe {std::mem::transmute(CLEAR.load(Ordering::Acquire))};
    unsafe {original(manager);}
    if !ACTIVE.load(Ordering::Acquire)||BOUND.load(Ordering::Acquire)!=manager as usize {return;}
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
    super::log(format!("Spirit use request: effect={effect} state={state} stone={} active_stone={} goods={} active_goods={}",field(0x38),field(0x3c),field(0xe0),field(0xe4)));
    let accepted=unsafe {original(manager,effect)};
    super::log(format!("Spirit use result: accepted={accepted} pending={} active={} stone={}",field(0x20),field(0x24),field(0x3c)));
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
    if let Err(error)=unsafe {install_trace()} {
        super::log(format!("Spirit usage tracing skipped; summon-point refresh remains active: {error}"));
    }
    Ok(())
}

unsafe fn install_trace()->Result<(),String> {
    let query=super::items::checked_function(0x4b73d0,&[0x48,0x89,0x5c,0x24,0x08,0x48,0x89,0x74,0x24,0x10,0x57,0x48,0x83,0xec,0x20])?;
    let address=super::items::checked_function(0x4b8690,&[0x48,0x89,0x5c,0x24,0x08,0x48,0x89,0x74,0x24,0x10,0x57,0x48,0x83,0xec,0x20])? as *mut c_void;
    let hook=unsafe {MhHook::new(address,request as *mut c_void)}.map_err(|e|format!("Spirit request trace: {e:?}"))?;
    USE_STATE.store(query,Ordering::Release);
    ORIGINAL.store(hook.trampoline() as usize,Ordering::Release);
    unsafe {MH_EnableHook(address)}.ok().map_err(|e|format!("Spirit request trace enable: {e:?}"))?;
    Ok(())
}

pub const SUMMONED_FLAG:u32=45020968;
#[derive(Default)]
pub struct Spirits {script:Option<TalkScript>,reported:bool,failed:bool,status:Option<(u32,bool,bool,bool)>,registration:Option<u32>}
// Accessed only from the serialized game task, like the native NPC dialogs.
unsafe impl Send for Spirits {}

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
            // Initialize only after the destination stone has loaded.
            super::native_talk::set_flag(SUMMONED_FLAG,false)?;
            self.script=Some(TalkScript::new(BlockId::from(block),2000,handle));
            self.reported=false;
            super::log(format!("Spirit stone bound: {handle:?}"));
        }
        let used=super::native_talk::flag(SUMMONED_FLAG)?;
        let manager=&world.summon_buddy_manager;
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
