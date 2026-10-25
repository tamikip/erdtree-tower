use eldenring::cs::{EquipDataItem,GaitemCategory,ItemCategory,ItemId,PlayerGameData};

fn bottle(id:ItemId)->bool {id.category()==ItemCategory::Goods&&matches!(id.param_id(),250|251)}
pub fn empty(data:&PlayerGameData)->bool {
    data.equipment.equip_inventory_data.items_data.items().any(|e|bottle(e.item_id)&&e.item_id.param_id()==251&&e.quantity>0)
        &&!data.equipment.equip_inventory_data.items_data.items().any(|e|bottle(e.item_id)&&e.item_id.param_id()==250&&e.quantity>0)
}
pub fn ensure(data:&mut PlayerGameData)->Result<(),String> {
    if !data.equipment.equip_inventory_data.items_data.items().any(|e|bottle(e.item_id)&&e.quantity>0) {
        super::items::give(&[(0x40000000|251,1,-1)])?;
        if !empty(data) {return Err("灵药瓶尚未进入背包".into());}
    }
    Ok(())
}
fn replace_slots(slots:&mut [EquipDataItem],target:&EquipDataItem) {
    for slot in slots {
        let h=slot.gaitem_handle;
        if matches!(h.category(),Ok(GaitemCategory::Goods))&&!h.is_indexed()&&matches!(h.selector(),250|251) {
            slot.index=target.index;slot.gaitem_handle=target.gaitem_handle;
        }
    }
}
pub fn refill(data:&mut PlayerGameData)->Result<(),String> {
    if !empty(data) {return Ok(());}
    let remove_fn=super::items::checked_function(0x24d1e0,&[0x48,0x89,0x5c,0x24,0x20,0x89,0x54,0x24,0x10,0x55,0x56,0x57,0x41,0x56,0x41,0x57])?;
    super::items::give(&[(0x40000000|250,1,-1)])?;
    let full=ItemId::new(ItemCategory::Goods,250).unwrap();
    if !data.equipment.equip_inventory_data.items_data.items().any(|e|e.item_id==full&&e.quantity>0) {return Err("灵药尚未补充，请清理背包后重试".into());}
    let inventory=&mut data.equipment.equip_inventory_data;
    let items=&inventory.items_data;
    let obsolete:Vec<_>=items.normal_entries().iter().enumerate().filter_map(|(i,e)|e.as_option().filter(|e|bottle(e.item_id)&&e.item_id.param_id()==251).map(|_|items.key_items_capacity+i as u32)).collect();
    let remove:unsafe extern "C" fn(*mut eldenring::cs::EquipInventoryData,u32,i32)->usize=unsafe {std::mem::transmute(remove_fn)};
    for slot in obsolete.into_iter().rev(){unsafe{remove(inventory,slot,1);}}
    let items=&inventory.items_data;
    let entry=items.get_entry(full).ok_or("灵药条目未就绪")?;
    let target=EquipDataItem{gaitem_handle:entry.gaitem_handle,index:items.find_item_idx(full).ok_or("灵药索引未就绪")? as i32};
    replace_slots(&mut data.equipment.equip_item_data.quick_slots,&target);
    replace_slots(&mut data.equipment.equip_item_data.pouch_slots,&target);
    for id in data.equipment.equipment_entries.quick_tems.iter_mut().chain(data.equipment.equipment_entries.pouch.iter_mut()) {
        if id.as_valid().is_some_and(bottle){*id=full.into();}
    }
    Ok(())
}

#[test]
fn only_physick_equipment_references_are_replaced() {
    use eldenring::cs::GaitemHandle;
    let slot=|id:u32|EquipDataItem{gaitem_handle:unsafe {std::mem::transmute::<u32,GaitemHandle>(0xb0000000|id)},index:42};
    let mut slots=[slot(251),slot(1001),slot(250)];let target=slot(250);
    replace_slots(&mut slots,&target);
    assert_eq!(slots[0].gaitem_handle.selector(),250);
    assert_eq!(slots[1].gaitem_handle.selector(),1001);
    assert_eq!(slots[2].gaitem_handle.selector(),250);
}
