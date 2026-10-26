use eldenring::cs::{ChrIns,ChrInsExt,EquipParamWeapon,EquipParamProtector,EquipParamAccessory,ItemCategory,ItemId,PlayerGameData,SoloParamRepository,SpEffectParam};
use fromsoftware_shared::FromStatic;
use erdtree_tower::{contracts::{Kind,State},Run,Phase};

const FIRST:i32=9600000;
fn effects(kind:Kind)->Vec<i32> {
    let base=FIRST+kind.index() as i32*10;
    if kind==Kind::Mystic {vec![base,base+1,base+2]}else if kind==Kind::Ascetic{vec![base]}else{vec![base,base+1]}
}
pub fn sync(chr:&mut ChrIns,run:Option<&Run>,light:bool) {
    if validate().is_err(){return;}
    let desired=active_effects(run,chr.modules.data.hp,chr.modules.data.max_hp,light);
    let present:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).collect();
    for id in Kind::ALL.into_iter().flat_map(effects) {
        if present.contains(&id)&&!desired.contains(&id){chr.remove_speffect(id);}
    }
    for id in desired {if !present.contains(&id){chr.apply_speffect(id,false);}}
}
fn active_effects(run:Option<&Run>,hp:i32,max_hp:i32,light:bool)->Vec<i32> {
    let mut ids=desired(run);
    if !light {ids.retain(|id|*id!=FIRST+70);}
    if hp<=0||max_hp<=0||i64::from(hp)*2>=i64::from(max_hp) {ids.retain(|id|*id!=FIRST+80);}
    ids
}
fn is_light(weight:f32,maximum:f32)->bool {weight.is_finite()&&weight>=0.0&&maximum.is_finite()&&maximum>0.0&&f64::from(weight)*10.0<f64::from(maximum)*3.0}
pub fn light_load(data:&PlayerGameData)->bool {
    let Ok(params)=(unsafe {SoloParamRepository::instance()})else{return false;};
    let mut weight=0.0;
    for (slot,&id) in data.equipment.chr_asm.equipment_param_ids.iter().enumerate() {
        if id<0 {continue;}
        let value=match slot {
            0..=5=>params.get::<EquipParamWeapon>(id as u32/100*100).map(|p|p.weight()),
            12..=15=>params.get::<EquipParamProtector>(id as u32).map(|p|p.weight()),
            17..=20=>params.get::<EquipParamAccessory>(id as u32).map(|p|p.weight()),
            _=>continue,
        };
        let Some(value)=value else{return false;};weight+=value;
    }
    is_light(weight,data.max_equip_load)
}
fn victory_resources(kind:Option<Kind>,hp:i32,max_hp:i32,fp:i32,max_fp:i32)->(i32,i32) {
    if kind!=Some(Kind::Ascetic)||hp<=0{return (hp,fp);}
    let restore=|value:i32,max:i32,rate:i64| (i64::from(value)+i64::from(max.max(0))*rate/100).min(i64::from(max.max(0))) as i32;
    (restore(hp,max_hp,10),restore(fp,max_fp,15))
}
pub fn heal_victory(chr:&mut ChrIns,run:&Run) {
    if chr.chr_flags1c5.death_flag(){return;}
    let kind=run.contract.as_ref().filter(|c|c.act==run.act).and_then(|c|c.chosen);
    let data=&mut chr.modules.data;
    (data.hp,data.fp)=victory_resources(kind,data.hp,data.max_hp,data.fp,data.max_fp);
}
fn desired(run:Option<&Run>)->Vec<i32> {
    run.filter(|r|!matches!(r.phase,Phase::Dead|Phase::Won))
        .and_then(|r|r.contract.as_ref()).and_then(|c|c.chosen).map(effects).unwrap_or_default()
}
pub fn validate()->Result<(),String> {
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"契约参数尚未就绪")?;
    for id in Kind::ALL.into_iter().flat_map(effects) {
        if params.get::<SpEffectParam>(id as u32).is_none(){return Err("缺少本版契约参数，请使用更新后的启动器".into());}
    }
    Ok(())
}
pub fn observe_peak(data:&PlayerGameData,state:&mut State) {
    let Ok(params)=(unsafe {SoloParamRepository::instance()})else{return;};
    let mut scan=|inventory:&eldenring::cs::EquipInventoryData| {
        for item in inventory.items_data.items().filter(|i|i.quantity>0&&i.item_id.category()==ItemCategory::Weapon) {
            let id=item.item_id.param_id();
            let Some(base)=params.get::<EquipParamWeapon>(id/100*100)else{continue;};
            if base.origin_equip_wep1()<0 {continue;}
            let cap=if base.origin_equip_wep16()<0{10}else{25};
            state.peak=state.peak.max(((id%100).min(cap)*50/cap) as u8);
        }
    };
    scan(&data.equipment.equip_inventory_data);if let Some(storage)=data.storage.as_ref(){scan(storage);}
}
pub fn gift_entries(state:&State,index:usize,data:&PlayerGameData)->Result<[(u32,u32);2],String> {
    let base=*state.gear.get(index).ok_or("装备选择不存在")?;
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"装备参数尚未就绪")?;
    let id=super::smith::reward_id(data,base)?;
    let extra=match state.chosen {
        Some(Kind::Mystic|Kind::Ascetic) if base/1_000_000==34=>(0x40000000|6000,1),
        Some(Kind::Mystic|Kind::Ascetic) if base/1_000_000==33=>(0x40000000|4000,1),
        Some(Kind::Hunter)=>(erdtree_tower::contracts::hunter_ammo(base).ok_or("猎手装备弹药类型无效")?,30),
        _=>(0,0),
    };
    for (id,qty) in [(id,1),extra] {if qty>0 {
        let category=if id>>28==4{ItemCategory::Goods}else{ItemCategory::Weapon};
        let item=ItemId::new(category,id&0x0fffffff).map_err(|e|e.to_string())?;
        if params.get_equip_param(item).is_none(){return Err("适配装备参数不完整，未发放".into());}
    }}
    Ok([(id,1),extra])
}
pub fn give(data:&mut PlayerGameData,id:u32,qty:u32)->Result<(),String> {
    if qty==0{return Ok(());}
    let category=if id>>28==4{ItemCategory::Goods}else{ItemCategory::Weapon};
    let item=ItemId::new(category,id&0x0fffffff).map_err(|e|e.to_string())?;
    let count=|data:&PlayerGameData| {
        data.equipment.equip_inventory_data.items_data.items().filter(|i|i.item_id==item).map(|i|u64::from(i.quantity)).sum::<u64>()
            +data.storage.as_ref().map_or(0,|s|s.items_data.items().filter(|i|i.item_id==item).map(|i|u64::from(i.quantity)).sum::<u64>())
    };
    let before=count(data);
    if category==ItemCategory::Goods&&before>0{return Ok(());}
    super::items::give(&[(id as i32,qty as i32,-1)])?;
    if count(data)<before+u64::from(qty){return Err("适配装备未完整进入背包，请清理背包后重试".into());}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conditional_boons_switch_without_removing_drawbacks() {
        let mut run=Run::new(42,erdtree_tower::Class::Samurai);run.enable_contracts();
        for kind in [Kind::Feather,Kind::Blood] {
            run.contract.as_mut().unwrap().chosen=Some(kind);
            assert_eq!(active_effects(Some(&run),49,100,true),effects(kind));
            let inactive=active_effects(Some(&run),50,100,false);
            assert_eq!(inactive,vec![FIRST+kind.index() as i32*10+1]);
            run.phase=Phase::Dead;assert!(active_effects(Some(&run),49,100,true).is_empty());run.phase=Phase::Map;
        }
        assert!(is_light(29.9,100.0));assert!(!is_light(30.0,100.0));assert!(!is_light(0.0,0.0));
        assert!(!is_light(f32::NAN,100.0));
    }
    #[test]
    fn ascetic_victory_restores_both_resources_without_overheal_or_revive() {
        assert_eq!(victory_resources(Some(Kind::Ascetic),100,1000,10,200),(200,40));
        assert_eq!(victory_resources(Some(Kind::Ascetic),990,1000,199,200),(1000,200));
        assert_eq!(victory_resources(Some(Kind::Ascetic),0,1000,0,200),(0,0));
        assert_eq!(victory_resources(None,100,1000,10,200),(100,10));
    }
    #[test]
    fn effects_replace_previous_act_and_clear_on_death_win_or_hub() {
        let mut run=Run::new(42,erdtree_tower::Class::Astrologer);run.enable_contracts();
        assert!(desired(Some(&run)).is_empty());
        run.contract.as_mut().unwrap().choose(0,run.seed,run.class).unwrap();
        assert_eq!(desired(Some(&run)),effects(Kind::Mystic));
        assert_eq!(desired(Some(&run)).len(),3);
        for phase in [Phase::Dead,Phase::Won] {run.phase=phase;assert!(desired(Some(&run)).is_empty());}
        assert!(desired(None).is_empty());
        run.phase=Phase::Map;run.act=2;
        run.contract=Some(State::new(21,run.class,2,Some(Kind::Mystic),20));
        assert!(desired(Some(&run)).is_empty());
        run.contract.as_mut().unwrap().choose(0,21,run.class).unwrap();
        assert!(!desired(Some(&run)).iter().any(|id|effects(Kind::Mystic).contains(id)));
    }
}
