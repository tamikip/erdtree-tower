use std::{collections::HashMap,sync::{LazyLock,Mutex}};
use eldenring::cs::SummonBuddyManager;

pub(super) fn custom(param:i32)->bool {
    param.checked_sub(1600000000).is_some_and(|n|n>=0
        &&n/40000<(erdtree_tower::rewards::CUSTOM_SPIRITS.len() as i32)&&n%1000<3)
}
#[derive(Clone,Copy)]
struct Sample {custom:bool,dismissing:bool,alive:bool,native_animation:bool,remaining:f32,alpha:f32,modifier:f32}
#[derive(Clone,Copy)]
struct Saved {alpha:f32,modifier:f32,applied:f32,finished:bool}
#[derive(Debug,PartialEq)]
enum Change {Start(f32),Restore(f32,f32),Complete}
fn step(saved:&mut Option<Saved>,s:Sample)->Option<Change> {
    let eligible=s.custom&&s.dismissing&&s.alive&&!s.native_animation;
    if let Some(old)=*saved {
        if !eligible {
            *saved=None;
            // Do not overwrite another native effect that took ownership of the fade.
            return (s.modifier==old.applied).then_some(Change::Restore(old.alpha,old.modifier));
        }
        if !old.finished&&s.alpha<=0.01 {
            *saved=Some(Saved{finished:true,..old});return Some(Change::Complete);
        }
        return None;
    }
    if !eligible||!s.remaining.is_finite()||s.remaining<=0.0||s.remaining>60.0
        ||!s.alpha.is_finite()||!(0.0..=1.0).contains(&s.alpha)||s.alpha==0.0
        ||!s.modifier.is_finite()||s.modifier!=0.0{return None;}
    let applied=-s.alpha/s.remaining.min(1.5).max(0.016);
    *saved=Some(Saved{alpha:s.alpha,modifier:s.modifier,applied,finished:false});
    Some(Change::Start(applied))
}
#[derive(Default)]
struct State {manager:usize,members:HashMap<(u64,usize),Option<Saved>>}
static STATE:LazyLock<Mutex<State>>=LazyLock::new(||Mutex::new(State::default()));

// Cosmetic only: remote replicas use their own native dismissal marker, never local-owner cleanup.
pub(super) unsafe fn tick(manager:*mut u8) {
    if manager.is_null(){return;}
    let Ok(mut state)=STATE.try_lock() else{return;};
    if state.manager!=manager as usize{state.members.clear();state.manager=manager as usize;}
    let buddy=unsafe {&*manager.cast::<SummonBuddyManager>()};
    let mut present=std::collections::HashSet::new();
    for group in buddy.groups.iter().flat_map(|pair|pair.second.iter()) {
        if !custom(group.buddy_param_id){continue;}
        let Some(chr)=super::live_chr_mut(group) else{continue;};
        let handle=chr.field_ins_handle;
        let key=((u64::from(handle.block_id.0 as u32)<<32)|u64::from(handle.selector.0),group.chr_ins.as_ptr() as usize);
        present.insert(key);
        let time=&chr.modules.time_act;
        let animation=time.anim_queue.get(time.read_idx as usize).map(|a|a.anim_id);
        let sample=Sample{custom:true,dismissing:group.disappear_requested,
            alive:chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag(),native_animation:matches!(animation,Some(1840|60505)),
            remaining:group.disappear_delay_sec,alpha:chr.base_transparency,modifier:chr.base_transparency_modifier};
        match step(state.members.entry(key).or_default(),sample) {
            Some(Change::Start(rate))=>{
                chr.base_transparency_modifier=rate;
                crate::log(format!("ASH_DISMISS_FADE start handle={handle} npc={} remote={} remaining={:.3} alpha={:.3} rate={rate:.3}; native removal retained",chr.npc_param_id,group.is_remote,sample.remaining,sample.alpha));
            }
            Some(Change::Restore(alpha,modifier))=>{
                chr.base_transparency=alpha;chr.base_transparency_modifier=modifier;
                crate::log(format!("ASH_DISMISS_FADE released handle={handle} native_animation={} alive={}",sample.native_animation,sample.alive));
            }
            Some(Change::Complete)=>crate::log(format!("ASH_DISMISS_FADE complete handle={handle} alpha={:.3} remaining={:.3}",sample.alpha,sample.remaining)),
            None=>{}
        }
    }
    state.members.retain(|key,_|present.contains(key));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample()->Sample{Sample{custom:true,dismissing:true,alive:true,native_animation:false,remaining:3.0,alpha:1.0,modifier:0.0}}
    #[test]
    fn fade_finishes_before_native_removal_without_extending_timer(){
        for remaining in [0.1,0.5,1.5,3.0,5.0] {
            let mut saved=None;
            let Some(Change::Start(rate))=step(&mut saved,Sample{remaining,..sample()}) else{panic!("fade missing")};
            assert!((1.0+rate*remaining).max(0.0)<0.00001);
            assert_eq!(step(&mut saved,Sample{remaining,alpha:0.5,modifier:rate,..sample()}),None);
        }
    }
    #[test]
    fn native_animation_ordinary_summons_death_and_existing_fades_are_untouched(){
        for s in [Sample{custom:false,..sample()},Sample{native_animation:true,..sample()},Sample{alive:false,..sample()},
            Sample{dismissing:false,..sample()},Sample{remaining:f32::NAN,..sample()},Sample{remaining:0.0,..sample()},
            Sample{remaining:61.0,..sample()},Sample{alpha:0.0,..sample()},Sample{modifier:-0.5,..sample()}] {
            assert_eq!(step(&mut None,s),None);
        }
    }
    #[test]
    fn cancelled_dismissal_restores_only_our_visual_change(){
        let mut saved=None;
        let Some(Change::Start(rate))=step(&mut saved,sample()) else{panic!()};
        assert_eq!(step(&mut saved,Sample{dismissing:false,alpha:0.4,modifier:rate,..sample()}),Some(Change::Restore(1.0,0.0)));
        step(&mut saved,sample());
        assert_eq!(step(&mut saved,Sample{native_animation:true,modifier:-0.25,..sample()}),None);
        assert!(saved.is_none());
    }
    #[test]
    fn custom_range_excludes_vanilla_and_invalid_slots(){
        assert!(custom(1600000000));assert!(custom(1600039002));
        assert!(!custom(20000000));assert!(!custom(1600000003));assert!(!custom(i32::MIN));
    }
    #[test]
    fn completion_is_observed_once_and_cancellation_still_restores(){
        let mut saved=None;
        let Some(Change::Start(rate))=step(&mut saved,sample()) else{panic!()};
        let end=Sample{alpha:0.0,modifier:rate,..sample()};
        assert_eq!(step(&mut saved,end),Some(Change::Complete));
        assert_eq!(step(&mut saved,end),None);
        assert_eq!(step(&mut saved,Sample{dismissing:false,..end}),Some(Change::Restore(1.0,0.0)));
    }
}
