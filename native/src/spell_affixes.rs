use std::{collections::HashMap,sync::Mutex,time::{Duration,Instant}};
use eldenring::cs::{ItemCategory,PlayerGameData};
use erdtree_tower::spell_affixes as rules;

static PENDING:Mutex<Vec<(u32,Instant)>>=Mutex::new(Vec::new());
static CHECKED:Mutex<Option<Instant>>=Mutex::new(None);
pub fn granted(entries:&[(i32,i32,i32)]) {
    let Ok(mut pending)=PENDING.lock()else{return;};
    for &(id,_,_) in entries {
        let id=id as u32;if rules::variant(id).is_none(){continue;}
        pending.retain(|(old,_)|rules::base(*old)!=rules::base(id));pending.push((id,Instant::now()));
    }
}
fn choose(ids:impl IntoIterator<Item=u32>,requested:&[u32])->HashMap<u32,u32> {
    let mut chosen=HashMap::new();
    for id in ids {
        let Some(v)=rules::variant(id)else{continue;};
        let old=chosen.entry(v.base).or_insert(id);
        if (v.tier,id)>(rules::variant(*old).unwrap().tier,*old){*old=id;}
    }
    for &id in requested {if let Some(v)=rules::variant(id){chosen.insert(v.base,id);}}
    chosen
}
pub fn sync(data:&mut PlayerGameData)->Result<(),String> {
    let mut checked=CHECKED.lock().map_err(|_|"法术词缀同步锁不可用")?;
    if checked.is_some_and(|at|at.elapsed()<Duration::from_millis(500)){return Ok(());}
    *checked=Some(Instant::now());
    let mut pending=PENDING.lock().map_err(|_|"法术词缀领取锁不可用")?;
    pending.retain(|(_,at)|at.elapsed()<Duration::from_secs(10));
    let held:Vec<_>=data.equipment.equip_inventory_data.items_data.items()
        .filter(|e|e.quantity>0&&e.item_id.category()==ItemCategory::Goods).map(|e|0x40000000|e.item_id.param_id()).collect();
    let ready:Vec<_>=pending.iter().map(|(id,_)|*id).filter(|id|held.contains(id)).collect();
    let chosen=choose(held.iter().copied(),&ready);
    if chosen.is_empty(){return Ok(());}
    let function=super::items::checked_function(0x24d1e0,&[0x48,0x89,0x5c,0x24,0x20,0x89,0x54,0x24,0x10,0x55,0x56,0x57,0x41,0x56,0x41,0x57])?;
    let remove:unsafe extern "C" fn(*mut eldenring::cs::EquipInventoryData,u32,i32)->usize=unsafe{std::mem::transmute(function)};
    for inventory in std::iter::once(&mut data.equipment.equip_inventory_data).chain(data.storage.as_mut().map(|s|&mut **s)) {
        let items=&inventory.items_data;
        let slots:Vec<_>=items.current_key_entries().iter().enumerate().map(|(i,e)|(i as u32,e))
            .chain(items.normal_entries().iter().enumerate().map(|(i,e)|(items.key_items_capacity+i as u32,e)))
            .filter_map(|(i,e)|e.as_option().filter(|e|e.item_id.category()==ItemCategory::Goods&&e.quantity>0).and_then(|e|{
                let id=0x40000000|e.item_id.param_id();
                chosen.get(&rules::base(id)).filter(|&&target|target!=id).map(|_|(i,e.quantity as i32,id))
            })).collect();
        for (slot,quantity,id) in slots.into_iter().rev(){unsafe{remove(inventory,slot,quantity);}
            super::log(format!("SPELL_AFFIX replaced old={id} target={} quantity={quantity}",chosen[&rules::base(id)]));}
    }
    let mut seen=std::collections::HashSet::new();
    for entry in &mut data.equipment.equip_magic_data.entries {
        if entry.param_id<0{continue;}
        let base=rules::base(0x40000000|entry.param_id as u32);
        if let Some(&id)=chosen.get(&base) {
            if seen.insert(base){entry.param_id=(id&0x0fffffff) as i32;}else{entry.param_id=-1;entry.charges=0;}
        }
    }
    pending.retain(|(id,_)|!ready.contains(id));Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_keeps_one_family_and_honors_selected_affix() {
        let base=0x40000000|4000;
        let variants:Vec<_>=rules::VARIANTS.iter().filter(|v|v.base==base).collect();
        let low=variants.iter().find(|v|v.tier==0).unwrap().item.id;
        let high=variants.iter().find(|v|v.tier==3).unwrap().item.id;
        assert_eq!(choose([base,low,high],&[])[&base],high);
        assert_eq!(choose([base,low,high],&[low])[&base],low);
        assert!(choose([base],&[]).is_empty());
    }
}
