// 旅程档案的「武器偏好」页（兵器研习）：只负责画与收集点击，升级、开关偏好由 archive_ui 按返回的动作处理。
// 左侧分类列表，中间是这一类的武器卡（菱形格表示研习等级、金点表示已启用），右侧是选中武器的详情与升级。
use hudhook::imgui::{Ui,TextureId};
use erdtree_tower::{legacy::Legacy,weapon_training::{TYPES,weight_percent}};
use crate::{archive_style::{self as style,GOLD,WHITE,MUTED,DIM,RED,Tone},run_screen::wrapped,law_ui::{panel,ornament}};

pub const WEAPON_GROUPS:[&str;8]=["剑与刀","槌与斧","长柄武器","其他近战","盾牌","施法工具","弓弩","幽影武器"];
/// 这一类武器在 TYPES 里的行号（从 1 开始）
pub fn weapon_rows(group:usize)->Vec<usize> {
    TYPES.iter().enumerate().filter(|(_, (kind,_))|match group {
        0=>(1..=10).contains(kind),1=>(11..=15).contains(kind),2=>(16..=19).contains(kind),3=>(20..=24).contains(kind),
        4=>(30..=32).contains(kind),5=>(33..=34).contains(kind),6=>(40..=44).contains(kind),_=>(60..=68).contains(kind),
    }).map(|(i,_)|i+1).collect()
}
pub enum WeaponAction {Group(usize),Row(usize),Upgrade,Toggle}
#[allow(clippy::too_many_arguments)]
pub fn draw(ui:&Ui,data:&Legacy,group:usize,row:usize,notice:Option<&str>,icon:&mut dyn FnMut(u32)->Option<TextureId>,ready:bool,s:f32,ox:f32,oy:f32)->Option<WeaponAction> {
    let p=|x:f32,y:f32|[ox+x*s,oy+y*s];
    let text=|x:f32,y:f32,value:&str,color:[f32;4],scale:f32| {ui.set_window_font_scale(scale*s);ui.set_cursor_screen_pos(p(x,y));ui.text_colored(color,value);};
    let right_text=|x:f32,y:f32,value:&str,color:[f32;4],scale:f32| {ui.set_window_font_scale(scale*s);let w=ui.calc_text_size(value)[0];ui.set_cursor_screen_pos([ox+x*s-w,oy+y*s]);ui.text_colored(color,value);};
    let mut action=None;
    // ── 左：分类 ──
    panel(&ui.get_window_draw_list(),p(56.0,170.0),p(300.0,800.0),s);
    text(80.0,186.0,"兵器研习",GOLD,1.35);
    ornament(&ui.get_window_draw_list(),p(178.0,236.0),110.0*s,s);
    for (index,name) in WEAPON_GROUPS.iter().enumerate() {
        let y=256.0+index as f32*58.0;let (min,max)=(p(66.0,y),p(290.0,y+50.0));
        ui.set_cursor_screen_pos(min);ui.set_window_font_scale(s);
        if ui.invisible_button(format!("weapon-group-{index}"),[max[0]-min[0],max[1]-min[1]])&&ready {action=Some(WeaponAction::Group(index));}
        let active=group==index;let hover=ui.is_item_hovered();
        let draw=ui.get_window_draw_list();
        if active {
            draw.add_rect_filled_multicolor(min,max,style::rgba([0.42,0.33,0.18,1.0],0.85),style::rgba([0.2,0.16,0.09,1.0],0.5),style::rgba([0.2,0.16,0.09,1.0],0.5),style::rgba([0.42,0.33,0.18,1.0],0.85));
            draw.add_rect(min,[min[0]+3.0*s,max[1]],style::rgba(GOLD,1.0)).filled(true).build();
            draw.add_polyline(vec![[min[0]-5.0*s,(min[1]+max[1])*0.5],[min[0],(min[1]+max[1])*0.5-5.0*s],[min[0]+5.0*s,(min[1]+max[1])*0.5],[min[0],(min[1]+max[1])*0.5+5.0*s]],style::rgba(GOLD,1.0)).filled(true).build();
        } else if hover {draw.add_rect(min,max,style::rgba([0.3,0.25,0.16,1.0],0.45)).filled(true).build();}
        // 分类图标：这一类第一种武器的图标
        let first=weapon_rows(index).first().map(|r|TYPES[r-1].0);
        if let Some(texture)=first.and_then(crate::reward_ui::weapon_type_icon).and_then(&mut *icon) {
            let light=if active{255}else{170};draw.add_image(texture,p(76.0,y+7.0),p(112.0,y+43.0)).col(hudhook::imgui::ImColor32::from_rgba(light,light,light,255)).build();
        }
        drop(draw);
        text(122.0,y+13.0,name,if active{GOLD}else{MUTED},1.02);
        right_text(278.0,y+15.0,&weapon_rows(index).len().to_string(),if active{GOLD}else{DIM},0.9);
    }
    ornament(&ui.get_window_draw_list(),p(178.0,736.0),100.0*s,s);
    let enabled=data.training.preferences().len();
    style::centered(ui,&format!("已启用  {enabled}  类偏好"),p(66.0,748.0),p(290.0,786.0),if enabled>0{GOLD}else{MUTED},0.92,s);
    // ── 中：这一类的武器卡 ──
    text(330.0,184.0,"投资你的打法，让心仪的武器更常出现",MUTED,0.95);
    text(330.0,222.0,WEAPON_GROUPS[group],WHITE,1.3);
    ui.get_window_draw_list().add_circle(p(888.0,236.0),4.0*s,style::rgba(GOLD,1.0)).filled(true).build();
    text(900.0,226.0,"金色圆点表示已启用",MUTED,0.82);
    for (slot,item) in weapon_rows(group).into_iter().enumerate() {
        let (kind,name)=TYPES[item-1];let level=data.training.level_for(kind);let active=data.training.enabled.contains(&kind)&&level>0;
        let (x,y)=(330.0+(slot%3) as f32*246.0,268.0+(slot/3) as f32*130.0);let (min,max)=(p(x,y),p(x+232.0,y+118.0));
        ui.set_cursor_screen_pos(min);ui.set_window_font_scale(s);
        if ui.invisible_button(format!("weapon-pref-{item}"),[max[0]-min[0],max[1]-min[1]])&&ready {action=Some(WeaponAction::Row(item));}
        let focus=row==item;let hover=ui.is_item_hovered();
        let draw=ui.get_window_draw_list();
        style::card(&draw,min,max,focus,hover,s);
        if let Some(texture)=crate::reward_ui::weapon_type_icon(kind).and_then(&mut *icon) {draw.add_image(texture,p(x+8.0,y+16.0),p(x+94.0,y+102.0)).build();}
        if active {draw.add_circle(p(x+214.0,y+18.0),4.5*s,style::rgba(GOLD,1.0)).filled(true).build();draw.add_circle(p(x+214.0,y+18.0),8.0*s,style::rgba(GOLD,0.25)).thickness(2.0*s).build();}
        style::pips(&draw,p(x+104.0,y+92.0),5,level,6.0*s,s);
        drop(draw);
        text(x+104.0,y+16.0,name,if focus{GOLD}else{WHITE},1.08);
        text(x+104.0,y+50.0,&format!("{:.1}×",weight_percent(level) as f32/100.0),if active{GOLD}else{MUTED},0.92);
    }
    // ── 右：详情 ──
    let (kind,name)=TYPES[row.saturating_sub(1).min(TYPES.len()-1)];
    let level=data.training.level_for(kind);let active=data.training.enabled.contains(&kind);let cost=data.training.cost_for(kind);
    let (left,right)=(1090.0,1530.0);
    panel(&ui.get_window_draw_list(),p(left,170.0),p(right,800.0),s);
    {let draw=ui.get_window_draw_list();style::coin(&draw,p(1370.0,198.0),11.0*s,s);}
    text(1390.0,187.0,"传承点",MUTED,0.9);
    right_text(right-24.0,184.0,&data.training_balance().to_string(),GOLD,1.2);
    if let Some(texture)=crate::reward_ui::weapon_type_icon(kind).and_then(&mut *icon) {ui.get_window_draw_list().add_image(texture,p(1108.0,226.0),p(1238.0,356.0)).build();}
    text(1256.0,240.0,name,GOLD,1.65);
    text(1256.0,296.0,&format!("研习等级  {level} / 5"),MUTED,0.95);
    {let draw=ui.get_window_draw_list();style::pips(&draw,p(1256.0,338.0),5,level,8.0*s,s);}
    ornament(&ui.get_window_draw_list(),p((left+right)*0.5,384.0),190.0*s,s);
    // 抽取权重：现在 → 下一级
    let now=format!("{:.1}×",weight_percent(level) as f32/100.0);
    if level<5 {
        let next=format!("{:.1}×",weight_percent(level+1) as f32/100.0);
        ui.set_window_font_scale(2.0*s);let (a,b)=(ui.calc_text_size(&now)[0]/s,ui.calc_text_size(&next)[0]/s);
        let total=a+56.0+b;let x0=(left+right)*0.5-total*0.5;
        text(x0,402.0,&now,WHITE,2.0);text(x0+a+14.0,410.0,"→",MUTED,1.4);text(x0+a+56.0,402.0,&next,GOLD,2.0);
    } else {style::centered(ui,&format!("{now}  ·  研习已满级"),p(left,402.0),p(right,446.0),GOLD,1.6,s);}
    {
        let draw=ui.get_window_draw_list();let (bx0,bx1,by)=(1128.0,1492.0,466.0);
        draw.add_rect(p(bx0,by),p(bx1,by+6.0),hudhook::imgui::ImColor32::from_rgba(40,36,30,255)).filled(true).build();
        draw.add_rect_filled_multicolor(p(bx0,by),p(bx0+(bx1-bx0)*f32::from(level)/5.0,by+6.0),style::rgba([0.6,0.45,0.2,1.0],1.0),style::rgba(GOLD,1.0),style::rgba(GOLD,1.0),style::rgba([0.6,0.45,0.2,1.0],1.0));
        draw.add_rect(p(bx0,by),p(bx1,by+6.0),style::rgba(GOLD,0.35)).thickness(s).build();
    }
    wrapped(ui,p(1128.0,494.0),ox+1494.0*s,"只影响武器类型的相对出率，不提高金紫品质。每类独立升级，下一局生效。",MUTED,0.9*s);
    let can_upgrade=cost.is_some_and(|c|data.training_balance()>=c);
    let label=match cost {None=>"研习已满级".to_string(),Some(c)=>format!("研习升级  ·  {c} 传承点")};
    if style::button_pad(ui,"weapon-training-upgrade",&label,Some(crate::pad::Input::new(crate::pad::Pad::Y,"Space")),p(1128.0,580.0),p(1492.0,634.0),if can_upgrade{Tone::Primary}else{Tone::Disabled},s)&&ready {action=Some(WeaponAction::Upgrade);}
    if let Some(c)=cost.filter(|_|!can_upgrade) {style::centered(ui,&format!("传承点不足 · 还差 {}",c.saturating_sub(data.training_balance())),p(left,640.0),p(right,668.0),RED,0.88,s);}
    let (toggle,tone)=if level==0 {("升级后即可启用偏好",Tone::Disabled)}else if active {("● 已启用偏好 · 关闭",Tone::Normal)}else{("启用此类偏好",Tone::Normal)};
    if style::button_pad(ui,"weapon-training-toggle",toggle,Some(crate::pad::CONFIRM),p(1128.0,676.0),p(1492.0,722.0),tone,s)&&ready {action=Some(WeaponAction::Toggle);}
    match notice {
        Some(message)=>wrapped(ui,p(1128.0,742.0),ox+1494.0*s,message,GOLD,0.86*s),
        None=>wrapped(ui,p(1128.0,742.0),ox+1494.0*s,"完成旅程获得传承点，花费不扣职业经验。",DIM,0.82*s),
    };
    action
}
