use std::time::{Duration,Instant};
use eldenring::{cs::{AttackElementCorrectParam,ChrIns,ChrInsExt,EquipParamGoods,EquipParamWeapon,FieldInsHandle,SoloParamRepository,SpEffectParam,WorldChrMan},param::SP_EFFECT_PARAM_ST};
use fromsoftware_shared::FromStatic;
use erdtree_tower::{Phase,Run,embers::{self,Ember,Gain,Strike,StrikeRequest}};

/// 余烬效果：scene-builder ember-patch 写入的 9670000 段
pub const FIRST:i32=9670000;
/// 无声处刑的旧常驻效果（处决状态 288）：只用于清理。挂着它时原版染红 / 染蓝凶刀的触发也会生效，处决会回血回蓝，所以不再挂上
const EXECUTION:i32=FIRST;
/// 无声处刑：致命一击后挂上 5 秒（精力消耗减半），由原生处决事件触发，不依赖处决状态
const EXECUTION_RELIEF:i32=FIRST+2;
const BLOOD:i32=FIRST+10;
const FROST:i32=FIRST+11;
const REVENGE:i32=FIRST+20;
/// 圣杯余响：喝圣杯瓶后的增益（造成的伤害提高、受到的伤害降低），持续时间由运行时控制
const ECHO:i32=FIRST+15;
/// 法师余烬：施法时减伤、施法时霸体（韧性伤害 ×0）、专注消耗降低（常驻）、专注充足时增伤
const BARRIER:i32=FIRST+16;
const MEDITATION:i32=FIRST+17;
const GLINTSTONE:i32=FIRST+18;
const FULL_MOON:i32=FIRST+19;
/// 灵唤师的共鸣：本机召唤物在场时挂在玩家身上（造成的伤害提高），召唤物倒下或消失就摘掉
const SPIRIT_RESONANCE:i32=FIRST+21;
/// 战技的精粹：持有即常驻，战技增伤（只对战技生效）与战技专注消耗降低各一行
const ARTS_ATTACK:i32=FIRST+22;
const ARTS_COST:i32=FIRST+23;
/// 坩埚骑士的铁壁：整局常驻（格挡不耗精力；最大负重压到极低，始终超重）
const BULWARK:i32=FIRST+24;
/// 红狮子的不退：生命低于 LAST_STAND_HP_PERCENT% 时挂上（受到的削韧 ×1/3），回到以上就摘掉
const LAST_STAND:i32=FIRST+25;
pub(crate) fn last_stand(hp:i32,max:i32)->bool {hp>0&&max>0&&i64::from(hp)*100<i64::from(max)*i64::from(embers::LAST_STAND_HP_PERCENT)}
/// 鲜血骑士的破势：一次性发作在敌人身上的特效编号段（出血、睡眠、冻伤、发狂；毒、腐败、致死这些持续类不算）
const BURSTS:[(i32,i32,&str);4]=[(6400,6449,"出血"),(6450,6499,"睡眠"),(6700,6749,"冻伤"),(6750,6799,"发狂")];
fn burst_kind(id:i32)->Option<&'static str> {BURSTS.iter().find(|(low,high,_)|(*low..=*high).contains(&id)).map(|(_,_,name)|*name)}
/// 发作时把敌人的韧性压到这么低：下一下韧性伤害就失衡（直接写 0 不一定会触发失衡动作，留一点让引擎按正常流程判定）
pub const BREAK_TOUGHNESS:f32=0.001;
/// 鲜血骑士的破势：每只敌人上一帧身上的发作特效、全部特效、生命，以及上次失衡的时间（Boss 冷却用）
#[derive(Default)]
struct Break {key:u64,bursts:Vec<i32>,effects:Vec<i32>,hp:i32,last:Option<Instant>}
const FLASH:i32=FIRST+40;
/// 灵唤铃的号令：挂在召唤物身上
const SPIRIT_COMMAND:i32=FIRST+50;
fn frenzy_effect(stack:u8)->i32 {FIRST+30+i32::from(stack)}
/// 旧版本常驻效果只用于清理；原生处决由攻击表的 350501 事件识别。
const CHAIN_RESIDENT:i32=FIRST+5;
#[cfg(test)]
const CHAIN_MARK:i32=FIRST+7;
use std::sync::atomic::{AtomicUsize,AtomicBool,Ordering};
static APPLY_ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static CHAIN_OWNER:AtomicUsize=AtomicUsize::new(0);
static CRITICAL_PENDING:AtomicBool=AtomicBool::new(false);
static CRITICAL_HOOK_READY:AtomicBool=AtomicBool::new(false);
fn own_critical(owner:usize,source:usize,effect:i32)->bool {owner!=0&&source==owner&&effect==350501}
unsafe extern "C" fn observe_native_critical(_target:usize,effect:i32,source:usize) {
    if own_critical(CHAIN_OWNER.load(Ordering::Relaxed),source,effect){CRITICAL_PENDING.store(true,Ordering::Release);}
}
#[unsafe(naked)]
unsafe extern "C" fn critical_entry() {
    core::arch::naked_asm!(
        "sub rsp, 0xc8", "mov [rsp+0x20], rax", "mov [rsp+0x28], rcx", "mov [rsp+0x30], rdx",
        "mov [rsp+0x38], r8", "mov [rsp+0x40], r9", "mov [rsp+0x48], r10", "mov [rsp+0x50], r11",
        "movdqu [rsp+0x60], xmm0", "movdqu [rsp+0x70], xmm1", "movdqu [rsp+0x80], xmm2",
        "movdqu [rsp+0x90], xmm3", "movdqu [rsp+0xa0], xmm4", "movdqu [rsp+0xb0], xmm5",
        "call {}", "movdqu xmm0, [rsp+0x60]", "movdqu xmm1, [rsp+0x70]", "movdqu xmm2, [rsp+0x80]",
        "movdqu xmm3, [rsp+0x90]", "movdqu xmm4, [rsp+0xa0]", "movdqu xmm5, [rsp+0xb0]",
        "mov rax, [rsp+0x20]", "mov rcx, [rsp+0x28]", "mov rdx, [rsp+0x30]", "mov r8, [rsp+0x38]",
        "mov r9, [rsp+0x40]", "mov r10, [rsp+0x48]", "mov r11, [rsp+0x50]", "add rsp, 0xc8",
        "jmp qword ptr [rip+{}]",sym observe_native_critical,sym APPLY_ORIGINAL);
}
pub unsafe fn install_critical_trace()->Result<(),String> {
    use hudhook::mh::{MhHook,MH_EnableHook};
    use std::ffi::c_void;
    let address=super::items::checked_function(0x3e8e70,&[0x48,0x8b,0xc4,0x57,0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57,0x48,0x83,0xec,0x50])? as *mut c_void;
    let hook=unsafe{MhHook::new(address,critical_entry as *mut c_void)}.map_err(|e|format!("Critical trace hook: {e:?}"))?;
    APPLY_ORIGINAL.store(hook.trampoline() as usize,Ordering::Release);
    unsafe{MH_EnableHook(address)}.ok().map_err(|e|format!("Critical trace enable: {e:?}"))?;
    CRITICAL_HOOK_READY.store(true,Ordering::Release);
    super::log("CHAIN_TEST native attack effect listener installed: 350501 / 3e8e70");Ok(())
}
#[derive(Clone,Copy,Default)]
pub struct ChainTrace {pub level:u32,pub resident:bool,pub marker:bool,pub rate:Option<f32>,pub critical_only:bool,pub effects:usize}
static CHAIN_TRACE:std::sync::Mutex<ChainTrace>=std::sync::Mutex::new(ChainTrace{level:0,resident:false,marker:false,rate:None,critical_only:false,effects:0});
static CRITICAL_EVENTS:std::sync::atomic::AtomicU32=std::sync::atomic::AtomicU32::new(0);
pub fn chain_trace()->(ChainTrace,u32) {(CHAIN_TRACE.lock().map(|t|*t).unwrap_or_default(),CRITICAL_EVENTS.load(std::sync::atomic::Ordering::Relaxed))}
pub fn begin_chain_test(){CRITICAL_EVENTS.store(0,std::sync::atomic::Ordering::Relaxed);CRITICAL_PENDING.store(false,Ordering::Release);}
fn observe_chain(chr:&ChrIns,level:u32) {
    let present:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).collect();
    let active:Vec<_>=slots(Ember::Chain).unwrap().into_iter().filter(|id|present.contains(id)).collect();
    let row=unsafe{SoloParamRepository::instance()}.ok().and_then(|p|active.first().and_then(|id|p.get::<SpEffectParam>(*id as u32)));
    if let Ok(mut trace)=CHAIN_TRACE.lock(){*trace=ChainTrace{level,resident:CRITICAL_HOOK_READY.load(Ordering::Acquire),marker:CRITICAL_PENDING.load(Ordering::Acquire),
        rate:row.map(|r|r.physics_attack_rate()),critical_only:row.is_some_and(|r|r.state_info()==367&&r.throw_attack_param_change()),effects:active.len()};}
}
fn player_ids()->impl Iterator<Item=i32> {[EXECUTION,REVENGE,ECHO,BARRIER,MEDITATION,GLINTSTONE,FULL_MOON,SPIRIT_RESONANCE,ARTS_ATTACK,ARTS_COST,BULWARK,LAST_STAND,CHAIN_RESIDENT].into_iter().chain((1..=embers::FRENZY_STACKS).map(frenzy_effect))}
/// 叠层余烬不设上限：每种两行轮换（灵魂收容挂在召唤物身上，其余挂在玩家身上）。
/// 层数变了就把新倍率写进没挂着的那一行、换上它，再摘掉旧的那一行；不改正在生效的那一行
fn slots(ember:Ember)->Option<[i32;2]> {
    let base=match ember {Ember::Battlemage=>26,Ember::Godslayer=>60,Ember::Harvest=>62,Ember::SoulEater=>70,Ember::Chain=>80,Ember::SpiritKeeper=>90,_=>return None};
    Some([FIRST+base,FIRST+base+1])
}
const PLAYER_STACKING:[Ember;4]=[Ember::Godslayer,Ember::Harvest,Ember::SoulEater,Ember::Chain];
/// 处刑者的血宴：致命一击之后这么久之内自己打死的敌人算处决击杀（每次致命一击只算一只）
const HARVEST_WINDOW:Duration=Duration::from_secs(4);
fn holds(row:&SP_EFFECT_PARAM_ST,ember:Ember,rates:embers::Rates)->bool {
    if ember==Ember::SpiritKeeper {row.physics_attack_power_rate()==rates.attack&&row.slash_damage_cut_rate()==rates.cut}
    else if ember==Ember::Battlemage {row.slash_damage_cut_rate()==rates.cut}
    else {row.physics_attack_rate()==rates.attack&&row.max_hp_rate()==rates.hp}
}
fn write(row:&mut SP_EFFECT_PARAM_ST,ember:Ember,rates:embers::Rates) {
    if ember==Ember::Battlemage {
        let c=rates.cut;
        row.set_slash_damage_cut_rate(c);row.set_blow_damage_cut_rate(c);row.set_thrust_damage_cut_rate(c);row.set_neutral_damage_cut_rate(c);
        row.set_magic_damage_cut_rate(c);row.set_fire_damage_cut_rate(c);row.set_thunder_damage_cut_rate(c);row.set_dark_damage_cut_rate(c);
    } else if ember==Ember::SpiritKeeper {
        let a=rates.attack;
        row.set_physics_attack_power_rate(a);row.set_magic_attack_power_rate(a);row.set_fire_attack_power_rate(a);row.set_thunder_attack_power_rate(a);row.set_dark_attack_power_rate(a);
        let c=rates.cut;
        row.set_slash_damage_cut_rate(c);row.set_blow_damage_cut_rate(c);row.set_thrust_damage_cut_rate(c);row.set_neutral_damage_cut_rate(c);
        row.set_magic_damage_cut_rate(c);row.set_fire_damage_cut_rate(c);row.set_thunder_damage_cut_rate(c);row.set_dark_damage_cut_rate(c);
    } else {
        let a=rates.attack;
        row.set_physics_attack_rate(a);row.set_magic_attack_rate(a);row.set_fire_attack_rate(a);row.set_thunder_attack_rate(a);row.set_dark_attack_rate(a);
        row.set_max_hp_rate(rates.hp);
    }
}
/// 这一层数该挂的那一行；present：目标角色身上已有的效果。None 表示不挂（没有层数、参数缺失，或两行都还挂着、等下一帧）
fn scaled_slot(ember:Ember,level:u32,present:&[i32])->Option<i32> {
    let (slots,rates)=(slots(ember)?,embers::stack_rates(ember,level)?);
    if level==0 {return None;}
    let params=unsafe {SoloParamRepository::instance_mut()}.ok()?;
    let fits=|id:i32,params:&SoloParamRepository|params.get::<SpEffectParam>(id as u32).is_some_and(|row|holds(row,ember,rates));
    if let Some(id)=slots.iter().copied().filter(|id|fits(*id,params)).max_by_key(|id|present.contains(id)) {return Some(id);}
    let free=slots.into_iter().find(|id|!present.contains(id))?;
    write(params.get_mut::<SpEffectParam>(free as u32)?,ember,rates);
    Some(free)
}
/// 按 scaled_slot 的结果在一个角色身上换行：先挂新的再摘旧的（屠神者摘掉最大生命倍率时不会先把当前生命压低）
fn swap<C:ChrInsExt+?Sized>(chr:&mut C,present:&[i32],ember:Ember,want:Option<i32>) {
    let Some(slots)=slots(ember) else{return};
    if let Some(id)=want&&!present.contains(&id) {chr.apply_speffect(id,false);}
    for id in slots {if present.contains(&id)&&Some(id)!=want {chr.remove_speffect(id);}}
}
/// 击倒时按血条等级记魂点：带血条的 Boss、精英（含 Boss 房的随从）、普通敌人
fn kill_rank(bar_rank:u8)->u8 {match bar_rank {0=>2,3=>0,_=>1}}
fn ready(ids:&[i32])->bool {unsafe {SoloParamRepository::instance()}.is_ok_and(|p|ids.iter().all(|id|p.get::<SpEffectParam>(*id as u32).is_some()))}
/// 低于敌人最大生命 0.3% 的掉血视为持续伤害，不算命中；同一敌人 0.3 秒内只算一次
const HIT_PERMILLE:i64=3;
const HIT_INTERVAL:Duration=Duration::from_millis(300);

/// 行商的无底囊：投掷道具的使用动作（EquipParamGoods.goodsUseAnim）。17 飞刀与飞镖、2 投掷弯刀、3 与 42 投掷壶和抛出的石块碎片、
/// 20 附绳壶、50 辉石碎片、67 双手投出的重壶（各动作对应的道具见 README）
const THROW_ANIMS:[u8;7]=[2,3,17,20,42,50,67];
/// 持有时在本机内存里把这些道具改成“使用不消耗”（不改 regulation），记下改过的行；不再持有时只恢复这些行
static POUCH_ROWS:std::sync::Mutex<Option<Vec<u32>>>=std::sync::Mutex::new(None);
fn sync_pouch(on:bool) {
    let Ok(mut rows)=POUCH_ROWS.lock() else{return};
    if on==rows.is_some() {return;}
    let Ok(params)=(unsafe {SoloParamRepository::instance_mut()}) else{return};
    if on {
        let mut changed=Vec::new();
        for (id,row) in params.rows_mut::<EquipParamGoods>() {
            if row.goods_type()==0&&row.is_consume()&&THROW_ANIMS.contains(&row.goods_use_anim()) {row.set_is_consume(false);changed.push(id);}
        }
        super::log(format!("TOWER_EMBER pouch on: {} throwables no longer consumed",changed.len()));
        *rows=Some(changed);
    } else {
        for id in rows.take().unwrap_or_default() {if let Some(row)=params.get_mut::<EquipParamGoods>(id) {row.set_is_consume(true);}}
        super::log("TOWER_EMBER pouch off: throwables consumed again");
    }
}
/// 黄金树的恩泽：原版治疗类祷告的回复特效（changeHpPoint 为负即回复）：急救、治疗、大治疗、王之治疗、黄金树治疗（含给队友的）、
/// 赐福的恩惠、黄金树的赐福、兽性活力，以及 DLC 的远距治疗、小黄金树。圣杯瓶、道具、武器的回复不在内
const MERCY_HEALS:[u32;18]=[1642000,1642001,1642100,1642101,1642200,1642201,1642300,1642301,1642400,1642401,
    1643000,1643001,1643100,1643101,1685000,21640000,21640001,21667000];
/// 持有时在本机内存里把这些行的回复量乘 1+MERCY_HEAL_PERCENT%（不改 regulation），记下原值；不再持有时改回原值
static MERCY_ROWS:std::sync::Mutex<Option<Vec<(u32,i32)>>>=std::sync::Mutex::new(None);
pub(crate) fn mercy_heal(point:i32)->i32 {(i64::from(point)*i64::from(100+embers::MERCY_HEAL_PERCENT)/100) as i32}
fn sync_mercy(on:bool) {
    let Ok(mut rows)=MERCY_ROWS.lock() else{return};
    if on==rows.is_some() {return;}
    let Ok(params)=(unsafe {SoloParamRepository::instance_mut()}) else{return};
    if on {
        let mut changed=Vec::new();
        for id in MERCY_HEALS {
            let Some(row)=params.get_mut::<SpEffectParam>(id) else{continue};
            let point=row.change_hp_point();
            if point<0 {row.set_change_hp_point(mercy_heal(point));changed.push((id,point));}
        }
        super::log(format!("TOWER_EMBER mercy on: {} healing incantation effects +{}%",changed.len(),embers::MERCY_HEAL_PERCENT));
        *rows=Some(changed);
    } else {
        for (id,point) in rows.take().unwrap_or_default() {if let Some(row)=params.get_mut::<SpEffectParam>(id) {row.set_change_hp_point(point);}}
        super::log("TOWER_EMBER mercy off: healing incantations restored");
    }
}
/// 战斗法师：属性补正表里预留的专用行（battlemage-patch 写进 regulation）。转换时把武器原来的补正规则复制过来，
/// 再让智力或信仰也计入物理伤害；武器参数行改用这一行，智力 / 信仰补正提到 S。都只改本机内存，读档后按存档重新写
const BATTLEMAGE_ELEMENTS:u32=9670000;
/// 补正值：原版字母 S 的门槛是 175，取 180（强化后的倍率通常 ≥1，仍是 S）
const BATTLEMAGE_CORRECT:f32=180.0;
struct Converted {row:u32,stat:u8,correct:f32,elements:i32}
static BATTLEMAGE:std::sync::Mutex<Vec<Converted>>=std::sync::Mutex::new(Vec::new());
/// 铁匠菜单据此显示「战斗法师」一项
pub static BATTLEMAGE_HELD:AtomicBool=AtomicBool::new(false);
fn sync_battlemage(wanted:&[(u32,u8)]) {
    let Ok(mut applied)=BATTLEMAGE.lock() else{return};
    if applied.len()==wanted.len()&&applied.iter().zip(wanted).all(|(a,(row,stat))|a.row==*row&&a.stat==*stat) {return;}
    let Ok(params)=(unsafe {SoloParamRepository::instance_mut()}) else{return};
    // 先全部改回原样，再按存档重新写
    for c in applied.drain(..) {
        if let Some(row)=params.get_mut::<EquipParamWeapon>(c.row) {
            if c.stat==0 {row.set_correct_magic(c.correct);}else{row.set_correct_faith(c.correct);}
            row.set_attack_element_correct_id(c.elements);
        }
    }
    for (slot,&(row_id,stat)) in wanted.iter().enumerate().take(embers::BATTLEMAGE_WEAPONS) {
        let custom=BATTLEMAGE_ELEMENTS+slot as u32;
        let Some(row)=params.get::<EquipParamWeapon>(row_id) else{continue};
        let (correct,elements)=(if stat==0{row.correct_magic()}else{row.correct_faith()},row.attack_element_correct_id());
        let Some(source)=params.get::<AttackElementCorrectParam>(elements.max(0) as u32).map(|r|std::ptr::from_ref(r)) else{continue};
        let Some(target)=params.get_mut::<AttackElementCorrectParam>(custom) else{super::log_repeating("TOWER_EMBER battlemage: element correction rows missing; run battlemage-patch");return;};
        unsafe {std::ptr::copy_nonoverlapping(source,std::ptr::from_mut(target),1);}
        if stat==0 {target.set_is_magic_correct_by_physics(true);}else{target.set_is_faith_correct_by_physics(true);}
        let row=params.get_mut::<EquipParamWeapon>(row_id).unwrap();
        if stat==0 {row.set_correct_magic(correct.max(BATTLEMAGE_CORRECT));}else{row.set_correct_faith(correct.max(BATTLEMAGE_CORRECT));}
        row.set_attack_element_correct_id(custom as i32);
        applied.push(Converted{row:row_id,stat,correct,elements});
    }
    super::log(format!("TOWER_EMBER battlemage weapons: {wanted:?}"));
}
/// 疾行者的轻羽：当前动画是走路、奔跑（动画编号去掉动作组后在这一段）时把动作播放速度提到 1+STRIDE_PERCENT%，攻击、翻滚、施法不变。
/// 编号段按魂系列惯例先定（a000_020000 起的移动动作），实测后按 TOWER_EMBER stride anims 日志校准
const LOCOMOTION:std::ops::RangeInclusive<i32>=20000..=24999;
pub(crate) fn locomotion(anim:i32)->bool {anim>=0&&LOCOMOTION.contains(&(anim%1_000_000))}
static STRIDE_SET:AtomicBool=AtomicBool::new(false);
static STRIDE_SEEN:std::sync::Mutex<Vec<i32>>=std::sync::Mutex::new(Vec::new());
fn sync_stride(chr:&mut ChrIns,on:bool) {
    let anim={let t=&chr.modules.time_act;t.anim_queue[(t.read_idx%10) as usize].anim_id};
    if on && let Ok(mut seen)=STRIDE_SEEN.lock() && seen.len()<40 && !seen.contains(&anim) {
        seen.push(anim);super::log(format!("TOWER_EMBER stride anims: {seen:?}"));
    }
    let speed=1.0+embers::STRIDE_PERCENT as f32/100.0;
    let alive=chr.modules.data.hp>0;
    let behavior=&mut chr.modules.behavior;
    if on&&alive&&locomotion(anim) {
        if behavior.animation_speed<speed {behavior.animation_speed=speed;STRIDE_SET.store(true,Ordering::Relaxed);}
    } else if STRIDE_SET.swap(false,Ordering::Relaxed)&&(behavior.animation_speed-speed).abs()<0.001 {behavior.animation_speed=1.0;}
}
pub fn held(run:Option<&Run>)->Vec<Ember> {
    run.filter(|r|!matches!(r.phase,Phase::Dead|Phase::Won)).map(|r|r.embers.clone()).unwrap_or_default()
}
/// 玩家身上的余烬效果：以血还血、癫火狂热等只在战斗中按状态挂上；无声处刑在致命一击时由 Fight::tick 挂上
pub fn sync(chr:&mut ChrIns,run:Option<&Run>,fight:&Fight) {
    let mut want=Vec::new();
    let embers=held(run);
    sync_pouch(embers.contains(&Ember::Pouch));
    sync_mercy(embers.contains(&Ember::Mercy));
    sync_stride(chr,embers.contains(&Ember::Stride));
    let battlemage=embers.contains(&Ember::Battlemage);BATTLEMAGE_HELD.store(battlemage,Ordering::Relaxed);
    sync_battlemage(if battlemage {run.map_or(&[][..],|r|r.battlemage.as_slice())}else{&[]});
    let owner=if embers.contains(&Ember::Chain)||embers.contains(&Ember::Execution)||embers.contains(&Ember::Harvest){std::ptr::from_ref(chr) as usize}else{0};
    if CHAIN_OWNER.swap(owner,Ordering::AcqRel)!=owner{CRITICAL_PENDING.store(false,Ordering::Release);}
    if fight.revenge_active() {want.push(REVENGE);}
    if fight.echo_active() {want.push(ECHO);}
    if embers.contains(&Ember::Glintstone) {want.push(GLINTSTONE);}
    if embers.contains(&Ember::Bulwark) {want.push(BULWARK);}
    if embers.contains(&Ember::LastStand)&&last_stand(chr.modules.data.hp,chr.modules.data.max_hp) {want.push(LAST_STAND);}
    if embers.contains(&Ember::Arts) {want.extend([ARTS_ATTACK,ARTS_COST]);}
    if fight.casting() {
        if embers.contains(&Ember::Barrier) {want.push(BARRIER);}
        if embers.contains(&Ember::Meditation) {want.push(MEDITATION);}
    }
    if fight.full_moon {want.push(FULL_MOON);}
    if fight.resonance {want.push(SPIRIT_RESONANCE);}
    if fight.frenzy>0 {want.push(frenzy_effect(fight.frenzy));}
    let present:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).collect();
    let stacks=run.map(|r|r.ember_stacks).unwrap_or_default();
    for ember in PLAYER_STACKING {
        let level=if embers.contains(&ember) {stacks.level(ember)}else{0};
        swap(chr,&present,ember,scaled_slot(ember,level,&present));
    }
    // 战斗法师的减伤：按智力、信仰较高者，能力值变了就换一行
    let mind=if battlemage {unsafe {eldenring::cs::PlayerIns::local_player()}.ok().map_or(0,|p|{let d=unsafe {p.player_game_data.as_ref()};d.intelligence.max(d.faith)})}else{0};
    swap(chr,&present,Ember::Battlemage,scaled_slot(Ember::Battlemage,mind,&present));
    for id in player_ids() {if present.contains(&id)&&!want.contains(&id){chr.remove_speffect(id);}}
    if want.is_empty()||!ready(&want) {observe_chain(chr,stacks.level(Ember::Chain));return;}
    for id in want {if !present.contains(&id){chr.apply_speffect(id,false);}}
    observe_chain(chr,stacks.level(Ember::Chain));
}
fn distance(a:[f32;3],b:[f32;3])->f32 {((a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)+(a[2]-b[2]).powi(2)).sqrt()}
/// slot：这场战斗敌人名单里的位置（联机时房主与成员一致，成员发来的敌人侧效果按它找敌人）
struct Foe {handle:FieldInsHandle,hp:i32,max:i32,pos:[f32;3],alive:bool,boss:bool,rank:u8,slot:u8,hit_at:Option<Instant>}
/// 一帧的结果：噬神之蛇的饱食是否要补红瓶、让叠层余烬加层的事件、联机成员要请房主执行的敌人侧效果
#[derive(Default)]
pub struct Events {pub feast:bool,pub gains:Vec<Gain>,pub strikes:Vec<Strike>}
/// 本机玩家与本机召唤物：联机时只算他们的命中与击倒（看敌人的 last_hit_by）
fn own_handles(world:&WorldChrMan)->Vec<FieldInsHandle> {
    world.main_player.as_ref().map(|p|p.chr_ins.field_ins_handle).into_iter().chain(world.summon_buddy_chr_set.characters().map(|c|c.field_ins_handle)).collect()
}
/// 一场战斗内的余烬状态：按敌人生命变化判断命中与倒下，按玩家生命变化判断受伤
#[derive(Default)]
pub struct Fight {
    key:Option<(u64,u8,usize)>,foes:Vec<Foe>,player_hp:Option<i32>,flasks:Option<u8>,
    lifesteal:i64,rose:bool,dragon_ready:Option<Instant>,revenge_until:Option<Instant>,echo_until:Option<Instant>,pub frenzy:u8,
    /// 施法时：按住施法键期间与松开后 CAST_LINGER_MS 内；cast_seen 只用于每场战斗记一次日志
    cast_until:Option<Instant>,cast_seen:bool,
    /// 蕾娜拉的满月：专注不低于 MOON_FP_PERCENT%
    full_moon:bool,
    /// 灵唤师的共鸣：本机召唤物在场（这一帧有活着的召唤物）
    resonance:bool,
    /// 黑刀刺客的身法：退还翻滚与致命一击扣掉的精力
    footwork:Footwork,feasts:u8,missing:bool,
    /// 上一帧在场的召唤物位置（灵骸迸裂在它消失时引爆）
    spirit:Option<[f32;3]>,
    /// 上一帧玩家身上有处刑之链的致命一击标记
    critical_edge:CriticalEdge,
    /// 处刑者的血宴：最近一次致命一击的时间（之后 HARVEST_WINDOW 内自己打死的敌人算处决击杀，算过就清掉）
    critical_at:Option<Instant>,
    /// 房主：每个成员已执行到的敌人侧效果序号
    remote:Vec<(u64,u32)>,
    /// 联机时这场战斗是否已记过第一次“自己的命中”与“别人的命中”（日志，用来核对 last_hit_by 的归属）
    attributed:[bool;2],
    /// 鲜血骑士的破势：每只敌人的发作状态；probes：本场已记的校准日志条数
    breaks:Vec<Break>,probes:u8,
}
impl Fight {
    pub fn active(&self)->bool {self.key.is_some()}
    /// 鲜血骑士的破势（只由房主或单人执行，敌人由他们计算）：敌人身上新出现一次性发作时压低韧性，下一下就失衡；
    /// 同一个 Boss（有血条的）BREAKER_BOSS_COOLDOWN_SECONDS 秒内最多一次。在 tick 之后调用（tick 换场时会清掉状态）。
    /// 校准：敌人一帧掉了 10% 以上的生命时，把这一帧新出现的特效记进日志（每场最多 12 条），用来核对发作特效的编号段
    pub fn break_on_bursts(&mut self,world:&mut WorldChrMan,foes:&[(FieldInsHandle,u8,u8)]) {
        let now=Instant::now();
        for &(handle,bar,slot) in foes {
            let Some(chr)=world.chr_ins_by_handle_mut(&handle) else{continue};
            let (hp,max)=(chr.modules.data.hp,chr.modules.data.max_hp);
            if hp<=0||chr.chr_flags1c5.death_flag(){continue;}
            let key=(u64::from(handle.block_id.0 as u32)<<32)|u64::from(handle.selector.0);
            let effects:Vec<i32>=chr.special_effect.entries().map(|e|e.param_id).collect();
            let bursts:Vec<i32>=effects.iter().copied().filter(|id|burst_kind(*id).is_some()).collect();
            let index=match self.breaks.iter().position(|b|b.key==key) {Some(i)=>i,None=>{self.breaks.push(Break{key,bursts:bursts.clone(),effects:effects.clone(),hp,last:None});continue;}};
            let state=&mut self.breaks[index];
            if self.probes<12&&max>0&&state.hp-hp>max/10 {
                let fresh:Vec<i32>=effects.iter().copied().filter(|id|!state.effects.contains(id)).collect();
                self.probes+=1;super::log(format!("TOWER_EMBER breaker probe slot={slot} hp {}->{hp}/{max} new_effects={fresh:?}",state.hp));
            }
            let fresh=bursts.iter().copied().find(|id|!state.bursts.contains(id));
            state.bursts=bursts;state.effects=effects;state.hp=hp;
            let Some(id)=fresh else{continue};
            let boss=bar>0;
            if boss&&state.last.is_some_and(|at|now.duration_since(at)<Duration::from_secs(embers::BREAKER_BOSS_COOLDOWN_SECONDS)) {
                super::log(format!("TOWER_EMBER breaker slot={slot} {} effect={id}: boss cooldown",burst_kind(id).unwrap_or("?")));continue;
            }
            state.last=Some(now);
            let toughness=&mut chr.modules.toughness;let before=toughness.toughness;
            toughness.toughness=toughness.toughness.min(BREAK_TOUGHNESS);
            super::log(format!("TOWER_EMBER breaker slot={slot} {} effect={id} boss={boss} toughness {before}->{} max={}",burst_kind(id).unwrap_or("?"),toughness.toughness,toughness.toughness_max));
        }
    }
    pub fn revenge_active(&self)->bool {self.revenge_until.is_some_and(|until|Instant::now()<until)}
    pub fn echo_active(&self)->bool {self.echo_until.is_some_and(|until|Instant::now()<until)}
    pub fn casting(&self)->bool {self.cast_until.is_some_and(|until|Instant::now()<until)}
    /// foes：(句柄, 血条等级 enemies::bar_rank, 名单位置)；authority：房主或单人（敌人侧效果直接执行，否则交给房主）；
    /// party：联机（命中与击倒只算本机玩家与召唤物的）；keeper：灵魂收容当前层数
    pub fn tick(&mut self,world:&mut WorldChrMan,key:(u64,u8,usize),held:&[Ember],foes:&[(FieldInsHandle,u8,u8)],authority:bool,party:bool,flasks:Option<u8>,keeper:u32)->Events {
        if self.key!=Some(key) {*self=Self{key:Some(key),..Default::default()};}
        let drank=matches!((self.flasks,flasks),(Some(before),Some(after)) if after<before);self.flasks=flasks;
        let mut events=Events::default();
        if held.is_empty() {return events;}
        let has=|e:Ember|held.contains(&e);
        let now=Instant::now();
        let Some(player)=world.main_player.as_ref() else{return events};
        if player.chr_ins.modules.data.hp<=0||player.chr_ins.chr_flags1c5.death_flag() {return events;}
        // 致命一击（原生处决事件，见 install_critical_trace）：处刑之链加层，无声处刑挂上精力减半
        if has(Ember::Chain)||has(Ember::Execution)||has(Ember::Harvest) {
            let marked=CRITICAL_PENDING.swap(false,Ordering::AcqRel);
            let throwing=unsafe{std::ptr::addr_of!(player.chr_ins.modules.throw.throw_node.throw_state).cast::<u32>().read()}==3;
            if self.critical_edge.observe(marked,throwing) {
                if has(Ember::Chain) {events.gains.push(Gain::Critical);}
                if has(Ember::Harvest) {self.critical_at=Some(now);}
                let count=CRITICAL_EVENTS.fetch_add(1,std::sync::atomic::Ordering::Relaxed)+1;
                super::log(format!("CHAIN_TEST real critical marker observed count={count}"));
                if has(Ember::Execution)&&ready(&[EXECUTION_RELIEF]) {
                    world.main_player.as_mut().unwrap().chr_ins.apply_speffect(EXECUTION_RELIEF,false);
                    super::log("TOWER_EMBER execution relief applied");
                }
            }
        }
        let Some(player)=world.main_player.as_ref() else{return events};
        let data=&player.chr_ins.modules.data;
        let (hp,max_hp,stamina,max_stamina)=(data.hp,data.max_hp,data.stamina,data.max_stamina);
        // 法师余烬：施法键（主手、副手，含武器上的施法）按住即算施法，松开后再保持一会儿，出招动作放完前都算
        if has(Ember::Barrier)||has(Ember::Meditation) {
            let input=&player.chr_ins.modules.action_request.action_requests;
            if input.magic_r()||input.magic_l()||input.magic_r2()||input.magic_l2() {
                self.cast_until=Some(now+Duration::from_millis(embers::CAST_LINGER_MS));
                if !self.cast_seen {self.cast_seen=true;super::log("TOWER_EMBER cast detected");}
            }
        }
        // 黑刀刺客的身法：刚按下闪避键、短按松开（长按是冲刺）、刚进入致命一击时，退还这次动作扣掉的精力
        let mut refund=0;
        if has(Ember::Footwork) {
            let request=&player.chr_ins.modules.action_request;
            let (pressed,released)=(&request.new_action_presses,&request.released_actions);
            if pressed.sp_move() {self.footwork.held_since=Some(now);}
            let tap=released.sp_move()&&self.footwork.held_since.take().is_some_and(|t|now.duration_since(t)<FOOTWORK_TAP);
            let dodge=pressed.sp_move()||tap||pressed.rolling()||pressed.backstep()||pressed.emergencystep();
            let throwing=unsafe{std::ptr::addr_of!(player.chr_ins.modules.throw.throw_node.throw_state).cast::<u32>().read()}==3;
            let critical=throwing&&!self.footwork.throwing;self.footwork.throwing=throwing;
            refund=self.footwork.observe(stamina,dodge||critical,now);
            if refund>0&&!self.footwork.logged {self.footwork.logged=true;super::log(format!("TOWER_EMBER footwork refund {refund} critical={critical}"));}
        }
        self.full_moon=has(Ember::FullMoon)&&data.max_fp>0&&i64::from(data.fp)*100>=i64::from(data.max_fp)*i64::from(embers::MOON_FP_PERCENT);
        let p=player.chr_ins.modules.physics.position;let here=[p.0,p.1,p.2];
        let taken=self.player_hp.map_or(0,|before|(before-hp).max(0));self.player_hp=Some(hp);
        // 敌人：命中与倒下。单人时都算自己的；联机时按敌人的 last_hit_by 只算本机玩家与召唤物的（队友的不算进自己的余烬）
        let own=own_handles(world);
        let (mut hits,mut deaths)=(Vec::new(),Vec::new());
        for &(ref handle,rank,slot) in foes {
            let Some(chr)=world.chr_ins_by_handle(handle) else{continue};
            let (foe_hp,foe_max)=(chr.modules.data.hp,chr.modules.data.max_hp);
            let q=chr.modules.physics.position;let pos=[q.0,q.1,q.2];
            let alive=foe_hp>0&&!chr.chr_flags1c5.death_flag();
            let attacker=chr.last_hit_by;let mine=!party||own.contains(&attacker);
            let Some(index)=self.foes.iter().position(|f|f.handle==*handle) else {
                self.foes.push(Foe{handle:*handle,hp:foe_hp,max:foe_max,pos,alive,boss:rank<=1,rank,slot,hit_at:None});continue;
            };
            let foe=&mut self.foes[index];
            let drop=foe.hp-foe_hp;
            let died=foe.alive&&!alive;
            let hit=!died&&alive&&drop>0&&i64::from(drop)*1000>=i64::from(foe_max.max(1))*HIT_PERMILLE&&foe.hit_at.is_none_or(|t|now.duration_since(t)>=HIT_INTERVAL);
            if hit {foe.hit_at=Some(now);}
            (foe.hp,foe.max,foe.pos,foe.alive)=(foe_hp,foe_max,pos,alive);
            // 联机：每场战斗记一次“算进自己的”与“不算的”命中或击倒，实测时据此确认 last_hit_by 的归属
            if party&&(hit||died)&&!self.attributed[usize::from(!mine)] {
                self.attributed[usize::from(!mine)]=true;
                super::log(format!("TOWER_EMBER party {} by {attacker:?} counted={mine}",if died{"kill"}else{"hit"}));
            }
            if !mine {continue;}
            if died {deaths.push(index);} else if hit {hits.push((index,drop));}
        }
        // 召唤物：只看本机玩家的骨灰
        let spirit=world.summon_buddy_chr_set.characters().find(|c|c.modules.data.hp>0&&!c.chr_flags1c5.death_flag())
            .map(|c|{let q=c.modules.physics.position;(c.modules.data.hp,c.modules.data.max_hp,[q.0,q.1,q.2])});
        let vanished=self.spirit.filter(|_|spirit.is_none());self.spirit=spirit.map(|(_,_,pos)|pos);
        let resonance=has(Ember::SpiritResonance)&&spirit.is_some();
        if resonance!=self.resonance {super::log(format!("TOWER_EMBER spirit resonance {}",if resonance{"on"}else{"off"}));}
        self.resonance=resonance;
        for &dead in &deaths {events.gains.push(Gain::Kill{rank:kill_rank(self.foes[dead].rank),spirit:spirit.is_some()||vanished.is_some()});}
        // 处刑者的血宴：致命一击之后不久自己打死的那一只（只算第一只，范围伤害顺带打死的不算）
        if let Some(&dead)=deaths.first()&&self.critical_at.is_some_and(|at|now.duration_since(at)<HARVEST_WINDOW) {
            self.critical_at=None;let rank=kill_rank(self.foes[dead].rank);
            events.gains.push(Gain::CriticalKill{rank});super::log(format!("TOWER_EMBER harvest critical kill rank={rank}"));
        }
        // 灵魂收容：每只在场的召唤物挂上当前层数的效果
        if has(Ember::SpiritKeeper) {
            let living=|c:&&mut _|{let c:&ChrIns=c;c.modules.data.hp>0&&!c.chr_flags1c5.death_flag()};
            let present:Vec<i32>=world.summon_buddy_chr_set.characters().filter(living).flat_map(|c|c.special_effect.entries().map(|e|e.param_id).collect::<Vec<_>>()).collect();
            let want=scaled_slot(Ember::SpiritKeeper,keeper,&present);
            for chr in world.summon_buddy_chr_set.characters().filter(living) {
                let own:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).collect();
                swap(chr,&own,Ember::SpiritKeeper,want);
            }
        }
        // 玩家侧：所有玩家都生效
        let mut heal=0i64;
        // 仿身泪滴的替身：受到的伤害一部分转给召唤物（召唤物至少留 1 点生命，不由余烬直接打倒）
        let mut spirit_loss=0i32;
        if has(Ember::SpiritLink)&&taken>0 && let Some((spirit_hp,_,_))=spirit {
            spirit_loss=((i64::from(taken)*i64::from(embers::SPIRIT_SHARE_PERCENT)/100) as i32).min(spirit_hp-1).max(0);
            heal+=i64::from(spirit_loss);
        }
        if has(Ember::Miquella) {
            self.lifesteal+=hits.iter().map(|(_,d)|i64::from(*d)).sum::<i64>()*i64::from(embers::LIFESTEAL_PERCENT);
            heal+=self.lifesteal/100;self.lifesteal%=100;
        }
        if has(Ember::Frenzy) {
            if taken>0 {self.frenzy=0;} else {self.frenzy=(self.frenzy as usize+hits.len()).min(usize::from(embers::FRENZY_STACKS)) as u8;}
        }
        if has(Ember::Revenge) {
            if !hits.is_empty() {self.revenge_until=None;}
            if taken>0 {self.revenge_until=Some(now+Duration::from_secs(embers::REVENGE_SECONDS));}
        }
        let rise=has(Ember::Rise)&&!self.rose&&i64::from(hp)*100<i64::from(max_hp)*i64::from(embers::RISE_BELOW_PERCENT);
        let dragon=has(Ember::DragonHeart)&&stamina<=0&&max_stamina>0&&self.dragon_ready.is_none_or(|t|now>=t);
        // 殉道者的苦修：掉了多少生命（按最大生命的比例），就按同样的比例回复专注
        let martyr=if has(Ember::Martyr) {taken}else{0};
        if heal>0||rise||dragon||refund>0||martyr>0 {
            let data=&mut world.main_player.as_mut().unwrap().chr_ins.modules.data;
            if martyr>0 {
                let gain=embers::martyr_fp(martyr,data.max_hp,data.max_fp);
                if gain>0 {data.fp=(i64::from(data.fp.max(0))+i64::from(gain)).min(i64::from(data.max_fp)) as i32;}
            }
            if refund>0 {
                data.stamina=(i64::from(data.stamina.max(0))+i64::from(refund)).min(i64::from(data.max_stamina)) as i32;
                self.footwork.last=Some(data.stamina);
            }
            if rise {
                data.hp=data.hp.max((i64::from(data.max_hp)*i64::from(embers::RISE_TO_PERCENT)/100) as i32);self.rose=true;
                super::log(format!("TOWER_EMBER rise hp={}/{}",data.hp,data.max_hp));
            }
            if heal>0 {data.hp=(i64::from(data.hp)+heal).min(i64::from(data.max_hp)) as i32;}
            if dragon {
                data.stamina=(i64::from(data.stamina.max(0))+i64::from(data.max_stamina)*i64::from(embers::DRAGON_RESTORE_PERCENT)/100).min(i64::from(data.max_stamina)) as i32;
                self.dragon_ready=Some(now+Duration::from_secs(embers::DRAGON_COOLDOWN_SECONDS));
            }
            self.player_hp=Some(data.hp);
        }
        // 众灵的盛宴、灵唤铃的号令、替身的分担：都作用在本机召唤物上
        if spirit.is_some()&&(spirit_loss>0||has(Ember::SpiritCommand)||has(Ember::SpiritFeast)&&!deaths.is_empty()) {
            let feast_heal=if has(Ember::SpiritFeast) {deaths.len() as i64}else{0};
            if let Some(chr)=world.summon_buddy_chr_set.characters().find(|c|c.modules.data.hp>0&&!c.chr_flags1c5.death_flag()) {
                let data=&mut chr.modules.data;
                let restored=i64::from(data.max_hp)*i64::from(embers::SPIRIT_FEAST_HEAL_PERCENT)/100*feast_heal;
                data.hp=(i64::from(data.hp)-i64::from(spirit_loss)+restored).clamp(1,i64::from(data.max_hp)) as i32;
                if has(Ember::SpiritCommand)&&ready(&[SPIRIT_COMMAND])&&!chr.special_effect.entries().any(|e|e.param_id==SPIRIT_COMMAND) {chr.apply_speffect(SPIRIT_COMMAND,false);}
            }
            if feast_heal>0 {
                let data=&mut world.main_player.as_mut().unwrap().chr_ins.modules.data;
                data.fp=(i64::from(data.fp.max(0))+i64::from(data.max_fp)*i64::from(embers::SPIRIT_FEAST_FP_PERCENT)/100*feast_heal).min(i64::from(data.max_fp)) as i32;
            }
        }
        events.feast=has(Ember::Feast)&&!deaths.is_empty()&&self.feasts<embers::FEAST_PER_FIGHT;
        if events.feast {self.feasts+=1;}
        // 敌人侧：房主与单人直接执行；联机成员算出来交给房主（命中、击倒都只算自己的）
        let mut strikes:Vec<(usize,Strike)>=Vec::new();
        for &(index,_) in &hits {
            for ember in [Ember::Bloodlord,Ember::Frost] {if has(ember) {strikes.push((index,Strike::Buildup{slot:self.foes[index].slot,ember}));}}
        }
        let mut wound=|origin:[f32;3],radius:f32,ember:Ember,foes:&[Foe]| {
            for (index,foe) in foes.iter().enumerate() {if foe.alive&&distance(foe.pos,origin)<=radius {strikes.push((index,Strike::Wound{slot:foe.slot,ember}));}}
        };
        if has(Ember::Deathflame) {for &dead in &deaths {wound(self.foes[dead].pos,embers::DEATHFLAME_RADIUS,Ember::Deathflame,&self.foes);}}
        // 圣杯余响：喝圣杯瓶后一段时间内的增益（挂在玩家身上，由 sync 按 echo_until 挂上 / 摘掉；再喝一口重新计时）
        if has(Ember::Echo)&&drank {self.echo_until=Some(Instant::now()+Duration::from_secs(embers::ECHO_SECONDS));}
        if has(Ember::SpiritBurst) && let Some(origin)=vanished {wound(origin,embers::SPIRIT_BURST_RADIUS,Ember::SpiritBurst,&self.foes);super::log("TOWER_EMBER spirit burst");}
        if has(Ember::Thorns)&&taken>0 {
            let nearest=self.foes.iter().enumerate().filter(|(_,f)|f.alive).min_by(|(_,a),(_,b)|distance(a.pos,here).total_cmp(&distance(b.pos,here)));
            if let Some((index,foe))=nearest {strikes.push((index,Strike::Thorns{slot:foe.slot,taken:taken.min(embers::THORNS_TAKEN_LIMIT)}));}
        }
        if authority {for (index,strike) in strikes {self.strike(world,index,strike);}}
        else {events.strikes=strikes.into_iter().map(|(_,strike)|strike).collect();}
        events
    }
    /// 执行一条敌人侧效果。不由余烬直接击杀（至少留 1 点生命），避免绕过游戏的死亡流程
    fn strike(&mut self,world:&mut WorldChrMan,index:usize,strike:Strike) {
        if !ready(&[BLOOD,FROST,FLASH]) {if !self.missing{super::log("TOWER_EMBER missing ember effects; update regulation.bin");self.missing=true;}return;}
        let foe=&mut self.foes[index];
        let Some(chr)=world.chr_ins_by_handle_mut(&foe.handle) else{return};
        if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag() {return;}
        let max=chr.modules.data.max_hp;
        let damage=match strike {
            Strike::Buildup{ember,..}=>{chr.apply_speffect(if ember==Ember::Bloodlord{BLOOD}else{FROST},false);return;},
            Strike::Wound{ember,..}=>embers::enemy_damage(max,embers::wound_percent(ember).unwrap_or(0),foe.boss),
            Strike::Thorns{taken,..}=>embers::thorns_damage(taken,max,foe.boss),
        };
        if damage<=0 {return;}
        chr.modules.data.hp=(chr.modules.data.hp-damage).max(1);foe.hp=chr.modules.data.hp;
        chr.apply_speffect(FLASH,false);
    }
    /// 房主：执行成员发来的敌人侧效果。每个成员按序号只执行一次；成员没带对应的余烬、或找不到那个敌人的跳过
    pub fn apply_remote(&mut self,world:&mut WorldChrMan,foes:&[(FieldInsHandle,u8,u8)],member:u64,held:&[Ember],requests:&[StrikeRequest]) {
        let entry=match self.remote.iter().position(|(m,_)|*m==member) {Some(i)=>i,None=>{self.remote.push((member,0));self.remote.len()-1}};
        let last=self.remote[entry].1;
        let mut fresh:Vec<_>=requests.iter().filter(|r|r.seq>last).copied().collect();
        fresh.sort_by_key(|r|r.seq);
        let Some(newest)=fresh.last().map(|r|r.seq) else{return};
        self.remote[entry].1=newest;
        for request in fresh {
            if !request.strike.valid(held) {super::log(format!("TOWER_EMBER member {member} strike rejected: {:?}",request.strike));continue;}
            let Some(&(handle,rank,slot))=foes.iter().find(|f|f.2==request.strike.slot()) else{continue};
            let index=match self.foes.iter().position(|f|f.handle==handle) {
                Some(index)=>index,
                None=>{
                    let Some(chr)=world.chr_ins_by_handle(&handle) else{continue};
                    let q=chr.modules.physics.position;
                    let alive=chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag();
                    self.foes.push(Foe{handle,hp:chr.modules.data.hp,max:chr.modules.data.max_hp,pos:[q.0,q.1,q.2],alive,boss:rank<=1,rank,slot,hit_at:None});
                    self.foes.len()-1
                },
            };
            self.strike(world,index,request.strike);
        }
    }
}
/// 黑刀刺客的身法：动作开始后 FOOTWORK_ARM 内的第一次精力扣减退还；扣精力可能比识别早一两帧，识别前 FOOTWORK_RECENT 内的扣减也算
const FOOTWORK_ARM:Duration=Duration::from_millis(600);
const FOOTWORK_RECENT:Duration=Duration::from_millis(150);
/// 闪避键按住不到这么久就松开算翻滚，更久是冲刺
const FOOTWORK_TAP:Duration=Duration::from_millis(500);
#[derive(Default)]
struct Footwork {last:Option<i32>,recent:Option<(i32,Instant)>,armed:Option<Instant>,held_since:Option<Instant>,throwing:bool,logged:bool}
impl Footwork {
    /// stamina：本帧精力；trigger：本帧开始翻滚或致命一击。返回要退还的精力（每次动作只退一次）
    fn observe(&mut self,stamina:i32,trigger:bool,now:Instant)->i32 {
        let drop=self.last.map_or(0,|before|(before-stamina).max(0));self.last=Some(stamina);
        if drop>0 {self.recent=Some((drop,now));}
        if trigger {
            if let Some((amount,at))=self.recent.take()&&now.duration_since(at)<=FOOTWORK_RECENT {self.armed=None;return amount;}
            self.armed=Some(now+FOOTWORK_ARM);
        }
        if drop>0&&self.armed.is_some_and(|until|now<until) {self.armed=None;self.recent=None;return drop;}
        0
    }
}
#[test]
fn footwork_refunds_one_drop_per_dodge_or_critical() {
    let t=Instant::now();let ms=|n:u64|t+Duration::from_millis(n);
    let mut f=Footwork::default();
    assert_eq!(f.observe(100,false,ms(0)),0);
    assert_eq!(f.observe(100,true,ms(16)),0,"dodge pressed, nothing spent yet");
    assert_eq!(f.observe(80,false,ms(33)),20,"the roll's cost comes back");
    f.last=Some(100);
    assert_eq!(f.observe(70,false,ms(50)),0,"the next attack is paid");
    f.last=Some(70);
    assert_eq!(f.observe(50,false,ms(400)),0,"no dodge, no refund");
    assert_eq!(f.observe(50,true,ms(480)),20,"spent a frame before the critical was seen");
    f.last=Some(70);
    assert_eq!(f.observe(70,true,ms(1000)),0);
    assert_eq!(f.observe(60,false,ms(1700)),0,"armed window expired");
}
#[derive(Default)]
struct CriticalEdge {counted:bool,clear_frames:u8}
impl CriticalEdge {
    fn observe(&mut self,marked:bool,throwing:bool)->bool {
        if marked||throwing {self.clear_frames=0;}else{self.clear_frames=self.clear_frames.saturating_add(1);if self.clear_frames>=2{self.counted=false;}}
        if marked&&!self.counted {self.counted=true;true}else{false}
    }
}
#[test]
fn critical_marker_counts_once_per_execution_not_per_damage_stage() {
    let mut edge=CriticalEdge::default();
    assert!(!edge.observe(false,false));assert!(!edge.observe(false,true));
    assert!(edge.observe(true,true));assert!(!edge.observe(true,true));
    assert!(!edge.observe(false,true));assert!(!edge.observe(true,true));
    assert!(!edge.observe(true,false));assert!(!edge.observe(false,false));assert!(!edge.observe(false,false));
    assert!(edge.observe(true,true));
}
#[test]
fn critical_listener_rejects_other_players_and_noncritical_effects() {
    assert!(own_critical(123,123,350501));
    assert!(!own_critical(123,456,350501));assert!(!own_critical(0,0,350501));
    for effect in [350500,350502,9670007,0,-1]{assert!(!own_critical(123,123,effect));}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mercy_raises_heals_by_half_and_keeps_the_sign() {
        assert_eq!((mercy_heal(-230),mercy_heal(-8),mercy_heal(-115)),(-345,-12,-172));
        assert!(MERCY_HEALS.iter().all(|id|(1642000..1700000).contains(id)||(21640000..21670000).contains(id)));
    }
    #[test]
    fn stride_only_speeds_up_locomotion() {
        assert!(locomotion(20000)&&locomotion(24999)&&locomotion(30020100),"locomotion in any action group");
        assert!(!locomotion(27000)&&!locomotion(30030000)&&!locomotion(-1));
    }
    #[test]
    fn last_stand_turns_on_below_forty_percent() {
        assert!(last_stand(399,1000)&&!last_stand(400,1000)&&!last_stand(0,1000)&&!last_stand(10,0));
    }
    #[test]
    fn breaker_counts_only_burst_ailments() {
        for (id,kind) in [(6400,Some("出血")),(6416,Some("出血")),(6450,Some("睡眠")),(6701,Some("冻伤")),(6754,Some("发狂"))] {assert_eq!(burst_kind(id),kind,"{id}");}
        for id in [6399,6501,6601,6801,6850,9670010,9670011] {assert_eq!(burst_kind(id),None,"poison, rot, blight, death and our own buildup are not bursts: {id}");}
    }
    #[test]
    fn effect_ids_do_not_overlap() {
        let ids:Vec<_>=player_ids().collect();
        assert_eq!(ids.len(),8+usize::from(embers::FRENZY_STACKS));
        assert!(!ids.contains(&EXECUTION_RELIEF),"the relief expires on its own, sync never strips it");
        assert_eq!(frenzy_effect(embers::FRENZY_STACKS),FIRST+38);
        let mut all=ids.clone();all.extend([BLOOD,FROST,FLASH,SPIRIT_COMMAND,CHAIN_MARK,CHAIN_RESIDENT+1,EXECUTION+1,EXECUTION_RELIEF]);
        all.extend(Ember::ALL.into_iter().filter_map(slots).flatten());
        let count=all.len();all.sort();all.dedup();assert_eq!(all.len(),count,"effect ids overlap");
        assert!(all.iter().all(|id|(FIRST..FIRST+100).contains(id)),"inside the ember range");
        assert!(Ember::ALL.into_iter().all(|e|e.stacking()==slots(e).is_some()));
        assert!(PLAYER_STACKING.iter().all(|e|e.stacking()&&*e!=Ember::SpiritKeeper));
        assert_eq!([0,1,2,3].map(kill_rank),[2,1,1,0]);
    }
}
