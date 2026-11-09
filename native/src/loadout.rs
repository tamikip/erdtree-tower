use eldenring::cs::{CharaInitParam, EquipGameData, EquipInventoryData, ItemCategory, ItemId, PlayerGameData, SoloParamRepository};
use fromsoftware_shared::FromStatic;

fn clear_empty_icons(slots:&[eldenring::cs::EquipDataItem],ids:&mut [eldenring::cs::OptionalItemId]) {
    for (slot,id) in slots.iter().zip(ids) {
        if slot.index<0 && unsafe {std::mem::transmute::<_,u32>(slot.gaitem_handle)}==u32::MAX {*id=eldenring::cs::OptionalItemId::NONE;}
    }
}

pub fn repair_item_icons(data:&mut PlayerGameData) {
    let equipment=&mut data.equipment;
    clear_empty_icons(&equipment.equip_item_data.quick_slots,&mut equipment.equipment_entries.quick_tems);
    clear_empty_icons(&equipment.equip_item_data.pouch_slots,&mut equipment.equipment_entries.pouch);
}
fn invalid_empty_weapon(id:i32,handle:u32,index:u32)->bool {
    id==-1 && handle==u32::MAX && index==u32::MAX
}

pub fn repair_weapon_slots(data:&mut PlayerGameData)->Result<(),String> {
    let slots:Vec<_>=(0..6).filter(|&slot| {
        let e=&data.equipment;
        invalid_empty_weapon(e.chr_asm.equipment_param_ids[slot],unsafe {std::mem::transmute::<_,u32>(e.chr_asm.gaitem_handles[slot])},e.equipment_item_idx_list[slot])
    }).collect();
    if slots.is_empty(){return Ok(());}
    let function=super::items::checked_function(0x249160,&[0x40,0x53,0x55,0x56,0x57,0x41,0x56,0x48,0x81,0xec,0x90,0,0,0])?;
    let fist=ItemId::new(ItemCategory::Weapon,110000).map_err(|e|e.to_string())?;
    if data.equipment.equip_inventory_data.items_data.find_item_idx(fist).is_none() {
        super::items::give(&[(110000,1,-1)])?;
    }
    for slot in slots {equip(data,slot,Some(fist),function)?;}
    Ok(())
}
fn spent_ash(quantity:u32,goods_type:Option<u8>)->bool {
    quantity==0 && matches!(goods_type,Some(7|8))
}
/// 召唤师：战斗胜利时召唤物还活着，这次召唤不消耗骨灰——把用掉的骨灰补回 1 次，之后就不会被清理
pub fn ash_snapshot(data:&PlayerGameData)->Result<Vec<(u32,u32)>,String> {
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"物品参数尚未就绪")?;
    let mut ashes=Vec::new();
    for entry in data.equipment.equip_inventory_data.items_data.normal_entries() {
        let Some(e)=entry.as_option() else{continue};
        let kind=if e.item_id.category()==ItemCategory::Goods {params.get::<eldenring::cs::EquipParamGoods>(e.item_id.param_id()).map(|p|p.goods_type())}else{None};
        if matches!(kind,Some(7|8))&&e.quantity>0 {ashes.push((e.item_id.param_id(),e.quantity));}
    }
    Ok(ashes)
}
fn ash_refund_target(before:&[(u32,u32)],current:&[(u32,u32)],used:Option<u32>)->Option<u32> {
    let id=used?;
    let old=before.iter().find(|(item,_)|*item==id)?.1;
    let now=current.iter().find(|(item,_)|*item==id).map_or(0,|(_,n)|*n);
    (old>0&&now<old).then_some(id)
}
pub fn ash_refund_confirmed(before:&[(u32,u32)],current:&[(u32,u32)],used:Option<u32>)->bool {
    used.is_some_and(|id|before.iter().any(|&(item,n)|item==id&&n>0))
        &&ash_refund_target(before,current,used).is_none()
}
pub fn refund_spent_ashes(data:&mut PlayerGameData,before:&[(u32,u32)],used:Option<u32>)->Result<usize,String> {
    let current=ash_snapshot(data)?;
    let Some(id)=ash_refund_target(before,&current,used) else{return Ok(0);};
    let item=ItemId::new(ItemCategory::Goods,id).map_err(|e|e.to_string())?;
    super::items::give(&[(item.into_inner() as i32,1,-1)])?;
    super::log(format!("Spirit ash refund requested: goods={id} before={before:?} current={current:?}"));
    Ok(1)
}
pub fn clear_spent_ashes(data:&mut PlayerGameData)->Result<(),String> {
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"物品参数尚未就绪")?;
    let inventory=&data.equipment.equip_inventory_data.items_data;
    let spent:Vec<_>=inventory.normal_entries().iter().enumerate().filter_map(|(i,entry)| {
        let e=entry.as_option()?;
        let kind=if e.item_id.category()==ItemCategory::Goods {
            params.get::<eldenring::cs::EquipParamGoods>(e.item_id.param_id()).map(|p|p.goods_type())
        }else{None};
        spent_ash(e.quantity,kind).then_some((inventory.key_items_capacity+i as u32,e.item_id))
    }).collect();
    if spent.is_empty(){return Ok(());}
    let function=super::items::checked_function(0x24d1e0,&[0x48,0x89,0x5c,0x24,0x20,0x89,0x54,0x24,0x10,0x55,0x56,0x57,0x41,0x56,0x41,0x57])?;
    let equipment=&mut data.equipment;
    for (slots,ids) in [(&mut equipment.equip_item_data.quick_slots[..],&mut equipment.equipment_entries.quick_tems[..]),(&mut equipment.equip_item_data.pouch_slots[..],&mut equipment.equipment_entries.pouch[..])] {
        for (slot,id) in slots.iter_mut().zip(ids) {
            if spent.iter().any(|(index,item)|slot.index==*index as i32||id.as_valid()==Some(*item)) {
                slot.index=-1;slot.gaitem_handle=unsafe {std::mem::transmute::<u32,_>(u32::MAX)};
                *id=eldenring::cs::OptionalItemId::NONE;
            }
        }
    }
    let remove:unsafe extern "C" fn(*mut EquipInventoryData,u32,i32)->usize=unsafe {std::mem::transmute(function)};
    for &(index,_) in spent.iter().rev(){unsafe {remove(&mut equipment.equip_inventory_data,index,1);}}
    super::log(format!("Removed {} spent spirit ash entries",spent.len()));
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn ash_refund_is_confirmed_only_after_the_requested_item_returns() {
        let before=[(212001,1),(232000,1)];
        assert!(!super::ash_refund_confirmed(&before,&[(232000,1)],Some(212001)));
        assert!(!super::ash_refund_confirmed(&before,&[(212000,1),(232000,1)],Some(212001)));
        assert!(super::ash_refund_confirmed(&before,&before,Some(212001)));
        assert!(!super::ash_refund_confirmed(&before,&before,None));
    }
    #[test]
    fn refund_tracks_used_ash_even_when_entry_is_removed_or_stack_is_not_empty() {
        let before=[(200000,1),(201000,3)];
        assert_eq!(super::ash_refund_target(&before,&[(201000,3)],Some(200000)),Some(200000));
        assert_eq!(super::ash_refund_target(&before,&[(200000,1),(201000,2)],Some(201000)),Some(201000));
        assert_eq!(super::ash_refund_target(&before,&before,Some(200000)),None);
        assert_eq!(super::ash_refund_target(&before,&[],None),None);
        assert_eq!(super::ash_refund_target(&before,&[],Some(999999)),None);
    }
    use super::*;
    use eldenring::cs::{EquipDataItem,GaitemHandle,OptionalItemId};
    #[test]
    fn weapon_repair_only_targets_fully_unbound_invalid_slots() {
        assert!(invalid_empty_weapon(-1,u32::MAX,u32::MAX));
        for id in [110000,9000000,5000000] {
            assert!(!invalid_empty_weapon(id,u32::MAX,u32::MAX));
        }
        assert!(!invalid_empty_weapon(-1,0x808005d8,u32::MAX));
        assert!(!invalid_empty_weapon(-1,u32::MAX,397));
    }
    #[test]
    fn only_depleted_spirit_ashes_are_removed() {
        for kind in 0..=255 {
            assert_eq!(spent_ash(0,Some(kind)),matches!(kind,7|8));
            assert!(!spent_ash(1,Some(kind)));
        }
        assert!(!spent_ash(0,None));
    }
    #[test]
    fn reset_ash_icon_is_removed_but_equipped_items_are_preserved() {
        let ash=ItemId::new(ItemCategory::Goods,224000).unwrap();
        let slots=[EquipDataItem{gaitem_handle:unsafe {std::mem::transmute::<u32,GaitemHandle>(u32::MAX)},index:-1},EquipDataItem{gaitem_handle:unsafe {std::mem::transmute::<u32,GaitemHandle>(0xb0036b00)},index:400}];
        let mut ids=[OptionalItemId::from(ash),OptionalItemId::from(ash)];
        clear_empty_icons(&slots,&mut ids);
        assert!(!ids[0].is_valid());
        assert_eq!(ids[1].as_valid(),Some(ash));
    }
}

pub(crate) fn equip(data: &mut PlayerGameData, slot: usize, item: Option<ItemId>, function: usize) -> Result<(), String> {
    let (handle, index) = if let Some(item) = item {
        let inventory = &data.equipment.equip_inventory_data.items_data;
        let index = inventory.find_item_idx(item).ok_or("初始装备尚未进入背包")?;
        let handle = inventory.get_entry(item).ok_or("初始装备句柄不存在")?.gaitem_handle;
        (unsafe { std::mem::transmute::<_, u32>(handle) }, index)
    } else { (u32::MAX, u32::MAX) };
    let call: unsafe extern "C" fn(*mut EquipGameData,i32,*const u32,u32,i32,i32,i32)->usize = unsafe {std::mem::transmute(function)};
    unsafe {call(&mut data.equipment,slot as i32,&handle,index,1,1,0);}
    Ok(())
}
fn clear_inventory(inventory: &mut EquipInventoryData, function: usize) -> Result<(), String> {
    let seamless=super::seamless::active();
    let call: unsafe extern "C" fn(*mut EquipInventoryData,u32,i32)->usize = unsafe {std::mem::transmute(function)};
    let cap = inventory.items_data.key_items_capacity;
    let slots: Vec<u32> = inventory.items_data.current_key_entries().iter().enumerate()
        .filter(|(_,e)| e.as_option().is_some_and(|e|!super::seamless::preserve(e.item_id,seamless))).map(|(i,_)|i as u32)
        .chain(inventory.items_data.normal_entries().iter().enumerate().filter(|(_,e)|e.as_option().is_some_and(|e|!super::seamless::preserve(e.item_id,seamless))).map(|(i,_)|cap+i as u32)).collect();
    for slot in slots.into_iter().rev() {unsafe {call(inventory,slot,1);}}
    if inventory.items_data.items().any(|e|!super::seamless::preserve(e.item_id,seamless)) {return Err("背包清理未完成，停止发放初始装备".into());}
    Ok(())
}
/// 职业重做后的专属起手武器（在原版出身装备上调整）：狂战士右手换巨斧、战斧挪到右手第二格；召唤师右手换短剑
fn starting_weapons(class:erdtree_tower::Class,gear:&mut [i32;16]) {
    match class {
        erdtree_tower::Class::Hero=>{gear[3]=gear[1];gear[1]=15000000;},
        erdtree_tower::Class::Confessor=>{gear[1]=2010000;},
        _=>{},
    }
}
pub fn reset(data: &mut PlayerGameData, archetype: u8) -> Result<(), String> {
    if super::class_from_archetype(archetype).is_none() {return Err("无效初始职业".into());}
    let params = unsafe {SoloParamRepository::instance()}.map_err(|_|"初始职业参数未加载")?;
    let p = params.get::<CharaInitParam>(3000+archetype as u32).ok_or("初始职业参数不存在")?;
    let mut gear = [p.equip_wep_left(),p.equip_wep_right(),p.equip_subwep_left(),p.equip_subwep_right(),p.equip_subwep_left3(),p.equip_subwep_right3(),p.equip_arrow(),p.equip_bolt(),p.equip_sub_arrow(),p.equip_sub_bolt(),-1,-1,p.equip_helm(),p.equip_armer(),p.equip_gaunt(),p.equip_leg()];
    starting_weapons(super::class_from_archetype(archetype).unwrap(),&mut gear);
    let spells = [p.equip_spell_01(),p.equip_spell_02(),p.equip_spell_03(),p.equip_spell_04()];
    let equip_fn=super::items::checked_function(0x249160,&[0x40,0x53,0x55,0x56,0x57,0x41,0x56,0x48,0x81,0xec,0x90,0,0,0])?;
    let remove_fn=super::items::checked_function(0x24d1e0,&[0x48,0x89,0x5c,0x24,0x20,0x89,0x54,0x24,0x10,0x55,0x56,0x57,0x41,0x56,0x41,0x57])?;
    super::items::checked_function(0x5618b0,&[0x40,0x55,0x56,0x57,0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57])?;
    // Unbind engine equipment references before removing their inventory instances.
    for slot in 0..22 {equip(data,slot,None,equip_fn)?;}
    data.equipment.chr_asm.equipment.arm_style=eldenring::cs::ChrAsmArmStyle::OneHanded;
    let selected=&mut data.equipment.chr_asm.equipment.selected_slots;
    selected.left_weapon_slot=0;selected.right_weapon_slot=0;
    selected.left_arrow_slot=0;selected.right_arrow_slot=0;selected.left_bolt_slot=0;selected.right_bolt_slot=0;
    for slot in data.equipment.equip_item_data.quick_slots.iter_mut().chain(data.equipment.equip_item_data.pouch_slots.iter_mut()) {
        slot.index=-1; slot.gaitem_handle=unsafe {std::mem::transmute::<u32,_>(u32::MAX)};
    }
    repair_item_icons(data);
    data.equipment.physick_tears=[eldenring::cs::OptionalItemId::NONE;2];
    data.equipment.extra_physick_tear=eldenring::cs::OptionalItemId::NONE;
    for spell in &mut data.equipment.equip_magic_data.entries {spell.param_id=-1;spell.charges=0;}
    clear_inventory(&mut data.equipment.equip_inventory_data,remove_fn)?;
    if let Some(storage)=data.storage.as_mut() {clear_inventory(storage,remove_fn)?;}
    data.archetype=archetype;
    data.vigor=(p.base_vit() as u32+10).min(99);data.mind=p.base_wil() as u32;data.endurance=p.base_end() as u32;
    data.strength=p.base_str() as u32;data.dexterity=p.base_dex() as u32;data.intelligence=p.base_mag() as u32;
    data.faith=p.base_fai() as u32;data.arcane=p.base_luc() as u32;data.level=p.soul_lv() as u32;
    data.rune_count=0;data.rune_memory=0;data.matching_weapon_level=0;data.rune_arc_active=false;
    data.starting_gift=0;data.scadutree_blessing=0;data.reversed_spirit_ash=0;
    data.equipment.equip_magic_data.selected_slot=0;
    data.hp_estus_additional=0;data.fp_estus_additional=0;data.unlocked_talisman_slots=4;data.unlocked_magic_slots=2;
    for (slot,&id) in gear.iter().enumerate().filter(|(_,id)|**id>=0) {
        let category=if slot>=12 {ItemCategory::Protector} else {ItemCategory::Weapon};
        let full=if slot>=12 {id|0x10000000} else {id};
        let quantity=match slot {6=>i32::from(p.arrow_num()).max(1),7=>i32::from(p.bolt_num()).max(1),8=>i32::from(p.sub_arrow_num()).max(1),9=>i32::from(p.sub_bolt_num()).max(1),_=>1};
        super::random_ashes::give(&[(full,quantity,-1)])?;
        equip(data,slot,Some(ItemId::new(category,id as u32).map_err(|e|e.to_string())?),equip_fn)?;
    }
    repair_weapon_slots(data)?;
    for (slot,&id) in spells.iter().enumerate().filter(|(_,id)|**id>=0) {
        super::items::give(&[(id|0x40000000,1,-1)])?;
        data.equipment.equip_magic_data.entries[slot].param_id=id;
    }
    for (id,qty) in [(p.item_01(),p.item_num_01()),(p.item_02(),p.item_num_02()),(p.item_03(),p.item_num_03()),(p.item_04(),p.item_num_04()),(p.item_05(),p.item_num_05()),(p.item_06(),p.item_num_06()),(p.item_07(),p.item_num_07()),(p.item_08(),p.item_num_08()),(p.item_09(),p.item_num_09()),(p.item_10(),p.item_num_10()),(p.secondary_item_01(),p.secondary_item_num_01()),(p.secondary_item_02(),p.secondary_item_num_02()),(p.secondary_item_03(),p.secondary_item_num_03()),(p.secondary_item_04(),p.secondary_item_num_04()),(p.secondary_item_05(),p.secondary_item_num_05()),(p.secondary_item_06(),p.secondary_item_num_06())] {
        if id>=0 && qty>0 {super::items::give(&[(id|0x40000000,qty as i32,-1)])?;}
    }
    super::items::give_flasks(true,true)?;
    super::physick::ensure(data)?;
    let tear=erdtree_tower::physick_supply::STARTING_TEAR;
    super::items::give(&[(tear as i32,1,-1)])?;
    super::items::give(&[(0x40000000|9500,5,-1)])?;
    data.equipment.physick_tears[0]=ItemId::new(ItemCategory::Goods,tear&0x0fffffff).map_err(|e|e.to_string())?.into();
    super::items::give(&[(0x40000000|8590,1,-1)])?;
    // 开局护符（每个职业有自己的默认护符，职业传承可以更换）在大赐福开局时发放
    super::items::give(&[(0x40000000|8158,1,-1)])?;
    super::passives::give_starting(data,super::class_from_archetype(archetype).unwrap())?;
    super::log(format!("Initial class restored: archetype={archetype}, level={}",data.level));
    Ok(())
}
