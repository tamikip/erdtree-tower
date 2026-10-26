use eldenring::cs::SoloParamRepository;
use fromsoftware_shared::FromStatic;
use erdtree_tower::rewards::{CATALOG,ItemKind};

#[derive(Clone, Copy)]
pub struct Loot { pub id:i32, pub name:&'static str }
pub fn casting_tools(data:&eldenring::cs::PlayerGameData)->[bool;2] {
    let Ok(params)=(unsafe {SoloParamRepository::instance()}) else{return [false,false];};
    casting_tools_from(data.equipment.equip_inventory_data.items_data.items()
        .filter(|e|e.item_id.category()==eldenring::cs::ItemCategory::Weapon).map(|e|(e.item_id.param_id(),e.quantity)),
        |id|params.get::<eldenring::cs::EquipParamWeapon>(id).map(|p|[p.enable_magic(),p.enable_miracle()]))
}
fn casting_tools_from(entries:impl IntoIterator<Item=(u32,u32)>,lookup:impl Fn(u32)->Option<[bool;2]>)->[bool;2] {
    let mut tools=[false,false];
    for (id,_) in entries.into_iter().filter(|(_,quantity)|*quantity>0) {
        if let Some(capabilities)=lookup(id/100*100) {
            tools[0]|=capabilities[0];tools[1]|=capabilities[1];
        }
    }
    tools
}
pub fn count(data:&eldenring::cs::PlayerGameData,id:u32)->u64 {
    use eldenring::cs::{ItemCategory,ItemId};
    let category=match id>>28 {0=>ItemCategory::Weapon,1=>ItemCategory::Protector,2=>ItemCategory::Accessory,4=>ItemCategory::Goods,_=>return 0};
    let Ok(target)=ItemId::new(category,id&0x0fffffff) else{return 0;};
    let owns=|item|super::items::same_reward_item(item,target);
    data.equipment.equip_inventory_data.items_data.items().filter(|e|owns(e.item_id)).map(|e|u64::from(e.quantity)).sum::<u64>()
        +data.storage.as_ref().map_or(0,|s|s.items_data.items().filter(|e|owns(e.item_id)).map(|e|u64::from(e.quantity)).sum::<u64>())
}
fn locked_choice(roll:u64,weapons:&[u32],crystals:&[u32])->Result<u32,String> {
    let pool=match roll%3 {2=>return Ok(0x40002724),1 if !crystals.is_empty()=>crystals,_=>weapons};
    if pool.is_empty(){return Err("上锁宝箱奖励池为空".into());}
    Ok(pool[(roll/3) as usize%pool.len()])
}
pub fn locked_reward(data:&eldenring::cs::PlayerGameData,run:&erdtree_tower::Run,node:usize)->Result<u32,String> {
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"物品参数尚未加载")?;
    let weapons:Vec<_>=CATALOG.iter().filter(|i|i.kind==ItemKind::Weapon&&i.random_eligible())
        .filter(|i|params.get::<eldenring::cs::EquipParamWeapon>(i.id).is_some_and(|p|matches!(p.rarity(),2|3))).map(|i|i.id).collect();
    let crystals:Vec<_>=CATALOG.iter().filter(|i|i.kind==ItemKind::Crystal&&i.random_eligible()&&count(data,i.id)==0).map(|i|i.id).collect();
    let mut n=run.seed ^ (node as u64).wrapping_mul(7919) ^ 0x10cced;
    n=(n^(n>>30)).wrapping_mul(0xbf58476d1ce4e5b9);n=(n^(n>>27)).wrapping_mul(0x94d049bb133111eb);n^=n>>31;
    let id=locked_choice(n,&weapons,&crystals)?;
    erdtree_tower::affixes::roll(run,id,n,true).ok_or_else(||"词条奖励池为空".into())
}
#[test]
fn locked_chest_gives_one_of_three_categories_and_handles_owned_crystals() {
    let weapons=[3150000,9060000];let crystals=[0x40000000|11000];
    for roll in 0..300 {
        let item=locked_choice(roll,&weapons,&crystals).unwrap();
        assert!(match roll%3 {0=>weapons.contains(&item),1=>crystals.contains(&item),_=>item==0x40002724});
    }
    assert!(weapons.contains(&locked_choice(1,&weapons,&[]).unwrap()));
    assert!(locked_choice(0,&[],&[]).is_err());
}
pub fn in_chest_blast(slot:usize,position:[f32;3])->bool {
    if slot>=3||position.iter().any(|p|!p.is_finite()){return false;}
    let center_x=(slot as f32-1.0)*5.0;
    (position[0]-center_x).powi(2)+(position[1]-0.15).powi(2)+position[2].powi(2)<=25.0
}
pub fn choose(seed:u64,node:usize)->Loot {
    let mut n=seed.wrapping_add((node as u64).wrapping_mul(0x9e3779b97f4a7c15));
    n=(n^(n>>30)).wrapping_mul(0xbf58476d1ce4e5b9);
    n=(n^(n>>27)).wrapping_mul(0x94d049bb133111eb);n^=n>>31;
    let kinds=[ItemKind::Weapon,ItemKind::Talisman,ItemKind::Sorcery,ItemKind::Incantation,ItemKind::Spirit];
    let roll=n as usize%21;
    let kind=if n%10==0 {ItemKind::Tool}else{kinds[if roll==20 {4}else{roll/5}]};
    let pool:Vec<_>=CATALOG.iter().filter(|i|i.kind==kind&&i.random_eligible()).collect();
    let item=pool[(n>>32) as usize%pool.len()];
    Loot{id:item.id as i32,name:item.name}
}
fn category(id:i32)->i32 {match (id as u32)>>28 {4=>1,2=>4,1=>3,_=>2}}
pub fn chest_choice(seed:u64,node:usize,owned:impl Fn(u32)->bool)->Result<Loot,String> {
    for attempt in 0..1024 {
        let item=choose(seed.wrapping_add(attempt),node);
        if (item.id as u32)>>28!=4||!owned(item.id as u32){return Ok(item);}
    }
    Err("未找到可领取的宝箱物品".into())
}
fn choose_for_run(run:&erdtree_tower::Run,seed:u64,node:usize)->Loot {
    let mut item=choose(seed,node);
    if run.affix_equipment {
        if erdtree_tower::rewards::lookup(item.id as u32).is_some_and(|i|i.kind==ItemKind::Weapon)&&seed%4==0 {
            let armor:Vec<_>=CATALOG.iter().filter(|i|i.kind==ItemKind::Armor).collect();
            let a=armor[(seed as usize^node)%armor.len()];item=Loot{id:a.id as i32,name:a.name};
        }
        if let Some(id)=erdtree_tower::affixes::roll(run,item.id as u32,seed^node as u64,true) {
            if let Some(i)=erdtree_tower::rewards::lookup(id){item=Loot{id:id as i32,name:i.name};}
        }
    }
    item
}
pub fn chest_choice_for_run(run:&erdtree_tower::Run,node:usize,owned:impl Fn(u32)->bool)->Result<Loot,String> {
    if !run.affix_equipment{return chest_choice(run.seed,node,owned);}
    for attempt in 0..1024 {
        let item=choose_for_run(run,run.seed.wrapping_add(attempt),node);
        if (item.id as u32)>>28!=4||!owned(item.id as u32){return Ok(item);}
    }
    Err("未找到可领取的宝箱物品".into())
}
#[test]
fn native_chest_and_event_equipment_is_affixed_and_repeatable() {
    let mut seen_armor=false;
    for seed in 0..1000 {
        let mut run=erdtree_tower::Run::new(seed,erdtree_tower::Class::Samurai);
        let item=chest_choice_for_run(&run,8,|_|false).unwrap();
        assert_eq!(item.id,chest_choice_for_run(&run,8,|_|false).unwrap().id);
        let id=item.id as u32;let kind=erdtree_tower::rewards::lookup(id).unwrap().kind;
        if matches!(kind,ItemKind::Weapon|ItemKind::Armor){assert!(erdtree_tower::affixes::tier(id).is_some());}
        seen_armor|=kind==ItemKind::Armor;
        run.affix_equipment=false;assert_eq!(chest_choice_for_run(&run,8,|_|false).unwrap().id,choose(seed,8).id);
    }
    assert!(seen_armor);
}
pub fn regional_reward(data:&mut eldenring::cs::PlayerGameData,run:&erdtree_tower::Run,seed:u64,node:usize)->Result<Loot,String> {
    let tools=casting_tools(data);
    for attempt in 0..1024 {
        let item=choose_for_run(run,seed.wrapping_add(attempt),node);
        if !erdtree_tower::rewards::castable(item.id as u32,tools){continue;}
        if (item.id as u32)>>28==4 {
            let owns=|id:eldenring::cs::ItemId,quantity:u32|id.param_id()==(item.id as u32&0x0fffffff)&&id.category()==eldenring::cs::ItemCategory::Goods&&quantity>0;
            if data.equipment.equip_inventory_data.items_data.items().any(|e|owns(e.item_id,e.quantity))||data.storage.as_ref().is_some_and(|s|s.items_data.items().any(|e|owns(e.item_id,e.quantity))){continue;}
        }
        super::items::give_reward(data,item.id as u32)?;
        return Ok(item);
    }
    Err("未找到可领取的事件物品".into())
}
pub fn prepare(run:&erdtree_tower::Run,node:usize,data:&eldenring::cs::PlayerGameData)->Result<(),String> {
    use eldenring::cs::ItemLotParam_map;
    let tools=casting_tools(data);
    let item=chest_choice_for_run(run,node,|id|!erdtree_tower::rewards::castable(id,tools))?;
    let granted=super::smith::reward_id(data,item.id as u32)?;
    let params=unsafe {SoloParamRepository::instance_mut()}.map_err(|_|"宝箱参数未加载")?;
    let lot=params.get_mut::<ItemLotParam_map>(45010900).ok_or("缺少原生宝箱掉落参数")?;
    lot.set_lot_item_id01((granted&0x0fffffff) as i32);lot.set_lot_item_category01(category(item.id));lot.set_lot_item_num01(1);
    super::log(format!("Native chest node {node}: {}",item.name));Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upgraded_catalysts_still_unlock_spell_rewards() {
        let lookup=|id|match id {160001200=>Some([true,false]),170000000=>Some([false,true]),_=>None};
        for level in 0..=25 {
            assert_eq!(casting_tools_from([(160001200+level,1),(170000000+level.min(10),1)],lookup),[true,true]);
            assert_eq!(casting_tools_from([(160001200+level,0),(170000000+level.min(10),1)],lookup),[false,true]);
            assert_eq!(casting_tools_from([(160001300+level,1)],lookup),[false,false]);
        }
    }
    #[test]
    fn chest_catalyst_filter_keeps_only_castable_spells() {
        for tools in [[false,false],[true,false],[false,true],[true,true]] {
            let mut seen=[false,false];
            for seed in 0..1000 {
                let item=chest_choice(seed,9,|id|!erdtree_tower::rewards::castable(id,tools)).unwrap();
                assert!(erdtree_tower::rewards::castable(item.id as u32,tools));
                match erdtree_tower::rewards::lookup(item.id as u32).unwrap().kind {
                    ItemKind::Sorcery=>seen[0]=true,ItemKind::Incantation=>seen[1]=true,_=>{},
                }
            }
            assert_eq!(seen,tools);
        }
    }
    #[test]
    fn personal_chest_preserves_roll_unless_unique_goods_are_already_owned() {
        for seed in 0..500 {
            assert_eq!(chest_choice(seed,9,|_|false).unwrap().id,choose(seed,9).id);
            assert_ne!((chest_choice(seed,9,|_|true).unwrap().id as u32)>>28,4);
        }
    }
    #[test]
    fn chest_blast_covers_five_meters_once_at_each_chest_not_the_whole_room() {
        for slot in 0..3 {
            let x=(slot as f32-1.0)*5.0;
            assert!(in_chest_blast(slot,[x,0.15,4.9]));
            assert!(in_chest_blast(slot,[x+5.0,0.15,0.0]));
            assert!(!in_chest_blast(slot,[x,0.15,5.1]));
            assert!(!in_chest_blast(slot,[x,6.0,0.0]));
        }
        assert!(!in_chest_blast(3,[0.0;3]));assert!(!in_chest_blast(0,[f32::NAN;3]));
    }
    #[test]
    fn spirit_chest_weight_is_about_one_in_twenty_one() {
        let count=(0..10000).filter(|&seed|erdtree_tower::rewards::lookup(choose(seed,8).id as u32).unwrap().kind==ItemKind::Spirit).count();
        assert!((350..650).contains(&count));
    }
    #[test]
    fn chest_retry_keeps_same_item_and_pool_varies() {
        assert_eq!(choose(42,8).id,choose(42,8).id);
        let seen:std::collections::HashSet<_>=(0..10000).map(|seed|choose(seed,8).id).collect();
        assert!(seen.iter().all(|id|erdtree_tower::rewards::lookup(*id as u32).unwrap().random_eligible()));
        assert!(seen.len()>500);
        assert!(seen.iter().all(|id|CATALOG.iter().find(|i|i.id==*id as u32).unwrap().kind!=ItemKind::Armor));
        for kind in [ItemKind::Weapon,ItemKind::Talisman,ItemKind::Sorcery,ItemKind::Incantation,ItemKind::Spirit] {
            assert!(CATALOG.iter().any(|i|i.kind==kind&&seen.contains(&(i.id as i32))));
        }
        assert_eq!(category(0x10000000),3);assert_eq!(category(0x20000000),4);assert_eq!(category(0x40000000),1);assert_eq!(category(2000000),2);
    }
}
