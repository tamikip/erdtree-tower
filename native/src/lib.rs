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
use erdtree_tower::{Class, Phase, Run, options::Hotkey};
use fromsoftware_shared::{GameVersion, LANG_ID_EN, SharedTaskImpExt, program::Program};
use hudhook::{
    Hudhook, ImguiRenderLoop, MessageFilter, RenderContext,
    hooks::dx12::ImguiDx12Hooks,
    imgui::{Context, FontConfig, FontGlyphRanges, FontSource, Io, Key, Ui},
    windows::Win32::Foundation::HINSTANCE,
};

mod blessings;
mod suspension;
mod frost_shackle;
mod pad;
mod retry_item;
mod spell_affixes;
mod retry_rescue;
mod options;
mod gpu_engine;
mod overlay_gpu;
mod overlay_keys;
mod recovery;
mod native_settings;
mod native_settings_keys;
mod radahn_probe;
mod radahn_entity;
mod mausoleum;
mod stats;
mod enemies;
mod chariot;
mod map_guards;
mod merchant;
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
mod party_compat;
mod map_ink;
mod map_art;
mod oath_art;
mod native_shop;
mod native_talk;
mod reward_delay;
mod rebirth;
mod elite;
mod boss_affixes;
mod fancy_text;
mod scaling;
#[allow(dead_code)]
mod law;
mod embers;
mod debug_menu;
mod run_log;
mod build_panel;
mod archive_ui;
mod cosmetic_unlocks;
mod transmog;
mod transmog_ui;
mod archive_style;
mod weapon_ui;
mod class_ui;
mod legacy;
mod law_ui;
mod reward_ui;
mod random_ashes;
mod contracts;
mod rooms;
mod scarabs;
mod i18n;
mod map_scarabs;
mod chest_award;
mod ember_notice;
mod smith;
mod whetblades;
mod parkour_barrier;
mod event_hosts;
mod spirits;
mod bloodstain;
mod run_screen;
mod runtime;
mod party;
mod party_latency;
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
mod landing_probe;
mod render_diagnostics;
mod rest_end;
mod party_death;
mod seamless_revival;
mod weapon_glow;
mod dummy;
mod name_tags;
mod auras;
mod keep_runes;
mod throw_guard;
mod lobby;
mod material_award;
mod law_art;
mod scenery;
mod spirit_tune;
mod dual_bosses;
mod affix_stack;
mod sfx;

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

fn class_from_archetype(archetype: u8) -> Option<Class> {Class::from_archetype(archetype)}

struct TowerPreview {
    diagnostics:diagnostics::Changes,
    rewards: reward_ui::Panel,
    debug: debug_menu::Menu,
    visible: bool,
    run: Option<Run>,
    last_archetype: Option<u8>,
    last_prompt: u64,
    /// 正在显示的场地文件提示：(提示序号, 开始显示的时间)
    resource_alert: Option<(u64,std::time::Instant)>,
}

/// 整合包场地文件不完整的提示：屏幕上方一块醒目的面板（只画不接收输入，任何界面下都能看到）
fn draw_resource_alert(ui:&hudhook::imgui::Ui,files:&[String]) {
    use hudhook::imgui::ImColor32;
    let mut lines=vec![format!("{} 个场地文件缺失或与当前模组版本不一致，进入这些战斗会被拒绝；联机时队友出发后你会留在原地：",files.len())];
    lines.extend(files.iter().take(4).map(|f|format!("· {f}")));
    if files.len()>4 {lines.push(format!("…… 共 {} 个",files.len()));}
    lines.push("请关闭游戏，在启动器里点「校验并修复文件」，再从启动器启动。".into());
    let title="整合包文件不完整";
    let size=ui.io().display_size;let line_h=ui.text_line_height_with_spacing();
    let width=lines.iter().map(|l|ui.calc_text_size(crate::i18n::t(l))[0]).fold(ui.calc_text_size(crate::i18n::t(title))[0],f32::max)+48.0;
    let height=line_h*(lines.len() as f32+1.6)+32.0;
    let (x,y)=((size[0]-width)*0.5,size[1]*0.1);
    let draw=ui.get_foreground_draw_list();
    draw.add_rect([x,y],[x+width,y+height],ImColor32::from_rgba(18,10,10,235)).filled(true).rounding(6.0).build();
    draw.add_rect([x,y],[x+width,y+height],ImColor32::from_rgb(214,82,64)).rounding(6.0).thickness(2.0).build();
    draw.add_text([x+24.0,y+16.0],ImColor32::from_rgb(240,120,96),crate::i18n::t(title));
    for (i,line) in lines.iter().enumerate() {
        draw.add_text([x+24.0,y+16.0+line_h*(i as f32+1.6)],ImColor32::from_rgb(236,226,210),crate::i18n::t(line));
    }
}

impl TowerPreview {
    fn new() -> Self {
        Self {
            diagnostics:Default::default(),
            rewards: reward_ui::Panel::default(),
            debug: Default::default(),
            visible: true,
            run: Some(Run::new(42, Class::Vagabond)),
            last_archetype: None,
            last_prompt: 0,
            resource_alert: None,
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
        fancy_text::reset_fonts();
        log("UI_INIT stage=rewards");
        self.rewards.initialize(renderer);
        log("UI_INIT stage=map_art");
        map_art::initialize(renderer);
        log("UI_INIT stage=oath_art");
        oath_art::initialize(renderer);
        law_art::initialize(renderer);
        log("DX12 renderer initialized");
        // 正文与标题字体都只加载界面字表里的字（缺字表时退回原来的范围），字体贴图不会因为多一套字体而翻倍
        let ranges=|fallback:FontGlyphRanges|fancy_text::ui_glyph_ranges().map_or(fallback,FontGlyphRanges::from_slice);
        // Rasterize near the displayed size, keeping existing logical font sizes and layout.
        let [width,height]=ctx.io().display_size;
        let density=(width/1600.0).min(height/900.0).clamp(1.0,2.0);
        let mut font_density=1.0;
        // 正文：思源宋体常规体；字体文件缺失时退回系统黑体
        if let Ok(font) = std::fs::read(fancy_text::body_font_path()).or_else(|error|{log(format!("Body font missing, falling back to SimHei: {error}"));std::fs::read("C:/Windows/Fonts/simhei.ttf")}) {
            ctx.fonts().add_font(&[FontSource::TtfData {
                data: &font,
                size_pixels: 20.0*density,
                config: Some(FontConfig {
                    glyph_ranges: ranges(FontGlyphRanges::chinese_full()),
                    rasterizer_multiply: 1.15,
                    ..Default::default()
                }),
            }]);
            ctx.io_mut().font_global_scale=1.0/density;
            font_density=density;
        }
        // 标题字体：思源宋体子集，按 32 像素加载，供带动态特效的标题文字使用
        match std::fs::read(fancy_text::title_font_path()) {
            Ok(font)=>{
                fancy_text::set_title_font(fancy_text::TitleFont(ctx.fonts().add_font(&[FontSource::TtfData {
                    data: &font,
                    size_pixels: 32.0*font_density,
                    config: Some(FontConfig {glyph_ranges: ranges(FontGlyphRanges::chinese_simplified_common()),..Default::default()}),
                }])));
            },
            Err(error)=>log(format!("Title font missing: {error}")),
        }
        // 结算页黑体：思源黑体粗体的子集（约一千字），正文按 26 像素加载；大标题按 48 像素只加载标题用字；大号数字与评级字母另按 220 像素只加载这几个字
        match std::fs::read(fancy_text::button_font_path()) {
            Ok(font)=>{
                fancy_text::set_button_font(fancy_text::TitleFont(ctx.fonts().add_font(&[FontSource::TtfData {
                    data: &font,
                    // 和正文一样按屏幕密度加载（全局缩放是 1/密度），高分辨率下不糊、也不会变小
                    size_pixels: fancy_text::SANS_PIXELS*font_density,
                    // 字体里只有子集里的字，按全范围加载也只栅格化这些字
                    config: Some(FontConfig {glyph_ranges: FontGlyphRanges::chinese_full(),..Default::default()}),
                }])));
                fancy_text::set_heading_font(fancy_text::TitleFont(ctx.fonts().add_font(&[FontSource::TtfData {
                    data: &font,
                    size_pixels: fancy_text::HEADING_PIXELS*font_density,
                    config: Some(FontConfig {glyph_ranges: FontGlyphRanges::from_slice(fancy_text::heading_ranges()),..Default::default()}),
                }])));
                fancy_text::set_display_font(fancy_text::TitleFont(ctx.fonts().add_font(&[FontSource::TtfData {
                    data: &font,
                    size_pixels: 220.0,
                    config: Some(FontConfig {glyph_ranges: FontGlyphRanges::from_slice(&fancy_text::DISPLAY_CHARS),..Default::default()}),
                }])));
            },
            Err(error)=>log(format!("Summary font missing: {error}")),
        }
    }

    fn render(&mut self, ui: &mut Ui) {
        diagnostics::frame();
        native_settings::begin_frame();
        if options::pressed(ui,Hotkey::Settle) {
            if runtime::PLAYER_READY.load(Ordering::Relaxed) {
                runtime::QUICK_END.store(true,Ordering::Relaxed);
                log("Home: quick settlement requested");
            }else{log("Home: ignored before a character is loaded");}
        }
        if !ui.io().want_text_input&&options::pressed(ui,Hotkey::Recover) {runtime::RECOVER.store(true,Ordering::Relaxed);}
        if options::pressed(ui,Hotkey::Skip) {
            runtime::SKIP.store(true,Ordering::Relaxed);
        }
        static FIRST_FRAME: AtomicBool = AtomicBool::new(true);
        if FIRST_FRAME.swap(false, Ordering::Relaxed) {
            log("First overlay frame rendered");
        }
        let view = runtime::view();
        suspension::draw(ui,view.as_ref());
        let map_open=self.visible&&view.as_ref().and_then(|v|v.run.as_ref()).or(self.run.as_ref()).is_some_and(|r|matches!(r.phase,Phase::Map));
        if options::pressed(ui,Hotkey::Overlay)&&!map_open {
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
        // J：旅程档案（往期旅程与图鉴），战斗中不响应；档案打开时同一个键负责关闭
        let fighting=view.as_ref().and_then(|v|v.run.as_ref()).is_some_and(|r|matches!(r.phase,Phase::Combat{..}));
        if !run_screen::active()&&options::pressed(ui,Hotkey::Archive)&&!fighting&&!ui.io().want_text_input&&runtime::PLAYER_READY.load(Ordering::Relaxed) {run_screen::archive_entry();}
        let rewards=&mut self.rewards;
        let run_screen=run_screen::draw(ui,&mut |icon|rewards.icon(icon));
        if options::pressed(ui,Hotkey::Debug) {debug_menu::toggle();}
        self.debug.draw(ui,view.as_ref());
        if self.visible&&!run_screen&&let Some(combat)=view.as_ref().filter(|v|v.run.as_ref().is_some_and(|r|matches!(r.phase,Phase::Combat{..}))) {
            let draw=ui.get_foreground_draw_list();
            let line=|text:&str,y:f32,color:hudhook::imgui::ImColor32| {
                let x=(ui.io().display_size[0]-ui.calc_text_size(crate::i18n::t(text))[0])*0.5;
                draw.add_text([x+1.5,y+1.5],hudhook::imgui::ImColor32::from_rgba(0,0,0,200),crate::i18n::t(text));draw.add_text([x,y],color,crate::i18n::t(text));
            };
            let mut y=30.0;
            // Boss 词条：每个词条用自己的颜色与动态效果（与 Boss 身上的光效同色）
            if !combat.boss_affix_list.is_empty() {
                let font=fancy_text::title_font().map(|f|ui.push_font(f));
                let size=if font.is_some(){32.0}else{ui.current_font_size()*1.6};
                let parts:Vec<_>=combat.boss_affix_list.iter().map(|a|(a.name(),fancy_text::affix_style(*a))).collect();
                fancy_text::draw_line(fancy_text::Layer::Foreground,ui.io().display_size[0]*0.5,y-2.0,size,&parts,(" · ",[150,140,120]),ui.time() as f32);
                if let Some(font)=font {font.pop();}
                y+=size+6.0;
            } else if let Some(title)=&combat.boss_affixes {line(title,y,hudhook::imgui::ImColor32::from_rgb(232,190,92));y+=24.0;}
            // 黄金律法的护盾：展开前橙色预警，展开中金色倒计时
            if let Some((text,warning))=&combat.boss_alert {line(text,y,if *warning{hudhook::imgui::ImColor32::from_rgb(240,140,70)}else{hudhook::imgui::ImColor32::from_rgb(255,226,140)});y+=24.0;}
            if let Some(left)=combat.law_clock {
                let (text,color)=if left>0 {(format!("苦难律法 · 限时 {}:{:02}",left/60,left%60),if left<=30{hudhook::imgui::ImColor32::from_rgb(235,110,90)}else{hudhook::imgui::ImColor32::from_rgb(232,214,180)})}
                    else {(format!("苦难律法 · 已超时 {}:{:02} · 持续失血",-left/60,-left%60),hudhook::imgui::ImColor32::from_rgb(235,80,70))};
                line(&text,y,color);
            }
        }
        let key=format!("UI visible={} snapshot={} capture={} settlement={} view={:?}",self.visible,snapshot.is_some(),input::CAPTURE.load(Ordering::Relaxed),run_screen,view.as_ref().map(|v|(v.ready,v.busy,v.travelling,v.room.is_some(),v.shared_event,v.prompt)));
        if self.diagnostics.due(key.clone(),std::time::Instant::now()){log(key);}
        // 整合包场地文件不完整：屏幕上方醒目提示 20 秒（开局检查一次，之后每次出发被拒再提示）
        if let Some((id,files))=view.as_ref().and_then(|v|v.resource_alert.as_ref()) {
            if self.resource_alert.is_none_or(|(seen,_)|seen!=*id) {self.resource_alert=Some((*id,std::time::Instant::now()));}
            if self.resource_alert.is_some_and(|(_,at)|at.elapsed()<std::time::Duration::from_secs(20)) {draw_resource_alert(ui,files);}
        }
        if run_screen {return;}
        if let Some(view) = &view {
            if let Some(dummy)=&view.dummy {dummy::draw_hud(ui,dummy);}
            if view.room.is_some()||view.shared_event {
                return;
            }
            if view.travelling||view.run.as_ref().is_some_and(|r|r.retry_node.is_some()) {
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
                    else {
                        map_ui::draw(ui, run, view, &vitals);
                        if build_panel::open() {let rewards=&mut self.rewards;build_panel::draw(ui,run,view,&mut |icon|rewards.icon(icon));}
                    }
                }
            }
            return;
        }
    }
}

fn install_dx12(address:usize)->bool {
    let result=Hudhook::builder().with::<ImguiDx12Hooks>(TowerPreview::new()).with_hmodule(HINSTANCE(address as _)).build().apply();
    match result {Ok(())=>{log("DX12 hooks installed");true},Err(error)=>{log(format!("DX12 hook installation failed: {error:?}"));false}}
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
        options::init(diagnostics::module_path(address));
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
        native_settings::init(diagnostics::module_path(address));
        render_diagnostics::install();
        if options::get().renderer==erdtree_tower::options::Renderer::Window {
            use hudhook::mh::{MH_Initialize,MH_STATUS};
            match unsafe{MH_Initialize()} {
                MH_STATUS::MH_OK|MH_STATUS::MH_ERROR_ALREADY_INITIALIZED=>overlay_gpu::install(address),
                error=>{log(format!("Hook initialization failed: {error:?}"));return;}
            }
        } else if !install_dx12(address) {return;}
        log("Renderer selected; waiting for game systems");
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
        if let Err(error)=unsafe {radahn_entity::install()} {log(format!("Radahn event binding unavailable: {error}"));}
        if let Err(error)=unsafe {embers::install_critical_trace()} {log(format!("Critical stack detection unavailable: {error}"));}
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
        if let Err(error)=unsafe {seamless_revival::install()} {log(format!("PARTY_REVIVE unavailable: {error}"));}
        let recurring = task.run_recurring(
            capture as fn(&FD4TaskData),
            CSTaskGroupIndex::ChrIns_PostPhysics,
        );
        std::mem::forget(recurring);
        log("Character capture task registered");
        transmog::initialize(diagnostics::module_path(address));
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
        for (archetype,class) in [(2,Class::Hero),(6,Class::Confessor)] {
            assert_eq!(class_from_archetype(archetype),Some(class));
        }
        // 职业重做前的剑士、盗贼、囚犯、一贫如洗已移除
        for archetype in [1,3,8,9,10, 255] {
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
