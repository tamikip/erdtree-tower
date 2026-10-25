use eldenring::cs::{EquipDataItem, GaitemHandle, GaitemCategory, ItemCategory, ItemId, PlayerGameData};
use erdtree_tower::Class;

pub struct Flasks {
    pub remaining: [u8; 2],
    capacity: [u8; 2],
}

fn capacity(_class:Class,bonus:u8)->Result<[u8;2],String> {
    if bonus>4{return Err("圣杯容量增益无效".into());}
    Ok([5+bonus;2])
}

impl Flasks {
    pub fn missing(data: &PlayerGameData) -> (bool, bool) {
        let items = &data.equipment.equip_inventory_data.items_data;
        let present = |start| {
            items.items().any(|entry| {
                entry.item_id.category() == ItemCategory::Goods
                    && (start..=start + 25).contains(&entry.item_id.param_id())
            })
        };
        (!present(1000), !present(1050))
    }

    pub fn bind(data: &mut PlayerGameData, class: Class) -> Result<Self, String> {
        Self::bind_bonus(data,class,0)
    }

    pub fn bind_bonus(data:&mut PlayerGameData,class:Class,bonus:u8)->Result<Self,String> {
        let capacity=capacity(class,bonus)?;
        super::physick::ensure(data)?;
        let items = &data.equipment.equip_inventory_data.items_data;
        let find = |start| {
            resolve_flask(items.items().map(|e| (e.item_id, e.quantity)), start).map(|e| e.0)
        };
        find(1000).ok_or("等待角色取得红圣杯瓶后开始爬塔。")?;
        find(1050).ok_or("等待角色取得蓝圣杯瓶后开始爬塔。")?;
        let ledger = Self {
            remaining: capacity,
            capacity,
        };
        ledger.write(data)?;
        Ok(ledger)
    }

    pub fn total(&self) -> u8 {
        self.remaining[0] + self.remaining[1]
    }
    pub fn set_bonus(&mut self,data:&mut PlayerGameData,class:Class,bonus:u8)->Result<(),String> {
        let next=capacity(class,bonus)?;
        let old=self.capacity;self.capacity=next;
        if let Err(error)=self.write(data){self.capacity=old;return Err(error);}
        Ok(())
    }
    pub fn restore(&mut self,data:&mut PlayerGameData,total:u8)->Result<(),String>{
        let red=total.min(self.capacity[0]);self.remaining=[red,total.saturating_sub(red).min(self.capacity[1])];self.write(data)
    }

    pub fn observe(&mut self, data: &mut PlayerGameData, loading: bool) -> Result<(), String> {
        if loading {
            return Ok(());
        }
        let [red, blue] = current_flasks(data)?;
        self.remaining = observed_remaining(self.remaining, [red.1, blue.1], loading);
        self.write(data)
    }

    pub fn refill(&mut self, data: &mut PlayerGameData) -> Result<(), String> {
        self.apply_refill(data, refill_two(self.remaining, self.capacity))
    }

    pub fn camp_refill(&mut self, data: &mut PlayerGameData, added: [u8;2]) -> Result<(), String> {
        let target = camp_supply(self.remaining, self.capacity, added);
        if target == self.remaining && !super::physick::empty(data) { return Err("圣杯瓶与灵药已满".into()); }
        self.apply_refill(data, target)?;
        super::physick::refill(data)
    }

    pub fn fill(&mut self, data: &mut PlayerGameData) -> Result<(), String> {
        self.apply_refill(data, self.capacity)
    }

    fn apply_refill(&mut self, data: &mut PlayerGameData, target: [u8; 2]) -> Result<(), String> {
        let previous = self.remaining;
        self.remaining = target;
        if let Err(error) = self.write(data) {
            self.remaining = previous;
            let _ = self.write(data);
            return Err(error);
        }
        Ok(())
    }

    fn write(&self, data: &mut PlayerGameData) -> Result<(), String> {
        let selected = current_flasks(data)?;
        let targets=std::array::from_fn::<_,2,_>(|i|flask_target(selected[i].0,self.remaining[i] as u32));
        write_targets(data,targets)?;
        data.max_hp_flask = self.capacity[0];
        data.max_fp_flask = self.capacity[1];
        Ok(())
    }
}

fn write_targets(data:&mut PlayerGameData,targets:[(ItemId,u32);2])->Result<(),String> {
    let mut grants = Vec::new();
    for (id,quantity) in targets {
        if !data.equipment.equip_inventory_data.items_data.items().any(|e|e.item_id==id&&e.quantity>0) {
            grants.push(((0x40000000|id.param_id()) as i32,quantity as i32,-1));
        }
    }
    if !grants.is_empty() {
        super::items::give(&grants)?;
    }
    if targets.iter().any(|(id,_)|!data.equipment.equip_inventory_data.items_data.items().any(|e|e.item_id==*id&&e.quantity>0)) {
        return Err("补瓶尚未进入背包，未消耗节点，请重试".into());
    }
    let inventory=&mut data.equipment.equip_inventory_data;
    let items=&inventory.items_data;
    let obsolete:Vec<_>=items.normal_entries().iter().enumerate()
        .filter_map(|(i,e)|e.as_option().filter(|e|obsolete_flask(e.item_id,&targets)).map(|_|items.key_items_capacity+i as u32)).collect();
    if !obsolete.is_empty() {
        let function=super::items::checked_function(0x24d1e0,&[0x48,0x89,0x5c,0x24,0x20,0x89,0x54,0x24,0x10,0x55,0x56,0x57,0x41,0x56,0x41,0x57])?;
        let remove:unsafe extern "C" fn(*mut eldenring::cs::EquipInventoryData,u32,i32)->usize=unsafe {std::mem::transmute(function)};
        for slot in obsolete.into_iter().rev(){unsafe{remove(inventory,slot,1);}}
    }
    let items = &mut data.equipment.equip_inventory_data.items_data;
    for entry in items.items_mut() {
        for (id,quantity) in targets {
            if entry.item_id == id {
                entry.quantity = quantity;
            }
        }
    }
    let mut slots=Vec::new();
    for (id,_) in targets {
        let entry=items.items().find(|e|e.item_id==id).ok_or("圣杯瓶条目尚未就绪")?;
        let index=items.find_item_idx(id).ok_or("圣杯瓶快捷栏索引未就绪")? as i32;
        slots.push(EquipDataItem{gaitem_handle:entry.gaitem_handle,index});
    }
    normalize_slots(&mut data.equipment.equip_item_data.quick_slots,&slots);
    sync_slot_ids(&data.equipment.equip_item_data.quick_slots,&mut data.equipment.equipment_entries.quick_tems);
    Ok(())
}

fn flask_target(id:ItemId,remaining:u32)->(ItemId,u32) {
    (ItemId::new(ItemCategory::Goods,(id.param_id()&!1)+u32::from(remaining>0)).unwrap(),remaining.max(1))
}

#[test]
fn expanded_capacity_keeps_camp_supply_bounded() {
    for class in Class::ALL {for bonus in 0..=4 {
        let cap=capacity(class,bonus).unwrap();
        assert_eq!(cap,[5+bonus,5+bonus]);
        for blue in [false,true] {
            let supply=camp_supply([0,0],cap,if blue{[1,2]}else{[2,1]});
            assert_eq!(supply,if blue{[1,2.min(cap[1])]}else{[2,1]});
            assert_eq!(camp_supply(cap,cap,if blue{[1,2]}else{[2,1]}),cap);
        }
    }}
    assert!(capacity(Class::Samurai,5).is_err());
}

fn obsolete_flask(id:ItemId,targets:&[(ItemId,u32);2])->bool {
    id.category()==ItemCategory::Goods&&[1000..=1025,1050..=1075].iter().any(|range|range.contains(&id.param_id()))
        &&!targets.iter().any(|(target,_)|*target==id)
}

fn normalize_slots(slots:&mut [EquipDataItem;10],targets:&[EquipDataItem]) {
    for (color,target) in targets.iter().enumerate() {
        let start=if color==0 {1000}else{1050};
        let mut first=None;
        for (index,slot) in slots.iter_mut().enumerate() {
            let handle=slot.gaitem_handle;
            if matches!(handle.category(),Ok(GaitemCategory::Goods))&&!handle.is_indexed()&&(start..=start+25).contains(&handle.selector()) {
                if first.is_none() {first=Some(index);}
                slot.index=-1;slot.gaitem_handle=unsafe {std::mem::transmute::<u32,GaitemHandle>(u32::MAX)};
            }
        }
        let Some(index)=first.or_else(||slots.iter().position(|s|s.index<0)) else {continue;};
        slots[index].index=target.index;slots[index].gaitem_handle=target.gaitem_handle;
    }
}

fn sync_slot_ids(slots:&[EquipDataItem;10],ids:&mut [eldenring::cs::OptionalItemId;10]) {
    let flask=|id:u32|[1000..=1025,1050..=1075].iter().any(|r|r.contains(&id));
    for (slot,id) in slots.iter().zip(ids) {
        let handle=slot.gaitem_handle;
        let current=slot.index>=0&&matches!(handle.category(),Ok(GaitemCategory::Goods))&&!handle.is_indexed()&&flask(handle.selector());
        let stale=id.as_valid().is_some_and(|id|id.category()==ItemCategory::Goods&&flask(id.param_id()));
        if current {*id=ItemId::new(ItemCategory::Goods,handle.selector()).unwrap().into();}
        else if stale {*id=eldenring::cs::OptionalItemId::NONE;}
    }
}

fn current_flasks(data: &PlayerGameData) -> Result<[(ItemId, u32); 2], String> {
    let items = &data.equipment.equip_inventory_data.items_data;
    let find = |start| resolve_flask(items.items().map(|e| (e.item_id, e.quantity)), start);
    Ok([
        find(1000).ok_or("正在同步红圣杯瓶，暂不可选择节点。")?,
        find(1050).ok_or("正在同步蓝圣杯瓶，暂不可选择节点。")?,
    ])
}

pub fn restoration_upgrade(data:&PlayerGameData)->Result<[(i32,i32,i32);2],String> {
    restoration_plan(current_flasks(data)?,[data.max_hp_flask,data.max_fp_flask])
}
pub fn apply_restoration_upgrade(data:&mut PlayerGameData,plan:[(i32,i32,i32);2])->Result<(),String> {
    let capacity=[data.max_hp_flask.min(9),data.max_fp_flask.min(9)];
    let targets=plan.map(|(_,new,qty)|(ItemId::new(ItemCategory::Goods,new as u32).unwrap(),qty as u32));
    let result=write_targets(data,targets);
    data.max_hp_flask=capacity[0];data.max_fp_flask=capacity[1];
    result?;
    super::log(format!("Flask restoration upgraded: replacements={plan:?} capacity={capacity:?}"));
    Ok(())
}
fn restoration_plan(current:[(ItemId,u32);2],capacity:[u8;2])->Result<[(i32,i32,i32);2],String> {
    let level=current.iter().zip([1000,1050]).map(|((id,_),start)|(id.param_id()-start)/2).max().unwrap();
    if level>=12{return Err("圣杯瓶恢复量已达到 +12 上限".into());}
    Ok(std::array::from_fn(|i| {
        let (id,qty)=current[i];let qty=qty.min(u32::from(capacity[i].min(9)));let start=if i==0{1000}else{1050};
        (id.param_id() as i32,(start+(level+1)*2+u32::from(qty>0)) as i32,qty.max(1) as i32)
    }))
}
#[test]
fn restoration_upgrades_both_colors_without_refilling_or_exceeding_twelve() {
    let item=|id|ItemId::new(ItemCategory::Goods,id).unwrap();
    for level in 0..12 {
        assert_eq!(restoration_plan([(item(1001+level*2),2),(item(1050+level*2),0)],[5;2]).unwrap(),[(1001+level as i32*2,1003+level as i32*2,2),(1050+level as i32*2,1052+level as i32*2,1)]);
    }
    assert!(restoration_plan([(item(1025),3),(item(1075),2)],[5;2]).is_err());
}

fn resolve_flask(items: impl Iterator<Item = (ItemId, u32)>, start: u32) -> Option<(ItemId, u32)> {
    items
        .filter(|(id, _)| {
            id.category() == ItemCategory::Goods && (start..=start + 25).contains(&id.param_id())
        })
        .max_by_key(|(id, quantity)| (*quantity>0,id.param_id() % 2, *quantity, id.param_id()))
        .map(|(id, quantity)| (id, if id.param_id() % 2 == 0 { 0 } else { quantity }))
}

fn observed_remaining(previous: [u8; 2], quantities: [u32; 2], loading: bool) -> [u8; 2] {
    if loading {
        previous
    } else {
        std::array::from_fn(|i| quantities[i].min(u32::from(previous[i])) as u8)
    }
}

fn refill_two(mut current: [u8; 2], capacity: [u8; 2]) -> [u8; 2] {
    let mut left = 2;
    for i in 0..2 {
        let added = (capacity[i] - current[i]).min(left);
        current[i] += added;
        left -= added;
    }
    current
}

fn camp_supply(current: [u8; 2], capacity: [u8; 2], added: [u8;2]) -> [u8; 2] {
    std::array::from_fn(|i| current[i].saturating_add(added[i]).min(capacity[i]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restoration_upgrade_bounds_corrupted_twenty_charge_stacks() {
        assert_eq!(restoration_plan([(goods(1001),20),(goods(1051),20)],[5,7]).unwrap(),
            [(1001,1003,5),(1051,1053,7)]);
    }
    #[test]
    fn restoration_upgrade_preserves_expanded_and_empty_charge_counts() {
        for cap in 5u8..=9 {
            for red in 0..=cap {
                for blue in 0..=cap {
                    let plan=restoration_plan([(goods(if red==0 {1000}else{1001}),red as u32),
                        (goods(if blue==0 {1050}else{1051}),blue as u32)],[cap,cap]).unwrap();
                    assert_eq!(plan[0],(if red==0 {1000}else{1001},if red==0 {1002}else{1003},i32::from(red.max(1))));
                    assert_eq!(plan[1],(if blue==0 {1050}else{1051},if blue==0 {1052}else{1053},i32::from(blue.max(1))));
                }
            }
        }
    }
    #[test]
    fn observing_upgraded_flasks_keeps_bonus_charges_and_rejects_refills() {
        assert_eq!(observed_remaining([9,7],[9,20],false),[9,7]);
        assert_eq!(observed_remaining([8,6],[20,5],false),[8,5]);
    }
    #[test]
    fn camp_supply_preserves_selected_colors_and_caps_each_capacity() {
        assert_eq!(camp_supply([0, 0], [3, 2], [2,1]), [2, 1]);
        assert_eq!(camp_supply([0, 0], [3, 2], [1,2]), [1, 2]);
        assert_eq!(camp_supply([0, 0], [4, 1], [1,2]), [1, 1]);
        assert_eq!(camp_supply([3, 0], [4, 1], [2,1]), [4, 1]);
        assert_eq!(camp_supply([3, 2], [3, 2], [1,2]), [3, 2]);
        assert_eq!(camp_supply([1,2],[5,5],[3,0]),[4,2]);
        assert_eq!(camp_supply([1,2],[5,5],[0,3]),[1,5]);
        assert_eq!(camp_supply([4,4],[5,5],[3,0]),[5,4]);
        assert_eq!(camp_supply([4,4],[5,5],[0,3]),[4,5]);
    }
    #[test]
    fn removed_flasks_do_not_survive_in_equipment_display_ids() {
        let mut slots=std::array::from_fn(|_|EquipDataItem{gaitem_handle:unsafe {std::mem::transmute::<u32,GaitemHandle>(u32::MAX)},index:-1});
        slots[0]=EquipDataItem{gaitem_handle:GaitemHandle::from_parts(1001,GaitemCategory::Goods),index:396};
        slots[2]=EquipDataItem{gaitem_handle:GaitemHandle::from_parts(1051,GaitemCategory::Goods),index:397};
        let mut ids=std::array::from_fn(|i|goods(if i==9 {300}else{1000+i as u32}).into());
        sync_slot_ids(&slots,&mut ids);
        assert_eq!(ids[0].param_id(),Some(1001));assert_eq!(ids[2].param_id(),Some(1051));
        for i in [1,3,4,5,6,7,8]{assert!(!ids[i].is_valid());}
        assert_eq!(ids[9].param_id(),Some(300));
    }
    #[test]
    fn stale_zero_charge_entry_does_not_override_active_flask() {
        assert_eq!(resolve_flask([(goods(1001),0),(goods(1002),1)].into_iter(),1000),Some((goods(1002),0)));
    }
    #[test]
    fn empty_refill_cycles_keep_one_entry_per_color_and_preserve_other_goods() {
        let mut inventory=vec![(goods(1000),1),(goods(1001),4),(goods(1050),1),(goods(1051),1),(goods(300),8)];
        for remaining in [[0,0],[2,0],[0,0],[4,1],[0,1]] {
            let targets=[flask_target(goods(1001),remaining[0]),flask_target(goods(1051),remaining[1])];
            for (id,quantity) in targets {if !inventory.iter().any(|e|e.0==id){inventory.push((id,quantity));}}
            inventory.retain(|(id,_)|!obsolete_flask(*id,&targets));
            for (id,quantity) in targets {inventory.iter_mut().find(|e|e.0==id).unwrap().1=quantity;}
            assert_eq!(inventory.len(),3);
            assert!(inventory.contains(&(goods(300),8)));
            for (color,start) in [1000,1050].into_iter().enumerate(){assert_eq!(resolve_flask(inventory.iter().copied(),start).unwrap().1,remaining[color]);}
        }
    }
    #[test]
    fn rebinding_replaces_empty_flask_slots_without_accumulating_duplicates() {
        let item=|id,index|EquipDataItem{gaitem_handle:GaitemHandle::from_parts(id,GaitemCategory::Goods),index};
        let mut slots=std::array::from_fn(|i|item(if i%2==0 {1000}else{1051},i as i32));
        slots[4]=item(300,40);
        let targets=[item(1001,50),item(1051,51)];
        for _ in 0..4 {normalize_slots(&mut slots,&targets);}
        assert_eq!(slots.iter().filter(|s|s.index>=0).count(),3);
        assert_eq!(slots[0].index,50);assert_eq!(slots[1].index,51);assert_eq!(slots[4].index,40);
    }
    #[test]
    fn flask_sync_does_not_replace_other_items_in_full_quick_bar() {
        let mut slots=std::array::from_fn(|i|EquipDataItem{gaitem_handle:GaitemHandle::from_parts(300+i as u32,GaitemCategory::Goods),index:i as i32});
        let targets=[EquipDataItem{gaitem_handle:GaitemHandle::from_parts(1001,GaitemCategory::Goods),index:50}];
        normalize_slots(&mut slots,&targets);
        for (i,slot) in slots.iter().enumerate(){assert_eq!(slot.index,i as i32);assert_eq!(slot.gaitem_handle.selector(),300+i as u32);}
    }
    #[test]
    fn camp_adds_at_most_two_and_preserves_allocation() {
        assert_eq!(refill_two([1, 0], [4, 1]), [3, 0]);
        assert_eq!(refill_two([3, 0], [4, 1]), [4, 1]);
        assert_eq!(refill_two([3, 1], [3, 2]), [3, 2]);
        assert_eq!(refill_two([4, 1], [4, 1]), [4, 1]);
    }

    fn goods(id: u32) -> ItemId {
        ItemId::new(ItemCategory::Goods, id).unwrap()
    }

    #[test]
    fn resolves_live_flasks_after_game_replaces_starter_ids() {
        let inventory = [(goods(1001), 7), (goods(1051), 2)];
        assert_eq!(
            resolve_flask(inventory.into_iter(), 1000),
            Some((goods(1001), 7))
        );
        assert_eq!(
            resolve_flask(inventory.into_iter(), 1050),
            Some((goods(1051), 2))
        );
        assert_eq!(observed_remaining([4, 1], [7, 2], false), [4, 1]);
    }

    #[test]
    fn resolves_upgrades_and_empty_bottles_without_granting_charges() {
        assert_eq!(
            resolve_flask([(goods(1025), 3)].into_iter(), 1000),
            Some((goods(1025), 3))
        );
        assert_eq!(
            resolve_flask([(goods(1074), 1)].into_iter(), 1050),
            Some((goods(1074), 0))
        );
        assert_eq!(
            resolve_flask([(goods(1000), 1), (goods(1001), 2)].into_iter(), 1000),
            Some((goods(1001), 2))
        );
        assert_eq!(observed_remaining([4, 1], [3, 0], false), [3, 0]);
        assert_eq!(observed_remaining([3, 0], [4, 1], false), [3, 0]);
        assert_eq!(observed_remaining([3, 1], [0, 0], true), [3, 1]);
    }

    #[test]
    fn rejects_unrelated_items_and_missing_flasks() {
        let weapon = ItemId::new(ItemCategory::Weapon, 1001).unwrap();
        assert_eq!(
            resolve_flask([(weapon, 7), (goods(1026), 1)].into_iter(), 1000),
            None
        );
        assert_eq!(resolve_flask([(goods(1001), 4)].into_iter(), 1050), None);
    }
}
