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
fn new_spirits(level:u32)->Vec<(u32,&'static str)> {
    if ![0,10,39].contains(&level){return Vec::new();}
    erdtree_tower::rewards::CATALOG.iter().filter(|i|erdtree_tower::rewards::custom_spirit(i.id)).map(|i|(i.id+level,i.name)).collect()
}
pub fn valid_spirit_test_item(id:u32)->bool {
    erdtree_tower::rewards::custom_spirit(id)&&erdtree_tower::rewards::lookup(id).is_some()
}
fn custom_buddy_item(param:i32)->Option<u32> {
    let offset=param.checked_sub(1600000000)?;
    if !(0..480000).contains(&offset)||offset%1000!=0{return None;}
    let id=0x40000000|(270000+(offset/40000) as u32*1000+(offset%40000/1000) as u32);
    valid_spirit_test_item(id).then_some(id)
}
/// 每帧（游戏任务线程）：攻击立刻爆条开着时压低全部敌人的韧性；菜单打开时记下最近那个敌人的韧性。只看敌人，不动玩家、队友与召唤物
pub fn tick_enemies(world:&mut eldenring::cs::WorldChrMan) {
    let (open,instant)=(OPEN.load(Ordering::Relaxed),INSTANT_BREAK.load(Ordering::Relaxed));
    if !enabled()||!open&&!instant {return;}
    let Some(player)=world.main_player.as_ref() else{return};
    let origin={let p=&player.chr_ins.modules.physics.position;[p.0,p.1,p.2]};
    if open&&let Ok(mut rows)=SUMMONS.lock() {
        *rows=world.summon_buddy_manager.groups.iter().flat_map(|pair|pair.second.iter()).filter(|g|!g.is_remote).map(|group| {
            let chr=unsafe {group.chr_ins.as_ref()};let p=&chr.modules.physics.position;
            let distance=((p.0-origin[0]).powi(2)+(p.1-origin[1]).powi(2)+(p.2-origin[2]).powi(2)).sqrt();
            let name=custom_buddy_item(group.buddy_param_id).and_then(erdtree_tower::rewards::lookup).map_or("原版召唤物",|i|i.name);
            format!("{name} · 生命 {}/{} · 距离 {distance:.1} 米{}",chr.modules.data.hp,chr.modules.data.max_hp,if group.disappear_requested{" · 正在离场"}else{""})
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
pub struct Menu {enemy_filter:String,item_query:String,items:Vec<(u32,String)>,searched:String,count:i32,bout:[usize;2],bout_team:usize,bout_both:bool}
const LEVELS:[&str;4]=["无","I","II","III"];
impl Menu {
    pub fn draw(&mut self,ui:&Ui,view:Option<&crate::runtime::View>) {
        if !open() {return;}
        let size=ui.io().display_size;
        let mut keep=true;
        ui.window(format!("爬塔调试  [{}]###tower-debug",crate::options::label(erdtree_tower::options::Hotkey::Debug))).opened(&mut keep).position([size[0]*0.55,size[1]*0.06],Condition::FirstUseEver).size([size[0]*0.42,size[1]*0.86],Condition::FirstUseEver).build(|| {
            let Some(view)=view else {ui.text("运行时尚未就绪");return;};
            ui.text_colored([0.9,0.8,0.5,1.0],&view.status);
            let Some(run)=view.run.as_ref() else {ui.text("尚未开始旅程：在大赐福开始新局后再使用");return;};
            ui.text(format!("第 {} 幕 · 已走 {} 层 · {:?}{}",run.act,run.visited.len(),run.phase,if run.debug{" · 调试旅程"}else{""}));
            if let Some(_bar)=ui.tab_bar("##debug-tabs") {
                if let Some(_tab)=ui.tab_item("处刑之链实测") {chain_test(ui,run);}
                if let Some(_tab)=ui.tab_item("Boss 测试") {self.boss_tests(ui,view,run);}
                if let Some(_tab)=ui.tab_item("新骨灰测试") {self.spirit_tests(ui,view,run);}
                if let Some(_tab)=ui.tab_item("战斗") {self.combat(ui,run);}
                if let Some(_tab)=ui.tab_item("构筑") {self.build(ui,run);}
                if let Some(_tab)=ui.tab_item("物品") {self.items(ui);}
                if let Some(_tab)=ui.tab_item("状态") {self.state(ui,run);}
                if let Some(_tab)=ui.tab_item("灵庙测试") {
                    ui.text_wrapped("单人快捷测试：进入引路人问号房，再选择前往龙王平台灵庙或领取 800 卢恩。此旅程计为调试，不计入正式解锁。");
                    ui.text_wrapped("复制前在右手装备目标武器；可先在物品页用拼音搜索领取测试武器。");
                    let _disabled=ui.begin_disabled(PARTY_ROLE.load(Ordering::Relaxed)!=0||run.phase!=erdtree_tower::Phase::Map||view.busy||view.travelling);
                    if ui.button("进入灵庙事件测试（单人）"){send(Command::MausoleumTest);}
                }
                if let Some(_tab)=ui.tab_item("事件测试") {
                    use erdtree_tower::events::Draw;
                    ui.text_wrapped("先开始旅程并停在路线图。点击后直接进入指定问号事件；隐形道路在试炼 NPC 的菜单里选择。测试旅程不计正式解锁。仅限独自游玩。");
                    let _disabled=ui.begin_disabled(PARTY_ROLE.load(Ordering::Relaxed)!=0||run.phase!=erdtree_tower::Phase::Map||view.busy||view.travelling);
                    for (label,draw) in [("试炼岔路（含异端塔隐形桥）",Draw::Trial),("恢复泉水",Draw::Spring),("骰子赌徒",Draw::Dice),("盲盒商人",Draw::Mystery),("词条重铸",Draw::Reforge),("重练炉",Draw::Furnace),("粪金龟",Draw::Scarabs),("灵庙引路人",Draw::Mausoleum),("多宝箱",Draw::Chests),("祭坛",Draw::Altar),("上锁宝箱",Draw::Locked)] {
                        if ui.button(label){send(Command::EventTest(draw));}
                    }
                }
                if let Some(_tab)=ui.tab_item("场地") {arenas(ui,run);}
            }
        });
        if !keep {OPEN.store(false,Ordering::Relaxed);}
    }
    fn spirit_tests(&mut self,ui:&Ui,view:&crate::runtime::View,run:&Run) {
        let blocked=PARTY_ROLE.load(Ordering::Relaxed)!=0||view.travelling||!view.ready||matches!(run.phase,erdtree_tower::Phase::Reward{..});
        ui.text_wrapped("12 种新骨灰，每次领取 1 份，可重复补充。装备到快捷栏后，在战斗场地手动召唤；检验胜利返还请使用召唤师。");
        if PARTY_ROLE.load(Ordering::Relaxed)!=0 {ui.text_disabled("实际组队时禁止发放调试物品；无缝入口独自游玩可用。");}
        {
            let _disabled=ui.begin_disabled(blocked||view.busy||run.phase!=erdtree_tower::Phase::Map);
            if ui.button("获得双生的灵唤铃") {send(Command::Ember(Ember::SpiritTwin,true));}
            if let Some(index)=crate::enemies::debug_encounters().iter().position(|group|group.len()==1&&group[0].model==3251) {
                ui.same_line();
                if ui.button("进入大树守卫测试战斗") {send(Command::BossTest(index as u16));}
            }
        }
        ui.text_disabled("测试战斗会切换到第一幕 Boss 节点，并标记为调试旅程。");
        if run.boss_test.is_some() {
            let _disabled=ui.begin_disabled(blocked);
            if ui.button("结束测试，返回大赐福##spirit") {send(Command::EndBossTest);}
        }
        ui.separator();
        {
            let _disabled=ui.begin_disabled(blocked);
            for (index,level) in [0,10,39].into_iter().enumerate() {
                if index>0 {ui.same_line();}
                if ui.button(format!("全部领取 +{level}")) {for (id,_) in new_spirits(level) {send(Command::SpiritItem(id));}}
            }
        }
        if let Ok(rows)=SUMMONS.lock() {
            ui.text(format!("当前本机召唤物：{}",rows.len()));
            for row in rows.iter(){ui.text_wrapped(row);}
        }
        ui.separator();
        ui.child_window("##new-spirit-list").size([0.0,0.0]).build(|| {
            let _disabled=ui.begin_disabled(blocked);
            for (base,name) in new_spirits(0) {
                ui.text(name);
                for level in [0,10,39] {
                    ui.same_line();
                    if ui.small_button(format!("+{level}##spirit-{base}-{level}")) {send(Command::SpiritItem(base+level));}
                }
            }
        });
    }
    fn boss_tests(&mut self,ui:&Ui,view:&crate::runtime::View,run:&Run) {
        let role=PARTY_ROLE.load(Ordering::Relaxed);let party=role!=0;
        ui.text_wrapped("选择任意 Boss 组合直接测试，旅程会标记为调试，不计入正式成绩与解锁。");
        ui.text_wrapped(if party {"联机需全员开启调试并完成领奖、誓约选择；仅房主可发起或结束测试。"}else{"请在路线图状态发起；试验条目也会列出，可能存在原版场景依赖。"});
        if let Some(test)=run.boss_test {
            let group=crate::enemies::debug_encounters().get(usize::from(test.encounter)).cloned().unwrap_or_default();
            ui.text_wrapped(format!("当前测试 #{}：{}",test.encounter,crate::enemies::debug_group_name(&group)));
            if let Some((hp,max))=view.enemy_hp {ui.text(format!("敌人总生命：{hp}/{max}"));}
            let _disabled=ui.begin_disabled(role==2);
            if ui.button("结束测试，返回大赐福") {send(Command::EndBossTest);}
        }
        ui.separator();ui.input_text("搜索 Boss",&mut self.enemy_filter).build();ime_here(ui);
        let groups=crate::enemies::debug_encounters();
        ui.text(format!("共 {} 个 Boss 组合；多实体组合整组测试",groups.len()));
        let ready=role!=2&&run.phase==erdtree_tower::Phase::Map&&!view.travelling&&!view.busy;
        {
            let _disabled=ui.begin_disabled(!ready||party);
            if ui.button("碎星拉塔恩：陨石阶段复现（单人）") {send(Command::RadahnTest);}
        }
        ui.text_wrapped("自动降至 40% 血量，玩家免伤；靠近中心开战，必要时轻击一次。仅记录，不强制落地。");
        ui.child_window("##boss-tests").size([0.0,0.0]).build(|| {
            for (index,group) in groups.iter().enumerate() {
                let names=crate::enemies::debug_group_name(group);
                if !self.enemy_filter.trim().is_empty()&&!names.to_lowercase().contains(&self.enemy_filter.trim().to_lowercase()){continue;}
                let available=crate::enemies::boss_test_allowed(group,party,erdtree_tower::dlc::enemies_allowed());
                let _disabled=ui.begin_disabled(!ready||available.is_err());
                if ui.button(format!("测试##boss-test-{index}")){send(Command::BossTest(index as u16));}
                drop(_disabled);ui.same_line();
                ui.text_wrapped(format!("#{index} 第{}幕 {names} · {} 只{}{}",group[0].act,group.len(),if group.iter().any(|e|e.dlc){" · DLC"}else{""},if group.iter().any(|e|e.retired){" · 试验"}else{""}));
                if let Err(reason)=available {ui.text_disabled(reason);}
            }
        });
    }
    fn combat(&mut self,ui:&Ui,run:&Run) {
        ui.text("跳到节点（路线图上会变成可选）");
        ui.child_window("##debug-nodes").size([0.0,220.0]).build(|| {
            for layer in 0..run.floors() {
                ui.text(format!("第 {:>2} 层",layer+1));
                for (id,node) in run.nodes.iter().enumerate().filter(|(_,n)|n.layer==layer) {
                    ui.same_line();
                    let label=format!("{}##node-{id}",node.kind.label());
                    if ui.small_button(label) {send(Command::Jump(id));}
                }
            }
        });
        ui.separator();
        ui.text_colored([0.95,0.75,0.45,1.0],"斗蛐蛐实验：下一场生成两只敌人并改阵营，你在这场不会受伤，请不要攻击");
        let pool=crate::enemies::debug_pool();
        let names:Vec<String>=pool.iter().map(|e|format!("第{}幕 {}",e.act,e.name)).collect();
        for (slot,label) in ["红方##bout-a","蓝方（改阵营）##bout-b"].into_iter().enumerate() {
            ui.set_next_item_width(260.0);ui.combo_simple_string(label,&mut self.bout[slot],&names);
        }
        let teams:Vec<String>=BOUT_TEAMS.iter().map(|t|format!("阵营 {t}")).collect();
        ui.set_next_item_width(120.0);ui.combo_simple_string("##bout-team",&mut self.bout_team,&teams);
        ui.same_line();ui.checkbox("两只都改阵营",&mut self.bout_both);
        ui.same_line();
        if ui.small_button("设为下一场##bout") {
            let (a,b)=(pool[self.bout[0].min(pool.len()-1)],pool[self.bout[1].min(pool.len()-1)]);
            set_roster(vec![a,b]);
            if let Ok(mut bout)=BOUT.lock(){*bout=Some(BoutSetup{team:BOUT_TEAMS[self.bout_team.min(BOUT_TEAMS.len()-1)],both:self.bout_both});}
        }
        ui.separator();
        // 血条颜色标签测试：需要先在 Windows 上运行 text-update 写入测试文字
        let mut tags=crate::boss_affixes::bar_tag_test();
        if ui.checkbox("血条颜色标签测试（主 Boss 血条换成带颜色标签的测试文字）",&mut tags) {crate::boss_affixes::set_bar_tag_test(tags);}
        // 物品名颜色标签测试：text-update 把原版道具“饶舌头颅”（2207）改成带颜色标签的名字，领取后在背包里查看
        ui.same_line();
        if ui.small_button("发放颜色标签测试道具") {
            match crate::items::give(&[(0x40000000|2207,1,-1)]) {Ok(())=>crate::log("Color tag test goods given"),Err(error)=>crate::log(format!("Color tag test goods failed: {error}"))}
        }
        ui.separator();
        ui.text(format!("下一场战斗的敌人：{}",pending_roster().unwrap_or_else(||"按路线正常抽取".into())));
        if ui.small_button("恢复正常抽取") {if let Ok(mut roster)=ROSTER.lock(){*roster=None;}if let Ok(mut bout)=BOUT.lock(){*bout=None;}}
        if self.count<1 {self.count=1;}
        ui.set_next_item_width(120.0);ui.input_int("数量",&mut self.count).build();self.count=self.count.clamp(1,4);
        ui.set_next_item_width(240.0);ui.input_text("筛选##enemy",&mut self.enemy_filter).build();ime_here(ui);
        let filter=self.enemy_filter.trim().to_owned();
        ui.child_window("##debug-enemies").size([0.0,0.0]).build(|| {
            ui.text_colored([0.7,0.7,0.7,1.0],"怪物池");
            for enemy in crate::enemies::debug_pool().into_iter().filter(|e|filter.is_empty()||e.name.contains(filter.as_str())) {
                let tier=match enemy.kind {NodeKind::Monster=>"普通",NodeKind::Elite=>"强敌",_=>"Boss"};
                let label=format!("第{}幕 {tier} {}{}##pool-{}",enemy.act,enemy.name,if enemy.dlc{"（DLC）"}else{""},enemy.npc);
                if ui.selectable(label) {set_roster(vec![enemy;self.count as usize]);}
            }
            ui.text_colored([0.7,0.7,0.7,1.0],"Boss 组（整组生成）");
            for (index,parts) in crate::enemies::debug_encounters().into_iter().enumerate() {
                if PARTY_ROLE.load(Ordering::Relaxed)==0&&crate::enemies::party_only_group(&parts){continue;}
                let name=crate::enemies::debug_group_name(&parts);
                if !filter.is_empty()&&!name.to_lowercase().contains(&filter.to_lowercase()) {continue;}
                let trial=if parts[0].retired {"（试验）"}else{""};
                if ui.selectable(format!("第{}幕 {name}（{} 只）{trial}##boss-{index}",parts[0].act,parts.len())) {set_roster(parts);}
            }
        });
    }
    fn build(&mut self,ui:&Ui,run:&Run) {
        ui.text("誓约等级");
        for kind in Kind::ALL {
            let level=run.contract.as_ref().and_then(|c|c.level(kind)).unwrap_or(0);
            ui.text(format!("{}：{}",kind.name(),LEVELS[usize::from(level.min(3))]));
            for (target,label) in LEVELS.iter().enumerate() {
                ui.same_line_with_pos(220.0+target as f32*56.0);
                if ui.small_button(format!("{label}##oath-{}-{target}",kind.index())) {send(Command::Oath(kind,target as u8));}
            }
        }
        ui.separator();ui.text("余烬");
        if ui.small_button("预览原生余烬获得窗口（不发奖）") {crate::ember_notice::preview(Ember::SpiritTwin);OPEN.store(false,Ordering::Relaxed);}
        for (i,ember) in Ember::ALL.into_iter().enumerate() {
            if i%3!=0 {ui.same_line_with_pos(10.0+(i%3) as f32*200.0);}
            let mut held=run.embers.contains(&ember);
            if ui.checkbox(format!("{}##ember-{i}",ember.name()),&mut held) {send(Command::Ember(ember,held));}
        }
        ui.text("双生的灵唤铃：保留第一组；召唤师按各组存活返还骨灰");
        ui.separator();
        ui.text(format!("苦难律法：{} 分 · 律法码 {}",run.law.score(),run.law.code()));
        ui.same_line();if ui.small_button("清空律法") {send(Command::Law(Law::default()));}
        // 与律法界面相同：一条路线一行，从左到右依次解锁
        let mut row=None;
        for (id,node) in NODES.iter().enumerate().filter(|&(id,_)|Law::listed(id)) {
            if row==Some(node.row) {ui.same_line_with_pos(10.0+f32::from(node.col-1)*150.0);} else {row=Some(node.row);}
            let mut has=run.law.has(id);
            let _disabled=ui.begin_disabled(!run.law.available(id));
            if ui.checkbox(format!("{}##law-{id}",node.name),&mut has) {send(Command::Law(run.law.toggle(id)));}
        }
    }
    fn items(&mut self,ui:&Ui) {
        // 传说武器流光：配色只在本次游戏内生效；“预览”让任意近战武器都发光
        ui.text("传说武器流光");
        let labels:Vec<_>=crate::weapon_glow::Style::ALL.iter().map(|s|s.label()).collect();
        let mut index=crate::weapon_glow::Style::ALL.iter().position(|s|*s==crate::weapon_glow::style()).unwrap_or(0);
        ui.set_next_item_width(320.0);
        if ui.combo_simple_string("##glow-style",&mut index,&labels) {crate::weapon_glow::set_style(crate::weapon_glow::Style::ALL[index]);}
        ui.same_line();
        let mut preview=crate::weapon_glow::preview();
        if ui.checkbox("预览：手上任意近战武器都发光",&mut preview) {crate::weapon_glow::set_preview(preview);}
        ui.separator();
        ui.text_wrapped("支持中文、拼音和首字母，例如：长剑 / changjian / cj；也可搜索物品说明。");
        ui.set_next_item_width(320.0);
        let enter=ui.input_text("##item-query",&mut self.item_query).enter_returns_true(true).build();ime_here(ui);
        ui.same_line();
        if (ui.button("搜索")||enter)||self.searched!=self.item_query {
            self.items=crate::reward_ui::search_items(&self.item_query,80);self.searched=self.item_query.clone();
        }
        ui.text(format!("{} 条结果（最多显示 80 条）",self.items.len()));
        ui.child_window("##debug-items").size([0.0,0.0]).build(|| {
            for (id,text) in &self.items {
                if ui.small_button(format!("获得##item-{id}")) {send(Command::Item(*id));}
                ui.same_line();ui.text(text);
            }
        });
    }
    fn state(&mut self,ui:&Ui,run:&Run) {
        ui.text("切换幕（回到该幕起点，生命与圣杯瓶回满）");
        for act in 1..=3u8 {
            if act>1 {ui.same_line();}
            if ui.button(format!("第 {act} 幕")) {send(Command::Act(act));}
        }
        ui.separator();
        if ui.button("回满生命、专注与圣杯瓶") {send(Command::Heal);}
        let blocked=CHEATS_BLOCKED.load(Ordering::Relaxed);
        let disabled=ui.begin_disabled(blocked);
        let (mut god,mut one_shot)=(GOD.load(Ordering::Relaxed),ONE_SHOT.load(Ordering::Relaxed));
        let changed=ui.checkbox("无敌（仅玩家免伤，可正常攻击）",&mut god)|{ui.same_line();ui.checkbox("秒杀（每次命中 9999999 伤害）",&mut one_shot)};
        if changed {GOD.store(god,Ordering::Relaxed);ONE_SHOT.store(one_shot,Ordering::Relaxed);send(Command::Cheats(god,one_shot));}
        let mut instant=INSTANT_BREAK.load(Ordering::Relaxed);
        if ui.checkbox("攻击立刻爆条（压低敌人架势，下一次削韧命中触发失衡）",&mut instant) {INSTANT_BREAK.store(instant,Ordering::Relaxed);send(Command::InstantBreak(instant));}
        drop(disabled);
        if blocked {ui.text_disabled("组队或无缝会话尚未就绪，暂不可启用；独自游玩可使用。");}
        if let Some(text)=NEAREST.lock().ok().and_then(|n|n.clone()) {crate::fancy_text::text_wrapped(ui,text);}
        if ui.button("卢恩 +10000") {send(Command::Runes(10000));}
        ui.same_line();if ui.button("卢恩 +100000") {send(Command::Runes(100000));}
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
    ui.text("当前单机／联机三幕统一使用龙王平台，场地选择暂时停用。");
    let _disabled=ui.begin_disabled(true);
    let chosen=crate::arenas::override_key();
    if ui.radio_button_bool("按主题",chosen.is_none()) {crate::arenas::set_override(None);}
    let built=crate::arenas::built();
    if built.is_empty() {crate::fancy_text::text_wrapped(ui,"还没有生成好的场地：在 Windows 上运行 scene-builder arena-build 后，把输出的 arenas-built.txt 与地图放进 scene-assets。");}
    for (key,name) in built {
        if ui.radio_button_bool(format!("{name}##arena-{key}"),chosen==Some(key)) {crate::arenas::set_override(Some(key));}
    }
    drop(_disabled);
    ui.separator();
    if ui.button("记下当前位置") {
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
fn chain_test(ui:&Ui,run:&Run) {
    ui.text_wrapped("测试会将本局标记为调试，不计正式成绩。只修改当前角色的处刑之链。");
    if ui.button("开始真实触发测试（获得余烬，重置为 0 层）") {send(Command::ChainTest(0));}
    if ui.button("设为 10 层，对照 +60% 致命一击倍率") {send(Command::ChainTest(10));}
    ui.separator();
    let (trace,count)=crate::embers::chain_trace();
    let level=run.ember_stacks.level(Ember::Chain);
    ui.text(format!("持有余烬：{}   当前层数：{level}",run.embers.contains(&Ember::Chain)));
    ui.text(format!("本次观察到的真实致命一击标记：{count} 次"));
    ui.text(format!("原生处决监听：{}   待处理处决事件：{}",trace.resident,trace.marker));
    let expected=erdtree_tower::embers::stack_rates(Ember::Chain,level).unwrap().attack;
    ui.text(format!("预期倍率：{expected:.2}x   实际参数：{}",trace.rate.map_or("未挂载".into(),|rate|format!("{rate:.2}x"))));
    let ok=trace.level==level&&trace.resident&&if level==0 {trace.effects==0}else{trace.effects==1&&trace.critical_only&&trace.rate.is_some_and(|r|(r-expected).abs()<0.0001)};
    ui.text_colored(if ok{[0.5,0.95,0.6,1.0]}else{[1.0,0.65,0.35,1.0]},if ok{"参数检查通过（不代表已完成实战伤害验证）"}else{"参数未就绪或与层数不符"});
    ui.text(format!("已挂载倍率效果数：{}   限定致命一击：{}",trace.effects,trace.critical_only));
    ui.separator();
    ui.text_wrapped("进入战斗后背刺或处决一次，再打开此页：真实触发次数应 +1，层数应 +1，倍率应增加 0.06。普通攻击不应加层。营地假人不走战斗叠层检测，请使用战斗或 Boss 测试入口。");
    ui.text_wrapped("10 层按钮仅用于手动对照参数，不计入真实触发次数。比较伤害请保持同一把武器、同一敌人及其他增益不变，并关闭秒杀。");
}
fn set_roster(roster:Vec<Enemy>) {
    crate::log(format!("DEBUG_MENU next roster: {}",roster.iter().map(|e|e.name).collect::<Vec<_>>().join(",")));
    if let Ok(mut slot)=ROSTER.lock(){*slot=Some(roster);}
}

#[cfg(test)]
mod tests {
    #[test]
    fn custom_spirit_test_lists_all_presets_and_matches_live_buddy_ids() {
        for level in [0,10,39] {
            let items=super::new_spirits(level);assert_eq!(items.len(),12);
            for (index,(id,_)) in items.iter().enumerate() {
                assert!(super::valid_spirit_test_item(*id));
                assert_eq!(super::custom_buddy_item(1600000000+index as i32*40000+level as i32*1000),Some(*id));
            }
        }
        assert!(super::new_spirits(40).is_empty());
        for id in [0,0x40000000|200000,0x40000000|270040,0x40000000|282000] {assert!(!super::valid_spirit_test_item(id));}
        for param in [0,1599999999,1600000001,1600480000] {assert_eq!(super::custom_buddy_item(param),None);}
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
