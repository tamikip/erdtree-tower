// 苦难律法选择界面（参考危机合约的合约选择，扁平、高对比）：左边按“敌人 / 我方 / 经济 / 规则”分组的律法链，
// 右边已选律法列表，底部律法码、当前分数与评级进度、开始按钮。选中的律法铺成琥珀色，前置没满足的变暗。
// 文字用黑体（大标题按 48 像素加载、正文 26 像素，只缩小不放大；缺字时退回宋体），大号数字用结算页的显示字体。
use hudhook::imgui::{Condition,DrawListMut,ImColor32,Key,Ui,WindowFlags};
use erdtree_tower::law::{BRANCHES,GRADES,Law,NODES};

pub enum LawAction {Start(Law),Back,Archive}
/// shown：分数显示值（追着真实分数滚动）；flash：刚选中的节点与时间（卡片闪一下、列表里那一行滑入）
pub struct LawScreen {pub law:Law,pub seed:Option<u64>,focus:usize,opened:Option<f64>,copied:Option<f64>,shown:Option<f32>,flash:Option<(usize,f64)>}
impl LawScreen {pub fn new(law:Law)->Self {Self{law,seed:None,focus:0,opened:None,copied:None,shown:None,flash:None}}}

const AMBER:[f32;4]=[0.91,0.71,0.29,1.0];const WHITE:[f32;4]=[0.93,0.92,0.88,1.0];const MUTED:[f32;4]=[0.58,0.57,0.53,1.0];
const DIM:[f32;4]=[0.36,0.35,0.33,1.0];const INK:[f32;4]=[0.07,0.07,0.06,1.0];
const LABELS:[&str;4]=["ENEMY","SELF","ECONOMY","RULE"];
const ACCENT:[[f32;4];4]=[[0.86,0.25,0.23,1.0],[0.91,0.71,0.29,1.0],[0.34,0.69,0.38,1.0],[0.5,0.41,0.86,1.0]];
/// 律法卡片的大小与第一列的横坐标（1600×900 画布）
const TILE:[f32;2]=[138.0,50.0];const LEFT:f32=150.0;
/// 评级进度条按这个分数画满
const TRACK_MAX:f32=350.0;
fn col(c:[f32;4],a:f32)->ImColor32 {ImColor32::from_rgba((c[0]*255.0) as u8,(c[1]*255.0) as u8,(c[2]*255.0) as u8,(c[3]*a.clamp(0.0,1.0)*255.0) as u8)}
fn fade(c:[f32;4],k:f32)->[f32;4] {[c[0],c[1],c[2],c[3]*k.clamp(0.0,1.0)]}
fn ease(elapsed:f64,start:f64,len:f64)->f32 {let t=((elapsed-start)/len).clamp(0.0,1.0) as f32;1.0-(1.0-t).powi(3)}
/// 节点所在的显示行与卡片左上角：每条链一行；坚不可摧、元素庇护、异常耐性三条短链并排一行，疲惫、断念并排一行
fn place(id:usize)->(usize,[f32;2]) {
    let n=&NODES[id];let c=f32::from(n.col.saturating_sub(1));
    let (row,pair)=match n.row {0=>(0,None),1=>(1,None),2=>(2,Some(0.0)),9=>(2,Some(1.0)),10=>(2,Some(2.0)),3=>(3,None),4=>(4,None),5=>(5,Some(0.0)),6=>(5,Some(1.0)),7=>(6,None),_=>(7,None)};
    let x=match pair {Some(p)=>LEFT+p*300.0+c*144.0,None=>LEFT+c*158.0};
    (row,[x,124.0+row as f32*66.0+n.branch as f32*28.0])
}
/// 手柄方向键移动焦点：左右只在同一行里走，上下走到最近的一行里横向最近的卡片
fn moved(focus:usize,dx:i32,dy:i32)->usize {
    let center=|i:usize|{let (row,[x,_])=place(i);(row as i32,x+TILE[0]*0.5)};
    let (row,x)=center(focus);
    let key=|i:usize|{let (r,ix)=center(i);if dx!=0 {(ix-x).abs()}else{(r-row).abs() as f32*10_000.0+(ix-x).abs()}};
    (0..NODES.len()).filter(|&i|{let (r,ix)=center(i);if dx!=0 {r==row&&(ix-x)*dx as f32>0.0}else{(r-row)*dy>0}})
        .min_by(|&a,&b|key(a).total_cmp(&key(b))).unwrap_or(focus)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tiles_fit_the_board_and_follow_visual_navigation() {
        for id in 0..NODES.len() {
            let (_,[x,y])=place(id);
            assert!(x>=LEFT&&x+TILE[0]<=1046.0&&y>=116.0&&y+TILE[1]<=728.0,"{id}");
            for other in 0..id {
                let (_,[ox,oy])=place(other);
                assert!(x+TILE[0]<=ox||ox+TILE[0]<=x||y+TILE[1]<=oy||oy+TILE[1]<=y,"{id} {other}");
            }
        }
        assert_eq!(moved(6,1,0),7);
        assert_eq!(moved(7,1,0),27);
        assert_eq!(moved(0,0,1),3);
        assert_eq!(moved(27,0,-1),5);
        assert_eq!(moved(6,0,1),8);
        assert_eq!(moved(8,0,-1),6);
        assert_eq!(moved(14,1,0),14);
    }
}
/// 带金色边框与四角装饰的暗色面板（选择职业、档案等界面也用）
pub(crate) fn panel(draw:&DrawListMut<'_>,min:[f32;2],max:[f32;2],s:f32) {
    draw.add_rect_filled_multicolor(min,max,ImColor32::from_rgba(16,13,9,228),ImColor32::from_rgba(16,13,9,228),ImColor32::from_rgba(8,7,5,236),ImColor32::from_rgba(8,7,5,236));
    draw.add_rect(min,max,ImColor32::from_rgba(171,143,87,120)).thickness(1.0*s).build();
    let gold=ImColor32::from_rgba(214,180,112,230);let l=18.0*s;
    for (x,y,dx,dy) in [(min[0],min[1],1.0,1.0),(max[0],min[1],-1.0,1.0),(min[0],max[1],1.0,-1.0),(max[0],max[1],-1.0,-1.0)] {
        draw.add_line([x,y],[x+l*dx,y],gold).thickness(2.0*s).build();draw.add_line([x,y],[x,y+l*dy],gold).thickness(2.0*s).build();
    }
}
/// 两端渐隐的金色装饰线，中间一颗菱形
pub(crate) fn ornament(draw:&DrawListMut<'_>,center:[f32;2],half:f32,s:f32) {
    let (x,y)=(center[0],center[1]);
    draw.add_rect_filled_multicolor([x-half,y-0.7*s],[x-8.0*s,y+0.7*s],ImColor32::from_rgba(214,180,112,0),ImColor32::from_rgba(214,180,112,220),ImColor32::from_rgba(214,180,112,220),ImColor32::from_rgba(214,180,112,0));
    draw.add_rect_filled_multicolor([x+8.0*s,y-0.7*s],[x+half,y+0.7*s],ImColor32::from_rgba(214,180,112,220),ImColor32::from_rgba(214,180,112,0),ImColor32::from_rgba(214,180,112,0),ImColor32::from_rgba(214,180,112,220));
    draw.add_polyline(vec![[x,y-5.0*s],[x+5.0*s,y],[x,y+5.0*s],[x-5.0*s,y]],ImColor32::from_rgba(214,180,112,240)).filled(true).build();
}
/// 入场：标题淡入 → 各行律法卡从左往右依次滑入 → 已选列表逐行滑入 → 底部栏淡入。
/// 选中一张卡时卡片闪一下、列表里那一行从右侧滑入；分数滚动到新值。手柄：方向键移动、A 选择、X 开始、Y 清空、B 返回、View 档案
pub fn draw(ui:&Ui,screen:&mut LawScreen,record:Option<(u32,&str)>)->Option<LawAction> {
    let size=ui.io().display_size;let s=(size[0]/1600.0).min(size[1]/900.0);
    let (ox,oy)=((size[0]-1600.0*s)*0.5,(size[1]-900.0*s)*0.5);let p=|x:f32,y:f32|[ox+x*s,oy+y*s];
    let now=ui.time();let opened=*screen.opened.get_or_insert(now);let t=now-opened;let time=now as f32;
    let dt=ui.io().delta_time.clamp(0.0,0.1);
    // ── 背景：压暗的原画与缓慢漂移的细斜纹（和结算页一致）──
    let background=ui.get_background_draw_list();crate::oath_art::background(&background,size);
    background.add_rect([0.0,0.0],size,ImColor32::from_rgba(13,13,12,(176.0+56.0*ease(t,0.0,0.3)) as u8)).filled(true).build();
    let drift=(time*9.0*s)%(28.0*s);
    let mut x=-size[1]+drift;while x<size[0] {background.add_line([x,size[1]],[x+size[1],0.0],ImColor32::from_rgba(255,255,255,5)).thickness(1.0).build();x+=28.0*s;}
    drop(background);
    let ready=t>=0.35;
    let key=|k:Key|ready&&ui.is_key_pressed(k);
    let toggle=|screen:&mut LawScreen,id:usize| {
        screen.law=screen.law.toggle(id);screen.focus=id;
        if screen.law.has(id) {screen.flash=Some((id,now));crate::sfx::play("confirm");}else{crate::sfx::play("close");}
    };
    let before=screen.focus;
    if key(Key::GamepadDpadLeft){screen.focus=moved(screen.focus,-1,0);}
    if key(Key::GamepadDpadRight){screen.focus=moved(screen.focus,1,0);}
    if key(Key::GamepadDpadUp){screen.focus=moved(screen.focus,0,-1);}
    if key(Key::GamepadDpadDown){screen.focus=moved(screen.focus,0,1);}
    if screen.focus!=before {crate::sfx::play("hover");}
    if key(Key::GamepadFaceDown){toggle(screen,screen.focus);}
    if key(Key::GamepadFaceUp)&&!screen.law.is_empty(){screen.law=Law::default();crate::sfx::play("close");}
    if ui.io().key_ctrl&&key(Key::V) {if let Some(law)=ui.clipboard_text().and_then(|text|Law::parse(&text)){screen.law=law;crate::sfx::play("confirm");}}
    let mut action=if key(Key::Enter)||key(Key::GamepadFaceLeft){Some(LawAction::Start(screen.law))}else if key(Key::Escape)||key(Key::GamepadFaceRight){Some(LawAction::Back)}
        else if key(crate::options::key(erdtree_tower::options::Hotkey::Archive))||key(Key::GamepadBack){Some(LawAction::Archive)}else{None};
    let score=screen.law.score();
    let shown=screen.shown.map_or(score as f32,|v|v+(score as f32-v)*(1.0-(-dt*12.0).exp()));
    screen.shown=Some(if (shown-score as f32).abs()<0.5 {score as f32}else{shown});
    let shown=screen.shown.unwrap_or(0.0).round() as u32;
    let big=crate::run_screen::Big::new(ui);
    let mut hovered=None;let mut copy=false;let mut clear=false;
    ui.window("苦难律法##law").position([0.0,0.0],Condition::Always).size(size,Condition::Always)
        .flags(WindowFlags::NO_DECORATION|WindowFlags::NO_BACKGROUND|WindowFlags::NO_MOVE|WindowFlags::NO_NAV|WindowFlags::NO_SCROLL_WITH_MOUSE).build(|| {
        // px 为设计稿里的字号：27 以上且字都在大标题字表里时用 48 像素的大标题黑体；否则用 26 像素的黑体；黑体缺字时退回宋体
        let font_for=|value:&str,px:f32|->(Option<hudhook::imgui::FontStackToken<'_>>,f32) {
            use crate::fancy_text as ft;
            if px>=27.0 {if let Some(font)=ft::heading_font().filter(|_|ft::heading_covers(value)) {return (Some(ui.push_font(font)),ft::HEADING_PIXELS);}}
            if let Some(font)=ft::button_font().filter(|_|ft::sans_covers(value)) {return (Some(ui.push_font(font)),ft::SANS_PIXELS);}
            if px>=24.0 {if let Some(font)=ft::title_font() {return (Some(ui.push_font(font)),32.0);}}
            (None,20.0)
        };
        let text=|x:f32,y:f32,value:&str,color:[f32;4],px:f32,align:f32| {
            if color[3]<=0.01 {return;}
            let (_font,base)=font_for(value,px);ui.set_window_font_scale(px/base*s);
            ui.set_cursor_screen_pos([ox+x*s-ui.calc_text_size(value)[0]*align,oy+y*s]);ui.text_colored(color,value);
        };
        let measure=|value:&str,px:f32|->f32 {let (_font,base)=font_for(value,px);ui.set_window_font_scale(px/base*s);ui.calc_text_size(value)[0]/s};
        // 放不下时缩小字号（最多缩到 75%）
        let fitted=|value:&str,px:f32,width:f32|->f32 {let w=measure(value,px);if w>width {(px*width/w).max(px*0.75)}else{px}};
        // 字母间距拉开的英文小标签
        let micro=|x:f32,y:f32,value:&str,color:[f32;4],px:f32,align:f32| {
            let gap=px*0.28;let width=value.chars().map(|c|measure(&c.to_string(),px)+gap).sum::<f32>()-gap;
            let mut cx=x-width*align;for c in value.chars() {let piece=c.to_string();text(cx,y,&piece,color,px,0.0);cx+=measure(&piece,px)+gap;}
        };
        // 按键标签（crate::pad：手柄 / 键盘随最后用的设备切换）；返回宽度（设计单位）
        let badge=|x:f32,cy:f32,input:crate::pad::Input,tone:crate::pad::Tone,a:f32|->f32 {crate::pad::badge(ui,ox+x*s,oy+cy*s,28.0*s,input,tone,a)/s};
        let hit=|id:&str,min:[f32;2],max:[f32;2]|->(bool,bool) {
            ui.set_cursor_screen_pos(p(min[0],min[1]));ui.set_window_font_scale(s);
            let clicked=ui.invisible_button(id,[(max[0]-min[0])*s,(max[1]-min[1])*s]);(clicked&&ready,ui.is_item_hovered())
        };
        // ── 顶栏：返回、旅程档案；标题；历史最高 ──
        let head=ease(t,0.0,0.4);
        let (back,hover)=hit("##law-back",[36.0,26.0],[146.0,72.0]);if back {action=Some(LawAction::Back);}
        let w=badge(42.0,49.0,crate::pad::BACK,crate::pad::Tone::Light,head);text(54.0+w,37.0,"返回",fade(if hover{AMBER}else{WHITE},head),20.0,0.0);
        {let draw=ui.get_window_draw_list();draw.add_line(p(160.0,34.0),p(160.0,64.0),col(WHITE,0.18*head)).thickness(s).build();}
        let archive_key=crate::pad::Input::new(crate::pad::Pad::View,crate::options::label(erdtree_tower::options::Hotkey::Archive));
        let (archive,hover)=hit("##law-archive",[174.0,26.0],[380.0,72.0]);if archive {action=Some(LawAction::Archive);}
        let w=badge(178.0,49.0,archive_key,crate::pad::Tone::Light,head);
        text(190.0+w,37.0,"旅程档案",fade(if hover{AMBER}else{WHITE},head),20.0,0.0);
        micro(800.0,18.0,"CONTINGENCY",fade(MUTED,head),12.0,0.5);
        text(800.0,34.0+(1.0-head)*8.0,"苦难律法",fade(WHITE,head),44.0,0.5);
        {let draw=ui.get_window_draw_list();let half=22.0*ease(t,0.15,0.35);draw.add_rect(p(800.0-half,94.0),p(800.0+half,99.0),col(AMBER,head)).filled(true).build();}
        if let Some(seed)=screen.seed {text(800.0,104.0,&format!("种子 {seed}"),fade(MUTED,head),14.0,0.5);}
        match record {
            Some((best,class))=>{
                let h=40.0;let digits=best.to_string();let w=big.width(digits.len(),h*s)/s;
                big.draw_text(ui,&digits,ox+(1556.0-w)*s,oy+30.0*s,h*s,fade(WHITE,head));
                text(1540.0-w,40.0,&format!("{class} · 历史最高"),fade(MUTED,head),19.0,1.0);
            },
            None=>text(1556.0,40.0,"尚无纪录",fade(MUTED,head),19.0,1.0),
        }
        // ── 左：分组与律法链 ──
        let rows_of=|b:usize|{let rows:Vec<usize>=(0..NODES.len()).filter(|&i|NODES[i].branch==b).map(|i|place(i).0).collect();(rows.iter().copied().min().unwrap_or(0),rows.iter().copied().max().unwrap_or(0))};
        let row_y=|row:usize,b:usize|124.0+row as f32*66.0+b as f32*28.0;
        for (b,name) in BRANCHES.iter().enumerate() {
            let (first,last)=rows_of(b);let (top,bottom)=(row_y(first,b),row_y(last,b)+TILE[1]);
            let a=ease(t,0.05+first as f64*0.045,0.35);
            let draw=ui.get_window_draw_list();
            draw.add_rect(p(44.0,top),p(49.0,top+(bottom-top)*a),col(ACCENT[b],a)).filled(true).build();
            if b>0 {let y=top-17.0;draw.add_line(p(44.0,y),p(1046.0,y),col(WHITE,0.09*a)).thickness(s).build();}
            drop(draw);
            text(64.0,top-4.0,name,fade(WHITE,a),30.0,0.0);
            micro(65.0,top+36.0,LABELS[b],fade(MUTED,a),11.0,0.0);
            if last>first {
                let count=(0..NODES.len()).filter(|&i|NODES[i].branch==b).count();let on=(0..NODES.len()).filter(|&i|NODES[i].branch==b&&screen.law.has(i)).count();
                text(65.0,top+58.0,&format!("{on} / {count}"),fade(if on>0{AMBER}else{DIM},a),16.0,0.0);
            }
        }
        // 链之间的连线：两端都选中时亮成琥珀色
        {
            let draw=ui.get_window_draw_list();
            for (id,node) in NODES.iter().enumerate() {
                let Some(parent)=node.parent else{continue};
                let ((row,[x,y]),(_,[px,_]))=(place(id),place(parent));
                let a=ease(t,0.08+row as f64*0.045+f64::from(node.col)*0.03,0.35);
                let cy=y+TILE[1]*0.5;let on=screen.law.has(id);
                draw.add_line(p(px+TILE[0],cy),p(x,cy),if on {col(AMBER,a)}else{col(WHITE,0.22*a)}).thickness(if on{2.0}else{1.2}*s).build();
            }
        }
        for (id,node) in NODES.iter().enumerate() {
            let (row,[x0,y])=place(id);
            let a=ease(t,0.08+row as f64*0.045+f64::from(node.col)*0.03,0.35);let x=x0-(1.0-a)*14.0;
            let (clicked,hover)=hit(&format!("##law-{id}"),[x0,y],[x0+TILE[0],y+TILE[1]]);
            if clicked {toggle(screen,id);}
            if hover {hovered=Some(id);}
            if a<=0.0 {continue;}
            let (chosen,open)=(screen.law.has(id),screen.law.available(id));let focus=screen.focus==id||hover;
            let (min,max)=(p(x,y),p(x+TILE[0],y+TILE[1]));
            let draw=ui.get_window_draw_list();
            if chosen {
                draw.add_rect(min,max,col(if hover{[1.0,0.8,0.4,1.0]}else{AMBER},a)).filled(true).build();
            } else {
                draw.add_rect(min,max,ImColor32::from_rgba(22,22,23,(if open{235.0}else{190.0}*a) as u8)).filled(true).build();
                if hover {draw.add_rect(min,max,col(WHITE,0.06*a)).filled(true).build();}
                draw.add_rect(min,max,col(WHITE,if open{0.34}else{0.13}*a)).thickness(s).build();
            }
            if focus {
                let d=if chosen {3.0*s}else{0.0};
                draw.add_rect([min[0]-d,min[1]-d],[max[0]+d,max[1]+d],col(if chosen{WHITE}else{AMBER},a)).thickness(2.0*s).build();
            }
            // 图标格：选中时深色线框配深色图标，未选时黑底白图标
            let (imin,imax)=(p(x+6.0,y+6.0),p(x+44.0,y+44.0));
            if chosen {draw.add_rect(imin,imax,col(INK,0.85*a)).thickness(1.4*s).build();}
            else {draw.add_rect(imin,imax,ImColor32::from_rgba(6,6,6,(220.0*a) as u8)).filled(true).build();draw.add_rect(imin,imax,col(WHITE,0.16*a)).thickness(s).build();}
            let light=if chosen {24}else if open {(235.0*a) as u8}else{(80.0*a) as u8};
            crate::law_art::draw_line(&draw,node.art,p(x+9.0,y+9.0),p(x+41.0,y+41.0),light);
            // 刚选中：白光从卡面褪去
            if let Some((_,at))=screen.flash.filter(|f|f.0==id) {let k=1.0-ease(now,at,0.4);if k>0.0 {draw.add_rect(min,max,col(WHITE,0.55*k)).filled(true).build();}}
            if chosen {
                let pop=screen.flash.filter(|f|f.0==id).map_or(1.0,|(_,at)|ease(now,at,0.25));
                let (cx,cy)=(x+TILE[0]-14.0,y+TILE[1]-13.0);let k=pop*a;
                draw.add_polyline(vec![p(cx-5.0*k,cy),p(cx-1.5*k,cy+3.5*k),p(cx+5.5*k,cy-4.5*k)],col(INK,a)).thickness(2.2*s).build();
            }
            drop(draw);
            let ink=if chosen {INK}else if open {WHITE}else{DIM};
            let px=fitted(node.name,17.0,TILE[0]-56.0);
            text(x+52.0,y+5.0+(17.0-px)*0.5,node.name,fade(ink,a),px,0.0);
            text(x+52.0,y+27.0,&node.points.to_string(),fade(if chosen{INK}else if open{MUTED}else{DIM},a),16.0,0.0);
        }
        // ── 右：已选律法 ──
        {let draw=ui.get_window_draw_list();let a=ease(t,0.15,0.4);draw.add_line(p(1064.0,124.0),p(1064.0,124.0+604.0*a),col(WHITE,0.14)).thickness(s).build();}
        let side=ease(t,0.15,0.4);
        let selected=screen.law.selected();
        text(1086.0,116.0,"已选律法",fade(WHITE,side),28.0,0.0);
        micro(1098.0+measure("已选律法",28.0),130.0,"SELECTED",fade(MUTED,side),11.0,0.0);
        text(1556.0,116.0,&format!("{} 项",selected.len()),fade(WHITE,side),28.0,1.0);
        {let draw=ui.get_window_draw_list();draw.add_line(p(1086.0,162.0),p(1556.0,162.0),col(WHITE,0.14*side)).thickness(s).build();}
        // 子窗口的字号会再乘上父窗口当前的缩放：先把父窗口还原成 1
        ui.set_window_font_scale(1.0);
        ui.set_cursor_screen_pos(p(1086.0,166.0));
        // 细的滚动条：透明底、半透明白色滑块
        let _scroll=(ui.push_style_var(hudhook::imgui::StyleVar::ScrollbarSize(5.0*s)),ui.push_style_var(hudhook::imgui::StyleVar::ScrollbarRounding(0.0)));
        let _colors=[ui.push_style_color(hudhook::imgui::StyleColor::ScrollbarBg,[0.0,0.0,0.0,0.0]),ui.push_style_color(hudhook::imgui::StyleColor::ScrollbarGrab,[1.0,1.0,1.0,0.18]),
            ui.push_style_color(hudhook::imgui::StyleColor::ScrollbarGrabHovered,[1.0,1.0,1.0,0.3]),ui.push_style_color(hudhook::imgui::StyleColor::ScrollbarGrabActive,AMBER),ui.push_style_color(hudhook::imgui::StyleColor::ChildBg,[0.0,0.0,0.0,0.0])];
        ui.child_window("##law-list").size([474.0*s,566.0*s]).build(|| {
            let origin=ui.cursor_screen_pos();let (lx,ly)=((origin[0]-ox)/s,(origin[1]-oy)/s);
            if selected.is_empty() {
                text(lx+237.0,ly+230.0,"尚未选择律法",fade(MUTED,side),20.0,0.5);
                text(lx+237.0,ly+262.0,"点击左侧律法，接受对应的限制",fade(DIM,side),15.0,0.5);
            }
            let mut y=ly;
            for (row,&id) in selected.iter().enumerate() {
                let node=&NODES[id];
                let a=ease(t,0.25+row as f64*0.035,0.35)*screen.flash.filter(|f|f.0==id).map_or(1.0,|(_,at)|ease(now,at,0.3));
                let x=lx+(1.0-a)*24.0;
                // 效果一行放不下时折成两行
                let (_font,base)=font_for(node.effect,15.0);ui.set_window_font_scale(15.0/base*s);
                let lines=crate::fancy_text::wrap_lines(node.effect,400.0*s,|v|ui.calc_text_size(v)[0]);drop(_font);
                let h=40.0+lines.len() as f32*21.0;
                let draw=ui.get_window_draw_list();
                draw.add_rect(p(x,y+10.0),p(x+44.0,y+54.0),col(AMBER,a)).filled(true).build();
                if let Some((_,at))=screen.flash.filter(|f|f.0==id) {let k=1.0-ease(now,at,0.6);if k>0.0 {draw.add_rect(p(x,y),p(x+470.0,y+h),col(AMBER,0.12*k)).filled(true).build();}}
                draw.add_line(p(lx,y+h),p(lx+470.0,y+h),col(WHITE,0.1*a)).thickness(s).build();
                drop(draw);
                text(x+22.0,y+20.0,&node.points.to_string(),fade(INK,a),20.0,0.5);
                text(x+60.0,y+6.0,node.name,fade(WHITE,a),19.0,0.0);
                for (i,line) in lines.iter().enumerate() {text(x+60.0,y+34.0+i as f32*21.0,line,fade(MUTED,a),15.0,0.0);}
                y+=h;
            }
            ui.set_cursor_screen_pos(p(lx,y));ui.dummy([1.0,1.0]);
        });
        drop(_colors);drop(_scroll);
        // ── 悬停说明 ──
        if let Some(id)=hovered {
            let node=&NODES[id];let (_,[x,y])=place(id);
            let hint=if screen.law.has(id){"已选择 · 再次点击取消（其后的律法一并取消）".to_string()}else if screen.law.available(id){format!("点击接受 · +{} 分",node.points)}else{
                let missing=screen.law.missing_prerequisites(id);let points:u32=missing.iter().map(|&i|NODES[i].points).sum::<u32>()+node.points;
                format!("点击连同前置「{}」一起接受 · +{points} 分",missing.iter().rev().map(|&i|NODES[i].name).collect::<Vec<_>>().join("、"))
            };
            let w=[measure(node.name,19.0),measure(node.effect,16.0),measure(&hint,15.0)].into_iter().fold(0.0,f32::max)+36.0;
            let (tx,ty)=((x+10.0).min(1046.0-w),if y+TILE[1]+96.0>740.0 {y-92.0}else{y+TILE[1]+8.0});
            // 画在律法卡之后，盖在卡片上面（说明框只在左侧区域内，碰不到右侧列表与底栏）
            let draw=ui.get_window_draw_list();
            draw.add_rect(p(tx,ty),p(tx+w,ty+84.0),ImColor32::from_rgba(24,24,25,250)).filled(true).build();
            draw.add_rect(p(tx,ty),p(tx+w,ty+84.0),col(WHITE,0.2)).thickness(s).build();
            draw.add_rect(p(tx,ty),p(tx+4.0,ty+84.0),col(ACCENT[node.branch],1.0)).filled(true).build();
            drop(draw);
            text(tx+18.0,ty+8.0,node.name,WHITE,19.0,0.0);
            text(tx+18.0,ty+34.0,node.effect,MUTED,16.0,0.0);
            text(tx+18.0,ty+58.0,&hint,AMBER,15.0,0.0);
        }
        // ── 底栏：律法码、复制、清空；分数；评级进度；开始旅程 ──
        let foot=ease(t,0.35,0.4);
        {let draw=ui.get_window_draw_list();draw.add_line([0.0,oy+764.0*s],[size[0],oy+764.0*s],col(WHITE,0.12*foot)).thickness(s).build();}
        text(40.0,804.0,"律法码",fade(MUTED,foot),18.0,0.0);
        let code=screen.law.code();text(102.0,794.0,&code,fade(WHITE,foot),30.0,0.0);
        let cx=116.0+measure(&code,30.0);
        {let draw=ui.get_window_draw_list();draw.add_line(p(cx,798.0),p(cx,832.0),col(WHITE,0.16*foot)).thickness(s).build();}
        let (clicked,hover)=hit("##law-copy",[cx+12.0,794.0],[cx+110.0,836.0]);copy=clicked;
        {
            let draw=ui.get_window_draw_list();let c=col(if hover{AMBER}else{WHITE},foot);
            draw.add_rect(p(cx+22.0,807.0),p(cx+34.0,821.0),c).thickness(1.4*s).build();draw.add_rect(p(cx+26.0,803.0),p(cx+38.0,817.0),c).thickness(1.4*s).build();
        }
        text(cx+48.0,802.0,if screen.copied.is_some_and(|c|now-c<1.5){"已复制"}else{"复制"},fade(if hover{AMBER}else{WHITE},foot),19.0,0.0);
        let cx=cx+124.0;
        {let draw=ui.get_window_draw_list();draw.add_line(p(cx,798.0),p(cx,832.0),col(WHITE,0.16*foot)).thickness(s).build();}
        let (clicked,hover)=hit("##law-clear",[cx+12.0,794.0],[cx+110.0,836.0]);clear=clicked;
        let w=badge(cx+18.0,815.0,crate::pad::Input::new(crate::pad::Pad::Y,""),crate::pad::Tone::Light,foot);
        text(cx+18.0+if w>0.0{w+10.0}else{0.0},802.0,"清空",fade(if hover{AMBER}else{WHITE},foot),19.0,0.0);
        text(40.0,848.0,"Ctrl+V 粘贴律法码 · 取消律法会连带取消其后的律法",fade(DIM,foot),13.0,0.0);
        // 分数（显示字体的大号数字，追着真实分数滚动）
        let digits=shown.to_string();let h=58.0;let w=big.width(digits.len(),h*s)/s;
        big.draw_text(ui,&digits,ox+(690.0-w)*s,oy+784.0*s,h*s,fade(WHITE,foot));
        micro(706.0,792.0,"RISK",fade(MUTED,foot),12.0,0.0);
        text(706.0,812.0,"当前分数",fade(WHITE,foot),19.0,0.0);
        let grade=GRADES.iter().rposition(|(min,_)|score>=*min);
        let (bx,by,bw)=(830.0,804.0,400.0);
        {
            let draw=ui.get_window_draw_list();
            draw.add_rect(p(bx,by),p(bx+bw,by+4.0),col(WHITE,0.14*foot)).filled(true).build();
            let fill=(shown as f32/TRACK_MAX).min(1.0)*bw*foot;
            draw.add_rect(p(bx,by),p(bx+fill,by+4.0),col(AMBER,foot)).filled(true).build();
            for (min,_) in GRADES.iter() {let x=bx+bw*(*min as f32/TRACK_MAX);draw.add_line(p(x,by-5.0),p(x,by+9.0),col(if score>=*min{AMBER}else{WHITE},0.5*foot)).thickness(1.4*s).build();}
        }
        for (i,(min,name)) in GRADES.iter().enumerate() {
            let x=bx+bw*(*min as f32/TRACK_MAX);let current=grade==Some(i);
            text(x,816.0,name,fade(if current{AMBER}else if score>=*min{WHITE}else{MUTED},foot),if current{18.0}else{16.0},0.5);
        }
        let next=GRADES.iter().find(|(min,_)|score<*min);
        text(bx+bw*0.5,846.0,&next.map_or("已达最高评级".to_string(),|(min,name)|format!("距「{name}」还差 {} 分",min-score)),fade(MUTED,foot),15.0,0.5);
        // 开始旅程：琥珀色实心按钮，悬停时从左往右铺一层亮色
        let (clicked,hover)=hit("##law-start",[1290.0,780.0],[1556.0,852.0]);if clicked {action=Some(LawAction::Start(screen.law));}
        {
            let draw=ui.get_window_draw_list();
            draw.add_rect(p(1290.0,780.0),p(1556.0,852.0),col(AMBER,foot)).filled(true).build();
            if hover {draw.add_rect(p(1290.0,780.0),p(1556.0,852.0),col([1.0,0.86,0.5,1.0],0.55*foot)).filled(true).build();}
            // 右下角的小切角
            draw.add_triangle(p(1556.0,852.0),p(1540.0,852.0),p(1556.0,836.0),ImColor32::from_rgba(13,13,12,255)).filled(true).build();
        }
        let w=badge(1322.0,816.0,crate::pad::START,crate::pad::Tone::Dark,foot);
        text(1336.0+w,792.0,"开始旅程",fade(INK,foot),28.0,0.0);
        micro(1338.0+w,830.0,"START",fade(INK,0.7*foot),10.0,0.0);
    });
    if copy {ui.set_clipboard_text(screen.law.code());screen.copied=Some(now);crate::sfx::play("hover");}
    if clear&&!screen.law.is_empty() {screen.law=Law::default();crate::sfx::play("close");}
    action
}
