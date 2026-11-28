use eldenring::{cs::{BlockId,ChrInsExt,TalkScript,WorldChrMan},ez_state::EzStateValue};
use fromsoftware_shared::FromStatic;
use std::{ffi::c_void,sync::{Mutex,atomic::{AtomicBool,AtomicUsize,Ordering}}};
use hudhook::mh::{MhHook,MH_EnableHook};
mod seamless_hook;
mod dismissal;
mod fade;
pub(crate) mod landing;
pub(crate) mod refund;

static ACTIVE:AtomicBool=AtomicBool::new(false);
static ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static USE_STATE:AtomicUsize=AtomicUsize::new(0);
static CLEAR:AtomicUsize=AtomicUsize::new(0);
static REGISTER:AtomicUsize=AtomicUsize::new(0);
static BOUND:AtomicUsize=AtomicUsize::new(0);
pub fn begin_frame(){ACTIVE.store(false,Ordering::Release);BOUND.store(0,Ordering::Release);TWIN_ARMED.store(false,Ordering::Release);}
pub fn set_active(active:bool){ACTIVE.store(active,Ordering::Release);if !active {BOUND.store(0,Ordering::Release);RESUMMON_ARMED.store(false,Ordering::Release);reset_twin();}}
/// 双生的灵唤铃（余烬，单人与联机）：允许在同一场战斗中召唤任意数量的骨灰同时在场。房主走 Pending 的召唤点、成员走 party_spirits，都经过 Spirits::poll。
/// 原版只记一个“当前召唤”，场上有召唤物时不让用骨灰，所以在“使用前检查”和请求阶段把“有存活召唤物”暂时清掉（ARMED）。
/// 每次请求接受后，仅跳过更新函数中新召唤替换旧组的那一次清除（0x4b8a90 -> 0x4bba80），让新旧召唤物共存。
static TWIN:AtomicBool=AtomicBool::new(false);
static TWIN_ARMED:AtomicBool=AtomicBool::new(false);
static PARTY:AtomicBool=AtomicBool::new(false);
static CAP_NOTICED:AtomicBool=AtomicBool::new(false);
static CAP_NOTICE:Mutex<Option<String>>=Mutex::new(None);
static REPLACE_ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static REPLACE_CALLER:AtomicUsize=AtomicUsize::new(0);
static REPLACE_READY:AtomicBool=AtomicBool::new(false);
static REPLACEMENT:Mutex<Option<(usize,i32)>>=Mutex::new(None);
const PARTY_SUMMON_SOFT_CAP:usize=8;
const SOLO_SUMMON_SOFT_CAP:usize=24;
pub fn set_party(party:bool){PARTY.store(party,Ordering::Release);}
pub fn take_cap_notice()->Option<String>{CAP_NOTICE.lock().ok().and_then(|mut g|g.take())}
fn summon_slot_budget(capacity:usize,party:bool)->usize {
    if party {capacity.saturating_sub(4).min(PARTY_SUMMON_SOFT_CAP)}
    else {capacity.saturating_sub(2).min(SOLO_SUMMON_SOFT_CAP)}
}
fn summon_capacity_ok(used:usize,capacity:usize,party:bool)->bool {
    capacity==0||used<summon_slot_budget(capacity,party)
}
fn pool_usage(manager:&eldenring::cs::SummonBuddyManager)->(usize,usize) {
    let chr_set=unsafe {manager.chr_set.as_ref()};
    let capacity=chr_set.capacity as usize;
    let occupied=chr_set.characters().count();
    let grouped=manager.groups.iter().flat_map(|pair|pair.second.iter()).count();
    (occupied.max(grouped),capacity)
}
pub fn has_local_groups(world:&WorldChrMan)->bool {
    let Some(owner)=world.main_player.as_ref().map(|p|unsafe {p.player_game_data.as_ref()}.character_event_id as i32) else{return false;};
    world.summon_buddy_manager.groups.iter().filter(|pair|pair.first==owner)
        .flat_map(|pair|pair.second.iter()).any(|group|!group.is_remote)
}
/// 切图 / 传送门 / 暂退前记录场上残留的本机与远程骨灰，并立即清理本机残留召唤组，避免地图卸载与骨灰销毁撞车。
pub fn log_remaining(world:&WorldChrMan,reason:&str) {
    let entries:Vec<_>=world.summon_buddy_manager.groups.iter().flat_map(|pair| {
        let owner=pair.first;
        pair.second.iter().filter_map(move |group| {
            let Some(chr)=live_chr(group) else{return Some(format!("owner={owner} buddy={} chr=unloaded remote={} dismiss={}",group.buddy_param_id,group.is_remote,group.disappear_requested));};
            Some(format!("owner={owner} buddy={} npc={} remote={} hp={}/{} dismiss={} delay={:.2}",
                group.buddy_param_id,chr.npc_param_id,group.is_remote,chr.modules.data.hp,chr.modules.data.max_hp,group.disappear_requested,group.disappear_delay_sec))
        })
    }).collect();
    if !entries.is_empty() {
        let (used,capacity)=pool_usage(&world.summon_buddy_manager);
        super::log(format!("ASH_SAFETY {reason}: pool={used}/{capacity} active_goods={} pending={} entries=[{}]",
            world.summon_buddy_manager.active_summon_goods_id,world.summon_buddy_manager.request_summon_speffect_id,entries.join("; ")));
    }
}
fn native_warp_cleanup(party:bool, groups:impl Iterator<Item=(bool,i32)>)->bool {
    party && groups.into_iter().any(|(remote,param)|remote || fade::custom(param))
}
#[test]
fn coop_warp_with_retiring_custom_ash_uses_native_cleanup() {
    let reported=(false,1_600_280_000);
    assert!(native_warp_cleanup(true,[reported].into_iter()));
    assert!(native_warp_cleanup(true,[(false,100),(true,200)].into_iter()));
    assert!(!native_warp_cleanup(true,[(false,100)].into_iter()));
    assert!(!native_warp_cleanup(false,[reported].into_iter()));
}
pub fn prepare_warp(world:&mut WorldChrMan,reason:&str) {
    set_active(false);
    let buddy=&mut *world.summon_buddy_manager;
    let defer=native_warp_cleanup(super::runtime::in_party(),buddy.groups.iter()
        .flat_map(|p|p.second.iter()).map(|g|(g.is_remote,g.buddy_param_id)));
    if defer {
        buddy.disappear_requested=true;
        crate::log(format!("ASH_DISMISS {reason}: leaving co-op summon retirement to native warp"));
        return;
    }
    log_remaining(world,reason);
    world.summon_buddy_manager.disappear_requested=true;
    if !REPLACE_READY.load(Ordering::Acquire) {return;}
    let Some(owner)=world.main_player.as_ref().map(|p|unsafe {p.player_game_data.as_ref()}.character_event_id as i32) else{return;};
    let manager: *mut u8 = std::ptr::from_mut(&mut *world.summon_buddy_manager).cast();
    unsafe {dismissal::clear_local_now(manager,owner,REPLACE_ORIGINAL.load(Ordering::Acquire));}
}
/// F6 暂退请求时自动遣返场上本机骨灰，避免 `has_local_groups` 一直阻塞暂退。
pub fn dismiss_local_for_suspend(world:&mut WorldChrMan)->bool {
    if !has_local_groups(world) {return false;}
    world.summon_buddy_manager.disappear_requested=true;
    if REPLACE_READY.load(Ordering::Acquire)
        &&let Some(owner)=world.main_player.as_ref().map(|p|unsafe {p.player_game_data.as_ref()}.character_event_id as i32) {
        let manager:*mut u8=std::ptr::from_mut(&mut *world.summon_buddy_manager).cast();
        unsafe {dismissal::clear_local_now(manager,owner,REPLACE_ORIGINAL.load(Ordering::Acquire));}
    }
    true
}
fn reset_twin() {
    TWIN_ARMED.store(false,Ordering::Release);
    CAP_NOTICED.store(false,Ordering::Release);
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
        super::log(format!("TOWER_TWIN clear audit: caller_match={} owner={owner} pending={} cancel={} bound={} ticket={ticket:?}",caller==expected,buddy.request_summon_speffect_id,buddy.disappear_requested,BOUND.load(Ordering::Acquire)==manager as usize));
    }
    if expected!=0&&caller==expected&&ACTIVE.load(Ordering::Acquire)&&TWIN.load(Ordering::Acquire)
        &&BOUND.load(Ordering::Acquire)==manager as usize {
        let buddy=unsafe {&*manager.cast::<eldenring::cs::SummonBuddyManager>()};
        let living_player=unsafe {eldenring::cs::PlayerIns::local_player()}.is_ok_and(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()&&unsafe {p.player_game_data.as_ref()}.character_event_id as i32==owner&&super::arenas::for_block(i32::from(p.current_block_id)).is_some());
        let mut preserved=false;
        if let Ok(gate)=REPLACEMENT.lock() {
            preserved=preserve_replacement(*gate,manager as usize,buddy.request_summon_speffect_id,true,living_player,buddy.disappear_requested);
        }
        if preserved {
            super::log(format!("TOWER_TWIN preserved existing groups: owner={owner} pending={} caller=4b8a95",buddy.request_summon_speffect_id));return;
        }
    }
    unsafe {original(manager,owner);}
}
/// 第二只的召唤请求被原版拒绝时记下骨灰编号：扣骨灰与发召唤请求在使用动画里同时发生，被拒就会白扣一个，由运行时补回
/// 召唤师「群灵」：自己的召唤物出场后全部阵亡（或消失），重新开放召唤点，可以再召唤新的（骨灰照常消耗，胜利时还活着照样返还）
static RESUMMON:AtomicBool=AtomicBool::new(false);
static RESUMMON_ARMED:AtomicBool=AtomicBool::new(false);
pub fn set_resummon(on:bool){if !on{RESUMMON_ARMED.store(false,Ordering::Release);}if RESUMMON.swap(on,Ordering::AcqRel)!=on {super::log(format!("TOWER_RESUMMON enabled={on}"));}}
/// 刷怪前已召唤的骨灰，开战重绑召唤点后 used 可以为 false；以实际存活状态为准。
fn resummon_allowed(enabled:bool,_used:bool,seen:bool,buddies:usize)->bool {enabled&&seen&&buddies==0}
/// 召唤物组里的角色在切图、战斗结算、队友骨灰消散的那几帧可能已经半销毁：角色指针或它的模块容器是空的。
/// 这两个字段在类型上是 NonNull / OwnedPtr，普通的判空会被编译器当成「永远不为空」优化掉（r12 星涌就栽在这），
/// 所以这里用 volatile 读原始指针判空。阿苏勒狂战士 r16 闪退：结算时 fade::tick 读到 modules=null。
pub(crate) fn live_chr(group:&eldenring::cs::SummonBuddyGroup)->Option<&eldenring::cs::ChrIns> {
    let chr=unsafe {std::ptr::read_volatile(std::ptr::addr_of!(group.chr_ins).cast::<usize>())};
    if chr==0 {return None;}
    let modules=unsafe {std::ptr::read_volatile((chr+std::mem::offset_of!(eldenring::cs::ChrIns,modules)) as *const usize)};
    if modules==0 {return None;}
    Some(unsafe {&*(chr as *const eldenring::cs::ChrIns)})
}
pub(crate) fn live_chr_mut(group:&eldenring::cs::SummonBuddyGroup)->Option<&mut eldenring::cs::ChrIns> {
    live_chr(group)?;
    Some(unsafe {&mut *group.chr_ins.as_ptr()})
}
fn resummon_use_allowed(enabled:bool,armed:bool,pending:i32,cooldown:f32,registered:bool,buddies:usize)->bool {
    enabled&&armed&&pending<0&&cooldown<=0.0&&registered&&buddies==0
}
fn local_buddies(manager:&eldenring::cs::SummonBuddyManager,owner:i32)->usize {
    manager.groups.iter().filter(|pair|pair.first==owner).flat_map(|pair|pair.second.iter()).filter(|group| {
        let Some(chr)=live_chr(group) else{return false;};
        !group.is_remote&&chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag()
    }).count()
}
pub fn local_handles(world:&WorldChrMan)->Vec<eldenring::cs::FieldInsHandle> {
    let Some(owner)=world.main_player.as_ref().map(|p|unsafe {p.player_game_data.as_ref()}.character_event_id as i32) else{return Vec::new();};
    world.summon_buddy_manager.groups.iter().filter(|pair|pair.first==owner)
        .flat_map(|pair|pair.second.iter()).filter(|group|!group.is_remote)
        .filter_map(|group|live_chr(group).map(|chr|chr.field_ins_handle)).collect()
}
pub fn local_living_handles(world:&WorldChrMan)->Vec<eldenring::cs::FieldInsHandle> {
    let Some(owner)=world.main_player.as_ref().map(|p|unsafe {p.player_game_data.as_ref()}.character_event_id as i32) else{return Vec::new();};
    world.summon_buddy_manager.groups.iter().filter(|pair|pair.first==owner)
        .flat_map(|pair|pair.second.iter()).filter_map(|group| {
            let chr=live_chr(group)?;
            (!group.is_remote&&chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag()&&!chr.debug_flags.character_disabled()).then_some(chr.field_ins_handle)
        }).collect()
}
/// 场上当前存活的本机召唤物组数：战斗中优先按 refund 账本按次统计（同一骨灰连召两组也记 2 组），账本未开启时按 buddy_param_id/1000 去重兜底。
pub fn local_living_group_count(world:&WorldChrMan)->usize {
    let Some(owner)=world.main_player.as_ref().map(|p|unsafe {p.player_game_data.as_ref()}.character_event_id as i32) else{return 0;};
    let mut families=std::collections::BTreeSet::new();
    for group in world.summon_buddy_manager.groups.iter().filter(|pair|pair.first==owner).flat_map(|pair|pair.second.iter()) {
        let Some(chr)=live_chr(group) else{continue;};
        if !group.is_remote&&chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag()&&!chr.debug_flags.character_disabled() {
            families.insert(group.buddy_param_id/1000);
        }
    }
    if families.is_empty() {return 0;}
    refund::living_groups().filter(|&n|n>0).unwrap_or(families.len())
}
fn resummon_ready(manager:&eldenring::cs::SummonBuddyManager)->bool {
    if !ACTIVE.load(Ordering::Acquire)||BOUND.load(Ordering::Acquire)!=std::ptr::from_ref(manager) as usize{return false;}
    let Ok(player)=(unsafe {eldenring::cs::PlayerIns::local_player()}) else{return false;};
    if player.chr_ins.modules.data.hp<=0||player.chr_ins.chr_flags1c5.death_flag(){return false;}
    let Some(arena)=super::arenas::for_block(i32::from(player.current_block_id)) else{return false;};
    let owner=unsafe {player.player_game_data.as_ref()}.character_event_id as i32;
    resummon_use_allowed(RESUMMON.load(Ordering::Acquire),RESUMMON_ARMED.load(Ordering::Acquire),
        manager.request_summon_speffect_id,manager.item_use_cooldown_timer,manager.buddy_stone_entity_id==arena.stone,local_buddies(manager,owner))
        &&super::native_talk::flag(SUMMONED_FLAG)==Ok(false)
}
static TWIN_REFUND:std::sync::atomic::AtomicI32=std::sync::atomic::AtomicI32::new(0);
pub fn take_twin_refund()->Option<u32> {let id=TWIN_REFUND.swap(0,Ordering::AcqRel);(id>0).then_some(id as u32&0x0fffffff)}
pub fn set_twin(on:bool){let on=on&&REPLACE_READY.load(Ordering::Acquire);if TWIN.swap(on,Ordering::AcqRel)!=on {super::log(format!("TOWER_TWIN enabled={on}"));if !on{reset_twin();}}}
fn refresh_allowed(active:bool,bound:usize,manager:usize,block:i32,alive:bool,used:bool)->bool {
    active&&bound!=0&&bound==manager&&eligible(block,alive,used)
}
fn twin_use_allowed(armed:bool,pending:i32,cooldown:f32,in_range:bool,cancel:bool)->bool {
    armed&&pending<0&&cooldown<=0.0&&in_range&&!cancel
}
unsafe extern "C" fn use_state(manager:*mut u8,goods:i32)->i32 {
    let original:unsafe extern "C" fn(*mut u8,i32)->i32=unsafe {std::mem::transmute(USE_STATE.load(Ordering::Acquire))};
    let state=unsafe {original(manager,goods)};
    let buddy=unsafe {&*manager.cast::<eldenring::cs::SummonBuddyManager>()};
    if (goods==-1||goods>=0&&erdtree_tower::rewards::spirit_level(0x40000000|goods as u32).is_some())
        &&resummon_ready(buddy) {return 0;}
    if ACTIVE.load(Ordering::Acquire)&&TWIN.load(Ordering::Acquire)&&BOUND.load(Ordering::Acquire)==manager as usize {
        let buddy=unsafe {&*manager.cast::<eldenring::cs::SummonBuddyManager>()};
        let range=buddy.is_within_activation_range&&buddy.buddy_stone_entity_id!=0;
        if twin_use_allowed(TWIN_ARMED.load(Ordering::Acquire),buddy.request_summon_speffect_id,buddy.item_use_cooldown_timer,range,buddy.disappear_requested)
            &&super::native_talk::flag(SUMMONED_FLAG)==Ok(false)
            &&(goods==-1||goods>=0&&erdtree_tower::rewards::spirit_level(0x40000000|goods as u32).is_some())
            &&unsafe {eldenring::cs::PlayerIns::local_player()}.is_ok_and(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()&&super::arenas::for_block(i32::from(p.current_block_id)).is_some()) {
            return 0;
        }
    }
    state
}
unsafe extern "C" fn clear_frame(manager:*mut u8) {
    let original:unsafe extern "C" fn(*mut u8)=unsafe {std::mem::transmute(CLEAR.load(Ordering::Acquire))};
    unsafe {original(manager);}
    if super::runtime::in_party() && unsafe {eldenring::cs::GameMan::instance()}.is_ok_and(|g|g.warp_requested) {
        dismissal::reset_watch();
        return;
    }
    unsafe {fade::tick(manager);}
    if REPLACE_READY.load(Ordering::Acquire) {unsafe {dismissal::tick(manager,REPLACE_ORIGINAL.load(Ordering::Acquire));}}
    if !ACTIVE.load(Ordering::Acquire)||BOUND.load(Ordering::Acquire)!=manager as usize {return;}
    // 双生的灵唤铃 / 召唤师「群灵」：允许继续召唤时，在这一阶段的使用检查里清掉残留的“有存活召唤物”标记
    let buddy=unsafe {&mut *manager.cast::<eldenring::cs::SummonBuddyManager>()};
    let resummon_clear=RESUMMON.load(Ordering::Acquire)&&RESUMMON_ARMED.load(Ordering::Acquire)&&buddy.request_summon_speffect_id<0
        &&unsafe {eldenring::cs::PlayerIns::local_player()}.is_ok_and(|p|local_buddies(buddy,unsafe {p.player_game_data.as_ref()}.character_event_id as i32)==0);
    if TWIN_ARMED.load(Ordering::Acquire)||resummon_clear {buddy.player_has_alive_summon=false;}
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
    if !ACTIVE.load(Ordering::Acquire){
        let usage=refund::tracking().then(|| {
            let buddy=unsafe {&*manager.cast::<eldenring::cs::SummonBuddyManager>()};
            let goods=unsafe {manager.add(0xe0).cast::<i32>().read()};
            refund::capture(buddy,goods,effect)
        }).flatten();
        let accepted=unsafe {original(manager,effect)};
        if accepted&&let Some(usage)=usage {refund::accepted(usage);}
        return accepted;
    }
    let query:unsafe extern "C" fn(*mut u8,i32)->i32=unsafe {std::mem::transmute(USE_STATE.load(Ordering::Acquire))};
    let state=unsafe {query(manager,-1)};
    let field=|offset|unsafe {manager.add(offset).cast::<i32>().read()};
    super::log(format!("Spirit use request: effect={effect} state={state} stone={} active_stone={} goods={} active_goods={} twin_armed={}",field(0x38),field(0x3c),field(0xe0),field(0xe4),TWIN_ARMED.load(Ordering::Acquire)));
    let twin=TWIN.load(Ordering::Acquire)&&TWIN_ARMED.load(Ordering::Acquire)&&BOUND.load(Ordering::Acquire)==manager as usize;let goods=field(0xe0);
    let resummon=resummon_ready(unsafe {&*manager.cast::<eldenring::cs::SummonBuddyManager>()});
    // A native state-2 rejection requests disappearance; only clear it after all local summons are dead.
    if resummon {
        let buddy=unsafe {&mut *manager.cast::<eldenring::cs::SummonBuddyManager>()};
        buddy.disappear_requested=false;
        buddy.player_has_alive_summon=false;
    } else if twin {
        let buddy=unsafe {&mut *manager.cast::<eldenring::cs::SummonBuddyManager>()};
        if !buddy.disappear_requested&&buddy.request_summon_speffect_id<0 {buddy.player_has_alive_summon=false;}
    }
    let usage=(BOUND.load(Ordering::Acquire)==manager as usize).then(||refund::capture(unsafe {&*manager.cast::<eldenring::cs::SummonBuddyManager>()},goods,effect)).flatten();
    if let Ok(mut gate)=REPLACEMENT.lock(){*gate=None;}
    let accepted=unsafe {original(manager,effect)};
    if resummon&&accepted {RESUMMON_ARMED.store(false,Ordering::Release);super::log(format!("TOWER_RESUMMON accepted goods={goods} effect={effect}"));}
    if accepted&&let Some(usage)=usage {refund::accepted(usage);}
    TWIN_ARMED.store(false,Ordering::Release);
    if twin&&accepted {
        if let Ok(mut gate)=REPLACEMENT.lock(){*gate=Some((manager as usize,effect));}
        super::log(format!("TOWER_TWIN accepted goods={goods} effect={effect}; existing groups protected"));
    }
    super::log(format!("Spirit use result: accepted={accepted} pending={} active={} stone={}",field(0x20),field(0x24),field(0x3c)));
    if (twin||resummon)&&!accepted&&goods>0 {TWIN_REFUND.store(goods,Ordering::Release);super::log(format!("TOWER_SUMMON request rejected; ash {goods} will be returned"));}
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
    /// 上一次记录的本机存活召唤物数量
    buddies:Option<usize>,
    /// 召唤师再召唤：这一轮召唤的召唤物已经出场过（出场前召唤物数为 0，不能当成阵亡）
    spirit_seen:bool}
// Accessed only from the serialized game task, like the native NPC dialogs.
unsafe impl Send for Spirits {}

/// 双生的灵唤铃：当前没有待处理召唤且未请求离场时，始终重新开放召唤点，允许召唤任意数量骨灰同时在场
fn twin_reopen_allowed(twin:bool,used:bool,pending:i32,cancel:bool)->bool {twin&&used&&pending<0&&!cancel}
fn twin_armed(twin:bool,used:bool,pending:i32,cancel:bool)->bool {twin&&!used&&pending<0&&!cancel}
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
        // 即使 SUMMONED_FLAG 暂时为 true（正在处理跨帧召唤请求），也必须保持 BOUND 有效，否则 replacement_clear 会放行旧组清除。
        BOUND.store(std::ptr::from_ref(&**manager) as usize,Ordering::Release);
        if let Ok(mut gate)=REPLACEMENT.lock() {
            if gate.is_some_and(|(address,effect)|address!=std::ptr::from_ref(&**manager) as usize||effect!=manager.request_summon_speffect_id) {*gate=None;}
        }
        let owner=world.main_player.as_ref().map(|p|unsafe {p.player_game_data.as_ref()}.character_event_id as i32);
        let buddies=owner.map_or(0,|owner|local_buddies(manager,owner));
        if self.buddies!=Some(buddies) {
            super::log(format!("TOWER_TWIN alive spirits={buddies} (was {:?}) active_goods={} active_effect={} alive_flag={}",self.buddies,manager.active_summon_goods_id,manager.active_summon_speffect_id,manager.player_has_alive_summon));
            self.buddies=Some(buddies);
        }
        if buddies>0 {self.spirit_seen=true;}
        if owner.is_some()&&manager.request_summon_speffect_id<0&&resummon_allowed(RESUMMON.load(Ordering::Acquire),used,self.spirit_seen,buddies) {
            super::native_talk::set_flag(SUMMONED_FLAG,false)?;
            used=false;self.spirit_seen=false;reset_twin();
            RESUMMON_ARMED.store(true,Ordering::Release);
            super::log(format!("TOWER_RESUMMON all local spirits are gone; native use armed; active_goods={} active_effect={}",manager.active_summon_goods_id,manager.active_summon_speffect_id));
        }
        let twin=TWIN.load(Ordering::Acquire);
        let party=PARTY.load(Ordering::Acquire);
        let (used_slots,capacity)=pool_usage(manager);
        let within_cap=summon_capacity_ok(used_slots,capacity,party);
        if twin&&!within_cap {
            if !CAP_NOTICED.swap(true,Ordering::AcqRel) {
                let budget=summon_slot_budget(capacity,party);
                super::log(format!("TOWER_TWIN paused at summon safety cap: used={used_slots} budget={budget} capacity={capacity} party={party}"));
                if let Ok(mut notice)=CAP_NOTICE.lock() {
                    *notice=Some(if party {format!("场上召唤物已达联机安全上限（{budget}），待部分离场或阵亡后可继续召唤。")}
                        else {format!("场上召唤物已达引擎安全上限（{budget}），待部分阵亡后可继续召唤。")});
                }
            }
        } else if within_cap {
            CAP_NOTICED.store(false,Ordering::Release);
        }
        if twin_reopen_allowed(twin&&within_cap,used,manager.request_summon_speffect_id,manager.disappear_requested) {
            super::native_talk::set_flag(SUMMONED_FLAG,false)?;
            used=false;
            super::log(format!("TOWER_TWIN summon point reopened for additional spirit (alive={buddies} pool={used_slots}/{capacity})"));
        }
        TWIN_ARMED.store(twin_armed(twin&&within_cap,used,manager.request_summon_speffect_id,manager.disappear_requested),Ordering::Release);
        let status=(manager.buddy_stone_entity_id,manager.is_within_activation_range,manager.player_has_alive_summon,used);
        if self.status!=Some(status) {
            super::log(format!("Spirit state: stone={} range={} alive={} used={} cooldown={}",status.0,status.1,status.2,status.3,manager.item_use_cooldown_timer));
            self.status=Some(status);
        }
        if used {return Ok(());}
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
/// 自制山妖/山妖骑士骨灰 AI 修正：
/// 1) 生成骨灰时 CustomSpiritAssets 曾用友军模板 121000000 覆盖了 spEffectID0 与 spEffectID29，
///    导致山妖骑士(c5391, 153918037)与卡利亚骑士波尔斯(c4601, 146018069)丢失 5401（animIdOffset=1000000 持剑动作组）与 10144（山妖骑士 AI 状态标记）；
/// 2) 三只山妖骨灰沿用了敌人专用 logicId=460000、disableLocalSteering=1 与窄视野(eye_angX=35/eye_angY=60)，
///    易在召唤后呆站不寻路/不进战。此处在内存恢复 5401 + 10144，并将三只山妖骨灰统一到原版骨灰通用 logicId=10000、允许局部转向与全向视野。
const TROLL_KNIGHT_NPCS:[u32;2]=[153918037,146018069];
const TROLL_SPIRIT_THINKS:[u32;3]=[153918037,146018069,146008031];
const TROLL_KNIGHT_ANIM_OFFSET:i32=5401;
const TROLL_KNIGHT_AI_MARKER:i32=10144;
const BUDDY_COMMON_LOGIC:i32=10000;
fn repair_troll_knight_npc(row:&mut eldenring::param::NPC_PARAM_ST)->bool {
    let mut changed=false;
    if row.sp_effect_id0()==5400 {row.set_sp_effect_id0(TROLL_KNIGHT_ANIM_OFFSET);changed=true;}
    if row.sp_effect_id15()<=0 {row.set_sp_effect_id15(TROLL_KNIGHT_AI_MARKER);changed=true;}
    changed
}
fn repair_troll_think(row:&mut eldenring::param::NPC_THINK_PARAM_ST)->bool {
    let mut changed=false;
    if row.logic_id()!=BUDDY_COMMON_LOGIC {row.set_logic_id(BUDDY_COMMON_LOGIC);changed=true;}
    if row.disable_local_steering()!=0 {row.set_disable_local_steering(0);changed=true;}
    if row.eye_dist()<100 {row.set_eye_dist(100);changed=true;}
    if row.eye_ang_x()<180 {row.set_eye_ang_x(180);changed=true;}
    if row.eye_ang_y()<180 {row.set_eye_ang_y(180);changed=true;}
    if row.disable_dark()==0 {row.set_disable_dark(1);changed=true;}
    changed
}
pub fn repair_custom_spirit_ai() {
    use eldenring::cs::{NpcParam,NpcThinkParam,SoloParamRepository};
    static DONE:AtomicBool=AtomicBool::new(false);
    if DONE.load(Ordering::Relaxed) {return;}
    let Ok(params)=(unsafe {SoloParamRepository::instance_mut()}) else{return};
    let (mut found_npc,mut fixed_npc,mut found_think,mut fixed_think)=(0usize,0usize,0usize,0usize);
    for id in TROLL_KNIGHT_NPCS {
        if let Some(row)=params.get_mut::<NpcParam>(id) {found_npc+=1;if repair_troll_knight_npc(row){fixed_npc+=1;}}
    }
    for id in TROLL_SPIRIT_THINKS {
        if let Some(row)=params.get_mut::<NpcThinkParam>(id) {found_think+=1;if repair_troll_think(row){fixed_think+=1;}}
    }
    if found_npc==0&&found_think==0 {return;}
    if found_npc==TROLL_KNIGHT_NPCS.len()&&found_think==TROLL_SPIRIT_THINKS.len() {DONE.store(true,Ordering::Relaxed);}
    if fixed_npc>0||fixed_think>0 {
        super::log(format!("Custom troll spirit AI repaired: npc={fixed_npc}/{found_npc} think={fixed_think}/{found_think}"));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn troll_knight_spirit_repair_restores_anim_offset_marker_and_buddy_logic() {
        let mut npc:eldenring::param::NPC_PARAM_ST=unsafe {std::mem::zeroed()};
        npc.set_sp_effect_id0(5400);
        npc.set_sp_effect_id15(-1);
        assert!(super::repair_troll_knight_npc(&mut npc));
        assert_eq!(npc.sp_effect_id0(),5401);
        assert_eq!(npc.sp_effect_id15(),10144);
        assert!(!super::repair_troll_knight_npc(&mut npc),"idempotent on second pass");

        let mut think:eldenring::param::NPC_THINK_PARAM_ST=unsafe {std::mem::zeroed()};
        think.set_logic_id(460000);
        think.set_battle_goal_id(460100);
        think.set_disable_local_steering(1);
        think.set_eye_dist(26);
        think.set_eye_ang_x(35);
        think.set_eye_ang_y(60);
        think.set_disable_dark(0);
        assert!(super::repair_troll_think(&mut think));
        assert_eq!(think.logic_id(),10000);
        assert_eq!(think.battle_goal_id(),460100,"battle goal preserved");
        assert_eq!(think.disable_local_steering(),0);
        assert_eq!(think.eye_dist(),100);
        assert_eq!((think.eye_ang_x(),think.eye_ang_y(),think.disable_dark()),(180,180,1));
        assert!(!super::repair_troll_think(&mut think),"idempotent on second pass");
    }
    #[test]
    fn resummon_permission_keeps_native_lifetime_cooldown_and_arena_guards() {
        use super::resummon_use_allowed as allowed;
        assert!(allowed(true,true,-1,0.0,true,0));
        assert!(allowed(true,true,-1,-0.000045,true,0),"captured failed request after first summon died");
        assert!(!allowed(false,true,-1,0.0,true,0));
        assert!(!allowed(true,false,-1,0.0,true,0));
        assert!(!allowed(true,true,610039000,0.0,true,0));
        assert!(!allowed(true,true,-1,0.5,true,0));
        assert!(!allowed(true,true,-1,f32::NAN,true,0));
        assert!(!allowed(true,true,-1,0.0,false,0));
        assert!(!allowed(true,true,-1,0.0,true,1));
    }
    #[test]
    fn frame_pause_keeps_accepted_twin_request_until_encounter_cleanup() {
        super::ACTIVE.store(true,super::Ordering::Release);
        super::BOUND.store(10,super::Ordering::Release);
        super::RESUMMON_ARMED.store(true,super::Ordering::Release);
        *super::REPLACEMENT.lock().unwrap()=Some((10,232000));
        super::begin_frame();
        assert!(!super::ACTIVE.load(super::Ordering::Acquire));
        assert!(super::RESUMMON_ARMED.load(super::Ordering::Acquire));
        assert_eq!(*super::REPLACEMENT.lock().unwrap(),Some((10,232000)));
        super::set_active(true);
        assert_eq!(*super::REPLACEMENT.lock().unwrap(),Some((10,232000)));
        super::set_active(false);
        assert!(!super::RESUMMON_ARMED.load(super::Ordering::Acquire));
        assert_eq!(*super::REPLACEMENT.lock().unwrap(),None);
    }
    #[test]
    fn twin_use_preserves_cooldown_range_and_cancel_guards() {
        use super::twin_use_allowed as allowed;
        assert!(allowed(true,-1,0.0,true,false));
        assert!(allowed(true,-1,-0.01,true,false));
        assert!(!allowed(false,-1,0.0,true,false));
        assert!(!allowed(true,212000,0.0,true,false));
        assert!(!allowed(true,-1,0.01,true,false));
        assert!(!allowed(true,-1,f32::NAN,true,false));
        assert!(!allowed(true,-1,0.0,false,false));
        assert!(!allowed(true,-1,0.0,true,true));
    }
    #[test]
    fn replacement_guard_only_protects_the_matching_pending_summon() {
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
    fn twin_summon_reopens_after_every_completed_summon_until_dismissal() {
        use super::{twin_armed,twin_reopen_allowed};
        assert!(twin_reopen_allowed(true,true,-1,false),"reopens after 1st, 2nd, 3rd... summon finishes");
        assert!(!twin_reopen_allowed(false,true,-1,false),"no ember");
        assert!(!twin_reopen_allowed(true,false,-1,false),"already open");
        assert!(!twin_reopen_allowed(true,true,212000,false),"waits for pending summon to finish");
        assert!(!twin_reopen_allowed(true,true,-1,true),"never reopens during dismissal");
        assert!(twin_armed(true,false,-1,false),"armed before 1st and after every subsequent summon");
        assert!(!twin_armed(false,false,-1,false),"no ember");
        assert!(!twin_armed(true,true,-1,false),"waits for flag clear");
        assert!(!twin_armed(true,false,212000,false),"disarmed while a summon request is pending");
        assert!(!twin_armed(true,false,-1,true),"disarmed during dismissal");
    }
    #[test]
    fn summon_pool_soft_cap_is_stricter_in_party_and_reserves_engine_headroom() {
        use super::{summon_capacity_ok,summon_slot_budget};
        assert_eq!(summon_slot_budget(32,true),8);
        assert_eq!(summon_slot_budget(32,false),24);
        assert_eq!(summon_slot_budget(10,true),6);
        assert_eq!(summon_slot_budget(10,false),8);
        assert!(summon_capacity_ok(7,32,true));
        assert!(!summon_capacity_ok(8,32,true));
        assert!(summon_capacity_ok(23,32,false));
        assert!(!summon_capacity_ok(24,32,false));
        assert!(summon_capacity_ok(0,0,true));
    }
    #[test]
    fn summoners_can_call_again_only_after_their_spirits_are_gone() {
        use super::resummon_allowed as allowed;
        assert!(allowed(true,true,true,0),"the spirit came out and died");
        assert!(!allowed(true,true,false,0),"used the bell, spirit not out yet");
        assert!(!allowed(true,true,true,1),"a spirit is still alive");
        assert!(allowed(true,false,true,0),"pre-combat summons can die while the map flag is already clear");
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
