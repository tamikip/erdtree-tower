use eldenring::cs::{
    CSGaitemImp, CSWepGaitemIns, EquipGameData, EquipInventoryData, EquipParamWeapon,
    GaitemHandle, ItemCategory, ItemId, PlayerGameData, ReinforceParamWeapon, SoloParamRepository,
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
    let max = super::smith::level_max(params, base.reinforce_type_id()) as u32;
    let minimum = next_id(old as u32, cap, max, base.origin_equip_wep1() >= 0)?;
    let next = super::smith::target_with_peak(old,cap as i32,max as i32,super::smith::effective_peak(data,params))
        .map(|id|id as u32).unwrap_or(minimum);
    let reinforcement = i32::from(base.reinforce_type_id()) + (next % 100) as i32;
    if reinforcement < 0
        || params
            .get::<ReinforceParamWeapon>(reinforcement as u32)
            .is_none()
    {
        return Err("下一级强化参数不存在，未扣除资源".into());
    }
    swap_right(data, slot, old, next, "upgraded")
}
/// 词条重铸：右手的词条武器换成同一件武器的另一种词条组合，强化等级与战灰保留
pub fn reforge_target(data: &PlayerGameData, run: &erdtree_tower::Run, node: usize, guaranteed: bool) -> Result<u32, String> {
    let slot = data.equipment.chr_asm.equipment.active_right_weapon_slot() as usize;
    let old = data.equipment.chr_asm.equipment_param_ids[slot];
    if old < 0 {return Err("请在右手装备词条武器".into());}
    let (variant, level) = (old as u32 / 100 * 100, old as u32 % 100);
    if erdtree_tower::affixes::tier(variant).is_none() {return Err("右手武器没有词条，未扣除卢恩".into());}
    let target = erdtree_tower::affixes::reforge(run, node, variant, guaranteed).ok_or(if guaranteed {"传说品质的武器不能精炼，未扣除卢恩"} else {"这把武器没有其他词条组合，未扣除卢恩"})?;
    let params = unsafe { SoloParamRepository::instance() }.map_err(|_| "武器参数未就绪")?;
    let row = params.get::<EquipParamWeapon>(target).ok_or("重铸后的武器参数不存在")?;
    let next = target + level.min(super::smith::level_max(params, row.reinforce_type_id()) as u32);
    let reinforcement = i32::from(row.reinforce_type_id()) + (next % 100) as i32;
    if reinforcement < 0 || params.get::<ReinforceParamWeapon>(reinforcement as u32).is_none() {
        return Err("重铸后的强化参数不存在，未扣除卢恩".into());
    }
    Ok(next)
}
/// 重练炉：右手的词条武器换成同品质的另一把武器（强化等级按新武器的上限保留）；uses 是这座炉子此前重练的次数
pub fn furnace_target(data: &PlayerGameData, run: &erdtree_tower::Run, node: usize, uses: u8) -> Result<u32, String> {
    let slot = data.equipment.chr_asm.equipment.active_right_weapon_slot() as usize;
    let old = data.equipment.chr_asm.equipment_param_ids[slot];
    if old < 0 {return Err("请在右手装备词条武器".into());}
    let (variant, level) = (old as u32 / 100 * 100, old as u32 % 100);
    if erdtree_tower::affixes::tier(variant).is_none() {return Err("右手武器没有词条，未扣除卢恩".into());}
    let target = erdtree_tower::affixes::furnace(run, node, variant, uses).ok_or("没有可以换的同品质武器，未扣除卢恩")?;
    let params = unsafe { SoloParamRepository::instance() }.map_err(|_| "武器参数未就绪")?;
    let row = params.get::<EquipParamWeapon>(target).ok_or("重练后的武器参数不存在")?;
    // 普通与失色武器的满级不同：按等效等级换算（普通 +25 ≈ 失色 +10）
    let somber_old = params.get::<EquipParamWeapon>(variant).is_some_and(|p| p.origin_equip_wep16() < 0);
    let somber_new = row.origin_equip_wep16() < 0;
    let level = match (somber_old, somber_new) {(false, true) => level * 2 / 5, (true, false) => level * 5 / 2, _ => level};
    let next = target + level.min(super::smith::level_max(params, row.reinforce_type_id()) as u32);
    let reinforcement = i32::from(row.reinforce_type_id()) + (next % 100) as i32;
    if reinforcement < 0 || params.get::<ReinforceParamWeapon>(reinforcement as u32).is_none() {
        return Err("重练后的强化参数不存在，未扣除卢恩".into());
    }
    Ok(next)
}
/// 战斗法师：S 补正的门槛（原版补正字母 S ≥ 175）
pub const S_SCALING: f32 = 175.0;
/// 近战武器的种类（武器编号的百万位）：剑、刀、斧、槌、矛、拳爪鞭等；盾、法杖、圣印记、弓弩、火把、调香瓶、突刺盾不算
pub fn melee_kind(kind: u32) -> bool {matches!(kind, 1..=23 | 60 | 63 | 64 | 66 | 67 | 68)}
/// 战斗法师：右手武器能不能附加 S 补正，能的话返回它的武器参数行（等级去掉）。converted：这局已转换过的武器（换另一种补正时不算“已有 S”）
pub fn battlemage_target(data: &PlayerGameData, converted: &[(u32, u8)]) -> Result<u32, String> {
    let slot = data.equipment.chr_asm.equipment.active_right_weapon_slot() as usize;
    let old = data.equipment.chr_asm.equipment_param_ids[slot];
    if old < 0 {return Err("请在右手装备近战武器".into());}
    let (row_id, level) = (old as u32 / 100 * 100, old as u32 % 100);
    if !melee_kind((erdtree_tower::affixes::base(row_id) & 0x0fffffff) / 1_000_000) {return Err("只有近战武器能附加补正（盾、法杖、圣印记、弓弩不行）".into());}
    if converted.iter().any(|(row, _)| *row == row_id) {return Ok(row_id);}
    let params = unsafe { SoloParamRepository::instance() }.map_err(|_| "武器参数未就绪")?;
    let row = params.get::<EquipParamWeapon>(row_id).ok_or("武器参数不存在")?;
    let reinforce = params.get::<ReinforceParamWeapon>((i32::from(row.reinforce_type_id()) + level as i32) as u32).ok_or("强化参数不存在")?;
    let scaled = [
        row.correct_strength() * reinforce.correct_strength_rate(), row.correct_agility() * reinforce.correct_agility_rate(),
        row.correct_magic() * reinforce.correct_magic_rate(), row.correct_faith() * reinforce.correct_faith_rate(), row.correct_luck() * reinforce.correct_luck_rate(),
    ];
    if scaled.iter().any(|v| *v >= S_SCALING) {return Err("这把武器已经有 S 补正".into());}
    Ok(row_id)
}
pub fn reforge(data: &mut PlayerGameData, next: u32) -> Result<(), String> {
    let slot = data.equipment.chr_asm.equipment.active_right_weapon_slot() as usize;
    let old = data.equipment.chr_asm.equipment_param_ids[slot];
    swap_right(data, slot, old, next, "reforged")
}
/// 把右手武器换成编号为 next 的副本（保留战灰），自动装备后移除原武器
fn swap_right(data: &mut PlayerGameData, slot: usize, old: i32, next: u32, verb: &str) -> Result<(), String> {
    let function = super::items::checked_function(
        0x249160,
        &[
            0x40, 0x53, 0x55, 0x56, 0x57, 0x41, 0x56, 0x48, 0x81, 0xec, 0x90, 0, 0, 0,
        ],
    )?;
    let remove_fn = super::items::checked_function(0x24d1e0, &[0x48, 0x89, 0x5c, 0x24, 0x20, 0x89, 0x54, 0x24, 0x10, 0x55, 0x56, 0x57, 0x41, 0x56, 0x41, 0x57])?;
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
        "Weapon {verb} copy: {old} -> {next}, slot={slot}, gem={gem}"
    ));
    remove_original(data, handle, gem_handle, remove_fn);
    Ok(())
}

fn remove_original(data: &mut PlayerGameData, weapon: GaitemHandle, gem: GaitemHandle, function: usize) {
    if data.equipment.chr_asm.gaitem_handles.contains(&weapon) {
        return super::log("Weapon upgrade kept original: still equipped in another slot");
    }
    let remove: unsafe extern "C" fn(*mut EquipInventoryData, u32, i32) -> usize = unsafe { std::mem::transmute(function) };
    let inventory = &mut data.equipment.equip_inventory_data;
    let items = &inventory.items_data;
    let mut slots: Vec<u32> = items.normal_entries().iter().enumerate()
        .filter_map(|(i, e)| e.as_option().filter(|e| e.gaitem_handle == weapon || gem.is_indexed() && e.gaitem_handle == gem).map(|_| items.key_items_capacity + i as u32))
        .collect();
    slots.sort_unstable();
    for &slot in slots.iter().rev() {
        unsafe { remove(inventory, slot, 1); }
    }
    super::log(format!("Weapon upgrade removed original: entries={}", slots.len()));
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

/// cap：原版满级（普通 25、失色 10，决定步长）；max：实际能到的最高等级
fn next_id(id: u32, cap: u32, max: u32, upgradeable: bool) -> Result<u32, String> {
    if !upgradeable || id % 100 >= max {
        Err("当前武器无法继续强化，未扣除资源".into())
    } else {
        super::smith::reinforcement_target(id as i32,cap as i32,max as i32).map(|id|id as u32).ok_or("当前武器无法继续强化，未扣除资源".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reinforcement_preserves_affinity_and_checks_normal_and_somber_caps() {
        assert_eq!(next_id(901200, 25, 99, true).unwrap(), 901203);
        assert_eq!(next_id(901224, 25, 99, true).unwrap(), 901227);
        assert_eq!(next_id(901225, 25, 99, true).unwrap(), 901228);
        assert_eq!(next_id(901209, 10, 39, true).unwrap(), 901210);
        assert_eq!(next_id(901210, 10, 39, true).unwrap(), 901211);
        assert!(next_id(901299, 25, 99, true).is_err());
        assert!(next_id(901239, 10, 39, true).is_err());
        assert!(next_id(901225, 25, 25, true).is_err());
        assert!(next_id(110000, 25, 99, false).is_err());
    }
}
#[cfg(test)]
mod battlemage_tests {
    #[test]
    fn only_melee_weapons_take_battlemage_scaling() {
        for kind in [1,2,3,4,9,13,14,16,23,60,64,66,67,68] {assert!(super::melee_kind(kind),"{kind}");}
        for kind in [24,30,31,32,33,34,40,41,42,43,44,61,62] {assert!(!super::melee_kind(kind),"{kind}");}
    }
}
