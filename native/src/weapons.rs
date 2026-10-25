use eldenring::cs::{
    CSGaitemImp, CSWepGaitemIns, EquipGameData, EquipParamWeapon, GaitemHandle, ItemCategory,
    ItemId, PlayerGameData, ReinforceParamWeapon, SoloParamRepository,
};
use fromsoftware_shared::{FromStatic, Superclass};

pub fn upgrade(data: &mut PlayerGameData) -> Result<(), String> {
    let slot = data.equipment.chr_asm.equipment.active_right_weapon_slot() as usize;
    let old = data.equipment.chr_asm.equipment_param_ids[slot];
    if old < 0 {
        return Err("请在右手装备可强化武器".into());
    }
    let params = unsafe { SoloParamRepository::instance() }.map_err(|_| "武器参数未就绪")?;
    let base = params
        .get::<EquipParamWeapon>((old as u32 / 100) * 100)
        .ok_or("武器参数不存在")?;
    let cap = if base.origin_equip_wep16() < 0 {
        10
    } else {
        25
    };
    let minimum = next_id(old as u32, cap, base.origin_equip_wep1() >= 0)?;
    let next = super::smith::target_with_peak(old,cap as i32,super::smith::inventory_peak(data,params))
        .map(|id|id as u32).unwrap_or(minimum);
    let reinforcement = i32::from(base.reinforce_type_id()) + (next % 100) as i32;
    if reinforcement < 0
        || params
            .get::<ReinforceParamWeapon>(reinforcement as u32)
            .is_none()
    {
        return Err("下一级强化参数不存在，未扣除资源".into());
    }
    let function = super::items::checked_function(
        0x249160,
        &[
            0x40, 0x53, 0x55, 0x56, 0x57, 0x41, 0x56, 0x48, 0x81, 0xec, 0x90, 0, 0, 0,
        ],
    )?;
    let manager = unsafe { CSGaitemImp::instance() }.map_err(|_| "武器实例未就绪")?;
    let handle = data.equipment.chr_asm.gaitem_handles[slot];
    if handle.index() as usize >= manager.gaitems.len() {
        return Err("武器句柄无效".into());
    }
    let weapon = manager
        .gaitem_ins_by_handle(&handle)
        .and_then(|w| w.as_subclass::<CSWepGaitemIns>())
        .ok_or("请装备可强化武器")?;
    let gem_handle = weapon.gem_slot_table.gem_slots[0].gaitem_handle;
    let gem = if gem_handle.is_indexed() && (gem_handle.index() as usize) < manager.gaitems.len() {
        manager
            .gaitem_ins_by_handle(&gem_handle)
            .and_then(|g| g.item_id.param_id())
            .map_or(-1, |id| id as i32)
    } else {
        -1
    };
    let id = ItemId::new(ItemCategory::Weapon, next).map_err(|e| e.to_string())?;
    if matching_copy(data, id, gem).is_none() {
        if data
            .equipment
            .equip_inventory_data
            .items_data
            .is_normal_items_full()
        {
            return Err("背包已满，未强化或扣款".into());
        }
        super::items::give(&[(next as i32, 1, gem)])?;
    }
    let index = matching_copy(data, id, gem).ok_or("强化副本尚未进入背包，请重试；未扣款")?;
    let equipped_handle = data
        .equipment
        .equip_inventory_data
        .items_data
        .entry_at_slot(index)
        .and_then(|entry| entry.as_option())
        .ok_or("强化副本槽位未就绪")?
        .gaitem_handle;
    let equip: unsafe extern "C" fn(
        *mut EquipGameData,
        i32,
        *const GaitemHandle,
        u32,
        i32,
        i32,
        i32,
    ) -> usize = unsafe { std::mem::transmute(function) };
    unsafe {
        equip(
            &mut data.equipment,
            slot as i32,
            &equipped_handle,
            index,
            1,
            1,
            0,
        );
    }
    if data.equipment.chr_asm.equipment_param_ids[slot] != next as i32
        || data.equipment.chr_asm.gaitem_handles[slot] != equipped_handle
    {
        return Err("强化副本已保留在背包，自动装备未成功；未扣款，请重试".into());
    }
    super::log(format!(
        "Weapon upgraded copy: {old} -> {next}, slot={slot}, gem={gem}"
    ));
    Ok(())
}

fn matching_copy(data: &PlayerGameData, id: ItemId, gem: i32) -> Option<u32> {
    let manager = unsafe { CSGaitemImp::instance() }.ok()?;
    let items = &data.equipment.equip_inventory_data.items_data;
    items
        .normal_entries()
        .iter()
        .enumerate()
        .find_map(|(index, entry)| {
            let entry = entry.as_option()?;
            if entry.item_id != id || entry.gaitem_handle.index() as usize >= manager.gaitems.len()
            {
                return None;
            }
            let weapon = manager
                .gaitem_ins_by_handle(&entry.gaitem_handle)?
                .as_subclass::<CSWepGaitemIns>()?;
            let handle = weapon.gem_slot_table.gem_slots[0].gaitem_handle;
            let actual = if handle.is_indexed() && (handle.index() as usize) < manager.gaitems.len()
            {
                manager
                    .gaitem_ins_by_handle(&handle)
                    .and_then(|g| g.item_id.param_id())
                    .map_or(-1, |id| id as i32)
            } else {
                -1
            };
            (actual == gem).then_some(items.key_items_capacity + index as u32)
        })
}

fn next_id(id: u32, cap: u32, upgradeable: bool) -> Result<u32, String> {
    if !upgradeable || id % 100 >= cap {
        Err("当前武器无法继续强化，未扣除资源".into())
    } else {
        super::smith::reinforcement_target(id as i32,cap as i32).map(|id|id as u32).ok_or("当前武器无法继续强化，未扣除资源".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reinforcement_preserves_affinity_and_checks_normal_and_somber_caps() {
        assert_eq!(next_id(901200, 25, true).unwrap(), 901202);
        assert_eq!(next_id(901224, 25, true).unwrap(), 901225);
        assert_eq!(next_id(901209, 10, true).unwrap(), 901210);
        assert!(next_id(901225, 25, true).is_err());
        assert!(next_id(901210, 10, true).is_err());
        assert!(next_id(110000, 25, false).is_err());
    }
}
