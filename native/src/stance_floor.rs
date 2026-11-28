//! HansFix：失衡保底。高难度律法（不动如山）、Boss 词条、敌人自身的抗性叠在一起时，
//! 玩家的攻击可能完全打不动敌人的失衡值（结晶人就是这样）。这里保证玩家每次打中敌人，至少扣 1 点失衡。
//!
//! 做法：伤害钩子里记下「玩家打中了哪个敌人、那一刻它的失衡值」，下一帧（游戏任务线程）对比：
//! 这次命中加上一帧前后一共没扣到 1 点，就补扣到 1 点。只动失衡值，不动生命和其它东西。
//! 扣到见底时留 0.001（和余烬「破势」一样），让下一次命中按游戏自己的流程触发失衡。
use std::collections::HashMap;
use std::sync::Mutex;
use eldenring::cs::{FieldInsHandle,WorldChrMan};

/// 玩家句柄（每帧更新），钩子里用来判断攻击者是不是玩家
static PLAYERS:Mutex<Vec<u64>>=Mutex::new(Vec::new());
/// 这一帧玩家命中的敌人：(敌人, 命中那一刻的失衡值)
static HITS:Mutex<Vec<(FieldInsHandle,f32)>>=Mutex::new(Vec::new());
/// 上一帧每个敌人的失衡值
static LAST:Mutex<Option<HashMap<u64,f32>>>=Mutex::new(None);
const FLOOR:f32=1.0;
const BREAK:f32=0.001;

fn bits(handle:&FieldInsHandle)->u64 {unsafe {std::mem::transmute_copy::<FieldInsHandle,u64>(handle)}}

/// 伤害钩子里调用（不阻塞：拿不到锁就放弃这一次）
pub fn on_hit(attacker:FieldInsHandle,target:FieldInsHandle,stance_now:f32) {
    if !PLAYERS.try_lock().is_ok_and(|p|p.contains(&bits(&attacker))&&!p.contains(&bits(&target))) {return;}
    if let Ok(mut hits)=HITS.try_lock()&&hits.len()<64 {hits.push((target,stance_now));}
}

/// 每帧在战斗里调用：foes 是这场战斗的敌人
pub fn tick(world:&mut WorldChrMan,foes:&[FieldInsHandle]) {
    if let Ok(mut players)=PLAYERS.lock() {
        players.clear();
        players.extend(world.player_chr_set.characters().map(|p|bits(&p.chr_ins.field_ins_handle)));
        if let Some(me)=world.main_player.as_ref() {players.push(bits(&me.chr_ins.field_ins_handle));}
    }
    let hits=HITS.lock().map(|mut h|std::mem::take(&mut *h)).unwrap_or_default();
    let Ok(mut last)=LAST.lock() else{return};
    let last=last.get_or_insert_with(HashMap::new);
    for (target,at_hit) in &hits {
        let Some(chr)=world.chr_ins_by_handle_mut(target) else{continue};
        if chr.modules.data.hp<=0 {continue;}
        let armor=&mut chr.modules.super_armor;
        let (now,max)=(armor.sa_durability,armor.sa_durability_max);
        if !now.is_finite()||!max.is_finite()||max<=0.0||armor.poise_broken_state||now<=BREAK {continue;}
        // 命中前的失衡值：上一帧记的值和命中那一刻的值里取大的（命中那一刻可能已经扣过了）
        let before=last.get(&bits(target)).copied().unwrap_or(*at_hit).max(*at_hit);
        if before-now>=FLOOR {continue;}
        let want=(before-FLOOR).max(BREAK);
        if want<now {armor.sa_durability=want;}
    }
    last.clear();
    for handle in foes {
        if let Some(chr)=world.chr_ins_by_handle(handle) {last.insert(bits(handle),chr.modules.super_armor.sa_durability);}
    }
}
