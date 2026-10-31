// 旅程档案：往期旅程（最近的整局记录，可打开当时的结算页）、图鉴（誓约、余烬、Boss、词条的解锁情况）
// 与职业传承（局外养成：每个职业单独的熟练度，升级解锁可替换默认开局护符的护符）。
// 键盘 J、路线图边栏“档案”、苦难律法界面的按钮或手柄 View 键打开；Q / E 或 LB / RB 切换分页。
use erdtree_tower::{Class,contracts::Kind,embers::Ember,journal::{self,Codex,Outcome,Record},legacy::{self as heritage,Legacy}};
use hudhook::imgui::{Condition,ImColor32,Key,StyleVar,TextureId,Ui,WindowFlags};
use crate::law_ui::{ornament,panel};
use crate::run_screen::{oath_badge,wrapped};

pub enum ArchiveAction {Close,Open(Record)}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Item {Oath(usize),Ember(usize),Boss(usize),Affix(usize)}
/// 往期旅程只列最近的这么多局
const LIMIT:usize=100;
const ROMAN:[&str;4]=["","I","II","III"];
const GOLD:[f32;4]=[0.94,0.82,0.56,1.0];const WHITE:[f32;4]=[0.96,0.94,0.87,1.0];const MUTED:[f32;4]=[0.78,0.75,0.67,1.0];
const DIM:[f32;4]=[0.65,0.62,0.55,1.0];const RED:[f32;4]=[0.92,0.47,0.38,1.0];const BOON:[f32;4]=[0.86,0.8,0.64,1.0];
pub struct Archive {records:Vec<Record>,codex:Codex,bosses:Vec<(u8,String)>,affixes:Vec<String>,tab:usize,selected:usize,focus:usize,follow:bool,opened:Option<f64>,now:u64,
    /// 职业传承：角色存档与读到的数据；class 是选中的职业，talent 是选中的开局护符，column 0 为职业列表、1 为护符列表
    legacy:Option<((u32,i32),Legacy)>,class:usize,talent:usize,column:usize,notice:Option<(String,f64)>,
    /// 称号页：已解锁的称号编号（按全部旅程档案算）、选中的行
    titles:Vec<u8>,title_row:usize,
    /// 光环页：已解锁的光环编号、选中的行
    auras:Vec<u8>,aura_row:usize,weapon_row:usize,weapon_group:usize,appearance:crate::transmog_ui::Page}
impl Archive {
    pub fn progression(records:Vec<Record>)->Self {
        let mut archive=Self::new(records);
        archive.tab=2;
        archive.column=0;
        archive
    }
    pub fn new(records:Vec<Record>)->Self {
        let codex=journal::codex(&records);
        let owner=crate::legacy::owner();
        let unlocks=owner.map(crate::cosmetic_unlocks::Unlocks::load).unwrap_or_default();
        let titles=erdtree_tower::titles::TITLES.iter().map(|t|t.id).filter(|id|unlocks.title(*id,&records)).collect();
        let auras=erdtree_tower::auras::AURAS.iter().map(|a|a.id).filter(|id|unlocks.aura(*id,&records)).collect();
        let recent:Vec<Record>=records.into_iter().rev().take(LIMIT).collect();
        let mut bosses:Vec<(u8,String)>=Vec::new();
        for group in crate::enemies::debug_encounters() {
            if let Some(first)=group.first() && !bosses.iter().any(|(_,n)|n==first.name) {bosses.push((first.act,first.name.to_owned()));}
        }
        for (name,..) in &codex.bosses {if !bosses.iter().any(|(_,n)|n==name) {bosses.push((0,name.clone()));}}
        let mut affixes:Vec<String>=crate::reward_ui::affix_names().to_vec();
        for (name,_) in &codex.affixes {if !affixes.contains(name) {affixes.push(name.clone());}}
        let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|t|t.as_secs());
        let legacy=owner.map(|owner|(owner,crate::legacy::load(owner)));
        let class=recent_class(&recent).and_then(|c|Class::PLAYABLE.iter().position(|x|*x==c)).unwrap_or(0);
        Self{records:recent,codex,bosses,affixes,tab:0,selected:0,focus:0,follow:true,opened:None,now,legacy,class,talent:0,column:1,notice:None,titles,title_row:0,auras,aura_row:0,weapon_row:1,weapon_group:0,appearance:Default::default()}
    }
    fn unlocked(&self,item:Item)->bool {
        match item {
            Item::Oath(i)=>self.codex.oaths[i].1>0,
            Item::Ember(i)=>self.codex.embers[i].1>0,
            Item::Boss(i)=>self.codex.boss(&self.bosses[i].1).is_some(),
            Item::Affix(i)=>self.codex.affix(&self.affixes[i])>0,
        }
    }
}
/// 传承页默认选中最近一局的职业
fn recent_class(records:&[Record])->Option<Class> {records.first().map(|r|r.class)}
/// “3 小时前”之类的相对时间
fn ago(now:u64,time:u64)->String {
    if time==0 {return String::new();}
    let secs=now.saturating_sub(time);
    match secs {0..60=>"刚刚".into(),60..3600=>format!("{} 分钟前",secs/60),3600..86400=>format!("{} 小时前",secs/3600),_=>format!("{} 天前",secs/86400)}
}
/// 图鉴布局（画布单位，相对内容区左上角）：分区标题与每个条目的位置
struct Layout {headings:Vec<(&'static str,f32,usize,usize)>,items:Vec<(Item,[f32;4])>,height:f32}
fn layout(archive:&Archive)->Layout {
    let mut out=Layout{headings:vec![],items:vec![],height:0.0};let mut y=0.0;
    let sections:[(&str,usize,usize,f32,f32,fn(usize)->Item);4]=[
        ("誓约",Kind::ALL.len(),5,192.0,64.0,Item::Oath),("余烬",Ember::ALL.len(),6,160.0,118.0,Item::Ember),
        ("Boss",archive.bosses.len(),4,244.0,54.0,Item::Boss),("词条",archive.affixes.len(),4,244.0,46.0,Item::Affix)];
    for (title,count,columns,w,h,make) in sections {
        let unlocked=(0..count).filter(|&i|archive.unlocked(make(i))).count();
        out.headings.push((title,y,unlocked,count));y+=40.0;
        for i in 0..count {out.items.push((make(i),[(i%columns) as f32*(w+8.0),y+(i/columns) as f32*(h+8.0),w,h]));}
        y+=count.div_ceil(columns) as f32*(h+8.0)+18.0;
    }
    out.height=y;out
}
/// 方向键在图鉴里移动：只看大致在该方向上的条目（主轴位移不小于副轴的一半），取主轴最近、副轴偏差加倍计的那个
fn step(items:&[(Item,[f32;4])],from:usize,dx:f32,dy:f32)->usize {
    let center=|r:&[f32;4]|[r[0]+r[2]*0.5,r[1]+r[3]*0.5];
    let Some((_,rect))=items.get(from) else{return 0};let c=center(rect);
    items.iter().enumerate().filter(|(i,_)|*i!=from).filter_map(|(i,(_,r))|{
        let d=[center(r)[0]-c[0],center(r)[1]-c[1]];
        let (main,side)=if dx!=0.0 {(d[0]*dx,d[1].abs())}else{(d[1]*dy,d[0].abs())};
        (main>1.0&&main*2.0>=side).then_some((i,main+side*2.0))
    }).min_by(|a,b|a.1.total_cmp(&b.1)).map_or(from,|(i,_)|i)
}
pub fn draw(ui:&Ui,archive:&mut Archive,icon:&mut dyn FnMut(u32)->Option<TextureId>)->Option<ArchiveAction> {
    let size=ui.io().display_size;let s=(size[0]/1600.0).min(size[1]/900.0);
    let (ox,oy)=((size[0]-1600.0*s)*0.5,(size[1]-900.0*s)*0.5);let p=|x:f32,y:f32|[ox+x*s,oy+y*s];
    let background=ui.get_background_draw_list();crate::oath_art::background(&background,size);
    let shade=|a:u8|ImColor32::from_rgba(0,0,0,a);
    background.add_rect_filled_multicolor([0.0,0.0],size,shade(90),shade(90),shade(170),shade(170));drop(background);
    let opened=*archive.opened.get_or_insert(ui.time());let ready=ui.time()-opened>=0.3;
    let pressed=|keys:&[Key]|ready&&!ui.io().want_text_input&&keys.iter().any(|k|ui.is_key_pressed(*k));
    let mut action=None;
    if pressed(&[Key::Escape,Key::GamepadFaceRight,crate::options::key(erdtree_tower::options::Hotkey::Archive),Key::GamepadBack]) {action=Some(ArchiveAction::Close);}
    if pressed(&[Key::E,Key::GamepadR1]) {archive.tab=(archive.tab+1)%TABS.len();archive.follow=true;}
    if pressed(&[Key::Q,Key::GamepadL1]) {archive.tab=(archive.tab+TABS.len()-1)%TABS.len();archive.follow=true;}
    let layout=layout(archive);
    if archive.tab==0 {
        let count=archive.records.len();
        if count>0 {
            if pressed(&[Key::UpArrow,Key::W,Key::GamepadDpadUp]) {archive.selected=archive.selected.saturating_sub(1);archive.follow=true;}
            if pressed(&[Key::DownArrow,Key::S,Key::GamepadDpadDown]) {archive.selected=(archive.selected+1).min(count-1);archive.follow=true;}
            if pressed(&[Key::Enter,Key::GamepadFaceDown]) {action=Some(ArchiveAction::Open(archive.records[archive.selected].clone()));}
        }
    } else if archive.tab==2 {
        legacy_keys(archive,&pressed,ui.time());
    } else if archive.tab==3 {
        let count=erdtree_tower::titles::TITLES.len();
        if pressed(&[Key::UpArrow,Key::W,Key::GamepadDpadUp]) {archive.title_row=archive.title_row.saturating_sub(1);}
        if pressed(&[Key::DownArrow,Key::S,Key::GamepadDpadDown]) {archive.title_row=(archive.title_row+1).min(count-1);}
        if pressed(&[Key::Enter,Key::GamepadFaceDown]) {wear(archive,archive.title_row,ui.time());}
    } else if archive.tab==4 {
        let count=erdtree_tower::auras::AURAS.len()+1;
        if pressed(&[Key::UpArrow,Key::W,Key::GamepadDpadUp]) {archive.aura_row=archive.aura_row.saturating_sub(1);}
        if pressed(&[Key::DownArrow,Key::S,Key::GamepadDpadDown]) {archive.aura_row=(archive.aura_row+1).min(count-1);}
        if pressed(&[Key::Enter,Key::GamepadFaceDown]) {wear_aura(archive,archive.aura_row,ui.time());}
    } else if archive.tab==5 {
        if pressed(&[Key::PageUp,Key::GamepadL2]) {set_weapon_group(archive,(archive.weapon_group+7)%8);}
        if pressed(&[Key::PageDown,Key::GamepadR2]) {set_weapon_group(archive,(archive.weapon_group+1)%8);}
        let rows=weapon_rows(archive.weapon_group);let mut index=rows.iter().position(|r|*r==archive.weapon_row).unwrap_or(0);
        if pressed(&[Key::LeftArrow,Key::A,Key::GamepadDpadLeft]) {index=index.saturating_sub(1);}
        if pressed(&[Key::RightArrow,Key::D,Key::GamepadDpadRight]) {index=(index+1).min(rows.len()-1);}
        if pressed(&[Key::UpArrow,Key::W,Key::GamepadDpadUp]) {index=index.saturating_sub(3);}
        if pressed(&[Key::DownArrow,Key::S,Key::GamepadDpadDown]) {index=(index+3).min(rows.len()-1);}
        archive.weapon_row=rows[index];
        if pressed(&[Key::Enter,Key::GamepadFaceDown]) {train_weapon(archive,false,ui.time());}
        if pressed(&[Key::Space,Key::GamepadFaceUp]) {train_weapon(archive,true,ui.time());}
    } else if archive.tab==6 {
        // Appearance navigation and search are handled inside its panel.
    } else {
        for (keys,dx,dy) in [(&[Key::LeftArrow,Key::A,Key::GamepadDpadLeft][..],-1.0,0.0),(&[Key::RightArrow,Key::D,Key::GamepadDpadRight][..],1.0,0.0),
            (&[Key::UpArrow,Key::W,Key::GamepadDpadUp][..],0.0,-1.0),(&[Key::DownArrow,Key::S,Key::GamepadDpadDown][..],0.0,1.0)] {
            if pressed(keys) {archive.focus=step(&layout.items,archive.focus,dx,dy);archive.follow=true;}
        }
    }
    let moved=ui.io().mouse_delta!=[0.0,0.0];
    ui.window("旅程档案##archive").position([0.0,0.0],Condition::Always).size(size,Condition::Always)
        .flags(WindowFlags::NO_DECORATION|WindowFlags::NO_BACKGROUND|WindowFlags::NO_MOVE|WindowFlags::NO_NAV|WindowFlags::NO_SCROLL_WITH_MOUSE).build(|| {
        let text=|x:f32,y:f32,value:&str,color:[f32;4],scale:f32,align:f32| {
            ui.set_window_font_scale(scale*s);ui.set_cursor_screen_pos([ox+x*s-ui.calc_text_size(value)[0]*align,oy+y*s]);ui.text_colored(color,value);
        };
        {
            let title=crate::fancy_text::title_font().map(|font|ui.push_font(font));
            text(800.0,26.0,"旅 程 档 案",GOLD,if title.is_some(){1.9*20.0/32.0}else{1.9},0.5);
        }
        // 分页标签
        for (i,label) in TABS.into_iter().enumerate() {
            let x=800.0-(TABS.len()-1) as f32*80.0+i as f32*160.0;let active=archive.tab==i;
            ui.set_cursor_screen_pos(p(x-80.0,92.0));ui.set_window_font_scale(s);
            if ui.invisible_button(format!("tab-{i}"),[160.0*s,40.0*s])&&ready {archive.tab=i;archive.follow=true;}
            let hover=ui.is_item_hovered();
            text(x,98.0,label,if active{GOLD}else if hover{WHITE}else{MUTED},1.1,0.5);
            if active {ui.get_window_draw_list().add_rect(p(x-46.0,130.0),p(x+46.0,132.0),ImColor32::from_rgba(214,180,112,230)).filled(true).build();}
        }
        let draw=ui.get_window_draw_list();ornament(&draw,p(800.0,150.0),420.0*s,s);drop(draw);
        match archive.tab {0=>history(ui,archive,icon,&mut action,moved,ready,s,ox,oy),1=>codex(ui,archive,&layout,icon,moved,s,ox,oy),2=>legacy(ui,archive,icon,moved,ready,s,ox,oy),3=>titles(ui,archive,moved,ready,s,ox,oy),4=>auras(ui,archive,moved,ready,s,ox,oy),5=>weapons(ui,archive,icon,ready,s,ox,oy),_=>crate::transmog_ui::draw(ui,&mut archive.appearance,icon,ready,s,ox,oy)}
        archive.follow=false;
        // 底部：操作提示与返回
        use crate::pad::{self,Input,Pad};
        let pages=Input::new(Pad::Triggers,"PgUp / PgDn");
        let hint:&[(Option<Input>,&str)]=match archive.tab {
            0=>&[(Some(pad::MOVE_Y),"选择"),(Some(pad::CONFIRM),"查看结算"),(Some(pad::TABS),"切换分页")],
            1=>&[(Some(pad::MOVE),"选择"),(None,"鼠标悬停查看"),(Some(pad::TABS),"切换分页")],
            2=>&[(Some(pad::MOVE_Y),"选择"),(Some(Input::new(Pad::DpadX,"→")),"护符列表"),(Some(pad::CONFIRM),"选用"),(None,"每个职业单独计算，下一局生效")],
            3=>&[(Some(pad::MOVE_Y),"选择"),(Some(pad::CONFIRM),"佩戴 / 摘下"),(None,"联机时队友头顶的名牌会显示你的称号")],
            4=>&[(Some(pad::MOVE_Y),"选择"),(Some(pad::CONFIRM),"换上 / 摘下"),(None,"只有光效、不加数值，联机时队友也看得到")],
            5=>&[(Some(pages),"分类"),(Some(pad::MOVE),"选择"),(Some(pad::CONFIRM),"开关"),(Some(Input::new(Pad::Y,"Space")),"升级")],
            _=>&[(Some(pages),"翻页"),(None,"点击防具立即幻化 · 外观按角色保存")]};
        pad::hints(ui,p(60.0,856.0),0.0,hint,22.0*s,15.0*s,DIM,pad::Tone::Light);
        let pos=p(1220.0,822.0);let extent=[320.0*s,60.0*s];
        ui.set_cursor_screen_pos(pos);ui.set_window_font_scale(s);
        if ui.invisible_button("archive-close",extent)&&ready {action=Some(ArchiveAction::Close);}
        crate::oath_art::button(&ui.get_window_draw_list(),pos,[pos[0]+extent[0],pos[1]+extent[1]],if ui.is_item_hovered(){255}else{215});
        crate::pad::label_in(ui,pos,[pos[0]+extent[0],pos[1]+extent[1]],Some(crate::pad::BACK),"返回",GOLD,s,crate::pad::Tone::Light);
    });
    action
}
#[allow(clippy::too_many_arguments)]
fn history(ui:&Ui,archive:&mut Archive,icon:&mut dyn FnMut(u32)->Option<TextureId>,action:&mut Option<ArchiveAction>,moved:bool,ready:bool,s:f32,ox:f32,oy:f32) {
    let p=|x:f32,y:f32|[ox+x*s,oy+y*s];
    let text=|pos:[f32;2],value:&str,color:[f32;4],scale:f32,align:f32| {
        ui.set_window_font_scale(scale*s);ui.set_cursor_screen_pos([pos[0]-ui.calc_text_size(value)[0]*align,pos[1]]);ui.text_colored(color,value);
    };
    let codex=&archive.codex;
    let summary=if codex.runs==0 {"还没有完整的旅程记录".to_string()} else {
        format!("共 {} 局  ·  登顶 {} 次  ·  最远 第 {} 幕第 {} 层{}",codex.runs,codex.wins,codex.deepest.0,codex.deepest.1,if codex.best_law>0{format!("  ·  律法最高 {} 分",codex.best_law)}else{String::new()})
    };
    text(p(800.0,172.0),&summary,MUTED,0.9,0.5);
    if archive.records.is_empty() {text(p(800.0,460.0),"结束一局旅程后，记录会出现在这里",DIM,1.0,0.5);return;}
    ui.set_cursor_screen_pos(p(200.0,210.0));
    let _padding=ui.push_style_var(StyleVar::WindowPadding([0.0,0.0]));
    ui.child_window("archive-history").size([1200.0*s,590.0*s]).build(|| {
        let origin=ui.cursor_screen_pos();let (row,gap)=(76.0*s,8.0*s);
        if archive.follow {
            let (top,bottom)=(archive.selected as f32*(row+gap),archive.selected as f32*(row+gap)+row);let (scroll,view)=(ui.scroll_y(),ui.window_size()[1]);
            if top<scroll {ui.set_scroll_y(top);} else if bottom>scroll+view {ui.set_scroll_y(bottom-view);}
        }
        let width=ui.window_size()[0]-14.0*s;
        for (i,record) in archive.records.iter().enumerate() {
            let min=[origin[0],origin[1]+i as f32*(row+gap)];let max=[min[0]+width,min[1]+row];
            ui.set_cursor_screen_pos(min);ui.set_window_font_scale(s);
            let clicked=ui.invisible_button(format!("record-{i}"),[width,row]);
            let hover=ui.is_item_hovered();
            if hover&&moved {archive.selected=i;}
            if clicked&&ready {*action=Some(ArchiveAction::Open(record.clone()));}
            let selected=archive.selected==i;
            let draw=ui.get_window_draw_list();
            draw.add_rect(min,max,if selected{ImColor32::from_rgba(46,38,25,238)}else if hover{ImColor32::from_rgba(32,27,19,226)}else{ImColor32::from_rgba(18,16,12,214)}).filled(true).rounding(4.0*s).build();
            draw.add_rect(min,max,if selected{ImColor32::from_rgba(214,180,112,220)}else{ImColor32::from_rgba(171,143,87,60)}).rounding(4.0*s).thickness(if selected{1.6*s}else{s}).build();
            let (label,color)=match record.result {Outcome::Won=>("登顶",(214,172,96)),Outcome::Dead=>("倒下",(204,92,72)),Outcome::Ended=>("暂离",(140,130,110))};
            let (pill_min,pill_max)=([min[0]+18.0*s,min[1]+22.0*s],[min[0]+82.0*s,min[1]+54.0*s]);
            draw.add_rect(pill_min,pill_max,ImColor32::from_rgba(color.0,color.1,color.2,50)).filled(true).rounding(16.0*s).build();
            draw.add_rect(pill_min,pill_max,ImColor32::from_rgba(color.0,color.1,color.2,220)).rounding(16.0*s).thickness(1.3*s).build();
            drop(draw);
            text([min[0]+50.0*s,min[1]+26.0*s],label,[f32::from(color.0)/255.0,f32::from(color.1)/255.0,f32::from(color.2)/255.0,1.0],0.92,0.5);
            text([min[0]+104.0*s,min[1]+12.0*s],&format!("{}  ·  第 {} 幕第 {} 层",crate::map_ui::class_name(record.class),record.act,record.layer),WHITE,1.0,0.0);
            let law=if record.law.is_empty(){"无苦难律法".to_string()}else{format!("律法 {} 分 · {}",record.law.score(),record.law.grade().unwrap_or("普通"))};
            text([min[0]+104.0*s,min[1]+44.0*s],&format!("{law}  ·  种子 {}",record.seed),MUTED,0.82,0.0);
            // 誓约名与余烬徽记
            let oaths:Vec<_>=record.oaths.iter().map(|o|format!("{} {}",o.kind.name(),ROMAN[usize::from(o.level.min(3))]).trim().to_owned()).collect();
            let mut line=if oaths.is_empty(){"没有誓约".to_string()}else{oaths.join(" · ")};
            ui.set_window_font_scale(0.84*s);
            while ui.calc_text_size(&line)[0]>380.0*s&&line.chars().count()>4 {let keep=line.chars().count()-2;line=line.chars().take(keep).collect::<String>();line.push('…');line=line.replace("……","…");}
            text([min[0]+520.0*s,min[1]+12.0*s],&line,if oaths.is_empty(){DIM}else{BOON},0.84,0.0);
            for (n,ember) in record.embers.iter().take(10).enumerate() {
                let at=[min[0]+(520.0+n as f32*30.0)*s,min[1]+42.0*s];
                if let Some(texture)=crate::reward_ui::ember_icon(*ember).and_then(&mut *icon) {ui.get_window_draw_list().add_image(texture,at,[at[0]+26.0*s,at[1]+26.0*s]).build();}
            }
            if record.embers.is_empty() {text([min[0]+520.0*s,min[1]+46.0*s],"没有余烬",DIM,0.8,0.0);}
            let right=max[0]-18.0*s;
            let mut when=ago(archive.now,record.time);if record.debug {when=format!("调试  ·  {when}");}
            text([right,min[1]+12.0*s],&when,MUTED,0.82,1.0);
            let fatal=journal::review(record).fatal;
            let (verdict,color)=match record.result {
                Outcome::Won=>("三幕全部通关".to_string(),GOLD),
                Outcome::Dead=>(fatal.and_then(|f|f.enemies.first().cloned()).map_or("倒在途中".into(),|name|format!("倒在 {name} 面前")),RED),
                Outcome::Ended=>("中途结算".into(),MUTED),
            };
            text([right,min[1]+44.0*s],&verdict,color,0.82,1.0);
        }
        ui.set_cursor_screen_pos([origin[0],origin[1]+archive.records.len() as f32*(row+gap)]);ui.dummy([1.0,1.0]);
        // 鼠标滚轮之外，手柄右摇杆在其他地方时也滚动
        let wheel=ui.io().mouse_wheel;
        if wheel!=0.0&&!ui.is_window_hovered() {ui.set_scroll_y((ui.scroll_y()-wheel*60.0*s).clamp(0.0,ui.scroll_max_y()));}
    });
}
#[allow(clippy::too_many_arguments)]
fn codex(ui:&Ui,archive:&mut Archive,layout:&Layout,icon:&mut dyn FnMut(u32)->Option<TextureId>,moved:bool,s:f32,ox:f32,oy:f32) {
    let p=|x:f32,y:f32|[ox+x*s,oy+y*s];
    let text=|pos:[f32;2],value:&str,color:[f32;4],scale:f32,align:f32| {
        ui.set_window_font_scale(scale*s);ui.set_cursor_screen_pos([pos[0]-ui.calc_text_size(value)[0]*align,pos[1]]);ui.text_colored(color,value);
    };
    archive.focus=archive.focus.min(layout.items.len().saturating_sub(1));
    ui.set_cursor_screen_pos(p(70.0,176.0));
    let _padding=ui.push_style_var(StyleVar::WindowPadding([0.0,0.0]));
    ui.child_window("archive-codex").size([1014.0*s,624.0*s]).build(|| {
        let origin=ui.cursor_screen_pos();let at=|x:f32,y:f32|[origin[0]+x*s,origin[1]+y*s];
        if archive.follow && let Some((_,r))=layout.items.get(archive.focus) {
            let (top,bottom,scroll,view)=((r[1]-44.0)*s,(r[1]+r[3]+12.0)*s,ui.scroll_y(),ui.window_size()[1]);
            if top<scroll {ui.set_scroll_y(top.max(0.0));} else if bottom>scroll+view {ui.set_scroll_y(bottom-view);}
        }
        for (title,y,unlocked,total) in &layout.headings {
            text(at(0.0,*y+6.0),title,GOLD,1.05,0.0);
            ui.same_line();ui.set_window_font_scale(0.82*s);ui.text_colored(MUTED,format!("  {unlocked} / {total}"));
            let end=ui.item_rect_max();
            ui.get_window_draw_list().add_rect_filled_multicolor([end[0]+12.0*s,end[1]-9.0*s],[origin[0]+996.0*s,end[1]-8.0*s],
                ImColor32::from_rgba(214,180,112,150),ImColor32::from_rgba(214,180,112,0),ImColor32::from_rgba(214,180,112,0),ImColor32::from_rgba(214,180,112,150));
        }
        for (index,(item,r)) in layout.items.iter().enumerate() {
            let (min,max)=(at(r[0],r[1]),at(r[0]+r[2],r[1]+r[3]));
            let lit=archive.unlocked(*item);let focused=index==archive.focus;
            if moved&&ui.is_window_hovered()&&ui.is_mouse_hovering_rect(min,max) {archive.focus=index;}
            let draw=ui.get_window_draw_list();
            draw.add_rect(min,max,if focused{ImColor32::from_rgba(46,38,25,238)}else if lit{ImColor32::from_rgba(24,21,15,220)}else{ImColor32::from_rgba(14,13,11,210)}).filled(true).rounding(4.0*s).build();
            draw.add_rect(min,max,if focused{ImColor32::from_rgba(236,200,128,235)}else if lit{ImColor32::from_rgba(171,143,87,90)}else{ImColor32::from_rgba(90,80,62,70)}).rounding(4.0*s).thickness(if focused{1.8*s}else{s}).build();
            drop(draw);
            let name_color=if lit{WHITE}else{DIM};
            match *item {
                Item::Oath(i)=>{
                    let (kind,taken,best)=archive.codex.oaths[i];
                    oath_badge(ui,at(r[0]+30.0,r[1]+32.0),17.0*s,if lit{ROMAN[usize::from(best.min(3))]}else{""},lit,s);
                    text(at(r[0]+58.0,r[1]+10.0),if lit{kind.name()}else{"？？？"},name_color,0.95,0.0);
                    text(at(r[0]+58.0,r[1]+36.0),&if lit{format!("立誓 {taken} 次")}else{"未解锁".into()},MUTED,0.76,0.0);
                },
                Item::Ember(i)=>{
                    let (ember,taken)=archive.codex.embers[i];let cx=r[0]+r[2]*0.5;
                    if let Some(texture)=crate::reward_ui::ember_icon(ember).and_then(&mut *icon) {
                        let tint=if lit{255}else{38};
                        ui.get_window_draw_list().add_image(texture,at(cx-32.0,r[1]+8.0),at(cx+32.0,r[1]+72.0)).col(ImColor32::from_rgba(tint,tint,tint,255)).build();
                    }
                    let name=if lit{ember.name()}else{"？？？"};ui.set_window_font_scale(0.84*s);
                    let fit=((r[2]-10.0)*s/ui.calc_text_size(name)[0]).min(1.0);
                    text(at(cx,r[1]+76.0),name,name_color,0.84*fit,0.5);
                    text(at(cx,r[1]+97.0),&if lit{format!("获得 {taken} 次")}else{"未解锁".into()},MUTED,0.74,0.5);
                },
                Item::Boss(i)=>{
                    let (act,name)=&archive.bosses[i];
                    let stats=archive.codex.boss(name);
                    let label=if lit{name.as_str()}else{"？？？"};ui.set_window_font_scale(0.9*s);
                    let fit=((r[2]-20.0)*s/ui.calc_text_size(label)[0]).min(1.0);
                    text(at(r[0]+12.0,r[1]+7.0),label,name_color,0.9*fit,0.0);
                    let act=if *act>0{format!("第 {act} 幕")}else{String::new()};
                    let line=stats.map_or(if act.is_empty(){"未遇见".into()}else{format!("{act} · 未遇见")},|(seen,won)|format!("{act}{}击败 {won} / 交手 {seen}",if act.is_empty(){""}else{" · "}));
                    text(at(r[0]+12.0,r[1]+31.0),&line,MUTED,0.74,0.0);
                },
                Item::Affix(i)=>{
                    let name=&archive.affixes[i];let runs=archive.codex.affix(name);
                    text(at(r[0]+12.0,r[1]+12.0),if lit{name}else{"？？？"},name_color,0.88,0.0);
                    text(at(r[0]+r[2]-12.0,r[1]+15.0),&if lit{format!("{runs} 局")}else{"未获得".into()},MUTED,0.74,1.0);
                },
            }
        }
        ui.set_cursor_screen_pos(at(0.0,layout.height));ui.dummy([1.0,1.0]);
        let wheel=ui.io().mouse_wheel;
        if wheel!=0.0&&!ui.is_window_hovered() {ui.set_scroll_y((ui.scroll_y()-wheel*60.0*s).clamp(0.0,ui.scroll_max_y()));}
    });
    // 右侧：总览与选中条目的详情
    let draw=ui.get_window_draw_list();panel(&draw,p(1104.0,176.0),p(1530.0,800.0),s);drop(draw);
    let c=&archive.codex;
    text(p(1130.0,196.0),&format!("旅程 {} 局  ·  登顶 {} 次",c.runs,c.wins),WHITE,0.95,0.0);
    let deepest=if c.runs==0{"尚无记录".to_string()}else{format!("最远 第 {} 幕第 {} 层{}",c.deepest.0,c.deepest.1,if c.best_law>0{format!(" · 律法最高 {} 分",c.best_law)}else{String::new()})};
    text(p(1130.0,226.0),&deepest,MUTED,0.84,0.0);
    let draw=ui.get_window_draw_list();ornament(&draw,p(1317.0,268.0),180.0*s,s);drop(draw);
    let right=ox+1508.0*s;
    let Some(&(item,_))=layout.items.get(archive.focus) else{return};
    let lit=archive.unlocked(item);
    let hint=|ui:&Ui,y:f32,unlock:&str| {
        text(p(1130.0,y),"尚未解锁",DIM,1.05,0.0);
        wrapped(ui,p(1130.0,y+36.0),right,unlock,MUTED,0.86*s);
    };
    match item {
        Item::Oath(i)=>{
            let (kind,taken,best)=c.oaths[i];
            oath_badge(ui,p(1160.0,320.0),26.0*s,if lit{ROMAN[usize::from(best.min(3))]}else{""},lit,s);
            if !lit {hint(ui,300.0,"在幕间立下这条誓约后解锁");return;}
            text(p(1202.0,300.0),kind.name(),GOLD,1.2,0.0);
            text(p(1202.0,334.0),&format!("最高等级 {} · 立誓 {taken} 次",ROMAN[usize::from(best.min(3))]),MUTED,0.84,0.0);
            let y=wrapped(ui,p(1130.0,380.0),right,&kind.boon_at(best.max(1)).replace('\n',"，"),BOON,0.92*s);
            wrapped(ui,[ox+1130.0*s,y+6.0*s],right,&format!("代价：{}",kind.bane_at()),RED,0.92*s);
        },
        Item::Ember(i)=>{
            let (ember,taken)=c.embers[i];
            if let Some(texture)=crate::reward_ui::ember_icon(ember).and_then(&mut *icon) {
                let tint=if lit{255}else{38};
                ui.get_window_draw_list().add_image(texture,p(1130.0,292.0),p(1226.0,388.0)).col(ImColor32::from_rgba(tint,tint,tint,255)).build();
            }
            if !lit {hint(ui,410.0,"在普通战或精英战的奖励里选择这枚余烬后解锁");return;}
            text(p(1244.0,312.0),ember.name(),GOLD,1.15,0.0);
            text(p(1244.0,348.0),&format!("获得 {taken} 次"),MUTED,0.84,0.0);
            wrapped(ui,p(1130.0,410.0),right,&ember.effect().replace('\n',"，"),BOON,0.92*s);
        },
        Item::Boss(i)=>{
            let (act,name)=&archive.bosses[i];
            let Some((seen,won))=c.boss(name) else{hint(ui,300.0,"在 Boss 战中遇到后解锁");return;};
            text(p(1130.0,300.0),name,GOLD,1.15,0.0);
            if *act>0 {text(p(1130.0,338.0),&format!("第 {act} 幕 Boss"),MUTED,0.84,0.0);}
            text(p(1130.0,380.0),&format!("交手 {seen} 次  ·  击败 {won} 次"),WHITE,0.95,0.0);
            text(p(1130.0,412.0),&format!("胜率 {}%",won*100/seen.max(1)),BOON,0.9,0.0);
        },
        Item::Affix(i)=>{
            let name=&archive.affixes[i];let runs=c.affix(name);
            if !lit {hint(ui,300.0,"获得带有这条词条的武器或防具后解锁");return;}
            text(p(1130.0,300.0),name,GOLD,1.15,0.0);
            text(p(1130.0,338.0),&format!("出现在 {runs} 局旅程中"),MUTED,0.84,0.0);
            wrapped(ui,p(1130.0,380.0),right,"武器与防具的前缀决定品质与词条数量：品质越高，词条越多、数值越大。",BOON,0.88*s);
        },
    }
    text(p(1130.0,764.0),"只统计非调试旅程，整局结束后计入",DIM,0.76,0.0);
}

const TABS:[&str;7]=["往期旅程","图鉴","传承","称号","光环","武器偏好","防具幻化"];
const NO_AURA:erdtree_tower::auras::Aura=erdtree_tower::auras::Aura{id:0,name:"无光环",vfx:-1,source:"无特效",unlock:"默认设置",color:0xB0AAA0};
fn aura_choice(row:usize)->Option<&'static erdtree_tower::auras::Aura> {if row==0 {Some(&NO_AURA)}else{erdtree_tower::auras::AURAS.get(row-1)}}
/// 换上第 row 个光环；已经挂着时摘下
fn wear_aura(archive:&mut Archive,row:usize,now:f64) {
    let Some(aura)=aura_choice(row) else{return;};
    let Some((owner,_))=archive.legacy else{archive.notice=Some(("载入角色后才能更换光环".into(),now));return;};
    if aura.id!=0&&!archive.auras.contains(&aura.id) {archive.notice=Some((format!("尚未解锁：{}",aura.unlock),now));return;}
    let id=if crate::auras::worn()==aura.id {0}else{aura.id};
    archive.notice=Some((match crate::auras::choose(owner,id) {Ok(())=>if id==0 {"已摘下光环".into()}else{format!("已换上「{}」",aura.name)},Err(error)=>error},now));
}
#[allow(clippy::too_many_arguments)]
fn auras(ui:&Ui,archive:&mut Archive,moved:bool,ready:bool,s:f32,ox:f32,oy:f32) {
    let p=|x:f32,y:f32|[ox+x*s,oy+y*s];
    let text=|x:f32,y:f32,value:&str,color:[f32;4],scale:f32,align:f32| {
        ui.set_window_font_scale(scale*s);ui.set_cursor_screen_pos([ox+x*s-ui.calc_text_size(value)[0]*align,oy+y*s]);ui.text_colored(color,value);
    };
    let worn=crate::auras::worn();
    text(800.0,176.0,&format!("已解锁 {} / {}",archive.auras.len(),erdtree_tower::auras::AURAS.len()),MUTED,0.9,0.5);
    for (row,aura) in std::iter::once(&NO_AURA).chain(erdtree_tower::auras::AURAS.iter()).enumerate() {
        let (x,y)=(400.0,214.0+row as f32*56.0);let lit=aura.id==0||archive.auras.contains(&aura.id);
        ui.set_cursor_screen_pos(p(x,y));ui.set_window_font_scale(s);
        if ui.invisible_button(format!("aura-{row}"),[800.0*s,50.0*s])&&ready {archive.aura_row=row;wear_aura(archive,row,ui.time());}
        if ui.is_item_hovered()&&moved {archive.aura_row=row;}
        let focus=archive.aura_row==row;let on=worn==aura.id;
        let draw=ui.get_window_draw_list();
        draw.add_rect(p(x,y),p(x+800.0,y+50.0),ImColor32::from_rgba(24,20,16,if focus{220}else{150})).filled(true).rounding(6.0*s).build();
        if focus||on {draw.add_rect(p(x,y),p(x+800.0,y+50.0),ImColor32::from_rgba(214,180,112,if on{240}else{140})).rounding(6.0*s).thickness(if on{2.5}else{1.5}*s).build();}
        let c=aura.color;let (r,g,b)=((c>>16) as u8,(c>>8) as u8,c as u8);
        let pulse=if aura.id==0 {0.6}else if lit {0.6+0.4*(ui.time() as f32*2.2+row as f32).sin().abs()}else{0.25};
        draw.add_circle(p(x+34.0,y+28.0),15.0*s,ImColor32::from_rgba(r,g,b,(70.0*pulse) as u8)).filled(true).build();
        draw.add_circle(p(x+34.0,y+28.0),8.0*s,ImColor32::from_rgba(r,g,b,(255.0*pulse) as u8)).filled(true).build();
        drop(draw);
        let color=if lit{[f32::from(r)/255.0,f32::from(g)/255.0,f32::from(b)/255.0,1.0]}else{DIM};
        text(x+64.0,y+8.0,if lit{aura.name}else{"？？？"},color,1.05,0.0);
        text(x+64.0,y+32.0,&if aura.id==0 {"无身体光效、无粒子、无地面法阵".into()}else{format!("原版光效：{}",aura.source)},MUTED,0.72,0.0);
        text(x+330.0,y+18.0,aura.unlock,if lit{MUTED}else{DIM},0.8,0.0);
        if on {text(x+770.0,y+16.0,if aura.id==0 {"已选用"}else{"已换上"},GOLD,0.95,1.0);}
    }
    if let Some((notice,at))=&archive.notice {if ui.time()-at<3.0 {text(800.0,800.0,notice,GOLD,0.9,0.5);}}
}
/// 称号在自绘界面里的样子（与原版名牌上的 HUD 特效同色）
pub fn title_style(style:erdtree_tower::titles::Style)->crate::fancy_text::Style {
    use crate::fancy_text::Effect;use erdtree_tower::titles::Style as T;
    let (color,accent)=style.colors();let rgb=|c:u32|[(c>>16) as u8,(c>>8) as u8,c as u8];
    let effect=match style {T::Ash|T::Frost=>Effect::Pulse,T::Gold|T::Moon=>Effect::Shimmer,T::Ember=>Effect::Wave,T::Blood=>Effect::Jitter,T::Star=>Effect::Gradient};
    crate::fancy_text::Style{color:rgb(color),accent:rgb(accent),effect,glow:style!=T::Ash}
}
/// 戴上第 row 个称号；已经戴着时摘下
fn wear(archive:&mut Archive,row:usize,now:f64) {
    let title=&erdtree_tower::titles::TITLES[row];
    let Some((owner,_))=archive.legacy else{archive.notice=Some(("载入角色后才能佩戴称号".into(),now));return;};
    if !archive.titles.contains(&title.id) {archive.notice=Some((format!("尚未解锁：{}",title.unlock),now));return;}
    let id=if crate::name_tags::worn()==title.id {0}else{title.id};
    archive.notice=Some((match crate::name_tags::choose(owner,id) {Ok(())=>if id==0 {"已摘下称号".into()}else{format!("已佩戴「{}」",title.text)},Err(error)=>error},now));
}
#[allow(clippy::too_many_arguments)]
fn titles(ui:&Ui,archive:&mut Archive,moved:bool,ready:bool,s:f32,ox:f32,oy:f32) {
    let p=|x:f32,y:f32|[ox+x*s,oy+y*s];
    let text=|x:f32,y:f32,value:&str,color:[f32;4],scale:f32,align:f32| {
        ui.set_window_font_scale(scale*s);ui.set_cursor_screen_pos([ox+x*s-ui.calc_text_size(value)[0]*align,oy+y*s]);ui.text_colored(color,value);
    };
    let worn=crate::name_tags::worn();
    text(800.0,176.0,&format!("已解锁 {} / {}",archive.titles.len(),erdtree_tower::titles::TITLES.len()),MUTED,0.9,0.5);
    for (row,title) in erdtree_tower::titles::TITLES.iter().enumerate() {
        let (x,y)=(400.0,220.0+row as f32*78.0);let lit=archive.titles.contains(&title.id);
        ui.set_cursor_screen_pos(p(x,y));ui.set_window_font_scale(s);
        if ui.invisible_button(format!("title-{row}"),[800.0*s,68.0*s])&&ready {archive.title_row=row;wear(archive,row,ui.time());}
        if ui.is_item_hovered()&&moved {archive.title_row=row;}
        let focus=archive.title_row==row;
        let draw=ui.get_window_draw_list();
        draw.add_rect(p(x,y),p(x+800.0,y+68.0),ImColor32::from_rgba(24,20,16,if focus{220}else{150})).filled(true).rounding(6.0*s).build();
        if focus||worn==title.id {draw.add_rect(p(x,y),p(x+800.0,y+68.0),ImColor32::from_rgba(214,180,112,if worn==title.id{240}else{140})).rounding(6.0*s).thickness(if worn==title.id{2.5}else{1.5}*s).build();}
        drop(draw);
        if lit {crate::fancy_text::draw(crate::fancy_text::Layer::Window,p(x+28.0,y+12.0),30.0*s,title.text,&title_style(title.style),ui.time() as f32,1.0);}
        else {text(x+28.0,y+14.0,"？？？",DIM,1.2,0.0);}
        text(x+260.0,y+12.0,if lit{title.style.name()}else{"未解锁"},if lit{WHITE}else{DIM},0.9,0.0);
        text(x+260.0,y+38.0,title.unlock,MUTED,0.8,0.0);
        if worn==title.id {text(x+770.0,y+22.0,"佩戴中",GOLD,0.95,1.0);}
    }
    if let Some((notice,at))=&archive.notice {if ui.time()-at<3.0 {text(800.0,780.0,notice,GOLD,0.9,0.5);}}
}
/// 传承页的按键：职业列表上下切换职业，右移进入护符列表，上下选择、A 选用
use crate::weapon_ui::{WeaponAction,weapon_rows};
#[cfg(test)]
use crate::weapon_ui::WEAPON_GROUPS;
fn set_weapon_group(archive:&mut Archive,group:usize) {archive.weapon_group=group;archive.weapon_row=weapon_rows(group)[0];}
fn train_weapon(archive:&mut Archive,upgrade:bool,now:f64) {
    let Some((owner,_))=archive.legacy.as_ref() else {archive.notice=Some(("请先载入角色".into(),now));return;};
    let owner=*owner;let mut data=crate::legacy::load(owner);
    let selected=archive.weapon_row.checked_sub(1).and_then(|i|erdtree_tower::weapon_training::TYPES.get(i)).map(|&(id,_)|id);
    let result=if upgrade {selected.ok_or("请先选择要升级的武器类型").and_then(|kind|data.upgrade_training(kind))}
        else if selected.is_some_and(|kind|data.training.level_for(kind)==0){Err("请先升级此类武器研习")}
        else{data.training.choose(selected)};
    let result=result.map_err(str::to_owned).and_then(|_|crate::legacy::save(owner,&data));
    let message=match result {
        Ok(())=>{archive.legacy=Some((owner,data));if upgrade {"升级成功，下一局生效"}else{"偏好已保存，下一局生效"}.to_string()},
        Err(error)=>error,
    };
    archive.notice=Some((message,now));
}
#[allow(clippy::too_many_arguments)]
fn weapons(ui:&Ui,archive:&mut Archive,icon:&mut dyn FnMut(u32)->Option<TextureId>,ready:bool,s:f32,ox:f32,oy:f32) {
    let Some((_,data))=archive.legacy.clone() else {
        ui.set_window_font_scale(s);ui.set_cursor_screen_pos([ox+600.0*s,oy+430.0*s]);ui.text_colored(DIM,"载入角色后才能设置武器偏好");return;
    };
    let now=ui.time();
    let notice=archive.notice.as_ref().filter(|(_,at)|now-at<4.0).map(|(message,_)|message.clone());
    match crate::weapon_ui::draw(ui,&data,archive.weapon_group,archive.weapon_row,notice.as_deref(),icon,ready,s,ox,oy) {
        Some(WeaponAction::Group(group))=>set_weapon_group(archive,group),
        Some(WeaponAction::Row(row))=>archive.weapon_row=row,
        Some(WeaponAction::Upgrade)=>train_weapon(archive,true,now),
        Some(WeaponAction::Toggle)=>train_weapon(archive,false,now),
        None=>{},
    }
}
fn legacy_keys(archive:&mut Archive,pressed:&dyn Fn(&[Key])->bool,now:f64) {
    if archive.legacy.is_none() {return;}
    let (left,right,up,down)=(pressed(&[Key::LeftArrow,Key::A,Key::GamepadDpadLeft]),pressed(&[Key::RightArrow,Key::D,Key::GamepadDpadRight]),
        pressed(&[Key::UpArrow,Key::W,Key::GamepadDpadUp]),pressed(&[Key::DownArrow,Key::S,Key::GamepadDpadDown]));
    if archive.column==0 {
        if up {archive.class=(archive.class+Class::PLAYABLE.len()-1)%Class::PLAYABLE.len();}
        if down {archive.class=(archive.class+1)%Class::PLAYABLE.len();}
        if right||pressed(&[Key::Enter,Key::GamepadFaceDown]) {archive.column=1;}
    } else {
        if left {archive.column=0;}
        if up {archive.talent=archive.talent.saturating_sub(1);}
        if down {archive.talent=(archive.talent+1).min(4);}
        if pressed(&[Key::Enter,Key::GamepadFaceDown]) {choose(archive,archive.talent,now);}
    }
}
fn choose(archive:&mut Archive,index:usize,now:f64) {
    let class=Class::PLAYABLE[archive.class];let talisman=heritage::talismans(class)[index];
    let Some((owner,legacy))=&mut archive.legacy else{return};
    let result=legacy.get_mut(class).choose(class,talisman).map_err(str::to_owned).and_then(|_|crate::legacy::save(*owner,legacy));
    let name=crate::reward_ui::talisman_description(talisman).0;
    archive.notice=Some((match result {Ok(())=>format!("下一局以「{name}」开局"),Err(error)=>error},now));
}
/// 熟练度进度条
fn bar(ui:&Ui,min:[f32;2],max:[f32;2],fill:f32,s:f32) {
    let draw=ui.get_window_draw_list();
    draw.add_rect(min,max,ImColor32::from_rgba(56,50,40,230)).filled(true).rounding(3.0*s).build();
    if fill>0.0 {draw.add_rect_filled_multicolor(min,[min[0]+(max[0]-min[0])*fill.min(1.0),max[1]],ImColor32::from_rgba(150,110,60,255),ImColor32::from_rgba(222,180,98,255),ImColor32::from_rgba(222,180,98,255),ImColor32::from_rgba(150,110,60,255));}
}
#[allow(clippy::too_many_arguments)]
fn legacy(ui:&Ui,archive:&mut Archive,icon:&mut dyn FnMut(u32)->Option<TextureId>,moved:bool,ready:bool,s:f32,ox:f32,oy:f32) {
    let p=|x:f32,y:f32|[ox+x*s,oy+y*s];
    let text=|x:f32,y:f32,value:&str,color:[f32;4],scale:f32,align:f32| {
        ui.set_window_font_scale(scale*s);ui.set_cursor_screen_pos([ox+x*s-ui.calc_text_size(value)[0]*align,oy+y*s]);ui.text_colored(color,value);
    };
    let Some((_,data))=archive.legacy.clone() else {
        text(800.0,460.0,"载入角色后才能查看职业传承",DIM,1.0,0.5);return;
    };
    let now=ui.time();
    // 左侧：六个职业与各自的熟练度
    let draw=ui.get_window_draw_list();panel(&draw,p(70.0,176.0),p(430.0,800.0),s);drop(draw);
    text(250.0,190.0,"职  业",GOLD,1.05,0.5);
    for (i,class) in Class::PLAYABLE.iter().enumerate() {
        let y=232.0+i as f32*88.0;let entry=data.get(*class);let (level,into,need)=entry.progress();
        let (min,max)=(p(88.0,y),p(412.0,y+50.0));
        ui.set_cursor_screen_pos(min);ui.set_window_font_scale(s);
        if ui.invisible_button(format!("class-{i}"),[max[0]-min[0],max[1]-min[1]])&&ready {archive.class=i;archive.column=0;}
        let selected=archive.class==i;let hover=ui.is_item_hovered();
        let draw=ui.get_window_draw_list();
        if selected||hover {draw.add_rect(min,max,if selected{ImColor32::from_rgba(46,38,25,238)}else{ImColor32::from_rgba(30,26,19,220)}).filled(true).rounding(4.0*s).build();}
        if selected {draw.add_rect(min,max,if archive.column==0{ImColor32::from_rgba(236,200,128,235)}else{ImColor32::from_rgba(171,143,87,140)}).rounding(4.0*s).thickness(1.5*s).build();}
        drop(draw);
        text(104.0,y+6.0,crate::map_ui::class_name(*class),if selected{GOLD}else{WHITE},0.95,0.0);
        let unlocked=heritage::UNLOCKS.iter().filter(|l|level>=**l).count();
        text(396.0,y+7.0,&format!("Lv {level} · 护符 {}/{}",unlocked+1,heritage::UNLOCKS.len()+1),if unlocked>0{BOON}else{MUTED},0.8,1.0);
        bar(ui,p(104.0,y+36.0),p(396.0,y+40.0),if need==0{1.0}else{into as f32/need as f32},s);
    }
    // 右上：选中职业的熟练度与当前开局护符
    let class=Class::PLAYABLE[archive.class];let entry=data.get(class);let (level,into,need)=entry.progress();
    let chosen=entry.chosen(class);
    let draw=ui.get_window_draw_list();panel(&draw,p(450.0,176.0),p(1530.0,300.0),s);drop(draw);
    text(478.0,190.0,crate::map_ui::class_name(class),GOLD,1.5,0.0);
    text(478.0,240.0,&format!("熟练度 Lv {level}{}",if need==0{"（已满级）".to_string()}else{format!("  ·  {into} / {need}")}),WHITE,0.92,0.0);
    bar(ui,p(478.0,272.0),p(1000.0,278.0),if need==0{1.0}else{into as f32/need as f32},s);
    text(1500.0,196.0,"下一局的开局护符",MUTED,0.84,1.0);
    text(1500.0,224.0,&crate::reward_ui::talisman_description(chosen).0,GOLD,1.2,1.0);
    if let Some((message,at))=&archive.notice && now-at<2.5 {text(1500.0,264.0,message,BOON,0.84,1.0);}
    // 可选的开局护符：职业默认护符与四枚按熟练度解锁的护符
    for (i,talisman) in heritage::talismans(class).into_iter().enumerate() {
        let y=316.0+i as f32*96.0;let (min,max)=(p(450.0,y),p(1530.0,y+88.0));
        ui.set_cursor_screen_pos(min);ui.set_window_font_scale(s);
        let clicked=ui.invisible_button(format!("talisman-{i}"),[max[0]-min[0],max[1]-min[1]]);
        if ui.is_item_hovered()&&moved {archive.talent=i;archive.column=1;}
        if clicked&&ready {archive.talent=i;archive.column=1;choose(archive,i,now);}
        let required=heritage::unlock_level(i);let locked=level<required;let picked=talisman==chosen;let focused=archive.column==1&&archive.talent==i;
        let draw=ui.get_window_draw_list();
        draw.add_rect(min,max,if focused{ImColor32::from_rgba(46,38,25,238)}else if locked{ImColor32::from_rgba(14,13,11,210)}else{ImColor32::from_rgba(22,19,14,226)}).filled(true).rounding(4.0*s).build();
        draw.add_rect(min,max,if focused{ImColor32::from_rgba(236,200,128,235)}else if picked{ImColor32::from_rgba(214,180,112,170)}else{ImColor32::from_rgba(90,80,62,90)}).rounding(4.0*s).thickness(if focused||picked{1.8*s}else{s}).build();
        draw.add_rect(p(468.0,y+12.0),p(532.0,y+76.0),ImColor32::from_rgba(0,0,0,150)).filled(true).rounding(4.0*s).build();
        if let Some(texture)=crate::reward_ui::talisman_icon(talisman).and_then(&mut *icon) {
            let tint=if locked{60}else{255};
            draw.add_image(texture,p(470.0,y+14.0),p(530.0,y+74.0)).col(ImColor32::from_rgba(tint,tint,tint,255)).build();
        }
        drop(draw);
        let (name,info)=crate::reward_ui::talisman_description(talisman);
        text(552.0,y+12.0,&name,if locked{DIM}else if picked{GOLD}else{WHITE},1.05,0.0);
        let info=info.lines().next().unwrap_or("").to_owned();
        wrapped(ui,p(552.0,y+44.0),ox+1330.0*s,&info,if locked{DIM}else{BOON},0.84*s);
        let (status,color)=if picked {("已选用",GOLD)} else if locked {(if i==0 {""}else{"未解锁"},RED)} else if focused {("选用  [A]",GOLD)} else {("",MUTED)};
        if !status.is_empty() {text(1506.0,y+16.0,status,color,0.9,1.0);}
        let unlock=if i==0 {"默认".to_string()}else{format!("熟练度 Lv {required} 解锁")};
        text(1506.0,y+50.0,&unlock,if locked{RED}else{DIM},0.78,1.0);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weapon_categories_cover_every_type_once_and_have_icons() {
        let mut rows=Vec::new();
        for group in 0..WEAPON_GROUPS.len() {
            let current=weapon_rows(group);assert!(!current.is_empty()&&current.len()<=12);
            rows.extend(current);
        }
        rows.sort_unstable();
        assert_eq!(rows,(1..=erdtree_tower::weapon_training::TYPES.len()).collect::<Vec<_>>());
        for &(kind,name) in erdtree_tower::weapon_training::TYPES {
            assert!(crate::reward_ui::weapon_type_icon(kind).is_some(),"{name} missing icon");
        }
        let mut archive=Archive::new(vec![]);
        for group in 0..WEAPON_GROUPS.len(){set_weapon_group(&mut archive,group);assert!(weapon_rows(group).contains(&archive.weapon_row));}
    }
    #[test]
    fn aura_list_starts_with_no_effect_and_preserves_existing_ids() {
        assert_eq!(aura_choice(0).unwrap().id,0);
        for (row,aura) in erdtree_tower::auras::AURAS.iter().enumerate(){assert_eq!(aura_choice(row+1).unwrap().id,aura.id);}
        assert!(aura_choice(erdtree_tower::auras::AURAS.len()+1).is_none());
    }
    #[test]
    fn progression_opens_legacy_with_class_focus_without_changing_archive_default() {
        let archive=Archive::new(vec![]);
        let progression=Archive::progression(vec![]);
        assert_eq!(archive.tab,0);
        assert_eq!((progression.tab,progression.column),(2,0));
        assert_eq!(progression.class,archive.class);
    }
    #[test]
    fn relative_times_read_naturally() {
        assert_eq!(ago(1000,0),"");
        assert_eq!((ago(1000,990),ago(4000,1000),ago(20_000,1000),ago(100_000,1000)),("刚刚".into(),"50 分钟前".into(),"5 小时前".into(),"1 天前".into()));
    }
    #[test]
    fn codex_navigation_moves_between_neighbours() {
        let items=vec![(Item::Oath(0),[0.0,0.0,100.0,50.0]),(Item::Oath(1),[108.0,0.0,100.0,50.0]),(Item::Oath(2),[0.0,58.0,100.0,50.0]),(Item::Ember(0),[0.0,140.0,80.0,80.0])];
        assert_eq!((step(&items,0,1.0,0.0),step(&items,0,0.0,1.0),step(&items,2,0.0,1.0),step(&items,1,0.0,1.0)),(1,2,3,2));
        assert_eq!(step(&items,0,-1.0,0.0),0,"no neighbour keeps the focus");
    }
}
