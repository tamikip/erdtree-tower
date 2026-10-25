use hudhook::tracing::{self,Event,Metadata,Subscriber,span,field::{Field,Visit}};
use std::{collections::HashMap,sync::{Mutex,atomic::{AtomicU64,Ordering}},time::{Duration,Instant}};

#[derive(Default)]
struct Gate {sites:HashMap<usize,Instant>}
impl Gate {
    fn admit(&mut self,key:usize,now:Instant)->bool {
        if let Some(last)=self.sites.get_mut(&key) {
            if now.duration_since(*last)<Duration::from_secs(30){return false;}
            *last=now;return true;
        }
        if self.sites.len()>=1024{return false;}
        self.sites.insert(key,now);true
    }
}
#[derive(Default)]
struct Sink {gate:Mutex<Gate>,span:AtomicU64}
struct Fields(String);
impl Visit for Fields {
    fn record_debug(&mut self,field:&Field,value:&dyn std::fmt::Debug) {
        use std::fmt::Write;
        let _=write!(self.0," {}={value:?}",field.name());
    }
}
impl Subscriber for Sink {
    fn enabled(&self,meta:&Metadata<'_>)->bool {meta.target().starts_with("hudhook")}
    fn new_span(&self,_:&span::Attributes<'_>)->span::Id {span::Id::from_u64(self.span.fetch_add(1,Ordering::Relaxed)+1)}
    fn record(&self,_:&span::Id,_:&span::Record<'_>) {}
    fn record_follows_from(&self,_:&span::Id,_:&span::Id) {}
    fn enter(&self,_:&span::Id) {}
    fn exit(&self,_:&span::Id) {}
    fn event(&self,event:&Event<'_>) {
        let meta=event.metadata();
        if !self.enabled(meta){return;}
        let Ok(mut gate)=self.gate.try_lock() else{return;};
        if !gate.admit(meta as *const _ as usize,Instant::now()){return;}
        drop(gate);
        let mut fields=Fields(String::new());event.record(&mut fields);
        super::log(format!("DX12_TRACE {} {}:{}{}",meta.level(),meta.target(),meta.line().unwrap_or(0),fields.0));
    }
}
pub fn enabled()->bool {
    std::env::var("ERDTREE_TOWER_RENDER_DIAGNOSTIC").as_deref()==Ok("1")
        ||super::LOG_PATH.parent().is_some_and(|p|p.join("render-diagnostic.flag").is_file())
}
pub fn install() {
    if enabled() {
        match tracing::subscriber::set_global_default(Sink::default()) {
            Ok(())=>super::log("DX12 diagnostic tracing enabled; repeated call sites limited to once per 30 seconds"),
            Err(e)=>super::log(format!("DX12 diagnostic tracing unavailable: {e}")),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trace_gate_is_bounded_and_preserves_distinct_stages() {
        let mut gate=Gate::default();let now=Instant::now();
        assert!(gate.admit(0,now));assert!(!gate.admit(0,now+Duration::from_secs(29)));
        assert!(gate.admit(1,now));assert!(gate.admit(0,now+Duration::from_secs(30)));
        for key in 2..1024 {assert!(gate.admit(key,now));}
        assert!(!gate.admit(1024,now));assert!(gate.admit(1,now+Duration::from_secs(31)));
    }
}
