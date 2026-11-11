use eldenring::cs::{CSGaitemImp,CSWepGaitemIns,ItemCategory,PlayerGameData};
use fromsoftware_shared::{FromStatic,Superclass};
use erdtree_tower::mausoleum::Weapon;
pub const DOWN:u32=45011864;
pub const DOOR:u32=45011865;
pub const REQUIRED_SKULLS:usize=12;
pub fn restore_flow(copy:&erdtree_tower::mausoleum::CopyState)->Result<(),String> {
    super::native_talk::set_flag(DOWN,copy.lowered||copy.completed||copy.receipt.is_some())?;
    super::native_talk::set_flag(DOOR,copy.opened||copy.completed||copy.receipt.is_some())
}
pub fn flow()->Result<(bool,bool),String>{Ok((super::native_talk::flag(DOWN)?,super::native_talk::flag(DOOR)?))}
pub fn broken_skulls()->Result<usize,String> {
    (45011500..45011544).try_fold(0,|count,id|super::native_talk::flag(id).map(|broken|count+usize::from(broken)))
}
pub fn trace(world:&eldenring::cs::WorldChrMan,node:usize) {
    if !super::debug_menu::enabled(){return;}
    static LAST:std::sync::Mutex<Option<std::time::Instant>>=std::sync::Mutex::new(None);
    let Ok(mut last)=LAST.try_lock() else{return};
    if last.is_some_and(|t|t.elapsed()<std::time::Duration::from_secs(5)){return;}
    *last=Some(std::time::Instant::now());
    let actor=world.chr_sets.iter().flatten().flat_map(|s|s.characters()).find(|c|c.event_entity_id==45011860);
    let state=actor.map(|c|(c.npc_id,c.debug_flags.character_disabled(),c.modules.data.hp,c.special_effect.entries().filter_map(|e|[12450,12455,12460,12465].contains(&e.param_id).then_some(e.param_id)).collect::<Vec<_>>()));
    crate::log(format!("MAUSOLEUM_TRACE node={node} broken={:?}/44 required={REQUIRED_SKULLS} flow={:?} actor={state:?}",broken_skulls(),flow()));
}

pub fn active(run:Option<&erdtree_tower::Run>)->bool {
    run.is_some_and(|run|matches!(run.phase,erdtree_tower::Phase::Node{id}
        if run.nodes.get(id).is_some_and(|n|n.kind==erdtree_tower::NodeKind::Event))
        &&run.regional_event.mausoleum.as_ref().is_some_and(|copy|copy.accepted))
}
pub fn recovery_target(run:Option<&erdtree_tower::Run>,entrance:Option<super::party_death::Position>,flow:Option<(bool,bool)>,survivor:bool)->Option<super::party_death::Position> {
    let target=entrance?;
    (active(run)&&flow==Some((true,true))&&survivor&&target.block==0x0d000000
        &&target.xyz.iter().all(|v|v.is_finite())).then_some(target)
}
pub fn sync_visibility(run:Option<&erdtree_tower::Run>) {
    let wanted=active(run);
    if super::native_talk::flag(45011859).is_ok_and(|current|current!=wanted)
        &&super::native_talk::set_flag(45011859,wanted).is_ok() {
        crate::log(format!("MAUSOLEUM visibility corrected: active={wanted}"));
    }
}

pub fn inventory(data:&PlayerGameData)->Result<Vec<(Weapon,u64)>,String> {
    let manager=unsafe {CSGaitemImp::instance()}.map_err(|_|"武器实例尚未加载")?;
    let mut items=Vec::new();
    for entry in data.equipment.equip_inventory_data.items_data.normal_entries().iter().filter_map(|e|e.as_option()) {
        if entry.quantity==0||entry.item_id.category()!=ItemCategory::Weapon {continue;}
        let id=entry.item_id.param_id();
        if !erdtree_tower::rewards::lookup(id/10000*10000).is_some_and(|item|item.kind==erdtree_tower::rewards::ItemKind::Weapon){continue;}
        let weapon=manager.gaitem_ins_by_handle(&entry.gaitem_handle).and_then(|w|w.as_subclass::<CSWepGaitemIns>()).ok_or("武器实例无效，暂不能复制")?;
        let gem_handle=weapon.gem_slot_table.gem_slots[0].gaitem_handle;
        let gem=if gem_handle.is_indexed() {
            manager.gaitem_ins_by_handle(&gem_handle).and_then(|g|g.item_id.param_id()).ok_or("战灰实例无效，暂不能复制")? as i32
        }else{-1};
        items.push((Weapon{id,gem},u64::from(entry.quantity)));
    }
    Ok(items)
}
pub fn count(items:&[(Weapon,u64)],weapon:Weapon)->u64 {items.iter().filter(|(w,_)|*w==weapon).map(|(_,n)|*n).sum()}
pub fn equipped(data:&PlayerGameData)->Result<Weapon,String> {
    let slot=data.equipment.chr_asm.equipment.active_right_weapon_slot() as usize;
    let handle=data.equipment.chr_asm.gaitem_handles[slot];
    let manager=unsafe {CSGaitemImp::instance()}.map_err(|_|"武器实例未加载")?;
    let weapon=manager.gaitem_ins_by_handle(&handle).and_then(|w|w.as_subclass::<CSWepGaitemIns>()).ok_or("请先在右手装备要复制的武器")?;
    let gem_handle=weapon.gem_slot_table.gem_slots[0].gaitem_handle;
    let gem=if gem_handle.is_indexed(){manager.gaitem_ins_by_handle(&gem_handle).and_then(|g|g.item_id.param_id()).ok_or("战灰尚未加载")? as i32}else{-1};
    let id=data.equipment.chr_asm.equipment_param_ids[slot];
    if id<0{return Err("请先装备武器".into());}
    let chosen=Weapon{id:id as u32,gem};
    if count(&inventory(data)?,chosen)==0{return Err("这件装备不能复制".into());}Ok(chosen)
}
pub fn request_copy(data:&PlayerGameData,receipt:&erdtree_tower::mausoleum::Receipt)->Result<(),String> {
    if !receipt.weapon.valid()||receipt.target<2{return Err("复制记录无效".into());}
    let held=count(&inventory(data)?,receipt.weapon);
    if held>=receipt.target{return Ok(());}
    if held+1!=receipt.target{return Err("原武器数量发生变化，暂停复制以防重复领取".into());}
    if data.equipment.equip_inventory_data.items_data.is_normal_items_full(){return Err("背包已满，请整理后再复制".into());}
    crate::items::give(&[(receipt.weapon.id as i32,1,receipt.weapon.gem)])
}
#[cfg(test)]
mod tests {
    use super::*;
    fn visit()->erdtree_tower::Run {
        let mut run=erdtree_tower::Run::new(42,erdtree_tower::Class::Hero);
        let id=run.nodes.iter().position(|n|n.kind==erdtree_tower::NodeKind::Event).unwrap();
        run.phase=erdtree_tower::Phase::Node{id};run.regional_event.mausoleum=Some(erdtree_tower::mausoleum::CopyState::accepted());run
    }
    #[test]
    fn stomp_recovery_waits_for_open_door_and_a_surviving_teammate() {
        let run=visit();let entrance=super::super::party_death::Position{block:0x0d000000,xyz:[5.86,1.50,4.43]};
        for flow in [None,Some((false,false)),Some((true,false)),Some((false,true))] {
            assert_eq!(recovery_target(Some(&run),Some(entrance),flow,true),None);
        }
        assert_eq!(recovery_target(Some(&run),Some(entrance),Some((true,true)),false),None,"a party wipe must not be revived");
        assert_eq!(recovery_target(Some(&run),Some(entrance),Some((true,true)),true),Some(entrance));
    }
    #[test]
    fn stale_flags_and_unknown_entrances_never_trigger_mausoleum_recovery() {
        let mut run=visit();let p=super::super::party_death::Position{block:0x0d000000,xyz:[5.86,1.50,4.43]};
        assert_eq!(recovery_target(Some(&run),None,Some((true,true)),true),None);
        for bad in [super::super::party_death::Position{block:0x2d010000,..p},super::super::party_death::Position{xyz:[f32::NAN,1.0,0.0],..p}] {
            assert_eq!(recovery_target(Some(&run),Some(bad),Some((true,true)),true),None);
        }
        assert_eq!(recovery_target(None,Some(p),Some((true,true)),true),None);
        for phase in [erdtree_tower::Phase::Map,erdtree_tower::Phase::Dead,erdtree_tower::Phase::Won] {
            run.phase=phase;
            assert_eq!(recovery_target(Some(&run),Some(p),Some((true,true)),true),None);
        }
    }
    #[test]
    fn crushed_player_recovers_at_entrance_without_regranting_copy_rewards() {
        use super::super::party_death::{Action,Death,Position};
        let run=visit();let before=run.clone();let entry=Position{block:0x0d000000,xyz:[5.86,1.50,4.43]};
        let crushed=Position{xyz:[0.0,0.0,30.0],..entry};let mut death=Death::default();
        assert_eq!(death.observe(Some(7),Some((crushed,true)),false,false),Action::Waiting);
        let target=recovery_target(Some(&run),Some(entry),Some((true,true)),true).unwrap();
        assert_eq!(death.observe(Some(7),Some((crushed,true)),true,false),Action::RequestRevival(crushed));
        death.redirect(target);
        assert_eq!(death.observe_confirmed(Some(7),Some((target,false)),true,false,false),Action::Waiting);
        assert_eq!(death.observe_confirmed(Some(7),Some((target,false)),true,false,true),Action::FinishRevival);
        assert_eq!(run,before);
    }
    #[test]
    fn structure_is_visible_only_during_an_accepted_mausoleum_visit() {
        use erdtree_tower::{Run,Class,Phase,NodeKind};
        let mut run=Run::new(42,Class::Hero);
        let id=run.nodes.iter().position(|n|n.kind==NodeKind::Event).unwrap();
        assert!(!active(None));assert!(!active(Some(&run)));
        run.phase=Phase::Node{id};run.regional_event.mausoleum=Some(Default::default());
        assert!(!active(Some(&run)),"NPC offer does not show the structure");
        run.regional_event.mausoleum=Some(erdtree_tower::mausoleum::CopyState::accepted());
        assert!(active(Some(&run)));
        run.regional_event.mausoleum.as_mut().unwrap().completed=true;
        assert!(active(Some(&run)),"keep the exit available after copying");
        for phase in [Phase::Map,Phase::Combat{id},Phase::Dead,Phase::Won] {
            run.phase=phase;assert!(!active(Some(&run)),"stale visit data must not show the structure");
        }
    }
    #[test]fn inventory_receipt_does_not_confuse_reinforcement_affinity_or_ashes(){
        let weapon=Weapon{id:160020623,gem:12200};
        let items=[(weapon,1),(Weapon{id:160020624,..weapon},1),(Weapon{id:160020023,..weapon},1),(Weapon{gem:10000,..weapon},1),(weapon,1)];
        assert_eq!(count(&items,weapon),2);
    }
}
