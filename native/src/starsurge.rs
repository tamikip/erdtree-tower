//! HansFix 试验：星辉术士新天赋（替换原来的「进入战斗回一半专注；魔力伤害 +15%」）
//! 一、蓝瓶持续回复：喝蓝瓶不再瞬间回蓝；每喝一瓶叠一层，每层每 0.1 秒回 1 点专注，回满即停并清空层数。
//!    瞬间回复靠把职业常驻效果 9810030 的「蓝瓶回复倍率」改成 0 去掉（运行时改参数，不动 regulation）。
//! 二、交替法术叠层：魔法弹体命中敌人时按 Cycle 的规则计层，每层使自己的魔法伤害提高（第 1、2 层 ×1.2，之后逐层递减、趋近 ×1.05，乘算）；
//!    打满上限（用过的不同法术数 × 3）的那一发，它的每一次伤害判定都在敌人身上引发海摩大炮的爆炸，伤害为那一下的 2 倍（BURST_FACTOR）。
//!    叠层期间身上有魔法之境的蓝色上升线条；屏幕左上（图标栏下方一行）显示「图标 当前层/上限」。
//!    命中识别：生成弹体时（作者 star_stream 的钩子）记下施法者与魔法；敌人掉血时，0.7 秒内落在 6 米内的自己的弹体算命中。
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64,Ordering};
use std::time::{Duration,Instant};
use eldenring::cs::{AtkParam_Pc,Bullet,ChrIns,ChrInsExt,FieldInsHandle,SoloParamRepository,SpEffectParam,WorldChrMan};
use fromsoftware_shared::FromStatic;
use erdtree_tower::{Class,Phase,Run};

/// 星辉术士的职业常驻效果（scene-builder ClassAssets：9810000 + 30）
pub const CLASS_EFFECT:i32=9810030;
/// 借用职业重做前已弃用的旧被动行：叠层增伤、爆炸伤害
const BUFF:i32=9800000;
/// 增伤轮换用的第二行（层数变化时写进另一行再挂上，避免同一帧对同一编号先摘后挂）
const BUFF_ALT:i32=9800002;
/// 通关返还蓝瓶时在身上播放的特效行（借用旧被动行）
const GIFT_FX:i32=9800003;
/// passives 会清掉职业重做前的旧被动行 9800000–9800004，这几行要跳过
pub const BUFF_ROWS:[i32;3]=[BUFF,BUFF_ALT,GIFT_FX];
/// 魔法之境给玩家的增益（1413000）用的视觉特效：蓝色上升线条
const TERRA_VFX:i32=1413000;
/// 返还蓝瓶的视觉特效：染蓝凶刀（暗杀者的天青短刀）致命一击回蓝时的原版特效（SpEffect 350602 用的 SpEffectVfx 350600）
const GIFT_VFX:i32=350600;
/// 作者用作中性模板的常驻效果行（只改了战技消耗，这里会改回 1）
const NEUTRAL_TEMPLATE:u32=9600011;
/// 打满那一发：最后一次看到它的弹体之后，这么久内的伤害判定仍然算它的
const BURST_CAST_GRACE:Duration=Duration::from_millis(1500);
/// 测试用：记下玩家身上新出现的特效（找粪金龟回蓝的原版特效）
const FX_TRACE:bool=false;
/// 没有名单时（商人、事件房间），身边多远内的非玩家角色算作目标
const NEARBY_REACH:f32=60.0;
/// 同一魔法这么久内生成的弹体算同一次施法
const SAME_CAST:Duration=Duration::from_millis(1000);
/// 原版「大型蓝露滴粪金龟」（击杀恢复一瓶蓝瓶）：调试台生成在身边，用于对照回蓝效果。
/// SCARAB_GIFT（在脚下生成并击杀、借原版回蓝特效）是试过的备用方案，目前 victory() 不再使用，返还蓝瓶改用染蓝凶刀特效
const CERULEAN_SCARAB:(i32,i32,i32)=(4190,41901000,41900000);
const SCARAB_DEBUG:u8=1;
const SCARAB_GIFT:u8=2;
/// 返还用的那只生成在脚下多深（看不见、听不见它本身的死亡）
const GIFT_SCARAB_DEPTH:f32=40.0;
static SCARAB_REQUEST:std::sync::atomic::AtomicU8=std::sync::atomic::AtomicU8::new(0);
/// 等待生成：(开始时间, 用途)
static SCARAB_PENDING:Mutex<Option<(Instant,u8)>>=Mutex::new(None);
/// 已生成的那只：(句柄, 用途, 阶段开始时间, 阶段)
static SCARAB_WATCH:Mutex<Option<(FieldInsHandle,u8,Instant,u8)>>=Mutex::new(None);
pub fn request_scarab() {SCARAB_REQUEST.store(SCARAB_DEBUG,Ordering::Release);}
fn watched_scarab()->Option<FieldInsHandle> {SCARAB_WATCH.lock().ok().and_then(|w|w.map(|w|w.0))}
/// 返还走不通时退回：直接按账本补一瓶，配自制特效
fn gift_fallback(why:&str) {
    super::log(format!("STARSURGE gift scarab fallback: {why}"));
    FLASK_GIFT.store(true,Ordering::Release);GIFT_SHOW.store(true,Ordering::Release);
}
/// 每帧调用：处理粪金龟的生成、击杀与返还
pub fn debug_tick(world:&mut WorldChrMan) {
    use eldenring::cs::{ChrDebugSpawnRequest,NpcParam,NpcThinkParam};
    let (chr_id,npc,think)=CERULEAN_SCARAB;
    if let Ok(mut watch)=SCARAB_WATCH.lock() {
        if let Some((h,purpose,at,stage))=*watch {
            let alive=world.chr_ins_by_handle(&h).map(|c|(c.modules.data.hp,{let q=c.modules.physics.position;[q.0,q.1,q.2]}));
            match alive {
                Some((hp,pos)) if hp>0=>if purpose==SCARAB_GIFT {
                    if stage==0 {
                        // 用玩家的海摩大炮打死它，算玩家击杀；炮弹攻击力先恢复原值
                        restore_haima(true);
                        if let Some(c)=world.chr_ins_by_handle_mut(&h) {c.modules.data.hp=1;}
                        let shot=spawn_haima(world,pos);
                        super::log(format!("STARSURGE gift scarab shot={shot:?}"));
                        *watch=Some((h,purpose,Instant::now(),1));
                    }else if stage==1&&at.elapsed()>Duration::from_millis(1200) {
                        if let Some(c)=world.chr_ins_by_handle_mut(&h) {c.modules.data.hp=0;}
                        super::log("STARSURGE gift scarab: shot missed, hp set to 0");
                        *watch=Some((h,purpose,Instant::now(),2));
                    }else if stage==2&&at.elapsed()>Duration::from_secs(3) {
                        *watch=None;gift_fallback("scarab would not die");
                    }
                },
                _=>{
                    super::log(format!("STARSURGE {} scarab died hp={:?}",if purpose==SCARAB_GIFT {"gift"}else{"debug"},alive.map(|a|a.0)));
                    if purpose==SCARAB_GIFT {FLASK_GIFT.store(true,Ordering::Release);}
                    *watch=None;
                }
            }
        }
    }
    let Ok(mut pending)=SCARAB_PENDING.lock() else{return};
    if let Some((at,purpose))=*pending {
        let created=world.debug_chr_creator.last_created_chr.is_some_and(|c|unsafe {c.as_ref()}.npc_param_id==npc);
        if created {
            if let (Some(c),Ok(mut watch))=(world.debug_chr_creator.last_created_chr,SCARAB_WATCH.lock()) {*watch=Some((unsafe {c.as_ref()}.field_ins_handle,purpose,Instant::now(),0));}
        }
        if created||at.elapsed()>Duration::from_secs(5) {
            if world.debug_chr_creator.init_data.npc_param_id==npc {world.debug_chr_creator.spawn=false;}
            super::log(format!("STARSURGE scarab purpose={purpose} spawned={created}"));*pending=None;
            if !created&&purpose==SCARAB_GIFT {gift_fallback("spawn timed out");}
        }
        return;
    }
    let purpose=SCARAB_REQUEST.load(Ordering::Acquire);
    if purpose==0||world.debug_chr_creator.spawn {return;}
    SCARAB_REQUEST.store(0,Ordering::Release);
    let fail=|why:&str|if purpose==SCARAB_GIFT {gift_fallback(why);}else{super::log(format!("STARSURGE debug scarab: {why}"));};
    // 返还要靠玩家的海摩炮弹击杀：还没施过法（没有生成数据）就直接退回
    if purpose==SCARAB_GIFT&&TEMPLATE.lock().ok().and_then(|t|*t).is_none() {fail("no cast template");return;}
    let Ok(params)=(unsafe {SoloParamRepository::instance()}) else{fail("params unavailable");return};
    if params.get::<NpcParam>(npc as u32).is_none()||params.get::<NpcThinkParam>(think as u32).is_none() {fail("params missing");return;}
    let Some(player)=world.main_player.as_ref() else{fail("no player");return};
    let physics=&player.chr_ins.modules.physics;
    let offset=if purpose==SCARAB_GIFT {[0.0,-GIFT_SCARAB_DEPTH,0.0]}else{[3.0,0.2,3.0]};
    let pos=[physics.position.0-player.block_position.x+offset[0],physics.position.1-player.block_position.y+offset[1],physics.position.2-player.block_position.z+offset[2]];
    let init=&mut world.debug_chr_creator.init_data;
    unsafe {
        let bytes=std::ptr::from_mut(init).cast::<u8>();
        bytes.add(0x10).cast::<fromsoftware_shared::F32Vector4>().write(fromsoftware_shared::F32Vector4(0.0,0.0,0.0,0.0));
        bytes.add(0xc8).write(0);
    }
    world.debug_chr_creator.last_created_chr=None;
    world.spawn_debug_character(&ChrDebugSpawnRequest {chr_id,npc_param_id:npc,npc_think_param_id:think,chara_init_param_id:0,event_entity_id:45011899,talk_id:-1,pos_x:pos[0],pos_y:pos[1],pos_z:pos[2]});
    *pending=Some((Instant::now(),purpose));
    super::log(format!("STARSURGE scarab requested purpose={purpose}"));
}
/// 海摩大炮的飞行弹体：在敌人头顶朝下发射，立刻命中，播放原版命中特效并分裂出爆炸弹体
const HAIMA_SHOT:i32=10408000;
/// 爆炸伤害 = 最后一发法术伤害 × BURST_FACTOR
const BURST_FACTOR:i32=2;
/// 生成爆炸炮弹时临时清零海摩大炮的攻击行，这么久之后恢复原值（炮弹早已落地）
const RESTORE_AFTER:Duration=Duration::from_millis(600);
/// 待恢复的攻击行：(恢复时间, [(AtkParam_Pc 编号, 物理/魔力/火/雷/圣原值)])
static ATK_RESTORE:Mutex<Option<(Instant,Vec<(u32,[u16;5])>)>>=Mutex::new(None);
/// 到时间就把海摩大炮的攻击行恢复原值；force 为真时立即恢复
fn restore_haima(force:bool) {
    let Ok(mut pending)=ATK_RESTORE.lock() else{return};
    let Some((at,rows))=pending.as_ref() else{return};
    if !force&&Instant::now()<*at {return;}
    if let Ok(params)=unsafe {SoloParamRepository::instance_mut()} {
        for (id,[phys,mag,fire,thun,dark]) in rows.iter().copied() {
            if let Some(row)=params.get_mut::<AtkParam_Pc>(id) {row.set_atk_phys(phys);row.set_atk_mag(mag);row.set_atk_fire(fire);row.set_atk_thun(thun);row.set_atk_dark(dark);}
        }
    }
    *pending=None;
}
const BURST_DAMAGE:i32=9800001;
/// 原版「只对魔法有效」的增伤护符行，叠层增伤从它复制（保留攻击类型过滤）
const SORCERY_TEMPLATE:u32=330010;
/// 海摩大炮命中时的爆炸弹体（原版；塔里的魔法用的是 8000xxxx 的复制行，这一行没人用）
const HAIMA_BURST:i32=10408001;
const FP_PER_SECOND:f64=10.0;
const MAX_STEP:f64=0.25;
/// 第 1、2 层每层 ×1.20；第 k 层（k≥3）×(1.05 + 0.15 × 0.85^(k-2))，越往后越接近 ×1.05
const FIRST_LAYERS:f32=1.2;
const LAYER_FLOOR:f32=1.05;
const LAYER_DECAY:f32=0.85;
/// 诊断日志（弹体、漏认、增伤核对、爆炸坐标）；测试时改成 true
const DIAG:bool=false;
const HIT_MEMORY:Duration=Duration::from_millis(700);
/// 弹体命中那一帧之前最后被看到的位置可能离敌人十几米（r2b 实测 9–18 米），放宽到 20 米、取最近的
const HIT_REACH:f32=20.0;
const FRESH_CAST:Duration=Duration::from_millis(2500);
/// 爆炸余波：爆炸后这段时间内，同一敌人掉血小于（引爆伤害/12 与 ECHO_FLOOR 取大）的不算新判定
const ECHO_WINDOW:Duration=Duration::from_millis(800);
const ECHO_FLOOR:i32=10;
/// 同一敌人两次爆炸的最小间隔
const ECHO_GAP:Duration=Duration::from_millis(80);
/// 敌人离弹道（最后位置沿速度方向延伸 HIT_MEMORY）多近算命中
const TRAJECTORY_REACH:f32=6.0;

/// 持续回复的状态；纯逻辑，便于测试
#[derive(Default,Debug,Clone,PartialEq)]
pub struct Regen {pub layers:u32,carry:f64}
impl Regen {
    /// drank：这一帧喝掉的蓝瓶数；返回新的专注值
    pub fn step(&mut self,drank:u32,fp:i32,max_fp:i32,dt:f64)->i32 {
        self.layers+=drank;
        if self.layers==0||max_fp<=0 {self.carry=0.0;return fp;}
        if fp>=max_fp {*self=Self::default();return fp;}
        self.carry+=f64::from(self.layers)*FP_PER_SECOND*dt.clamp(0.0,MAX_STEP);
        let add=self.carry.floor();self.carry-=add;
        let next=(i64::from(fp.max(0))+add as i64).min(i64::from(max_fp)) as i32;
        if next>=max_fp {*self=Self::default();}
        next
    }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Outcome {
    /// 叠到第 n 层
    Stack(u32),
    /// 漏了法术：从这一发重新开始（第 1 层）
    Restart,
    /// 单一法术连发满 3 发：清零，没有爆炸
    Wrap,
    /// 两种以上法术交替打满上限：爆炸并清零
    Burst,
}
/// 叠层规则（纯逻辑，记在玩家身上，打中哪个敌人都算）：
/// - 每一发命中的魔法算一层，第 N 发就是第 N 层。
/// - 只要和上一发不是同一个魔法就能叠层；上限 = 这一轮里用过的不同魔法数 × 3，用了新魔法上限立刻更新。
///   例：ABABAC → 6/9；ABABACABDC → 10/12。
/// - 连续两发是同一个魔法：清零，这一发算新一轮的第 1 层。
/// - 打满上限时爆炸并清零（只用一种魔法到不了上限）。
#[derive(Default,Debug,Clone,PartialEq)]
pub struct Cycle {known:Vec<u32>,last:Option<u32>,pub layers:u32}
impl Cycle {
    pub fn cap(&self)->u32 {3*self.known.len() as u32}
    pub fn hit(&mut self,spell:u32)->Outcome {
        if self.last==Some(spell) {
            *self=Self{known:vec![spell],last:Some(spell),layers:1};
            return Outcome::Restart;
        }
        if !self.known.contains(&spell) {self.known.push(spell);}
        self.last=Some(spell);
        self.layers+=1;
        if self.layers>=self.cap() {
            let burst=self.known.len()>=2;
            *self=Self::default();
            return if burst {Outcome::Burst}else{Outcome::Wrap};
        }
        Outcome::Stack(self.layers)
    }
}
/// k 层时的伤害倍率
pub fn layer_factor(layer:u32)->f32 {if layer<=2 {FIRST_LAYERS}else{LAYER_FLOOR+(FIRST_LAYERS-LAYER_FLOOR)*LAYER_DECAY.powi(layer as i32-2)}}
pub fn multiplier(layers:u32)->f32 {(1..=layers).map(layer_factor).product()}

/// 生成弹体时记下的一条：弹体编号、魔法（基础编号）、属于哪一次施法
#[derive(Clone,Copy)]
struct Spawned {bullet:i32,spell:u32,cast:u64,at:Instant}
/// 场上的一颗自己的弹体（最近一次看到的位置）
struct Live {handle:FieldInsHandle,spell:u32,cast:u64,pos:[f32;3],vel:[f32;3],seen:Instant}
#[derive(Default)]
struct Combat {cycle:Cycle,live:Vec<Live>,hp:Vec<(FieldInsHandle,i32)>,counted:Vec<u64>,applied:u32,
    /// 打满上限的那一次施法（之后它的每一次伤害判定都爆炸）与最后一次确认它还在场的时间
    burst_cast:Option<(u64,Instant)>,
    /// 爆炸自己扣的血：(敌人, 数值, 时间)，敌人掉血时先扣掉这一部分
    pending:Vec<(FieldInsHandle,i32,Instant)>,
    /// 最近一次爆炸：(敌人, 时间, 引发它的那一下伤害)。海摩炮弹攻击力清零后仍有几点残余伤害，
    /// 这段时间里明显偏小的掉血算爆炸余波，不当作新的伤害判定（否则会一帧接一帧连锁引爆）
    echo:Vec<(FieldInsHandle,Instant,i32)>,
    /// 当前挂着的增伤行；下一帧要核对的增伤；爆炸后要核对的敌人（编号、爆炸前生命、时间）
    row:Option<i32>,verify:Option<(i32,u32)>,burst_check:Option<(FieldInsHandle,i32,Instant)>,burst_seen:bool}
/// drank：上次通关以来喝过蓝瓶；topped：喝过之后专注满过
struct State {seed:u64,regen:Regen,blue:Option<u8>,last:Option<Instant>,drank:bool,topped:bool,seen:Vec<i32>}
/// 叠层显示：高 32 位为当前层数，低 32 位为上限
static HUD:AtomicU64=AtomicU64::new(0);
fn publish_hud(layers:u32,cap:u32) {HUD.store((u64::from(layers)<<32)|u64::from(cap),Ordering::Relaxed);}
/// 通关奖励：喝过蓝、之后没满过蓝，通关时送一瓶蓝露滴（由 runtime 在能拿到圣杯瓶账本的地方发放）
static FLASK_GIFT:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
pub fn victory_gift_due(drank:bool,topped:bool)->bool {drank&&!topped}
/// 通关时调用（heal_victory）
pub fn victory() {
    let Ok(mut guard)=STATE.lock() else{return};
    let Some(state)=guard.as_mut() else{return};
    let due=victory_gift_due(state.drank,state.topped);
    super::log(format!("STARSURGE victory drank={} topped={} gift={due}",state.drank,state.topped));
    // 直接按账本补一瓶，身上播放染蓝凶刀（暗杀者的天青短刀）致命一击回蓝的原版特效
    if due {FLASK_GIFT.store(true,Ordering::Release);GIFT_SHOW.store(true,Ordering::Release);}
    state.drank=false;state.topped=false;
}
pub fn take_flask_gift()->bool {FLASK_GIFT.swap(false,Ordering::AcqRel)}
/// 返还成功后在身上播放特效与音效（由 sync 在下一帧执行，那里拿得到角色）
static GIFT_SHOW:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
fn show_gift(chr:&mut ChrIns) {
    if !GIFT_SHOW.swap(false,Ordering::AcqRel) {return;}
    if let Ok(params)=unsafe {SoloParamRepository::instance_mut()} {
        let template=params.get::<SpEffectParam>(NEUTRAL_TEMPLATE).map(|r|std::ptr::from_ref(r));
        if let (Some(template),Some(row))=(template,params.get_mut::<SpEffectParam>(GIFT_FX as u32)) {
            unsafe {std::ptr::copy_nonoverlapping(template,std::ptr::from_mut(row),1);}
            row.set_icon_id(-1);row.set_effect_endurance(1.5);row.set_arts_consumption_rate(1.0);
            row.set_sp_category(0);row.set_category_priority(0);row.set_save_category(-1);row.set_dont_delete_on_dead(false);
            row.set_vfx_id(GIFT_VFX);
        }
    }
    if chr.special_effect.entries().any(|e|e.param_id==GIFT_FX) {chr.remove_speffect(GIFT_FX);}
    chr.apply_speffect(GIFT_FX,false);
    super::log("STARSURGE victory flask fx");
}
static STATE:Mutex<Option<State>>=Mutex::new(None);
static COMBAT:Mutex<Option<Combat>>=Mutex::new(None);
/// 本机星辉术士的 FieldInsHandle（按 u64 读）；不是星辉术士时为 0
static OWNER:AtomicU64=AtomicU64::new(0);
static SPAWNS:Mutex<Vec<Spawned>>=Mutex::new(Vec::new());
static NEXT_CAST:AtomicU64=AtomicU64::new(1);
/// 诊断日志计数（各自有上限，避免刷屏）
static DIAG_SPAWNS:AtomicU64=AtomicU64::new(0);
static DIAG_MISSES:AtomicU64=AtomicU64::new(0);
static DIAG_ANY:AtomicU64=AtomicU64::new(0);
static DIAG_FOREIGN:AtomicU64=AtomicU64::new(0);
static UNKNOWN_SPAWNS:Mutex<Vec<i32>>=Mutex::new(Vec::new());
/// 最近一次自己施放魔法时的生成数据，爆炸弹体以它为模板
static TEMPLATE:Mutex<Option<[u8;0x110]>>=Mutex::new(None);

fn handle_bits(handle:&FieldInsHandle)->u64 {unsafe {std::ptr::read_unaligned((handle as *const FieldInsHandle).cast::<u64>())}}
/// 物品编号是不是魔法（含品质变体），是则返回基础编号
fn sorcery(goods:i32)->Option<u32> {
    if goods<=0 {return None;}
    let base=erdtree_tower::spell_affixes::base(0x40000000|goods as u32);
    if erdtree_tower::rewards::lookup(base).is_some_and(|i|i.kind==erdtree_tower::rewards::ItemKind::Sorcery) {return Some(base);}
    // 奖励表里没有的魔法（如岩石球）：按原版编号段认，本体 4000–4999、DLC 2004000–2004999
    let plain=base&!0x40000000;
    ((4000..=4999).contains(&plain)||(2004000..=2004999).contains(&plain)).then_some(base)
}
fn hit_child(bullet:i32)->Option<i32> {
    let params=unsafe {SoloParamRepository::instance()}.ok()?;
    params.get::<Bullet>(bullet as u32).map(|r|r.hit_bullet_id()).filter(|id|*id>0)
}

/// 由 star_stream 的生成弹体钩子调用（游戏线程，必须很快）
pub fn observe_spawn(data:&[u8;0x110]) {
    let owner=OWNER.load(Ordering::Relaxed);
    let from=u64::from_le_bytes(data[0..8].try_into().unwrap());
    if DIAG&&owner!=0&&DIAG_ANY.fetch_add(1,Ordering::Relaxed)<10 {
        super::log(format!("STARSURGE_ANY from={from:#x} me={owner:#x} bullet={} magic={}",i32::from_le_bytes(data[0x14..0x18].try_into().unwrap()),i32::from_le_bytes(data[0x0c..0x10].try_into().unwrap())));
    }
    if owner==0 {return;}
    let bullet=i32::from_le_bytes(data[0x14..0x18].try_into().unwrap());
    // 命中后分裂出来的子弹体，生成数据里的归属可能不是玩家（岩石球就是这样）：是自己弹体的子弹体也照收
    if from!=owner&&!SPAWNS.try_lock().is_ok_and(|s|s.iter().any(|p|hit_child(p.bullet)==Some(bullet)&&p.at.elapsed()<Duration::from_secs(5))) {return;}
    if bullet==HAIMA_BURST {return;}
    let behavior=i32::from_le_bytes(data[0x08..0x0c].try_into().unwrap());
    let magic=i32::from_le_bytes(data[0x0c..0x10].try_into().unwrap());
    let goods=i32::from_le_bytes(data[0x18..0x1c].try_into().unwrap());
    if DIAG&&DIAG_SPAWNS.fetch_add(1,Ordering::Relaxed)<40 {
        super::log(format!("STARSURGE_SPAWN bullet={bullet} behavior={behavior} magic={magic} goods={goods} sorcery={:?}",sorcery(goods).or_else(||sorcery(magic))));
    }
    // 施放魔法时物品编号一栏可能为空，魔法编号记在 magic 栏
    let goods=if sorcery(goods).is_some() {goods}else{magic};
    let now=Instant::now();
    let Ok(mut spawns)=SPAWNS.try_lock() else{return};
    spawns.retain(|s|now.duration_since(s.at)<Duration::from_secs(5));
    // 命中后分裂出来的子弹体继承母弹的那一次施法
    let parent=spawns.iter().rev().find(|s|hit_child(s.bullet)==Some(bullet)).copied();
    let (spell,cast)=if let Some(p)=parent {(p.spell,p.cast)} else {
        let Some(spell)=sorcery(goods) else{
            // 认不出的自己的弹体：每种记一次，方便查哪些魔法没被识别
            if let Ok(mut seen)=UNKNOWN_SPAWNS.try_lock() {
                if !seen.contains(&bullet)&&seen.len()<40 {seen.push(bullet);super::log(format!("STARSURGE_UNKNOWN bullet={bullet} behavior={behavior} magic={magic} goods={goods}"));}
            }
            return;
        };
        // 同一魔法 1 秒内连着生成的弹体算同一次施法：岩石弹的三颗、辉剑的多把；辉石魔砾一次施法会先后生成两颗（相隔约 0.57 秒）
        let same=spawns.last().filter(|s|s.spell==spell&&now.duration_since(s.at)<SAME_CAST).map(|s|s.cast);
        if same.is_none() {if let Ok(mut t)=TEMPLATE.try_lock(){*t=Some(*data);}}
        (spell,same.unwrap_or_else(||NEXT_CAST.fetch_add(1,Ordering::Relaxed)))
    };
    if spawns.len()<128 {spawns.push(Spawned{bullet,spell,cast,at:now});}
}

fn active(run:Option<&Run>,chr:&ChrIns)->bool {
    run.is_some_and(|r|r.class==Class::Astrologer&&!matches!(r.phase,Phase::Dead|Phase::Won))&&!chr.chr_flags1c5.death_flag()&&chr.modules.data.hp>0
}

/// 去掉蓝瓶的瞬间回复和原来的魔力伤害 +15%
fn patch_class_effect() {
    let Ok(params)=(unsafe {SoloParamRepository::instance_mut()}) else{return};
    let Some(row)=params.get_mut::<SpEffectParam>(CLASS_EFFECT as u32) else{return};
    if row.change_mp_estus_flask_correct_rate()!=0.0 {row.set_change_mp_estus_flask_correct_rate(0.0);super::log("STARSURGE class effect: FP flask instant restore removed");}
    if row.magic_attack_rate()!=1.0 {row.set_magic_attack_rate(1.0);}
}

/// blue：圣杯瓶账本里剩余的蓝瓶数
pub fn sync(chr:&mut ChrIns,run:Option<&Run>,blue:Option<u8>) {
    patch_class_effect();
    restore_haima(false);
    let Ok(mut guard)=STATE.lock() else{return};
    // 旅程结束、换了职业：清掉一切；过场、切换房间时 run 暂时为空，只暂停，记录保留
    if run.is_some_and(|r|r.class!=Class::Astrologer||matches!(r.phase,Phase::Dead|Phase::Won)) {
        if OWNER.swap(0,Ordering::Relaxed)!=0||guard.is_some() {end_combat(chr);}
        *guard=None;return;
    }
    if !active(run,chr) {OWNER.store(0,Ordering::Relaxed);return;}
    let seed=run.map_or(0,|r|r.seed);
    // 新的一局：清掉上一局的记录与叠层
    if guard.as_ref().is_some_and(|s|s.seed!=seed) {*guard=None;end_combat(chr);}
    OWNER.store(handle_bits(&chr.field_ins_handle),Ordering::Relaxed);
    let now=Instant::now();
    let state=guard.get_or_insert_with(||State{seed,regen:Regen::default(),blue,last:None,drank:false,topped:false,seen:vec![]});
    let drank=match (state.blue,blue) {(Some(before),Some(after)) if after<before=>u32::from(before-after),_=>0};
    if FX_TRACE&&state.blue!=blue {super::log(format!("STARSURGE blue flasks {:?}->{blue:?}",state.blue));}
    state.blue=blue;
    // 返还判定看最后一次喝蓝之后：每喝一瓶重新开始
    if drank>0 {state.drank=true;state.topped=false;}
    else if state.drank&&chr.modules.data.max_fp>0&&chr.modules.data.fp>=chr.modules.data.max_fp {state.topped=true;}
    if FX_TRACE {
        let fresh:Vec<i32>=chr.special_effect.entries().map(|e|e.param_id).filter(|id|!state.seen.contains(id)).collect();
        for id in fresh {if state.seen.len()<400 {state.seen.push(id);super::log(format!("STARSURGE_FX new effect on player: {id}"));}}
    }
    let dt=state.last.map_or(0.0,|t|now.duration_since(t).as_secs_f64());state.last=Some(now);
    let before=state.regen.layers;
    show_gift(chr);
    let vitals=&mut chr.modules.data;
    vitals.fp=state.regen.step(drank,vitals.fp,vitals.max_fp,dt);
    if drank>0||before!=state.regen.layers {super::log(format!("STARSURGE regen drank={drank} layers {before}->{} fp={}/{}",state.regen.layers,vitals.fp,vitals.max_fp));}
}

/// 把一行增伤写成「只对魔法有效、五种属性 ×multiplier」
fn write_buff(id:i32,layers:u32)->bool {
    let Ok(params)=(unsafe {SoloParamRepository::instance_mut()}) else{return false};
    let Some(template)=params.get::<SpEffectParam>(SORCERY_TEMPLATE).map(|r|std::ptr::from_ref(r)) else{return false};
    let Some(row)=params.get_mut::<SpEffectParam>(id as u32) else{return false};
    unsafe {std::ptr::copy_nonoverlapping(template,std::ptr::from_mut(row),1);}
    // 与作者复制 330010 时一致（AffixAssets/ContractAssets）：不与护符共用类别、不存档；否则挂不上
    row.set_icon_id(-1);row.set_effect_endurance(-1.0);
    row.set_sp_category(0);row.set_category_priority(0);row.set_save_category(-1);row.set_dont_delete_on_dead(false);
    let rate=multiplier(layers);
    row.set_physics_attack_rate(rate);row.set_magic_attack_rate(rate);row.set_fire_attack_rate(rate);row.set_thunder_attack_rate(rate);row.set_dark_attack_rate(rate);
    // 魔法之境同款的蓝色上升线条
    row.set_vfx_id(TERRA_VFX);
    true
}
/// 层数变化时：新倍率写进没在用的那一行并挂上，再摘掉旧的那一行（两个编号不同，同一帧先挂后摘不会互相抵消）
fn apply_buff(chr:&mut ChrIns,layers:u32,combat:&mut Combat) {
    // 上一帧挂上的增伤，这一帧核对一次
    if let Some((id,expected))=combat.verify.take().filter(|_|DIAG) {
        let present=chr.special_effect.entries().any(|e|e.param_id==id);
        let rate=unsafe {SoloParamRepository::instance()}.ok().and_then(|p|p.get::<SpEffectParam>(id as u32).map(|r|(r.magic_attack_rate(),r.fire_attack_rate())));
        super::log(format!("STARSURGE_BUFF verify row={id} layers={expected} present={present} rate={rate:?}"));
    }
    if layers==combat.applied {return;}
    combat.applied=layers;
    let old=combat.row.take();
    if layers>0 {
        let next=if old==Some(BUFF) {BUFF_ALT}else{BUFF};
        if write_buff(next,layers) {chr.apply_speffect(next,false);combat.row=Some(next);combat.verify=Some((next,layers));}
    }
    if let Some(id)=old {chr.remove_speffect(id);}
    for id in [BUFF,BUFF_ALT] {if Some(id)!=combat.row&&Some(id)!=old&&chr.special_effect.entries().any(|e|e.param_id==id) {chr.remove_speffect(id);}}
}
fn end_combat(chr:&mut ChrIns) {
    if let Ok(mut combat)=COMBAT.lock() {
        if let Some(c)=combat.as_mut() {apply_buff(chr,0,c);}
        *combat=None;
    }
    publish_hud(0,0);
}

/// 叠层显示的位置与大小（按 1080p 的像素，按屏幕高度缩放）；可在模组根目录的 starsurge-hud.ini 里改：x=、y=、size=
static HUD_LAYOUT:std::sync::LazyLock<[f32;3]>=std::sync::LazyLock::new(|| {
    let mut layout=[150.0f32,160.0,34.0];
    if let Ok(text)=std::fs::read_to_string(crate::LOG_PATH.with_file_name("starsurge-hud.ini")) {
        for line in text.lines() {
            let Some((k,v))=line.split_once('=') else{continue};
            let Ok(v)=v.trim().parse::<f32>() else{continue};
            match k.trim() {"x"=>layout[0]=v,"y"=>layout[1]=v,"size"=>layout[2]=v.max(8.0),_=>{}}
        }
    }
    layout
});
/// 星辉术士叠层图标（魔法之境的状态图标 20007 加浅蓝色滤镜，见 tools/hansfix/extract_icon.py）
pub const HUD_ICON:u32=990900;
/// 每帧绘制：图标 + 「当前层/上限」，数字与图标同高、紧贴图标
/// 叠层显示的淡入淡出：(当前透明度, 上一帧时间)
static HUD_FADE:Mutex<(f32,Option<Instant>)>=Mutex::new((0.0,None));
/// 原生状态栏是否显示（游戏把 HUD 整个关掉时不画）
fn native_hud_shown()->bool {
    unsafe {eldenring::cs::CSFeManImp::instance()}.ok().is_none_or(|fe|fe.hud_state!=eldenring::cs::CSFeManHudState::HideAll)
}
/// shown：调用方判断的「正在游戏中」（不在读图、过场、旅程界面）
pub fn draw_hud(ui:&hudhook::imgui::Ui,icon:&mut dyn FnMut(u32)->Option<hudhook::imgui::TextureId>,shown:bool) {
    let packed=HUD.load(Ordering::Relaxed);
    let (layers,cap)=((packed>>32) as u32,packed as u32);
    let visible=shown&&OWNER.load(Ordering::Relaxed)!=0&&layers>0&&cap>0&&native_hud_shown();
    // 跟原生状态栏一样淡入（0.25 秒）淡出（0.4 秒）
    let alpha={
        let Ok(mut fade)=HUD_FADE.lock() else{return};
        let now=Instant::now();let dt=fade.1.map_or(0.0,|t|now.duration_since(t).as_secs_f32()).min(0.1);fade.1=Some(now);
        fade.0=if visible {(fade.0+dt/0.25).min(1.0)}else{(fade.0-dt/0.4).max(0.0)};
        fade.0
    };
    if alpha<=0.0||layers==0||cap==0 {return;}
    use crate::fancy_text::{Effect,Layer,Style,draw};
    let s=(ui.io().display_size[1]/1080.0).max(0.5);
    let [x,y,size]=HUD_LAYOUT.map(|v|v*s);
    if let Some(texture)=icon(HUD_ICON) {
        ui.get_foreground_draw_list().add_image(texture,[x,y],[x+size,y+size]).col(hudhook::imgui::ImColor32::from_rgba(255,255,255,(alpha*255.0) as u8)).build();
    }
    let text=format!("{layers}/{cap}");
    let full=layers+1>=cap;
    let color=if full {[150,230,255]}else{[205,228,255]};
    draw(Layer::Foreground,[x+size+3.0*s,y+size*0.06],size*0.86,&text,&Style{color,accent:[255,255,255],effect:Effect::Plain,glow:true},ui.time() as f32,alpha);
}

/// 爆炸伤害直接扣在敌人的生命上，并同步记下的生命值：这一下不会被当成新的伤害判定（不会一帧接一帧地自己引爆）
fn burst_damage(world:&mut WorldChrMan,combat:&mut Combat,handle:&FieldInsHandle,amount:i32) {
    let Some(chr)=world.chr_ins_by_handle_mut(handle) else{return};
    let hp=chr.modules.data.hp.saturating_sub(amount.max(0)).max(0);
    chr.modules.data.hp=hp;
    if let Some(entry)=combat.hp.iter_mut().find(|(h,_)|h==handle) {entry.1=hp;}
}

fn distance(a:[f32;3],b:[f32;3])->f32 {((a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)+(a[2]-b[2]).powi(2)).sqrt()}
/// 点 foe 到线段 [pos, pos+vel*HIT_MEMORY] 的距离（弹体停住或速度异常时退化为到 pos 的距离）
pub fn trajectory_distance(pos:[f32;3],vel:[f32;3],foe:[f32;3])->f32 {
    let t=HIT_MEMORY.as_secs_f32();
    let seg=vel.map(|v|v*t);
    let len2=seg.iter().map(|s|s*s).sum::<f32>();
    if !len2.is_finite()||len2<1e-4 {return distance(pos,foe);}
    let rel=[foe[0]-pos[0],foe[1]-pos[1],foe[2]-pos[2]];
    let k=((rel[0]*seg[0]+rel[1]*seg[1]+rel[2]*seg[2])/len2).clamp(0.0,1.0);
    distance([pos[0]+seg[0]*k,pos[1]+seg[1]*k,pos[2]+seg[2]*k],foe)
}

/// 战斗中每帧调用：foes 为本场敌人
pub fn fight(world:&mut WorldChrMan,foes:&[FieldInsHandle]) {
    let owner=OWNER.load(Ordering::Relaxed);
    let Ok(mut guard)=COMBAT.lock() else{return};
    if owner==0 {return;}
    let combat=guard.get_or_insert_with(Combat::default);
    let now=Instant::now();
    // 商人、事件房间等没有名单的场合：身边 60 米内所有活着的非玩家角色都算（名单敌人照旧在前）
    let foes:Vec<FieldInsHandle>={
        let mut all=foes.to_vec();
        let scarab=watched_scarab();
        let players:Vec<FieldInsHandle>=world.player_chr_set.characters().map(|p|p.chr_ins.field_ins_handle).chain(world.main_player.iter().map(|p|p.chr_ins.field_ins_handle)).collect();
        let me=world.main_player.as_ref().map(|p|{let q=p.chr_ins.modules.physics.position;[q.0,q.1,q.2]});
        if let Some(me)=me {
            for chr in world.chr_sets.iter().flatten().flat_map(|set|set.characters()) {
                let h=chr.field_ins_handle;
                if players.contains(&h)||all.contains(&h)||scarab==Some(h)||chr.modules.data.hp<=0 {continue;}
                let q=chr.modules.physics.position;
                if distance([q.0,q.1,q.2],me)<=NEARBY_REACH {all.push(h);}
            }
        }
        all
    };
    let foes=&foes[..];
    // 自己的弹体：对上生成记录，记下位置
    let spawns:Vec<Spawned>=SPAWNS.lock().map(|s|s.clone()).unwrap_or_default();
    if let Ok(manager)=unsafe {eldenring::cs::CSBulletManager::instance()} {
        for bullet in manager.bullets() {
            // 不按弹体归属筛：有的魔法（如岩石球）分裂出的弹体归属可能不是玩家；只认编号对得上自己 2 秒内生成记录的
            let mine=handle_bits(&bullet.targeting_owner.owner_chr_handle)==owner;
            let q=bullet.physics.position;let pos=[q.0,q.1,q.2];let v=bullet.physics.velocity;let vel=[v.0,v.1,v.2];
            if let Some(live)=combat.live.iter_mut().find(|l|l.handle==bullet.field_ins_handle) {live.pos=pos;live.vel=vel;live.seen=now;continue;}
            let ids=[bullet.fly_state.base.param.param_id,bullet.wait_state.base.param.param_id,bullet.exp_state.base.param.param_id];
            let Some(s)=spawns.iter().rev().find(|s|ids.contains(&s.bullet)&&now.duration_since(s.at)<Duration::from_secs(2)) else{continue};
            if !mine&&DIAG_FOREIGN.fetch_add(1,Ordering::Relaxed)<10 {super::log(format!("STARSURGE_FOREIGN bullet ids={ids:?} spell={}",s.spell));}
            combat.live.push(Live{handle:bullet.field_ins_handle,spell:s.spell,cast:s.cast,pos,vel,seen:now});
        }
    }
    combat.live.retain(|l|now.duration_since(l.seen)<=HIT_MEMORY);
    // 敌人掉血：附近有自己的弹体就算这一发魔法命中
    let mut hits=Vec::new();
    for handle in foes {
        let Some(chr)=world.chr_ins_by_handle(handle) else{continue};
        let hp=chr.modules.data.hp;let q=chr.modules.physics.position;let pos=[q.0,q.1+1.0,q.2];
        let before=match combat.hp.iter_mut().find(|(h,_)|h==handle) {Some(entry)=>std::mem::replace(&mut entry.1,hp),None=>{combat.hp.push((*handle,hp));continue;}};
        if hp>=before {continue;}
        // 爆炸自己扣的血不算新的伤害判定：掉血里先扣掉它（0.6 秒内到账）
        let mut drop=before-hp;
        combat.pending.retain(|(_,_,t)|now.duration_since(*t)<Duration::from_millis(600));
        if let Some(i)=combat.pending.iter().position(|(h,amount,_)|h==handle&&drop>=*amount-1) {drop-=combat.pending[i].1;combat.pending.remove(i);}
        if drop<=0 {continue;}
        combat.echo.retain(|(_,t,_)|now.duration_since(*t)<ECHO_WINDOW);
        if combat.echo.iter().any(|(h,_,trigger)|h==handle&&drop<(trigger/12).max(ECHO_FLOOR)) {continue;}
        // 先按弹道：从最后被看到的位置沿速度方向延伸到 0.7 秒后，敌人离这段弹道 6 米内；再退回到离最后位置 20 米内最近的
        // 范围内同时有旧施法（已计过）和新施法的弹体时，优先算给还没计过的新施法（打满那一发除外，它要继续炸）
        let stale=|cast:u64|combat.counted.contains(&cast)&&combat.burst_cast.is_none_or(|(c,_)|c!=cast);
        let pick=|reach:f32,dist:&dyn Fn(&Live)->f32|combat.live.iter().map(|l|(stale(l.cast),dist(l),l)).filter(|(_,d,_)|*d<=reach)
            .min_by(|a,b|a.0.cmp(&b.0).then(a.1.total_cmp(&b.1))).map(|(_,_,l)|(l.spell,l.cast));
        let along=pick(TRAJECTORY_REACH,&|l|trajectory_distance(l.pos,l.vel,pos));
        let near=pick(HIT_REACH,&|l|distance(l.pos,pos));
        // 贴脸施放时弹体可能在被看到之前就已命中：算给 1 秒内刚生成、还没算过的那一次施法
        let fresh=spawns.iter().rev().find(|s|now.duration_since(s.at)<=FRESH_CAST&&!combat.counted.contains(&s.cast)).map(|s|(s.spell,s.cast));
        let nearest=along.filter(|(_,c)|!stale(*c)).or(near.filter(|(_,c)|!stale(*c))).or(fresh).or(along).or(near);
        if let Some((spell,cast))=nearest {hits.push((*handle,pos,drop,spell,cast));}
        else if spawns.iter().any(|s|now.duration_since(s.at)<=Duration::from_millis(1500))&&DIAG_MISSES.fetch_add(1,Ordering::Relaxed)<60 {
            let closest=combat.live.iter().map(|l|distance(l.pos,pos)).min_by(f32::total_cmp);
            let path=combat.live.iter().map(|l|trajectory_distance(l.pos,l.vel,pos)).min_by(f32::total_cmp);
            super::log(format!("STARSURGE_MISS damage={} live={} closest={closest:?} path={path:?} spawns={} foe=[{:.1},{:.1},{:.1}]",before-hp,combat.live.len(),spawns.len(),pos[0],pos[1],pos[2]));
        }
    }
    // 打满那一发：它的弹体还在场就续期
    if let Some((cast,seen))=combat.burst_cast.as_mut() {
        if combat.live.iter().any(|l|l.cast==*cast) {*seen=now;}
        if now.duration_since(*seen)>BURST_CAST_GRACE {combat.burst_cast=None;}
    }
    for (handle,pos,damage,spell,cast) in hits {
        // 打满那一发的后续每一次伤害判定（多段、多个敌人）都各炸一次
        if combat.burst_cast.is_some_and(|(c,_)|c==cast) {
            // 同一个敌人两次爆炸至少间隔 ECHO_GAP，避免同一跳伤害分几帧到账时连炸
            if combat.echo.iter().any(|(h,t,_)|*h==handle&&now.duration_since(*t)<ECHO_GAP) {continue;}
            super::log(format!("STARSURGE_HIT spell={spell} cast={cast} damage={damage} outcome=BurstFollowUp"));
            let amount=burst(world,&handle,pos,damage);burst_damage(world,combat,&handle,amount);
            combat.echo.push((handle,now,damage));
            continue;
        }
        if combat.counted.contains(&cast) {continue;}
        combat.counted.push(cast);if combat.counted.len()>64 {combat.counted.remove(0);}
        let outcome=combat.cycle.hit(spell);
        super::log(format!("STARSURGE_HIT spell={spell} cast={cast} damage={damage} outcome={outcome:?} layers={} cap={}",combat.cycle.layers,combat.cycle.cap()));
        if outcome==Outcome::Burst {
            let hp=world.chr_ins_by_handle(&handle).map_or(0,|c|c.modules.data.hp);
            let amount=burst(world,&handle,pos,damage);burst_damage(world,combat,&handle,amount);
            combat.echo.push((handle,now,damage));
            combat.burst_cast=Some((cast,now));
            combat.burst_check=Some((handle,hp,now));combat.burst_seen=false;
        }
    }
    // 爆炸后 1 秒内找场上的爆炸弹体（只记第一次看到）
    if let Some((_,_,at))=combat.burst_check {
        if DIAG&&now.duration_since(at)<=Duration::from_secs(1)&&!combat.burst_seen {
            if let Ok(manager)=unsafe {eldenring::cs::CSBulletManager::instance()} {
                for bullet in manager.bullets() {
                    let ids=[bullet.fly_state.base.param.param_id,bullet.wait_state.base.param.param_id,bullet.exp_state.base.param.param_id];
                    if !ids.contains(&HAIMA_BURST)&&!ids.contains(&HAIMA_SHOT) {continue;}
                    let q=bullet.physics.position;
                    super::log(format!("STARSURGE_BURST bullet seen after {}ms at [{:.1},{:.1},{:.1}] ids={ids:?} mine={}",now.duration_since(at).as_millis(),q.0,q.1,q.2,handle_bits(&bullet.targeting_owner.owner_chr_handle)==owner));
                    combat.burst_seen=true;break;
                }
            }
        }
    }
    // 爆炸后 0.5 秒核对敌人生命
    if let Some((handle,before,at))=combat.burst_check {
        if now.duration_since(at)>=Duration::from_millis(500) {
            let after=world.chr_ins_by_handle(&handle).map(|c|c.modules.data.hp);
            if DIAG {super::log(format!("STARSURGE_BURST check hp_before={before} hp_after={after:?}"));}
            combat.burst_check=None;
        }
    }
    let layers=combat.cycle.layers;
    publish_hud(layers,combat.cycle.cap());
    if let Some(player)=world.main_player.as_mut() {apply_buff(&mut player.chr_ins,layers,combat);}
}

/// 在 pos 头顶 2.5 米朝下发射一发海摩大炮炮弹（借用最近一次施法的生成数据，归属玩家）
fn spawn_haima(world:&WorldChrMan,pos:[f32;3])->Option<Result<(),i32>> {
    let template=TEMPLATE.lock().ok().and_then(|t|*t);
    let me=world.main_player.as_ref().map(|p|{let q=p.chr_ins.modules.physics.position;[q.0,q.1,q.2]});
    let cast_at=template.map(|d|[0x80,0x84,0x88].map(|o|f32::from_le_bytes(d[o..o+4].try_into().unwrap())));
    if DIAG {super::log(format!("STARSURGE_BURST coords template_pos={cast_at:?} player={me:?} foe={pos:?}"));}
    template.map(|mut data|{
        data[0x08..0x0c].copy_from_slice(&(-1i32).to_le_bytes());
        // 不带原魔法的编号，免得游戏按魔法去生成弹体
        data[0x0c..0x10].copy_from_slice(&(-1i32).to_le_bytes());
        data[0x18..0x1c].copy_from_slice(&(-1i32).to_le_bytes());
        data[0x14..0x18].copy_from_slice(&HAIMA_SHOT.to_le_bytes());
        data[0x1c..0x20].copy_from_slice(&(-1i32).to_le_bytes());
        data[0x20..0x28].fill(0xff);
        // 朝向矩阵（与 star_stream 相同的排法）：right / up / forward，forward 朝正下方
        for (offset,row) in [(0x50,[1.0f32,0.0,0.0]),(0x60,[0.0,0.0,-1.0]),(0x70,[0.0,-1.0,0.0])] {
            for (i,v) in row.into_iter().chain([0.0]).enumerate() {data[offset+i*4..offset+i*4+4].copy_from_slice(&v.to_le_bytes());}
        }
        let above=[pos[0],pos[1]+2.5,pos[2]];
        for (i,v) in above.into_iter().enumerate() {data[0x80+i*4..0x84+i*4].copy_from_slice(&v.to_le_bytes());}
        super::star_stream::spawn_raw(&data)
    })
}

/// 爆炸：在敌人头顶朝下发射海摩大炮的炮弹（只要特效与冲击，攻击行临时清零、0.6 秒后恢复），再按最后一发伤害扣血
fn burst(world:&mut WorldChrMan,handle:&FieldInsHandle,pos:[f32;3],damage:i32)->i32 {
    let amount=damage.saturating_mul(BURST_FACTOR);
    // 上一次的还没恢复就先恢复，再记下原值、临时清零
    restore_haima(true);
    if let Ok(params)=unsafe {SoloParamRepository::instance_mut()} {
        let mut saved=Vec::new();
        for bullet in [HAIMA_SHOT,HAIMA_BURST] {
            let Some(atk)=params.get::<Bullet>(bullet as u32).map(|b|b.atk_id_bullet()).filter(|id|*id>0) else{continue};
            if saved.iter().any(|(id,_)|*id==atk as u32) {continue;}
            if let Some(row)=params.get_mut::<AtkParam_Pc>(atk as u32) {
                saved.push((atk as u32,[row.atk_phys(),row.atk_mag(),row.atk_fire(),row.atk_thun(),row.atk_dark()]));
                row.set_atk_phys(0);row.set_atk_mag(0);row.set_atk_fire(0);row.set_atk_thun(0);row.set_atk_dark(0);
            }
        }
        if let Ok(mut pending)=ATK_RESTORE.lock() {*pending=Some((Instant::now()+RESTORE_AFTER,saved));}
        if let Some(row)=params.get_mut::<SpEffectParam>(BURST_DAMAGE as u32) {
            row.set_icon_id(-1);row.set_effect_endurance(0.0);row.set_change_hp_point(amount);row.set_change_hp_rate(0.0);
            row.set_effect_target_self_target(true);
        }
    }
    let spawned=spawn_haima(world,pos);
    let _=handle;
    super::log(format!("STARSURGE_BURST damage={damage} amount={amount} spawn={spawned:?}"));
    amount
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rock_sling_and_other_vanilla_sorceries_are_recognised() {
        assert!(sorcery(4710).is_some(),"Rock Sling");
        assert!(sorcery(4000).is_some(),"Glintstone Pebble");
        assert!(sorcery(6000).is_none(),"incantations are not sorceries");
        assert!(sorcery(-1).is_none());
    }
    #[test]
    fn one_drink_restores_ten_fp_per_second_until_full() {
        let mut r=Regen::default();
        let mut fp=r.step(1,50,100,0.0);assert_eq!((fp,r.layers),(50,1));
        for _ in 0..10 {fp=r.step(0,fp,100,0.1);}
        assert_eq!(fp,60);
        for _ in 0..60 {fp=r.step(0,fp,100,0.1);}
        assert_eq!((fp,r.layers),(100,0),"stops and clears once full");
    }
    #[test]
    fn regen_is_smooth_and_continues_while_fp_is_spent() {
        let mut r=Regen::default();
        let mut fp=r.step(1,0,100,0.0);
        for _ in 0..5 {fp=r.step(0,fp,100,1.0/60.0);}
        assert_eq!(fp,0,"less than 0.1 s: nothing yet");
        for _ in 0..2 {fp=r.step(0,fp,100,1.0/60.0);}
        assert_eq!(fp,1,"one point per 0.1 s, accumulated per frame");
        fp=r.step(0,fp-1,100,0.1);assert_eq!((fp,r.layers),(1,1),"spending FP does not stop it");
    }
    #[test]
    fn extra_drinks_stack_the_rate_and_drinking_at_full_is_wasted() {
        let mut r=Regen::default();
        let fp=r.step(2,0,100,0.0);assert_eq!(r.step(0,fp,100,0.1),2);
        let mut full=Regen::default();assert_eq!((full.step(1,100,100,0.1),full.layers),(100,0));
        let mut long=Regen::default();long.step(1,0,100,0.0);assert_eq!(long.step(0,0,100,5.0),2,"long frames are capped");
    }
    use Outcome::*;
    fn run(c:&mut Cycle,spells:&str)->Vec<Outcome> {spells.bytes().map(|b|c.hit(u32::from(b))).collect()}
    fn stacks(from:u32,to:u32)->Vec<Outcome> {(from..=to).map(Stack).collect()}
    #[test]
    fn examples_from_the_design() {
        let mut c=Cycle::default();
        assert_eq!(run(&mut c,"ABABAC"),stacks(1,6));assert_eq!((c.layers,c.cap()),(6,9),"ABABAC -> 6/9");
        let mut c=Cycle::default();
        assert_eq!(run(&mut c,"ABABACABDC"),stacks(1,10));assert_eq!((c.layers,c.cap()),(10,12),"ABABACABDC -> 10/12");
    }
    #[test]
    fn two_spells_burst_on_the_sixth_hit() {
        let mut c=Cycle::default();
        let out=run(&mut c,"ABABAB");
        assert_eq!(out[..5],stacks(1,5)[..]);assert_eq!(out[5],Burst);assert_eq!(c,Cycle::default());
    }
    #[test]
    fn consecutive_repeat_restarts_at_layer_one() {
        let mut c=Cycle::default();
        assert_eq!(run(&mut c,"ABA"),stacks(1,3));
        assert_eq!(c.hit(u32::from(b'A')),Restart);assert_eq!((c.layers,c.cap()),(1,3));
        assert_eq!(run(&mut c,"BAB"),stacks(2,4),"the new cycle keeps growing");
    }
    #[test]
    fn single_spell_never_stacks_past_one() {
        let mut c=Cycle::default();
        assert_eq!(run(&mut c,"AAA"),[Stack(1),Restart,Restart]);assert_eq!(c.layers,1);
    }
    #[test]
    fn a_new_spell_before_the_cap_raises_it() {
        let mut c=Cycle::default();
        run(&mut c,"ABABA");assert_eq!((c.layers,c.cap()),(5,6));
        assert_eq!(c.hit(u32::from(b'C')),Stack(6));assert_eq!(c.cap(),9,"C at 6 raises the cap first: no burst");
        assert_eq!(run(&mut c,"ABC"),[Stack(7),Stack(8),Burst]);
    }
    #[test]
    fn victory_gift_needs_a_drink_and_no_full_fp_since() {
        assert!(victory_gift_due(true,false));
        assert!(!victory_gift_due(true,true),"FP was full again after drinking");
        assert!(!victory_gift_due(false,false),"never drank");
    }
    #[test]
    fn fast_bullets_match_foes_along_their_path() {
        // 最后一帧离敌人 15 米、正朝它飞（30 m/s）：弹道距离接近 0
        assert!(trajectory_distance([0.0,1.0,0.0],[30.0,0.0,0.0],[15.0,1.0,0.0])<0.01);
        // 敌人在侧面 10 米外：不算
        assert!(trajectory_distance([0.0,1.0,0.0],[30.0,0.0,0.0],[15.0,1.0,10.0])>6.0);
        // 敌人在弹体身后：按到最后位置的距离
        assert!((trajectory_distance([0.0,0.0,0.0],[30.0,0.0,0.0],[-3.0,0.0,0.0])-3.0).abs()<0.01);
        // 停住的弹体
        assert!((trajectory_distance([0.0,0.0,0.0],[0.0,0.0,0.0],[0.0,4.0,0.0])-4.0).abs()<0.01);
    }
    #[test]
    fn multiplier_compounds() {
        assert_eq!(multiplier(0),1.0);
        assert!((multiplier(1)-1.2).abs()<0.0001);
        assert!((multiplier(2)-1.44).abs()<0.0001);
        assert!((layer_factor(3)-1.1775).abs()<0.0001);
        // 模拟表：5 层 2.24，8 层 3.13，11 层 4.06，14 层 5.05
        for (layers,expected) in [(5,2.24),(8,3.13),(11,4.06),(14,5.05)] {assert!((multiplier(layers)-expected).abs()<0.01,"{layers}: {}",multiplier(layers));}
        assert!((2..40).all(|k|layer_factor(k+1)<layer_factor(k)||k<2)&&layer_factor(60)>1.05&&layer_factor(60)<1.051);
    }
}
