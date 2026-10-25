use eldenring::cs::{
    EquipGameData, EquipInventoryData, EquipInventoryDataListEntry, InventoryItemsData,
    PlayerGameData,
};
fn main() {
    println!("item_data={} quick_ids={} pouch_ids={} selected={}",std::mem::offset_of!(EquipGameData,equip_item_data),std::mem::offset_of!(eldenring::cs::ChrAsmEquipEntries,quick_tems),std::mem::offset_of!(eldenring::cs::ChrAsmEquipEntries,pouch),std::mem::offset_of!(eldenring::cs::EquipItemData,selected_quick_slot));
    println!("asm={} entries={}", std::mem::offset_of!(EquipGameData,chr_asm),std::mem::offset_of!(EquipGameData,equipment_entries));
    println!(
        "equipment={} inventory={} items={} normal_head={} normal_len={} key_head={} key_len={} entry_size={}",
        std::mem::offset_of!(PlayerGameData, equipment),
        std::mem::offset_of!(EquipGameData, equip_inventory_data),
        std::mem::offset_of!(EquipInventoryData, items_data),
        std::mem::offset_of!(InventoryItemsData, normal_items_head),
        std::mem::offset_of!(InventoryItemsData, normal_items_len),
        std::mem::offset_of!(InventoryItemsData, key_items_head),
        std::mem::offset_of!(InventoryItemsData, key_items_len),
        std::mem::size_of::<EquipInventoryDataListEntry>()
    );
}
