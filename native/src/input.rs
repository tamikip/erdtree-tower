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
pub struct ActionGuard {
    clock:std::time::Instant,
    window:ActionWindow,
    saved:Option<(usize,eldenring::cs::FieldInsHandle,bool)>,
}
impl Default for ActionGuard {
    fn default()->Self {Self{clock:std::time::Instant::now(),window:ActionWindow::default(),saved:None}}
}
impl ActionGuard {
    pub fn update(&mut self,chr:&mut eldenring::cs::ChrIns,active:bool) {
        let owner=std::ptr::from_ref(chr) as usize;
        if self.saved.is_some_and(|(address,handle,_)|address!=owner||handle!=chr.field_ins_handle) {self.saved=None;}
        if self.window.blocked(self.clock.elapsed(),active) {
            self.saved.get_or_insert((owner,chr.field_ins_handle,chr.debug_flags.disabled_secondary_actions()));
            chr.debug_flags.set_disabled_secondary_actions(true);
        } else if let Some((_,_,previous))=self.saved.take() {
            chr.debug_flags.set_disabled_secondary_actions(previous);
        }
    }
}

pub fn capture_route(visible: bool, player_present: bool, view: &crate::runtime::View) -> bool {
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
    fn SetCursorPos(x: i32, y: i32) -> i32;
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

pub unsafe fn install() -> Result<(), String> {
    let address = SetCursorPos as *mut c_void;
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
