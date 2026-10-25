use erdtree_tower::map_drawing::{self,Canvas,Edit,Point,Scope,StrokeId,MAX_POINTS};
use hudhook::imgui::{ImColor32,MouseButton,StyleColor,Ui};
#[derive(Clone,Copy,Default,PartialEq,Eq)]
pub enum Mode {#[default] Select,Draw,Erase}
#[derive(Default)]
struct Ink {scope:Option<Scope>,mode:Mode,draft:Vec<Point>,erased:Vec<StrokeId>,active:bool,last_time:f64}
thread_local! {static INK:std::cell::RefCell<Ink>=std::cell::RefCell::new(Ink::default());}
pub fn toolbar(ui:&Ui,scope:Scope,pos:[f32;2],scale:f32,ready:bool) {
    INK.with(|ink| {
        let mut ink=ink.borrow_mut();
        if ink.scope!=Some(scope){*ink=Ink{scope:Some(scope),..Default::default()};}
        ui.set_cursor_pos(pos);
        let _disabled=ui.begin_disabled(!ready);
        for (i,(mode,label)) in [(Mode::Select,"选路"),(Mode::Draw,"划线"),(Mode::Erase,"擦除")].into_iter().enumerate() {
            ui.set_cursor_pos([pos[0],pos[1]+i as f32*40.0*scale]);
            let _selected=(ink.mode==mode).then(||ui.push_style_color(StyleColor::Button,[0.53,0.40,0.16,1.0]));
            if ui.button_with_size(label,[108.0*scale,32.0*scale]) {ink.mode=mode;ink.active=false;ink.draft.clear();ink.erased.clear();}
        }
        ui.set_cursor_pos([pos[0],pos[1]+122.0*scale]);
        ui.text_wrapped(match ink.mode {Mode::Select=>"拖动地图或滚轮翻页",Mode::Draw=>"按住左键划线，松开同步",Mode::Erase=>"按住左键擦除整条笔迹"});
    });
}
pub fn selecting()->bool {INK.with(|ink|ink.borrow().mode==Mode::Select)}
fn screen(p:Point,origin:[f32;2],size:[f32;2])->[f32;2] {
    [origin[0]+f32::from(p.x)*size[0]/10000.0,origin[1]+f32::from(p.y)*size[1]/10000.0]
}
fn point(p:[f32;2],origin:[f32;2],size:[f32;2])->Point {
    Point{x:((p[0]-origin[0])/size[0]*10000.0).clamp(0.0,10000.0).round() as u16,
        y:((p[1]-origin[1])/size[1]*10000.0).clamp(0.0,10000.0).round() as u16}
}
fn append(points:&mut Vec<Point>,p:Point) {
    if points.last()==Some(&p){return;}
    if points.len()==MAX_POINTS {
        let last=*points.last().unwrap();
        *points=points.iter().step_by(2).copied().collect();
        if points.last()!=Some(&last){points.push(last);}
    }
    points.push(p);
}
pub fn overlay(ui:&Ui,scope:Scope,origin:[f32;2],size:[f32;2],ready:bool) {
    let canvas=crate::runtime::map_canvas().filter(|c|c.scope==scope);
    INK.with(|ink| {
        let mut ink=ink.borrow_mut();
        if ui.time()-ink.last_time>0.15||!ready {ink.active=false;ink.draft.clear();ink.erased.clear();}
        ink.last_time=ui.time();
        let mouse=ui.io().mouse_pos;
        let hovered=ui.is_window_hovered()&&ui.is_mouse_pos_valid(mouse);
        let inside=hovered&&(0..2).all(|a|mouse[a]>=origin[a]&&mouse[a]<=origin[a]+size[a]);
        if ready&&inside&&ink.mode!=Mode::Select&&ui.is_mouse_clicked(MouseButton::Left) {
            ink.active=true;ink.draft.clear();ink.erased.clear();
        }
        if ink.active&&(ui.is_mouse_down(MouseButton::Left)||ui.is_mouse_released(MouseButton::Left))&&inside {
            match ink.mode {
                Mode::Draw=>append(&mut ink.draft,point(mouse,origin,size)),
                Mode::Erase=>if let Some(canvas)=&canvas {for s in &canvas.strokes {
                    if !ink.erased.contains(&s.id)&&s.points.windows(2).any(|w|map_drawing::near_segment(mouse,screen(w[0],origin,size),screen(w[1],origin,size),12.0)) {ink.erased.push(s.id);}
                }},
                _=>{},
            }
        }
        let draw=ui.get_window_draw_list();
        if let Some(Canvas{strokes,..})=&canvas {for s in strokes {
            if ink.erased.contains(&s.id){continue;}
            let colors=[[36,94,176],[180,43,53],[30,123,68],[133,54,164],[183,95,24],[27,132,142],[162,54,108],[72,78,88]];
            let c=colors[(s.id.player as usize)%colors.len()];
            for w in s.points.windows(2) {draw.add_line(screen(w[0],origin,size),screen(w[1],origin,size),ImColor32::from_rgb(c[0],c[1],c[2])).thickness(3.5).build();}
        }}
        for w in ink.draft.windows(2) {draw.add_line(screen(w[0],origin,size),screen(w[1],origin,size),ImColor32::from_rgb(36,94,176)).thickness(3.5).build();}
        if ink.active&&!ui.is_mouse_down(MouseButton::Left) {
            let edit=match ink.mode {
                Mode::Draw if ink.draft.len()>=2=>Some(Edit::Add(std::mem::take(&mut ink.draft))),
                Mode::Erase if !ink.erased.is_empty()=>Some(Edit::Erase(std::mem::take(&mut ink.erased))),
                _=>None,
            };
            if let Some(edit)=edit {if !crate::runtime::submit_map_edit(scope,edit){crate::log("MAP_DRAWING UI edit rejected");}}
            ink.active=false;ink.draft.clear();ink.erased.clear();
        }
    });
}
#[cfg(test)]
pub fn test_mode(mode:Mode) {INK.with(|ink|ink.borrow_mut().mode=mode);}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalized_coordinates_preserve_map_positions_across_scrolling_and_resolutions() {
        let p=point([150.0,300.0],[50.0,100.0],[200.0,400.0]);
        assert_eq!(p,Point{x:5000,y:5000});
        assert_eq!(screen(p,[10.0,-400.0],[600.0,800.0]),[310.0,0.0]);
    }
    #[test]
    fn long_strokes_keep_their_endpoints_with_bounded_samples() {
        let mut points=vec![];
        for x in 0..1000 {append(&mut points,Point{x,y:x});}
        assert!(points.len()<=MAX_POINTS);assert_eq!(points[0].x,0);assert_eq!(points.last().unwrap().x,999);
    }
}
