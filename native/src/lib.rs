use std::{
    ffi::c_void,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    sync::{LazyLock, Mutex},
    time::Duration,
};

use eldenring::{
    cs::{CSTaskGroupIndex, CSTaskImp, PlayerIns},
    fd4::FD4TaskData,
    util::system::wait_for_system_init,
};
use erdtree_tower::{Class, Phase, Run};
use fromsoftware_shared::{GameVersion, LANG_ID_EN, SharedTaskImpExt, program::Program};
use hudhook::{
    Hudhook, ImguiRenderLoop, MessageFilter, RenderContext,
    hooks::dx12::ImguiDx12Hooks,
    imgui::{Context, FontConfig, FontGlyphRanges, FontSource, Io, Key, Ui},
    windows::Win32::Foundation::HINSTANCE,
};

mod blessings;
mod enemies;
mod chariot;
mod map_guards;
mod boss_bars;
mod flasks;
mod physick;
mod input;
mod hub;
mod items;
mod loot;
mod loadout;
mod level_cost;
mod map_ui;
mod map_ink;
mod map_art;
mod oath_art;
mod native_shop;
mod native_talk;
mod reward_delay;
mod rebirth;
mod elite;
mod boss_affixes;
mod scaling;
mod reward_ui;
mod contracts;
mod rooms;
mod scarabs;
mod map_scarabs;
mod chest_award;
mod smith;
mod spirits;
mod bloodstain;
mod run_screen;
mod runtime;
mod party;
mod travel;
mod grace_travel;
mod arenas;
mod weapons;
mod seamless_probe;
mod seamless;
mod steam_messages;
mod passives;
mod star_stream;
mod bgm;
mod ailments;
mod diagnostics;
mod render_diagnostics;
mod rest_end;
mod party_death;
mod seamless_revival;

#[derive(Clone, Copy)]
struct Snapshot {
    hp: i32,
    max_hp: i32,
    fp: i32,
    max_fp: i32,
    archetype: u8,
}

static SNAPSHOT: LazyLock<Mutex<Option<Snapshot>>> = LazyLock::new(|| Mutex::new(None));
static LOG_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::var_os("ERDTREE_TOWER_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("tower-preview.log")
});

fn log(message: impl std::fmt::Display) {
    diagnostics::write(message);
}
fn log_repeating(message: impl std::fmt::Display) {
    static SEEN:std::sync::Mutex<Vec<(String,std::time::Instant)>>=std::sync::Mutex::new(Vec::new());
    let message=message.to_string();let now=std::time::Instant::now();
    let Ok(mut seen)=SEEN.lock() else{return log(message);};
    seen.retain(|(_,at)|now.duration_since(*at)<std::time::Duration::from_secs(30));
    if seen.iter().any(|(m,_)|*m==message){return;}
    if seen.len()<32{seen.push((message.clone(),now));}
    log(message);
}

enum SupportedVersion {
    Ww2720,
}

impl GameVersion for SupportedVersion {
    const NAME: &'static str = "elden ring";
    fn from_lang_version(lang: u16, version: &str) -> Option<Self> {
        (lang == LANG_ID_EN && version == "2.7.2.0").then_some(Self::Ww2720)
    }
}

fn capture(_: &FD4TaskData) {
    diagnostics::task();
    runtime::tick();
    static FIRST_CAPTURE: AtomicBool = AtomicBool::new(true);
    static FIRST_PLAYER: AtomicBool = AtomicBool::new(true);
    if FIRST_CAPTURE.swap(false, Ordering::Relaxed) {
        log("Game task callback active");
    }
    // Reads stay on the game's task thread; renderer receives only copied scalars.
    let snapshot = unsafe { PlayerIns::local_player().ok() }.and_then(|player| {
        let _=level_cost::configure(unsafe {player.player_game_data.as_ref()}.archetype);
        let data = &player.chr_ins.modules.data;
        if data.max_hp <= 0 || data.max_hp > 1_000_000 || data.hp < 0 || data.hp > data.max_hp {
            return None;
        }
        Some(Snapshot {
            hp: data.hp,
            max_hp: data.max_hp,
            fp: data.fp,
            max_fp: data.max_fp,
            archetype: unsafe { player.player_game_data.as_ref() }.archetype,
        })
    });
    if let Some(player) = snapshot
        && FIRST_PLAYER.swap(false, Ordering::Relaxed)
    {
        log(format!(
            "Player detected: archetype={} hp={}/{}",
            player.archetype, player.hp, player.max_hp
        ));
    }
    if let Ok(mut guard) = SNAPSHOT.try_lock() {
        *guard = snapshot;
    }
}

fn class_from_archetype(archetype: u8) -> Option<Class> {
    match archetype {
        6=>Some(Class::Confessor),8=>Some(Class::Prisoner),
        _=>Class::ALL.get(archetype as usize).copied(),
    }
}

struct TowerPreview {
    diagnostics:diagnostics::Changes,
    rewards: reward_ui::Panel,
    visible: bool,
    run: Option<Run>,
    last_archetype: Option<u8>,
    last_prompt: u64,
}

impl TowerPreview {
    fn new() -> Self {
        Self {
            diagnostics:Default::default(),
            rewards: reward_ui::Panel::default(),
            visible: true,
            run: Some(Run::new(42, Class::Vagabond)),
            last_archetype: None,
            last_prompt: 0,
        }
    }
}

impl ImguiRenderLoop for TowerPreview {
    fn before_render(&mut self, ctx: &mut Context, renderer: &mut dyn RenderContext) {
        self.rewards.load_icons(renderer);
        let player_present = SNAPSHOT.lock().is_ok_and(|snapshot| snapshot.is_some());
        let capture = runtime::with_view(|view| input::capture_route(self.visible, player_present, view)).unwrap_or(false);
        input::CAPTURE.store(capture, Ordering::Relaxed);
        ctx.io_mut().mouse_draw_cursor = capture;
        let map=runtime::with_view(|view|view.run.as_ref().is_some_and(|run|
            matches!(run.phase,Phase::Map)&&!run.contract_pending())).unwrap_or(false);
        reward_ui::poll_pad(ctx.io_mut(),capture&&input::active(),map);
        let io = ctx.io_mut();
        if capture {
            let mut point = io.mouse_pos;
            for axis in 0..2 {
                let end = (io.display_size[axis] - 1.0).max(0.0);
                point[axis] = if point[axis].is_finite() && point[axis] > -1.0e10 {
                    point[axis].clamp(0.0, end)
                } else {
                    end * 0.5
                };
            }
            if point != io.mouse_pos {
                io.add_mouse_pos_event(point);
            }
        }
    }

    fn before_wnd_proc(
        &self,
        hwnd: hudhook::windows::Win32::Foundation::HWND,
        _: u32,
        _: hudhook::windows::Win32::Foundation::WPARAM,
        _: hudhook::windows::Win32::Foundation::LPARAM,
    ) -> hudhook::BeforeWndProc {
        input::WINDOW.store(hwnd.0 as isize, Ordering::Relaxed);
        hudhook::BeforeWndProc::Continue
    }

    fn message_filter(&self, _: &Io) -> MessageFilter {
        let player_present = SNAPSHOT.lock().is_ok_and(|snapshot| snapshot.is_some());
        if runtime::with_view(|view| input::capture_route(self.visible, player_present, view)).unwrap_or(false)
        {
            MessageFilter::InputAll
        } else {
            MessageFilter::empty()
        }
    }

    fn initialize(&mut self, ctx: &mut Context, renderer: &mut dyn RenderContext) {
        log("UI_INIT stage=rewards");
        self.rewards.initialize(renderer);
        log("UI_INIT stage=map_art");
        map_art::initialize(renderer);
        log("UI_INIT stage=oath_art");
        oath_art::initialize(renderer);
        log("DX12 renderer initialized");
        if let Ok(font) = std::fs::read("C:/Windows/Fonts/simhei.ttf") {
            ctx.fonts().add_font(&[FontSource::TtfData {
                data: &font,
                size_pixels: 20.0,
                config: Some(FontConfig {
                    glyph_ranges: FontGlyphRanges::chinese_full(),
                    ..Default::default()
                }),
            }]);
        }
    }

    fn render(&mut self, ui: &mut Ui) {
        diagnostics::frame();
        if ui.is_key_pressed_no_repeat(Key::Home) {
            if runtime::PLAYER_READY.load(Ordering::Relaxed) {
                runtime::QUICK_END.store(true,Ordering::Relaxed);
                log("Home: quick settlement requested");
            }else{log("Home: ignored before a character is loaded");}
        }
        static FIRST_FRAME: AtomicBool = AtomicBool::new(true);
        if FIRST_FRAME.swap(false, Ordering::Relaxed) {
            log("First overlay frame rendered");
        }
        let view = runtime::view();
        let map_open=self.visible&&view.as_ref().and_then(|v|v.run.as_ref()).or(self.run.as_ref()).is_some_and(|r|matches!(r.phase,Phase::Map));
        if ui.is_key_pressed_no_repeat(Key::F10)&&!map_open {
            self.visible = !self.visible;
            log(format!("F10: visible={}", self.visible));
        }
        let snapshot = SNAPSHOT.lock().ok().and_then(|guard| *guard);
        if let Some(snapshot) = snapshot
            && self.last_archetype != Some(snapshot.archetype)
        {
            self.last_archetype = Some(snapshot.archetype);
            self.run = class_from_archetype(snapshot.archetype).map(|class| Run::new(42, class));
            log(format!("Class binding: archetype={}", snapshot.archetype));
        }
        let run_screen=run_screen::draw(ui);
        if self.visible&&!run_screen&&let Some(title)=view.as_ref().filter(|v|v.run.as_ref().is_some_and(|r|matches!(r.phase,Phase::Combat{..}))).and_then(|v|v.boss_affixes.as_ref()) {
            let draw=ui.get_foreground_draw_list();let size=ui.calc_text_size(title);let x=(ui.io().display_size[0]-size[0])*0.5;
            draw.add_text([x+1.5,31.5],hudhook::imgui::ImColor32::from_rgba(0,0,0,200),title);
            draw.add_text([x,30.0],hudhook::imgui::ImColor32::from_rgb(232,190,92),title);
        }
        let key=format!("UI visible={} snapshot={} capture={} settlement={} view={:?}",self.visible,snapshot.is_some(),input::CAPTURE.load(Ordering::Relaxed),run_screen,view.as_ref().map(|v|(v.ready,v.busy,v.travelling,v.room.is_some(),v.shared_event,v.prompt)));
        if self.diagnostics.due(key.clone(),std::time::Instant::now()){log(key);}
        if run_screen {return;}
        if let Some(view) = &view {
            if view.room.is_some()||view.shared_event {
                return;
            }
            if view.travelling {
                input::CAPTURE.store(false, Ordering::Relaxed);
                return;
            }
        }
        if let Some(view) = &view
            && self.last_prompt != view.prompt
        {
            self.last_prompt = view.prompt;
            self.visible = !view
                .run
                .as_ref()
                .is_some_and(|run| matches!(run.phase, Phase::Combat { .. }));
        }
        if !self.visible {
            return;
        }
        let active_run = view
            .as_ref()
            .and_then(|view| view.run.as_ref())
            .or(self.run.as_ref());
        if self.visible && ui.is_key_pressed(Key::F6) {
            match active_run.map(|run| &run.phase) {
                Some(Phase::Map) => {
                    if let Some(id) = active_run.and_then(|run| run.available().first().copied()) {
                        runtime::submit(runtime::Action::Enter(id));
                    }
                }
                _ => {}
            }
        }
        let combat = active_run.is_some_and(|run| matches!(run.phase, Phase::Combat { .. }));
        if !combat {
            if let (Some(run), Some(view)) = (active_run, view.as_ref()) {
                if snapshot.is_some() {
                    let vitals = snapshot
                        .map(|s| {
                            format!("生命 {}/{}    专注 {}/{}", s.hp, s.max_hp, s.fp, s.max_fp)
                        })
                        .unwrap_or_default();
                    if run.contract.as_ref().is_some_and(|c|c.selection_pending()) {self.rewards.draw_contract(ui,run,view);}
                    else if run.contract_pending()||matches!(run.phase,Phase::Reward{..}) {self.rewards.draw(ui,run,view);}
                    else if run.story_index().is_ok() {self.rewards.draw_story(ui,run,view);}
                    else if map_ui::draw(ui, run, view, &vitals) {self.visible=false;}
                }
            }
            return;
        }
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// Called by the Windows loader; the module remains loaded while tasks are registered.
pub unsafe extern "system" fn DllMain(module: HINSTANCE, reason: u32, _: *mut c_void) -> i32 {
    if reason != 1 {
        return 1;
    }
    let address = module.0 as usize;
    std::thread::spawn(move || {
        diagnostics::startup(address);
        diagnostics::watchdog();
        if std::env::var("ERDTREE_TOWER_PREVIEW").as_deref() != Ok("1") {
            log("Initialization stopped: launcher marker missing");
            return;
        }
        let program = Program::current();
        let Program::Mapping(mapping) = program else {
            log("Initialization stopped: executable mapping unavailable");
            return;
        };
        if let Err(error) = SupportedVersion::detect(&mapping) {
            log(format!("Initialization stopped: {error}"));
            return;
        }
        if seamless_probe::enabled(std::env::var("ERDTREE_TOWER_SEAMLESS_PROBE").ok().as_deref()) {
            log("Read-only Seamless probe: tower runtime, hooks, resets and item writes are disabled");
            if let Err(error)=wait_for_system_init(&program,Duration::from_secs(60)){log(format!("Probe initialization failed: {error}"));return;}
            match CSTaskImp::wait_for_instance(Duration::from_secs(30)) {
                Ok(task)=>{let recurring=task.run_recurring(seamless_probe::capture as fn(&FD4TaskData),CSTaskGroupIndex::ChrIns_PostPhysics);std::mem::forget(recurring);log("Read-only Seamless probe registered");},
                Err(error)=>log(format!("Probe scheduler unavailable: {error}")),
            }
            return;
        }
        render_diagnostics::install();
        log("Executable version accepted; installing DX12 hooks");
        let result = Hudhook::builder()
            .with::<ImguiDx12Hooks>(TowerPreview::new())
            .with_hmodule(HINSTANCE(address as _))
            .build()
            .apply();
        if let Err(error) = result {
            log(format!("DX12 hook installation failed: {error:?}"));
            return;
        }
        log("DX12 hooks installed; waiting for game systems");
        if render_diagnostics::enabled() {
            log("Render-only diagnostic: tower gameplay callbacks and character resets disabled; remain at title screen");
            return;
        }
        if let Err(error) = unsafe { input::install() } {
            log(error);
        }
        if let Err(error)=unsafe {level_cost::install()} {log(format!("Linear leveling unavailable: {error}"));}
        if let Err(error)=unsafe {smith::install()} {log(format!("Smith reinforcement unavailable: {error}"));}
        if let Err(error)=unsafe {spirits::install()} {log(format!("Spirit use trace unavailable: {error}"));}
        if let Err(error)=unsafe {star_stream::install()} {log(format!("Star stream unavailable: {error}"));}
        if let Err(error) = wait_for_system_init(&program, Duration::from_secs(60)) {
            log(format!("Game system initialization failed: {error}"));
            return;
        }
        let task = match CSTaskImp::wait_for_instance(Duration::from_secs(30)) {
            Ok(task) => task,
            Err(error) => {
                log(format!("Game scheduler unavailable: {error}"));
                return;
            }
        };
        let recurring = task.run_recurring(
            capture as fn(&FD4TaskData),
            CSTaskGroupIndex::ChrIns_PostPhysics,
        );
        std::mem::forget(recurring);
        log("Character capture task registered");
        if let Err(error)=unsafe {seamless_revival::install()} {log(format!("PARTY_REVIVE unavailable: {error}"));}
    });
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn class_ids_follow_paramdex() {
        assert_eq!(class_from_archetype(7), Some(Class::Samurai));
        assert_eq!(class_from_archetype(0), Some(Class::Vagabond));
        assert_eq!(class_from_archetype(4), Some(Class::Astrologer));
        assert_eq!(class_from_archetype(5), Some(Class::Prophet));
        for (archetype,class) in [(1,Class::Warrior),(2,Class::Hero),(3,Class::Bandit),(6,Class::Confessor),(8,Class::Prisoner),(9,Class::Wretch)] {
            assert_eq!(class_from_archetype(archetype),Some(class));
        }
        for archetype in [10, 255] {
            assert_eq!(class_from_archetype(archetype), None);
        }
    }
    #[test]
    fn map_preview_exists_before_player_is_available() {
        let preview = TowerPreview::new();
        assert!(preview.visible);
        assert_eq!(preview.run.unwrap().class, Class::Vagabond);
        assert_eq!(preview.last_archetype, None);
    }
    #[test]
    fn ww_2720_world_layout_matches_native_constructor() {
        use eldenring::cs::WorldChrMan;
        assert_eq!(std::mem::offset_of!(WorldChrMan, main_player), 0x1e508);
        assert_eq!(std::mem::offset_of!(WorldChrMan, net_chr_sync), 0x1e5e0);
        assert_eq!(std::mem::offset_of!(WorldChrMan, debug_chr_creator), 0x1e658);
        assert_eq!(std::mem::offset_of!(WorldChrMan, chr_cam), 0x1ecf0);
        let mut world = std::mem::MaybeUninit::<WorldChrMan>::zeroed();
        unsafe {
            let bytes = world.as_mut_ptr().cast::<u8>();
            bytes.add(0x1e658).cast::<usize>().write(0x12345000);
            let field = std::ptr::addr_of!((*world.as_ptr()).debug_chr_creator);
            assert_eq!(field.cast::<usize>().read(), 0x12345000);
        }
    }
    #[test]
    fn unknown_versions_are_rejected() {
        assert!(SupportedVersion::from_lang_version(LANG_ID_EN, "2.7.2.0").is_some());
        assert!(SupportedVersion::from_lang_version(LANG_ID_EN, "2.7.1.0").is_none());
        assert!(SupportedVersion::from_lang_version(0x11, "2.7.1.0").is_none());
    }
}
