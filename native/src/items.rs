use eldenring::cs::{EquipParamGoods, MapItemMan, SoloParamRepository};
use fromsoftware_shared::FromStatic;
use std::ffi::c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
}

pub fn give_flasks(red: bool, blue: bool) -> Result<(), String> {
    let params = unsafe { SoloParamRepository::instance() }.map_err(|_| "物品参数未就绪")?;
    if params.get::<EquipParamGoods>(1000).is_none()
        || params.get::<EquipParamGoods>(1050).is_none()
    {
        return Err("未找到本体圣杯瓶参数".into());
    }
    let entries: Vec<_> = [(red, 0x400003e8), (blue, 0x4000041a)]
        .into_iter()
        .filter(|e| e.0)
        .map(|(_, id)| (id, 1, -1))
        .collect();
    give(&entries)
}

pub fn checked_function(rva: usize, expected: &[u8]) -> Result<usize, String> {
    let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
    if base == 0 {
        return Err("游戏模块未就绪".into());
    }
    let function = base + rva;
    if unsafe { std::slice::from_raw_parts(function as *const u8, expected.len()) } != expected {
        return Err("游戏物品接口版本不匹配，未修改物品".into());
    }
    Ok(function)
}

pub fn give(entries: &[(i32, i32, i32)]) -> Result<(), String> {
    if entries.is_empty() {
        return Ok(());
    }
    if entries.len() > 2 || entries.iter().any(|(_, qty, _)| *qty <= 0) {
        return Err("无效物品请求".into());
    }
    let manager = unsafe { MapItemMan::instance_mut() }.map_err(|_| "物品系统未就绪")?;
    // Verified against the launcher's SHA-256-pinned 2.7.2.0 executable and CT ItemGive ABI.
    let expected = [
        0x40, 0x55, 0x56, 0x57, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57,
    ];
    let function = checked_function(0x5618b0, &expected)?;
    let mut items = [0i32; 9];
    for &(id, qty, gem) in entries {
        let offset = 1 + items[0] as usize * 4;
        items[offset..offset + 4].copy_from_slice(&[id, qty, -1, gem]);
        items[0] += 1;
    }
    let mut output = [0u32; 8];
    let give: unsafe extern "C" fn(*mut MapItemMan, *const i32, *mut u32, i32) =
        unsafe { std::mem::transmute(function) };
    unsafe {
        give(manager, items.as_ptr(), output.as_mut_ptr(), 0);
    }
    super::log(format!("Item grant requested: {entries:?}"));
    Ok(())
}

fn reward_item(id:u32)->Result<(eldenring::cs::ItemId,&'static str),String> {
    use eldenring::cs::{ItemCategory,ItemId};
    let reward=erdtree_tower::rewards::lookup(id).ok_or("未知奖励物品")?;
    let category=match id>>28 {0=>ItemCategory::Weapon,1=>ItemCategory::Protector,2=>ItemCategory::Accessory,4=>ItemCategory::Goods,_=>return Err("无效奖励类型".into())};
    let item=ItemId::new(category,id&0x0fffffff).map_err(|e|e.to_string())?;
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"奖励物品参数未就绪")?;
    if params.get_equip_param(item).is_none(){return Err("奖励物品参数不存在".into());}
    Ok((item,reward.name))
}
fn held(data:&eldenring::cs::PlayerGameData,item:eldenring::cs::ItemId)->u64 {
    let bag=data.equipment.equip_inventory_data.items_data.items().filter(|e|e.item_id==item).map(|e|e.quantity as u64).sum::<u64>();
    bag+data.storage.as_ref().map_or(0,|s|s.items_data.items().filter(|e|e.item_id==item).map(|e|e.quantity as u64).sum::<u64>())
}
pub fn check_reward(data:&eldenring::cs::PlayerGameData,id:u32)->Result<u64,String> {
    let (item,name)=reward_item(id)?;let before=held(data,item);
    if item.category()==eldenring::cs::ItemCategory::Goods&&before>0 {return Err(format!("已拥有{name}，请选择其他奖励"));}
    Ok(before)
}
pub fn give_reward(data:&mut eldenring::cs::PlayerGameData,id:u32)->Result<(),String> {
    let before=check_reward(data,id)?;
    give(&[(id as i32,1,-1)])?;
    if held(data,reward_item(id)?.0)<=before {return Err("奖励未进入背包或仓库，请检查容量后重试".into());}
    Ok(())
}

pub fn give_stack(data:&mut eldenring::cs::PlayerGameData,id:u32)->Result<(),String> {
    let count=|data:&eldenring::cs::PlayerGameData| {
        data.equipment.equip_inventory_data.items_data.items().filter(|e|e.item_id.category()==eldenring::cs::ItemCategory::Goods&&e.item_id.param_id()==id).map(|e|u64::from(e.quantity)).sum::<u64>()
        +data.storage.as_ref().map_or(0,|s|s.items_data.items().filter(|e|e.item_id.category()==eldenring::cs::ItemCategory::Goods&&e.item_id.param_id()==id).map(|e|u64::from(e.quantity)).sum::<u64>())
    };
    let before=count(data);give(&[((0x40000000|id) as i32,1,-1)])?;
    if count(data)<=before{return Err("材料未进入背包或仓库，请清理容量后重试".into());}Ok(())
}
