use eldenring::cs::{ItemCategory,PlayerGameData};
pub const TEAR:i32=8185;
pub fn count(data:&PlayerGameData)->u64 {
    data.equipment.equip_inventory_data.items_data.items().filter(|e|e.item_id.category()==ItemCategory::Goods&&e.item_id.param_id()==TEAR as u32).map(|e|u64::from(e.quantity)).sum::<u64>()
        +data.storage.as_ref().map_or(0,|s|s.items_data.items().filter(|e|e.item_id.category()==ItemCategory::Goods&&e.item_id.param_id()==TEAR as u32).map(|e|u64::from(e.quantity)).sum::<u64>())
}
