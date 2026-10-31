use std::{collections::VecDeque,sync::{Mutex,atomic::{AtomicBool,Ordering}}};
use erdtree_tower::{Class,Run};
#[cfg(test)]
use erdtree_tower::Phase;
use hudhook::imgui::{Ui,Key,Condition,StyleColor,TextureId};
use erdtree_tower::law::Law;
use erdtree_tower::{journal::{Outcome,Record},legacy::Gain};

static SCREEN:Mutex<VecDeque<Screen>>=Mutex::new(VecDeque::new());
static PENDING:Mutex<Option<(Record,Option<Gain>)>>=Mutex::new(None);
static SEED:Mutex<Option<u64>>=Mutex::new(None);
static LAW_START:Mutex<Option<(Option<u64>,Law,Class)>>=Mutex::new(None);
/// 客机开局：选完职业后等待房主的苦难律法同步。
static CLASS_START:Mutex<Option<Class>>=Mutex::new(None);
static ACTIVE:AtomicBool=AtomicBool::new(false);
enum Screen {Summary{record:Record,gain:Option<Gain>,from_archive:bool,focus:usize,opened:Option<f64>,
    /// 往期最好的一局（不含这一局，先比评级再比评分）：新纪录提示用；从档案打开时为 None
    best:Option<erdtree_tower::score::Score>,cues:Cues},Archive(Box<crate::archive_ui::Archive>),Notice{message:String,opened:Option<f64>},TrialRewards{items:Vec<u32>,opened:Option<f64>},Seed{input:String,error:String,digit:u8,opened:Option<f64>},Law{screen:crate::law_ui::LawScreen,seed:Option<u64>,record:Option<(u32,String)>,class:Class},
    /// 选择职业；law 为 false 时（客机）选完直接开局
    Class{screen:crate::class_ui::ClassScreen,seed:Option<u64>,law:bool}}
fn show(next:Screen) {
    let Ok(mut screens)=SCREEN.lock() else{return;};
    if screens.iter().any(|s|match (s,&next) {(Screen::Notice{message:a,..},Screen::Notice{message:b,..})=>a==b,(Screen::Seed{..},Screen::Seed{..})|(Screen::Law{..},Screen::Law{..})|(Screen::Class{..},Screen::Class{..})=>true,_=>false}) {return;}
    screens.push_back(next);ACTIVE.store(true,Ordering::Relaxed);
}
pub fn party_notice(message:String) {show(Screen::Notice{message,opened:None});}
pub fn trial_rewards(before:&Run,after:&Run) {
    let items=after.elite_buffs.iter().copied().filter(|id|!before.elite_buffs.contains(id)).collect();
    show(Screen::TrialRewards{items,opened:None});
}
pub fn active()->bool {ACTIVE.load(Ordering::Relaxed)}
pub fn trial_rewards_active()->bool {SCREEN.lock().is_ok_and(|s|s.iter().any(|s|matches!(s,Screen::TrialRewards{..})))}
#[cfg(test)]
pub fn summary(run:&Run) {summary_with((Record::new(run,vec![],vec![],0),None));}
/// 整局结束：回到大赐福后显示这条结算记录（附带这一局的职业熟练度变化）
pub fn summary_with(summary:(Record,Option<Gain>)) {if let Ok(mut pending)=PENDING.lock(){*pending=Some(summary);}}
/// 打开旅程档案（往期旅程与图鉴）；其他界面打开时盖在它们上面，关闭后回到原界面
pub fn archive_entry() {
    open_archive(false);
}
pub fn progression_entry() {
    open_archive(true);
}
fn open_archive(progression:bool) {
    let Ok(mut screens)=SCREEN.lock() else{return;};
    if screens.iter().any(|s|matches!(s,Screen::Archive(_))) {return;}
    let records=crate::run_log::records();
    let archive=if progression {crate::archive_ui::Archive::progression(records)}else{crate::archive_ui::Archive::new(records)};
    screens.push_front(Screen::Archive(Box::new(archive)));ACTIVE.store(true,Ordering::Relaxed);
}
pub fn hub_ready() {
    if let Ok(mut screen)=SCREEN.lock() {
        if !screen.is_empty(){return;}
        if let Some((record,gain))=PENDING.lock().ok().and_then(|mut pending|pending.take()) {
            let best=erdtree_tower::score::best(&crate::run_log::records(),&record);
            screen.push_back(Screen::Summary{record,gain,from_archive:false,focus:0,opened:None,best,cues:Cues::default()});ACTIVE.store(true,Ordering::Relaxed);
        }
    }
}
pub fn seed_entry() {show(Screen::Seed{input:String::new(),error:String::new(),digit:0,opened:None});}
pub fn take_seed()->Option<u64> {SEED.lock().ok()?.take()}
/// 开局：先选职业（默认选中 current），单人或房主再立苦难律法；seed 为空时随机开局
pub fn class_entry(seed:Option<u64>,law:bool,current:Class) {
    let focus=Class::PLAYABLE.iter().position(|c|*c==current).unwrap_or(0);
    show(Screen::Class{screen:crate::class_ui::ClassScreen::new(focus),seed,law});
}
fn law_screen(seed:Option<u64>,class:Class)->Screen {
    let mut screen=crate::law_ui::LawScreen::new(Law::default());screen.seed=seed;
    let record=crate::legacy::owner().and_then(|owner|crate::law::best(owner,class)).map(|best|(best,crate::map_ui::class_name(class).to_owned()));
    Screen::Law{screen,seed,record,class}
}
pub fn take_law_start()->Option<(Option<u64>,Law,Class)> {LAW_START.lock().ok()?.take()}
pub fn take_class_start()->Option<Class> {CLASS_START.lock().ok()?.take()}
fn parse_seed(text:&str)->Result<u64,&'static str> {
    let text=text.trim();
    if text.is_empty()||!text.bytes().all(|c|c.is_ascii_digit()){return Err("请输入数字种子");}
    text.parse().map_err(|_|"种子超出范围")
}
pub fn draw(ui:&Ui,icon:&mut dyn FnMut(u32)->Option<TextureId>)->bool {
    let Ok(mut screen)=SCREEN.lock() else{return false;};
    let Some(current)=screen.front_mut() else{return false;};
    let mut next=None;
    let done=match current {
        Screen::Law{screen:law,seed,record,class}=>match crate::law_ui::draw(ui,law,record.as_ref().map(|(best,class)|(*best,class.as_str()))) {
            Some(crate::law_ui::LawAction::Start(law))=>{if let Ok(mut start)=LAW_START.lock(){*start=Some((*seed,law,*class));}true},
            // 返回：回到选择职业
            Some(crate::law_ui::LawAction::Back)=>{next=Some(Screen::Class{screen:crate::class_ui::ClassScreen::new(Class::PLAYABLE.iter().position(|c|c==class).unwrap_or(0)),seed:*seed,law:true});true},
            Some(crate::law_ui::LawAction::Archive)=>{next=Some(Screen::Archive(Box::new(crate::archive_ui::Archive::new(crate::run_log::records()))));false},
            None=>false,
        },
        Screen::Class{screen:classes,seed,law}=>match crate::class_ui::draw(ui,classes) {
            Some(crate::class_ui::ClassAction::Pick(index))=>{
                let class=Class::PLAYABLE[index.min(Class::PLAYABLE.len()-1)];
                if *law {next=Some(law_screen(*seed,class));}
                else if let Ok(mut start)=CLASS_START.lock() {*start=Some(class);}
                true
            },
            Some(crate::class_ui::ClassAction::Back)=>true,
            None=>false,
        },
        Screen::Archive(archive)=>match crate::archive_ui::draw(ui,archive,icon) {
            Some(crate::archive_ui::ArchiveAction::Close)=>true,
            Some(crate::archive_ui::ArchiveAction::Open(record))=>{next=Some(Screen::Summary{record,gain:None,from_archive:true,focus:0,opened:None,best:None,cues:Cues::default()});false},
            None=>false,
        },
        _=>render(ui,current,icon),
    };
    if done {screen.pop_front();}
    if let Some(next)=next {screen.push_front(next);}
    ACTIVE.store(!screen.is_empty(),Ordering::Relaxed);
    true
}
fn render(ui:&Ui,current:&mut Screen,icon:&mut dyn FnMut(u32)->Option<TextureId>)->bool {
    if let Screen::Summary{record,gain,from_archive,focus,opened,best,cues}=current {
        let opened=opened.get_or_insert(ui.time());let elapsed=ui.time()-*opened;
        // 入场动画中按确认或点一下：直接跳到动画结束（这一帧不算关闭）
        let ready=elapsed>=ANIM_END;
        if !ready&&([Key::Enter,Key::GamepadFaceDown,Key::Escape,Key::GamepadFaceRight].into_iter().any(|k|ui.is_key_pressed_no_repeat(k))||ui.is_mouse_clicked(hudhook::imgui::MouseButton::Left)) {*opened-=ANIM_END-elapsed;}
        let elapsed=ui.time()-*opened;
        return summary_page(ui,record,*gain,focus,*from_archive,*best,elapsed,cues,icon,ready);
    }
    if let Screen::Seed{input,error,digit,opened}=current {let opened=*opened.get_or_insert(ui.time());return seed_page(ui,input,error,digit,ui.time()-opened>=0.35);}
    let size=ui.io().display_size;
    crate::oath_art::background(&ui.get_background_draw_list(),size);
    let mut close=false;
    let _background=ui.push_style_color(StyleColor::WindowBg,[0.035,0.035,0.025,0.95]);
    let _text=ui.push_style_color(StyleColor::Text,[0.85,0.76,0.52,1.0]);
    ui.window("旅程##run-screen").position([size[0]*0.5,size[1]*0.5],Condition::Always).position_pivot([0.5,0.5])
        .size([620.0f32.min(size[0]*0.9),510.0f32.min(size[1]*0.9)],Condition::Always).title_bar(false).resizable(false).movable(false).build(|| {
        ui.dummy([0.0,15.0]);
        match current {
            Screen::Notice{message,opened}=>{
                let opened=*opened.get_or_insert(ui.time());
                ui.text("暂时无法开始联机旅程");ui.separator();ui.spacing();crate::fancy_text::text_wrapped(ui,message);
                ui.spacing();ui.text("详细原因已记录到 tower-preview.log。");ui.spacing();
                if (crate::pad::button(ui,"notice-ok","确定",Some(crate::pad::CONFIRM),[0.0,0.0])||ui.is_key_pressed(Key::Enter)||ui.is_key_pressed(Key::GamepadFaceDown)||ui.is_key_pressed(Key::Escape)||ui.is_key_pressed(Key::GamepadFaceRight))&&ui.time()-opened>=0.35 {close=true;}
            },
            Screen::TrialRewards{items,opened}=>{
                let opened=*opened.get_or_insert(ui.time());
                ui.text("男生女生向前冲 · 挑战完成");ui.separator();ui.spacing();
                ui.text("已获得以下护符赐福");ui.spacing();
                for &id in items.iter() {
                    let (name,info)=crate::reward_ui::talisman_description(id);
                    ui.text_colored([0.9,0.78,0.5,1.0],name);crate::fancy_text::text_wrapped(ui,info);ui.spacing();ui.separator();ui.spacing();
                }
                if items.is_empty(){crate::fancy_text::text_wrapped(ui,"已拥有全部可获得的护符赐福，本次没有新增赐福。");}
                crate::fancy_text::text_wrapped(ui,"本局持续生效，不占护符栏位，无需装备。");ui.spacing();
                let ready=ui.time()-opened>=0.35;
                if (crate::pad::button(ui,"trial-ok","确定，继续选路",Some(crate::pad::CONFIRM),[0.0,0.0])||ui.is_key_pressed(Key::Enter)||ui.is_key_pressed(Key::GamepadFaceDown))&&ready {close=true;}
            },
            Screen::Summary{..}|Screen::Seed{..}|Screen::Law{..}|Screen::Class{..}|Screen::Archive(_)=>unreachable!(),
        }
    });
    close
}
#[cfg(test)]
pub fn test_frames(context:&mut hudhook::imgui::Context) {
    let mut run=Run::new(u64::MAX,erdtree_tower::Class::Samurai);run.phase=Phase::Dead;
    summary(&run);
    assert!(!active(),"Death must only queue the result, not capture input");
    assert!(!draw(context.frame(),&mut |_|None),"Death and loading must remain unobstructed");context.render();
    hub_ready();
    assert!(active());
    assert!(PENDING.lock().unwrap().is_none());
    SCREEN.lock().unwrap().clear();ACTIVE.store(false,Ordering::Relaxed);
    hub_ready();assert!(!active(),"The result must only open once");
    let before=run.clone();let mut after=before.clone();
    after.elite_buffs=vec![1000,1150];trial_rewards(&before,&after);
    assert!(trial_rewards_active());
    assert!(matches!(SCREEN.lock().unwrap().front(),Some(Screen::TrialRewards{items,..}) if items==&vec![1000,1150]));
    party_notice("队伍已解散".into());party_notice("队伍已解散".into());
    {let screens=SCREEN.lock().unwrap();assert!(matches!(screens.front(),Some(Screen::TrialRewards{..}))&&screens.len()==2,"A notice must queue behind unacknowledged rewards");}
    SCREEN.lock().unwrap().clear();ACTIVE.store(false,Ordering::Relaxed);
    for size in [[1280.0,720.0],[1920.0,1080.0],[3440.0,1440.0]] {
        context.io_mut().display_size=size;
        for mut screen in [Screen::Notice{message:"爬塔协议版本不匹配：本机 23，对方 22。\n请所有玩家更新同一版爬塔补丁。".into(),opened:None},Screen::Summary{record:Record::new(&run,vec![erdtree_tower::journal::Fight{act:1,layer:9,kind:"精英".into(),won:false,enemies:vec!["黑骑士".into()]}],vec![],0),gain:Some(Gain{class:erdtree_tower::Class::Samurai,xp:320,before:4,after:5}),from_archive:false,focus:0,opened:None,best:None,cues:Cues::default()},Screen::TrialRewards{items:vec![1000,1150],opened:None},Screen::TrialRewards{items:vec![],opened:None},Screen::Seed{input:u64::MAX.to_string(),error:String::new(),digit:9,opened:None}] {
            render(context.frame(),&mut screen,&mut |_|None);
            let data=context.render();assert!(data.total_vtx_count>0);
            if matches!(screen,Screen::Summary{..}) {if let Ok(path)=std::env::var("ERDTREE_UI_PREVIEW") {
                use std::io::Write;
                let mut f=std::fs::File::create(std::path::Path::new(&path).join(format!("summary-{}-{}.draw",size[0],size[1]))).unwrap();
                writeln!(f,"{} {}",size[0],size[1]).unwrap();
                for list in data.draw_lists(){for command in list.commands(){if let hudhook::imgui::DrawCmd::Elements{count,cmd_params:p}=command {
                    writeln!(f,"C {} {} {} {} {}",p.texture_id.id(),p.clip_rect[0],p.clip_rect[1],p.clip_rect[2],p.clip_rect[3]).unwrap();
                    for triangle in list.idx_buffer()[p.idx_offset..p.idx_offset+count].chunks_exact(3) {
                        write!(f,"T").unwrap();for index in triangle {let v=list.vtx_buffer()[*index as usize+p.vtx_offset];write!(f," {} {} {} {} {} {} {} {}",v.pos[0],v.pos[1],v.uv[0],v.uv[1],v.col[0],v.col[1],v.col[2],v.col[3]).unwrap();}writeln!(f).unwrap();
                    }
                }}}
            }}
        }
    }
}
const SEED_KEYS:[&str;12]=["1","2","3","4","5","退格","6","7","8","9","0","清空"];
const SEED_DIGITS:usize=20;
fn seed_key(input:&mut String,error:&mut String,key:&str) {
    match key {"退格"=>{input.pop();},"清空"=>input.clear(),digit=>if input.len()<SEED_DIGITS {input.push_str(digit);}}
    error.clear();
}
fn seed_page(ui:&Ui,input:&mut String,error:&mut String,selected:&mut u8,ready:bool)->bool {
    let size=ui.io().display_size;
    let s=(size[0]/1600.0).min(size[1]/900.0);
    let mid=size[0]*0.5;let top=(size[1]-690.0*s)*0.5;
    let gold=[0.84,0.72,0.46,1.0];let muted=[0.64,0.61,0.52,1.0];let white=[0.92,0.88,0.77,1.0];let red=[0.9,0.46,0.38,1.0];
    crate::oath_art::background(&ui.get_background_draw_list(),size);
    let key=|k:Key|ready&&ui.is_key_pressed(k);
    let digits=[(Key::Alpha0,Key::Keypad0),(Key::Alpha1,Key::Keypad1),(Key::Alpha2,Key::Keypad2),(Key::Alpha3,Key::Keypad3),(Key::Alpha4,Key::Keypad4),
        (Key::Alpha5,Key::Keypad5),(Key::Alpha6,Key::Keypad6),(Key::Alpha7,Key::Keypad7),(Key::Alpha8,Key::Keypad8),(Key::Alpha9,Key::Keypad9)];
    for (n,(a,b)) in digits.into_iter().enumerate() {if key(a)||key(b){seed_key(input,error,&n.to_string());}}
    if key(Key::Backspace)||key(Key::GamepadFaceUp){seed_key(input,error,"退格");}
    if key(Key::Delete){seed_key(input,error,"清空");}
    if ui.io().key_ctrl&&key(Key::V) {
        for c in ui.clipboard_text().unwrap_or_default().chars().filter(char::is_ascii_digit) {seed_key(input,error,&c.to_string());}
    }
    if ui.is_key_pressed(Key::GamepadDpadLeft){*selected=(*selected+11)%12;}
    if ui.is_key_pressed(Key::GamepadDpadRight){*selected=(*selected+1)%12;}
    if ui.is_key_pressed(Key::GamepadDpadUp)||ui.is_key_pressed(Key::GamepadDpadDown){*selected=(*selected+6)%12;}
    *selected%=12;
    if key(Key::GamepadFaceDown){seed_key(input,error,SEED_KEYS[usize::from(*selected)]);}
    let mut start=key(Key::Enter)||key(Key::KeypadEnter)||key(Key::GamepadFaceLeft);
    let mut close=key(Key::Escape)||key(Key::GamepadFaceRight);
    let mut pressed=None;
    ui.window("以种子开始##seed").position([0.0,0.0],Condition::Always).size(size,Condition::Always)
        .flags(hudhook::imgui::WindowFlags::NO_DECORATION|hudhook::imgui::WindowFlags::NO_BACKGROUND|hudhook::imgui::WindowFlags::NO_MOVE|hudhook::imgui::WindowFlags::NO_NAV).build(|| {
        let text=|x:f32,y:f32,value:&str,color:[f32;4],scale:f32| {
            ui.set_window_font_scale(scale*s);ui.set_cursor_pos([x-ui.calc_text_size(value)[0]*0.5,top+y*s]);ui.text_colored(color,value);
        };
        text(mid,12.0,"开始新旅程",muted,1.0);
        text(mid,57.0,"以 种 子 开 启 旅 程",gold,2.1);
        text(mid,130.0,"相同种子生成相同的路线与敌人，奖励仍独立随机",muted,1.0);
        let draw=ui.get_window_draw_list();
        for y in [194.0,540.0] {draw.add_line([mid-410.0*s,top+y*s],[mid+410.0*s,top+y*s],hudhook::imgui::ImColor32::from_rgba(171,143,87,100)).build();}
        draw.add_rect([mid-394.0*s,top+222.0*s],[mid+394.0*s,top+300.0*s],hudhook::imgui::ImColor32::from_rgba(10,9,7,210)).filled(true).rounding(6.0*s).build();
        draw.add_rect([mid-394.0*s,top+222.0*s],[mid+394.0*s,top+300.0*s],hudhook::imgui::ImColor32::from_rgba(171,143,87,170)).rounding(6.0*s).thickness(1.5*s).build();
        drop(draw);
        if input.is_empty() {text(mid,247.0,"输入 0 – 18446744073709551615 之间的整数",muted,1.0);}
        else {text(mid,240.0,&format!("{input}{}",if (ui.time()*2.0) as i64%2==0 {"|"}else{" "}),white,1.6);}
        if error.is_empty() {text(mid,318.0,&format!("{} / {SEED_DIGITS} 位  ·  可直接用键盘输入，Ctrl+V 粘贴",input.len()),muted,0.9);}
        else {text(mid,318.0,error,red,1.0);}
        let (w,h,gap)=(118.0,60.0,16.0);
        for (i,label) in SEED_KEYS.iter().enumerate() {
            let x=mid+(-394.0+(i%6) as f32*(w+gap))*s;let y=370.0+(i/6) as f32*(h+gap);
            let pos=[x,top+y*s];let extent=[w*s,h*s];
            ui.set_cursor_screen_pos(pos);if ui.invisible_button(format!("##seed-key-{i}"),extent)&&ready{pressed=Some(i);}
            let focus=usize::from(*selected)==i;let draw=ui.get_window_draw_list();
            crate::oath_art::button(&draw,pos,[pos[0]+extent[0],pos[1]+extent[1]],if ui.is_item_hovered()||focus{255}else{190});
            if focus {draw.add_rect([pos[0]-3.0*s,pos[1]-3.0*s],[pos[0]+extent[0]+3.0*s,pos[1]+extent[1]+3.0*s],hudhook::imgui::ImColor32::from_rgba(214,184,117,230)).rounding(5.0*s).thickness(2.0*s).build();}
            drop(draw);
            let label_scale=if label.len()>1 {0.95}else{1.25};
            ui.set_window_font_scale(label_scale*s);
            ui.set_cursor_pos([x+extent[0]*0.5-ui.calc_text_size(label)[0]*0.5,pos[1]+(extent[1]-ui.text_line_height())*0.5]);
            ui.text_colored(if focus{white}else{gold},label);
        }
        {
            use crate::pad::{self,Input,Pad};
            let items:&[(Option<Input>,&str)]=if pad::using_pad() {&[(Some(pad::MOVE),"选择"),(Some(Input::new(Pad::A,"")),"按下"),(Some(Input::new(Pad::Y,"")),"退格"),(Some(pad::START),"开始"),(Some(pad::BACK),"取消")]}
                else {&[(None,"直接输入数字"),(Some(Input::new(Pad::Y,"Backspace")),"退格"),(Some(pad::START),"开始"),(Some(pad::BACK),"取消")]};
            pad::hints(ui,[mid,top+566.0*s],0.5,items,24.0*s,16.0*s,muted,pad::Tone::Light);
        }
        ui.set_window_font_scale(s);
        let button=|label:&str,input:crate::pad::Input,x:f32| {
            let pos=[x,top+602.0*s];let extent=[320.0*s,65.0*s];
            ui.set_cursor_screen_pos(pos);let clicked=ui.invisible_button(label,extent);
            crate::oath_art::button(&ui.get_window_draw_list(),pos,[pos[0]+extent[0],pos[1]+extent[1]],if ui.is_item_hovered(){255}else{215});
            crate::pad::label_in(ui,pos,[pos[0]+extent[0],pos[1]+extent[1]],Some(input),label,gold,s,crate::pad::Tone::Light);clicked
        };
        start|=button("开始旅程",crate::pad::START,mid-340.0*s)&&ready;
        close|=button("取消",crate::pad::BACK,mid+20.0*s)&&ready;
    });
    if let Some(i)=pressed {seed_key(input,error,SEED_KEYS[i]);}
    if start&&!close {
        match parse_seed(input) {Ok(seed)=>{if let Ok(mut pending)=SEED.lock(){*pending=Some(seed);return true;}},Err(message)=>*error=message.into()}
    }
    close
}
const ROMAN:[&str;4]=["","I","II","III"];
/// 从 (x,y) 起写一段在 right 处折行的文字，返回下一行的 y（都是屏幕坐标）
/// 按中文规则换行（见 fancy_text::wrap_lines），逐行画出
pub(crate) fn wrapped(ui:&Ui,pos:[f32;2],right:f32,value:&str,color:[f32;4],scale:f32)->f32 {
    ui.set_window_font_scale(scale);
    let lines=crate::fancy_text::wrap_lines(value,(right-pos[0]).max(1.0),|t|ui.calc_text_size(t)[0]);
    let step=ui.text_line_height_with_spacing();let mut y=pos[1];let mut bottom=pos[1];
    for line in lines {ui.set_cursor_screen_pos([pos[0],y]);ui.text_colored(color,&line);bottom=ui.item_rect_max()[1];y+=step;}
    bottom
}
/// 结算页：标题与终局说明，下方三块面板（旅程 · 誓约 · 余烬），底部种子与按钮。
/// from_archive 时从旅程档案打开，第二个按钮改为返回档案
/// 结算页大数字与评级字母的笔画（单位方框内的折线，y 向下）：几何笔画放大不糊，方角像工业风的显示字体
fn strokes(c:char)->&'static [&'static [[f32;2]]] {
    match c {
        '0'=>&[&[[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0],[0.0,0.0]]],
        '1'=>&[&[[0.2,0.0],[0.62,0.0],[0.62,1.0]]],
        '2'=>&[&[[0.0,0.0],[1.0,0.0],[1.0,0.5],[0.0,0.5],[0.0,1.0],[1.0,1.0]]],
        '3'=>&[&[[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]],&[[0.3,0.5],[1.0,0.5]]],
        '4'=>&[&[[0.0,0.0],[0.0,0.6],[1.0,0.6]],&[[0.75,0.3],[0.75,1.0]]],
        '5'=>&[&[[1.0,0.0],[0.0,0.0],[0.0,0.5],[1.0,0.5],[1.0,1.0],[0.0,1.0]]],
        // S 两头往回勾，免得看成 5
        'S'=>&[&[[1.0,0.22],[1.0,0.0],[0.0,0.0],[0.0,0.5],[1.0,0.5],[1.0,1.0],[0.0,1.0],[0.0,0.78]]],
        '6'=>&[&[[1.0,0.0],[0.0,0.0],[0.0,1.0],[1.0,1.0],[1.0,0.5],[0.0,0.5]]],
        '7'=>&[&[[0.0,0.0],[1.0,0.0],[1.0,1.0]]],
        '8'=>&[&[[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0],[0.0,0.0]],&[[0.0,0.5],[1.0,0.5]]],
        '9'=>&[&[[1.0,0.5],[0.0,0.5],[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]]],
        'A'=>&[&[[0.0,1.0],[0.0,0.25],[0.25,0.0],[0.75,0.0],[1.0,0.25],[1.0,1.0]],&[[0.0,0.58],[1.0,0.58]]],
        'B'=>&[&[[0.0,0.0],[0.78,0.0],[1.0,0.16],[1.0,0.34],[0.78,0.5],[0.0,0.5]],&[[0.78,0.5],[1.0,0.66],[1.0,0.84],[0.78,1.0],[0.0,1.0],[0.0,0.0]]],
        'C'=>&[&[[1.0,0.0],[0.0,0.0],[0.0,1.0],[1.0,1.0]]],
        'D'=>&[&[[0.0,0.0],[0.7,0.0],[1.0,0.3],[1.0,0.7],[0.7,1.0],[0.0,1.0],[0.0,0.0]]],
        _=>&[],
    }
}
const GLYPH_WIDTH:f32=0.56;const GLYPH_GAP:f32=0.15;const GLYPH_STROKE:f32=0.18;
/// 用笔画画一串字（数字与评级字母），左上角为 origin；每段笔画是带方头的实心四边形，转角处自然补齐
fn draw_glyphs(draw:&hudhook::imgui::DrawListMut<'_>,text:&str,origin:[f32;2],height:f32,color:hudhook::imgui::ImColor32) {
    let (w,t)=(GLYPH_WIDTH*height,GLYPH_STROKE*height);
    for (i,c) in text.chars().enumerate() {
        let x=origin[0]+i as f32*(GLYPH_WIDTH+GLYPH_GAP)*height;
        let at=|p:[f32;2]|[x+t*0.5+p[0]*(w-t),origin[1]+t*0.5+p[1]*(height-t)];
        for line in strokes(c) {for pair in line.windows(2) {
            let (a,b)=(at(pair[0]),at(pair[1]));let (dx,dy)=(b[0]-a[0],b[1]-a[1]);let len=(dx*dx+dy*dy).sqrt().max(1e-3);
            let (ux,uy)=(dx/len*t*0.5,dy/len*t*0.5);let (nx,ny)=(-uy,ux);
            let (a,b)=([a[0]-ux,a[1]-uy],[b[0]+ux,b[1]+uy]);
            draw.add_polyline(vec![[a[0]+nx,a[1]+ny],[b[0]+nx,b[1]+ny],[b[0]-nx,b[1]-ny],[a[0]-nx,a[1]-ny]],color).filled(true).build();
        }}
    }
}
/// 结算页入场动画的总长（秒）：动画没放完时按确认是跳过，放完后才是关闭
const ANIM_END:f64=3.0;
/// 离场动画（斜向暗幕扫过）的时长
const EXIT:f64=0.32;
/// 从 start 起 len 秒内由 0 到 1（先快后慢）
fn ease(elapsed:f64,start:f64,len:f64)->f32 {let t=((elapsed-start)/len).clamp(0.0,1.0) as f32;1.0-(1.0-t).powi(3)}
/// 带回弹的 0→1（越过一点再回来），用于弹出与落下
fn pop(elapsed:f64,start:f64,len:f64)->f32 {
    let t=((elapsed-start)/len).clamp(0.0,1.0) as f32;let c=1.70158;let u=t-1.0;
    if t<=0.0 {0.0}else{1.0+(c+1.0)*u*u*u+c*u*u}
}
/// 0–1 的伪随机（粒子用，结果固定）
fn hash(i:u32)->f32 {let mut x=i.wrapping_mul(0x9E37_79B9)^0x85EB_CA6B;x^=x>>15;x=x.wrapping_mul(0x2C1B_3C6D);x^=x>>12;(x&0xFFFF) as f32/65535.0}
/// 第 i 位数字停下的时间：从左到右依次停
fn lock_time(i:usize)->f64 {0.8+i as f64*0.14}
/// 第 i 位在 t 时刻滚到的位置（整数部分为显示的数字，小数部分为滚动偏移）：停之前匀速滚，最后 0.25 秒减速，正好停在 digit
fn roll(digit:u32,lock:f64,t:f64)->f32 {
    const SPEED:f32=22.0;let x=(lock-t).max(0.0) as f32;
    let behind=if x>=0.25 {SPEED*(x-0.125)}else{2.0*SPEED*x*x};
    digit as f32+20.0-behind
}
/// 裁剪：f 里画的东西（含文字）只留在 min..max 之内（屏幕坐标）
fn clip<R>(min:[f32;2],max:[f32;2],f:impl FnOnce()->R)->R {
    use hudhook::imgui::sys::{ImVec2,igPopClipRect,igPushClipRect};
    unsafe {igPushClipRect(ImVec2{x:min[0],y:min[1]},ImVec2{x:max[0],y:max[1]},true);}
    let result=f();unsafe {igPopClipRect();}result
}
/// 大号数字与评级字母：思源黑体粗体按 220 像素加载的那份（fancy_text::display_font）；字体缺失时退回几何笔画
#[derive(Clone,Copy)]
pub(crate) struct Big {font:Option<hudhook::imgui::FontId>,top:f32,height:f32,advance:f32,size:f32}
impl Big {
    pub(crate) fn new(ui:&Ui)->Self {
        let fallback=Self{font:None,top:0.0,height:1.0,advance:GLYPH_WIDTH+GLYPH_GAP,size:1.0};
        let Some(font)=crate::fancy_text::display_font() else {return fallback};
        let _font=ui.push_font(font);
        use hudhook::imgui::sys::{ImFont_FindGlyphNoFallback,igGetFont};
        unsafe {
            // 显示字号要乘上全局缩放（正文按屏幕密度加载、全局缩放为 1/密度）
            let f=igGetFont();let size=(*f).FontSize;
            let zero=ImFont_FindGlyphNoFallback(f,'0' as _);if zero.is_null()||size<=0.0 {return fallback;}
            let advance=('0'..='9').map(|c|{let g=ImFont_FindGlyphNoFallback(f,c as _);if g.is_null(){0.0}else{(*g).AdvanceX}}).fold(0.0f32,f32::max);
            Self{font:Some(font),top:(*zero).Y0/size,height:((*zero).Y1-(*zero).Y0)/size,advance:advance/size,size:size*ui.io().font_global_scale}
        }
    }
    /// 字高为 h 时一个字的格宽（与 h 同单位）
    pub(crate) fn cell(&self,h:f32)->f32 {if self.font.is_some() {self.advance/self.height*h}else{(GLYPH_WIDTH+GLYPH_GAP)*h}}
    pub(crate) fn width(&self,count:usize,h:f32)->f32 {self.cell(h)*count as f32-if self.font.is_some(){0.0}else{GLYPH_GAP*h}}
    /// 在格子里居中画一个字：x 为格子左边，top 为字顶，h 为字高（都是屏幕像素）
    pub(crate) fn draw(&self,ui:&Ui,c:char,x:f32,top:f32,h:f32,color:[f32;4]) {
        if color[3]<=0.0 {return;}
        let piece=c.to_string();
        match self.font {
            Some(font)=>{
                let _font=ui.push_font(font);let k=h/(self.height*self.size);ui.set_window_font_scale(k);
                let w=ui.calc_text_size(&piece)[0];let (x0,y0)=(x+(self.cell(h)-w)*0.5,top-self.top*self.size*k);
                // 加粗：大字号时左右各多画一份、错开一点，笔画更厚（参考图那种粗黑体）
                let spread=if h>60.0 {h*0.012}else{0.0};
                for dx in if spread>0.0 {[-spread,0.0,spread]}else{[0.0,0.0,0.0]} {ui.set_cursor_screen_pos([x0+dx,y0]);ui.text_colored(color,&piece);if spread<=0.0 {break;}}
            },
            None=>{
                let c=|v:f32|(v.clamp(0.0,1.0)*255.0) as u8;
                draw_glyphs(&ui.get_window_draw_list(),&piece,[x,top],h,hudhook::imgui::ImColor32::from_rgba(c(color[0]),c(color[1]),c(color[2]),c(color[3])));
            },
        }
    }
    pub(crate) fn draw_text(&self,ui:&Ui,text:&str,x:f32,top:f32,h:f32,color:[f32;4]) {for (i,c) in text.chars().enumerate() {self.draw(ui,c,x+i as f32*self.cell(h),top,h,color);}}
}
/// 结算页的音效与交互进度：一次性音效、分数滚动上一次“嗒”的时间、每一位是否已停、上一帧悬停的按钮、
/// 余烬下划线的当前位置、按钮悬停的过渡、离场开始的时间
#[derive(Default)]
struct Cues {fired:u16,last_tick:f64,ticks:u32,locked:u16,hover:Option<u8>,focus_x:Option<f32>,hover_t:[f32;2],closing:Option<f64>,last_frame:f64}
const CUE_OPEN:u16=1;const CUE_STAMP:u16=2;const CUE_RECORD:u16=4;const CUE_SWEEP:u16=8;const CUE_TOTAL:u16=16;
impl Cues {fn once(&mut self,cue:u16,name:&'static str) {if self.fired&cue==0 {self.fired|=cue;crate::sfx::play(name);}}}
/// 结算页（参考危机合约的结算画面）：左边是本局评分的大数字与评级，右边是苦难律法与分数构成，余烬、誓约压成小图标。
/// 入场：右侧面板擦开 → 每一位数字老虎机式滚动、从左到右停下 → 高光扫过 → 评级砸下（碎片、冲击环、速度线、震屏）
/// → 结果逐字打出、新纪录弹出 → 分数构成逐行落下、总分滚动 → 余烬、誓约与按钮依次弹出。之后背景斜纹缓慢漂移、金尘上浮、评级呼吸、高光定时扫过。
/// 离开时一道斜向暗幕扫过再关闭
fn summary_page(ui:&Ui,record:&Record,gain:Option<Gain>,focus:&mut usize,from_archive:bool,best:Option<erdtree_tower::score::Score>,elapsed:f64,cues:&mut Cues,icon:&mut dyn FnMut(u32)->Option<TextureId>,ready:bool)->bool {
    use hudhook::imgui::{ImColor32,WindowFlags};
    let size=ui.io().display_size;let s=(size[0]/1600.0).min(size[1]/900.0);
    let (ox,oy)=((size[0]-1600.0*s)*0.5,(size[1]-900.0*s)*0.5);let p=|x:f32,y:f32|[ox+x*s,oy+y*s];
    let gold=[0.91,0.71,0.29,1.0];let white=[0.93,0.91,0.86,1.0];let muted=[0.58,0.57,0.53,1.0];let dim=[0.38,0.37,0.35,1.0];let red=[0.9,0.36,0.3,1.0];
    let fade=|c:[f32;4],k:f32|[c[0],c[1],c[2],c[3]*k.clamp(0.0,1.0)];
    let rgba=|c:[f32;4],a:f32|ImColor32::from_rgba((c[0]*255.0) as u8,(c[1]*255.0) as u8,(c[2]*255.0) as u8,(a.clamp(0.0,1.0)*255.0) as u8);
    // 笔画字的淡入：不透明、颜色从背景色过渡（半透明的笔画在转角重叠处会叠出深色方块）
    let solid=|c:[f32;4],k:f32|->[f32;4] {let k=k.clamp(0.0,1.0);let bg=[0.047,0.047,0.043];[bg[0]+(c[0]-bg[0])*k,bg[1]+(c[1]-bg[1])*k,bg[2]+(c[2]-bg[2])*k,if k>0.0{1.0}else{0.0}]};
    let big=Big::new(ui);
    let time=ui.time() as f32;let dt=(elapsed-cues.last_frame).clamp(0.0,0.1) as f32;cues.last_frame=elapsed;
    let score=erdtree_tower::score::Score::of(record);let total=score.total();let rank=score.rank();
    let (won,dead)=(record.result==Outcome::Won,record.result==Outcome::Dead);
    let review=erdtree_tower::journal::review(record);
    // 新纪录：先比评级、再比评分，超过往期最好的一局（第一局只要立了律法或走出第一幕也算）
    let fresh=!from_archive&&!record.debug&&best.map_or(score.key()>(1,0),|b|score.key()>b.key());
    let digits:Vec<u32>=total.to_string().chars().filter_map(|c|c.to_digit(10)).collect();
    // 时间轴（秒）：数字停完后高光扫过，再砸评级
    let last_lock=lock_time(digits.len()-1);let sweep_at=last_lock+0.12;let stamp=last_lock+0.3;
    let status_at=stamp+0.12;let record_at=stamp+0.32;let lower=stamp+0.38;let buttons=stamp+0.62;
    cues.once(CUE_OPEN,"open");
    for (i,_) in digits.iter().enumerate() {
        if elapsed>=lock_time(i)&&cues.locked&(1<<i)==0 {cues.locked|=1<<i;crate::sfx::play(["lock1","lock2","lock3","lock4","lock5","lock6"][i.min(5)]);}
    }
    if elapsed>=0.4&&elapsed<last_lock-0.05&&elapsed-cues.last_tick>=0.05 {cues.last_tick=elapsed;cues.ticks+=1;crate::sfx::play(["tick1","tick2","tick3"][(cues.ticks%3) as usize]);}
    if elapsed>=sweep_at {cues.once(CUE_SWEEP,"sweep");}
    if elapsed>=stamp {cues.once(CUE_STAMP,"stamp");}
    if fresh&&elapsed>=record_at {cues.once(CUE_RECORD,"record");}
    // ── 背景：压暗的原画、缓慢漂移的斜纹、上浮的金尘、四角暗角、右上角依次亮起的金色斜条 ──
    let background=ui.get_background_draw_list();crate::oath_art::background(&background,size);
    background.add_rect([0.0,0.0],size,ImColor32::from_rgba(12,12,11,(170.0+62.0*ease(elapsed,0.0,0.35)) as u8)).filled(true).build();
    let drift=(time*9.0*s)%(28.0*s);
    let mut x=-size[1]+drift;while x<size[0] {background.add_line([x,size[1]],[x+size[1],0.0],ImColor32::from_rgba(255,255,255,6)).thickness(1.0).build();x+=28.0*s;}
    for i in 0..46u32 {
        let speed=0.025+hash(i)*0.05;let life=((time*speed+hash(i+101)) % 1.0+1.0)%1.0;
        let x=hash(i+7)*size[0]+(time*0.6+i as f32).sin()*14.0*s;let y=size[1]*(1.0-life);
        let a=(life*(1.0-life)*4.0)*(0.10+0.22*hash(i+31))*ease(elapsed,0.2,0.8);
        background.add_circle([x,y],(1.0+hash(i+13)*1.8)*s,rgba(gold,a)).filled(true).build();
    }
    let edge=ImColor32::from_rgba(0,0,0,150);let clear=ImColor32::from_rgba(0,0,0,0);
    background.add_rect_filled_multicolor([0.0,0.0],[size[0]*0.18,size[1]],edge,clear,clear,edge);
    background.add_rect_filled_multicolor([size[0]*0.82,0.0],size,clear,edge,edge,clear);
    for i in 0..4 {let k=pop(elapsed,0.1+i as f64*0.06,0.3);let x0=1452.0+i as f32*30.0+(1.0-k)*30.0;background.add_polyline(vec![p(x0,26.0),p(x0+18.0,26.0),p(x0+6.0,46.0),p(x0-12.0,46.0)],rgba(gold,0.9*k.min(1.0))).filled(true).build();}
    drop(background);
    // 余烬焦点：方向键左右切换，鼠标移动时跟随悬停
    let count=record.embers.len();
    let closing=cues.closing.is_some();
    if count>0&&ready&&!closing {
        let pressed=|keys:&[Key]|keys.iter().any(|k|ui.is_key_pressed(*k));
        if pressed(&[Key::LeftArrow,Key::GamepadDpadLeft]) {*focus=(*focus+count-1)%count;}
        if pressed(&[Key::RightArrow,Key::GamepadDpadRight]) {*focus=(*focus+1)%count;}
    }
    *focus=(*focus).min(count.saturating_sub(1));
    let mut close=false;let mut copy=false;let mut hovered=None;
    let focus_target=170.0+*focus as f32*54.0;
    let focus_x=cues.focus_x.map_or(focus_target,|x|x+(focus_target-x)*(1.0-(-dt*18.0).exp()));cues.focus_x=Some(focus_x);
    let hover_t=cues.hover_t;
    ui.window("旅程结算##summary").position([0.0,0.0],Condition::Always).size(size,Condition::Always)
        .flags(WindowFlags::NO_DECORATION|WindowFlags::NO_BACKGROUND|WindowFlags::NO_MOVE|WindowFlags::NO_NAV|WindowFlags::NO_SCROLL_WITH_MOUSE).build(|| {
        // 文字用黑体（黑体子集里缺字时退回宋体）；scale 按 20 像素的宋体算，黑体按字号换算成同样大小
        let font_for=|value:&str|->(Option<hudhook::imgui::FontStackToken<'_>>,f32) {
            match crate::fancy_text::button_font().filter(|_|crate::fancy_text::sans_covers(value)) {
                Some(font)=>(Some(ui.push_font(font)),20.0/crate::fancy_text::SANS_PIXELS),None=>(None,1.0)}
        };
        let text=|x:f32,y:f32,value:&str,color:[f32;4],scale:f32,align:f32| {
            if color[3]<=0.01 {return;}
            let (_font,k)=font_for(value);
            ui.set_window_font_scale(scale*k*s);ui.set_cursor_screen_pos([ox+x*s-ui.calc_text_size(value)[0]*align,oy+y*s]);ui.text_colored(color,value);
        };
        // 量一段文字的宽度（设计单位），与 text 用同一种字体
        let measure=|value:&str,scale:f32|->f32 {let (_font,k)=font_for(value);ui.set_window_font_scale(scale*k*s);ui.calc_text_size(value)[0]/s};
        // 逐字打出：已出现的字正常，正在出现的那个字淡入
        let typed=|x:f32,y:f32,value:&str,color:[f32;4],scale:f32,start:f64,rate:f64| {
            let shown=((elapsed-start)*rate).max(0.0);
            let (_font,fk)=font_for(value);
            ui.set_window_font_scale(scale*fk*s);
            // 每个字的位置按前面整段的宽度算（逐字累加会因取整越来越宽）
            for (i,(at,c)) in value.char_indices().enumerate() {
                let k=(shown-i as f64).clamp(0.0,1.0) as f32;if k<=0.0 {break;}
                let cx=x+ui.calc_text_size(&value[..at])[0]/s;
                ui.set_cursor_screen_pos([ox+cx*s,oy+(y+(1.0-k)*6.0)*s]);ui.text_colored(fade(color,k*color[3]),&value[at..at+c.len_utf8()]);
            }
        };
        // 从中间往两头长出来的细线，生长的两端带一点亮光
        let grow_line=|x0:f32,x1:f32,y:f32,alpha:f32,k:f32| {
            if k<=0.0 {return;}
            let (c,half)=((x0+x1)*0.5,(x1-x0)*0.5*k);let draw=ui.get_window_draw_list();
            draw.add_line(p(c-half,y),p(c+half,y),rgba(white,alpha)).thickness(s).build();
            if k<1.0 {for end in [c-half,c+half] {draw.add_circle(p(end,y),2.2*s,rgba(gold,0.9*(1.0-k))).filled(true).build();}}
        };
        // 评级砸下时左边一震
        let shake=if elapsed>stamp&&elapsed<stamp+0.26 {((elapsed-stamp)*95.0).sin() as f32*5.0*(1.0-((elapsed-stamp)/0.26) as f32)}else{0.0};
        // ── 左：本局评分 ──
        let k=ease(elapsed,0.05,0.35);
        text(110.0-(1.0-k)*40.0,142.0,"本局评分",fade(gold,k),1.5,0.0);
        {let draw=ui.get_window_draw_list();let w=96.0*ease(elapsed,0.12,0.4);
         draw.add_rect(p(110.0,184.0),p(110.0+w,187.0),rgba(gold,k)).filled(true).build();
         if w>0.0&&w<96.0 {draw.add_rect(p(110.0+w-10.0,183.0),p(110.0+w,188.0),rgba([1.0,0.95,0.8,1.0],1.0)).filled(true).build();}}
        // 大数字：每一位在自己的格子里竖向滚动，停下时带一点回弹；停之前偏暗，停下后变亮
        let height=if digits.len()>3 {166.0}else{196.0};let number_top=214.0;let number_x=110.0+shake;
        let cell=big.cell(height);let appear=ease(elapsed,0.2,0.3);
        if appear>0.0 {
            for (i,&digit) in digits.iter().enumerate() {
                let lock=lock_time(i);let x=number_x+i as f32*cell;
                let (clip_min,clip_max)=(p(x-4.0,number_top-8.0),p(x+cell+4.0,number_top+height+8.0));
                if elapsed>=lock {
                    let after=(elapsed-lock) as f32;let bounce=height*0.09*(-after*14.0).exp()*(after*30.0).cos();
                    let flash=(1.0-after/0.25).max(0.0);
                    let color=[white[0]+(1.0-white[0])*flash,white[1]+(0.95-white[1])*flash,white[2]+(0.75-white[2])*flash,1.0];
                    clip(clip_min,clip_max,||big.draw(ui,char::from_digit(digit,10).unwrap_or('0'),ox+x*s,oy+(number_top+bounce)*s,height*s,solid(color,appear)));
                    // 停下的那一下：格子底下闪一道金线
                    if flash>0.0 {ui.get_window_draw_list().add_rect(p(x+cell*0.08,number_top+height+12.0),p(x+cell*0.92,number_top+height+15.0),rgba(gold,flash)).filled(true).build();}
                } else {
                    let position=roll(digit,lock,elapsed);let shown=position.floor();let offset=position-shown;
                    let (now,next)=((shown as i64).rem_euclid(10) as u32,((shown as i64+1).rem_euclid(10)) as u32);
                    let color=solid([0.62,0.6,0.56,1.0],appear);
                    clip(clip_min,clip_max,|| {
                        big.draw(ui,char::from_digit(now,10).unwrap_or('0'),ox+x*s,oy+(number_top-offset*height*1.15)*s,height*s,color);
                        big.draw(ui,char::from_digit(next,10).unwrap_or('0'),ox+x*s,oy+(number_top+(1.0-offset)*height*1.15)*s,height*s,color);
                    });
                }
            }
            // 高光：所有数字停下后扫过一次，之后每 6 秒一次；只照亮笔画（在一条窄带里用亮色重画一遍数字）
            let since=elapsed-sweep_at;
            let phase=if since<0.0 {-1.0}else if since<0.6 {since/0.6}else{((since-0.6)%6.0)/0.6};
            if (0.0..1.0).contains(&phase) {
                let width=cell*digits.len() as f32;let x0=number_x-50.0+(width+100.0)*phase as f32;
                let text:String=digits.iter().map(|d|char::from_digit(*d,10).unwrap_or('0')).collect();
                for (half,light) in [(26.0,[1.0,0.97,0.86,1.0]),(12.0,[1.0,1.0,0.96,1.0])] {
                    clip(p(x0-half,number_top-8.0),p(x0+half,number_top+height+8.0),||big.draw_text(ui,&text,ox+number_x*s,oy+number_top*s,height*s,light));
                }
            }
        }
        // 评级：放大落下、越过一点再回来；落下瞬间闪光、冲击环、碎片四散、速度线
        let badge=(110.0+big.width(digits.len(),height)+36.0).min(840.0)+shake;let side=height*0.86;let badge_top=number_top+height-side;
        let (cx,cy)=(badge+side*0.5,badge_top+side*0.5);
        if elapsed>=stamp-0.12 {
            let draw=ui.get_window_draw_list();
            // 落下前一瞬：地面上的阴影收拢
            let pre=((elapsed-(stamp-0.12))/0.12).clamp(0.0,1.0) as f32;
            if elapsed<stamp+0.1 {let h=side*0.5*(1.6-0.6*pre);draw.add_rect(p(cx-h,cy-h),p(cx+h,cy+h),ImColor32::from_rgba(0,0,0,(110.0*pre*(1.0-ease(elapsed,stamp,0.1))) as u8)).filled(true).build();}
            if elapsed>=stamp {
                let k=pop(elapsed,stamp,0.26);let fall=ease(elapsed,stamp,0.12);let grow=1.0+(1.0-fall)*0.9+(k-fall).max(-0.15)*0.0;
                let settle=if elapsed>stamp+0.12 {1.0+(1.0-k)*0.0-(pop(elapsed,stamp+0.12,0.2)-1.0)*0.12}else{grow};
                let half=side*0.5*settle;
                let flash=(1.0-ease(elapsed,stamp+0.1,0.35))*f32::from(elapsed>=stamp+0.1);
                // 呼吸的外发光（入场后一直在）
                let breathe=0.5+0.5*(time*2.2).sin();
                for ring in 1..=3 {let h=half+ring as f32*5.0;draw.add_rect(p(cx-h,cy-h),p(cx+h,cy+h),rgba(gold,(0.06+0.05*breathe)*fall/ring as f32)).thickness(3.0*s).build();}
                if flash>0.0 {draw.add_rect(p(cx-half,cy-half),p(cx+half,cy+half),rgba([1.0,0.92,0.7,1.0],0.3*flash)).filled(true).build();}
                draw.add_rect(p(cx-half,cy-half),p(cx+half,cy+half),rgba(gold,fall)).thickness(4.0*s).build();
                let ring=((elapsed-stamp-0.05)/0.55) as f32;
                if (0.0..1.0).contains(&ring) {
                    let h=side*0.5*(1.0+0.9*ring);draw.add_rect(p(cx-h,cy-h),p(cx+h,cy+h),rgba(gold,0.8*(1.0-ring))).thickness((3.0-2.0*ring)*s).build();
                    // 速度线：八个方向的短线向外射出
                    for d in 0..8 {
                        let a=d as f32*std::f32::consts::FRAC_PI_4+0.39;let (dx,dy)=(a.cos(),a.sin());
                        let r0=side*(0.62+0.7*ring);let r1=r0+side*0.28*(1.0-ring);
                        draw.add_line(p(cx+dx*r0,cy+dy*r0),p(cx+dx*r1,cy+dy*r1),rgba([1.0,0.9,0.6,1.0],0.85*(1.0-ring))).thickness(2.2*s).build();
                    }
                }
                // 碎片：十四片金色小三角，飞出、旋转、变小、淡出
                let t=((elapsed-stamp)/0.75) as f32;
                if (0.0..1.0).contains(&t) {
                    for i in 0..14u32 {
                        let a=hash(i+200)*std::f32::consts::TAU;let speed=side*(0.7+hash(i+300)*0.9);
                        let travel=speed*(1.0-(1.0-t).powi(2));let (px,py)=(cx+a.cos()*travel,cy+a.sin()*travel+side*0.25*t*t);
                        let r=(5.0+hash(i+400)*6.0)*(1.0-t);let spin=a+t*(6.0+hash(i+500)*8.0);
                        let tri:Vec<[f32;2]>=(0..3).map(|j|{let b=spin+j as f32*2.1;p(px+b.cos()*r,py+b.sin()*r*0.6)}).collect();
                        draw.add_polyline(tri,rgba(gold,1.0-t)).filled(true).build();
                    }
                }
                drop(draw);
                let h=side*0.6*settle;
                big.draw(ui,rank,ox+(cx-big.cell(h)*0.5)*s,oy+(cy-h*0.5)*s,h*s,solid(gold,fall));
            }
        }
        // 结果一行：逐字打出
        let mut status=format!("{}  ·  {}  ·  第{}幕 第{}层",if won{"登顶成功"}else if dead{"旅程终结"}else{"暂离结算"},crate::map_ui::class_name(record.class),
            ["一","二","三"][usize::from(record.act.clamp(1,3)-1)],record.layer);
        if won {status=format!("登顶成功  ·  {}  ·  三幕全部通关",crate::map_ui::class_name(record.class));}
        let status_y=number_top+height+30.0;
        typed(110.0,status_y,&status,white,1.3,status_at,55.0);
        let tag_x=110.0+measure(&status,1.3)+24.0;
        let k=ease(elapsed,status_at,0.3);
        if fresh&&elapsed>=record_at {
            // 新纪录：带回弹弹出、闪白，之后每隔一会儿有一道高光扫过
            let k=pop(elapsed,record_at,0.32);let grow=0.55+0.45*k;
            let w=measure("新纪录",1.0)+24.0;
            let (tx,ty)=(tag_x+w*0.5,status_y+16.5);let (hw,hh)=(w*0.5*grow,15.5*grow);
            let draw=ui.get_window_draw_list();
            let flash=1.0-ease(elapsed,record_at+0.08,0.3);
            draw.add_rect(p(tx-hw,ty-hh),p(tx+hw,ty+hh),rgba(gold,k.min(1.0))).filled(true).build();
            if flash>0.0 {draw.add_rect(p(tx-hw,ty-hh),p(tx+hw,ty+hh),rgba([1.0,1.0,1.0,1.0],0.6*flash)).filled(true).build();
                let h=1.0+0.6*(1.0-flash);draw.add_rect(p(tx-hw*h,ty-hh*h),p(tx+hw*h,ty+hh*h),rgba(gold,0.7*flash)).thickness(1.5*s).build();}
            let sweep=(((elapsed-record_at-0.4)/2.2).fract() as f32)*1.8-0.4;
            if elapsed>record_at+0.4&&(0.0..1.0).contains(&sweep) {
                let x0=tx-hw+sweep*hw*2.0;
                draw.with_clip_rect_intersect(p(tx-hw,ty-hh),p(tx+hw,ty+hh),|| {
                    draw.add_polyline(vec![p(x0,ty-hh),p(x0+12.0,ty-hh),p(x0+2.0,ty+hh),p(x0-10.0,ty+hh)],ImColor32::from_rgba(255,248,220,130)).filled(true).build();
                });
            }
            drop(draw);
            if grow>0.8 {text(tag_x+12.0,status_y+4.0,"新纪录",fade([0.07,0.07,0.06,1.0],(grow-0.8)*5.0),1.0,0.0);}
        } else if let Some(best)=best.filter(|_|!from_archive&&!fresh) {text(tag_x,status_y+4.0,&format!("历史最高 {} · {}",best.rank(),best.total()),fade(muted,k),0.95,0.0);}
        let mut y=status_y+42.0;
        if record.debug {typed(110.0,y,"调试旅程，不计入纪录",muted,0.95,status_at+0.2,60.0);y+=32.0;}
        if let Some(fatal)=&review.fatal {typed(110.0,y,&format!("倒在 {} 面前",fatal.enemies.join("、")),red,1.0,status_at+0.25,50.0);}
        // 余烬：图标依次带回弹弹出，选中下划线滑到选中的图标下
        let row_y=number_top+height+150.0;
        let k=ease(elapsed,lower,0.3);let slide=(1.0-k)*30.0;
        text(110.0-slide,row_y+10.0,"余烬",fade(muted,k),1.05,0.0);
        if count==0 {text(170.0-slide,row_y+11.0,"无",fade(dim,k),0.95,0.0);}
        for (i,ember) in record.embers.iter().enumerate() {
            let ki=pop(elapsed,lower+0.05+i as f64*0.05,0.32);if ki<=0.0 {continue;}
            let (mx,my)=(192.0+i as f32*54.0,row_y+22.0);let half=22.0*ki;
            let (min,max)=(p(mx-half,my-half),p(mx+half,my+half));
            let selected=i==*focus;
            let draw=ui.get_window_draw_list();
            match crate::reward_ui::ember_icon(*ember).and_then(&mut *icon) {
                Some(texture)=>{let light=if selected{255}else{165};draw.add_image(texture,min,max).col(ImColor32::from_rgba(light,light,light,(255.0*ki.min(1.0)) as u8)).build();},
                None=>{draw.add_circle(p(mx,my),20.0*s*ki,rgba([0.47,0.24,0.12,1.0],ki)).filled(true).build();},
            }
            drop(draw);
            if ready&&ui.io().mouse_delta!=[0.0,0.0]&&ui.is_mouse_hovering_rect(min,max) {*focus=i;}
        }
        if count>0 {
            let ku=ease(elapsed,lower+0.2,0.3);
            ui.get_window_draw_list().add_rect(p(focus_x-3.0,row_y+48.0),p(focus_x+47.0,row_y+50.0),rgba(gold,ku)).filled(true).build();
        }
        if let Some(ember)=record.embers.get(*focus) {
            let k=ease(elapsed,lower+0.25,0.3);
            text(110.0-slide,row_y+60.0,ember.name(),fade(gold,k),0.95,0.0);
            let indent=110.0+measure(ember.name(),0.95)+14.0;
            text(indent-slide,row_y+61.0,&ember.effect().replace('\n',"，"),fade(muted,k),0.9,0.0);
        }
        // 誓约：菱形徽记依次放大出现
        let oath_y=row_y+108.0;let k=ease(elapsed,lower+0.15,0.3);let slide=(1.0-k)*30.0;
        text(110.0-slide,oath_y+1.0,"誓约",fade(muted,k),1.05,0.0);
        if record.oaths.is_empty() {text(170.0-slide,oath_y+2.0,"无",fade(dim,k),0.95,0.0);}
        let mut ox2=170.0;
        for (i,oath) in record.oaths.iter().enumerate() {
            let ki=pop(elapsed,lower+0.2+i as f64*0.07,0.3);
            if ki>0.05 {oath_badge(ui,p(ox2+13.0,oath_y+15.0),13.0*s*ki,if ki>0.7{ROMAN[usize::from(oath.level.min(3))]}else{""},true,s);}
            text(ox2+34.0,oath_y+2.0,oath.kind.name(),fade(white,ki.min(1.0)),0.95,0.0);
            ox2+=34.0+measure(oath.kind.name(),0.95)+28.0;
        }
        // 职业熟练度（局外养成）：逐字打出
        if let Some(gain)=gain {
            let class=crate::map_ui::class_name(gain.class);
            let unlocked:Vec<_>=gain.unlocked().into_iter().map(|id|crate::reward_ui::talisman_description(id).0).collect();
            let line=if !unlocked.is_empty() {format!("{class}熟练度 +{} · 升到 Lv {}，解锁开局护符「{}」",gain.xp,gain.after,unlocked.join("」「"))}
                else if gain.after>gain.before {format!("{class}熟练度 +{} · 升到 Lv {}",gain.xp,gain.after)}
                else {format!("{class}熟练度 +{} · Lv {}",gain.xp,gain.after)};
            typed(110.0,oath_y+54.0,&line,if unlocked.is_empty(){muted}else{gold},0.95,lower+0.35,70.0);
        }
        // ── 右：苦难律法与分数构成（从上往下擦开，擦开的边缘一道金线） ──
        let (left,right,top,bottom)=(990.0,1510.0,96.0,772.0);
        let wipe=ease(elapsed,0.0,0.55);let edge_y=top+(bottom-top)*wipe;
        {let draw=ui.get_window_draw_list();
         draw.add_rect(p(left,top),p(right,edge_y),rgba(white,0.22)).thickness(s).build();
         draw.add_rect(p(left,top),p(left+4.0,top+86.0*ease(elapsed,0.15,0.35)),rgba(gold,1.0)).filled(true).build();
         if wipe<1.0 {draw.add_rect(p(left,edge_y-1.5),p(right,edge_y+1.5),rgba(gold,0.9)).filled(true).build();
            draw.add_rect_filled_multicolor(p(left,edge_y-26.0),p(right,edge_y),rgba(gold,0.0),rgba(gold,0.0),rgba(gold,0.12),rgba(gold,0.12));}}
        // 擦开范围以外的内容不画（文字也画在窗口绘制列表里，用底层的裁剪栈）
        let (clip_min,clip_max)=(p(left-2.0,top-2.0),p(right+60.0,edge_y+2.0));
        unsafe {hudhook::imgui::sys::igPushClipRect(hudhook::imgui::sys::ImVec2{x:clip_min[0],y:clip_min[1]},hudhook::imgui::sys::ImVec2{x:clip_max[0],y:clip_max[1]},true);}
        let k=ease(elapsed,0.15,0.3);
        text(left+30.0-(1.0-k)*20.0,top+20.0,"苦难律法",fade(white,k),1.9,0.0);
        // 律法等级（老练、艰难、噩梦、危机）写在标题右边；分数就是左边的本局评分
        let grade=if record.law.is_empty() {"未立律法"}else{record.law.grade().unwrap_or("普通")};
        text(right-30.0+(1.0-k)*20.0,top+26.0,grade,fade(if record.law.is_empty(){dim}else{gold},k),1.6,1.0);
        let detail=if record.law.is_empty() {"本局没有立下苦难律法".to_string()}else{format!("{} 项  ·  律法码 {}",record.law.selected().len(),record.law.code())};
        typed(left+30.0,top+94.0,&detail,muted,1.0,0.3,60.0);
        grow_line(left+30.0,right-30.0,top+128.0,0.18,ease(elapsed,0.3,0.4));
        // 接受的律法节点：像翻牌一样依次翻开（节点多时缩小、只留图标）
        let nodes:Vec<usize>=(0..erdtree_tower::law::NODES.len()).filter(|&i|record.law.has(i)).collect();
        let named=nodes.len()<=18;let columns=if named{6}else{9};
        let cell=(right-left-60.0)/columns as f32;let tile=cell-12.0;let row=if named{tile+34.0}else{tile+10.0};
        if nodes.is_empty() {text((left+right)*0.5,top+230.0,"本局没有接受律法",fade(dim,k),0.95,0.5);}
        for (i,&id) in nodes.iter().enumerate() {
            let node=&erdtree_tower::law::NODES[id];
            let start=0.42+i as f64*0.045;let flip=ease(elapsed,start,0.3);if flip<=0.0 {continue;}
            let (cx,cy)=(left+30.0+cell*(i%columns) as f32,top+146.0+row*(i/columns) as f32);
            let (mx,my)=(cx+tile*0.5,cy+tile*0.5);let (hw,hh)=(tile*0.5*(flip*std::f32::consts::FRAC_PI_2).sin().max(0.04),tile*0.5);
            let (min,max)=(p(mx-hw,my-hh),p(mx+hw,my+hh));
            let draw=ui.get_window_draw_list();
            let back=flip<0.5;
            draw.add_rect(min,max,if back{rgba(gold,0.85)}else{ImColor32::from_rgba(26,25,23,255)}).filled(true).build();
            if !back {
                draw.add_rect(min,max,rgba(white,0.16)).thickness(s).build();
                let bar=ease(elapsed,start+0.15,0.2);draw.add_rect(min,[min[0]+3.0*s,min[1]+(max[1]-min[1])*bar],rgba(gold,1.0)).filled(true).build();
                let pad=tile*0.22;crate::law_art::draw_line(&draw,node.art,p(mx-hw+pad*hw/(tile*0.5),my-hh+pad),p(mx+hw-pad*hw/(tile*0.5),my+hh-pad),240);
                let glint=1.0-ease(elapsed,start+0.15,0.3);if glint>0.0 {draw.add_rect(min,max,rgba([1.0,0.95,0.8,1.0],0.4*glint)).filled(true).build();}
            }
            drop(draw);
            if named {
                let kn=ease(elapsed,start+0.2,0.25);let fit=(cell/measure(node.name,0.9)).min(1.0);
                text(cx+tile*0.5,cy+tile+5.0,node.name,fade(muted,kn),0.9*fit,0.5);
            }
            if ready&&ui.is_mouse_hovering_rect(min,max) {ui.tooltip_text(format!("{} · {}",node.name,node.effect));}
        }
        // 旅程统计：一行一行落下（落下时背后闪一道亮条、数字从 0 涨上去），最后一行落定时一响
        let lines=score.lines();let row_h=38.0;
        let mut y=bottom-30.0-row_h*lines.len() as f32;
        text(left+30.0,y-36.0,"旅程",fade(muted,ease(elapsed,0.7,0.3)),1.0,0.0);
        grow_line(left+30.0,right-30.0,y-8.0,0.18,ease(elapsed,0.7,0.35));
        let mut at=0.78;
        for (name,amount) in lines {
            let k=ease(elapsed,at,0.25);let dx=(1.0-k)*30.0;
            let glow=1.0-ease(elapsed,at+0.05,0.4);
            if k>0.0&&glow>0.0 {ui.get_window_draw_list().add_rect_filled_multicolor(p(left+20.0,y-4.0),p(right-20.0,y+row_h-8.0),rgba(gold,0.16*glow),rgba(gold,0.0),rgba(gold,0.0),rgba(gold,0.16*glow));}
            let n=ease(elapsed,at,0.35);
            text(left+30.0+dx,y,name,fade(muted,k),1.15,0.0);
            text(right-30.0+dx,y,&((amount as f32*n).round() as u32).to_string(),fade(white,k),1.15,1.0);
            y+=row_h;at+=0.09;
        }
        if elapsed>=at+0.2 {cues.once(CUE_TOTAL,"total");}
        unsafe {hudhook::imgui::sys::igPopClipRect();}
        // ── 底：细线从中间长出，种子淡入，按钮依次从下方滑入 ──
        grow_line(60.0,1540.0,802.0,0.22,ease(elapsed,buttons-0.15,0.4));
        let k=ease(elapsed,buttons-0.05,0.3);
        let seed_label=if record.whole_seed{"种子"}else{"本幕种子"};
        text(110.0,828.0,seed_label,fade(muted,k),1.05,0.0);
        typed(110.0+measure(seed_label,1.05)+14.0,828.0,&record.seed.to_string(),white,1.05,buttons,80.0);
        // 按钮：黑体粗体、字距拉开（参考危机合约）；悬停时亮色从左往右铺满
        let button=|label:&str,key:crate::pad::Input,x0:f32,x1:f32,primary:bool,appear:f32,hover_k:f32|->(bool,bool) {
            let lift=(1.0-appear)*18.0;let (min,max)=(p(x0,816.0+lift),p(x1,868.0+lift));
            ui.set_cursor_screen_pos(min);ui.set_window_font_scale(s);let clicked=ui.invisible_button(label,[max[0]-min[0],max[1]-min[1]]);
            let hover=ui.is_item_hovered();let draw=ui.get_window_draw_list();let a=appear.min(1.0);
            let fill=min[0]+(max[0]-min[0])*hover_k;
            if primary {
                draw.add_rect(min,max,rgba(gold,0.9*a)).filled(true).build();
                if hover_k>0.0 {draw.add_rect(min,[fill,max[1]],rgba([1.0,0.82,0.42,1.0],a)).filled(true).build();}
            } else {
                if hover_k>0.0 {draw.add_rect(min,[fill,max[1]],rgba(white,0.12*a)).filled(true).build();}
                draw.add_rect(min,max,rgba(white,(0.42+0.33*hover_k)*a)).thickness(1.4*s).build();
            }
            drop(draw);
            let ink=fade(if primary{[0.06,0.06,0.05,1.0]}else{white},a);
            let font=crate::fancy_text::button_font().map(|f|ui.push_font(f));
            let scale=if font.is_some(){0.78*30.0/crate::fancy_text::SANS_PIXELS}else{1.15};let gap=(7.0+2.0*hover_k)*s;
            ui.set_window_font_scale(scale*s);
            let widths:Vec<f32>=label.chars().map(|c|ui.calc_text_size(c.to_string())[0]).collect();
            let width=widths.iter().sum::<f32>()+gap*(widths.len().saturating_sub(1)) as f32;
            let height=ui.calc_text_size(label)[1];
            // 按键标签（crate::pad，手柄 / 键盘随最后用的设备切换）：主按钮深底金字，次按钮浅底深字，悬停时略放大
            let h=(25.0+3.0*hover_k)*s;let badge=crate::pad::width(ui,key,h);let badge_gap=if badge>0.0 {12.0*s}else{0.0};
            let mut x=(min[0]+max[0]-width-badge-badge_gap)*0.5;let mid=(min[1]+max[1])*0.5;
            crate::pad::badge(ui,x,mid,h,key,if primary {crate::pad::Tone::Dark}else{crate::pad::Tone::Light},a);
            ui.set_window_font_scale(scale*s);
            x+=badge+badge_gap;let y=mid-height*0.5;
            ui.set_window_font_scale(scale*s);
            for (c,w) in label.chars().zip(&widths) {ui.set_cursor_screen_pos([x,y]);ui.text_colored(ink,c.to_string());x+=w+gap;}
            drop(font);
            (clicked,hover)
        };
        let (primary,key,keys):(&str,crate::pad::Input,&[Key])=if from_archive {("返回档案",crate::pad::BACK,&[Key::Escape,Key::GamepadFaceRight,Key::Enter,Key::GamepadFaceDown])}else{("返回大赐福",crate::pad::CONFIRM,&[Key::Enter,Key::GamepadFaceDown])};
        let (clicked,hover)=button(primary,key,1112.0,1342.0,true,pop(elapsed,buttons,0.35),hover_t[0]);if hover{hovered=Some(0);}
        close=clicked||keys.iter().any(|k|ui.is_key_pressed_no_repeat(*k));
        let (clicked,hover)=button("复制种子",crate::pad::Input::new(crate::pad::Pad::Y,""),1362.0,1540.0,false,pop(elapsed,buttons+0.08,0.35),hover_t[1]);if hover{hovered=Some(1);}
        if clicked||ui.is_key_pressed_no_repeat(Key::GamepadFaceUp) {ui.set_clipboard_text(record.seed.to_string());copy=true;}
    });
    // 按钮悬停的过渡（约 0.15 秒铺满）与音效
    for (i,t) in cues.hover_t.iter_mut().enumerate() {let target=if hovered==Some(i as u8)&&ready{1.0}else{0.0};*t+=(target-*t)*(1.0-(-dt*16.0).exp());}
    if ready&&!closing&&hovered.is_some()&&hovered!=cues.hover {crate::sfx::play("hover");}
    cues.hover=hovered;
    if copy&&!closing {crate::sfx::play("confirm");}
    // 离场：一道带金边的斜向暗幕从右往左扫过，扫完才关闭
    if close&&ready&&!closing {crate::sfx::play("confirm");crate::sfx::play("close");cues.closing=Some(elapsed);}
    if let Some(start)=cues.closing {
        let k=ease(elapsed,start,EXIT);let draw=ui.get_foreground_draw_list();
        let slant=size[1]*0.35;let x=size[0]+slant-(size[0]+slant*2.0)*k;
        draw.add_polyline(vec![[x,0.0],[size[0]+slant,0.0],[size[0]+slant,size[1]],[x-slant,size[1]]],ImColor32::from_rgba(8,8,7,250)).filled(true).build();
        draw.add_line([x,0.0],[x-slant,size[1]],rgba(gold,1.0)).thickness(3.0*s).build();
        return elapsed>=start+EXIT;
    }
    false
}
/// 誓约等级徽记：菱形底，罗马数字写等级；lit 为 false 时是未解锁的灰色
pub(crate) fn oath_badge(ui:&Ui,c:[f32;2],h:f32,numeral:&str,lit:bool,s:f32) {
    use hudhook::imgui::ImColor32;
    let draw=ui.get_window_draw_list();
    let diamond=vec![[c[0],c[1]-h],[c[0]+h,c[1]],[c[0],c[1]+h],[c[0]-h,c[1]]];
    draw.add_polyline(diamond.clone(),if lit{ImColor32::from_rgba(74,50,28,255)}else{ImColor32::from_rgba(28,25,20,255)}).filled(true).build();
    let mut outline=diamond.clone();outline.push(diamond[0]);
    draw.add_polyline(outline,if lit{ImColor32::from_rgba(214,180,112,255)}else{ImColor32::from_rgba(90,80,62,200)}).thickness(1.6*s).build();drop(draw);
    if !numeral.is_empty() {
        ui.set_window_font_scale(0.85*h/19.0);let w=ui.calc_text_size(numeral);
        ui.set_cursor_screen_pos([c[0]-w[0]*0.5,c[1]-w[1]*0.5]);ui.text_colored(if lit{[0.93,0.9,0.81,1.0]}else{[0.45,0.43,0.38,1.0]},numeral);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seed_input_accepts_full_u64_without_float_precision_loss() {
        assert_eq!(parse_seed(" 18446744073709551615 "),Ok(u64::MAX));
        assert_eq!(parse_seed("0"),Ok(0));
        for text in ["","-1","1.5","1e3","18446744073709551616","abc"]{assert!(parse_seed(text).is_err());}
    }
}
