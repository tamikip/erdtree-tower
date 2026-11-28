//! 诊断（HansFix 测试版）：战斗中每 3 秒记一次每个敌人的失衡（韧性）状态，查「结晶人失衡值完全不变」。
//! 只读不写，最多记 400 行。
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32,Ordering};
use std::time::{Duration,Instant};
use eldenring::cs::{FieldInsHandle,WorldChrMan};

static LINES:AtomicU32=AtomicU32::new(0);
static LAST:Mutex<Vec<(FieldInsHandle,Instant)>>=Mutex::new(Vec::new());

pub fn observe(world:&WorldChrMan,handles:&[FieldInsHandle]) {
    if LINES.load(Ordering::Relaxed)>=400 {return;}
    let Ok(mut last)=LAST.try_lock() else{return};
    let now=Instant::now();
    for handle in handles {
        if last.iter().any(|(h,t)|h==handle&&now.duration_since(*t)<Duration::from_secs(3)) {continue;}
        let Some(chr)=world.chr_ins_by_handle(handle) else{continue};
        if chr.modules.data.hp<=0 {continue;}
        last.retain(|(h,_)|h!=handle);
        if last.len()>64 {last.clear();}
        last.push((*handle,now));
        let (armor,tough)=(&chr.modules.super_armor,&chr.modules.toughness);
        let effects:Vec<i32>=chr.special_effect.entries().map(|e|e.param_id).take(16).collect();
        if LINES.fetch_add(1,Ordering::Relaxed)<400 {
            super::log(format!("STANCE_TRACE npc={} model={} hp={}/{} sa={:.1}/{:.1} sa_recover={:.2} broken={} toughness={:.1}/{:.1} t_recover={:.2} effects={effects:?}",
                chr.npc_param_id,chr.npc_id,chr.modules.data.hp,chr.modules.data.max_hp,armor.sa_durability,armor.sa_durability_max,armor.recover_time,armor.poise_broken_state,
                tough.toughness,tough.toughness_max,tough.recover_time));
        }
    }
}
