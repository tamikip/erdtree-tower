// 调试菜单：开启调试后由快捷键打开。普通命令仅单人；Boss 测试由房主在全员同意调试后同步发起。
use std::{collections::VecDeque,sync::{LazyLock,Mutex,atomic::{AtomicBool,Ordering}}};
use hudhook::imgui::{Condition,Ui};
use erdtree_tower::{NodeKind,Run,contracts::Kind,embers::Ember,law::{Law,NODES}};
use crate::enemies::Enemy;

pub enum Command {EventTest(erdtree_tower::events::Draw),MausoleumTest,BossTest(u16),RadahnTest,EndBossTest,ChainTest(u32),Jump(usize),Act(u8),Oath(Kind,u8),Ember(Ember,bool),Law(Law),Item(u32),SpiritItem(u32),Heal,Runes(u32),Cheats(bool,bool),InstantBreak(bool)}

// tower-debug.enable 文件，或启动参数 debug=1（启动器的「调试模式」）
static ENABLED:LazyLock<bool>=LazyLock::new(||crate::LOG_PATH.with_file_name("tower-debug.enable").exists()||crate::options::get().debug);
static OPEN:AtomicBool=AtomicBool::new(false);
static QUEUE:Mutex<VecDeque<Command>>=Mutex::new(VecDeque::new());
/// 无敌（玩家不会死、敌人打不中玩家）与秒杀（玩家每次命中 9999999 伤害）：游戏自带的调试开关（WorldChrManDbgFlags），只在单人生效
static GOD:AtomicBool=AtomicBool::new(false);
static ONE_SHOT:AtomicBool=AtomicBool::new(false);
pub fn replace_cheats(god:bool,one_shot:bool)->(bool,bool){(GOD.swap(god,Ordering::Relaxed),ONE_SHOT.swap(one_shot,Ordering::Relaxed))}
static APPLIED:Mutex<Option<(bool,bool)>>=Mutex::new(None);
/// 攻击立刻爆条（单人调试）：每帧把敌人的韧性压到接近 0，下一下韧性伤害就失衡
static INSTANT_BREAK:AtomicBool=AtomicBool::new(false);
static CHEATS_BLOCKED:AtomicBool=AtomicBool::new(false);
/// 离玩家最近的敌人的韧性：调试菜单打开时游戏任务线程每帧写，菜单里显示
static NEAREST:Mutex<Option<String>>=Mutex::new(None);
static SUMMONS:Mutex<Vec<String>>=Mutex::new(Vec::new());
static SUMMON_TRACE:LazyLock<Mutex<std::collections::HashMap<String,(std::time::Instant,u32)>>>=LazyLock::new(||Mutex::new(Default::default()));
fn trace_custom_spirits(world:&eldenring::cs::WorldChrMan) {
    let Some(player)=world.main_player.as_ref() else{return};
    let origin=&player.chr_ins.modules.physics.position;
    let Ok(mut traces)=SUMMON_TRACE.try_lock() else{return};
    let now=std::time::Instant::now();
    if traces.len()>64 {traces.clear();}
    for group in world.summon_buddy_manager.groups.iter().flat_map(|pair|pair.second.iter()).filter(|g|!g.is_remote) {
        let Some(chr)=crate::spirits::live_chr(group) else {continue;};
        let Some(goods)=custom_buddy_item(group.buddy_param_id) else {continue;};
        let state=traces.entry(chr.field_ins_handle.to_string()).or_insert((now,0));
        if state.1>=120||state.1>0&&now.duration_since(state.0)<std::time::Duration::from_millis(250) {continue;}
        state.0=now;state.1+=1;
        let physics=&chr.modules.physics;let p=&physics.position;
        let time=&chr.modules.time_act;
        let animation=time.anim_queue.get(time.read_idx as usize).map(|a|a.anim_id);
        crate::log(format!("ASH_PHYS goods={} npc={} sample={} handle={} buddy={} hp={}/{} dead={} dismiss={} pos=[{:.3},{:.3},{:.3}] player=[{:.3},{:.3},{:.3}] delta_y={:.3} solid={} grounded={} falling={} gravity_off={} fade_gravity_off={} disabled={}",
            goods&0x0fffffff,chr.npc_param_id,state.1,chr.field_ins_handle,group.buddy_param_id,chr.modules.data.hp,chr.modules.data.max_hp,chr.chr_flags1c5.death_flag(),group.disappear_requested,
            p.0,p.1,p.2,origin.0,origin.1,origin.2,p.1-origin.1,physics.touching_solid_ground,physics.is_touching_ground,physics.is_falling,
            physics.gravity_disabled,physics.fade_out_gravity_disabled,chr.debug_flags.character_disabled()));
        crate::log(format!("ASH_COLLISION goods={} handle={} sample={} animation={animation:?} ctrl={:#x} proxy={:#x} map_disabled={} capsule_disabled={} shape=[{:.3},{:.3}] char_shape=[{:.3},{:.3}] push_up={:.3} gravity_mult={:.3} high_adjust={} proxy_update={} effects={:?}",
            goods&0x0fffffff,chr.field_ins_handle,state.1,chr.chr_ctrl.flags.0,chr.chr_ctrl.chr_proxy_flags.0,chr.chr_ctrl.flags.disable_map_collision(),chr.chr_ctrl.flags.disable_character_capsule_collision(),
            physics.hit_height,physics.hit_radius,physics.chr_hit_height,physics.chr_hit_radius,physics.chr_push_up_factor,physics.gravity_multiplier,
            physics.adjust_to_hi_collision,physics.chr_proxy_pos_update_requested,chr.special_effect.entries().map(|e|e.param_id).collect::<Vec<_>>()));
    }
}
fn new_spirits(level:u32)->Vec<(u32,&'static str)> {
    if ![0,10,39].contains(&level){return Vec::new();}
    erdtree_tower::rewards::CATALOG.iter().filter(|i|erdtree_tower::rewards::custom_spirit(i.id)).map(|i|(i.id+level,i.name)).collect()
}
pub fn valid_spirit_test_item(id:u32)->bool {
    erdtree_tower::rewards::custom_spirit(id)&&erdtree_tower::rewards::lookup(id).is_some()
}
fn custom_buddy_item(param:i32)->Option<u32> {
    static COUNTS:std::sync::LazyLock<Vec<i32>>=std::sync::LazyLock::new(||include_str!("../../scene-assets/custom-spirit-definitions.tsv").lines()
        .map(|line|line.split('|').nth(10).map_or(1,|n|n.parse().expect("Invalid spirit count"))).collect());
    let offset=param.checked_sub(1600000000)?;
    let families=erdtree_tower::rewards::CUSTOM_SPIRITS.len() as i32;
    if !(0..families*40000).contains(&offset)||offset%1000>=COUNTS[(offset/40000) as usize]{return None;}
    let id=0x40000000|(erdtree_tower::rewards::CUSTOM_SPIRIT_FIRST+(offset/40000) as u32*1000+(offset%40000/1000) as u32);
    valid_spirit_test_item(id).then_some(id)
}
/// 每帧（游戏任务线程）：攻击立刻爆条开着时压低全部敌人的韧性；菜单打开时记下最近那个敌人的韧性。只看敌人，不动玩家、队友与召唤物
pub fn tick_enemies(world:&mut eldenring::cs::WorldChrMan) {
    crate::gesture_trace::tick(world);
    if enabled(){trace_custom_spirits(world);}
    let (open,instant)=(OPEN.load(Ordering::Relaxed),INSTANT_BREAK.load(Ordering::Relaxed));
    if !enabled()||!open&&!instant {return;}
    let Some(player)=world.main_player.as_ref() else{return};
    let origin={let p=&player.chr_ins.modules.physics.position;[p.0,p.1,p.2]};
    if open&&let Ok(mut rows)=SUMMONS.lock() {
        *rows=world.summon_buddy_manager.groups.iter().flat_map(|pair|pair.second.iter()).filter(|g|!g.is_remote).filter_map(|group| {
            let chr=crate::spirits::live_chr(group)?;let p=&chr.modules.physics.position;
            let distance=((p.0-origin[0]).powi(2)+(p.1-origin[1]).powi(2)+(p.2-origin[2]).powi(2)).sqrt();
            let name=custom_buddy_item(group.buddy_param_id).and_then(erdtree_tower::rewards::lookup).map_or("原版召唤物",|i|i.name);
            Some(format!("{name} · 生命 {}/{} · 距离 {distance:.1} 米 · 高差 {:.1} · 落地 {} · 下坠 {}{}",chr.modules.data.hp,chr.modules.data.max_hp,p.1-origin[1],chr.modules.physics.is_touching_ground,chr.modules.physics.is_falling,if group.disappear_requested{" · 正在离场"}else{""}))
        }).collect();
    }
    let friends:Vec<eldenring::cs::FieldInsHandle>=world.player_chr_set.characters().map(|p|p.chr_ins.field_ins_handle)
        .chain(world.summon_buddy_chr_set.characters().map(|c|c.field_ins_handle)).chain(std::iter::once(player.chr_ins.field_ins_handle)).collect();
    let mut nearest:Option<(f32,String)>=None;
    for chr in world.chr_sets.iter().flatten().flat_map(|set|set.characters()) {
        if friends.contains(&chr.field_ins_handle)||chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag()||chr.debug_flags.character_disabled(){continue;}
        if chr.modules.super_armor.sa_durability_max<=0.0 {continue;}
        if instant {crate::embers::prime_stance_break(chr);}
        if !open {continue;}
        let p=&chr.modules.physics.position;
        let distance=((p.0-origin[0]).powi(2)+(p.1-origin[1]).powi(2)+(p.2-origin[2]).powi(2)).sqrt();
        if nearest.as_ref().is_some_and(|(best,_)|*best<=distance) {continue;}
        let (t,armor)=(&chr.modules.toughness,&chr.modules.super_armor);
        nearest=Some((distance,format!("最近的敌人：NPC {}　距离 {distance:.1} 米　生命 {}/{}\n架势（处决失衡）{:.3} / {:.1}　恢复计时 {:.1} 秒\n装备韧度 {:.1} / {:.1}",
            chr.npc_param_id,chr.modules.data.hp,chr.modules.data.max_hp,armor.sa_durability,armor.sa_durability_max,armor.recover_time,t.toughness,t.toughness_max)));
    }
    if open&&let Ok(mut shown)=NEAREST.lock() {*shown=Some(nearest.map_or_else(||"附近没有敌人".to_owned(),|(_,text)|text));}
}
static BOUT_PROTECT:AtomicBool=AtomicBool::new(false);
static PLAYER_PROTECTION:Mutex<PlayerProtection>=Mutex::new(PlayerProtection{previous:None});
struct PlayerProtection {previous:Option<(usize,bool)>}
impl PlayerProtection {
    fn update(&mut self,owner:usize,current:bool,wanted:bool)->bool {
        if self.previous.is_some_and(|(old,_)|old!=owner){self.previous=None;}
        if wanted {self.previous.get_or_insert((owner,current));true}
        else {self.previous.take().map_or(current,|(_,before)|if current{before}else{current})}
    }
}
fn sync_player_hit(world:&mut eldenring::cs::WorldChrMan) {
    if let Ok(mut state)=PLAYER_PROTECTION.lock(){
        if let Some(player)=world.main_player.as_mut(){
            let chr=&mut player.chr_ins;let owner=std::ptr::from_ref(chr) as usize;
            let wanted=GOD.load(Ordering::Relaxed)||BOUT_PROTECT.load(Ordering::Relaxed);
            let hit=state.update(owner,chr.debug_flags.disabled_hit(),wanted);
            chr.debug_flags.set_disabled_hit(hit);
        }else{state.previous=None;}
    }
}
fn write_cheat_flags(flags:&mut eldenring::cs::WorldChrManDbgFlags,want:(bool,bool)) {
    flags.player_no_dead=want.0;flags.all_no_hit=false;flags.player_exterminate=want.1;
}
/// 每帧：把开关写进游戏的调试标志；联机或调试菜单不可用时全部关掉
pub fn apply_cheats(party:bool) {
    use fromsoftware_shared::FromStatic;
    CHEATS_BLOCKED.store(party,Ordering::Relaxed);
    if party||!enabled() {GOD.store(false,Ordering::Relaxed);ONE_SHOT.store(false,Ordering::Relaxed);BOUT_PROTECT.store(false,Ordering::Relaxed);INSTANT_BREAK.store(false,Ordering::Relaxed);}
    if let Ok(world)=unsafe {eldenring::cs::WorldChrMan::instance_mut()}{sync_player_hit(world);}
    let want=(GOD.load(Ordering::Relaxed),ONE_SHOT.load(Ordering::Relaxed));
    let Ok(mut applied)=APPLIED.lock() else{return};
    if *applied==Some(want)||applied.is_none()&&want==(false,false) {return;}
    let Ok(flags)=(unsafe {eldenring::cs::WorldChrManDbgFlags::instance_mut()}) else{return};
    write_cheat_flags(flags,want);
    *applied=Some(want);
    crate::log(format!("DEBUG_MENU cheats god={} one_shot={}",want.0,want.1));
}
static ROSTER:Mutex<Option<Vec<Enemy>>>=Mutex::new(None);

pub fn enabled()->bool {*ENABLED}
pub fn open()->bool {OPEN.load(Ordering::Relaxed)}
pub fn toggle() {
    if !enabled() {return;}
    let now=!OPEN.load(Ordering::Relaxed);OPEN.store(now,Ordering::Relaxed);
    crate::log(format!("DEBUG_MENU open={now}"));
}
/// 正在输入的文字框：让输入法的候选框开在它下面（中文输入见 hudhook 的 renderer/ime.rs）
fn ime_here(ui:&Ui) {
    if ui.is_item_active() {let min=ui.item_rect_min();let max=ui.item_rect_max();hudhook::set_ime_position(min[0],max[1]);}
}
fn send(command:Command) {if let Ok(mut queue)=QUEUE.lock(){queue.push_back(command);}}
pub fn take()->Option<Command> {let mut queue=QUEUE.lock().ok()?;let index=queue.iter().position(|c|!matches!(c,Command::EventTest(_)|Command::MausoleumTest|Command::BossTest(_)|Command::RadahnTest|Command::EndBossTest|Command::ChainTest(_)))?;queue.remove(index)}
pub fn take_test()->Option<Command> {
    let mut queue=QUEUE.lock().ok()?;
    let index=queue.iter().position(|c|matches!(c,Command::EventTest(_)|Command::MausoleumTest|Command::BossTest(_)|Command::RadahnTest|Command::EndBossTest|Command::ChainTest(_)))?;
    queue.remove(index)
}
static PARTY_ROLE:std::sync::atomic::AtomicU8=std::sync::atomic::AtomicU8::new(0);
pub fn set_party_role(host:Option<bool>){PARTY_ROLE.store(host.map_or(0,|h|if h{1}else{2}),Ordering::Relaxed);}
pub fn close(){OPEN.store(false,Ordering::Relaxed);}
pub fn clear_roster(){if let Ok(mut roster)=ROSTER.lock(){*roster=None;}if let Ok(mut bout)=BOUT.lock(){*bout=None;}}
/// 下一场单人战斗改用指定的敌人，生成时取出
pub fn take_roster()->Option<Vec<Enemy>> {ROSTER.lock().ok()?.take()}
fn pending_roster()->Option<String> {ROSTER.lock().ok()?.as_ref().map(|r|r.iter().map(|e|e.name).collect::<Vec<_>>().join("、"))}

/// 斗蛐蛐实验：下一场生成两只敌人，给它们挂上改阵营的效果（scene-builder bout-patch，9680000+阵营值），
/// 玩家在这场里不受伤害，记录哪一只先倒下。用来实机确认怪物能否互相攻击
pub const BOUT_FIRST:i32=9680000;
pub const BOUT_TEAMS:[u8;6]=[29,24,26,27,30,34];
#[derive(Clone,Copy,Debug)]
pub struct BoutSetup {pub team:u8,pub both:bool}
static BOUT:Mutex<Option<BoutSetup>>=Mutex::new(None);
pub fn take_bout()->Option<BoutSetup> {BOUT.lock().ok()?.take()}
#[derive(Default)]
pub struct Bout {setup:Option<BoutSetup>,applied:Vec<eldenring::cs::FieldInsHandle>,started:Option<std::time::Instant>,done:bool,guarded:bool}
impl Bout {
    pub fn begin(&mut self,setup:BoutSetup) {
        crate::log(format!("BOUT begin team={} both={}",setup.team,setup.both));
        *self=Self{setup:Some(setup),..Default::default()};
    }
    pub fn active(&self)->bool {self.setup.is_some()}
    /// 每帧：敌人就位后挂阵营效果，玩家不受伤害；第一只敌人倒下时记下胜者并恢复玩家
    pub fn tick(&mut self,world:&mut eldenring::cs::WorldChrMan,foes:&[(eldenring::cs::FieldInsHandle,&'static str)])->Option<String> {
        use eldenring::cs::ChrInsExt;
        let setup=self.setup?;
        if self.done {return None;}
        if !self.guarded&&world.main_player.is_some(){BOUT_PROTECT.store(true,Ordering::Relaxed);sync_player_hit(world);self.guarded=true;}
        let effect=BOUT_FIRST+i32::from(setup.team);
        for (index,(handle,name)) in foes.iter().enumerate() {
            if self.applied.contains(handle)||!(setup.both||index==1) {continue;}
            if let Some(chr)=world.chr_ins_by_handle_mut(handle) {chr.apply_speffect(effect,false);self.applied.push(*handle);crate::log(format!("BOUT team {} applied to {name}",setup.team));}
        }
        if foes.len()<2||self.applied.len()<usize::from(setup.both)+1 {return None;}
        let started=*self.started.get_or_insert_with(std::time::Instant::now);
        let alive:Vec<_>=foes.iter().map(|(handle,_)|world.chr_ins_by_handle(handle).is_some_and(|c|c.modules.data.hp>0&&!c.chr_flags1c5.death_flag())).collect();
        let hp:Vec<_>=foes.iter().map(|(handle,_)|world.chr_ins_by_handle(handle).map_or(0,|c|c.modules.data.hp)).collect();
        let message=if alive.iter().filter(|a|!**a).count()>=1 {
            let winner=alive.iter().position(|a|*a).map_or("同归于尽".to_owned(),|i|format!("{} 获胜",foes[i].1));
            format!("斗蛐蛐实验（阵营 {}）：{winner}，用时 {} 秒；现在可以动手清场",setup.team,started.elapsed().as_secs())
        } else if started.elapsed()>std::time::Duration::from_secs(120) {
            format!("斗蛐蛐实验（阵营 {}）：120 秒内没有分出胜负，剩余生命 {:?}；可能没有互相攻击",setup.team,hp)
        } else {return None;};
        self.done=true;self.release(world);crate::log(format!("BOUT result {message}"));Some(message)
    }
    fn release(&mut self,world:&mut eldenring::cs::WorldChrMan) {
        if self.guarded {BOUT_PROTECT.store(false,Ordering::Relaxed);sync_player_hit(world);self.guarded=false;}
    }
    /// 战斗结束或中断时调用，保证玩家恢复正常
    pub fn end(&mut self,world:&mut eldenring::cs::WorldChrMan) {if self.active() {self.release(world);*self=Self::default();}}
}

#[derive(Default)]
pub struct Menu {enemy_filter:String,item_query:String,items:Vec<(u32,String)>,searched:String,count:i32}
const LEVELS:[&str;4]=["无","I","II","III"];
impl Menu {
    pub fn draw(&mut self,ui:&Ui,view:Option<&crate::runtime::View>) {
        if !open() {return;}
        let size=ui.io().display_size;
        let mut keep=true;
        ui.window(format!("爬塔调试  [{}]###tower-debug",crate::options::label(erdtree_tower::options::Hotkey::Debug))).opened(&mut keep).position([size[0]*0.55,size[1]*0.06],Condition::FirstUseEver).size([size[0]*0.42,size[1]*0.86],Condition::FirstUseEver).build(|| {
            let Some(view)=view else {ui.text(crate::i18n::t("运行时尚未就绪"));return;};
            ui.text_colored([0.9,0.8,0.5,1.0],crate::i18n::t(&view.status));
            let Some(run)=view.run.as_ref() else {ui.text(crate::i18n::t("尚未开始旅程：在大赐福开始新局后再使用"));return;};
            ui.text(crate::i18n::t(format!("第 {} 幕 · 已走 {} 层 · {:?}{}",run.act,run.visited.len(),run.phase,if run.debug{" · 调试旅程"}else{""})));
            if let Some(_bar)=ui.tab_bar("##debug-tabs") {
                if let Some(_tab)=ui.tab_item("Boss") {self.boss_tests(ui,view,run);}
                if let Some(_tab)=ui.tab_item("骨灰") {self.spirit_tests(ui,view,run);}
                if let Some(_tab)=ui.tab_item("战斗") {self.combat(ui,run);}
                if let Some(_tab)=ui.tab_item("构筑") {self.build(ui,run);}
                if let Some(_tab)=ui.tab_item("物品") {self.items(ui);}
                if let Some(_tab)=ui.tab_item("状态") {self.state(ui,run);}
                if let Some(_tab)=ui.tab_item("界面预览") {
                    ui.text_wrapped(crate::i18n::t("只打开界面看效果，不改变旅程进度。"));
                    if ui.button(crate::i18n::t("联机无法开始的提示")) {crate::run_screen::party_notice("爬塔握手未完成（队伍 3 人）：\n队员 76561199000000000：尚未收到握手消息\n尚未收到响应不等于版本不匹配；请检查对应队员是否从爬塔入口启动。".into());}
                    if ui.button(crate::i18n::t("试炼完成：两项护符赐福")) {crate::run_screen::preview_trial("史东薇尔突围",vec![1000,1150]);}
                    if ui.button(crate::i18n::t("试炼完成：没有新增赐福")) {crate::run_screen::preview_trial("异端魔法师塔·隐形桥",vec![]);}
                    ui.separator();
                    ui.text_wrapped(crate::i18n::t("癫火舞会试跳：方向键按箭头，判定点按空格；不发奖励。选择后自动关闭调试菜单。"));
                    for (index,song) in erdtree_tower::dance::songs().iter().enumerate() {
                        for (label,difficulty) in [("普通",erdtree_tower::dance::Difficulty::Normal),("困难",erdtree_tower::dance::Difficulty::Hard)] {
                            if ui.button(format!("{}（{} BPM）· {}##dance-{index}-{label}",song.name,song.bpm,crate::i18n::t(label))) {
                                let seed=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7,|d|d.as_secs());
                                crate::dance_ui::start(index,difficulty,seed);close();
                            }
                            ui.same_line();
                        }
                        ui.new_line();
                    }
                }
                if let Some(_tab)=ui.tab_item("事件") {
                    use erdtree_tower::events::Draw;
                    ui.text_wrapped(crate::i18n::t("先开始旅程并停在路线图。联机由房主选择事件，全员需更新版本、开启调试并完成领奖。隐形道路在试炼 NPC 菜单里选择；测试旅程不计正式解锁。"));
                    let _disabled=ui.begin_disabled(PARTY_ROLE.load(Ordering::Relaxed)==2||run.phase!=erdtree_tower::Phase::Map||view.busy||view.travelling);
                    for (label,draw) in [("余烬熔炉",Draw::EmberFurnace),("骨灰融合炉",Draw::SpiritFurnace),("癫火祭坛",Draw::FrenzyAltar),("坚持30秒大混战",Draw::Survival),("舞会",Draw::Dance),("试炼岔路（含异端塔隐形桥）",Draw::Trial),("恢复泉水",Draw::Spring),("骰子赌徒",Draw::Dice),("盲盒商人",Draw::Mystery),("词条重铸",Draw::Reforge),("重练炉",Draw::Furnace),("粪金龟",Draw::Scarabs),("灵庙引路人",Draw::Mausoleum),("多宝箱",Draw::Chests),("祭坛",Draw::Altar),("上锁宝箱",Draw::Locked)] {
                        if ui.button(crate::i18n::t(label)){send(Command::EventTest(draw));}
                    }
                }
                if let Some(_tab)=ui.tab_item("场地") {arenas(ui,run);}
            }
        });
        if !keep {OPEN.store(false,Ordering::Relaxed);}
    }
    fn spirit_tests(&mut self,ui:&Ui,view:&crate::runtime::View,run:&Run) {
        let blocked=PARTY_ROLE.load(Ordering::Relaxed)!=0||view.travelling||!view.ready||matches!(run.phase,erdtree_tower::Phase::Reward{..});
        ui.text_wrapped(crate::i18n::t("全部自制骨灰（含按怪池补齐的），每次领取 1 份，可重复补充。装备到快捷栏后，在战斗场地手动召唤；检验胜利返还请使用召唤师。"));
        if PARTY_ROLE.load(Ordering::Relaxed)!=0 {ui.text_disabled(crate::i18n::t("实际组队时禁止发放调试物品；无缝入口独自游玩可用。"));}
        ui.separator();
        {
            let _disabled=ui.begin_disabled(blocked);
            for (index,level) in [0,10,39].into_iter().enumerate() {
                if index>0 {ui.same_line();}
                if ui.button(crate::i18n::t(format!("全部领取 +{level}"))) {for (id,_) in new_spirits(level) {send(Command::SpiritItem(id));}}
            }
        }
        if let Ok(rows)=SUMMONS.lock() {
            ui.text(crate::i18n::t(format!("当前本机召唤物：{}",rows.len())));
            for row in rows.iter(){ui.text_wrapped(crate::i18n::t(row));}
        }
        ui.separator();
        ui.child_window("##new-spirit-list").size([0.0,0.0]).build(|| {
            let _disabled=ui.begin_disabled(blocked);
            for (base,name) in new_spirits(0) {
                ui.text(crate::i18n::t(name));
                for level in [0,10,39] {
                    ui.same_line();
                    if ui.small_button(crate::i18n::t(format!("+{level}##spirit-{base}-{level}"))) {send(Command::SpiritItem(base+level));}
                }
            }
        });
    }
    fn boss_tests(&mut self,ui:&Ui,view:&crate::runtime::View,run:&Run) {
        let role=PARTY_ROLE.load(Ordering::Relaxed);let party=role!=0;
        ui.text_wrapped(crate::i18n::t("选择任意 Boss 组合直接测试，旅程会标记为调试，不计入正式成绩与解锁。"));
        ui.text_wrapped(crate::i18n::t(if party {"联机需全员开启调试并完成领奖、誓约选择；仅房主可发起或结束测试。"}else{"请在路线图状态发起；试验条目也会列出，可能存在原版场景依赖。"}));
        if let Some(test)=run.boss_test {
            let group=crate::enemies::debug_encounters().get(usize::from(test.encounter)).cloned().unwrap_or_default();
            ui.text_wrapped(crate::i18n::t(format!("当前测试 #{}：{}",test.encounter,crate::enemies::debug_group_name(&group))));
            if let Some((hp,max))=view.enemy_hp {ui.text(crate::i18n::t(format!("敌人总生命：{hp}/{max}")));}
            let _disabled=ui.begin_disabled(role==2);
            if ui.button(crate::i18n::t("结束测试，返回大赐福")) {send(Command::EndBossTest);}
        }
        ui.separator();ui.input_text(crate::i18n::t("搜索 Boss"),&mut self.enemy_filter).build();ime_here(ui);
        let groups=crate::enemies::debug_encounters();
        ui.text(crate::i18n::t(format!("共 {} 个 Boss 组合；多实体组合整组测试",groups.len())));
        let ready=role!=2&&run.phase==erdtree_tower::Phase::Map&&!view.travelling&&!view.busy;
        ui.child_window("##boss-tests").size([0.0,0.0]).build(|| {
            for (index,group) in groups.iter().enumerate() {
                let names=crate::enemies::debug_group_name(group);
                if !crate::i18n::matches(&names,&self.enemy_filter){continue;}
                let available=crate::enemies::boss_test_allowed(group,party,erdtree_tower::dlc::enemies_allowed());
                let _disabled=ui.begin_disabled(!ready||available.is_err());
                if ui.button(crate::i18n::t(format!("测试##boss-test-{index}"))){send(Command::BossTest(index as u16));}
                drop(_disabled);ui.same_line();
                ui.text_wrapped(crate::i18n::t(format!("#{index} 第{}幕 {names} · {} 只{}{}",group[0].act,group.len(),if group.iter().any(|e|e.dlc){" · DLC"}else{""},if group.iter().any(|e|e.retired){" · 试验"}else{""})));
                if let Err(reason)=available {ui.text_disabled(crate::i18n::t(reason));}
            }
        });
    }
    fn combat(&mut self,ui:&Ui,run:&Run) {
        ui.text(crate::i18n::t("跳到节点（路线图上会变成可选）"));
        ui.child_window("##debug-nodes").size([0.0,220.0]).build(|| {
            for layer in 0..run.floors() {
                ui.text(crate::i18n::t(format!("第 {:>2} 层",layer+1)));
                for (id,node) in run.nodes.iter().enumerate().filter(|(_,n)|n.layer==layer) {
                    ui.same_line();
                    let label=format!("{}##node-{id}",node.kind.label());
                    if ui.small_button(crate::i18n::t(label)) {send(Command::Jump(id));}
                }
            }
        });
        ui.separator();
        ui.text(crate::i18n::t(format!("下一场战斗的敌人：{}",pending_roster().unwrap_or_else(||"按路线正常抽取".into()))));
        if ui.small_button(crate::i18n::t("恢复正常抽取")) {if let Ok(mut roster)=ROSTER.lock(){*roster=None;}if let Ok(mut bout)=BOUT.lock(){*bout=None;}}
        if self.count<1 {self.count=1;}
        ui.set_next_item_width(120.0);ui.input_int("数量",&mut self.count).build();self.count=self.count.clamp(1,4);
        ui.set_next_item_width(240.0);ui.input_text(crate::i18n::t("筛选##enemy"),&mut self.enemy_filter).build();ime_here(ui);
        let filter=self.enemy_filter.trim().to_owned();
        ui.child_window("##debug-enemies").size([0.0,0.0]).build(|| {
            ui.text_colored([0.7,0.7,0.7,1.0],crate::i18n::t("怪物池"));
            for enemy in crate::enemies::debug_pool().into_iter().filter(|e|crate::i18n::matches(e.name,&filter)) {
                let tier=match enemy.kind {NodeKind::Monster=>"普通",NodeKind::Elite=>"强敌",_=>"Boss"};
                let label=format!("第{}幕 {tier} {}{}##pool-{}",enemy.act,enemy.name,if enemy.dlc{"（DLC）"}else{""},enemy.npc);
                if ui.selectable(crate::i18n::t(label)) {set_roster(vec![enemy;self.count as usize]);}
            }
            ui.text_colored([0.7,0.7,0.7,1.0],crate::i18n::t("Boss 组（整组生成）"));
            for (index,parts) in crate::enemies::debug_encounters().into_iter().enumerate() {
                if PARTY_ROLE.load(Ordering::Relaxed)==0&&crate::enemies::party_only_group(&parts){continue;}
                let name=crate::enemies::debug_group_name(&parts);
                if !crate::i18n::matches(&name,&filter) {continue;}
                let trial=if parts[0].retired {"（试验）"}else{""};
                if ui.selectable(crate::i18n::t(format!("第{}幕 {name}（{} 只）{trial}##boss-{index}",parts[0].act,parts.len()))) {set_roster(parts);}
            }
        });
    }
    fn build(&mut self,ui:&Ui,run:&Run) {
        ui.text(crate::i18n::t("誓约等级"));
        for kind in Kind::ALL {
            let level=run.contract.as_ref().and_then(|c|c.level(kind)).unwrap_or(0);
            ui.text(crate::i18n::t(format!("{}：{}",kind.name(),LEVELS[usize::from(level.min(3))])));
            for (target,label) in LEVELS.iter().enumerate() {
                ui.same_line_with_pos(220.0+target as f32*56.0);
                if ui.small_button(crate::i18n::t(format!("{label}##oath-{}-{target}",kind.index()))) {send(Command::Oath(kind,target as u8));}
            }
        }
        ui.separator();ui.text(crate::i18n::t("余烬"));
        if ui.small_button(crate::i18n::t("预览原生余烬获得窗口（不发奖）")) {crate::ember_notice::preview(Ember::SpiritTwin);OPEN.store(false,Ordering::Relaxed);}
        for (i,ember) in Ember::ALL.into_iter().enumerate().filter(|(_,e)|!e.retired()) {
            if i%3!=0 {ui.same_line_with_pos(10.0+(i%3) as f32*200.0);}
            let mut held=run.embers.contains(&ember);
            if ui.checkbox(crate::i18n::t(format!("{}##ember-{i}",ember.name())),&mut held) {send(Command::Ember(ember,held));}
        }
        ui.separator();
        ui.text(crate::i18n::t(format!("苦难律法：{} 分 · 律法码 {}",run.law.score(),run.law.code())));
        ui.same_line();if ui.small_button(crate::i18n::t("清空律法")) {send(Command::Law(Law::default()));}
        // 与律法界面相同：一条路线一行，从左到右依次解锁
        let mut row=None;
        for (id,node) in NODES.iter().enumerate().filter(|&(id,_)|Law::listed(id)) {
            if row==Some(node.row) {ui.same_line_with_pos(10.0+f32::from(node.col-1)*150.0);} else {row=Some(node.row);}
            let mut has=run.law.has(id);
            let _disabled=ui.begin_disabled(!run.law.available(id));
            if ui.checkbox(crate::i18n::t(format!("{}##law-{id}",node.name)),&mut has) {send(Command::Law(run.law.toggle(id)));}
        }
    }
    fn items(&mut self,ui:&Ui) {
        if PARTY_ROLE.load(Ordering::Relaxed)==0 && ui.button(crate::i18n::t("词条对照：领取两把传说匕首和两枚史诗红琥珀链坠")) {
            for id in [160030000,160030000,537970914,537970914] { send(Command::Item(id)); }
        }
        // 传说武器流光：配色只在本次游戏内生效；“预览”让任意近战武器都发光
        ui.text(crate::i18n::t("传说武器流光"));
        let labels:Vec<_>=crate::weapon_glow::Style::ALL.iter().map(|s|s.label()).collect();
        let mut index=crate::weapon_glow::Style::ALL.iter().position(|s|*s==crate::weapon_glow::style()).unwrap_or(0);
        ui.set_next_item_width(320.0);
        if ui.combo_simple_string("##glow-style",&mut index,&labels) {crate::weapon_glow::set_style(crate::weapon_glow::Style::ALL[index]);}
        ui.same_line();
        let mut preview=crate::weapon_glow::preview();
        if ui.checkbox(crate::i18n::t("预览：手上任意近战武器都发光"),&mut preview) {crate::weapon_glow::set_preview(preview);}
        ui.separator();
        ui.text_wrapped(crate::i18n::t("支持中文、拼音和首字母，例如：长剑 / changjian / cj；也可搜索物品说明。"));
        ui.set_next_item_width(320.0);
        let enter=ui.input_text(crate::i18n::t("##item-query"),&mut self.item_query).enter_returns_true(true).build();ime_here(ui);
        ui.same_line();
        if (ui.button(crate::i18n::t("搜索"))||enter)||self.searched!=self.item_query {
            self.items=crate::reward_ui::search_items(&self.item_query,80);self.searched=self.item_query.clone();
        }
        ui.text(crate::i18n::t(format!("{} 条结果（最多显示 80 条）",self.items.len())));
        ui.child_window("##debug-items").size([0.0,0.0]).build(|| {
            for (id,text) in &self.items {
                if ui.small_button(crate::i18n::t(format!("获得##item-{id}"))) {send(Command::Item(*id));}
                ui.same_line();ui.text(crate::i18n::t(text));
            }
        });
    }
    fn state(&mut self,ui:&Ui,run:&Run) {
        ui.text(crate::i18n::t("切换幕（回到该幕起点，生命与圣杯瓶回满）"));
        for act in 1..=3u8 {
            if act>1 {ui.same_line();}
            if ui.button(crate::i18n::t(format!("第 {act} 幕"))) {send(Command::Act(act));}
        }
        ui.separator();
        if ui.button(crate::i18n::t("回满生命、专注与圣杯瓶")) {send(Command::Heal);}
        if ui.button(crate::i18n::t("获得惜别泪滴（HansFix 测试）")) {send(Command::Item(crate::tears::ITEM));}
        if ui.button(crate::i18n::t("生成大型蓝露滴粪金龟（星辉术士测试）")) {crate::starsurge::request_scarab();}
        let blocked=CHEATS_BLOCKED.load(Ordering::Relaxed);
        let disabled=ui.begin_disabled(blocked);
        let (mut god,mut one_shot)=(GOD.load(Ordering::Relaxed),ONE_SHOT.load(Ordering::Relaxed));
        let changed=ui.checkbox(crate::i18n::t("无敌（仅玩家免伤，可正常攻击）"),&mut god)|{ui.same_line();ui.checkbox(crate::i18n::t("秒杀（每次命中 9999999 伤害）"),&mut one_shot)};
        if changed {GOD.store(god,Ordering::Relaxed);ONE_SHOT.store(one_shot,Ordering::Relaxed);send(Command::Cheats(god,one_shot));}
        let mut instant=INSTANT_BREAK.load(Ordering::Relaxed);
        if ui.checkbox(crate::i18n::t("攻击立刻爆条（压低敌人架势，下一次削韧命中触发失衡）"),&mut instant) {INSTANT_BREAK.store(instant,Ordering::Relaxed);send(Command::InstantBreak(instant));}
        drop(disabled);
        if blocked {ui.text_disabled(crate::i18n::t("组队或无缝会话尚未就绪，暂不可启用；独自游玩可使用。"));}
        if let Some(text)=NEAREST.lock().ok().and_then(|n|n.clone()) {crate::fancy_text::text_wrapped(ui,text);}
        if ui.button(crate::i18n::t("卢恩 +10000")) {send(Command::Runes(10000));}
        ui.same_line();if ui.button(crate::i18n::t("卢恩 +100000")) {send(Command::Runes(100000));}
        ui.separator();
        crate::fancy_text::text_wrapped(ui,format!("种子 {}　职业 {}　卢恩 {}　圣杯瓶 {}/{}",run.initial_seed.unwrap_or(run.seed),crate::map_ui::class_name(run.class),run.gold,run.flasks,run.flask_limit));
        crate::fancy_text::text_wrapped(ui,"使用过调试功能的旅程会标记为调试旅程：不计入苦难律法纪录，结算页会注明。联机时调试命令不会执行。");
    }
}
/// 主题幕的战斗场地：查看本幕主题与场地、指定下一战的场地（单人）、记下当前位置（地图块内坐标与朝向）用于调整场地坐标
fn arenas(ui:&Ui,run:&Run) {
    let theme=erdtree_tower::themes::for_run(run);
    let current=crate::arenas::for_run(run,false);
    crate::fancy_text::text_wrapped(ui,format!("本幕主题：{}（{}）　场地键：{}",theme.name,theme.region,if theme.arena.is_empty(){"（用竞技场）"}else{theme.arena}));
    crate::fancy_text::text_wrapped(ui,format!("下一战（单人）：{}　地图块 {:#010x}",current.name,current.block));
    ui.separator();
    ui.text(crate::i18n::t("当前三幕统一使用龙王平台。"));
    ui.separator();
    if ui.button(crate::i18n::t("记下当前位置")) {
        let line=match unsafe {eldenring::cs::PlayerIns::local_player()} {
            Ok(player)=>{
                let p=player.block_position;let block=i32::from(player.current_block_id).to_be_bytes();
                let yaw=player.chr_ins.modules.physics.orientation.to_euler_angles().1.to_degrees();
                format!("m{:02}_{:02}_{:02}_{:02}|{:.3}|{:.3}|{:.3}|{:.1}",block[0],block[1],block[2],block[3],p.x,p.y,p.z,yaw)
            },
            Err(_)=>"角色未加载".into(),
        };
        let path=crate::LOG_PATH.with_file_name("tower-arena-points.txt");
        let saved=std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f|{use std::io::Write;writeln!(f,"{line}")});
        crate::log(format!("ARENA_POINT {line}"));
        POINT.with(|p|*p.borrow_mut()=format!("{line}{}",if saved.is_ok(){"（已写入 tower-arena-points.txt）"}else{"（写入失败，见日志）"}));
    }
    POINT.with(|p|if !p.borrow().is_empty(){crate::fancy_text::text_wrapped(ui,&*p.borrow());});
}
thread_local! {static POINT:std::cell::RefCell<String>=const {std::cell::RefCell::new(String::new())};}
fn set_roster(roster:Vec<Enemy>) {
    crate::log(format!("DEBUG_MENU next roster: {}",roster.iter().map(|e|e.name).collect::<Vec<_>>().join(",")));
    if let Ok(mut slot)=ROSTER.lock(){*slot=Some(roster);}
}

#[cfg(test)]
mod tests {
    #[test]
    fn custom_spirit_test_lists_all_presets_and_matches_live_buddy_ids() {
        for level in [0,10,39] {
            let items=super::new_spirits(level);assert_eq!(items.len(),erdtree_tower::rewards::CUSTOM_SPIRITS.len());
            for (index,(id,_)) in items.iter().enumerate() {
                assert!(super::valid_spirit_test_item(*id));
                assert_eq!(super::custom_buddy_item(1600000000+index as i32*40000+level as i32*1000),Some(*id));
                let count=include_str!("../../scene-assets/custom-spirit-definitions.tsv").lines().nth(index).unwrap().split('|').nth(10).map_or(1,|n|n.parse::<i32>().unwrap());
                for slot in 0..count {assert_eq!(super::custom_buddy_item(1600000000+index as i32*40000+level as i32*1000+slot),Some(*id));}
                assert_eq!(super::custom_buddy_item(1600000000+index as i32*40000+level as i32*1000+count),None);
            }
        }
        assert!(super::new_spirits(40).is_empty());
        for id in [0,0x40000000|200000,0x40000000|270040,0x40000000|368000] {assert!(!super::valid_spirit_test_item(id));}
        for param in [0,1599999999,1600000001,1600000000+98*40000] {assert_eq!(super::custom_buddy_item(param),None);}
    }
    #[test]
    fn god_mode_leaves_global_hit_and_enemy_protection_off() {
        let mut flags:eldenring::cs::WorldChrManDbgFlags=unsafe {std::mem::zeroed()};
        super::write_cheat_flags(&mut flags,(true,false));
        assert!(flags.player_no_dead);assert!(!flags.all_no_hit&&!flags.all_no_damage&&!flags.all_no_dead);
        super::write_cheat_flags(&mut flags,(false,true));
        assert!(!flags.player_no_dead&&flags.player_exterminate&&!flags.all_no_hit);
    }
    #[test]
    fn player_protection_restores_owner_and_tracks_reloads_and_bout_overlap() {
        let mut state=super::PlayerProtection{previous:None};
        assert!(state.update(1,false,true));assert!(state.update(1,true,true));
        assert!(!state.update(1,true,false));
        assert!(state.update(1,true,true));assert!(state.update(1,true,false));
        assert!(state.update(1,false,true));assert!(!state.update(2,false,false));
        assert!(state.update(2,false,true));assert!(!state.update(2,false,false));
    }
}
