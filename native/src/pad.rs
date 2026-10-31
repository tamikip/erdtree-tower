// 按键提示：手柄与键盘共用一套标签。最后用的是手柄就画手柄键（A/B/X/Y 实心圆、LB/RB/View 圆角条、方向键十字），
// 用的是键盘或鼠标就画键帽（Enter、Esc、方向键……）。样式与结算页、苦难律法页的按键标签一致。
// 文字用黑体（scene-assets/fonts/TowerSans-Bold.otf），按目标像素换算缩放，画完把窗口字号还原，不影响调用方后面的文字。
use std::sync::atomic::{AtomicBool,Ordering};
use hudhook::imgui::{ImColor32,Key,StyleColor,Ui};

#[derive(Clone,Copy,PartialEq,Eq,Debug)]
pub enum Pad {A,B,X,Y,View,
    /// LB 与 RB 两个键（切换分页）
    Bumpers,
    /// LT 与 RT 两个键
    Triggers,
    /// 十字键：左右 / 上下 / 四向
    DpadX,DpadY,Dpad}
/// 一个操作在手柄与键盘上的按键；键盘写法里用“ / ”分开多个键，方向键写 ←→、↑↓ 或 ←↑↓→；键盘上没有对应键时写空串
#[derive(Clone,Copy,Debug)]
pub struct Input {pub pad:Pad,pub key:&'static str}
impl Input {pub const fn new(pad:Pad,key:&'static str)->Self {Self{pad,key}}}
pub const CONFIRM:Input=Input::new(Pad::A,"Enter");
pub const BACK:Input=Input::new(Pad::B,"Esc");
pub const MOVE_X:Input=Input::new(Pad::DpadX,"←→");
pub const MOVE_Y:Input=Input::new(Pad::DpadY,"↑↓");
pub const MOVE:Input=Input::new(Pad::Dpad,"←↑↓→");
pub const TABS:Input=Input::new(Pad::Bumpers,"Q / E");
/// 手柄 X 与键盘 Enter 都是“开始”的界面（种子输入、苦难律法）
pub const START:Input=Input::new(Pad::X,"Enter");

/// 最近一次输入来自手柄
static PAD:AtomicBool=AtomicBool::new(false);
/// 这一帧有手柄输入（路线图上摇杆会移动鼠标，这一帧的鼠标移动不算键鼠输入）
static PAD_FRAME:AtomicBool=AtomicBool::new(false);
/// 已经见过手柄连接：第一次连上时直接切到手柄提示
static CONNECTED:AtomicBool=AtomicBool::new(false);
pub fn using_pad()->bool {PAD.load(Ordering::Relaxed)}
/// 直接指定提示用哪种设备（Linux 预览工具录两种样式用）
#[allow(dead_code)]
pub fn set_pad(pad:bool) {PAD.store(pad,Ordering::Relaxed);}
/// 读手柄时调用（reward_ui::poll_pad）：found 为连着手柄，active 为有按键按下或摇杆推动
pub fn note_pad(found:bool,active:bool) {
    if found&&!CONNECTED.swap(true,Ordering::Relaxed) {PAD.store(true,Ordering::Relaxed);}
    if found&&active {PAD.store(true,Ordering::Relaxed);PAD_FRAME.store(true,Ordering::Relaxed);}
}
/// 每帧调用一次：键盘按键、鼠标点击或（不是摇杆带动的）鼠标移动都切回键盘提示
pub fn update(ui:&Ui) {
    if PAD_FRAME.swap(false,Ordering::Relaxed) {return;}
    let io=ui.io();
    let mouse=io.mouse_delta.iter().any(|d|d.abs()>0.5)||io.mouse_down.iter().any(|d|*d);
    let key=Key::VARIANTS.iter().any(|k|{let name=format!("{k:?}");!name.starts_with("Gamepad")&&!name.starts_with("Mouse")&&!name.starts_with("Mod")&&ui.is_key_pressed(*k)});
    if mouse||key {PAD.store(false,Ordering::Relaxed);}
}

/// 标签配色：Light 浅底深字（深色界面上的默认）；Dark 深底金字（放在金色 / 琥珀色按钮上）；Outline 只有描边
#[derive(Clone,Copy,PartialEq,Eq,Debug)]
pub enum Tone {Light,Dark,Outline}
const PAPER:[f32;4]=[0.93,0.91,0.86,1.0];const INK:[f32;4]=[0.07,0.07,0.06,1.0];const GOLD:[f32;4]=[0.91,0.71,0.29,1.0];
fn col(c:[f32;4],a:f32)->ImColor32 {ImColor32::from_rgba((c[0]*255.0) as u8,(c[1]*255.0) as u8,(c[2]*255.0) as u8,(c[3]*a.clamp(0.0,1.0)*255.0) as u8)}
fn colors(tone:Tone)->([f32;4],[f32;4]) {match tone {Tone::Light=>(PAPER,INK),Tone::Dark=>(INK,GOLD),Tone::Outline=>([0.0,0.0,0.0,0.0],PAPER)}}

/// 窗口字号还原：记下进来时的字号，离开时换算回去（子窗口的字号还会乘父窗口的缩放，按实际字号还原最稳）
struct Restore(f32);
impl Restore {fn new(ui:&Ui)->Self {Self(ui.current_font_size())}}
fn set_px(ui:&Ui,px:f32) {ui.set_window_font_scale(1.0);let base=ui.current_font_size().max(1.0);ui.set_window_font_scale(px/base);}
fn restore(ui:&Ui,r:Restore) {ui.set_window_font_scale(1.0);let base=ui.current_font_size().max(1.0);ui.set_window_font_scale(r.0/base);}
/// 在黑体（缺字时用当前字体）、px 像素下量 / 画一段文字
fn sans_width(ui:&Ui,text:&str,px:f32)->f32 {
    let _font=crate::fancy_text::button_font().filter(|_|crate::fancy_text::sans_covers(text)).map(|f|ui.push_font(f));set_px(ui,px);ui.calc_text_size(text)[0]
}
fn sans_text(ui:&Ui,pos:[f32;2],text:&str,px:f32,color:[f32;4]) {
    let _font=crate::fancy_text::button_font().filter(|_|crate::fancy_text::sans_covers(text)).map(|f|ui.push_font(f));set_px(ui,px);
    ui.set_cursor_screen_pos(pos);ui.text_colored(color,text);
}
fn pad_label(pad:Pad)->&'static str {match pad {Pad::A=>"A",Pad::B=>"B",Pad::X=>"X",Pad::Y=>"Y",Pad::View=>"View",_=>""}}
/// 一个标签由几块组成：圆形键、圆角条、十字键、键帽、方向键帽
enum Part {Circle(&'static str),Pill(&'static str),Cross(u8),Cap(String),Arrow(u8)}
fn parts(input:Input,pad:bool)->Vec<Part> {
    if pad {
        match input.pad {
            Pad::A|Pad::B|Pad::X|Pad::Y=>vec![Part::Circle(pad_label(input.pad))],
            Pad::Bumpers=>vec![Part::Pill("LB"),Part::Pill("RB")],
            Pad::Triggers=>vec![Part::Pill("LT"),Part::Pill("RT")],
            Pad::DpadX=>vec![Part::Cross(0b1100)],Pad::DpadY=>vec![Part::Cross(0b0011)],Pad::Dpad=>vec![Part::Cross(0b1111)],
            other=>vec![Part::Pill(pad_label(other))],
        }
    } else {
        // 键盘上没有对应按键（只能点击）的操作：键盘提示里不画标签
        if input.key.is_empty() {return Vec::new();}
        input.key.split(" / ").flat_map(|key|{
            let arrows:Vec<Part>=key.chars().filter_map(|c|match c {'↑'=>Some(Part::Arrow(0)),'↓'=>Some(Part::Arrow(1)),'←'=>Some(Part::Arrow(2)),'→'=>Some(Part::Arrow(3)),_=>None}).collect();
            if !arrows.is_empty()&&arrows.len()==key.chars().count() {arrows}else{vec![Part::Cap(key.to_string())]}
        }).collect()
    }
}
fn part_width(ui:&Ui,part:&Part,h:f32)->f32 {
    match part {
        Part::Circle(_)|Part::Cross(_)=>h,
        Part::Arrow(_)=>h*0.86,
        Part::Pill(text)=>(sans_width(ui,text,h*0.46)+h*0.62).max(h*1.3),
        Part::Cap(text)=>(sans_width(ui,text,h*0.5)+h*0.56).max(h*0.86),
    }
}
/// 标签宽度（屏幕像素），不画
pub fn width(ui:&Ui,input:Input,h:f32)->f32 {
    let r=Restore::new(ui);let parts=parts(input,using_pad());
    let w=parts.iter().map(|p|part_width(ui,p,h)).sum::<f32>()+h*0.14*(parts.len().saturating_sub(1)) as f32;restore(ui,r);w
}
/// 画一个按键标签：left 为左边、cy 为竖直中心、h 为高度（都是屏幕像素）；返回宽度
pub fn badge(ui:&Ui,left:f32,cy:f32,h:f32,input:Input,tone:Tone,alpha:f32)->f32 {
    let r=Restore::new(ui);
    let (fill,ink)=colors(tone);let edge=if tone==Tone::Outline {col(PAPER,0.7*alpha)}else{col(fill,alpha)};
    let mut x=left;
    for (i,part) in parts(input,using_pad()).iter().enumerate() {
        if i>0 {x+=h*0.14;}
        let w=part_width(ui,part,h);let (top,bottom)=(cy-h*0.5,cy+h*0.5);
        let draw=ui.get_window_draw_list();
        match part {
            Part::Circle(text)=>{
                draw.add_circle([x+h*0.5,cy],h*0.5,col(fill,alpha)).filled(true).num_segments(32).build();
                if tone==Tone::Outline {draw.add_circle([x+h*0.5,cy],h*0.5-0.75,edge).thickness(1.4).num_segments(32).build();}
                drop(draw);let tw=sans_width(ui,text,h*0.6);sans_text(ui,[x+(h-tw)*0.5,cy-h*0.6*0.62],text,h*0.6,[ink[0],ink[1],ink[2],ink[3]*alpha]);
            },
            Part::Pill(text)=>{
                draw.add_rect([x,top],[x+w,bottom],col(fill,alpha)).filled(true).rounding(h*0.5).build();
                if tone==Tone::Outline {draw.add_rect([x,top],[x+w,bottom],edge).rounding(h*0.5).thickness(1.4).build();}
                drop(draw);let tw=sans_width(ui,text,h*0.46);sans_text(ui,[x+(w-tw)*0.5,cy-h*0.46*0.62],text,h*0.46,[ink[0],ink[1],ink[2],ink[3]*alpha]);
            },
            Part::Cap(text)=>{
                draw.add_rect([x,top],[x+w,bottom],col(fill,alpha)).filled(true).rounding(h*0.18).build();
                // 键帽底边一道深色，像键盘按键的厚度
                if tone!=Tone::Outline {draw.add_rect([x+1.0,bottom-h*0.1],[x+w-1.0,bottom],ImColor32::from_rgba(0,0,0,(60.0*alpha) as u8)).filled(true).rounding(h*0.18).build();}
                else {draw.add_rect([x,top],[x+w,bottom],edge).rounding(h*0.18).thickness(1.4).build();}
                drop(draw);let tw=sans_width(ui,text,h*0.5);sans_text(ui,[x+(w-tw)*0.5,cy-h*0.5*0.66],text,h*0.5,[ink[0],ink[1],ink[2],ink[3]*alpha]);
            },
            Part::Arrow(dir)=>{
                draw.add_rect([x,top],[x+w,bottom],col(fill,alpha)).filled(true).rounding(h*0.18).build();
                if tone==Tone::Outline {draw.add_rect([x,top],[x+w,bottom],edge).rounding(h*0.18).thickness(1.4).build();}
                let (cx,a)=(x+w*0.5,h*0.2);let c=col(ink,alpha);
                let tri=match dir {0=>[[cx,cy-a],[cx+a,cy+a*0.6],[cx-a,cy+a*0.6]],1=>[[cx,cy+a],[cx-a,cy-a*0.6],[cx+a,cy-a*0.6]],2=>[[cx-a,cy],[cx+a*0.6,cy-a],[cx+a*0.6,cy+a]],_=>[[cx+a,cy],[cx-a*0.6,cy+a],[cx-a*0.6,cy-a]]};
                draw.add_triangle(tri[0],tri[1],tri[2],c).filled(true).build();
            },
            Part::Cross(arms)=>{
                // 十字键：浅色十字，要按的方向画出深色小三角（上、下、左、右 = 第 0–3 位）
                let (cx,t)=(x+h*0.5,h*0.17);let c=col(fill,alpha);
                draw.add_rect([cx-t,top],[cx+t,bottom],c).filled(true).rounding(h*0.08).build();
                draw.add_rect([x,cy-t],[x+h,cy+t],c).filled(true).rounding(h*0.08).build();
                if tone==Tone::Outline {
                    draw.add_rect([cx-t,top],[cx+t,bottom],edge).rounding(h*0.08).thickness(1.2).build();draw.add_rect([x,cy-t],[x+h,cy+t],edge).rounding(h*0.08).thickness(1.2).build();
                }
                let (k,a)=(h*0.33,h*0.09);let ink=col(ink,alpha);
                for bit in 0..4 {
                    if arms&(1<<bit)==0 {continue;}
                    let tri=match bit {0=>[[cx,cy-k-a],[cx+a,cy-k+a*0.6],[cx-a,cy-k+a*0.6]],1=>[[cx,cy+k+a],[cx-a,cy+k-a*0.6],[cx+a,cy+k-a*0.6]],2=>[[cx-k-a,cy],[cx-k+a*0.6,cy-a],[cx-k+a*0.6,cy+a]],_=>[[cx+k+a,cy],[cx+k-a*0.6,cy+a],[cx+k-a*0.6,cy-a]]};
                    draw.add_triangle(tri[0],tri[1],tri[2],ink).filled(true).build();
                }
            },
        }
        x+=w;
    }
    restore(ui,r);x-left
}
/// 一行操作提示：[键] 说明  [键] 说明……。anchor 为屏幕坐标（x 按 align 对齐：0 左、0.5 中、1 右；y 为这一行的竖直中心）；
/// h 为标签高度、px 为说明文字字号（屏幕像素）。没有按键的项（如“鼠标悬停查看”）只写文字
pub fn hints(ui:&Ui,anchor:[f32;2],align:f32,items:&[(Option<Input>,&str)],h:f32,px:f32,color:[f32;4],tone:Tone) {
    let r=Restore::new(ui);
    let (gap,spacing)=(h*0.32,h*1.0);
    let sizes:Vec<(f32,f32)>=items.iter().map(|(input,label)|(input.map_or(0.0,|i|width(ui,i,h)),sans_width(ui,label,px))).collect();
    let total=sizes.iter().map(|(b,l)|b+if *b>0.0 {gap}else{0.0}+l).sum::<f32>()+spacing*(items.len().saturating_sub(1)) as f32;
    let mut x=anchor[0]-total*align;
    for ((input,label),(bw,lw)) in items.iter().zip(sizes) {
        if let Some(input)=input.filter(|_|bw>0.0) {badge(ui,x,anchor[1],h,input,tone,color[3]);x+=bw+gap;}
        sans_text(ui,[x,anchor[1]-px*0.66],label,px,color);x+=lw+spacing;
    }
    restore(ui,r);
}
/// 按钮里的文字：标签 + 文字在 min..max 里居中。文字用调用方当前的字体，窗口字号设为 scale（与原来画按钮文字的写法一致）
pub fn label_in(ui:&Ui,min:[f32;2],max:[f32;2],input:Option<Input>,label:&str,color:[f32;4],scale:f32,tone:Tone) {
    ui.set_window_font_scale(scale);
    let size=ui.calc_text_size(label);let h=(ui.text_line_height()*1.08).min((max[1]-min[1])*0.62);
    let bw=input.map(|i|width(ui,i,h)).filter(|w|*w>0.0).map_or(0.0,|w|w+h*0.4);
    let x=(min[0]+max[0]-size[0]-bw)*0.5;let cy=(min[1]+max[1])*0.5;
    // 文字会自动乘上样式的整体透明度（动画淡入淡出时 push 的 StyleVar::Alpha），标签是画在绘制列表上的，要手动乘
    if let Some(input)=input.filter(|_|bw>0.0) {badge(ui,x,cy,h,input,tone,color[3]*ui.clone_style().alpha);}
    ui.set_window_font_scale(scale);ui.set_cursor_screen_pos([x+bw,cy-size[1]*0.5]);ui.text_colored(color,label);
}
/// 代替 ImGui 默认按钮：沿用当前样式的按钮底色、圆角与边框，文字左边加按键标签。size 为屏幕像素（0 表示按内容）。
/// 先画外观、最后放按钮本体，按钮是最后一个控件，后面的 same_line 等排版照常
pub fn button(ui:&Ui,id:&str,label:&str,input:Option<Input>,size:[f32;2])->bool {
    let style=ui.clone_style();let pad=style.frame_padding;
    let text=ui.calc_text_size(label);let h=ui.text_line_height()*1.05;
    let bw=input.map(|i|width(ui,i,h)).filter(|w|*w>0.0).map_or(0.0,|w|w+h*0.35);
    let size=[if size[0]>0.0{size[0]}else{text[0]+bw+pad[0]*2.0},if size[1]>0.0{size[1]}else{text[1]+pad[1]*2.0}];
    let min=ui.cursor_screen_pos();let max=[min[0]+size[0],min[1]+size[1]];
    let hovered=ui.is_mouse_hovering_rect(min,max)&&ui.is_window_hovered();
    let active=hovered&&ui.is_mouse_down(hudhook::imgui::MouseButton::Left);
    let color=ui.style_color(if active {StyleColor::ButtonActive}else if hovered {StyleColor::ButtonHovered}else{StyleColor::Button});
    let draw=ui.get_window_draw_list();
    draw.add_rect(min,max,color).filled(true).rounding(style.frame_rounding).build();
    if style.frame_border_size>0.0 {draw.add_rect(min,max,ui.style_color(StyleColor::Border)).rounding(style.frame_rounding).thickness(style.frame_border_size).build();}
    drop(draw);
    let ink=ui.style_color(StyleColor::Text);
    let x=(min[0]+max[0]-text[0]-bw)*0.5;let cy=(min[1]+max[1])*0.5;
    if let Some(input)=input.filter(|_|bw>0.0) {badge(ui,x,cy,h,input,Tone::Light,ink[3]);}
    ui.set_cursor_screen_pos([x+bw,cy-text[1]*0.5]);ui.text_colored(ink,label);
    ui.set_cursor_screen_pos(min);
    ui.invisible_button(id,size)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_parts_split_keys_and_arrows() {
        assert!(matches!(parts(CONFIRM,true).as_slice(),[Part::Circle("A")]));
        assert!(matches!(parts(TABS,true).as_slice(),[Part::Pill("LB"),Part::Pill("RB")]));
        assert!(matches!(parts(TABS,false).as_slice(),[Part::Cap(q),Part::Cap(e)] if q=="Q"&&e=="E"));
        assert!(matches!(parts(MOVE_X,false).as_slice(),[Part::Arrow(2),Part::Arrow(3)]));
        assert!(matches!(parts(MOVE,true).as_slice(),[Part::Cross(0b1111)]));
        assert!(matches!(parts(Input::new(Pad::View,"J"),false).as_slice(),[Part::Cap(j)] if j=="J"));
        assert!(parts(Input::new(Pad::X,""),false).is_empty());
    }
}
