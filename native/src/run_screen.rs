use std::{collections::VecDeque,sync::{Mutex,atomic::{AtomicBool,Ordering}}};
use erdtree_tower::{Run,Phase};
use hudhook::imgui::{Ui,Key,Condition,StyleColor};

static SCREEN:Mutex<VecDeque<Screen>>=Mutex::new(VecDeque::new());
static PENDING:Mutex<Option<Run>>=Mutex::new(None);
static SEED:Mutex<Option<u64>>=Mutex::new(None);
static ACTIVE:AtomicBool=AtomicBool::new(false);
enum Screen {Summary{run:Run,opened:Option<f64>},Notice{message:String,opened:Option<f64>},TrialRewards{items:Vec<u32>,opened:Option<f64>},Seed{input:String,error:String,digit:u8,opened:Option<f64>}}
fn show(next:Screen) {
    let Ok(mut screens)=SCREEN.lock() else{return;};
    if screens.iter().any(|s|match (s,&next) {(Screen::Notice{message:a,..},Screen::Notice{message:b,..})=>a==b,(Screen::Seed{..},Screen::Seed{..})=>true,_=>false}) {return;}
    screens.push_back(next);ACTIVE.store(true,Ordering::Relaxed);
}
pub fn party_notice(message:String) {show(Screen::Notice{message,opened:None});}
pub fn trial_rewards(before:&Run,after:&Run) {
    let items=after.elite_buffs.iter().copied().filter(|id|!before.elite_buffs.contains(id)).collect();
    show(Screen::TrialRewards{items,opened:None});
}
pub fn active()->bool {ACTIVE.load(Ordering::Relaxed)}
pub fn trial_rewards_active()->bool {SCREEN.lock().is_ok_and(|s|s.iter().any(|s|matches!(s,Screen::TrialRewards{..})))}
pub fn summary(run:&Run) {if let Ok(mut pending)=PENDING.lock(){*pending=Some(run.clone());}}
pub fn hub_ready() {
    if let Ok(mut screen)=SCREEN.lock() {
        if !screen.is_empty(){return;}
        if let Some(run)=PENDING.lock().ok().and_then(|mut pending|pending.take()) {
            screen.push_back(Screen::Summary{run,opened:None});ACTIVE.store(true,Ordering::Relaxed);
        }
    }
}
pub fn seed_entry() {show(Screen::Seed{input:String::new(),error:String::new(),digit:0,opened:None});}
pub fn take_seed()->Option<u64> {SEED.lock().ok()?.take()}
fn parse_seed(text:&str)->Result<u64,&'static str> {
    let text=text.trim();
    if text.is_empty()||!text.bytes().all(|c|c.is_ascii_digit()){return Err("请输入数字种子");}
    text.parse().map_err(|_|"种子超出范围")
}
pub fn draw(ui:&Ui)->bool {
    let Ok(mut screen)=SCREEN.lock() else{return false;};
    let Some(current)=screen.front_mut() else{return false;};
    if render(ui,current) {screen.pop_front();ACTIVE.store(!screen.is_empty(),Ordering::Relaxed);}
    true
}
fn render(ui:&Ui,current:&mut Screen)->bool {
    if let Screen::Summary{run,opened}=current {let opened=*opened.get_or_insert(ui.time());return summary_page(ui,run,ui.time()-opened>=0.35);}
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
                ui.text("暂时无法开始联机旅程");ui.separator();ui.spacing();ui.text_wrapped(message);
                ui.spacing();ui.text("详细原因已记录到 tower-preview.log。");ui.spacing();
                if (ui.button("确定 [Enter / A]")||ui.is_key_pressed(Key::Enter)||ui.is_key_pressed(Key::GamepadFaceDown)||ui.is_key_pressed(Key::Escape)||ui.is_key_pressed(Key::GamepadFaceRight))&&ui.time()-opened>=0.35 {close=true;}
            },
            Screen::TrialRewards{items,opened}=>{
                let opened=*opened.get_or_insert(ui.time());
                ui.text("男生女生向前冲 · 挑战完成");ui.separator();ui.spacing();
                ui.text("已获得以下护符赐福");ui.spacing();
                for &id in items.iter() {
                    let (name,info)=crate::reward_ui::talisman_description(id);
                    ui.text_colored([0.9,0.78,0.5,1.0],name);ui.text_wrapped(info);ui.spacing();ui.separator();ui.spacing();
                }
                if items.is_empty(){ui.text_wrapped("已拥有全部可获得的护符赐福，本次没有新增赐福。");}
                ui.text_wrapped("本局持续生效，不占护符栏位，无需装备。");ui.spacing();
                let ready=ui.time()-opened>=0.35;
                if (ui.button("确定，继续选路  [Enter / A]")||ui.is_key_pressed(Key::Enter)||ui.is_key_pressed(Key::GamepadFaceDown))&&ready {close=true;}
            },
            Screen::Summary{..}|Screen::Seed{..}=>unreachable!(),
        }
    });
    close
}
#[cfg(test)]
pub fn test_frames(context:&mut hudhook::imgui::Context) {
    let mut run=Run::new(u64::MAX,erdtree_tower::Class::Samurai);run.phase=Phase::Dead;
    summary(&run);
    assert!(!active(),"Death must only queue the result, not capture input");
    assert!(!draw(context.frame()),"Death and loading must remain unobstructed");context.render();
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
        for mut screen in [Screen::Notice{message:"爬塔协议版本不匹配：本机 23，对方 22。\n请所有玩家更新同一版爬塔补丁。".into(),opened:None},Screen::Summary{run:run.clone(),opened:None},Screen::TrialRewards{items:vec![1000,1150],opened:None},Screen::TrialRewards{items:vec![],opened:None},Screen::Seed{input:u64::MAX.to_string(),error:String::new(),digit:9,opened:None}] {
            render(context.frame(),&mut screen);
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
        text(mid,556.0,"手柄：方向键选择  ·  A 按下  ·  Y 退格  ·  X 开始  ·  B 取消",muted,0.85);
        ui.set_window_font_scale(s);
        let button=|label:&str,x:f32| {
            let pos=[x,top+602.0*s];let extent=[320.0*s,65.0*s];
            ui.set_cursor_screen_pos(pos);let clicked=ui.invisible_button(label,extent);
            crate::oath_art::button(&ui.get_window_draw_list(),pos,[pos[0]+extent[0],pos[1]+extent[1]],if ui.is_item_hovered(){255}else{215});
            text(x+extent[0]*0.5,622.0,label,gold,1.0);clicked
        };
        start|=button("开始旅程  [Enter / X]",mid-340.0*s)&&ready;
        close|=button("取消  [Esc / B]",mid+20.0*s)&&ready;
    });
    if let Some(i)=pressed {seed_key(input,error,SEED_KEYS[i]);}
    if start&&!close {
        match parse_seed(input) {Ok(seed)=>{if let Ok(mut pending)=SEED.lock(){*pending=Some(seed);return true;}},Err(message)=>*error=message.into()}
    }
    close
}
fn summary_page(ui:&Ui,run:&Run,ready:bool)->bool {
    let size=ui.io().display_size;
    let s=(size[0]/1600.0).min(size[1]/900.0);
    let mid=size[0]*0.5;let top=(size[1]-690.0*s)*0.5;
    let gold=[0.84,0.72,0.46,1.0];let muted=[0.64,0.61,0.52,1.0];let white=[0.92,0.88,0.77,1.0];
    crate::oath_art::background(&ui.get_background_draw_list(),size);
    let mut close=false;
    ui.window("旅程结算##summary").position([0.0,0.0],Condition::Always).size(size,Condition::Always)
        .flags(hudhook::imgui::WindowFlags::NO_DECORATION|hudhook::imgui::WindowFlags::NO_BACKGROUND|hudhook::imgui::WindowFlags::NO_MOVE|hudhook::imgui::WindowFlags::NO_NAV).build(|| {
        let text=|x:f32,y:f32,value:&str,color:[f32;4],scale:f32| {
            ui.set_window_font_scale(scale*s);ui.set_cursor_pos([x-ui.calc_text_size(value)[0]*0.5,top+y*s]);ui.text_colored(color,value);
        };
        text(mid,12.0,crate::map_ui::class_name(run.class),muted,1.0);
        text(mid,57.0,if run.phase==Phase::Won{"登 顶 成 功"}else if run.phase==Phase::Dead{"旅 程 终 结"}else{"暂 离 结 算 · 可 继 续"},gold,2.1);
        let layer=run.visited.last().and_then(|&id|run.nodes.get(id)).map_or(1,|node|node.layer+1);
        text(mid,130.0,&format!("第 {} 幕  ·  第 {} 层",run.act,run.reached_layer.max(layer)),white,1.25);
        let draw=ui.get_window_draw_list();
        for y in [194.0,358.0,463.0] {draw.add_line([mid-410.0*s,top+y*s],[mid+410.0*s,top+y*s],hudhook::imgui::ImColor32::from_rgba(171,143,87,100)).build();}
        drop(draw);
        for (i,(label,value)) in [("完成节点",run.completed_nodes as usize),("完成战斗",run.completed_battles as usize),("获得物品",run.claimed_items.len())].into_iter().enumerate() {
            let x=mid+(i as f32-1.0)*275.0*s;
            text(x,224.0,&value.to_string(),white,2.1);text(x,292.0,label,muted,1.0);
        }
        text(mid-210.0*s,392.0,&format!("圣杯上限  红 +{}  /  蓝 +{}",run.flask_bonus,run.flask_bonus),gold,1.0);
        text(mid+210.0*s,392.0,&format!("记忆栏位  +{}",run.memory_bonus),gold,1.0);
        let seed=run.initial_seed.unwrap_or(run.seed);
        text(mid,487.0,if run.initial_seed.is_some()||run.act==1{"本局种子"}else{"本幕种子"},muted,0.9);
        text(mid,523.0,&seed.to_string(),white,1.25);
        ui.set_window_font_scale(s);
        let button=|label:&str,x:f32| {
            let pos=[x,top+602.0*s];let extent=[320.0*s,65.0*s];
            ui.set_cursor_screen_pos(pos);let clicked=ui.invisible_button(label,extent);
            crate::oath_art::button(&ui.get_window_draw_list(),pos,[pos[0]+extent[0],pos[1]+extent[1]],if ui.is_item_hovered(){255}else{215});
            text(x+extent[0]*0.5,622.0,label,gold,1.0);clicked
        };
        if button("复制种子  [Y]",mid-340.0*s)||ui.is_key_pressed_no_repeat(Key::GamepadFaceUp){ui.set_clipboard_text(seed.to_string());}
        close=button("返回大赐福  [A]",mid+20.0*s)||ui.is_key_pressed_no_repeat(Key::Enter)||ui.is_key_pressed_no_repeat(Key::GamepadFaceDown);
    });
    close&&ready
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
