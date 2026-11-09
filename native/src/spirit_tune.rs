// 营地原生调灵：临时设置追赶目标与单次凭证材料，关闭菜单后恢复参数。
use std::sync::atomic::{AtomicBool,AtomicU8,Ordering};
use eldenring::cs::{EquipParamGoods,EquipMtrlSetParam,ItemCategory,PlayerGameData,SoloParamRepository};
use fromsoftware_shared::FromStatic;
use erdtree_tower::rewards::{spirit_level,spirit_tune_target,SPIRIT_MAX};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Offer {pub source:u32,pub target:u32}
/// 本局记录的骨灰最高等级（运行时每帧同步）
static PEAK:AtomicU8=AtomicU8::new(0);
pub fn set_peak(peak:u8) {PEAK.store(peak.min(SPIRIT_MAX),Ordering::Relaxed);}

/// 背包里骨灰的最高等级
pub fn inventory_peak(data:&PlayerGameData)->u8 {
    data.equipment.equip_inventory_data.items_data.items()
        .filter(|e|e.quantity>0&&e.item_id.category()==ItemCategory::Goods)
        .filter_map(|e|spirit_level(0x40000000|e.item_id.param_id())).max().unwrap_or(0)
}
fn peak(data:&PlayerGameData)->u8 {PEAK.load(Ordering::Relaxed).max(inventory_peak(data))}
fn choices(owned:&[u32],peak:u8)->Vec<Offer> {
    let mut offers:Vec<Offer>=Vec::new();
    for &source in owned {
        let Some(target)=spirit_tune_target(0x40000000|source,peak).map(|t|t&0x0fffffff) else{continue};
        if owned.contains(&target)||offers.iter().any(|o|o.source==source) {continue;}
        offers.push(Offer{source,target});
    }
    offers.sort_by_key(|o|std::cmp::Reverse(o.target-o.source));
    offers
}
/// 可调灵的骨灰：背包里没到 +39、且还没有持有目标等级的骨灰；升得多的排前面
pub fn offers(data:&PlayerGameData)->Vec<Offer> {
    let owned:Vec<u32>=data.equipment.equip_inventory_data.items_data.items()
        .filter(|e|e.quantity>0&&e.item_id.category()==ItemCategory::Goods).map(|e|e.item_id.param_id())
        .filter(|id|spirit_level(0x40000000|id).is_some()).collect();
    choices(&owned,peak(data))
}
const MATERIAL:u32=700000;
pub const OPEN_EVENT:i32=136;
pub const MENU_KIND:i32=23;
static ACTIVE:AtomicBool=AtomicBool::new(false);
fn native_fields(id:u32,offers:&[Offer])->[i32;3] {
    [offers.iter().find(|o|o.source==id).map_or(-1,|o|o.target as i32),MATERIAL as i32,0]
}
fn fields(row:&eldenring::param::EQUIP_PARAM_GOODS_ST)->[i32;3] {[row.reinforce_goods_id(),row.reinforce_material_id(),row.reinforce_price()]}
fn set_fields(row:&mut eldenring::param::EQUIP_PARAM_GOODS_ST,value:[i32;3]) {
    row.set_reinforce_goods_id(value[0]);row.set_reinforce_material_id(value[1]);row.set_reinforce_price(value[2]);
}
struct Patch {id:u32,address:usize,before:[i32;3],after:[i32;3]}
impl Patch {
    fn can_restore(&self,address:usize,current:[i32;3])->bool {address==self.address&&current==self.after}
}
pub struct Session {rows:Vec<Patch>}
impl Drop for Session {
    fn drop(&mut self) {
        if let Ok(params)=unsafe {SoloParamRepository::instance_mut()} {
            for patch in &self.rows {
                if let Some(row)=params.get_mut::<EquipParamGoods>(patch.id)
                    &&patch.can_restore(row as *mut _ as usize,fields(row)) {set_fields(row,patch.before);}
            }
        }
        ACTIVE.store(false,Ordering::Release);
        super::log("Native spirit tuning parameter session restored");
    }
}
/// Only while the native menu is open: the engine previews and commits the same target/material pair.
pub fn prepare(data:&PlayerGameData)->Result<Session,String> {
    if super::smith::remaining(data)==0 {return Err("强化石不足".into());}
    let offers=offers(data);
    if offers.is_empty(){return Err("没有可调灵的骨灰".into());}
    let params=unsafe {SoloParamRepository::instance_mut()}.map_err(|_|"骨灰参数未就绪")?;
    let material=params.get::<EquipMtrlSetParam>(MATERIAL).ok_or("营地强化材料参数缺失")?;
    if material.material_id01()!=super::smith::VOUCHER as i32||material.material_cate01()!=4||material.item_num01()!=1
        ||[material.item_num02(),material.item_num03(),material.item_num04(),material.item_num05(),material.item_num06()].iter().any(|n|*n>0) {
        return Err("营地强化材料必须只消耗一张凭证".into());
    }
    let ids:std::collections::BTreeSet<u32>=erdtree_tower::rewards::CATALOG.iter()
        .filter(|item|item.kind==erdtree_tower::rewards::ItemKind::Spirit)
        .flat_map(|item|{let base=erdtree_tower::rewards::spirit_base(item.id)&0x0fffffff;(0..=u32::from(SPIRIT_MAX)).map(move |n|base+n)}).collect();
    for offer in &offers {
        if !ids.contains(&offer.source)||!ids.contains(&offer.target)||params.get::<EquipParamGoods>(offer.target).is_none() {
            return Err("调灵目标参数缺失，未打开菜单".into());
        }
    }
    let mut rows=Vec::new();
    for id in ids {
        let row=params.get::<EquipParamGoods>(id).ok_or("骨灰等级参数缺失，未打开菜单")?;
        rows.push(Patch{id,address:row as *const _ as usize,before:fields(row),after:native_fields(id,&offers)});
    }
    if ACTIVE.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).is_err(){return Err("调灵菜单已打开".into());}
    let session=Session{rows};
    for patch in &session.rows {set_fields(params.get_mut::<EquipParamGoods>(patch.id).expect("validated spirit row"),patch.after);}
    super::log(format!("Native spirit tuning: {offers:?}, material={MATERIAL}, runes=0"));
    Ok(session)
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_menu_matches_the_games_buddy_upgrade_entry() {
        assert_eq!(super::OPEN_EVENT,136);
        assert_eq!(super::MENU_KIND,eldenring::cs::MenuType::UnkEvent136 as i32);
    }
    #[test]
    fn native_preview_uses_catchup_target_one_voucher_and_no_runes() {
        let offer=super::Offer{source:200000,target:200007};
        assert_eq!(super::native_fields(200000,&[offer]),[200007,700000,0]);
        assert_eq!(super::native_fields(200007,&[offer]),[-1,700000,0]);
        assert_eq!(super::native_fields(201000,&[offer]),[-1,700000,0]);
    }
    #[test]
    fn native_cleanup_only_restores_the_same_unchanged_parameter_row() {
        let patch=super::Patch{id:200000,address:100,before:[200001,1,500],after:[200007,700000,0]};
        assert!(patch.can_restore(100,patch.after));
        assert!(!patch.can_restore(200,patch.after));
        assert!(!patch.can_restore(100,[200009,700000,0]));
    }
    #[test]
    fn choices_skip_maxed_and_already_owned_targets_and_catch_up_low_ashes() {
        let ash=erdtree_tower::rewards::CATALOG.iter().find(|i|i.kind==erdtree_tower::rewards::ItemKind::Spirit).unwrap().id&0x0fffffff;
        let other=erdtree_tower::rewards::CATALOG.iter().filter(|i|i.kind==erdtree_tower::rewards::ItemKind::Spirit).nth(1).unwrap().id&0x0fffffff;
        let offers=super::choices(&[ash,other+6],4);
        assert_eq!(offers,vec![super::Offer{source:ash,target:ash+5},super::Offer{source:other+6,target:other+7}]);
        assert!(super::choices(&[ash+39],3).is_empty());
        assert!(super::choices(&[ash,ash+5],4).iter().all(|o|o.source!=ash),"the target level is already held");
        assert!(super::choices(&[110000],0).is_empty(),"not a spirit ash");
    }
}
