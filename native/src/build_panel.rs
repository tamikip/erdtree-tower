// 路线图的构筑面板：职业、誓约、余烬、护符赐福、苦难律法与身上的词条装备（边栏按钮或 C 键开关）
use crate::run_screen::wrapped;
use crate::runtime::View;
use erdtree_tower::Run;
use hudhook::imgui::{Condition, ImColor32, StyleColor, TextureId, Ui, WindowFlags};
use std::sync::atomic::{AtomicBool, Ordering};

/// 构筑面板是否打开（边栏按钮或 C 键切换）
pub(crate) static BUILD_OPEN: AtomicBool = AtomicBool::new(false);
const LEVELS: [&str; 4] = ["", "I", "II", "III"];
const GOLD: [f32; 4] = [0.86, 0.74, 0.48, 1.0];
const WHITE: [f32; 4] = [0.93, 0.9, 0.81, 1.0];
const BOON: [f32; 4] = [0.84, 0.79, 0.66, 1.0];
const MUTED: [f32; 4] = [0.62, 0.59, 0.51, 1.0];
const RED: [f32; 4] = [0.92, 0.47, 0.38, 1.0];
enum Badge {Icon(Option<TextureId>), Level(&'static str)}
fn content_right(ui:&Ui)->f32 {ui.cursor_screen_pos()[0]+ui.content_region_avail()[0]}
fn role_heading(role:&crate::class_ui::Role)->String {
    // Each component has its own catalog entry, including role descriptions with middle dots.
    format!("{} · {}",crate::i18n::t(role.role),crate::i18n::t(role.mechanic.0))
}
/// 分区标题：金色标题、条目数与向右渐隐的细线
fn heading(ui: &Ui, s: f32, title: &str, count: Option<usize>) {
    let start = ui.cursor_screen_pos();
    let start = [start[0], start[1] + 10.0 * s];
    ui.set_window_font_scale(1.02 * s);ui.set_cursor_screen_pos(start);ui.text_colored(GOLD,crate::i18n::t(title));
    let mut end = ui.item_rect_max();
    if let Some(count) = count {ui.same_line();ui.set_window_font_scale(0.8 * s);ui.text_colored(MUTED,crate::i18n::t(format!("×{count}")));end = ui.item_rect_max();}
    let right = content_right(ui);let y = (start[1] + end[1]) * 0.5;
    ui.get_window_draw_list().add_rect_filled_multicolor([end[0] + 10.0 * s, y - 0.6 * s], [right, y + 0.6 * s],
        ImColor32::from_rgba(214, 180, 112, 150), ImColor32::from_rgba(214, 180, 112, 0), ImColor32::from_rgba(214, 180, 112, 0), ImColor32::from_rgba(214, 180, 112, 150));
    ui.set_cursor_screen_pos([start[0], end[1] + 10.0 * s]);ui.dummy([0.0, 0.0]);
}
/// 一个条目：左侧图标或等级徽记，右侧标题与几行说明
fn entry(ui: &Ui, s: f32, badge: Badge, title: &str, title_color: [f32; 4], lines: &[(String, [f32; 4])]) {
    let start = ui.cursor_screen_pos();let side = 42.0 * s;
    let right = content_right(ui);
    let draw = ui.get_window_draw_list();
    draw.add_rect(start, [start[0] + side, start[1] + side], ImColor32::from_rgba(0, 0, 0, 150)).filled(true).rounding(4.0 * s).build();
    match &badge {
        Badge::Icon(Some(texture)) => {draw.add_image(*texture, [start[0] + 2.0 * s, start[1] + 2.0 * s], [start[0] + side - 2.0 * s, start[1] + side - 2.0 * s]).build();}
        Badge::Icon(None) => {}
        Badge::Level(_) => {
            let (c, h) = ([start[0] + side * 0.5, start[1] + side * 0.5], 17.0 * s);
            let diamond = vec![[c[0], c[1] - h], [c[0] + h, c[1]], [c[0], c[1] + h], [c[0] - h, c[1]]];
            draw.add_polyline(diamond.clone(), ImColor32::from_rgba(74, 50, 28, 255)).filled(true).build();
            let mut outline = diamond.clone();outline.push(diamond[0]);
            draw.add_polyline(outline, ImColor32::from_rgba(214, 180, 112, 255)).thickness(1.5 * s).build();
        }
    }
    draw.add_rect(start, [start[0] + side, start[1] + side], ImColor32::from_rgba(171, 143, 87, 90)).rounding(4.0 * s).build();
    drop(draw);
    if let Badge::Level(numeral) = badge && !numeral.is_empty() {
        ui.set_window_font_scale(0.8 * s);let w = ui.calc_text_size(crate::i18n::t(numeral));
        ui.set_cursor_screen_pos([start[0] + (side - w[0]) * 0.5, start[1] + (side - w[1]) * 0.5]);ui.text_colored(WHITE,crate::i18n::t(numeral));
    }
    let x = start[0] + side + 12.0 * s;
    let mut y = wrapped(ui, [x, start[1]], right, title, title_color, 0.95 * s) + 2.0 * s;
    for (line, color) in lines {y = wrapped(ui, [x, y], right, line, *color, 0.84 * s) + 1.0 * s;}
    ui.set_cursor_screen_pos([start[0], y.max(start[1] + side) + 10.0 * s]);ui.dummy([0.0, 0.0]);
}
/// Use two columns only when the localized labels and values fit; wrap labels within their own cell.
fn stats(ui: &Ui, s: f32, summary: &crate::stats::Summary) {
    heading(ui, s, "数值加成", None);
    if summary.lines.is_empty() {empty(ui, s, "身上暂时没有数值加成");}
    let left = ui.cursor_screen_pos()[0];
    let right = content_right(ui);
    let width = (right-left).max(1.0);
    let gap = 12.0 * s;
    ui.set_window_font_scale(0.84 * s);
    let required = summary.lines.iter().map(|line|
        ui.calc_text_size(crate::i18n::t(line.label))[0]+gap+ui.calc_text_size(&line.value)[0]
    ).fold(0.0,f32::max);
    let columns = if required <= (width-18.0*s)*0.5 {2} else {1};
    let column = (width-(columns-1) as f32*18.0*s)/columns as f32;
    let mut y = ui.cursor_screen_pos()[1];
    let mut bottom = y;
    let mut group = "";
    let mut index = 0;
    for line in &summary.lines {
        if line.group != group {
            if index % columns != 0 {y = bottom + 6.0 * s;}
            group = line.group;index = 0;
            ui.set_window_font_scale(0.78 * s);ui.set_cursor_screen_pos([left, y]);ui.text_colored(MUTED,crate::i18n::t(group));
            y = ui.item_rect_max()[1] + 4.0 * s;
            bottom = y;
        }
        let x = left + (index % columns) as f32*(column+18.0*s);
        ui.set_window_font_scale(0.84 * s);
        let w = ui.calc_text_size(&line.value)[0];
        let value_x = x + column - w;
        let label_bottom = wrapped(ui,[x,y],value_x-gap,line.label,WHITE,0.84*s);
        ui.set_cursor_screen_pos([value_x,y]);ui.text_colored(if line.good {BOON} else {RED},&line.value);
        bottom = bottom.max(label_bottom).max(ui.item_rect_max()[1]);
        #[cfg(test)]
        STAT_ROWS.with(|rows|rows.borrow_mut().push(([x,y,value_x-gap,label_bottom],[value_x,y,ui.item_rect_max()[0],ui.item_rect_max()[1]],right)));
        index += 1;
        if index % columns == 0 {y = bottom + 6.0 * s;}
    }
    if index % columns != 0 {y = bottom + 6.0 * s;}
    if summary.conditional > 0 {
        y = wrapped(ui, [left, y + 2.0 * s], right, &format!("另有 {} 个带触发条件的效果（低血量、只对某类攻击生效等）未计入", summary.conditional), MUTED, 0.78 * s);
    }
    ui.set_cursor_screen_pos([left, y + 6.0 * s]);ui.dummy([0.0, 0.0]);
}
fn empty(ui: &Ui, s: f32, text: &str) {
    let pos = ui.cursor_screen_pos();let right = content_right(ui);
    let y = wrapped(ui, pos, right, text, MUTED, 0.84 * s);
    ui.set_cursor_screen_pos([pos[0], y + 6.0 * s]);ui.dummy([0.0, 0.0]);
}
/// 当前构筑：誓约、余烬、护符赐福、苦难律法与身上的词条装备
pub fn draw(ui: &Ui, run: &Run, view: &View, icon: &mut dyn FnMut(u32) -> Option<TextureId>) {
    let size = ui.io().display_size;let s = (size[0] / 1600.0).min(size[1] / 900.0).clamp(0.65, 1.6);
    let width = 480.0 * s;let pos = [size[0] - 165.0 * s - width - 14.0 * s, 20.0 * s];let extent = [width, size[1] - 40.0 * s];
    let mut close = false;
    ui.window("当前构筑##build").position(pos, Condition::Always).size(extent, Condition::Always)
        .flags(WindowFlags::NO_DECORATION | WindowFlags::NO_BACKGROUND | WindowFlags::NO_MOVE | WindowFlags::NO_NAV | WindowFlags::NO_SCROLL_WITH_MOUSE).build(|| {
        let draw = ui.get_window_draw_list();
        // 窗口自带的裁剪区会切掉四角装饰，边框按整个窗口范围绘制
        let max = [pos[0] + extent[0], pos[1] + extent[1]];
        draw.with_clip_rect(pos, max, || crate::law_ui::panel(&draw, [pos[0] + 1.0 * s, pos[1] + 1.0 * s], [max[0] - 1.0 * s, max[1] - 1.0 * s], s));
        crate::law_ui::ornament(&draw, [pos[0] + width * 0.5, pos[1] + 70.0 * s], width * 0.42, s);
        drop(draw);
        ui.set_window_font_scale(1.3 * s);let title = "当 前 构 筑";
        ui.set_cursor_screen_pos([pos[0] + (width - ui.calc_text_size(crate::i18n::t(title))[0]) * 0.5, pos[1] + 20.0 * s]);ui.text_colored(GOLD,crate::i18n::t(title));
        // 右上角关闭（C 键同样可以关闭）
        let corner = [pos[0] + width - 46.0 * s, pos[1] + 18.0 * s];
        ui.set_cursor_screen_pos(corner);close = ui.invisible_button("close-build", [30.0 * s, 30.0 * s]);
        let color = if ui.is_item_hovered() {ImColor32::from_rgba(240, 214, 160, 255)} else {ImColor32::from_rgba(171, 143, 87, 220)};
        let (a, b) = ([corner[0] + 9.0 * s, corner[1] + 9.0 * s], [corner[0] + 21.0 * s, corner[1] + 21.0 * s]);
        let draw = ui.get_window_draw_list();
        draw.add_line(a, b, color).thickness(1.8 * s).build();draw.add_line([a[0], b[1]], [b[0], a[1]], color).thickness(1.8 * s).build();drop(draw);
        let build_key = crate::pad::Input::new(crate::pad::Pad::Y, crate::options::label(erdtree_tower::options::Hotkey::Build));
        crate::pad::hints(ui, [pos[0] + 26.0 * s, pos[1] + 36.0 * s], 0.0, &[(Some(build_key), "开关")], 20.0 * s, 14.0 * s, MUTED, crate::pad::Tone::Light);
        ui.set_cursor_screen_pos([pos[0] + 22.0 * s, pos[1] + 84.0 * s]);
        let _child = ui.push_style_color(StyleColor::ChildBg, [0.0, 0.0, 0.0, 0.0]);
        let _track = ui.push_style_color(StyleColor::ScrollbarBg, [0.0, 0.0, 0.0, 0.0]);
        let _grab = ui.push_style_color(StyleColor::ScrollbarGrab, [0.45, 0.37, 0.22, 0.8]);
        let _grab_hover = ui.push_style_color(StyleColor::ScrollbarGrabHovered, [0.62, 0.5, 0.3, 0.9]);
        ui.child_window("build-content").size([width - 36.0 * s, extent[1] - 104.0 * s]).build(|| {
            let start=ui.cursor_screen_pos();let right=content_right(ui);
            let mut y=wrapped(ui,start,right,&format!("灵药储备：{} 瓶（喝完手中一瓶后自动补充）",run.physick_supply.reserve),BOON,0.84*s);
            if run.physick_supply.issued {y=wrapped(ui,[start[0],y+4.0*s],right,"灵药补充待确认；未确认前不会重复发放。",MUTED,0.84*s);}
            ui.set_cursor_screen_pos([start[0],y+6.0*s]);ui.dummy([0.0,0.0]);
            // 手柄右摇杆与鼠标滚轮：指针不在面板上时也滚动面板
            let wheel = ui.io().mouse_wheel;
            if wheel != 0.0 && !ui.is_window_hovered() {ui.set_scroll_y((ui.scroll_y() - wheel * 48.0 * s).clamp(0.0, ui.scroll_max_y()));}
            stats(ui, s, &view.stats);
            // 职业与机制
            heading(ui, s, "职业", None);
            let role = erdtree_tower::Class::PLAYABLE.iter().position(|c| *c == run.class).map(|i| &crate::class_ui::ROLES[i]);
            {
                let pos = ui.cursor_screen_pos();let right = content_right(ui);
                let color = role.map_or(WHITE, |r| [f32::from(r.accent.0) / 255.0, f32::from(r.accent.1) / 255.0, f32::from(r.accent.2) / 255.0, 1.0]);
                let mut y = wrapped(ui, pos, right, crate::map_ui::class_name(run.class), color, 1.0 * s) + 3.0 * s;
                match role {
                    Some(role) => {
                        y = wrapped(ui, [pos[0], y], right, &role_heading(role), GOLD, 0.86 * s) + 2.0 * s;
                        y = wrapped(ui, [pos[0], y], right, role.mechanic.1, BOON, 0.84 * s);
                    }
                    None => {y = wrapped(ui, [pos[0], y], right, "旧版职业：沿用原来的职业被动", MUTED, 0.84 * s);}
                }
                ui.set_cursor_screen_pos([pos[0], y + 8.0 * s]);ui.dummy([0.0, 0.0]);
            }
            let oaths = run.contract.as_ref().map(|c| c.active()).unwrap_or_default();
            heading(ui, s, "誓约", Some(oaths.len()).filter(|n| *n > 0));
            if oaths.is_empty() {empty(ui, s, "尚未立下誓约");}
            for oath in oaths {
                let (boon, bane) = if oath.level == 0 {(oath.kind.boon().to_owned(), oath.kind.bane())} else {(oath.kind.boon_at(oath.level), oath.kind.bane_at())};
                entry(ui, s, Badge::Level(LEVELS[usize::from(oath.level.min(3))]), oath.kind.name(), WHITE, &[(erdtree_tower::localization::inline(&boon), BOON), (format!("代价：{bane}"), RED)]);
            }
            heading(ui, s, "余烬", Some(run.embers.len()).filter(|n| *n > 0));
            if run.embers.is_empty() {empty(ui, s, "还没有余烬：普通战与精英战的奖励里可能出现");}
            for ember in &run.embers {
                let mut lines=Vec::new();
                let mut name=ember.name().to_owned();
                if ember.stacking() {
                    let level=run.ember_stacks.level(*ember);
                    name=format!("{name} · {level} 层");
                    lines.push((format!("当前加成：{}", run.ember_stacks.bonus(*ember).unwrap_or_default()), WHITE));
                    if *ember==erdtree_tower::embers::Ember::SoulEater {
                        let step=erdtree_tower::embers::SOUL_POINTS;
                        let points=run.ember_stacks.souls%step;
                        lines.push((format!("下一层：{points}/{step} 魂点（还差 {} 点）",step-points),GOLD));
                    }
                }
                lines.push((erdtree_tower::localization::inline(&ember.effect()), BOON));
                if let Some(label)=crate::reward_ui::ember_class_label(*ember) {lines.push((label, MUTED));}
                entry(ui, s, Badge::Icon(crate::reward_ui::ember_icon(*ember).and_then(&mut *icon)), &name, crate::reward_ui::rarity_color(usize::from(ember.quality())), &lines);
            }
            if !run.elite_buffs.is_empty() {
                heading(ui, s, "护符赐福", Some(run.elite_buffs.len()));
                for id in &run.elite_buffs {
                    let (name, info) = crate::reward_ui::talisman_description(*id);
                    let lines: Vec<_> = (!info.is_empty()).then(|| (erdtree_tower::localization::inline(&info), BOON)).into_iter().collect();
                    entry(ui, s, Badge::Icon(crate::reward_ui::talisman_icon(*id).and_then(&mut *icon)), &name, WHITE, &lines);
                }
            }
            heading(ui, s, "苦难律法", None);
            if run.law.is_empty() {empty(ui, s, "未立下苦难律法");}
            else {
                let start = ui.cursor_screen_pos();
                let mut y = wrapped(ui, start, content_right(ui), &format!("{} 分 · {} · 律法码 {}", run.law.score(), run.law.grade().unwrap_or("普通"), run.law.code()), WHITE, 0.9 * s) + 6.0 * s;
                // 律法节点图标，悬停看效果
                let (side, gap) = (32.0 * s, 6.0 * s);let per_row = ((ui.window_size()[0] - 14.0 * s) / (side + gap)).floor().max(1.0) as usize;
                let nodes = run.law.selected();
                for (i, id) in nodes.iter().enumerate() {
                    let node = &erdtree_tower::law::NODES[*id];
                    let min = [start[0] + (i % per_row) as f32 * (side + gap), y + (i / per_row) as f32 * (side + gap)];let max = [min[0] + side, min[1] + side];
                    let draw = ui.get_window_draw_list();
                    draw.add_rect(min, max, ImColor32::from_rgba(0, 0, 0, 150)).filled(true).rounding(3.0 * s).build();
                    crate::law_art::draw_line(&draw, node.art, [min[0] + 5.0 * s, min[1] + 5.0 * s], [max[0] - 5.0 * s, max[1] - 5.0 * s], 240);
                    draw.add_rect(min, max, ImColor32::from_rgba(171, 143, 87, 110)).rounding(3.0 * s).build();drop(draw);
                    if ui.is_mouse_hovering_rect(min, max) {ui.set_window_font_scale(s);ui.tooltip_text(crate::i18n::t(format!("{}\n{}", node.name, node.effect)));}
                }
                if !nodes.is_empty() {y += nodes.len().div_ceil(per_row) as f32 * (side + gap) + 2.0 * s;}
                let names: Vec<_> = nodes.iter().map(|id| erdtree_tower::law::NODES[*id].name).collect();
                let right = content_right(ui);
                y = wrapped(ui, [start[0], y], right, &names.join("、"), MUTED, 0.82 * s);
                ui.set_cursor_screen_pos([start[0], y + 8.0 * s]);ui.dummy([0.0, 0.0]);
            }
            heading(ui, s, "装备词条", Some(view.build.len()).filter(|n| *n > 0));
            if view.build.is_empty() {empty(ui, s, "装备栏武器与身上防具没有词条");}
            for gear in &view.build {
                let lines: Vec<_> = gear.lines.iter().map(|line| (format!("· {line}"), WHITE)).collect();
                // 名字由下面单独用品质特效画，这里留空标题占位
                let start = ui.cursor_screen_pos();
                entry(ui, s, Badge::Icon(icon(gear.icon)), "　", WHITE, &lines);
                let font = crate::fancy_text::title_font().map(|f| ui.push_font(f));
                ui.set_window_font_scale(s * if font.is_some() {0.62} else {0.95});
                let size = ui.current_font_size();
                crate::fancy_text::quality_name(crate::fancy_text::Layer::Window, start[0] + 54.0 * s, start[1] - 1.0 * s, size, &gear.name, crate::reward_ui::rarity_color(usize::from(gear.quality)), usize::from(gear.quality), 0.0, ui.time() as f32);
                if let Some(font) = font {font.pop();}
            }
        });
    });
    if close {BUILD_OPEN.store(false, Ordering::Relaxed);}
}
pub fn open() -> bool {BUILD_OPEN.load(Ordering::Relaxed)}
pub fn toggle() {BUILD_OPEN.store(!open(), Ordering::Relaxed);}

#[cfg(test)]
thread_local! {static STAT_ROWS:std::cell::RefCell<Vec<([f32;4],[f32;4],f32)>>=const {std::cell::RefCell::new(Vec::new())};}

#[cfg(test)]
mod tests {
    use super::*;
    use erdtree_tower::localization::{self,Language};
    use hudhook::imgui::{Context,FontConfig,FontGlyphRanges,FontSource};

    // ImGui and localization own process-global state. Run this preview in its own test process.
    #[test]
    #[ignore = "run explicitly with --ignored --test-threads=1; optional ERDTREE_BUILD_PREVIEW exports frames"]
    fn localized_build_panel_fits_real_font_metrics() {
        struct RestoreLanguage(bool);
        impl Drop for RestoreLanguage {fn drop(&mut self){localization::set(if self.0 {Language::English} else {Language::Chinese});}}
        let _restore=RestoreLanguage(localization::english());
        let mut ctx=Context::create();ctx.set_ini_filename(None);
        ctx.io_mut().delta_time=1.0/60.0;
        let font=include_bytes!("../../scene-assets/fonts/TowerSerif-Regular.otf");
        ctx.fonts().add_font(&[FontSource::TtfData{data:font,size_pixels:20.0,
            config:Some(FontConfig{glyph_ranges:FontGlyphRanges::chinese_full(),..Default::default()})}]);
        let output=std::env::var_os("ERDTREE_BUILD_PREVIEW").map(std::path::PathBuf::from);
        let atlas=ctx.fonts().build_rgba32_texture();
        if let Some(path)=&output {
            std::fs::create_dir_all(path).unwrap();
            std::fs::write(path.join("font.rgba"),atlas.data).unwrap();
            std::fs::write(path.join("font.size"),format!("{} {}",atlas.width,atlas.height)).unwrap();
        }
        let mut run=Run::new(42,erdtree_tower::Class::Prophet);run.physick_supply.reserve=3;
        let mut view=crate::runtime::view().unwrap();
        view.stats=crate::stats::Summary{
            lines:erdtree_tower::stat_summary::lines(&[2.373;25]),conditional:3,
        };
        for language in [Language::English,Language::Chinese] {
            localization::set(language);
            for role in &crate::class_ui::ROLES {
                let heading=role_heading(role);
                if language==Language::English {
                    assert!(!heading.chars().any(|c|('\u{3400}'..='\u{9fff}').contains(&c)),"{heading}");
                } else {assert_eq!(heading,format!("{} · {}",role.role,role.mechanic.0));}
            }
            if language==Language::English {
                assert_eq!(role_heading(&crate::class_ui::ROLES[4]),"Incantations · Healing · Support · Sacred Oath");
            }
            for (width,height) in [(1280,720),(1920,1080),(2560,1440),(3840,2160)] {
                ctx.io_mut().display_size=[width as f32,height as f32];
                // Let the child window settle its scrollbar before measuring its content area.
                for _ in 0..3 {draw(ctx.frame(),&run,&view,&mut |_|None);ctx.render();}
                STAT_ROWS.with(|rows|rows.borrow_mut().clear());
                draw(ctx.frame(),&run,&view,&mut |_|None);
                let data=ctx.render();assert!(data.total_vtx_count>0);
                STAT_ROWS.with(|rows| {
                    let rows=rows.borrow();assert_eq!(rows.len(),view.stats.lines.len());
                    if language==Language::English {assert!(rows[1].0[1]>=rows[0].0[3],"long English labels need one column");}
                    for (i,(label,value,right)) in rows.iter().enumerate() {
                        assert!(label[0]<label[2]&&label[2]<value[0],"label/value overlap at {width}, row {i}");
                        assert!(value[2]<=right+1.0,"value clipped at {width}, row {i}");
                        let boxes=[*label,*value];
                        for (other_label,other_value,_) in &rows[..i] {
                            for a in boxes {for b in [*other_label,*other_value] {
                                assert!(a[2]<=b[0]||b[2]<=a[0]||a[3]<=b[1]||b[3]<=a[1],"rows overlap at {width}, row {i}");
                            }}
                        }
                    }
                });
                if let Some(path)=&output {
                    crate::reward_ui::write_frame(&path.join(format!("build-{}-{width}.draw",language.id())),data,width,height);
                }
            }
            if let Some(path)=&output {
                // A shorter build also makes the complete class heading visible in the preview.
                let all=view.stats.lines.clone();view.stats.lines.truncate(5);
                ctx.io_mut().display_size=[1920.0,1080.0];
                for _ in 0..3 {draw(ctx.frame(),&run,&view,&mut |_|None);ctx.render();}
                draw(ctx.frame(),&run,&view,&mut |_|None);
                crate::reward_ui::write_frame(&path.join(format!("build-{}-class.draw",language.id())),ctx.render(),1920,1080);
                view.stats.lines=all;
            }
        }
    }
}
