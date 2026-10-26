use crate::runtime::{self, Action, View};
use erdtree_tower::{Class, NodeKind, Phase, Reward, Run};
use hudhook::imgui::{Condition, ImColor32, StyleColor, Ui};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct MapScroll {
    target: Option<f32>,
    anchor: Option<([f32; 2], f32)>,
    dragging: bool,
    last_time: f64,
}

impl MapScroll {
    fn gesture(&mut self, mouse: [f32; 2], start: bool, down: bool, current: f32, max: f32) -> bool {
        if start {
            self.anchor = Some((mouse, current));
            self.dragging = false;
            self.target = Some(current);
        }
        if let Some((origin, scroll)) = self.anchor {
            if (mouse[0]-origin[0]).hypot(mouse[1]-origin[1]) > 6.0 { self.dragging = true; }
            if self.dragging { self.target = Some((scroll + origin[1] - mouse[1]).clamp(0.0, max)); }
        }
        let blocked = self.dragging;
        if !down { self.anchor = None; self.dragging = false; }
        blocked
    }

    fn step(&mut self, current: f32, max: f32, wheel: f32, dt: f32, direct: bool) -> f32 {
        let target = (self.target.unwrap_or(current) - wheel * 120.0).clamp(0.0, max);
        self.target = Some(target);
        if direct || (target-current).abs() < 0.25 { target }
        else { current + (target-current) * (1.0-(-18.0*dt.clamp(0.0,0.1)).exp()) }
    }
}

thread_local! {static SCROLL: std::cell::RefCell<MapScroll> = std::cell::RefCell::new(MapScroll::default());}
#[cfg(test)]
static TEST_TARGET:std::sync::Mutex<([f32;2],bool)>=std::sync::Mutex::new(([0.0,0.0],false));
#[cfg(test)]
static TEST_FUTURE:std::sync::Mutex<([f32;2],bool)>=std::sync::Mutex::new(([0.0,0.0],false));
#[cfg(test)]
static TEST_ENTER:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(usize::MAX);

fn enter_node(id:usize) {
    #[cfg(test)]
    TEST_ENTER.store(id,Ordering::Relaxed);
    runtime::submit(Action::Enter(id));
}

#[cfg(test)]
mod tests {
    #[test]
    fn map_drag_suppresses_release_but_preserves_clicks() {
        let mut s = super::MapScroll::default();
        assert!(!s.gesture([50.0,50.0],true,true,100.0,500.0));
        assert!(!s.gesture([52.0,52.0],false,false,100.0,500.0));
        s.gesture([50.0,50.0],true,true,100.0,500.0);
        assert!(s.gesture([50.0,100.0],false,true,100.0,500.0));
        assert_eq!(s.step(100.0,500.0,0.0,0.016,true),50.0);
        assert!(s.gesture([50.0,50.0],false,false,50.0,500.0));
        assert!(!s.gesture([50.0,50.0],false,false,100.0,500.0));
    }

    #[test]
    fn wheel_scroll_eases_and_clamps_at_both_ends() {
        let mut s = super::MapScroll::default();
        let first=s.step(100.0,500.0,-1.0,1.0/60.0,false);
        assert!(first>100.0 && first<220.0);
        let mut current=first;
        for _ in 0..60 {current=s.step(current,500.0,0.0,1.0/60.0,false);}
        assert_eq!(current,220.0);
        assert_eq!(s.step(current,500.0,-100.0,0.016,true),500.0);
        assert_eq!(s.step(500.0,500.0,100.0,0.016,true),0.0);
    }
    #[test]
    fn full_route_renders_during_loading_and_after_player_ready() {
        let mut context = hudhook::imgui::Context::create();
        context.set_ini_filename(None);
        context.io_mut().display_size = [1920.0, 1080.0];
        context.io_mut().delta_time = 1.0 / 60.0;
        let font=std::fs::read("C:/Windows/Fonts/simhei.ttf").unwrap();
        context.fonts().add_font(&[hudhook::imgui::FontSource::TtfData{data:&font,size_pixels:20.0,config:Some(hudhook::imgui::FontConfig{glyph_ranges:hudhook::imgui::FontGlyphRanges::chinese_full(),..Default::default()})}]);
        context.fonts().build_rgba32_texture();
        crate::reward_ui::test_pad_frames(&mut context);
        let run = erdtree_tower::Run::new(42, erdtree_tower::Class::Samurai);
        let mut view = crate::runtime::view().unwrap();
        for ready in [false, true, true] {
            view.ready = ready;
            super::draw(context.frame(), &run, &view, "HP 455/455");
            assert!(context.render().total_vtx_count > 0);
        }
        crate::reward_ui::test_frames(&mut context);
        crate::run_screen::test_frames(&mut context);
        context.io_mut().config_flags.insert(hudhook::imgui::ConfigFlags::NAV_ENABLE_GAMEPAD);
        context.io_mut().backend_flags.insert(hudhook::imgui::BackendFlags::HAS_GAMEPAD);
        for key in [hudhook::imgui::Key::Escape,hudhook::imgui::Key::GamepadFaceRight] {
            context.io_mut().add_key_event(key,true);
            assert_eq!(super::draw(context.frame(),&run,&view,""),key==hudhook::imgui::Key::GamepadFaceRight,"Only B may close the map");
            context.render();
            context.io_mut().add_key_event(key,false);
            super::draw(context.frame(),&run,&view,"");context.render();
        }
        for (width,height) in [(1280,720),(1920,1080),(3440,1440)] {
            crate::map_art::preview();
            context.io_mut().display_size=[width as f32,height as f32];
            for _ in 0..3 {super::draw(context.frame(),&run,&view,"生命 455 / 455    专注 82 / 82");context.render();}
            context.io_mut().mouse_pos=super::TEST_TARGET.lock().unwrap().0;
            for _ in 0..20 {super::draw(context.frame(),&run,&view,"生命 455 / 455    专注 82 / 82");context.render();}
            let target=*super::TEST_TARGET.lock().unwrap();
            assert!(target.1,"available node should respond to pointer hover: {width}x{height}, {:?}",target.0);
            context.io_mut().config_flags.insert(hudhook::imgui::ConfigFlags::NAV_ENABLE_GAMEPAD);
            context.io_mut().backend_flags.insert(hudhook::imgui::BackendFlags::HAS_GAMEPAD);
            context.io_mut().add_key_event(hudhook::imgui::Key::GamepadFaceDown,true);
            super::TEST_ENTER.store(usize::MAX,super::Ordering::Relaxed);
            super::draw(context.frame(),&run,&view,"");context.render();
            assert!(super::TEST_TARGET.lock().unwrap().1,"A must retain the node under the map pointer");
            assert_eq!(super::TEST_ENTER.load(super::Ordering::Relaxed),run.available()[0],"A must activate the pointed node");
            super::TEST_ENTER.store(usize::MAX,super::Ordering::Relaxed);
            super::draw(context.frame(),&run,&view,"");context.render();
            assert_eq!(super::TEST_ENTER.load(super::Ordering::Relaxed),usize::MAX,"Holding A must not repeat travel");
            context.io_mut().add_key_event(hudhook::imgui::Key::GamepadFaceDown,false);
            super::draw(context.frame(),&run,&view,"");context.render();
            let target=super::TEST_TARGET.lock().unwrap().0;
            crate::reward_ui::test_map_pad(context.io_mut(),target,false);
            super::draw(context.frame(),&run,&view,"");context.render();
            crate::reward_ui::test_map_pad(context.io_mut(),target,true);
            context.io_mut().add_mouse_pos_event([0.0,0.0]);
            context.io_mut().add_key_event(hudhook::imgui::Key::GamepadFaceDown,false);
            super::TEST_ENTER.store(usize::MAX,super::Ordering::Relaxed);
            super::draw(context.frame(),&run,&view,"");context.render();
            assert_eq!(super::TEST_ENTER.load(super::Ordering::Relaxed),run.available()[0],"Map must retain controller input when backend events overwrite ImGui input");
            crate::reward_ui::test_map_pad(context.io_mut(),target,true);
            super::TEST_ENTER.store(usize::MAX,super::Ordering::Relaxed);
            super::draw(context.frame(),&run,&view,"");context.render();
            assert_eq!(super::TEST_ENTER.load(super::Ordering::Relaxed),usize::MAX);
            crate::reward_ui::poll_pad(context.io_mut(),false,false);
            super::draw(context.frame(),&run,&view,"");context.render();
            context.io_mut().add_mouse_pos_event(super::TEST_FUTURE.lock().unwrap().0);
            for _ in 0..20 {super::draw(context.frame(),&run,&view,"生命 455 / 455    专注 82 / 82");context.render();}
            assert!(super::TEST_FUTURE.lock().unwrap().1,"future nodes must support hover without unlocking travel");
            for mode in [crate::map_ink::Mode::Draw,crate::map_ink::Mode::Erase] {
                crate::map_ink::test_mode(mode);
                context.io_mut().mouse_pos=super::TEST_TARGET.lock().unwrap().0;
                super::TEST_ENTER.store(usize::MAX,super::Ordering::Relaxed);
                context.io_mut().mouse_down[0]=true;
                super::draw(context.frame(),&run,&view,"");context.render();
                context.io_mut().mouse_down[0]=false;
                super::draw(context.frame(),&run,&view,"");context.render();
                assert_eq!(super::TEST_ENTER.load(super::Ordering::Relaxed),usize::MAX,"Drawing and erasing must never activate a node");
            }
            crate::map_ink::test_mode(crate::map_ink::Mode::Select);
            let before=super::TEST_TARGET.lock().unwrap().0[1];
            context.io_mut().mouse_pos=[width as f32*0.5,height as f32*0.5];
            super::draw(context.frame(),&run,&view,"");context.render();
            context.io_mut().mouse_down[0]=true;
            super::draw(context.frame(),&run,&view,"");context.render();
            context.io_mut().mouse_pos[1]+=100.0;
            for _ in 0..3 {super::draw(context.frame(),&run,&view,"");context.render();}
            assert!((super::TEST_TARGET.lock().unwrap().0[1]-before-100.0).abs()<2.0,"drag should move the map with the pointer");
            context.io_mut().mouse_down[0]=false;
            super::draw(context.frame(),&run,&view,"");context.render();
            super::draw(context.frame(),&run,&view,"生命 455 / 455    专注 82 / 82");
            let data=context.render();assert!(data.total_vtx_count>0);
            if let Ok(path)=std::env::var("ERDTREE_UI_PREVIEW") {
                use std::io::Write;
                let mut f=std::fs::File::create(std::path::Path::new(&path).join(format!("map-{width}-{height}.draw"))).unwrap();
                writeln!(f,"{width} {height}").unwrap();
                for list in data.draw_lists(){for command in list.commands(){if let hudhook::imgui::DrawCmd::Elements{count,cmd_params:p}=command {
                    writeln!(f,"C {} {} {} {} {}",p.texture_id.id(),p.clip_rect[0],p.clip_rect[1],p.clip_rect[2],p.clip_rect[3]).unwrap();
                    for triangle in list.idx_buffer()[p.idx_offset..p.idx_offset+count].chunks_exact(3) {
                        write!(f,"T").unwrap();for index in triangle {let v=list.vtx_buffer()[*index as usize+p.vtx_offset];write!(f," {} {} {} {} {} {} {} {}",v.pos[0],v.pos[1],v.uv[0],v.uv[1],v.col[0],v.col[1],v.col[2],v.col[3]).unwrap();}writeln!(f).unwrap();
                    }
                }}}
            }
        }
    }
}

pub fn class_name(class: Class) -> &'static str {
    match class {
        Class::Vagabond => "流浪骑士",
        Class::Samurai => "武士",
        Class::Astrologer => "观星者",
        Class::Prophet => "预言家",
        Class::Warrior => "剑士",
        Class::Hero => "勇者",
        Class::Bandit => "盗贼",
        Class::Prisoner => "囚犯",
        Class::Confessor => "密使",
        Class::Wretch => "一贫如洗",
    }
}


pub fn draw(ui: &Ui, run: &Run, view: &View, _vitals: &str)->bool {
    if ui.is_key_pressed_no_repeat(hudhook::imgui::Key::GamepadFaceRight) {return true;}
    let size=ui.io().display_size;
    let scale=(size[0]/1600.0).min(size[1]/900.0).clamp(0.65,1.6);
    let left=0.0;let right=165.0*scale;let margin=18.0*scale;
    let gold=[0.84,0.72,0.46,1.0];
    let _background=ui.push_style_color(StyleColor::WindowBg,[0.04,0.05,0.04,0.96]);
    let _text=ui.push_style_color(StyleColor::Text,gold);
    let _button=ui.push_style_color(StyleColor::Button,[0.28,0.23,0.13,1.0]);
    let _hover=ui.push_style_color(StyleColor::ButtonHovered,[0.45,0.35,0.18,1.0]);
    ui.window("黄金树爬塔##route").position([0.0,0.0],Condition::Always).size(size,Condition::Always)
        .flags(hudhook::imgui::WindowFlags::NO_NAV)
        .title_bar(false).resizable(false).movable(false).collapsible(false).scroll_bar(false).build(|| {
            crate::oath_art::background(&ui.get_window_draw_list(),size);
            ui.set_window_font_scale(scale);
            ui.set_cursor_pos([size[0]-right+margin,65.0*scale]);ui.text("图例");
            for (i,kind) in [NodeKind::Monster,NodeKind::Elite,NodeKind::Camp,NodeKind::Shop,NodeKind::Chest,NodeKind::Event,NodeKind::Boss].into_iter().enumerate() {
                let y=(126.0+i as f32*62.0)*scale;
                let draw=ui.get_window_draw_list();let p=[size[0]-right+35.0*scale,y];
                draw.add_circle(p,23.0*scale,ImColor32::from_rgb(203,183,136)).filled(true).build();
                if !crate::map_art::icon_scaled(&draw,p,kind,1.0,0.8*scale){glyph(&draw,p,kind,ImColor32::from_rgb(65,49,28));}
                ui.set_cursor_pos([size[0]-right+68.0*scale,y-10.0*scale]);ui.text(if kind==NodeKind::Boss{"首领"}else{kind.label()});
            }
            crate::map_ink::toolbar(ui,erdtree_tower::map_drawing::Scope::of(run),[size[0]-right+margin,555.0*scale],scale,view.ready&&!view.busy&&run.phase==Phase::Map);
            ui.set_cursor_pos([left+margin,margin]);
            let _paper=ui.push_style_color(StyleColor::ChildBg,[0.84,0.79,0.66,1.0]);
            let _ink=ui.push_style_color(StyleColor::Text,[0.20,0.17,0.11,1.0]);
            let _popup=ui.push_style_color(StyleColor::PopupBg,[0.91,0.86,0.72,1.0]);
            ui.child_window("route-content").flags(hudhook::imgui::WindowFlags::NO_NAV).scroll_bar(false).scrollable(false).size([size[0]-left-right-2.0*margin,size[1]-2.0*margin]).build(|| {
                let _disabled=ui.begin_disabled(!view.ready||view.busy);
                match &run.phase {
                    Phase::Reward { .. } => { ui.text("选择战利品以继续旅程。"); }
                    Phase::Dead => { ui.text("旅程结束"); ui.text_wrapped("角色死亡。重新启动游戏可开始新一局。"); }
                    Phase::Won => { ui.text("登顶 · 本局通关"); }
                    Phase::Map => route(ui, run, view.ready && !view.busy,&view.ballots),
                    Phase::Node { id } => {
                        match run.nodes[*id].kind {
                            NodeKind::Camp | NodeKind::Chest | NodeKind::Shop => { ui.text("使用房间内的原生交互继续。"); }
                            NodeKind::Event => {
                                ui.text("进入事件区域，开启宝匣或与守坛人交谈。");
                            }
                            _ => {}
                        }
                    }
                    _ => { ui.text_wrapped(&view.status); }
                }
            });
            drop(_popup);drop(_ink);drop(_paper);
        });
    false
}
pub(crate) fn reward_description(run: &Run, reward: &Reward) -> String {
    match reward {
        Reward::Blessing(kind) => super::blessings::description(
            *kind,
            run.blessings
                .iter()
                .find(|b| b.kind == *kind)
                .map_or(1, |b| (b.level + 1).min(3)),
        ),
        Reward::UpgradeWeapon => {
            "右手武器升至背包最高等效等级，再普通 +3 / 失色 +1；保留质变与战灰，满级封顶"
                .into()
        }
        Reward::Gold(amount) => format!("获得 {amount} 卢恩"),
        Reward::FlaskCapacity=>"红、蓝圣杯瓶上限各 +1；本局生效，不补充剩余次数".into(),
        Reward::MemorySlot=>"记忆栏位 +1；魔法与祷告共用，本局生效".into(),
        Reward::StarStream=>"彗星亚兹勒随视角左右瞄准，光束保持水平；本局生效".into(),
        Reward::TalismanBuff(id)=>format!("{}：直接生效，不占护符栏位，持续本局",erdtree_tower::rewards::lookup(0x20000000|id).map_or("护符效果",|i|i.name)),
        Reward::SacredTear=>"圣杯滴露 ×1：交给服务 NPC 提升红、蓝瓶恢复量".into(),
        Reward::Item(id)=>erdtree_tower::rewards::lookup(*id).map_or("未知奖励".into(),|item|
            format!("{}：{}（领取实物，按原版要求装备或记忆）",item.kind.label(),item.name)),
    }
}

fn route(ui: &Ui, run: &Run, ready: bool,ballots:&[erdtree_tower::party_vote::Ballot]) {
    let selecting=crate::map_ink::selecting();
    let available = run.available();
    let origin = ui.cursor_screen_pos();
    let content = ui.content_region_avail();
    let viewport = ui.window_size()[1] - 16.0;
    let (dragged, scroll) = SCROLL.with(|state| {
        let mut state=state.borrow_mut();
        if ui.time()-state.last_time > 0.15 { *state=MapScroll::default(); }
        state.last_time=ui.time();
        let mouse=ui.io().mouse_pos;
        let hovered=ui.is_window_hovered();
        let left=hudhook::imgui::MouseButton::Left;
        if !ready || !selecting || !ui.is_mouse_pos_valid(mouse) {state.anchor=None;state.dragging=false;}
        let blocked=state.gesture(mouse,ready&&selecting&&hovered&&ui.is_mouse_clicked(left),ui.is_mouse_down(left),ui.scroll_y(),ui.scroll_max_y());
        let wheel=if ready&&hovered&&state.anchor.is_none(){ui.io().mouse_wheel}else{0.0};
        let next=state.step(ui.scroll_y(),ui.scroll_max_y(),wheel,ui.io().delta_time,blocked);
        (blocked,next)
    });
    let last=run.floors()-1;
    let height = viewport.max(170.0+last as f32*120.0);
    let row = (height - 170.0) / last as f32;
    let radius = 24.0;
    let span=content[0].min(1060.0);
    let mut reachable = vec![false; run.nodes.len()];
    for id in run.start_nodes() {reachable[id]=true;}
    for id in 0..run.nodes.len() {if reachable[id] {for &next in &run.nodes[id].next {reachable[next]=true;}}}
    let mut peers=vec![0usize;run.nodes.iter().map(|n|n.layer+1).max().unwrap_or(0)];let mut ranks=vec![0usize;run.nodes.len()];
    for (i,n) in run.nodes.iter().enumerate() {if reachable[i] {ranks[i]=peers[n.layer];peers[n.layer]+=1;}}
    let points:Vec<[f32;2]>=(0..run.nodes.len()).map(|id| {
        let layer=run.nodes[id].layer;let count=peers[layer];let rank=ranks[id];
        let center=if count<=1 {0.5}else{0.13+rank as f32*0.74/(count-1) as f32};
        let jitter = ((run.seed.wrapping_add(id as u64 * 7919) % 101) as f32 / 100.0 - 0.5) * 0.065;
        let sway = ((layer as f32 * 1.7).sin()) * 0.045;
        [
            origin[0] + (content[0]-span)*0.5+span * (if count<=1 {0.5}else{center+jitter+sway}),
            origin[1] + 70.0 + (last - layer) as f32 * row,
        ]
    }).collect();
    let point=|id:usize|points[id];
    let draw = ui.get_window_draw_list();
    let ink = ImColor32::from_rgb(64, 54, 38);
    let mut hovered=None;let mut pressed=None;
    let pad=crate::reward_ui::map_command();
    let pad_point=pad.and_then(|(point,_)|point);
    let confirm=pad.map_or_else(||ui.is_key_pressed_no_repeat(hudhook::imgui::Key::GamepadFaceDown),|(_,pressed)|pressed);
    let clip_min=ui.window_pos();let clip_max=[clip_min[0]+ui.window_size()[0],clip_min[1]+ui.window_size()[1]];
    for id in (0..run.nodes.len()).filter(|&id|reachable[id]) {
        let pos=point(id);
        ui.set_cursor_screen_pos([pos[0]-36.0,pos[1]-36.0]);
        let enabled=ready&&selecting&&available.contains(&id);
        let clicked=selecting&&ui.invisible_button(format!("node-{id}"),[72.0,72.0]);
        let pointed=if let Some(p)=pad_point {
            ui.is_item_visible()&&(0..2).all(|axis|p[axis]>=clip_min[axis]&&p[axis]<clip_max[axis]&&(p[axis]-pos[axis]).abs()<=36.0)
        }else{ui.is_window_hovered()&&ui.is_item_visible()&&ui.is_mouse_hovering_rect([pos[0]-36.0,pos[1]-36.0],[pos[0]+36.0,pos[1]+36.0])};
        if enabled&&!dragged&&(clicked||pointed&&confirm){enter_node(id);}
        if pointed&&selecting {
            hovered=Some(id);ui.set_mouse_cursor(Some(if enabled{hudhook::imgui::MouseCursor::Hand}else{hudhook::imgui::MouseCursor::Arrow}));
        }
        if ui.is_item_active()&&enabled {pressed=Some(id);}
    }
    thread_local! {static EMPHASIS:std::cell::RefCell<Vec<f32>>=const {std::cell::RefCell::new(Vec::new())};}
    let emphasis=EMPHASIS.with(|values| {
        let mut values=values.borrow_mut();values.resize(run.nodes.len(),0.0);
        for (id,value) in values.iter_mut().enumerate(){let target=if hovered==Some(id){1.0}else{0.0};*value+=(target-*value)*(ui.io().delta_time*14.0).min(1.0);}
        values.clone()
    });
    #[cfg(test)]
    if let Some(&id)=available.first(){*TEST_TARGET.lock().unwrap()=(point(id),hovered==Some(id));}
    #[cfg(test)]
    if let Some(id)=available.first().and_then(|&id|run.nodes[id].next.first()).copied(){*TEST_FUTURE.lock().unwrap()=(point(id),hovered==Some(id));}
    crate::map_art::paper(&draw,[origin[0]-8.0,origin[1]-8.0],[origin[0]+content[0]+8.0,origin[1]+height+8.0]);
    for n in 0..500 {
        let x=origin[0]+(n*71%997) as f32/997.0*content[0];let y=origin[1]+(n*137%1451) as f32/1451.0*height;
        draw.add_line([x,y],[x+3.0,y+1.0],ImColor32::from_rgba(109,87,48,25)).build();
    }
    draw.add_line([origin[0]+39.0,origin[1]+65.0],[origin[0]+39.0,origin[1]+height-80.0],ImColor32::from_rgba(98,81,50,65)).build();
    for floor in 0..run.floors() {
        let y = origin[1] + 60.0 + (last - floor) as f32 * row;
        draw.add_text(
            [origin[0] + 8.0, y - 8.0],
            ImColor32::from_rgba(98, 81, 50, 140),
            format!("{:02}", floor + 1),
        );
    }
    for (id, node) in run.nodes.iter().enumerate() {
        if !reachable[id] { continue; }
        for &next in &node.next {
            let preview=hovered==Some(id)||hovered==Some(next);
            let taken = run.visited.windows(2).any(|w| w == [id, next]);
            let a = point(id);
            let b = point(next);
            let curve = |t: f32| {
                let smooth = t * t * (3.0 - 2.0 * t);
                [
                    a[0] + (b[0] - a[0]) * smooth,
                    a[1] + (b[1] - a[1]) * t,
                ]
            };
            let color = if preview {
                ImColor32::from_rgb(191,117,15)
            } else if taken {
                ImColor32::from_rgb(60, 94, 67)
            } else if run.visited.last()==Some(&id)&&available.contains(&next) {
                ImColor32::from_rgb(179,126,32)
            } else {
                ImColor32::from_rgba(78, 64, 39, 165)
            };
            let steps=(((b[0]-a[0]).abs()+(b[1]-a[1]).abs())/7.0).ceil().max(30.0) as usize;
            for segment in 0..steps {
                let p=curve(segment as f32/steps as f32);let q=curve((segment+1) as f32/steps as f32);
                if (p[0]-a[0]).hypot(p[1]-a[1])<radius+6.0||(q[0]-b[0]).hypot(q[1]-b[1])<radius+6.0 {continue;}
                if taken || segment % 3 != 0 {
                    draw.add_line(
                        p,
                        q,
                        color,
                    )
                    .thickness(if preview {3.8} else if taken { 3.0 } else { 1.5 })
                    .build();
                }
            }
        }
    }
    for (id, node) in run.nodes.iter().enumerate() {
        if !reachable[id] { continue; }
        let pos = point(id);
        let enabled = ready && available.contains(&id);
        let focus=emphasis[id];
        let zoom=1.0+0.18*focus-if pressed==Some(id){0.09}else{0.0};
        let visited = run.visited.contains(&id);
        let color = if visited {
            ImColor32::from_rgb(50, 95, 61)
        } else if enabled {
            ink
        } else {
            ImColor32::from_rgb(139, 122, 88)
        };
        let paper = if enabled {
            ImColor32::from_rgb(232, 202, 127)
        } else {
            ImColor32::from_rgb(215, 202, 169)
        };
        if enabled {
            for ring in 0..4 {
                draw.add_circle(pos,radius+5.0+ring as f32*4.0,ImColor32::from_rgba(233,163,43,(focus*(50.0-ring as f32*10.0)) as u8)).thickness(5.0).build();
            }
            let pulse = (ui.time() as f32 * 2.0).sin() * 2.0;
            draw.add_circle(
                pos,
                radius + 9.0 + pulse,
                ImColor32::from_rgba(149, 102, 26, 115),
            )
            .thickness(1.5)
            .build();
        }
        if enabled { draw.add_circle(pos, radius*zoom, paper).filled(true).build(); }
        if enabled || run.visited.last()==Some(&id) {
            draw.add_circle(pos, radius*zoom, color).thickness(2.0+focus).build();
        }
        if !crate::map_art::icon_scaled(&draw,pos,node.kind,if enabled||visited{1.0}else{0.8},zoom){glyph(&draw, pos, node.kind, color);}
        if run.visited.last()==Some(&id) {draw.add_text([pos[0]-38.0,pos[1]+32.0],color,"当前位置");}
        if node.kind==NodeKind::Boss {draw.add_text([pos[0]-38.0,pos[1]+32.0],color,"最终试炼");}
        if visited {
            draw.add_circle([pos[0] + 18.0, pos[1] + 17.0], 4.0, color)
                .filled(true)
                .build();
        }
        let voters:Vec<_>=ballots.iter().enumerate().filter(|(_,b)|b.node==Some(id)).collect();
        for (offset,(index,_)) in voters.iter().enumerate() {
            let x=pos[0]+(offset as f32-(voters.len()-1) as f32*0.5)*23.0;
            let y=pos[1]-radius-17.0;
            let colors=[[62,114,185],[173,67,68],[53,135,86],[154,102,179],[180,126,42],[52,139,142],[171,91,129],[99,103,114]];
            let c=colors[*index%colors.len()];
            draw.add_circle([x,y],10.0,ImColor32::from_rgb(c[0],c[1],c[2])).filled(true).build();
            draw.add_text([x-5.0,y-10.0],ImColor32::WHITE,(index+1).to_string());
        }
        if hovered==Some(id) {
                let mut lines=vec![if node.kind==NodeKind::Boss {
                    crate::enemies::node_roster(run,id).first().map_or("首领",|enemy|enemy.name).to_owned()
                } else {node.kind.label().to_owned()}];
                let affixes=erdtree_tower::boss_affixes::roll(run,id);
                if !affixes.is_empty() {
                    lines[0]=format!("{} {}",erdtree_tower::boss_affixes::title(&affixes),lines[0]);
                    lines.extend(affixes.iter().map(|a|format!("{}：{}",a.name(),a.description())));
                    lines.push(format!("胜利额外获得 {} 卢恩",erdtree_tower::boss_affixes::bonus_runes(run,id)));
                }
                let width=lines.iter().map(|l|ui.calc_text_size(l)[0]).fold(0.0,f32::max);
                draw.add_rect([pos[0]-width*0.5-9.0,pos[1]+39.0],[pos[0]+width*0.5+9.0,pos[1]+69.0+22.0*(lines.len()-1) as f32],ImColor32::from_rgb(77,57,29)).rounding(5.0).filled(true).build();
                for (row,line) in lines.iter().enumerate() {
                    let x=if row==0 {pos[0]-ui.calc_text_size(line)[0]*0.5}else{pos[0]-width*0.5};
                    draw.add_text([x,pos[1]+43.0+22.0*row as f32],if row==0 {ImColor32::from_rgb(255,231,168)}else{ImColor32::from_rgb(232,214,180)},line);
                }
                draw.add_circle(pos, radius + 5.0, ImColor32::from_rgb(143, 95, 22))
                    .thickness(2.0)
                    .build();
        }
    }
    drop(draw);
    crate::map_ink::overlay(ui,erdtree_tower::map_drawing::Scope::of(run),[origin[0]+(content[0]-span)*0.5,origin[1]+70.0],[span,height-170.0],ready);
    ui.set_cursor_screen_pos(origin);
    ui.dummy([content[0], height]);
    ui.set_scroll_y(scroll);
    static LAST_ROUTE: AtomicU64 = AtomicU64::new(u64::MAX);
    let signature=run.seed.rotate_left(8)^run.visited.len() as u64
        ^u64::from(ui.io().display_size[0].to_bits()).rotate_left(29)^u64::from(ui.io().display_size[1].to_bits());
    if LAST_ROUTE.swap(signature, Ordering::Relaxed) != signature {
        if let Some(id) = available.first() {
            ui.set_scroll_y(((last - run.nodes[*id].layer) as f32 * row - viewport * 0.62 + 70.0).max(0.0));
            SCROLL.with(|state| *state.borrow_mut()=MapScroll::default());
        }
    }
}
fn glyph(draw: &hudhook::imgui::DrawListMut<'_>, p: [f32; 2], kind: NodeKind, color: ImColor32) {
    let q = |x: f32, y: f32| [p[0] + x, p[1] + y];
    let line = |a: [f32; 2], b: [f32; 2]| {
        draw.add_line(q(a[0], a[1]), q(b[0], b[1]), color)
            .thickness(2.5)
            .build();
    };
    match kind {
        NodeKind::Monster | NodeKind::Elite => {
            line([-11.0, 12.0], [11.0, -12.0]);
            line([11.0, 12.0], [-11.0, -12.0]);
            line([-12.0, 3.0], [-3.0, 12.0]);
            line([12.0, 3.0], [3.0, 12.0]);
            if kind == NodeKind::Elite {
                draw.add_circle(q(0.0, -3.0), 8.0, color)
                    .thickness(2.0)
                    .build();
            }
        }
        NodeKind::Camp => {
            line([-12.0, 13.0], [12.0, 7.0]);
            line([-12.0, 7.0], [12.0, 13.0]);
            for (a, b) in [
                ([-9.0, 4.0], [-7.0, -6.0]),
                ([-7.0, -6.0], [-2.0, -2.0]),
                ([-2.0, -2.0], [2.0, -14.0]),
                ([2.0, -14.0], [10.0, -2.0]),
                ([10.0, -2.0], [7.0, 5.0]),
                ([7.0, 5.0], [-9.0, 4.0]),
            ] {
                line(a, b);
            }
        }
        NodeKind::Chest => {
            draw.add_rect(q(-13.0, -6.0), q(13.0, 11.0), color)
                .rounding(2.0)
                .thickness(2.0)
                .build();
            line([-13.0, 0.0], [13.0, 0.0]);
            line([-13.0, -6.0], [-8.0, -12.0]);
            line([-8.0, -12.0], [8.0, -12.0]);
            line([8.0, -12.0], [13.0, -6.0]);
            draw.add_rect(q(-2.0, -2.0), q(2.0, 5.0), color)
                .filled(true)
                .build();
        }
        NodeKind::Shop => {
            draw.add_circle(q(0.0, 4.0), 11.0, color)
                .thickness(2.5)
                .build();
            line([-6.0, -6.0], [-9.0, -14.0]);
            line([-9.0, -14.0], [9.0, -14.0]);
            line([9.0, -14.0], [6.0, -6.0]);
            line([-7.0, -6.0], [7.0, -6.0]);
            line([0.0, -1.0], [0.0, 9.0]);
            line([-3.0, 3.0], [3.0, 3.0]);
        }
        NodeKind::Boss => {
            for (a, b) in [
                ([-14.0, -10.0], [-9.0, 10.0]),
                ([-9.0, 10.0], [9.0, 10.0]),
                ([9.0, 10.0], [14.0, -10.0]),
                ([14.0, -10.0], [5.0, -2.0]),
                ([5.0, -2.0], [0.0, -14.0]),
                ([0.0, -14.0], [-5.0, -2.0]),
                ([-5.0, -2.0], [-14.0, -10.0]),
            ] {
                line(a, b);
            }
            line([-8.0, 5.0], [8.0, 5.0]);
        }
        NodeKind::Event => {
            draw.add_text(q(-5.0, -11.0), color, "?");
        }
    }
}
