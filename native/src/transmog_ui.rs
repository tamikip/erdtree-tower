use hudhook::imgui::{Ui,TextureId,Key,ImColor32};
use erdtree_tower::transmog::{PARTS,BARE};
use crate::{transmog,run_screen::wrapped,law_ui::{panel,ornament},archive_style::{self as style,GOLD,WHITE,MUTED,DIM,Tone}};
#[derive(Default)]
pub struct Page {part:usize,page:usize,selection:usize,search:String,notice:String}
fn apply(page:&mut Page,id:i32) {
    let result=crate::legacy::owner().ok_or("请先载入角色".to_string()).and_then(|owner|transmog::choose(owner,page.part,id));
    page.notice=match result {Ok(())=>"外观已保存，属性与词条不变".into(),Err(error)=>error};
}
/// 这一部位的代表图标（目录里第一件），部位按钮与“跟随实际装备”的占位用
fn part_icon(part:usize)->Option<u32> {transmog::catalog().iter().find(|a|a.slot==part).map(|a|a.icon)}
/// 某一部位当前外观的名字与图标（跟随实际装备、隐藏此部位时没有图标）
fn look(part:usize,id:i32)->(&'static str,Option<u32>) {
    if id==-1 {("跟随实际装备",None)}else if id==BARE[part] {("隐藏此部位",None)}
    else {transmog::catalog().iter().find(|a|a.id==id).map_or(("未知防具",None),|a|(a.name.as_str(),Some(a.icon)))}
}
/// 防具幻化页（参考概念图）：左边部位切换、搜索框与 4 × 3 的防具卡，右边是四个部位的当前外观
#[allow(clippy::too_many_arguments)]
pub fn draw(ui:&Ui,page:&mut Page,icon:&mut dyn FnMut(u32)->Option<TextureId>,ready:bool,s:f32,ox:f32,oy:f32) {
    let p=|x:f32,y:f32|[ox+x*s,oy+y*s];
    let text=|x:f32,y:f32,value:&str,color:[f32;4],scale:f32|{ui.set_window_font_scale(scale*s);ui.set_cursor_screen_pos(p(x,y));ui.text_colored(color,crate::i18n::t(value));};
    text(76.0,176.0,"防具幻化",GOLD,1.45);
    text(214.0,186.0,"只改变外观，保留当前装备的防御、重量与词条",MUTED,0.92);
    if !transmog::unlocked() {
        panel(&ui.get_window_draw_list(),p(76.0,222.0),p(1530.0,806.0),s);
        style::centered(ui,"尚未解锁",p(76.0,365.0),p(1530.0,417.0),GOLD,1.6,s);
        ornament(&ui.get_window_draw_list(),p(803.0,445.0),200.0*s,s);
        style::centered(ui,erdtree_tower::transmog::LOCKED_REASON,p(76.0,480.0),p(1530.0,524.0),WHITE,1.15,s);
        style::centered(ui,"单人、联机均可 · 调试旅程不计入",p(76.0,548.0),p(1530.0,584.0),MUTED,0.95,s);
        return;
    }
    // 部位切换：四段，带代表图标
    for (part,name) in PARTS.iter().enumerate() {
        let x=76.0+part as f32*232.0;let (min,max)=(p(x,222.0),p(x+218.0,270.0));
        ui.set_cursor_screen_pos(min);ui.set_window_font_scale(s);
        if ui.invisible_button(format!("transmog-part-{part}"),[max[0]-min[0],max[1]-min[1]])&&ready {page.part=part;page.page=0;page.selection=0;}
        let active=page.part==part;let hover=ui.is_item_hovered();
        let draw=ui.get_window_draw_list();style::card(&draw,min,max,active,hover,s);
        if let Some(texture)=part_icon(part).and_then(&mut *icon) {let light=if active{255}else{150};draw.add_image(texture,p(x+30.0,224.0),p(x+74.0,268.0)).col(ImColor32::from_rgba(light,light,light,255)).build();}
        drop(draw);
        text(x+92.0,234.0,name,if active{GOLD}else{MUTED},1.1);
    }
    // 搜索框
    let choices:Vec<_>=transmog::catalog().iter().filter(|a|a.slot==page.part&&crate::i18n::matches(&a.name,&page.search)&&erdtree_tower::dlc::item_allowed(0x10000000|a.id as u32)).collect();
    let (changed,typing)=style::search(ui,"##transmog-search",&mut page.search,p(76.0,284.0),p(1000.0,322.0),"搜索防具…",&format!("{} 件",choices.len()),s);
    if changed {page.page=0;page.selection=0;}
    let pages=choices.len().div_ceil(12).max(1);page.page=page.page.min(pages-1);
    let key=|k|ready&&!typing&&ui.is_key_pressed(k);
    if key(Key::PageUp)||key(Key::GamepadL2){page.page=page.page.saturating_sub(1);page.selection=0;}
    if key(Key::PageDown)||key(Key::GamepadR2){page.page=(page.page+1).min(pages-1);page.selection=0;}
    let visible:Vec<_>=choices.iter().skip(page.page*12).take(12).copied().collect();
    if !visible.is_empty() {
        if key(Key::LeftArrow)||key(Key::GamepadDpadLeft){page.selection=page.selection.saturating_sub(1);}
        if key(Key::RightArrow)||key(Key::GamepadDpadRight){page.selection=(page.selection+1).min(visible.len()-1);}
        if key(Key::UpArrow)||key(Key::GamepadDpadUp){page.selection=page.selection.saturating_sub(4);}
        if key(Key::DownArrow)||key(Key::GamepadDpadDown){page.selection=(page.selection+4).min(visible.len()-1);}
        page.selection=page.selection.min(visible.len()-1);
        if key(Key::Enter)||key(Key::GamepadFaceDown){apply(page,visible[page.selection].id);}
    }
    // 防具卡：图标在上、名字在下；正在用的挂“当前外观”角标
    let worn=transmog::current();
    for (i,item) in visible.iter().enumerate() {
        let (x,y)=(76.0+(i%4) as f32*232.0,338.0+(i/4) as f32*140.0);let (min,max)=(p(x,y),p(x+218.0,y+128.0));
        ui.set_cursor_screen_pos(min);ui.set_window_font_scale(s);
        if ui.invisible_button(format!("transmog-armor-{}",item.id),[max[0]-min[0],max[1]-min[1]])&&ready{page.selection=i;apply(page,item.id);}
        let active=worn.armor[page.part]==item.id;let hover=ui.is_item_hovered();let focus=page.selection==i;
        let draw=ui.get_window_draw_list();style::card(&draw,min,max,focus||active,hover,s);
        if let Some(texture)=icon(item.icon){draw.add_image(texture,p(x+67.0,y+8.0),p(x+151.0,y+92.0)).build();}
        draw.add_line(p(x+14.0,y+96.0),p(x+204.0,y+96.0),ImColor32::from_rgba(120,104,74,90)).thickness(s).build();
        drop(draw);
        ui.set_window_font_scale(0.95*s);let fit=(200.0*s/ui.calc_text_size(crate::i18n::t(&item.name))[0]).min(1.0);
        style::centered(ui,&item.name,p(x,y+98.0),p(x+218.0,y+126.0),if active{GOLD}else if focus{[1.0,0.95,0.82,1.0]}else{WHITE},0.95*fit,s);
        if active {style::ribbon(ui,"当前外观",p(x+218.0,y),s);}
    }
    if visible.is_empty(){style::centered(ui,"没有匹配的防具",p(76.0,440.0),p(1000.0,480.0),DIM,1.0,s);}
    // 翻页：两侧按钮，中间页码圆点
    if style::button(ui,"transmog-prev","〈  上一页",p(206.0,766.0),p(372.0,806.0),if page.page>0{Tone::Normal}else{Tone::Disabled},s)&&ready {page.page=page.page.saturating_sub(1);page.selection=0;}
    if style::button(ui,"transmog-next","下一页  〉",p(704.0,766.0),p(870.0,806.0),if page.page+1<pages{Tone::Normal}else{Tone::Disabled},s)&&ready {page.page=(page.page+1).min(pages-1);page.selection=0;}
    {let draw=ui.get_window_draw_list();style::dots(&draw,p(512.0,778.0),pages,page.page,s);}
    style::centered(ui,&format!("{} / {}",page.page+1,pages),p(440.0,788.0),p(584.0,810.0),MUTED,0.82,s);
    // ── 右：当前外观 ──
    let (left,right,top,bottom)=(1070.0,1530.0,222.0,806.0);
    panel(&ui.get_window_draw_list(),p(left,top),p(right,bottom),s);
    style::centered(ui,"当前外观",p(left,top+12.0),p(right,top+50.0),GOLD,1.3,s);
    ornament(&ui.get_window_draw_list(),p((left+right)*0.5,top+60.0),170.0*s,s);
    // 中间：所选部位的外观放大
    let (center_min,center_max)=(p(1222.0,330.0),p(1378.0,486.0));
    {
        let draw=ui.get_window_draw_list();
        let c=[(center_min[0]+center_max[0])*0.5,(center_min[1]+center_max[1])*0.5];
        for ring in 0..3 {draw.add_circle(c,(70.0+ring as f32*10.0)*s,ImColor32::from_rgba(226,190,120,(40-ring*12) as u8)).thickness(1.2*s).num_segments(48).build();}
        draw.add_circle(c,66.0*s,ImColor32::from_rgba(10,9,7,230)).filled(true).num_segments(48).build();
        let (_,selected_icon)=look(page.part,worn.armor[page.part]);
        match selected_icon.or_else(||part_icon(page.part)).and_then(&mut *icon) {
            Some(texture)=>{let light=if selected_icon.is_some(){255}else{70};draw.add_image(texture,p(1240.0,348.0),p(1360.0,468.0)).col(ImColor32::from_rgba(light,light,light,255)).build();},
            None=>{},
        }
    }
    // 四个部位：头部、手部在左，胸部、腿部在右；点一下切换到这个部位
    let tiles=[(0usize,1100.0,312.0),(1,1416.0,312.0),(2,1100.0,486.0),(3,1416.0,486.0)];
    for (part,x,y) in tiles {
        let (min,max)=(p(x,y),p(x+84.0,y+84.0));
        ui.set_cursor_screen_pos(min);ui.set_window_font_scale(s);
        if ui.invisible_button(format!("transmog-slot-{part}"),[max[0]-min[0],max[1]-min[1]])&&ready {page.part=part;page.page=0;page.selection=0;}
        let active=page.part==part;let hover=ui.is_item_hovered();
        let (name,armor_icon)=look(part,worn.armor[part]);
        let draw=ui.get_window_draw_list();
        // 连线：从部位格连到中间的放大图
        let (fx,fy)=if x<1300.0 {(x+84.0,y+42.0)}else{(x,y+42.0)};let (tx,ty)=(if x<1300.0{1222.0}else{1378.0},408.0);
        draw.add_line(p(fx,fy),p((fx+tx)*0.5,fy),ImColor32::from_rgba(226,190,120,if active{200}else{80})).thickness(s).build();
        draw.add_line(p((fx+tx)*0.5,fy),p(tx,ty),ImColor32::from_rgba(226,190,120,if active{200}else{80})).thickness(s).build();
        style::card(&draw,min,max,active,hover,s);
        match armor_icon.or_else(||part_icon(part)).and_then(&mut *icon) {
            Some(texture)=>{let light=if armor_icon.is_some(){255}else{60};draw.add_image(texture,p(x+6.0,y+6.0),p(x+78.0,y+78.0)).col(ImColor32::from_rgba(light,light,light,255)).build();},
            None=>{},
        }
        drop(draw);
        style::centered(ui,PARTS[part],p(x-30.0,y+88.0),p(x+114.0,y+112.0),if active{GOLD}else{MUTED},0.9,s);
        ui.set_window_font_scale(0.82*s);let fit=(150.0*s/ui.calc_text_size(crate::i18n::t(name))[0]).min(1.0);
        style::centered(ui,name,p(x-34.0,y+112.0),p(x+118.0,y+134.0),if armor_icon.is_some(){WHITE}else{DIM},0.82*fit,s);
    }
    if style::button(ui,"transmog-hide","隐藏当前部位",p(1100.0,668.0),p(1292.0,712.0),Tone::Normal,s)&&ready{apply(page,BARE[page.part]);}
    if style::button(ui,"transmog-restore","恢复当前部位",p(1308.0,668.0),p(1500.0,712.0),Tone::Normal,s)&&ready{apply(page,-1);}
    let status=if page.notice.is_empty(){transmog::status()}else{page.notice.clone()};
    ui.set_window_font_scale(0.85*s);let w=ui.calc_text_size(crate::i18n::t(&status))[0];
    if w<=400.0*s {style::centered(ui,&status,p(left,730.0),p(right,760.0),if page.notice.is_empty(){DIM}else{GOLD},0.85,s);}
    else {wrapped(ui,p(1100.0,730.0),ox+1500.0*s,&status,DIM,0.85*s);}
}
