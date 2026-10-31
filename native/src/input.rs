use eldenring::fd4::FD4PadManager;
use fromsoftware_shared::FromStatic;
use hudhook::mh::{MH_EnableHook, MhHook};
use std::{
    ffi::c_void,
    sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering},
};

pub static CAPTURE: AtomicBool = AtomicBool::new(false);
pub fn capture_actions(shared_event:bool,overlay:bool,dialog:bool)->bool {!shared_event&&(overlay||dialog)}
pub static WINDOW: AtomicIsize = AtomicIsize::new(0);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);

#[derive(Default)]
struct ActionWindow {last_active:Option<std::time::Duration>}
impl ActionWindow {
    fn blocked(&mut self,now:std::time::Duration,active:bool)->bool {
        if active {self.last_active=Some(now);}
        active || self.last_active.is_some_and(|at|now.saturating_sub(at)<std::time::Duration::from_millis(350))
    }
}
/// 界面打开期间锁住角色：禁止攻击等动作（disabled_secondary_actions）与移动（disabled_movement）。
/// 正常情况下游戏输入已经被 PadCapture 停掉，这是第二道保险：即使输入漏到游戏（例如别的模组把轮询又打开），
/// 在三选一等界面按左右也不会让角色走动或出招。关闭后 350 毫秒再还原成打开前的状态
pub struct ActionGuard {
    clock:std::time::Instant,
    window:ActionWindow,
    saved:Option<(usize,eldenring::cs::FieldInsHandle,bool,bool)>,
}
impl Default for ActionGuard {
    fn default()->Self {Self{clock:std::time::Instant::now(),window:ActionWindow::default(),saved:None}}
}
impl ActionGuard {
    pub fn update(&mut self,chr:&mut eldenring::cs::ChrIns,active:bool) {
        let owner=std::ptr::from_ref(chr) as usize;
        if self.saved.is_some_and(|(address,handle,_,_)|address!=owner||handle!=chr.field_ins_handle) {self.saved=None;}
        if self.window.blocked(self.clock.elapsed(),active) {
            self.saved.get_or_insert((owner,chr.field_ins_handle,chr.debug_flags.disabled_secondary_actions(),chr.debug_flags.disabled_movement()));
            chr.debug_flags.set_disabled_secondary_actions(true);
            chr.debug_flags.set_disabled_movement(true);
        } else if let Some((_,_,actions,movement))=self.saved.take() {
            chr.debug_flags.set_disabled_secondary_actions(actions);
            chr.debug_flags.set_disabled_movement(movement);
        }
    }
}

pub fn capture_route(visible: bool, player_present: bool, view: &crate::runtime::View) -> bool {
    if crate::debug_menu::open(){return player_present;}
    if crate::run_screen::active(){return player_present;}
    if view.room.is_some()||view.shared_event {
        return false;
    }
    visible
        && player_present
        && !view.travelling
        && view
            .run
            .as_ref()
            .is_none_or(|run| !matches!(run.phase, erdtree_tower::Phase::Combat { .. }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_guard_combat_releases_stale_overlay_and_dialog_locks() {
        for overlay in [false,true] {for dialog in [false,true] {
            assert!(!super::capture_actions(true,overlay,dialog));
            assert_eq!(super::capture_actions(false,overlay,dialog),overlay||dialog);
        }}
    }
    use super::*;
    #[test]
    fn menu_actions_stay_blocked_through_the_closing_click_then_release() {
        use std::time::Duration as D;
        let mut window=ActionWindow::default();
        assert!(!window.blocked(D::ZERO,false));
        assert!(window.blocked(D::from_secs(1),true));
        assert!(window.blocked(D::from_secs(8),true));
        assert!(window.blocked(D::from_millis(8349),false));
        assert!(!window.blocked(D::from_millis(8350),false));
        assert!(window.blocked(D::from_secs(9),true));
    }
    #[test]
    fn native_room_menus_never_capture_game_input() {
        let mut view = crate::runtime::View {
            shared_event:false,
            ballots:vec![],
            runes: 0,
            boss_affixes: None,
            boss_alert: None,
            boss_affix_list: vec![],
            law_clock: None,
            build: vec![],
            dummy: None,
            stats: Default::default(),
            run: None,
            status: String::new(),
            busy: false,
            enemy_hp: None,
            prompt: 0,
            ready: true,
            travelling: false,
            room: Some(crate::rooms::RoomView {
                message: String::new(),
                loading: false,
                completed: false,
            }),
        };
        assert!(!capture_route(true, true, &view));
        assert!(!capture_route(false, true, &view));
        assert!(!capture_route(false, false, &view));
        view.room.as_mut().unwrap().loading = true;
        assert!(!capture_route(true, true, &view));
    }
    #[test]
    fn initialization_blocks_actions_but_keeps_cursor_available() {
        let mut view = crate::runtime::View {
            shared_event:false,
            ballots:vec![],
            runes: 0,
            boss_affixes: None,
            boss_alert: None,
            boss_affix_list: vec![],
            law_clock: None,
            build: vec![],
            dummy: None,
            stats: Default::default(),
            run: None,
            status: String::new(),
            busy: true,
            enemy_hp: None,
            prompt: 0,
            ready: false,
            travelling: false,
            room: None,
        };
        assert!(capture_route(true, true, &view));
        view.shared_event=true;
        assert!(!capture_route(true,true,&view));
        view.shared_event=false;
        assert!(!capture_route(true, false, &view));
        assert!(!capture_route(false, true, &view));
        view.travelling = true;
        assert!(!capture_route(true, true, &view));
        view.travelling = false;
        let mut run = erdtree_tower::Run::new(42, erdtree_tower::Class::Samurai);
        run.enter(0).unwrap();
        view.run = Some(run);
        assert!(!capture_route(true, true, &view));
    }
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetCursorPos(point:*mut DesktopPoint)->i32;
    fn GetForegroundWindow() -> isize;
}

#[repr(C)]
#[derive(Default)]
struct DesktopPoint {x:i32,y:i32}
pub fn desktop_pointer()->Option<[i32;2]> {
    let mut point=DesktopPoint::default();
    (unsafe{GetCursorPos(&mut point)}!=0).then_some([point.x,point.y])
}

pub fn active() -> bool {
    CAPTURE.load(Ordering::Relaxed)
        && WINDOW.load(Ordering::Relaxed) != 0
        && unsafe { GetForegroundWindow() } == WINDOW.load(Ordering::Relaxed)
}

unsafe extern "system" fn cursor_position(x: i32, y: i32) -> i32 {
    if active() {
        return 1;
    }
    let original: unsafe extern "system" fn(i32, i32) -> i32 =
        unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(x, y) }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const i8) -> *mut c_void;
}
// [fix] Hook the real user32!SetCursorPos. With the MinGW (windows-gnu) toolchain, `SetCursorPos as
// *mut c_void` is the address of a local import thunk inside this DLL, so hooking it only caught this
// DLL's own calls: the game kept re-centring the cursor while reward/oath screens were open.
fn user32_set_cursor_pos() -> Result<*mut c_void, String> {
    let name: Vec<u16> = "user32.dll".encode_utf16().chain(std::iter::once(0)).collect();
    let module = unsafe { GetModuleHandleW(name.as_ptr()) };
    if module.is_null() { return Err("Cursor hook: user32.dll not loaded".into()); }
    let address = unsafe { GetProcAddress(module, c"SetCursorPos".as_ptr()) };
    if address.is_null() { return Err("Cursor hook: SetCursorPos not found".into()); }
    Ok(address)
}

#[test]
fn cursor_hook_resolves_loaded_system_export() {
    assert!(!user32_set_cursor_pos().unwrap().is_null());
}
pub unsafe fn install() -> Result<(), String> {
    let address = user32_set_cursor_pos()?;
    let hook = unsafe { MhHook::new(address, cursor_position as *mut c_void) }
        .map_err(|error| format!("Cursor hook: {error:?}"))?;
    ORIGINAL.store(hook.trampoline() as usize, Ordering::Release);
    unsafe { MH_EnableHook(address) }
        .ok()
        .map_err(|error| format!("Cursor enable: {error:?}"))?;
    Ok(())
}

#[derive(Default)]
pub struct PadCapture {
    saved: Option<(usize, bool)>,
}

impl PadCapture {
    pub fn update(&mut self, capture: bool) {
        let Ok(manager) = (unsafe { FD4PadManager::instance_mut() }) else {
            return;
        };
        let Some(pad) = manager.get_in_game_pad_mut() else {
            return;
        };
        let address = std::ptr::from_ref(pad) as usize;
        if self.saved.is_some_and(|(owner, _)| owner != address) {
            self.saved = None;
        }
        if capture {
            self.saved.get_or_insert((address, pad.allow_polling));
            pad.allow_polling = false;
        } else if let Some((_, previous)) = self.saved.take() {
            pad.allow_polling = previous;
        }
    }
}
