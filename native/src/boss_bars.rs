use std::sync::{Mutex,atomic::{AtomicBool,Ordering}};
static INITIALIZED:AtomicBool=AtomicBool::new(false);
static FLAGS:Mutex<Vec<u32>>=Mutex::new(Vec::new());
const ACTIVE_FLAG:u32=47000003;
pub fn flag(index:u32,slot:usize)->Option<u32> {
    (index>0&&slot<3).then(||47000000+(index-1)*4+slot as u32)
}
fn sync_flags(active:Vec<u32>) {
    let Ok(mut previous)=FLAGS.lock() else{return;};
    if !INITIALIZED.load(Ordering::Relaxed) {
        if crate::native_talk::set_flag(ACTIVE_FLAG,false).is_err(){return;}
        for index in 1..=include_str!("../../scene-assets/boss-pool.txt").lines().count() as u32 {
            for slot in 0..3 {if crate::native_talk::set_flag(flag(index,slot).unwrap(),false).is_err(){return;}}
        }
        INITIALIZED.store(true,Ordering::Relaxed);
    }
    for old in previous.iter().filter(|f|!active.contains(f)){let _=crate::native_talk::set_flag(*old,false);}
    for new in active.iter().filter(|f|!previous.contains(f)){let _=crate::native_talk::set_flag(*new,true);}
    let _=crate::native_talk::set_flag(ACTIVE_FLAG,!active.is_empty());
    *previous=active;
}
pub fn sync(active:Vec<(usize,u32,eldenring::cs::FieldInsHandle)>) {
    use fromsoftware_shared::FromStatic;
    // Map-event entity lookup cannot reliably resolve debug-spawned characters.
    sync_flags(Vec::new());
    let Ok(frontend)=(unsafe {eldenring::cs::CSFeManImp::instance_mut()}) else{return;};
    for (slot,entry) in frontend.boss_health_displays.iter_mut().enumerate() {
        let desired=active.iter().find(|(s,index,handle)|*s==slot&&*index>0&&!handle.is_empty())
            .map(|(_,index,handle)|(9600000+*index as i32-1,*handle));
        bind(entry,desired);
    }
}
fn bind(entry:&mut eldenring::cs::BossHealthDisplayEntry,desired:Option<(i32,eldenring::cs::FieldInsHandle)>) {
    if let Some((name,handle))=desired {
        if entry.fmg_id!=name||entry.field_ins_handle!=handle {
            entry.damage_taken=0;
            crate::log(format!("Native boss bar bound: name={name} handle={handle}"));
        }
        entry.fmg_id=name;entry.field_ins_handle=handle;
    }else if (9600000..9600000+include_str!("../../scene-assets/boss-pool.txt").lines().count() as i32).contains(&entry.fmg_id) {
        entry.fmg_id=-1;entry.field_ins_handle.selector.0=u32::MAX;entry.damage_taken=0;
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn native_display_uses_live_handle_and_clears_only_tower_slots() {
        use eldenring::cs::{BossHealthDisplayEntry,FieldInsHandle,FieldInsSelector,BlockId};
        let mut entry:BossHealthDisplayEntry=unsafe {std::mem::zeroed()};
        let handle=FieldInsHandle{selector:FieldInsSelector(0x17200000),block_id:BlockId::from(-1)};
        super::bind(&mut entry,Some((9600005,handle)));
        assert_eq!(entry.field_ins_handle,handle);assert_eq!(entry.fmg_id,9600005);
        entry.damage_taken=125;super::bind(&mut entry,Some((9600005,handle)));assert_eq!(entry.damage_taken,125);
        super::bind(&mut entry,None);assert!(entry.field_ins_handle.is_empty());assert_eq!(entry.fmg_id,-1);
        entry.fmg_id=1234;entry.field_ins_handle=handle;super::bind(&mut entry,None);assert_eq!(entry.field_ins_handle,handle);
    }
    #[test]
    fn bar_slots_are_distinct_and_regular_enemies_have_none() {
        assert_eq!(super::flag(0,0),None);assert_eq!(super::flag(1,3),None);
        let count=include_str!("../../scene-assets/boss-pool.txt").lines().count() as u32;
        let flags:std::collections::HashSet<_>=(1..=count).flat_map(|i|(0..3).map(move|s|super::flag(i,s).unwrap())).collect();
        assert_eq!(flags.len(),count as usize*3);
        assert!(!flags.contains(&super::ACTIVE_FLAG));
        assert!(![45020970,45020971].contains(&super::ACTIVE_FLAG));
        let script=include_str!("../../scene-builder/BossAssets.cs");
        assert!(!script.contains("45020971"));
        assert!(script.contains(&format!("{}u",super::ACTIVE_FLAG)));
    }
}
