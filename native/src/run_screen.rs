use std::sync::{Mutex,atomic::{AtomicBool,Ordering}};
use erdtree_tower::{Run,Phase};
use hudhook::imgui::{Ui,Key,Condition,StyleColor};

static SCREEN:Mutex<Option<Screen>>=Mutex::new(None);
static PENDING:Mutex<Option<Run>>=Mutex::new(None);
static SEED:Mutex<Option<u64>>=Mutex::new(None);
static ACTIVE:AtomicBool=AtomicBool::new(false);
enum Screen {Summary(Run),Notice{message:String,opened:Option<f64>},TrialRewards{items:Vec<u32>,opened:Option<f64>},Seed{input:String,error:String,digit:u8}}
pub fn party_notice(message:String) {
    if let Ok(mut screen)=SCREEN.lock(){*screen=Some(Screen::Notice{message,opened:None});ACTIVE.store(true,Ordering::Relaxed);}
}
pub fn trial_rewards(before:&Run,after:&Run) {
    let items=after.elite_buffs.iter().copied().filter(|id|!before.elite_buffs.contains(id)).collect();
    if let Ok(mut screen)=SCREEN.lock(){*screen=Some(Screen::TrialRewards{items,opened:None});ACTIVE.store(true,Ordering::Relaxed);}
}
pub fn active()->bool {ACTIVE.load(Ordering::Relaxed)}
pub fn trial_rewards_active()->bool {SCREEN.lock().is_ok_and(|s|matches!(*s,Some(Screen::TrialRewards{..})))}
pub fn summary(run:&Run) {if let Ok(mut pending)=PENDING.lock(){*pending=Some(run.clone());}}
pub fn hub_ready() {
    if let Ok(mut screen)=SCREEN.lock() {
        if screen.is_some(){return;}
        if let Some(run)=PENDING.lock().ok().and_then(|mut pending|pending.take()) {
            *screen=Some(Screen::Summary(run));ACTIVE.store(true,Ordering::Relaxed);
        }
    }
}
pub fn seed_entry() {if let Ok(mut screen)=SCREEN.lock(){*screen=Some(Screen::Seed{input:String::new(),error:String::new(),digit:0});ACTIVE.store(true,Ordering::Relaxed);}}
pub fn take_seed()->Option<u64> {SEED.lock().ok()?.take()}
fn parse_seed(text:&str)->Result<u64,&'static str> {
    let text=text.trim();
    if text.is_empty()||!text.bytes().all(|c|c.is_ascii_digit()){return Err("请输入数字种子");}
    text.parse().map_err(|_|"种子超出范围")
}
pub fn draw(ui:&Ui)->bool {
    let Ok(mut screen)=SCREEN.lock() else{return false;};
    let Some(current)=screen.as_mut() else{return false;};
    if render(ui,current) {*screen=None;ACTIVE.store(false,Ordering::Relaxed);}
    true
}
fn render(ui:&Ui,current:&mut Screen)->bool {
    if let Screen::Summary(run)=current {return summary_page(ui,run);}
    let size=ui.io().display_size;
    crate::oath_art::background(&ui.get_background_draw_list(),size);
    let mut close=false;
    let _background=ui.push_style_color(StyleColor::WindowBg,[0.035,0.035,0.025,0.95]);
    let _text=ui.push_style_color(StyleColor::Text,[0.85,0.76,0.52,1.0]);
    ui.window("旅程##run-screen").position([size[0]*0.5,size[1]*0.5],Condition::Always).position_pivot([0.5,0.5])
        .size([620.0f32.min(size[0]*0.9),510.0f32.min(size[1]*0.9)],Condition::Always).title_bar(false).resizable(false).movable(false).build(|| {
        ui.dummy([0.0,15.0]);
        match current {
            Screen::Summary(_)=>unreachable!(),
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
            Screen::Seed{input,error,digit}=>{
                ui.text("通过种子开始游戏");ui.separator();ui.spacing();
                ui.input_text("种子",input).chars_decimal(true).build();
                if ui.is_key_pressed(Key::GamepadDpadLeft){*digit=(*digit+9)%10;}
                if ui.is_key_pressed(Key::GamepadDpadRight){*digit=(*digit+1)%10;}
                for n in 0..10u8 {
                    if n>0 {ui.same_line();}
                    if ui.button(if n==*digit {format!("[{n}]")}else{format!(" {n} ")})&&input.len()<20 {input.push(char::from(b'0'+n));}
                }
                if ui.is_key_pressed(Key::GamepadFaceDown)&&input.len()<20 {input.push(char::from(b'0'+*digit));}
                if ui.button("退格 [Y]")||ui.is_key_pressed(Key::GamepadFaceUp){input.pop();}
                ui.text("手柄：左右选数字 · A 输入 · X 开始");
                ui.text_wrapped("相同种子生成相同路线和敌人；奖励独立随机。");
                if !error.is_empty(){ui.text_wrapped(&*error);}
                ui.spacing();
                if ui.button("开始旅程  [Enter / X]")||ui.is_key_pressed(Key::Enter)||ui.is_key_pressed(Key::GamepadFaceLeft) {
                    match parse_seed(input) {Ok(seed)=>{if let Ok(mut pending)=SEED.lock(){*pending=Some(seed);close=true;}},Err(message)=>*error=message.into()}
                }
                if ui.button("取消  [Esc / B]")||ui.is_key_pressed(Key::Escape)||ui.is_key_pressed(Key::GamepadFaceRight){close=true;}
            }
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
    *SCREEN.lock().unwrap()=None;ACTIVE.store(false,Ordering::Relaxed);
    hub_ready();assert!(!active(),"The result must only open once");
    let before=run.clone();let mut after=before.clone();
    after.elite_buffs=vec![1000,1150];trial_rewards(&before,&after);
    assert!(trial_rewards_active());
    assert!(matches!(&*SCREEN.lock().unwrap(),Some(Screen::TrialRewards{items,..}) if items==&vec![1000,1150]));
    *SCREEN.lock().unwrap()=None;ACTIVE.store(false,Ordering::Relaxed);
    for size in [[1280.0,720.0],[1920.0,1080.0],[3440.0,1440.0]] {
        context.io_mut().display_size=size;
        for mut screen in [Screen::Notice{message:"爬塔协议版本不匹配：本机 23，对方 22。\n请所有玩家更新同一版爬塔补丁。".into(),opened:None},Screen::Summary(run.clone()),Screen::TrialRewards{items:vec![1000,1150],opened:None},Screen::TrialRewards{items:vec![],opened:None},Screen::Seed{input:u64::MAX.to_string(),error:String::new(),digit:9}] {
            render(context.frame(),&mut screen);
            let data=context.render();assert!(data.total_vtx_count>0);
            if matches!(screen,Screen::Summary(_)) {if let Ok(path)=std::env::var("ERDTREE_UI_PREVIEW") {
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
fn summary_page(ui:&Ui,run:&Run)->bool {
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
    close
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
