use std::{
    ffi::c_void,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use eldenring::{
    cs::{CSTaskGroupIndex, CSTaskImp, PlayerIns},
    fd4::FD4TaskData,
    util::system::wait_for_system_init,
};
use erdtree_tower::{Class, Phase, Run, options::Hotkey};
use fromsoftware_shared::{GameVersion, LANG_ID_EN, LANG_ID_JP, SharedTaskImpExt, program::Program};
use hudhook::{
    Hudhook, ImguiRenderLoop, MessageFilter, RenderContext,
    hooks::dx12::ImguiDx12Hooks,
    imgui::{Context, FontConfig, FontGlyphRanges, FontSource, Io, Ui},
    windows::Win32::Foundation::HINSTANCE,
};

mod blessings;
mod skill_elements;
mod crash_trace;
mod crash_guard;
mod tears;
mod spawn_guard;
mod spawn_grace;
mod glintblade;
mod capture;
mod suspension;
mod frost_shackle;
mod pad;
mod retry_item;
mod spell_affixes;
mod starsurge;
mod stance_trace;
mod stance_floor;
mod retry_rescue;
mod options;
mod gpu_engine;
mod overlay_gpu;
mod overlay_keys;
mod recovery;
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
mod adaptation;
mod leech;
mod fancy_text;
mod scaling;
#[allow(dead_code)]
mod law;
mod embers;
mod debug_menu;
mod gesture_trace;
mod run_log;
mod run_clock;
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
mod dance_ui;
mod custom_event_ui;
mod dance_camera;
mod dance_motion;
mod dance_stage;
mod dance_art;
mod notice_ui;
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
mod reconnect;
mod party_latency;
mod travel;
mod grace_travel;
mod arenas;
mod weapons;
mod seamless_probe;
mod seamless;
mod steam_messages;
mod passives;
mod shadow_archery;
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
mod damage;
mod lobby;
mod material_award;
mod law_art;
mod scenery;
mod spirit_tune;
mod dual_bosses;
mod affix_stack;
mod instance_description;
mod card_affixes;
mod weapon_traits;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SupportedVersion {
    Ww2720,
    Build272,
}

impl GameVersion for SupportedVersion {
    const NAME: &'static str = "elden ring";
    fn from_lang_version(lang: u16, version: &str) -> Option<Self> {
        if lang != LANG_ID_EN && lang != LANG_ID_JP {
            return None;
        }
        if lang == LANG_ID_EN && version == "2.7.2.0" {
            return Some(Self::Ww2720);
        }
        // 同 1.17.2 游戏版本的不同区域/小修订 exe（如日区 2.7.2.1）放行 2.7.2.*，由运行时函数字节校验兜底
        version
            .strip_prefix("2.7.2.")
            .filter(|rev| !rev.is_empty() && rev.bytes().all(|b| b.is_ascii_digit()))
            .map(|_| Self::Build272)
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
    map_preview: bool,
    run: Option<Run>,
    last_archetype: Option<u8>,
    last_prompt: u64,
    /// 顶部状态提示栏：(提示序号, 首次显示时间)
    status_banner: Option<(u64,std::time::Instant)>,
    /// 正在显示的场地文件提示：(提示序号, 开始显示的时间)
    resource_alert: Option<(u64,std::time::Instant)>,
}

fn native_hud_visible()->bool {
    use fromsoftware_shared::FromStatic;
    unsafe {eldenring::cs::CSFeManImp::instance()}.ok().is_none_or(|fe|fe.hud_state!=eldenring::cs::CSFeManHudState::HideAll)
}

/// 在前景层指定字号绘制带深色阴影的文字（不依赖当前窗口缩放）
fn draw_fg_text(pos:[f32;2],px:f32,col:hudhook::imgui::ImColor32,shadow_alpha:u8,text:&str) {
    use hudhook::imgui::{ImColor32,sys};
    if text.is_empty() {return;}
    let range=text.as_bytes().as_ptr_range();
    unsafe {
        let raw=sys::igGetForegroundDrawList();
        let font=sys::igGetFont();
        let off=(px*0.065).clamp(1.0,2.2);
        let shadow=ImColor32::from_rgba(0,0,0,shadow_alpha).to_bits();
        sys::ImDrawList_AddText_FontPtr(raw,font,px,sys::ImVec2{x:pos[0]+off,y:pos[1]+off},shadow,range.start.cast(),range.end.cast(),0.0,std::ptr::null());
        sys::ImDrawList_AddText_FontPtr(raw,font,px,sys::ImVec2{x:pos[0],y:pos[1]},col.to_bits(),range.start.cast(),range.end.cast(),0.0,std::ptr::null());
    }
}

/// 两侧渐隐的暗金横幅底（用于顶部词条与状态提示）
fn draw_tapered_ribbon(draw:&hudhook::imgui::DrawListMut<'_>,cx:f32,y0:f32,y1:f32,half_core:f32,wing:f32,bg_alpha:u8,line_rgb:[u8;3],line_alpha:u8,s:f32) {
    use hudhook::imgui::ImColor32;
    let mid=ImColor32::from_rgba(11,10,9,bg_alpha);
    let clear=ImColor32::from_rgba(11,10,9,0);
    draw.add_rect_filled_multicolor([cx-half_core-wing,y0],[cx-half_core,y1],clear,mid,mid,clear);
    draw.add_rect([cx-half_core,y0],[cx+half_core,y1],mid).filled(true).build();
    draw.add_rect_filled_multicolor([cx+half_core,y0],[cx+half_core+wing,y1],mid,clear,clear,mid);
    let lc=ImColor32::from_rgba(line_rgb[0],line_rgb[1],line_rgb[2],line_alpha);
    let l0=ImColor32::from_rgba(line_rgb[0],line_rgb[1],line_rgb[2],0);
    let t=1.0*s;
    for y in [y0,y1-t] {
        draw.add_rect_filled_multicolor([cx-half_core-wing*0.8,y],[cx-half_core,y+t],l0,lc,lc,l0);
        draw.add_rect([cx-half_core,y],[cx+half_core,y+t],lc).filled(true).build();
        draw.add_rect_filled_multicolor([cx+half_core,y],[cx+half_core+wing*0.8,y+t],lc,l0,l0,lc);
    }
}

/// 顶部紧凑胶囊徽章（用于 Boss 护盾预警、苦难律法限时等）
fn draw_top_pill(ui:&hudhook::imgui::Ui,cx:f32,y:f32,s:f32,label:&str,accent:[u8;3],text_rgb:[u8;3],pulse:f32,digits:Option<&str>)->f32 {
    use hudhook::imgui::ImColor32;
    let label_px=17.0*s;
    let digit_px=20.0*s;
    let pad_x=16.0*s;
    let pad_y=6.0*s;
    let gap=if digits.is_some(){10.0*s}else{0.0};
    let lw=fancy_text::measure(label,label_px);
    let dw=digits.map_or(0.0,|d|{
        let token=fancy_text::button_font().map(|f|ui.push_font(f));
        let w=fancy_text::measure(d,digit_px);
        drop(token);
        w
    });
    let diamond_w=12.0*s;
    let content_w=diamond_w+lw+gap+dw+diamond_w;
    let w=content_w+pad_x*2.0;
    let h=digit_px.max(label_px)+pad_y*2.0;
    let (x0,x1)=(cx-w*0.5,cx+w*0.5);
    let y1=y+h;
    let r=4.0*s;
    let draw=ui.get_foreground_draw_list();
    draw.add_rect([x0+1.5*s,y+2.5*s],[x1+1.5*s,y1+2.5*s],ImColor32::from_rgba(0,0,0,125)).filled(true).rounding(r).build();
    draw.add_rect_filled_multicolor([x0,y],[x1,y1],
        ImColor32::from_rgba(16,13,11,218),ImColor32::from_rgba(16,13,11,218),
        ImColor32::from_rgba(10,8,7,230),ImColor32::from_rgba(10,8,7,230));
    if pulse>0.01 {
        draw.add_rect([x0,y],[x1,y1],ImColor32::from_rgba(accent[0],accent[1],accent[2],(42.0*pulse) as u8)).filled(true).rounding(r).build();
    }
    draw.add_rect([x0,y],[x1,y1],ImColor32::from_rgba(accent[0],accent[1],accent[2],(145.0+90.0*pulse) as u8)).rounding(r).thickness(1.2*s).build();
    let cy=y+h*0.5;
    let rad=3.2*s;
    let dcol=ImColor32::from_rgba(accent[0],accent[1],accent[2],(190.0+65.0*pulse) as u8);
    for dx in [x0+pad_x+3.0*s,x1-pad_x-3.0*s] {
        draw.add_polyline(vec![[dx,cy-rad],[dx+rad,cy],[dx,cy+rad],[dx-rad,cy]],dcol).filled(true).build();
    }
    let tx=x0+pad_x+diamond_w;
    draw_fg_text([tx,cy-label_px*0.52],label_px,ImColor32::from_rgb(text_rgb[0],text_rgb[1],text_rgb[2]),220,label);
    if let Some(d)=digits {
        let token=fancy_text::button_font().map(|f|ui.push_font(f));
        draw_fg_text([tx+lw+gap,cy-digit_px*0.52],digit_px,ImColor32::from_rgb(accent[0].max(text_rgb[0]),accent[1].max(text_rgb[1]),accent[2].max(text_rgb[2])),225,d);
        drop(token);
    }
    h
}

/// 顶部状态提示横幅（原指南针区域）：传送状态、传送门就绪、余烬叠层、捕捉成功、联机等待等
fn draw_top_status_banner(ui:&hudhook::imgui::Ui,y:f32,text:&str,alpha:f32) {
    use hudhook::imgui::ImColor32;
    if text.trim().is_empty() || alpha<=0.02 {return;}
    let size=ui.io().display_size;
    let s=(size[1]/1080.0).clamp(0.65,1.8);
    let px=18.5*s;
    let max_w=(size[0]*0.56).max(320.0*s);
    let translated=crate::i18n::t(text);
    let lines=fancy_text::wrap_lines(&translated,max_w,|piece|fancy_text::measure(piece,px));
    if lines.is_empty() {return;}
    let text_w=lines.iter().map(|l|fancy_text::measure(l,px)).fold(0.0f32,f32::max);
    let line_h=px*1.32;
    let pad_y=8.0*s;
    let h=pad_y*2.0+line_h*lines.len() as f32-(line_h-px)*0.4;
    let cx=size[0]*0.5;
    let half_core=(text_w*0.5+28.0*s).max(110.0*s);
    let wing=76.0*s;
    let warn=text.contains("失败")||text.contains("超时")||text.contains("拒绝")||text.contains("上限")||text.contains("不足")||text.contains("错误");
    let line_rgb=if warn {[224,104,84]} else {[202,168,100]};
    let text_rgb=if warn {[248,186,166]} else {[242,226,186]};
    let a=alpha.clamp(0.0,1.0);
    let draw=ui.get_foreground_draw_list();
    draw_tapered_ribbon(&draw,cx,y,y+h,half_core,wing,(196.0*a) as u8,line_rgb,(155.0*a) as u8,s);
    let cy=y+h*0.5;
    let rad=3.2*s;
    let dcol=ImColor32::from_rgba(line_rgb[0],line_rgb[1],line_rgb[2],(210.0*a) as u8);
    for dx in [cx-text_w*0.5-15.0*s,cx+text_w*0.5+15.0*s] {
        draw.add_polyline(vec![[dx,cy-rad],[dx+rad,cy],[dx,cy+rad],[dx-rad,cy]],dcol).filled(true).build();
    }
    let col=ImColor32::from_rgba(text_rgb[0],text_rgb[1],text_rgb[2],(255.0*a) as u8);
    let shadow_a=(215.0*a) as u8;
    for (i,line) in lines.iter().enumerate() {
        let lw=fancy_text::measure(line,px);
        draw_fg_text([cx-lw*0.5,y+pad_y+i as f32*line_h],px,col,shadow_a,line);
    }
}

/// 战斗顶部 HUD（原指南针区域）：Boss 词条、护盾预警、苦难律法限时；返回占用后的底部 Y
fn draw_combat_top_hud(ui:&hudhook::imgui::Ui,combat:&runtime::View)->f32 {
    use hudhook::imgui::ImColor32;
    let size=ui.io().display_size;
    let s=(size[1]/1080.0).clamp(0.65,1.8);
    let cx=size[0]*0.5;
    let now=ui.time() as f32;
    let mut y=(20.0*s).max(size[1]*0.018);

    // 1. Boss 词条：带两侧渐隐暗金衬带，明亮天空下也清晰可读
    if !combat.boss_affix_list.is_empty() {
        let font=fancy_text::title_font().map(|f|ui.push_font(f));
        let px=if font.is_some(){30.0*s}else{26.0*s};
        let parts:Vec<_>=combat.boss_affix_list.iter().map(|a|(a.name(),fancy_text::affix_style(*a))).collect();
        let sep=" · ";
        let sep_w=fancy_text::measure(sep,px);
        let total_w:f32=parts.iter().map(|(t,_)|fancy_text::measure(t,px)).sum::<f32>()+sep_w*parts.len().saturating_sub(1) as f32;
        let pad_y=6.0*s;
        let h=px+pad_y*2.0;
        let draw=ui.get_foreground_draw_list();
        draw_tapered_ribbon(&draw,cx,y,y+h,(total_w*0.5+34.0*s).max(120.0*s),84.0*s,168,[204,168,98],135,s);
        drop(draw);
        fancy_text::draw_line(fancy_text::Layer::Foreground,cx,y+pad_y-1.0*s,px,&parts,(sep,[168,156,130]),now);
        if let Some(font)=font {font.pop();}
        y+=h+8.0*s;
    } else if let Some(title)=&combat.boss_affixes {
        let translated=crate::i18n::t(title);
        let px=22.0*s;
        let w=fancy_text::measure(&translated,px);
        let h=px+12.0*s;
        let draw=ui.get_foreground_draw_list();
        draw_tapered_ribbon(&draw,cx,y,y+h,(w*0.5+28.0*s).max(100.0*s),68.0*s,168,[204,168,98],130,s);
        draw_fg_text([cx-w*0.5,y+6.0*s],px,ImColor32::from_rgb(236,198,108),220,&translated);
        y+=h+8.0*s;
    }

    // 2. Boss 词条即时预警（如黄金护盾）
    if let Some((text,warning))=&combat.boss_alert {
        let translated=crate::i18n::t(text);
        let pulse=0.5+0.5*(now*if *warning{6.0}else{3.0}).sin();
        let (accent,fg)=if *warning {([242,138,66],[255,220,184])} else {([236,196,94],[255,240,196])};
        let h=draw_top_pill(ui,cx,y,s,&translated,accent,fg,pulse,None);
        y+=h+7.0*s;
    }

    // 3. 苦难律法限时 / 超时失血倒计时
    if let Some(left)=combat.law_clock {
        let urgent=left<=30;
        let pulse=if left<=0 {0.5+0.5*(now*6.5).sin()} else if urgent {0.4+0.6*(now*4.5).sin()} else {0.0};
        let (label,digits,accent,fg)=if left>0 {
            (crate::i18n::t("苦难律法 · 限时"),format!("{:02}:{:02}",left/60,left%60),
             if urgent {[236,96,76]} else {[206,174,108]},
             if urgent {[248,186,172]} else {[232,220,192]})
        } else {
            let over=-left;
            (crate::i18n::t("苦难律法 · 超时失血"),format!("+{:02}:{:02}",over/60,over%60),[236,72,62],[252,188,178])
        };
        let h=draw_top_pill(ui,cx,y,s,&label,accent,fg,pulse,Some(&digits));
        y+=h+7.0*s;
    }
    y
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

fn map_preview_available(view:&runtime::View)->bool {
    view.ready&&!view.travelling&&view.run.as_ref().is_some_and(|run|run.retry_node.is_none()&&!matches!(run.phase,Phase::Dead|Phase::Won))
}

impl TowerPreview {
    fn new() -> Self {
        Self {
            diagnostics:Default::default(),
            rewards: reward_ui::Panel::default(),
            debug: Default::default(),
            visible: true,
            map_preview: false,
            run: Some(Run::new(42, Class::Vagabond)),
            last_archetype: None,
            last_prompt: 0,
            status_banner: None,
            resource_alert: None,
        }
    }
}

impl ImguiRenderLoop for TowerPreview {
    fn before_render(&mut self, ctx: &mut Context, renderer: &mut dyn RenderContext) {
        self.rewards.load_icons(renderer);
        let player_present = SNAPSHOT.lock().is_ok_and(|snapshot| snapshot.is_some());
        let capture = runtime::with_view(|view| input::capture_route(self.visible, player_present, view)
            || self.map_preview&&player_present&&map_preview_available(view)).unwrap_or(false);
        input::CAPTURE.store(capture, Ordering::Relaxed);
        input::watch_pointer();
        if {let io=ctx.io();io.mouse_down[0]||io.mouse_wheel!=0.0||io.mouse_delta[0]!=0.0||io.mouse_delta[1]!=0.0} {input::note_mouse();}
        let map=runtime::with_view(|view|view.run.as_ref().is_some_and(|run|
            matches!(run.phase,Phase::Map)&&!run.contract_pending())).unwrap_or(false);
        reward_ui::poll_pad(ctx.io_mut(),capture&&input::active(),map);
        ctx.io_mut().mouse_draw_cursor = capture&&input::cursor_arrow();
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
        if input::dance_input_active(){return MessageFilter::InputAll;}
        let player_present = SNAPSHOT.lock().is_ok_and(|snapshot| snapshot.is_some());
        if input::foreground()&&runtime::with_view(|view| input::capture_route(self.visible, player_present, view)
            ||self.map_preview&&player_present&&map_preview_available(view)).unwrap_or(false)
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
        dance_art::initialize(renderer);
        log("DX12 renderer initialized");
        // 正文与标题字体都只加载界面字表里的字（缺字表时退回原来的范围），字体贴图不会因为多一套字体而翻倍
        let ranges=|fallback:FontGlyphRanges|fancy_text::ui_glyph_ranges().map_or(fallback,FontGlyphRanges::from_slice);
        // Rasterize near the displayed size, keeping existing logical font sizes and layout.
        let [width,height]=ctx.io().display_size;
        let density=(width/1600.0).min(height/900.0).clamp(1.0,2.0);
        let mut font_density=1.0;
        // 繁体：界面字表里没有繁体字形，另有补充字体（只含字表里简体字对应的繁体字），语言设为繁体时合并进正文与标题字体
        let traditional=crate::options::get().language==erdtree_tower::localization::Language::TraditionalChinese;
        let tw_ranges=if traditional {fancy_text::traditional_glyph_ranges()}else{None};
        let tw_body=tw_ranges.and_then(|_|std::fs::read(fancy_text::traditional_body_font_path()).ok());
        let tw_title=tw_ranges.and_then(|_|std::fs::read(fancy_text::traditional_title_font_path()).ok());
        if traditional {log(format!("TRADITIONAL fonts: glyphs={} body={} title={}",tw_ranges.is_some(),tw_body.is_some(),tw_title.is_some()));}
        // 正文：思源宋体常规体；字体文件缺失时退回系统黑体
        if let Ok(font) = std::fs::read(fancy_text::body_font_path()).or_else(|error|{log(format!("Body font missing, falling back to SimHei: {error}"));std::fs::read("C:/Windows/Fonts/simhei.ttf")}) {
            let mut sources=vec![FontSource::TtfData {
                data: &font,
                size_pixels: 20.0*density,
                config: Some(FontConfig {
                    glyph_ranges: ranges(FontGlyphRanges::chinese_full()),
                    rasterizer_multiply: 1.15,
                    ..Default::default()
                }),
            }];
            if let (Some(data),Some(extra))=(tw_body.as_deref(),tw_ranges) {
                sources.push(FontSource::TtfData {data,size_pixels:20.0*density,config:Some(FontConfig {glyph_ranges:FontGlyphRanges::from_slice(extra),rasterizer_multiply:1.15,..Default::default()})});
            }
            ctx.fonts().add_font(&sources);
            ctx.io_mut().font_global_scale=1.0/density;
            font_density=density;
        }
        // 标题字体：思源宋体子集，按 32 像素加载，供带动态特效的标题文字使用
        match std::fs::read(fancy_text::title_font_path()) {
            Ok(font)=>{
                let mut sources=vec![FontSource::TtfData {
                    data: &font,
                    size_pixels: 32.0*font_density,
                    config: Some(FontConfig {glyph_ranges: ranges(FontGlyphRanges::chinese_simplified_common()),..Default::default()}),
                }];
                if let (Some(data),Some(extra))=(tw_title.as_deref(),tw_ranges) {
                    sources.push(FontSource::TtfData {data,size_pixels:32.0*font_density,config:Some(FontConfig {glyph_ranges:FontGlyphRanges::from_slice(extra),..Default::default()})});
                }
                fancy_text::set_title_font(fancy_text::TitleFont(ctx.fonts().add_font(&sources)));
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
        let mut menu_input=input::menu_frame();
        diagnostics::frame();
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
        if let Some(view)=&view {
            if self.last_prompt!=view.prompt {
                self.last_prompt=view.prompt;
                self.visible=!view.run.as_ref().is_some_and(|run|matches!(run.phase,Phase::Combat{..}));
                self.map_preview=false;
                self.status_banner=(!view.status.trim().is_empty()).then_some((view.prompt,std::time::Instant::now()));
            }
            if !map_preview_available(view) {self.map_preview=false;}
            if !ui.io().want_text_input&&!run_screen::active()&&!debug_menu::open()
                &&options::pressed(ui,Hotkey::Map)&&map_preview_available(view) {
                if self.map_preview {self.map_preview=false;}
                else if view.room.is_none()&&!view.shared_event&&view.run.as_ref().is_some_and(|run|run.phase==Phase::Map&&!run.contract_pending()) {self.visible=!self.visible;}
                else {self.map_preview=true;}
                if !self.map_preview {input::CAPTURE.store(input::capture_route(self.visible,runtime::PLAYER_READY.load(Ordering::Relaxed),view),Ordering::Relaxed);}
            }
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
        // 癫火舞会：跳舞时只画舞会画面（它自己接管输入）
        let run_screen=dance_ui::draw(ui)||run_screen::draw(ui,&mut |icon|rewards.icon(icon));
        // HansFix 惜别泪滴：左上角状态小图标（画在最上层，全屏面板 / 路线图预览打开时不画）。
        // 身上的特效由游戏原生渲染（regulation 里 SpEffectVfx 1699000 / 1699001），不用叠加层画。
        tears::draw_hud(ui,&mut |icon|rewards.icon(icon),!run_screen&&!self.map_preview);
        if options::pressed(ui,Hotkey::Debug) {debug_menu::toggle();}
        self.debug.draw(ui,view.as_ref());
        // 战斗顶部 HUD（原指南针位置：Boss 词条、护盾预警、苦难律法限时）
        let hud_visible=native_hud_visible();
        let mut top_y=(24.0*(ui.io().display_size[1]/1080.0).clamp(0.65,1.8)).max(ui.io().display_size[1]*0.02);
        if hud_visible&&!run_screen&&!self.map_preview&&let Some(combat)=view.as_ref().filter(|v|v.run.as_ref().is_some_and(|r|matches!(r.phase,Phase::Combat{..}))) {
            top_y=draw_combat_top_hud(ui,combat);
        }
        let key=format!("UI visible={} snapshot={} capture={} settlement={} view={:?}",self.visible,snapshot.is_some(),input::CAPTURE.load(Ordering::Relaxed),run_screen,view.as_ref().map(|v|(v.ready,v.busy,v.travelling,v.room.is_some(),v.shared_event,v.prompt)));
        if self.diagnostics.due(key.clone(),std::time::Instant::now()){log(key);}
        // 整合包场地文件不完整：屏幕上方醒目提示 20 秒（开局检查一次，之后每次出发被拒再提示）
        if let Some((id,files))=view.as_ref().and_then(|v|v.resource_alert.as_ref()) {
            if self.resource_alert.is_none_or(|(seen,_)|seen!=*id) {self.resource_alert=Some((*id,std::time::Instant::now()));}
            if self.resource_alert.is_some_and(|(_,at)|at.elapsed()<std::time::Duration::from_secs(20)) {draw_resource_alert(ui,files);}
        }
        // F6 暂退提示：在路线图、战斗、营地、联机离队、返回大赐福等所有界面顶部都可见，避免无反馈或客机看不到分离迷雾提示
        if let Some((text,alpha))=suspension::active_notice() {
            let s=(ui.io().display_size[1]/1080.0).clamp(0.65,1.8);
            draw_top_status_banner(ui,top_y,&text,alpha);
            top_y+=52.0*s;
        }
        top_y=custom_event_ui::draw_hud(ui,top_y);
        if run_screen {return;}
        // 右上角双计时器（本局用时 + 本场/上场用时）：非全屏结算/开局页且 HUD 显示时常驻；打开路线图时自动避让右侧边栏
        let showing_map=self.map_preview&&view.as_ref().is_some_and(|v|map_preview_available(v)&&v.run.is_some())
            ||(self.visible&&view.as_ref().is_some_and(|v|v.room.is_none()&&!v.shared_event&&!v.travelling
                &&v.run.as_ref().is_some_and(|r|r.retry_node.is_none()&&!matches!(r.phase,Phase::Combat{..}|Phase::Reward{..})&&!r.contract_pending())));
        if hud_visible {
            let map_offset=if showing_map {
                let sz=ui.io().display_size;
                let map_scale=(sz[0]/1600.0).min(sz[1]/900.0).clamp(0.65,1.6);
                236.0*map_scale
            } else {0.0};
            run_clock::draw(ui,map_offset);
        }
        if self.map_preview&&let Some(view)=view.as_ref().filter(|view|map_preview_available(view))&&let Some(run)=&view.run {
            menu_input.show();
            map_ui::preview(ui,run,view);
            if build_panel::open() {let rewards=&mut self.rewards;build_panel::draw(ui,run,view,&mut |icon|rewards.icon(icon));}
            return;
        }
        if let Some(view) = &view {
            if let Some(dummy)=&view.dummy {dummy::draw_hud(ui,dummy);}
            // 事件房（shared_event）里 view.ready 是 false，但角色在场景里，叠层也要显示
            starsurge::draw_hud(ui,&mut |icon|rewards.icon(icon),(view.ready||view.shared_event)&&!view.travelling);
            let travelling_notice=view.travelling||view.run.as_ref().is_some_and(|r|r.retry_node.is_some());
            let fatal_notice=view.status.contains("已停止")||view.status.contains("已断开")||view.status.contains("返回大赐福")||view.status.contains("回到大赐福")||view.status.contains("联机通道波动");
            let in_live_world=travelling_notice||fatal_notice||view.room.is_some()||view.shared_event
                ||view.run.as_ref().is_some_and(|r|matches!(r.phase,Phase::Combat{..}));
            if hud_visible&&in_live_world&&runtime::PLAYER_READY.load(Ordering::Relaxed)&&!view.status.trim().is_empty() {
                let persistent=travelling_notice||fatal_notice||view.status.contains("传送门")||view.status.contains("等待");
                let alpha=if persistent {
                    self.status_banner.map_or(1.0,|(_,at)|(at.elapsed().as_secs_f32()/0.18).min(1.0))
                } else if let Some((_,at))=self.status_banner {
                    let e=at.elapsed().as_secs_f32();
                    if e<0.18 {e/0.18} else if e<5.2 {1.0} else if e<5.9 {(5.9-e)/0.7} else {0.0}
                } else {0.0};
                if alpha>0.01 {draw_top_status_banner(ui,top_y,&view.status,alpha);}
            }
            if view.room.is_some()||view.shared_event {
                return;
            }
            if travelling_notice {
                input::CAPTURE.store(false, Ordering::Relaxed);
                return;
            }
        }
        if !self.visible {
            return;
        }
        let active_run = view
            .as_ref()
            .and_then(|view| view.run.as_ref())
            .or(self.run.as_ref());
        let combat = active_run.is_some_and(|run| matches!(run.phase, Phase::Combat { .. }));
        if !combat {
            if let (Some(run), Some(view)) = (active_run, view.as_ref()) {
                if snapshot.is_some() {
                    let vitals = snapshot
                        .map(|s| {
                            format!("生命 {}/{}    专注 {}/{}", s.hp, s.max_hp, s.fp, s.max_fp)
                        })
                        .unwrap_or_default();
                    if run.contract.as_ref().is_some_and(|c|c.selection_pending()) {
                        menu_input.show();
                        self.rewards.draw_contract(ui,run,view);
                    } else if run.contract_pending()||matches!(run.phase,Phase::Reward{..}) {
                        menu_input.show();
                        self.rewards.draw(ui,run,view);
                    }
                    else {
                        menu_input.show();
                        map_ui::draw(ui, run, view, &vitals);
                        if build_panel::open() {let rewards=&mut self.rewards;build_panel::draw(ui,run,view,&mut |icon|rewards.icon(icon));}
                    }
                }
            }
            return;
        }
    }
}

fn effective_renderer(configured:erdtree_tower::options::Renderer,wine:bool)->erdtree_tower::options::Renderer {
    if wine {erdtree_tower::options::Renderer::Game} else {configured}
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
        crash_trace::install(address,LOG_PATH.parent().map(std::path::Path::to_path_buf).unwrap_or_else(std::env::temp_dir));
        crash_guard::install();
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
        match SupportedVersion::detect(&mapping) {
            Ok(version) => log(format!("Detected game version: {version:?}")),
            Err(error) => {
                log(format!("Initialization stopped: {error}"));
                return;
            }
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
        let configured_renderer=options::get().renderer;
        let wine=diagnostics::running_under_wine();
        let renderer=effective_renderer(configured_renderer,wine);
        if wine&&configured_renderer==erdtree_tower::options::Renderer::Window {
            log("Wine/Proton detected; forcing in-process DX12 renderer instead of DirectComposition window");
        }
        if renderer==erdtree_tower::options::Renderer::Window {
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
        if let Err(error)=instance_description::install() {log(format!("Instance descriptions unavailable: {error}"));}
        if let Err(error)=unsafe {spirits::install()} {log(format!("Spirit use trace unavailable: {error}"));}
        if let Err(error)=unsafe {star_stream::install()} {log(format!("Star stream unavailable: {error}"));}
        if let Err(error)=unsafe {radahn_entity::install()} {log(format!("Radahn event binding unavailable: {error}"));}
        if let Err(error)=unsafe {embers::install_critical_trace()} {log(format!("Critical stack detection unavailable: {error}"));}
        match unsafe {damage::install()} {
            Ok(())=>adaptation::install(),
            Err(error)=>log(format!("Damage interface unavailable: {error}")),
        }
        if let Err(error) = wait_for_system_init(&program, Duration::from_secs(60)) {
            log(format!("Game system initialization failed: {error}"));
            return;
        }
        //[fix] 独立窗口不经过 hudhook 的十余秒阻塞，会在游戏建好 CS 任务单例之前就走这里；
        //一次性的 wait_for_instance 立刻报 "Could not translate RVA to VA" 就放弃，塔的任务、交谈和交互全部装不上，所以改成重试等它出现。
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut attempt = 1u32;
        let task = loop {
            match CSTaskImp::wait_for_instance(Duration::from_secs(1)) {
                Ok(task) => break task,
                Err(error) => {
                    if Instant::now() >= deadline {
                        log(format!("Game scheduler unavailable: {error}"));
                        return;
                    }
                    if attempt == 1 || attempt % 10 == 0 {
                        log(format!("Game scheduler not ready (attempt {attempt}): {error}"));
                    }
                    attempt += 1;
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        };
        if let Err(error)=unsafe {seamless_revival::install()} {log(format!("PARTY_REVIVE unavailable: {error}"));}
        let recurring = task.run_recurring(
            capture as fn(&FD4TaskData),
            CSTaskGroupIndex::ChrIns_PostPhysics,
        );
        std::mem::forget(recurring);
        log("Character capture task registered");
        let camera_task=task.run_recurring(dance_camera::tick as fn(&FD4TaskData),CSTaskGroupIndex::CameraStep);
        std::mem::forget(camera_task);
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
        assert_eq!(SupportedVersion::from_lang_version(LANG_ID_EN, "2.7.2.0"), Some(SupportedVersion::Ww2720));
        assert_eq!(SupportedVersion::from_lang_version(LANG_ID_EN, "2.7.2.1"), Some(SupportedVersion::Build272));
        assert_eq!(SupportedVersion::from_lang_version(LANG_ID_JP, "2.7.2.1"), Some(SupportedVersion::Build272));
        assert_eq!(SupportedVersion::from_lang_version(LANG_ID_JP, "2.7.2.0"), Some(SupportedVersion::Build272));
        assert!(SupportedVersion::from_lang_version(LANG_ID_EN, "2.7.1.0").is_none());
        assert!(SupportedVersion::from_lang_version(LANG_ID_EN, "2.7.3.0").is_none());
        assert!(SupportedVersion::from_lang_version(LANG_ID_EN, "2.7.2").is_none());
        assert!(SupportedVersion::from_lang_version(LANG_ID_EN, "2.7.2.a").is_none());
        assert!(SupportedVersion::from_lang_version(LANG_ID_JP, "2.7.1.0").is_none());
        assert!(SupportedVersion::from_lang_version(0x12, "2.7.2.0").is_none());
    }
    #[test]
    fn wine_forces_game_dx12_renderer() {
        use erdtree_tower::options::Renderer;
        assert_eq!(effective_renderer(Renderer::Window, true), Renderer::Game);
        assert_eq!(effective_renderer(Renderer::Game, true), Renderer::Game);
        assert_eq!(effective_renderer(Renderer::Window, false), Renderer::Window);
        assert_eq!(effective_renderer(Renderer::Game, false), Renderer::Game);
    }
}
