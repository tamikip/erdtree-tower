use std::{sync::{LazyLock,Mutex},time::{Duration,Instant}};
use eldenring::{cs::{CSSessionManager,GameMan,WorldChrMan},fd4::FD4TaskData};
use fromsoftware_shared::FromStatic;

#[derive(Default)]
struct Probe { last:Option<Instant>, snapshot:String }
static PROBE:LazyLock<Mutex<Probe>>=LazyLock::new(||Mutex::new(Probe::default()));
#[link(name="kernel32")]
unsafe extern "system" {fn GetModuleHandleW(name:*const u16)->*mut std::ffi::c_void;}

pub fn enabled(value:Option<&str>)->bool {value==Some("1")}

pub fn capture(_: &FD4TaskData) {
    let Ok(mut probe)=PROBE.try_lock() else {return;};
    if probe.last.is_some_and(|t|t.elapsed()<Duration::from_secs(2)){return;}
    probe.last=Some(Instant::now());
    let game=unsafe {GameMan::instance()}.ok();
    let world=unsafe {WorldChrMan::instance()}.ok();
    let session=unsafe {CSSessionManager::instance()}.ok();
    let session_text=session.map(|s|{
        let roster=super::seamless::roster(s).map(|r|format!("members={} local_is_host={}",r.members.len(),r.local==r.host)).unwrap_or_else(|e|format!("pending={e}"));
        format!("lobby={:?} protocol={:?} limit={} roster={roster}",s.lobby_state,s.protocol_state,s.session_player_limit)
    }).unwrap_or_else(||"session=unavailable".into());
    let player_text=world.and_then(|w|w.main_player.as_ref()).map(|p|format!("block={:08x} alive={}",i32::from(p.current_block_id),p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag())).unwrap_or_else(||"player=unloaded".into());
    let module:Vec<_>="ersc.dll\0".encode_utf16().collect();
    let loaded=!unsafe {GetModuleHandleW(module.as_ptr())}.is_null();
    let text=format!("SEAMLESS PROBE ersc_loaded={loaded} {} {} {}",game.map(|g|format!("world={:?} official_online={} warping={}",g.event_world_type,g.is_in_online_mode,g.warp_requested)).unwrap_or_else(||"game=unavailable".into()),session_text,player_text);
    if text!=probe.snapshot {super::log(&text);probe.snapshot=text;}
}

#[test]
fn probe_requires_explicit_launcher_marker() {
    assert!(enabled(Some("1")));
    for value in [None,Some(""),Some("0"),Some("true")] {assert!(!enabled(value));}
}

#[test]
fn entity_probe_layout_matches_locked_dependency() {
    use eldenring::cs::{ChrIns,ChrSet,ChrSetEntry,NetChrSync,NetChrSetSync};
    use std::mem::{offset_of,size_of};
    let layout=[
        ("debug_set",offset_of!(WorldChrMan,debug_chr_set)),("sets",offset_of!(WorldChrMan,chr_sets)),
        ("capacity",offset_of!(ChrSet<ChrIns>,capacity)),("entries",offset_of!(ChrSet<ChrIns>,entries)),
        ("entry_size",size_of::<ChrSetEntry<ChrIns>>()),("load",offset_of!(ChrSetEntry<ChrIns>,chr_load_status)),
        ("update",offset_of!(ChrSetEntry<ChrIns>,chr_update_type)),("model",offset_of!(ChrIns,npc_id)),
        ("param",offset_of!(ChrIns,npc_param_id)),("character",offset_of!(ChrIns,character_id)),("p2p",offset_of!(ChrIns,p2p_entity_handle)),
        ("authority",offset_of!(ChrIns,network_authority)),("entity",offset_of!(ChrIns,event_entity_id)),
        ("sync",offset_of!(ChrIns,net_chr_sync_flags)),("debug_flags",offset_of!(ChrIns,debug_flags)),("backread",offset_of!(ChrIns,backread_state)),("load_state",offset_of!(ChrIns,load_state))
        ,("net",offset_of!(WorldChrMan,net_chr_sync)),("net_sets",offset_of!(NetChrSync,net_chr_set_sync)),
        ("net_set",offset_of!(NetChrSetSync,chr_set))
    ];
    assert_eq!(size_of::<ChrSetEntry<ChrIns>>(),16);
    println!("ENTITY_PROBE_LAYOUT={layout:?}");
}

#[test]
fn landing_probe_layout() {
    use eldenring::cs::{PlayerIns,ChrIns,ChrCtrl,ChrInsModuleContainer,CSChrPhysicsModule,CSChrFallModule};
    use std::mem::offset_of;
    let layout=[("player",offset_of!(WorldChrMan,main_player)),("block_pos",offset_of!(PlayerIns,block_position)),
        ("block",offset_of!(PlayerIns,current_block_id)),("modules",offset_of!(ChrIns,modules)),("ctrl",offset_of!(ChrIns,chr_ctrl)),
        ("proxy",offset_of!(ChrCtrl,chr_proxy_flags)),("physics",offset_of!(ChrInsModuleContainer,physics)),
        ("fall",offset_of!(ChrInsModuleContainer,fall)),("timer",offset_of!(CSChrFallModule,fall_timer)),
        ("position",offset_of!(CSChrPhysicsModule,position)),("update",offset_of!(CSChrPhysicsModule,chr_proxy_pos_update_requested)),
        ("ground",offset_of!(CSChrPhysicsModule,standing_on_solid_ground)),("falling",offset_of!(CSChrPhysicsModule,is_falling)),
        ("gravity",offset_of!(CSChrPhysicsModule,gravity_disabled))];
    println!("LANDING_LAYOUT={layout:?}");
}
