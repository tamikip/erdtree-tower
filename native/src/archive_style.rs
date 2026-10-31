// 旅程档案里武器偏好、防具幻化两页共用的样式：暗底金边的按钮、带外发光的卡片、角标、搜索框、页码圆点与菱形等级格。
// 坐标都是屏幕像素，s 为界面缩放（1600×900 画布）。不用 ImGui 默认的蓝色控件。
use hudhook::imgui::{DrawListMut,ImColor32,StyleColor,StyleVar,Ui};

pub const GOLD:[f32;4]=[0.94,0.82,0.56,1.0];
pub const WHITE:[f32;4]=[0.95,0.93,0.86,1.0];
pub const MUTED:[f32;4]=[0.72,0.70,0.65,1.0];
pub const DIM:[f32;4]=[0.48,0.46,0.42,1.0];
pub const RED:[f32;4]=[0.9,0.42,0.34,1.0];

pub fn rgba(c:[f32;4],a:f32)->ImColor32 {ImColor32::from_rgba((c[0]*255.0) as u8,(c[1]*255.0) as u8,(c[2]*255.0) as u8,(a.clamp(0.0,1.0)*255.0) as u8)}
fn gold(a:f32)->ImColor32 {ImColor32::from_rgba(226,190,120,(a.clamp(0.0,1.0)*255.0) as u8)}

/// 卡片底：上亮下暗的渐变与细边；聚焦（选中）时金边加一圈外发光，悬停时边框变亮
pub fn card(draw:&DrawListMut<'_>,min:[f32;2],max:[f32;2],focus:bool,hover:bool,s:f32) {
    if focus {for ring in 1..=3 {let r=ring as f32*2.5*s;draw.add_rect([min[0]-r,min[1]-r],[max[0]+r,max[1]+r],gold(0.10/ring as f32)).rounding(5.0*s).thickness(2.0*s).build();}}
    let (top,bottom)=if focus {(ImColor32::from_rgba(52,43,28,245),ImColor32::from_rgba(24,20,14,245))}else{(ImColor32::from_rgba(30,28,24,238),ImColor32::from_rgba(14,13,11,238))};
    draw.add_rect_filled_multicolor(min,max,top,top,bottom,bottom);
    let border=if focus {gold(0.95)}else if hover {gold(0.55)}else{ImColor32::from_rgba(110,98,74,120)};
    draw.add_rect(min,max,border).thickness(if focus{1.6*s}else{s}).build();
}
/// 文字在 min..max 里居中（scale 按 20 像素字号算）
pub fn centered(ui:&Ui,label:&str,min:[f32;2],max:[f32;2],color:[f32;4],scale:f32,s:f32) {
    ui.set_window_font_scale(scale*s);let size=ui.calc_text_size(label);
    ui.set_cursor_screen_pos([(min[0]+max[0]-size[0])*0.5,(min[1]+max[1]-size[1])*0.5]);ui.text_colored(color,label);
}
#[derive(Clone,Copy,PartialEq,Eq)]
pub enum Tone {Normal,Primary,Disabled}
/// 主题按钮：暗底金边，悬停时边框与文字变亮、底色从左往右铺一层金；Primary 的底色带金，Disabled 灰掉且点不动
pub fn button(ui:&Ui,id:&str,label:&str,min:[f32;2],max:[f32;2],tone:Tone,s:f32)->bool {button_pad(ui,id,label,None,min,max,tone,s)}
/// 同 button，文字左边加一个按键标签（手柄 / 键盘随最后用的设备切换）
#[allow(clippy::too_many_arguments)]
pub fn button_pad(ui:&Ui,id:&str,label:&str,input:Option<crate::pad::Input>,min:[f32;2],max:[f32;2],tone:Tone,s:f32)->bool {
    ui.set_cursor_screen_pos(min);ui.set_window_font_scale(s);
    let clicked=ui.invisible_button(id,[max[0]-min[0],max[1]-min[1]]);let hover=ui.is_item_hovered()&&tone!=Tone::Disabled;
    let draw=ui.get_window_draw_list();
    let (top,bottom)=match tone {
        Tone::Primary=>(ImColor32::from_rgba(92,72,38,240),ImColor32::from_rgba(52,40,22,240)),
        Tone::Normal=>(ImColor32::from_rgba(34,30,24,235),ImColor32::from_rgba(18,16,13,235)),
        Tone::Disabled=>(ImColor32::from_rgba(28,27,25,220),ImColor32::from_rgba(20,19,18,220)),
    };
    draw.add_rect_filled_multicolor(min,max,top,top,bottom,bottom);
    if hover {draw.add_rect_filled_multicolor(min,max,gold(0.16),gold(0.04),gold(0.04),gold(0.16));}
    let border=match tone {Tone::Disabled=>ImColor32::from_rgba(90,84,72,140),_ if hover=>gold(1.0),Tone::Primary=>gold(0.85),Tone::Normal=>gold(0.5)};
    draw.add_rect(min,max,border).thickness(if hover{1.6*s}else{1.1*s}).build();
    // 两端的小菱形，像档案里其他按钮的金饰
    if tone!=Tone::Disabled {
        let y=(min[1]+max[1])*0.5;let r=3.5*s;
        for x in [min[0],max[0]] {draw.add_polyline(vec![[x,y-r],[x+r,y],[x,y+r],[x-r,y]],border).filled(true).build();}
    }
    drop(draw);
    let color=match tone {Tone::Disabled=>DIM,_ if hover=>[1.0,0.95,0.8,1.0],Tone::Primary=>GOLD,Tone::Normal=>WHITE};
    crate::pad::label_in(ui,min,max,input.filter(|_|tone!=Tone::Disabled),label,color,s,crate::pad::Tone::Light);
    clicked&&tone!=Tone::Disabled
}
/// 右上角的金色角标（如“当前外观”）：right_top 为卡片右上角
pub fn ribbon(ui:&Ui,label:&str,right_top:[f32;2],s:f32) {
    ui.set_window_font_scale(0.78*s);let size=ui.calc_text_size(label);
    let (w,h)=(size[0]+16.0*s,size[1]+6.0*s);let min=[right_top[0]-w,right_top[1]];
    let draw=ui.get_window_draw_list();
    draw.add_polyline(vec![[min[0]-8.0*s,min[1]],[right_top[0],min[1]],[right_top[0],min[1]+h],[min[0],min[1]+h]],gold(0.95)).filled(true).build();
    drop(draw);
    ui.set_cursor_screen_pos([min[0]+8.0*s-4.0*s,min[1]+3.0*s]);ui.text_colored([0.12,0.09,0.05,1.0],label);
}
/// 搜索框：暗色内凹的框、金色细边、左边放大镜、右边写结果数；返回（内容是否改变, 是否正在输入）
pub fn search(ui:&Ui,id:&str,value:&mut String,min:[f32;2],max:[f32;2],hint:&str,count:&str,s:f32)->(bool,bool) {
    let draw=ui.get_window_draw_list();
    draw.add_rect_filled_multicolor(min,max,ImColor32::from_rgba(8,8,7,235),ImColor32::from_rgba(8,8,7,235),ImColor32::from_rgba(18,16,13,235),ImColor32::from_rgba(18,16,13,235));
    drop(draw);
    let (cx,cy,r)=(min[0]+20.0*s,(min[1]+max[1])*0.5-1.0*s,6.5*s);
    ui.set_window_font_scale(s);
    let height=ui.text_line_height();
    ui.set_cursor_screen_pos([min[0]+38.0*s,(min[1]+max[1]-height)*0.5-3.0*s]);ui.set_next_item_width(max[0]-min[0]-130.0*s);
    let changed={
        let _colors=[ui.push_style_color(StyleColor::FrameBg,[0.0,0.0,0.0,0.0]),ui.push_style_color(StyleColor::FrameBgHovered,[0.0,0.0,0.0,0.0]),
            ui.push_style_color(StyleColor::FrameBgActive,[0.0,0.0,0.0,0.0]),ui.push_style_color(StyleColor::Text,WHITE),ui.push_style_color(StyleColor::TextSelectedBg,[0.6,0.48,0.25,0.6])];
        let _border=ui.push_style_var(StyleVar::FrameBorderSize(0.0));
        ui.input_text(id,value).hint(hint).build()
    };
    let typing=ui.is_item_active();let hover=ui.is_item_hovered();
    let draw=ui.get_window_draw_list();
    draw.add_rect(min,max,if typing {gold(0.95)}else if hover {gold(0.6)}else{gold(0.35)}).thickness(if typing{1.5*s}else{s}).build();
    let lens=if typing {gold(1.0)}else{rgba(MUTED,0.9)};
    draw.add_circle([cx,cy],r,lens).thickness(1.6*s).num_segments(20).build();
    draw.add_line([cx+r*0.7,cy+r*0.7],[cx+r*1.6,cy+r*1.6],lens).thickness(2.0*s).build();
    drop(draw);
    ui.set_window_font_scale(0.85*s);let size=ui.calc_text_size(count);
    ui.set_cursor_screen_pos([max[0]-16.0*s-size[0],(min[1]+max[1]-size[1])*0.5]);ui.text_colored(MUTED,count);
    (changed,typing)
}
/// 页码圆点：当前页实心金色；页数太多时按比例只画最多 15 个
pub fn dots(draw:&DrawListMut<'_>,center:[f32;2],pages:usize,current:usize,s:f32) {
    let shown=pages.clamp(1,15);let gap=14.0*s;let start=center[0]-gap*(shown-1) as f32*0.5;
    let lit=if pages<=1 {0}else{(current*(shown-1)+(pages-1)/2)/(pages-1)};
    for i in 0..shown {
        let at=[start+gap*i as f32,center[1]];
        if i==lit {draw.add_circle(at,4.2*s,gold(1.0)).filled(true).build();}else{draw.add_circle(at,3.2*s,gold(0.45)).thickness(1.2*s).build();}
    }
}
/// 菱形等级格：前 filled 个实心金色，其余空心
pub fn pips(draw:&DrawListMut<'_>,start:[f32;2],count:u8,filled:u8,size:f32,s:f32) {
    for i in 0..count {
        let (x,y)=(start[0]+size+i as f32*size*2.7,start[1]);
        let points=vec![[x,y-size],[x+size,y],[x,y+size],[x-size,y]];
        if i<filled {draw.add_polyline(points,gold(1.0)).filled(true).build();}
        else {let mut closed=points.clone();closed.push(points[0]);draw.add_polyline(closed,ImColor32::from_rgba(150,138,112,200)).thickness(1.2*s).build();}
    }
}
/// 传承点的小徽记：金圈里一颗菱形
pub fn coin(draw:&DrawListMut<'_>,center:[f32;2],radius:f32,s:f32) {
    draw.add_circle(center,radius,ImColor32::from_rgba(40,32,20,255)).filled(true).build();
    draw.add_circle(center,radius,gold(0.95)).thickness(1.4*s).build();
    let r=radius*0.5;draw.add_polyline(vec![[center[0],center[1]-r],[center[0]+r,center[1]],[center[0],center[1]+r],[center[0]-r,center[1]]],gold(0.95)).filled(true).build();
}
