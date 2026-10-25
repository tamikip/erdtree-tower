use std::{ffi::c_void,sync::atomic::{AtomicU64,AtomicUsize,Ordering}};
use eldenring::cs::{CharaInitParam,CalcCorrectGraph,SoloParamRepository};
use fromsoftware_shared::FromStatic;
use hudhook::mh::{MhHook,MH_EnableHook};

static ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static CURVE:AtomicU64=AtomicU64::new(0);
type Cost=unsafe extern "C" fn(i32)->u64;

fn fixed_cost(next:i32,start:u32)->Option<u64> {
    (start>0&&next>start as i32).then_some(1000)
}
unsafe extern "C" fn calculate(next:i32)->u64 {
    let curve=CURVE.load(Ordering::Acquire);
    let start=(curve>>32) as u32;
    if let Some(cost)=fixed_cost(next,start) {return cost;}
    let original:Cost=unsafe {std::mem::transmute(ORIGINAL.load(Ordering::Acquire))};
    unsafe {original(next)}
}
pub fn configure(archetype:u8)->Result<(),String> {
    let address=ORIGINAL.load(Ordering::Acquire);
    if address==0 {return Err("升级价格接口未加载，暂不开放加点".into());}
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"升级参数未就绪")?;
    let start=params.get::<CharaInitParam>(3000+u32::from(archetype)).ok_or("职业升级参数不存在")?.soul_lv() as u32;
    if CURVE.load(Ordering::Acquire)>>32==u64::from(start) {return Ok(());}
    if params.get::<CalcCorrectGraph>(200).is_none(){return Err("升级费用曲线未加载".into());}
    let original:Cost=unsafe {std::mem::transmute(address)};
    let first=unsafe {original(start as i32+1)};
    if start==0||start>713||first==0||first>1_000_000 {return Err("初始升级价格无效".into());}
    CURVE.store((u64::from(start)<<32)|first,Ordering::Release);
    super::log(format!("Fixed level cost: class={archetype}, starting level={start}, cost=1000"));
    Ok(())
}
pub unsafe fn install()->Result<(),String> {
    // GetLevelUpSoulCost (ESD env 105) and native leveling share this next-level cost function.
    let address=super::items::checked_function(0x687670,&[0x40,0x53,0x48,0x83,0xec,0x50,0x8b,0xd9,0x0f,0x29,0x74,0x24,0x40,0x48,0x8d,0x4c,0x24,0x20])? as *mut c_void;
    let hook=unsafe {MhHook::new(address,calculate as *mut c_void)}.map_err(|e|format!("Level cost hook: {e:?}"))?;
    ORIGINAL.store(hook.trampoline() as usize,Ordering::Release);
    if let Err(error)=unsafe {MH_EnableHook(address)}.ok() {
        ORIGINAL.store(0,Ordering::Release);
        return Err(format!("Level cost enable: {error:?}"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_menu_single_and_multi_level_cost_is_fixed() {
        assert_eq!(fixed_cost(20,0),None);
        for start in [1,5,6,7,8,9,10] {
            assert_eq!(fixed_cost(start,start as u32),None);
            for next in start+1..=713 {assert_eq!(fixed_cost(next,start as u32),Some(1000));}
            assert_eq!((1..=4).map(|step|fixed_cost(start+step,start as u32).unwrap()).sum::<u64>(),4000);
        }
    }
}
