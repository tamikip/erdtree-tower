use std::{sync::Mutex,time::{Duration,Instant}};
use eldenring::cs::{ChrInsExt,FieldInsHandle,WorldChrMan};

pub const EFFECT:i32=9670028;
const DURATION:Duration=Duration::from_secs(10);
type Key=(u64,u8,usize);
#[derive(Default)]
struct Edge {previous:Option<Vec<i32>>}
impl Edge {
    fn observe(&mut self,effects:Vec<i32>)->bool {
        let fresh=self.previous.as_ref().is_some_and(|old|effects.iter().any(|id|!old.contains(id)));
        self.previous=Some(effects);fresh
    }
}
struct Target {handle:FieldInsHandle,address:usize,slot:u8,edge:Edge,until:Option<Instant>,written:Option<(f32,f32)>}
#[derive(Default)]
struct State {key:Option<Key>,targets:Vec<Target>}
static STATE:Mutex<State>=Mutex::new(State{key:None,targets:Vec::new()});
fn active(until:Option<Instant>,now:Instant)->bool {until.is_some_and(|t|t>now)}
fn refresh(until:&mut Option<Instant>,now:Instant) {*until=Some(now+DURATION);}

fn restore(current:&mut f32,written:&mut Option<(f32,f32)>) {
    if let Some((base,slow))=written.take() {if (*current-slow).abs()<0.0001 {*current=base;}}
}
fn slow(current:&mut f32)->Option<(f32,f32)> {
    if !current.is_finite()||*current<=0.0{return None;}
    let base=*current;*current=base*0.7;Some((base,*current))
}
// Restore our previous frame before other speed modifiers run; apply once after them.
pub fn begin_frame(world:&mut WorldChrMan) {
    let Ok(mut state)=STATE.lock() else{return};
    for target in &mut state.targets {
        if let Some(chr)=world.chr_ins_by_handle_mut(&target.handle)&&chr as *mut _ as usize==target.address {
            restore(&mut chr.modules.behavior.animation_speed,&mut target.written);
        }
    }
}
pub fn finish_frame(world:&mut WorldChrMan,clear:bool) {
    let Ok(mut state)=STATE.lock() else{return};let now=Instant::now();
    for target in &mut state.targets {
        let Some(chr)=world.chr_ins_by_handle_mut(&target.handle) else{continue};
        if chr as *mut _ as usize!=target.address {continue;}
        let live=!clear&&chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag()&&active(target.until,now);
        if live {
            if !chr.special_effect.entries().any(|e|e.param_id==EFFECT){chr.apply_speffect(EFFECT,false);}
            target.written=slow(&mut chr.modules.behavior.animation_speed);
        } else {
            restore(&mut chr.modules.behavior.animation_speed,&mut target.written);
            if target.until.take().is_some(){chr.remove_speffect(EFFECT);}
        }
    }
    if clear {*state=State::default();}
}
pub fn observe(world:&mut WorldChrMan,key:Key,foes:&[(FieldInsHandle,u8,u8)],enabled:bool) {
    let changed=STATE.lock().is_ok_and(|s|s.key!=Some(key));
    if changed {finish_frame(world,true);}
    let Ok(mut state)=STATE.lock() else{return};state.key=Some(key);
    state.targets.retain_mut(|t|{
        let Some(chr)=world.chr_ins_by_handle_mut(&t.handle) else{return false};
        if chr as *mut _ as usize!=t.address{return false;}
        if foes.iter().any(|(h,_,slot)|*slot==t.slot&&h.block_id==t.handle.block_id&&h.selector==t.handle.selector){return true;}
        restore(&mut chr.modules.behavior.animation_speed,&mut t.written);
        if t.until.is_some(){chr.remove_speffect(EFFECT);}false
    });
    for &(handle,_,slot) in foes {
        let Some(chr)=world.chr_ins_by_handle_mut(&handle) else{continue};
        if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(){continue;}
        let address=chr as *mut _ as usize;
        let i=state.targets.iter().position(|t|t.address==address&&t.slot==slot).unwrap_or_else(||{
            state.targets.push(Target{handle,address,slot,edge:Edge::default(),until:None,written:None});state.targets.len()-1
        });
        let effects=chr.special_effect.entries().map(|e|e.param_id).filter(|id|(6700..=6749).contains(id)).collect();
        let target=&mut state.targets[i];
        if target.edge.observe(effects)&&enabled {
            refresh(&mut target.until,Instant::now());
            chr.remove_speffect(EFFECT);chr.apply_speffect(EFFECT,false);
            super::log(format!("FROST_SHACKLE slot={slot} speed=0.7 attack=0.8 duration=10s"));
        }
    }
}
pub fn snapshot()->Vec<(u8,u16)> {
    let now=Instant::now();STATE.lock().map(|s|s.targets.iter().filter_map(|t|{
        let left=t.until?.checked_duration_since(now)?.as_millis().min(10000) as u16;
        (left>0).then_some((t.slot,left))
    }).collect()).unwrap_or_default()
}
pub fn receive(world:&mut WorldChrMan,key:Key,foes:&[(FieldInsHandle,u8,u8)],active:&[(u8,Instant)]) {
    observe(world,key,foes,false);
    let Ok(mut state)=STATE.lock() else{return};
    for t in &mut state.targets {
        // Preserve expired deadlines until finish_frame removes the native effect.
        if let Some((_,until))=active.iter().find(|(slot,_)|*slot==t.slot){t.until=Some(*until);}
        else if t.until.is_some(){t.until=Some(Instant::now());}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_new_frost_bursts_trigger() {
        let mut e=Edge::default();assert!(!e.observe(vec![6700]));assert!(!e.observe(vec![6700]));
        assert!(!e.observe(vec![]));assert!(e.observe(vec![6700]));assert!(!e.observe(vec![6700]));
        assert!(e.observe(vec![6701]));
    }
    #[test]
    fn slowdown_composes_without_compounding_and_restores() {
        for base in [1.0,1.1,1.3] {
            let mut speed=base;let mut written=None;
            for _ in 0..1000 {restore(&mut speed,&mut written);written=slow(&mut speed);assert!((speed-base*0.7).abs()<0.0001);}
            restore(&mut speed,&mut written);assert!((speed-base).abs()<0.0001);
        }
    }
    #[test]
    fn external_speed_changes_are_not_overwritten() {
        let mut speed=1.0;let mut written=slow(&mut speed);speed=1.3;
        restore(&mut speed,&mut written);assert_eq!(speed,1.3);
        written=slow(&mut speed);assert!((speed-0.91).abs()<0.0001);
        restore(&mut speed,&mut written);assert_eq!(speed,1.3);
    }
    #[test]
    fn duration_is_ten_seconds_and_refresh_does_not_stack() {
        let start=Instant::now();let mut until=None;assert!(!active(until,start));
        refresh(&mut until,start);assert!(active(until,start+Duration::from_millis(9999)));
        assert!(!active(until,start+Duration::from_secs(10)));
        refresh(&mut until,start+Duration::from_secs(4));
        assert!(active(until,start+Duration::from_secs(10)));
        assert!(!active(until,start+Duration::from_secs(14)));
    }
}
