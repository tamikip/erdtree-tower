use erdtree_tower::Phase;
use eldenring::cs::FieldArea;
use fromsoftware_shared::FromStatic;
use std::sync::Mutex;

const LOCK_FLAG:i32=45007995;

#[derive(Default)]
struct Override { original:Option<i32> }
impl Override {
    fn apply(&mut self,current:&mut i32,locked:bool) {
        if locked {
            if *current!=LOCK_FLAG {self.original=Some(*current);*current=LOCK_FLAG;}
        } else if let Some(original)=self.original.take() {
            if *current==LOCK_FLAG {*current=original;}
        }
    }
}
fn locked(phase:Option<&Phase>,at_hub:bool)->bool {
    phase.is_some_and(|p|!at_hub||!matches!(p,Phase::Dead|Phase::Won))
}
pub fn sync(phase:Option<&Phase>,at_hub:bool) {
    static GUARD:Mutex<Override>=Mutex::new(Override{original:None});
    let Ok(area)=(unsafe {FieldArea::instance_mut()}) else{return;};
    let Ok(mut guard)=GUARD.lock() else{return;};
    let active=locked(phase,at_hub);
    if active && super::native_talk::set_flag(LOCK_FLAG as u32,false).is_err(){return;}
    guard.apply(&mut area.enable_fast_travel_event_flag,active);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lock_covers_run_and_return_but_not_idle_hub() {
        assert!(locked(Some(&Phase::Map),true));
        assert!(locked(Some(&Phase::Combat{id:0}),false));
        for phase in [Phase::Dead,Phase::Won] {
            assert!(locked(Some(&phase),false));
            assert!(!locked(Some(&phase),true));
        }
        assert!(!locked(None,true));
    }
    #[test]
    fn restores_engine_value_without_clobbering_map_changes() {
        let mut guard=Override::default();let mut flag=123;
        guard.apply(&mut flag,true);assert_eq!(flag,LOCK_FLAG);
        guard.apply(&mut flag,true);
        guard.apply(&mut flag,false);assert_eq!(flag,123);
        guard.apply(&mut flag,true);flag=456;
        guard.apply(&mut flag,true);guard.apply(&mut flag,false);assert_eq!(flag,456);
        guard.apply(&mut flag,true);flag=789;
        guard.apply(&mut flag,false);assert_eq!(flag,789);
    }
}
