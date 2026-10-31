// 选择职业：大赐福“开始新旅程”后先选职业，再进入苦难律法。六张职业卡并排，方向键或鼠标选择，点击或 Enter / A 选定后再确认一次，Esc / B 取消选定或返回。
use hudhook::imgui::{Condition,DrawListMut,ImColor32,Key,Ui,WindowFlags};
use crate::law_ui::ornament;
use crate::run_screen::wrapped;

pub enum ClassAction {Pick(usize),Back}
/// 一个职业的卡面：名字、定位、机制名与说明、起手、擅长
pub struct Role {pub name:&'static str,pub role:&'static str,pub mechanic:(&'static str,&'static str),pub kit:&'static str,pub bias:&'static str,pub accent:(u8,u8,u8)}
/// 顺序与 erdtree_tower::Class::PLAYABLE 一致
pub const ROLES:[Role;6]=[
    Role{name:"守誓骑士",role:"坦克 · 举盾防守",mechanic:("誓守","举盾防御时受到的所有伤害降低 25%（含物理）；防御时精力消耗降低 30%"),kit:"长剑、斧枪、鸢形盾",bias:"直剑、大剑、斧枪、中盾与大盾",accent:(196,160,96)},
    Role{name:"狂战士",role:"重武器 · 硬抗",mechanic:("狂血","受到的削韧降低 30%；生命低于一半时受到的伤害降低 15%；打赢一场战斗回复 8% 生命"),kit:"巨斧、战斧、大皮盾",bias:"斧、大斧、槌、大槌、特大武器",accent:(200,84,64)},
    Role{name:"影刃",role:"敏捷 · 致命一击",mechanic:("影步","致命一击后 10 秒内对敌人的伤害提高 20%；精力恢复速度提高"),kit:"打刀、长弓、红棘木盾",bias:"刀、匕首、曲剑、弓",accent:(150,172,190)},
    Role{name:"星辉术士",role:"远程法术",mechanic:("星涌","进入战斗时专注回复一半；魔法专注消耗降低 15%"),kit:"辉石杖、短剑、辉石魔砾、辉石彗星",bias:"法杖与魔法",accent:(110,140,220)},
    Role{name:"祷誓者",role:"祷告 · 治疗 · 辅助",mechanic:("圣誓","祷告专注消耗降低 15%；打赢一场战斗回复 25% 专注"),kit:"短矛、指头圣印记、治疗、招来火焰",bias:"圣印记、祷告与矛",accent:(226,196,120)},
    Role{name:"召唤师",role:"骨灰为核心",mechanic:("群灵","战斗胜利时召唤物还活着，这次召唤不消耗骨灰；每页奖励都有骨灰，召唤流余烬必定可出"),kit:"短剑、熨斗形盾、圣印记、离群野狼与骸骨民兵的骨灰",bias:"骨灰、轻武器与祷告",accent:(140,196,170)},
];
/// 选中一张卡后要再确认一次才开始；confirm 之后播一段离场（选中的卡闪一下、其余变暗）再交给下一页
pub struct ClassScreen {pub focus:usize,opened:Option<f64>,chosen:Option<(usize,f64)>,leaving:Option<f64>,lift:[f32;6],dim:[f32;6]}
impl ClassScreen {pub fn new(focus:usize)->Self {Self{focus:focus.min(ROLES.len()-1),opened:None,chosen:None,leaving:None,lift:[0.0;6],dim:[0.0;6]}}}
/// 0→1 的缓出（三次方）
fn ease(elapsed:f64,start:f64,len:f64)->f32 {let t=((elapsed-start)/len).clamp(0.0,1.0) as f32;1.0-(1.0-t).powi(3)}
/// 离场动画的长度（秒），播完才返回 Pick
const LEAVE:f64=0.45;
/// 选定后要过这么久才接受确认，免得双击直接开局
const CONFIRM_DELAY:f64=0.25;
/// 入场：标题落下、装饰线展开 → 六张卡从下往上依次升起（插画自上而下显出、落定时边框一闪）→ 卡上文字与底部提示淡入。
/// 选择：方向键或鼠标移动焦点，焦点卡平滑抬起、外框呼吸；点击或 Enter / A 选定一张卡，其余变暗，底部出现确认栏，
/// 再点一次这张卡、确认按钮或 Enter / A 才开始；Esc / B 或方向键取消选定。
/// 文字：标题与职业名用 32 像素的标题宋体、正文用 20 像素的正文宋体，都按不大于加载字号的大小画，不放大位图
pub fn draw(ui:&Ui,screen:&mut ClassScreen)->Option<ClassAction> {
    let size=ui.io().display_size;let s=(size[0]/1600.0).min(size[1]/900.0);
    let (ox,oy)=((size[0]-1600.0*s)*0.5,(size[1]-900.0*s)*0.5);let p=|x:f32,y:f32|[ox+x*s,oy+y*s];
    let now=ui.time();let opened=*screen.opened.get_or_insert(now);let t=now-opened;
    if t==0.0 {crate::sfx::play("open");}
    let dt=ui.io().delta_time.clamp(0.0,0.1);
    let background=ui.get_background_draw_list();crate::oath_art::background(&background,size);
    let shade=|a:f32|ImColor32::from_rgba(0,0,0,(a*ease(t,0.0,0.35)) as u8);
    background.add_rect_filled_multicolor([0.0,0.0],size,shade(80.0),shade(80.0),shade(170.0),shade(170.0));drop(background);
    let leaving=screen.leaving.map(|at|now-at);
    if leaving.is_some_and(|l|l>=LEAVE) {return Some(ClassAction::Pick(screen.chosen.map_or(screen.focus,|c|c.0)));}
    let ready=t>=0.3&&leaving.is_none();
    let pressed=|keys:&[Key]|ready&&keys.iter().any(|k|ui.is_key_pressed(*k));
    let mut action=None;
    // 选定 / 确认 / 取消（键盘、手柄与鼠标都走这里）
    let choose=|screen:&mut ClassScreen,index:usize| {
        match screen.chosen {
            Some((chosen,at)) if chosen==index=>{if now-at>=CONFIRM_DELAY {screen.leaving=Some(now);crate::sfx::play("confirm");}},
            _=>{screen.chosen=Some((index,now));screen.focus=index;crate::sfx::play("confirm");},
        }
    };
    let step=|screen:&mut ClassScreen,delta:usize| {
        if screen.chosen.take().is_some() {crate::sfx::play("close");}
        screen.focus=(screen.focus+delta)%ROLES.len();crate::sfx::play("hover");
    };
    if pressed(&[Key::LeftArrow,Key::A,Key::GamepadDpadLeft]) {step(screen,ROLES.len()-1);}
    if pressed(&[Key::RightArrow,Key::D,Key::GamepadDpadRight]) {step(screen,1);}
    // 确认键不吃按住连发：按住 Enter 不会在选定后自动确认
    if ready&&[Key::Enter,Key::GamepadFaceDown].iter().any(|k|ui.is_key_pressed_no_repeat(*k)) {choose(screen,screen.focus);}
    else if pressed(&[Key::Escape,Key::GamepadFaceRight]) {
        if screen.chosen.take().is_some() {crate::sfx::play("close");}else{action=Some(ClassAction::Back);}
    }
    let moved=ui.io().mouse_delta!=[0.0,0.0];let time=now as f32;
    // 焦点与“其余变暗”的平滑过渡
    let follow=1.0-(-dt*14.0).exp();
    for i in 0..ROLES.len() {
        let lift=if screen.focus==i {1.0}else{0.0};screen.lift[i]+=(lift-screen.lift[i])*follow;
        let dim=match screen.chosen {Some((c,_)) if c!=i=>1.0,_=>0.0};screen.dim[i]+=(dim-screen.dim[i])*follow;
    }
    let fade=|c:[f32;4],k:f32|[c[0],c[1],c[2],c[3]*k.clamp(0.0,1.0)];
    let title_font=crate::fancy_text::title_font();
    ui.window("选择职业##classes").position([0.0,0.0],Condition::Always).size(size,Condition::Always)
        .flags(WindowFlags::NO_DECORATION|WindowFlags::NO_BACKGROUND|WindowFlags::NO_MOVE|WindowFlags::NO_NAV|WindowFlags::NO_SCROLL_WITH_MOUSE).build(|| {
        // px 为设计稿里的字号：22 以上用标题宋体（32 像素加载），以下用正文宋体（20 像素加载），都只缩小不放大
        let text=|x:f32,y:f32,value:&str,color:[f32;4],px:f32,align:f32| {
            if color[3]<=0.01 {return;}
            let font=title_font.filter(|_|px>=22.0).map(|f|ui.push_font(f));
            ui.set_window_font_scale(px/if font.is_some(){32.0}else{20.0}*s);
            ui.set_cursor_screen_pos([ox+x*s-ui.calc_text_size(value)[0]*align,oy+y*s]);ui.text_colored(color,value);
        };
        let measure=|value:&str,px:f32|->f32 {
            let _font=title_font.filter(|_|px>=22.0).map(|f|ui.push_font(f));
            ui.set_window_font_scale(px/if title_font.is_some()&&px>=22.0{32.0}else{20.0}*s);ui.calc_text_size(value)[0]/s
        };
        let (gold,white,muted,boon)=([0.86,0.74,0.48,1.0],[0.93,0.9,0.81,1.0],[0.62,0.59,0.51,1.0],[0.86,0.8,0.64,1.0]);
        // ── 标题：落下淡入，装饰线从中间展开 ──
        let head=ease(t,0.0,0.45);
        text(800.0,26.0+(head-1.0)*14.0,"选  择  职  业",fade(gold,head),32.0,0.5);
        text(800.0,80.0,"每个职业有一项专属机制、起手装备与擅长的奖励；职业传承按职业分别计算",fade(muted,ease(t,0.15,0.4)),17.0,0.5);
        let line=ease(t,0.2,0.5);
        if line>0.0 {let draw=ui.get_window_draw_list();ornament(&draw,p(800.0,122.0),(12.0+408.0*line)*s,s);}
        for (i,role) in ROLES.iter().enumerate() {
            let start=0.18+i as f64*0.07;
            let rise=ease(t,start,0.55);let content=ease(t,start+0.25,0.4);let reveal=ease(t,start+0.12,0.5);
            let flash=if t>=start+0.35 {1.0-ease(t,start+0.35,0.45)}else{0.0};
            let (lift,dim)=(screen.lift[i],screen.dim[i]);
            let chosen=screen.chosen.is_some_and(|c|c.0==i);
            let pick=match screen.chosen {Some((c,at)) if c==i=>ease(now,at,0.3),_=>0.0};
            let (x,y,w,h)=(52.0+i as f32*250.0,160.0-14.0*lift-6.0*pick+(1.0-rise)*70.0,236.0,640.0);
            let (min,max)=(p(x,y),p(x+w,y+h));
            ui.set_cursor_screen_pos(min);ui.set_window_font_scale(s);
            let clicked=ui.invisible_button(format!("role-{i}"),[w*s,h*s]);
            if ui.is_item_hovered()&&moved&&ready&&screen.chosen.is_none()&&screen.focus!=i {screen.focus=i;crate::sfx::play("hover");}
            if clicked&&ready {choose(screen,i);}
            if rise<=0.0 {continue;}
            let (r,g,b)=role.accent;let accent=[f32::from(r)/255.0,f32::from(g)/255.0,f32::from(b)/255.0,1.0];
            let a=|k:f32|(k.clamp(0.0,1.0)*255.0) as u8;
            let draw=ui.get_window_draw_list();
            // 卡底与金边（随入场淡入）；焦点卡外框按职业色呼吸，选定的卡再加一圈金色双框
            draw.add_rect_filled_multicolor(min,max,ImColor32::from_rgba(16,13,9,a(0.9*rise)),ImColor32::from_rgba(16,13,9,a(0.9*rise)),ImColor32::from_rgba(8,7,5,a(0.93*rise)),ImColor32::from_rgba(8,7,5,a(0.93*rise)));
            draw.add_rect(min,max,ImColor32::from_rgba(171,143,87,a((0.47+0.3*lift)*rise))).thickness(s).build();
            let corner=ImColor32::from_rgba(214,180,112,a(0.9*rise));let l=(18.0+8.0*lift)*s;
            for (cx,cy,dx,dy) in [(min[0],min[1],1.0,1.0),(max[0],min[1],-1.0,1.0),(min[0],max[1],1.0,-1.0),(max[0],max[1],-1.0,-1.0)] {
                draw.add_line([cx,cy],[cx+l*dx,cy],corner).thickness(2.0*s).build();draw.add_line([cx,cy],[cx,cy+l*dy],corner).thickness(2.0*s).build();
            }
            if lift>0.01 {
                let breath=(0.43+0.31*(time*2.2).sin())*lift*rise;
                for ring in 1..=3 {
                    let d=(ring as f32*2.5+1.0)*s;
                    draw.add_rect([min[0]-d,min[1]-d],[max[0]+d,max[1]+d],ImColor32::from_rgba(r,g,b,a(breath/ring as f32))).rounding(3.0*s).thickness(2.0*s).build();
                }
            }
            if pick>0.0 {
                let d=(10.0-4.0*pick)*s;let gold32=|k:f32|ImColor32::from_rgba(240,208,140,a(k*pick));
                draw.add_rect([min[0]-d,min[1]-d],[max[0]+d,max[1]+d],gold32(0.95)).thickness(1.6*s).build();
                draw.add_rect([min[0]-d-4.0*s,min[1]-d-4.0*s],[max[0]+d+4.0*s,max[1]+d+4.0*s],gold32(0.35)).thickness(s).build();
            }
            if flash>0.0 {draw.add_rect(min,max,ImColor32::from_rgba(255,240,200,a(0.8*flash))).thickness((1.0+2.0*flash)*s).build();}
            // 职业插画：自上而下显出，前沿一道亮线；焦点卡亮、缓慢推近（贴图未加载时退回职业色的菱形徽记）
            let (art_min,art_max)=(p(x+8.0,y+8.0),p(x+w-8.0,y+218.0));
            let front=art_min[1]+(art_max[1]-art_min[1])*reveal;
            if reveal>0.0 {
                draw.with_clip_rect_intersect(art_min,[art_max[0],front],|| {
                    let zoom=1.0+0.035*lift*(0.5-0.5*(time*0.6).cos());
                    let light=(185.0+70.0*lift-90.0*dim).clamp(0.0,255.0)*rise;
                    if crate::oath_art::ready() {crate::oath_art::class_art(&draw,i,art_min,art_max,light as u8,zoom);}
                    else {
                        let c=p(x+w*0.5,y+110.0);let hr=40.0*s;
                        let diamond=vec![[c[0],c[1]-hr],[c[0]+hr,c[1]],[c[0],c[1]+hr],[c[0]-hr,c[1]]];
                        draw.add_polyline(diamond.clone(),ImColor32::from_rgba(r/4,g/4,b/4,255)).filled(true).build();
                        let mut outline=diamond.clone();outline.push(diamond[0]);
                        draw.add_polyline(outline,ImColor32::from_rgba(r,g,b,255)).thickness(2.0*s).build();
                    }
                });
                effects(&draw,i,art_min,[art_max[0],front],lift>0.5,time,s,content*(1.0-0.6*dim));
                if reveal<1.0 {draw.add_line([art_min[0],front],[art_max[0],front],ImColor32::from_rgba(255,236,190,a(0.9*(1.0-reveal)))).thickness(2.0*s).build();}
                draw.add_rect(art_min,[art_max[0],front],ImColor32::from_rgba(r,g,b,a((0.24+0.35*lift)*rise))).thickness(s).build();
            }
            drop(draw);
            // 卡上文字
            let k=content*(1.0-0.45*dim);
            text(x+w*0.5,y+228.0,role.name,fade(if lift>0.5||chosen{gold}else{white},k),26.0,0.5);
            text(x+w*0.5,y+266.0,role.role,fade(accent,k),16.5,0.5);
            if content>0.0 {let draw=ui.get_window_draw_list();ornament(&draw,p(x+w*0.5,y+298.0),(20.0+70.0*content)*s,s);}
            let right=ox+(x+w-18.0)*s;
            text(x+18.0,y+312.0,&format!("机制 · {}",role.mechanic.0),fade(gold,k),19.0,0.0);
            // 正文 18 像素（正文宋体按 20 像素加载，只缩小）；数值写成“降低 25%”而不是“-25%”：细横线的减号缩小后不到一像素高，会整个消失
            let next=wrapped(ui,p(x+18.0,y+342.0),right,role.mechanic.1,fade(boon,k),0.9*s);
            let mut yy=(next-oy)/s+16.0;
            text(x+18.0,yy,"起手",fade(gold,k),18.0,0.0);yy+=27.0;
            yy=(wrapped(ui,p(x+18.0,yy),right,role.kit,fade(white,k),0.9*s)-oy)/s+14.0;
            text(x+18.0,yy,"擅长",fade(gold,k),18.0,0.0);yy+=27.0;
            wrapped(ui,p(x+18.0,yy),right,role.bias,fade(white,k),0.9*s);
            // 选定后其余卡蒙一层暗色；选定的卡挂“已选择”角标
            let draw=ui.get_window_draw_list();
            if dim>0.01 {draw.add_rect(min,max,ImColor32::from_rgba(0,0,0,a(0.42*dim))).filled(true).build();}
            drop(draw);
            if chosen&&pick>0.3 {crate::archive_style::ribbon(ui,"已选择",[max[0],min[1]+8.0*s],s);}
        }
        // ── 底部：未选定时是操作提示，选定后升起确认栏 ──
        match screen.chosen {
            None=>{
                let a=ease(t,0.75,0.4);
                crate::pad::hints(ui,p(800.0,852.0),0.5,&[(Some(crate::pad::MOVE_X),"选择"),(Some(crate::pad::CONFIRM),"选定"),(Some(crate::pad::BACK),"返回")],24.0*s,16.0*s,fade(muted,a),crate::pad::Tone::Light);
            },
            Some((index,at))=>{
                let k=ease(now,at,0.3);let lift=(1.0-k)*24.0;
                let question=format!("以「{}」开始旅程？",ROLES[index].name);
                let (bw,gap)=(220.0,16.0);let qw=measure(&question,24.0);
                let total=qw+28.0+bw*2.0+gap;let x0=800.0-total*0.5;let y0=822.0+lift;
                let draw=ui.get_window_draw_list();
                let band=|a:f32|ImColor32::from_rgba(10,8,5,(a*k*255.0) as u8);
                draw.add_rect_filled_multicolor(p(x0-160.0,y0-8.0),p(800.0,y0+54.0),band(0.0),band(0.75),band(0.75),band(0.0));
                draw.add_rect_filled_multicolor(p(800.0,y0-8.0),p(x0+total+160.0,y0+54.0),band(0.75),band(0.0),band(0.0),band(0.75));
                drop(draw);
                text(x0,y0+8.0,&question,fade(gold,k),24.0,0.0);
                let bx=x0+qw+28.0;
                if crate::archive_style::button_pad(ui,"class-confirm","确认",Some(crate::pad::CONFIRM),p(bx,y0),p(bx+bw,y0+46.0),crate::archive_style::Tone::Primary,s)&&ready {choose(screen,index);}
                if crate::archive_style::button_pad(ui,"class-cancel","重新选择",Some(crate::pad::BACK),p(bx+bw+gap,y0),p(bx+bw*2.0+gap,y0+46.0),crate::archive_style::Tone::Normal,s)&&ready {screen.chosen=None;crate::sfx::play("close");}
            },
        }
    });
    // 离场：选定的卡外扩一圈金光，全屏渐暗
    if let Some(l)=leaving {
        let k=ease(l,0.0,LEAVE);let draw=ui.get_foreground_draw_list();
        if let Some((i,_))=screen.chosen {
            let (x,y)=(52.0+i as f32*250.0,154.0-14.0*screen.lift[i]);let d=(4.0+36.0*k)*s;
            draw.add_rect([p(x,y)[0]-d,p(x,y)[1]-d],[p(x+236.0,y+640.0)[0]+d,p(x+236.0,y+640.0)[1]+d],ImColor32::from_rgba(255,226,160,((1.0-k)*230.0) as u8)).thickness((3.0-2.0*k)*s).build();
        }
        draw.add_rect([0.0,0.0],size,ImColor32::from_rgba(0,0,0,(k*k*200.0) as u8)).filled(true).build();
    }
    action
}

/// 0..1 的确定性随机数：同一颗粒子每帧取到相同的值
fn rand(i:u32,k:u32)->f32 {
    let mut x=i.wrapping_mul(0x9E37_79B1)^k.wrapping_mul(0x85EB_CA77);
    x^=x>>15;x=x.wrapping_mul(0x2C1B_3C6D);x^=x>>12;
    (x&0xffff) as f32/65535.0
}
/// 带一圈柔光的粒子
fn mote(draw:&DrawListMut<'_>,c:[f32;2],radius:f32,(r,g,b):(u8,u8,u8),alpha:f32) {
    let a=|k:f32|(alpha.clamp(0.0,1.0)*k*255.0) as u8;
    draw.add_circle(c,radius*2.6,ImColor32::from_rgba(r,g,b,a(0.18))).filled(true).build();
    draw.add_circle(c,radius,ImColor32::from_rgba(r,g,b,a(0.95))).filled(true).build();
}
/// 卡面插画上的动态效果：每个职业一种粒子；选中时粒子更多更亮，并有一道光带扫过
fn effects(draw:&DrawListMut<'_>,index:usize,min:[f32;2],max:[f32;2],focused:bool,time:f32,s:f32,alpha:f32) {
    let (w,h)=(max[0]-min[0],max[1]-min[1]);
    if alpha<=0.0||h<=0.0 {return;}
    let count=if focused {22}else{7};let dim=if focused {alpha}else{0.45*alpha};
    draw.with_clip_rect_intersect(min,max,|| {
        for i in 0..count {
            let (a,b,c,d)=(rand(i,index as u32*7+1),rand(i,index as u32*7+2),rand(i,index as u32*7+3),rand(i,index as u32*7+4));
            match index {
                // 狂战士：翻飞的红色余烬，忽明忽暗
                1=>{
                    let life=(time*(0.35+0.4*a)+b).fract();
                    let x=min[0]+w*c+(time*4.0+i as f32).sin()*6.0*s;let y=max[1]-life*h*1.1;
                    let flicker=0.6+0.4*(time*13.0+i as f32*1.7).sin();
                    mote(draw,[x,y],(1.4+1.6*d)*s,(255,96,52),(std::f32::consts::PI*life).sin()*flicker*dim);
                },
                // 影刃：横向飘过的雾
                2=>{
                    let life=(time*(0.05+0.05*a)+b).fract();
                    let x=min[0]-40.0*s+(w+80.0*s)*life;let y=min[1]+h*(0.45+0.5*c);
                    draw.add_circle([x,y],(18.0+16.0*d)*s,ImColor32::from_rgba(150,160,176,((std::f32::consts::PI*life).sin()*0.12*dim*255.0) as u8)).filled(true).build();
                },
                // 星辉术士：闪烁的星光，较亮的带十字光芒
                3=>{
                    let (x,y)=(min[0]+w*a,min[1]+h*0.75*b);
                    let twinkle=(0.5+0.5*(time*(2.0+3.0*c)+d*6.283).sin())*dim;
                    mote(draw,[x,y],(0.9+1.3*d)*s,(150,186,255),twinkle);
                    if d>0.7 {
                        let l=(5.0+5.0*twinkle)*s;let color=ImColor32::from_rgba(190,214,255,(twinkle*200.0) as u8);
                        draw.add_line([x-l,y],[x+l,y],color).thickness(s).build();draw.add_line([x,y-l],[x,y+l],color).thickness(s).build();
                    }
                },
                // 守誓骑士、祷誓者、召唤师：缓缓上升的光点
                _=>{
                    let (speed,color,size,sway,spread)=match index {
                        0=>(0.18,(255,208,120),1.0,3.0,1.0),
                        4=>(0.14,(255,232,168),1.4,4.0,0.45),
                        _=>(0.12,(120,240,210),2.0,10.0,1.0),
                    };
                    let life=(time*speed*(0.6+0.8*a)+b).fract();
                    let x=min[0]+w*((1.0-spread)*0.5+spread*c)+(time*1.5+i as f32).sin()*sway*s;let y=max[1]-life*h*1.05;
                    mote(draw,[x,y],(size+1.2*d)*s,color,(std::f32::consts::PI*life).sin()*dim);
                },
            }
        }
        // 选中时一道光带每 4.5 秒从左到右扫过插画
        if focused {
            let progress=(time%4.5)/1.6;
            if progress<1.0 {
                let center=min[0]-60.0*s+(w+120.0*s)*progress;let half=26.0*s;
                let (clear,bright)=(ImColor32::from_rgba(255,244,220,0),ImColor32::from_rgba(255,244,220,(46.0*alpha) as u8));
                draw.add_rect_filled_multicolor([center-half,min[1]],[center,max[1]],clear,bright,bright,clear);
                draw.add_rect_filled_multicolor([center,min[1]],[center+half,max[1]],bright,clear,clear,bright);
            }
        }
    });
}
