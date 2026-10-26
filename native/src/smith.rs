use eldenring::cs::{EquipParamGoods, ItemCategory, PlayerGameData, SoloParamRepository};
use fromsoftware_shared::FromStatic;
use std::{ffi::c_void,sync::atomic::{AtomicUsize,Ordering}};
use hudhook::mh::{MhHook,MH_EnableHook};

static ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static OFFER_DIAGNOSTICS:std::sync::Mutex<std::collections::BTreeSet<String>>=std::sync::Mutex::new(std::collections::BTreeSet::new());
fn log_offer(message:String) {
    if let Ok(mut seen)=OFFER_DIAGNOSTICS.lock() {
        if seen.len()<2048&&seen.insert(message.clone()) {super::log(message);}
    }
}
#[repr(C)]
struct Offer {weapon:i32,material:i32,flags:u32}
fn update_offer(offer:&mut Offer,old:i32,next:i32,material:i32) {
    if offer.flags&7==1&&offer.weapon==old+1 {
        offer.weapon=next;offer.material=material;
    }
}
#[repr(C)]
struct Offers {allocator:usize,begin:*mut Offer,end:*mut Offer,capacity:*mut Offer}
type BuildOffers=unsafe extern "C" fn(*mut Offers,i32)->usize;
pub fn reinforcement_target(id:i32,cap:i32)->Option<i32> {
    target_with_peak(id,cap,0)
}
pub fn target_with_peak(id:i32,cap:i32,peak:i32)->Option<i32> {
    if id<0||!matches!(cap,10|25)||id%100>=cap {return None;}
    let step=if cap==25 {3}else{1};
    Some(id-id%100+(id%100).max(peak.clamp(0,50)*cap/50).saturating_add(step).min(cap))
}
pub fn received_with_peak(id:u32,cap:Option<i32>,peak:i32)->u32 {
    if id>>28!=0 {return id;}
    let Some(cap)=cap.filter(|cap|matches!(cap,10|25)) else{return id;};
    id-id%100+(id%100).max((peak.clamp(0,50)*cap/50) as u32).min(cap as u32)
}
pub fn acquisition_id(id:u32,peak:i32,params:&SoloParamRepository)->Result<u32,String> {
    use eldenring::cs::{EquipParamWeapon,ReinforceParamWeapon};
    if id>>28!=0 {return Ok(id);}
    let base=params.get::<EquipParamWeapon>(id/100*100).ok_or("武器参数未就绪")?;
    if base.origin_equip_wep1()<0 {return Ok(id);}
    let cap=if base.origin_equip_wep16()<0 {10}else{25};
    let next=received_with_peak(id,Some(cap),peak);
    let reinforce=i32::from(base.reinforce_type_id())+(next%100) as i32;
    if reinforce<0||params.get::<ReinforceParamWeapon>(reinforce as u32).is_none() {
        return Err("追平等级的武器参数不完整，未发放或扣款".into());
    }
    Ok(next)
}
pub fn reward_id(data:&PlayerGameData,id:u32)->Result<u32,String> {
    if id>>28!=0 {return Ok(id);}
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"武器参数未就绪")?;
    acquisition_id(id,inventory_peak(data,params),params)
}
pub fn inventory_peak(data:&PlayerGameData,params:&SoloParamRepository)->i32 {
    data.equipment.equip_inventory_data.items_data.items()
        .filter(|e|e.quantity>0&&e.item_id.category()==ItemCategory::Weapon)
        .filter_map(|e| {
            let id=e.item_id.param_id();
            let base=params.get::<eldenring::cs::EquipParamWeapon>(id/100*100)?;
            let cap=if base.origin_equip_wep16()<0 {10}else{25};
            (base.origin_equip_wep1()>=0&&id%100<=cap).then_some((id%100*50/cap) as i32)
        }).max().unwrap_or(0)
}
unsafe extern "C" fn build_offers(list:*mut Offers,id:i32)->usize {
    let original:BuildOffers=unsafe {std::mem::transmute(ORIGINAL.load(Ordering::Acquire))};
    let count=unsafe {original(list,id)};
    let Ok(params)=(unsafe {SoloParamRepository::instance()}) else{log_offer(format!("Smith bypass: id={id} params unavailable"));return count;};
    let Some(base)=params.get::<eldenring::cs::EquipParamWeapon>((id.max(0)/100*100) as u32) else{log_offer(format!("Smith bypass: id={id} weapon parameter missing"));return count;};
    if id<0||base.material_set_id()!=700000||base.origin_equip_wep1()<0 {
        log_offer(format!("Smith bypass: id={id} material={} origin1={}",base.material_set_id(),base.origin_equip_wep1()));
        return count;
    }
    let cap=if base.origin_equip_wep16()<0 {10}else{25};
    let player=unsafe {eldenring::cs::PlayerIns::local_player()}.ok();
    let peak=player.as_ref().map(|p|inventory_peak(unsafe {p.player_game_data.as_ref()},params)).unwrap_or(0);
    let Some(next)=target_with_peak(id,cap,peak) else{return count;};
    let Some(reinforce)=params.get::<eldenring::cs::ReinforceParamWeapon>((i32::from(base.reinforce_type_id())+next%100) as u32) else{log_offer(format!("Smith bypass: id={id} next={next} reinforcement parameter missing"));return count;};
    if list.is_null()||count>1024 {return count;}
    let list=unsafe {&mut *list};
    if list.begin.is_null()||(list.end as usize).checked_sub(list.begin as usize)!=Some(count*std::mem::size_of::<Offer>()) {return count;}
    // Pinned native offer entries: target ID, material set, category/operation bits.
    let mut changed=0;
    let mut offers=Vec::new();
    for offer in unsafe {std::slice::from_raw_parts_mut(list.begin,count)} {
        offers.push((offer.weapon,offer.flags));
        let before=offer.weapon;
        update_offer(offer,id,next,base.material_set_id()+i32::from(reinforce.material_set_id()));
        changed+=usize::from(before!=offer.weapon);
    }
    log_offer(format!("Smith offer: id={id} cap={cap} peak={peak} next={next} player={} changed={changed} offers={offers:?}",player.is_some()));
    count
}
pub unsafe fn install()->Result<(),String> {
    let address=super::items::checked_function(0xd58100,&[0x48,0x8b,0xc4,0x89,0x50,0x10,0x48,0x89,0x48,0x08,0x55,0x56,0x57,0x41,0x54])? as *mut c_void;
    let hook=unsafe {MhHook::new(address,build_offers as *mut c_void)}.map_err(|e|format!("Smith offer hook: {e:?}"))?;
    ORIGINAL.store(hook.trampoline() as usize,Ordering::Release);
    if let Err(error)=unsafe {MH_EnableHook(address)}.ok() {
        ORIGINAL.store(0,Ordering::Release);
        return Err(format!("Smith offer enable: {error:?}"));
    }
    Ok(())
}

pub const VOUCHER:u32=7500100;
const OLD_VOUCHER:u32=9500100;
pub const SHOP_FIRST:i32=9510000;
pub const SHOP_LAST:i32=9510099;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct TalismanOffer {pub source:u32,pub target:u32,pub text:i32}
fn eligible_offer(source:u32,target:u32,owned:&[u32])->bool {
    source!=target&&owned.contains(&source)&&!owned.contains(&target)
}
pub fn talisman_offers(data:&PlayerGameData)->Result<Vec<TalismanOffer>,String> {
    use eldenring::cs::{ShopLineupParam,EquipMtrlSetParam,EquipParamAccessory};
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"护符参数未就绪")?;
    let owned:Vec<_>=data.equipment.equip_inventory_data.items_data.items()
        .filter(|e|e.quantity>0&&e.item_id.category()==ItemCategory::Accessory).map(|e|e.item_id.param_id()).collect();
    let mut offers=Vec::new();
    for row in SHOP_FIRST..=SHOP_LAST {
        let Some(shop)=params.get::<ShopLineupParam>(row as u32) else{continue;};
        let material=params.get::<EquipMtrlSetParam>(shop.mtrl_id() as u32).ok_or("护符兑换材料缺失")?;
        let source=material.material_id02() as u32;let target=shop.equip_id() as u32;
        if shop.equip_type()!=2||material.material_id01()!=VOUCHER as i32||material.item_num01()!=1||material.item_num02()!=1
            ||material.material_cate02()!=2||params.get::<EquipParamAccessory>(source).is_none()||params.get::<EquipParamAccessory>(target).is_none(){return Err("护符兑换参数无效".into());}
        if eligible_offer(source,target,&owned){offers.push(TalismanOffer{source,target,text:9515000+row-SHOP_FIRST});}
    }
    Ok(offers)
}
pub fn upgrade_talisman(data:&mut PlayerGameData,offer:TalismanOffer)->Result<(),String> {
    use eldenring::cs::{ItemId,EquipInventoryData};
    if remaining(data)!=1||!talisman_offers(data)?.contains(&offer){return Err("护符或强化凭证已变化，请重新选择".into());}
    let remove_fn=super::items::checked_function(0x24d1e0,&[0x48,0x89,0x5c,0x24,0x20,0x89,0x54,0x24,0x10,0x55,0x56,0x57,0x41,0x56,0x41,0x57])?;
    let equip_fn=super::items::checked_function(0x249160,&[0x40,0x53,0x55,0x56,0x57,0x41,0x56,0x48,0x81,0xec,0x90,0,0,0])?;
    let source=ItemId::new(ItemCategory::Accessory,offer.source).map_err(|e|e.to_string())?;
    let target=ItemId::new(ItemCategory::Accessory,offer.target).map_err(|e|e.to_string())?;
    let voucher=ItemId::new(ItemCategory::Goods,VOUCHER).map_err(|e|e.to_string())?;
    let slots:Vec<_>=(17..=20).filter(|&i|data.equipment.chr_asm.equipment_param_ids[i]==offer.source as i32).collect();
    super::items::give(&[((0x20000000|offer.target) as i32,1,-1)])?;
    if data.equipment.equip_inventory_data.items_data.get_entry(target).is_none(){return Err("高阶护符未进入背包，未消耗材料".into());}
    for slot in slots {super::loadout::equip(data,slot,Some(target),equip_fn)?;}
    let remove:unsafe extern "C" fn(*mut EquipInventoryData,u32,i32)->usize=unsafe {std::mem::transmute(remove_fn)};
    for item in [source,voucher] {
        let inventory=&mut data.equipment.equip_inventory_data;
        let index=inventory.items_data.find_item_idx(item).ok_or("护符兑换材料发生变化")?;
        unsafe {remove(inventory,index,1);}
    }
    if remaining(data)!=0{return Err("强化凭证消耗未确认，请停止操作".into());}
    super::log(format!("Talisman upgraded: {} -> {}, voucher consumed",offer.source,offer.target));Ok(())
}
pub fn configure_menu()->Result<(),String> {
    if ORIGINAL.load(Ordering::Acquire)==0 {return Err("强化接口未加载，请检查日志".into());}
    if let Ok(mut seen)=OFFER_DIAGNOSTICS.lock(){seen.clear();}
    if let (Ok(player),Ok(params))=(unsafe {eldenring::cs::PlayerIns::local_player()},unsafe {SoloParamRepository::instance()}) {
        let data=unsafe {player.player_game_data.as_ref()};
        for (location,inventory) in std::iter::once(("bag",&data.equipment.equip_inventory_data)).chain(data.storage.as_ref().map(|s|("storage",&**s))) {
            let weapons:Vec<_>=inventory.items_data.items().filter(|e|e.quantity>0&&e.item_id.category()==ItemCategory::Weapon)
                .map(|e|{let id=e.item_id.param_id();(id,params.get::<eldenring::cs::EquipParamWeapon>(id/100*100).map(|p|(p.origin_equip_wep1(),p.origin_equip_wep16()))) }).collect();
            super::log(format!("Smith inventory: location={location} weapons={weapons:?}"));
        }
        super::log(format!("Smith inventory peak: {}",inventory_peak(data,params)));
    }
    // Pinned executable 2.7.2.0: the native eligibility predicate reads these four category gates.
    super::items::checked_function(0x78589b,&[0x8b,0x80,0x7c,0x04,0,0,0x85,0xc0,0x74,0x12])?;
    let menu=unsafe {eldenring::cs::CSMenuManImp::instance_mut()}.map_err(|_|"强化菜单未就绪")?;
    let bytes=unsafe {std::slice::from_raw_parts_mut(menu as *mut _ as *mut u8,std::mem::size_of::<eldenring::cs::CSMenuManImp>())};
    let previous=enable_categories(bytes)?;
    super::log(format!("Smith native category gates: {previous:?} -> [1, 1, 1, 1]"));
    Ok(())
}
fn enable_categories(menu:&mut [u8])->Result<[i32;4],String> {
    let fields=menu.get_mut(0x47c..0x48c).ok_or("强化菜单结构不匹配")?;
    let previous=std::array::from_fn(|i|i32::from_le_bytes(fields[i*4..i*4+4].try_into().unwrap()));
    for field in fields.chunks_exact_mut(4){field.copy_from_slice(&1i32.to_le_bytes());}
    Ok(previous)
}
#[derive(Default)]
pub struct Grant {requested:Option<std::time::Duration>,confirmed:bool}
#[derive(Debug,PartialEq)]
pub enum GrantStep {Request,Waiting,Ready,Failed}
impl Grant {
    pub fn poll(&mut self,now:std::time::Duration,quantity:u32)->GrantStep {
        if self.confirmed {return GrantStep::Ready;}
        let Some(start)=self.requested else {self.requested=Some(now);return GrantStep::Request;};
        if quantity>0 {self.confirmed=true;return GrantStep::Ready;}
        if now.saturating_sub(start)>=std::time::Duration::from_secs(5) {GrantStep::Failed}else{GrantStep::Waiting}
    }
}

pub fn remaining(data:&PlayerGameData)->u32 {
    data.equipment.equip_inventory_data.items_data.items()
        .filter(|e|e.item_id.category()==ItemCategory::Goods&&e.item_id.param_id()==VOUCHER)
        .map(|e|e.quantity).sum()
}
pub fn clear(data:&mut PlayerGameData) {
    if let Err(error)=clear_checked(data) {super::log(error);}
}
fn clear_checked(data:&mut PlayerGameData)->Result<(),String> {
    let function=super::items::checked_function(0x24d1e0,&[0x48,0x89,0x5c,0x24,0x20,0x89,0x54,0x24,0x10,0x55,0x56,0x57,0x41,0x56,0x41,0x57])?;
    let remove:unsafe extern "C" fn(*mut eldenring::cs::EquipInventoryData,u32,i32)->usize=unsafe {std::mem::transmute(function)};
    for inventory in std::iter::once(&mut data.equipment.equip_inventory_data).chain(data.storage.as_mut().map(|s|&mut **s)) {
        let items=&inventory.items_data;
        let slots:Vec<_>=items.current_key_entries().iter().enumerate().map(|(i,e)|(i as u32,e))
            .chain(items.normal_entries().iter().enumerate().map(|(i,e)|(items.key_items_capacity+i as u32,e)))
            .filter_map(|(i,e)|e.as_option().filter(|e|e.item_id.category()==ItemCategory::Goods&&[VOUCHER,OLD_VOUCHER,OLD_VOUCHER&0x7fffff].contains(&e.item_id.param_id())).map(|_|i)).collect();
        for slot in slots.into_iter().rev() {
            unsafe {remove(inventory,slot,1);}
        }
    }
    Ok(())
}
pub fn prepare(data:&mut PlayerGameData)->Result<(),String> {
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"铁匠参数未就绪")?;
    if params.get::<EquipParamGoods>(VOUCHER).is_none(){return Err("缺少营地强化凭证参数，请更新完整启动包".into());}
    clear_checked(data)?;
    super::items::give(&[((0x40000000|VOUCHER) as i32,1,-1)])?;
    Ok(())
}

pub fn spent(issued:bool,completed:bool,remaining:u32)->bool {issued&&!completed&&remaining==0}
#[cfg(test)]
mod tests {
    #[test]
    fn talisman_choices_require_owned_lower_tier_and_reject_duplicates() {
        assert!(super::eligible_offer(4000,4002,&[4000,6110]));
        assert!(!super::eligible_offer(4000,4002,&[6110]));
        assert!(!super::eligible_offer(4000,4002,&[4000,4002]));
        assert!(!super::eligible_offer(4002,4002,&[4002]));
    }
    #[test]
    fn catch_up_converts_caps_without_rounding_up_or_losing_affinity() {
        use super::target_with_peak;
        assert_eq!(target_with_peak(901200,25,40),Some(901223));
        assert_eq!(target_with_peak(901200,10,40),Some(901209));
        assert_eq!(target_with_peak(901200,25,35),Some(901220));
        assert_eq!(target_with_peak(901200,10,38),Some(901208));
        assert_eq!(target_with_peak(901220,25,40),Some(901223));
        assert_eq!(target_with_peak(901208,10,40),Some(901209));
        assert_eq!(target_with_peak(901224,25,50),Some(901225));
        assert_eq!(target_with_peak(901210,10,50),None);
        assert_eq!(target_with_peak(-1,25,50),None);
    }
    #[test]
    fn camp_offer_catches_normal_weapon_up_to_somber_six() {
        let peak=6*50/10;
        for level in [0,1,2,6,10,12] {
            let old=901200+level;
            let next=super::target_with_peak(old,25,peak).unwrap();
            assert_eq!(next,901218);
            let mut offer=super::Offer{weapon:old+1,material:700001,flags:1};
            super::update_offer(&mut offer,old,next,700018);
            assert_eq!(offer.weapon,901218);
            assert_eq!(offer.material,700018);
        }
    }
    #[test]
    fn acquisition_matches_peak_without_an_upgrade_step_or_affinity_loss() {
        use super::received_with_peak as receive;
        assert_eq!(receive(160001200,Some(25),30),160001215);
        assert_eq!(receive(160001200,Some(10),30),160001206);
        assert_eq!(receive(160001215,Some(25),30),160001215);
        assert_eq!(receive(160001218,Some(25),30),160001218);
        assert_eq!(receive(160001200,Some(10),36),160001207);
        assert_eq!(receive(160001200,Some(25),50),160001225);
        assert_eq!(receive(160001200,Some(10),50),160001210);
        assert_eq!(receive(160001200,Some(25),0),160001200);
        for id in [50000000,110000,0x40000000|2200006,0x10000000|5260000] {
            assert_eq!(receive(id,None,50),id);
        }
        assert_eq!(receive(0x40000000|2200006,Some(25),50),0x40000000|2200006);
    }
    #[test]
    fn upgrading_is_one_step_above_acquisition_for_every_valid_peak_and_level() {
        for cap in [10,25] {for peak in 0..=50 {for level in 0..cap {
            let id=160001200+level;
            let received=super::received_with_peak(id as u32,Some(cap),peak) as i32;
            let upgraded=super::target_with_peak(id,cap,peak).unwrap();
            assert_eq!(upgraded,160001200+(received%100+if cap==25 {3}else{1}).min(cap));
            assert!(upgraded>id&&upgraded%100<=cap);
        }}}
    }
    #[test]
    fn native_offer_changes_only_reinforcement_not_affinity_or_reversion() {
        use super::{Offer,update_offer};
        assert_eq!(std::mem::size_of::<Offer>(),12);
        for (id,flags,expected) in [(901201,1,901202),(901201,0x101,901202),(901201,4,901201),(901200,1,901200),(901301,1,901301)] {
            let mut offer=Offer{weapon:id,material:700001,flags};
            update_offer(&mut offer,901200,901202,700002);
            assert_eq!(offer.weapon,expected);assert_eq!(offer.flags,flags);
            assert_eq!(offer.material,if expected!=id {700002}else{700001});
        }
    }
    #[test]
    fn reinforcement_step_preserves_affinity_and_caps() {
        assert_eq!(super::reinforcement_target(901200,25),Some(901203));
        assert_eq!(super::reinforcement_target(901223,25),Some(901225));
        assert_eq!(super::reinforcement_target(901224,25),Some(901225));
        assert_eq!(super::reinforcement_target(901225,25),None);
        assert_eq!(super::reinforcement_target(901200,10),Some(901201));
        assert_eq!(super::reinforcement_target(901209,10),Some(901210));
        assert_eq!(super::reinforcement_target(901210,10),None);
        assert_eq!(super::reinforcement_target(-1,25),None);
    }
    #[test]
    fn opening_smith_enables_all_categories_without_changing_adjacent_menu_state() {
        let mut state=[0xabu8;0x490];
        state[0x47c..0x48c].fill(0);
        assert_eq!(super::enable_categories(&mut state).unwrap(),[0;4]);
        for category in state[0x47c..0x48c].chunks_exact(4) {assert_eq!(i32::from_le_bytes(category.try_into().unwrap()),1);}
        assert!(state[..0x47c].iter().chain(state[0x48c..].iter()).all(|b|*b==0xab));
        assert_eq!(super::enable_categories(&mut state).unwrap(),[1;4]);
        assert!(super::enable_categories(&mut state[..0x48b]).is_err());
    }
    #[test]
    fn voucher_fits_unindexed_goods_handle() {
        use eldenring::cs::{GaitemHandle,GaitemCategory};
        let handle=GaitemHandle::from_parts(super::VOUCHER,GaitemCategory::Goods);
        assert!(!handle.is_indexed());assert_eq!(handle.selector(),super::VOUCHER);
        assert_eq!(super::OLD_VOUCHER&0x7fffff,1111492);
    }
    #[test]
    fn delayed_inventory_confirmation_never_reissues_even_after_timeout() {
        use super::{Grant,GrantStep};use std::time::Duration;
        let mut grant=Grant::default();
        assert_eq!(grant.poll(Duration::ZERO,0),GrantStep::Request);
        for ms in 1..1000 {assert_eq!(grant.poll(Duration::from_millis(ms),0),GrantStep::Waiting);}
        assert_eq!(grant.poll(Duration::from_secs(1),1),GrantStep::Ready);
        assert_eq!(grant.poll(Duration::from_secs(2),0),GrantStep::Ready);
        let mut missing=Grant::default();missing.poll(Duration::ZERO,0);
        for seconds in 6..60 {assert_eq!(missing.poll(Duration::from_secs(seconds),0),GrantStep::Failed);}
        assert_eq!(missing.poll(Duration::from_secs(60),1),GrantStep::Ready);
    }
    #[test]
    fn cancellation_keeps_allowance_and_consumption_completes_only_once() {
        assert!(!super::spent(false,false,0));
        assert!(!super::spent(true,false,1));
        assert!(super::spent(true,false,0));
        assert!(!super::spent(true,true,0));
    }
}
