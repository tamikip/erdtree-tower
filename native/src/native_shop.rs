use eldenring::cs::{
    CSEventFlagMan, EquipParamWeapon, EquipParamGoods, ItemCategory, PlayerGameData, ShopLineupParam,
    SoloParamRepository,
};
use erdtree_tower::{Blessing, BlessingKind, NodeKind, Run};
use fromsoftware_shared::FromStatic;
pub const GOODS: u32 = 9500000;
pub const STOCK_COUNT: u32 = 37;
fn key_offer(roll:u64)->bool {roll%100<20}
#[test]
fn key_shop_roll_has_exact_twenty_percent_and_one_key_for_5000() {
    assert_eq!((0..100).filter(|&r|key_offer(r)).count(),20);
    let mut count=0;
    for seed in 0..1000 {
        let run=Run::new(seed,erdtree_tower::Class::Samurai);
        let rows=stock(&run);
        if let Some(key)=rows.iter().find(|s|s.kind==3&&s.id==8000) {
            count+=1;assert_eq!((key.price,key.quantity),(5000,1));
        }
        assert_eq!(rows,stock(&run));
    }
    assert!((160..=240).contains(&count));
}
const STOCK_FLAGS: u32 = 45020800;
fn weapon_price(rarity:u8)->i32 {match rarity {0|1=>3000,2=>5000,_=>8000}}
#[test]
fn new_shop_sells_affixed_equipment_and_legacy_stock_is_preserved() {
    for seed in 0..40 {
        let mut run=Run::new(seed,erdtree_tower::Class::Astrologer);
        let items=stock(&run);assert_eq!(items,stock(&run));
        for item in items.iter().filter(|i|i.quantity==1&&matches!(i.kind,0|1)) {
            let id=if item.kind==1{0x10000000|item.id as u32}else{item.id as u32};
            assert!(erdtree_tower::affixes::tier(id).is_some());
        }
        run.affix_equipment=false;
        assert!(stock(&run).iter().filter(|i|i.kind==0).all(|i|i.id<100000000));
    }
}
fn talisman_price(tier:u8)->i32 {match tier {0=>1000,1=>2000,2=>4000,_=>8000}}
#[test]
fn talisman_prices_follow_card_quality_and_class_discount() {
    for (tier,price) in [(0,1000),(1,2000),(2,4000),(3,8000)] {
        assert_eq!(talisman_price(tier),price);
        assert_eq!(super::passives::shop_price(erdtree_tower::Class::Prisoner,price),price*4/5);
    }
    for item in stock(&Run::new(42,erdtree_tower::Class::Samurai)).iter().filter(|s|s.kind==2) {
        assert_eq!(item.price,talisman_price(super::reward_ui::item_quality(0x20000000|item.id as u32)));
    }
}
fn is_spell(item:&Stock)->bool {
    use erdtree_tower::rewards::{lookup,ItemKind};
    item.kind==3 && lookup(0x40000000|item.id as u32)
        .is_some_and(|item|matches!(item.kind,ItemKind::Sorcery|ItemKind::Incantation))
}
#[test]
fn larval_tear_is_always_available_for_5000_runes() {
    for seed in 0..64 {
        let rows=stock(&Run::new(seed,erdtree_tower::Class::Samurai));
        let tears:Vec<_>=rows.iter().filter(|s|s.kind==3&&s.id==8185).collect();
        assert_eq!(tears.len(),1);
        assert_eq!((tears[0].price,tears[0].quantity),(5000,-1));
    }
}
#[test]
fn crystal_shop_roll_is_rare_stable_and_costs_6000() {
    let mut count=0;
    for seed in 0..1000 {
        let run=Run::new(seed,erdtree_tower::Class::Samurai);
        let rows=stock(&run);
        if let Some(item)=rows.iter().find(|s|s.kind==3&&(11000..=11031).contains(&s.id)) {
            count+=1;assert_eq!((item.price,item.quantity),(6000,1));
            assert_eq!(rows,stock(&run));
        }
    }
    assert!((25..=75).contains(&count));
}
fn stock_flag(index:u32)->u32 {if index<21 {STOCK_FLAGS+index*8}else if index<26 {45020700+(index-21)*8}else{45020600+(index-26)*8}}
#[test]
fn random_shop_only_sells_base_talismans() {
    for seed in 0..500 {
        let run=Run::new(seed,erdtree_tower::Class::Samurai);
        for item in stock(&run).iter().filter(|s|s.kind==2) {
            assert!(erdtree_tower::rewards::lookup(0x20000000|item.id as u32).unwrap().random_eligible());
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stock {
    pub kind: u8,
    pub id: i32,
    pub price: i32,
    pub quantity: i16,
}
fn hash(mut n: u64) -> u64 {
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
    n ^ (n >> 31)
}
fn select(mut pool:Vec<Stock>,count:usize,seed:u64,visit:usize)->Vec<Stock> {
    assert!(pool.len()>=count);
    pool.sort_by_key(|s|hash(seed^s.id as u64));
    (0..count).map(|i|pool[(visit*count+i)%pool.len()].clone()).collect()
}
fn extras(group:&str)->Vec<Stock> {
    include_str!("../../scene-assets/shop-extras.txt").lines().filter_map(|line| {
        let p:Vec<_>=line.split('|').collect();
        (p[0]==group).then(||Stock{kind:p[1].parse().unwrap(),id:p[2].parse().unwrap(),price:p[3].parse().unwrap(),quantity:p[4].parse().unwrap()})
    }).collect()
}
pub fn stock(run:&Run)->Vec<Stock> {
    use erdtree_tower::{Class,rewards::{CATALOG,ItemKind}};
    let visit=run.visited.iter().filter(|&&i|run.nodes[i].kind==NodeKind::Shop).count();
    let catalog:Vec<_>=include_str!("../../scene-assets/shop-catalog.txt").lines().map(|line| {
        let p:Vec<_>=line.split('|').collect();
        Stock{kind:p[0].parse().unwrap(),id:p[1].parse().unwrap(),price:p[2].parse().unwrap(),quantity:1}
    }).collect();
    let mut result=Vec::new();
    let catalyst=match run.class {Class::Astrologer|Class::Prisoner=>Some("staff"),Class::Prophet|Class::Confessor=>Some("seal"),_=>None};
    if let Some(group)=catalyst {result.extend(select(extras(group),2,run.seed,visit));}
    result.extend(select(catalog.iter().filter(|s|s.kind==0).cloned().collect(),if catalyst.is_some(){3}else{5},run.seed,visit));
    for (kind,equip,price) in [(ItemKind::Sorcery,3,1800),(ItemKind::Incantation,3,1800),(ItemKind::Armor,1,1500)] {
        result.extend(select(CATALOG.iter().filter(|s|s.kind==kind).map(|s|Stock{kind:equip,id:(s.id&0x0fffffff) as i32,price,quantity:1}).collect(),3,run.seed,visit));
    }
    for group in ["food","pot","knife"] {result.extend(select(extras(group),1,run.seed,visit));}
    result.extend(select(catalog.iter().filter(|s|s.kind==4).cloned().collect(),2,run.seed,visit));
    let pool=CATALOG.iter().filter(|s|s.kind==ItemKind::Talisman&&s.random_eligible()).map(|s|Stock{kind:2,id:(s.id&0x0fffffff) as i32,price:talisman_price(super::reward_ui::item_quality(s.id)),quantity:1}).collect();
    result.extend(select(pool,2,run.seed,visit));
    if hash(run.seed ^ visit as u64 ^ 0xa549)%4==0 {
        result.extend(select(CATALOG.iter().filter(|s|s.kind==ItemKind::Spirit).map(|s|Stock{kind:3,id:(s.id&0x0fffffff) as i32,price:2000,quantity:1}).collect(),1,run.seed,visit));
    } else {
        let pool=extras("food").into_iter().filter(|s|!result.iter().any(|r|r.kind==s.kind&&r.id==s.id)).collect();
        result.extend(select(pool,1,run.seed,visit));
    }
    for item in &mut result {
        if item.kind==3 {item.id=(erdtree_tower::rewards::random_spirit_level(0x40000000|item.id as u32,run.reward_seed()^visit as u64)&0x0fffffff) as i32;}
    }
    result.push(Stock{kind:3,id:8185,price:5000,quantity:-1});
    if let Some(id)=erdtree_tower::rewards::crystal_offer(run,visit as u64,0x5a09) {
        result[21]=Stock{kind:3,id:(id&0x0fffffff) as i32,price:6000,quantity:1};
    }
    if key_offer(hash(run.seed ^ visit as u64 ^ 0x8000cafe)) {
        result.push(Stock{kind:3,id:8000,price:5000,quantity:1});
    } else {
        let pool=extras("food").into_iter().filter(|s|!result.iter().any(|r|r.kind==s.kind&&r.id==s.id)).collect();
        result.extend(select(pool,1,run.seed ^ 0x8000cafe,visit));
    }
    result.push(Stock{kind:0,id:50000000,price:20,quantity:100});
    let arrows=[50010000,50090000,50100000,50200000];
    let arrow=arrows[hash(run.reward_seed() ^ visit as u64 ^ 0xa440) as usize%arrows.len()];
    result.push(Stock{kind:0,id:arrow,price:60,quantity:20});
    result.extend(extras("resistance"));
    affix_stock(run,visit,&mut result);
    result
}
fn affix_stock(run:&Run,visit:usize,result:&mut [Stock]) {
    for (slot,item) in result.iter_mut().enumerate() {
        if item.quantity==1&&matches!(item.kind,0|1) {
            let tag=if item.kind==0{0}else{0x10000000};
            if let Some(id)=erdtree_tower::affixes::roll(run,tag|item.id as u32,(visit as u64).wrapping_mul(7919)^slot as u64,true){item.id=(id&0x0fffffff) as i32;}
        }
    }
}
fn ensure_pots(data:&mut PlayerGameData)->Result<(),String> {
    let count=||data.equipment.equip_inventory_data.items_data.items()
        .filter(|e|e.item_id.category()==ItemCategory::Goods&&e.item_id.param_id()==9500).map(|e|e.quantity as i32).sum::<i32>();
    let missing=(3-count()).max(0);
    if missing>0 {
        super::items::give(&[(0x4000251c,missing,-1)])?;
        if count()<3 {return Err("龟裂壶尚未进入背包，请重试".into());}
    }
    Ok(())
}
pub fn prepare(run: &Run, node: usize, refresh: bool, data:&mut PlayerGameData) -> Result<erdtree_tower::ShopStock, String> {
    let params = unsafe { SoloParamRepository::instance_mut() }.map_err(|_| "商店参数未加载")?;
    let saved=run.shop_stock.as_ref().filter(|saved|!refresh&&saved.node==node&&saved.items.len()==STOCK_COUNT as usize);
    let stock=if let Some(saved)=saved {saved.items.iter().map(|[kind,id,price,quantity]|Stock{kind:*kind as u8,id:*id,price:*price,quantity:*quantity as i16}).collect()}else{
        let tools=super::loot::casting_tools(data);
        let mut stock = stock(run);
        filter_spells(&mut stock,tools,run.reward_seed());
        affix_stock(run,run.visited.iter().filter(|&&i|run.nodes[i].kind==NodeKind::Shop).count(),&mut stock);
        for item in &mut stock {
            let rarity=if item.kind==0&&(item.id<50000000||erdtree_tower::affixes::tier(item.id as u32).is_some()) {
                Some(params.get::<EquipParamWeapon>(item.id as u32).ok_or("商店武器参数不存在")?.rarity())
            } else if is_spell(item) {
                Some(params.get::<EquipParamGoods>(item.id as u32).ok_or("商店法术参数不存在")?.rarity())
            } else {None};
            if let Some(rarity)=rarity {item.price=weapon_price(rarity);}
        }
        for item in &mut stock {item.price=super::passives::shop_price(run.class,item.price);}
        stock
    };
    if stock.len()!=STOCK_COUNT as usize {return Err("商店货单数量与原生商品槽位不符".into());}
    for i in 0..STOCK_COUNT {
        if params.get::<ShopLineupParam>(GOODS + i).is_none() {
            return Err("缺少原生商店参数，请使用完整启动包".into());
        }
    }
    for item in &stock {
        let category=match item.kind {
            0=>ItemCategory::Weapon,1=>ItemCategory::Protector,2=>ItemCategory::Accessory,3=>ItemCategory::Goods,4=>ItemCategory::Gem,
            _=>return Err("无效商店商品类型".into()),
        };
        let id=eldenring::cs::ItemId::new(category,item.id as u32).map_err(|e|e.to_string())?;
        if params.get_equip_param(id).is_none() {
            return Err(format!("商品参数不存在：{} / {}",item.kind,item.id));
        }
    }
    ensure_pots(data)?;
    let flags = unsafe { CSEventFlagMan::instance_mut() }.map_err(|_| "商店库存标记未加载")?;
    for flag in (0..STOCK_COUNT).flat_map(|i|stock_flag(i)..stock_flag(i)+8).filter(|_|refresh) {
        flags.virtual_memory_flag.set_flag(flag, false);
    }
    for (i, item) in stock.iter().enumerate() {
        let row = params.get_mut::<ShopLineupParam>(GOODS + i as u32).unwrap();
        row.set_equip_id(item.id);
        row.set_equip_type(item.kind);
        row.set_value(item.price);
        row.set_cost_type(0);
        row.set_mtrl_id(-1);
        row.set_sell_quantity(item.quantity);
        row.set_set_num(1);
        row.set_icon_id(-1);
        row.set_name_msg_id(-1);
        row.set_menu_title_msg_id(-1);
        row.set_menu_icon_id(-1);
        row.set_value_add(0);
        row.set_value_magnification(1.0);
    }
    super::log(format!("Native rune shop stock: {stock:?}"));
    Ok(erdtree_tower::ShopStock{node,items:stock.iter().map(|s|[i32::from(s.kind),s.id,s.price,i32::from(s.quantity)]).collect()})
}
fn filter_spells(stock:&mut [Stock],tools:[bool;2],seed:u64) {
    use erdtree_tower::rewards::{CATALOG,ItemKind,castable};
    for index in 0..stock.len() {
        if stock[index].kind==3&&!castable(0x40000000|stock[index].id as u32,tools) {
            let pool:Vec<_>=CATALOG.iter().filter(|i|i.kind==ItemKind::Weapon&&i.random_eligible()
                &&!stock.iter().any(|s|s.kind==0&&s.id==i.id as i32)).collect();
            stock[index]=Stock{kind:0,id:pool[hash(seed^index as u64) as usize%pool.len()].id as i32,price:3000,quantity:1};
        }
    }
}
#[test]
fn shop_catalyst_filter_preserves_stock_indices_and_allowed_spells() {
    for tools in [[false,false],[true,false],[false,true],[true,true]] {
        let original=stock(&Run::new(42,erdtree_tower::Class::Astrologer));
        let mut filtered=original.clone();filter_spells(&mut filtered,tools,42);
        assert_eq!(filtered.len(),STOCK_COUNT as usize);
        for (before,after) in original.iter().zip(&filtered) {
            if before.kind==3&&!erdtree_tower::rewards::castable(0x40000000|before.id as u32,tools) {
                assert_eq!(after.kind,0);
            } else {assert_eq!(before,after);}
        }
        let mut repeated=original;filter_spells(&mut repeated,tools,42);assert_eq!(filtered,repeated);
    }
}
pub fn pending(data: &PlayerGameData) -> Vec<usize> {
    data.equipment
        .equip_inventory_data
        .items_data
        .items()
        .filter(|e| {
            e.quantity > 0
                && e.item_id.category() == ItemCategory::Goods
                && (GOODS..GOODS + 8).contains(&e.item_id.param_id())
        })
        .map(|e| (e.item_id.param_id() - GOODS) as usize)
        .collect()
}
pub fn consume(data: &mut PlayerGameData) {
    for e in data.equipment.equip_inventory_data.items_data.items_mut() {
        if e.item_id.category() == ItemCategory::Goods
            && (GOODS..GOODS + 8).contains(&e.item_id.param_id())
        {
            e.quantity = 0;
        }
    }
}
pub fn add_blessings(run: &mut Run, indices: &[usize]) {
    for &index in indices {
        let kind = BlessingKind::ALL[index];
        if let Some(b) = run.blessings.iter_mut().find(|b| b.kind == kind) {
            b.level = (b.level + 1).min(3);
        } else {
            run.blessings.push(Blessing { kind, level: 1 });
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn every_shop_has_all_resistance_medicines_and_boluses_ten_each() {
        for class in erdtree_tower::Class::ALL {for seed in 0..32 {
            let rows=super::stock(&erdtree_tower::Run::new(seed,class));
            for id in [900,910,920,930,940,950,960,1150,1160,1170,1180] {
                let matches:Vec<_>=rows.iter().filter(|s|s.kind==3&&s.id==id).collect();
                assert_eq!(matches.len(),1,"missing or duplicate medicine {id}");
                assert_eq!((matches[0].quantity,matches[0].price),(10,300));
            }
        }}
    }
    use super::*;
    #[test]
    fn each_shop_has_fixed_arrow_supply_and_one_stable_element() {
        let mut seen=std::collections::HashSet::new();
        for class in erdtree_tower::Class::ALL {for seed in 0..100 {
            let run=Run::new_personal(seed,class,seed+1000);
            let rows=stock(&run);
            assert_eq!(rows,stock(&run));
            let arrows:Vec<_>=rows.iter().filter(|s|s.kind==0&&(50000000..60000000).contains(&s.id)).collect();
            assert_eq!(arrows.len(),2);
            assert_eq!((arrows[0].id,arrows[0].quantity,arrows[0].price),(50000000,100,20));
            assert!([50010000,50090000,50100000,50200000].contains(&arrows[1].id));
            assert_eq!((arrows[1].quantity,arrows[1].price),(20,60));
            seen.insert(arrows[1].id);
        }}
        assert_eq!(seen.len(),4);
    }
    #[test]
    fn spell_pricing_excludes_other_goods_and_discounts_after_rarity() {
        use erdtree_tower::{Class,rewards::{CATALOG,ItemKind}};
        for item in CATALOG.iter() {
            let stock=Stock{kind:3,id:(item.id&0x0fffffff) as i32,price:1800,quantity:1};
            if item.id>>28==4 {assert_eq!(is_spell(&stock),matches!(item.kind,ItemKind::Sorcery|ItemKind::Incantation));}
        }
        for id in [8000,8185,10020,11000] {assert!(!is_spell(&Stock{kind:3,id,price:5000,quantity:1}));}
        for (rarity,price,discounted) in [(0,3000,2400),(1,3000,2400),(2,5000,4000),(3,8000,6400)] {
            assert_eq!(weapon_price(rarity),price);
            assert_eq!(super::super::passives::shop_price(Class::Prisoner,weapon_price(rarity)),discounted);
        }
    }
    #[test]
    fn weapon_prices_increase_with_native_rarity() {
        assert_eq!(weapon_price(1),3000);
        assert_eq!(weapon_price(2),5000);
        assert_eq!(weapon_price(3),8000);
        assert!((0..u8::MAX).all(|r|weapon_price(r)<=weapon_price(r+1)));
    }
    #[test]
    fn expanded_stock_has_required_categories_quantities_and_class_catalysts() {
        use erdtree_tower::{Class,rewards::{CATALOG,ItemKind}};
        for class in Class::ALL {
            for seed in 0..64 {
                let rows=stock(&Run::new(seed,class));
                assert_eq!(rows.len(),STOCK_COUNT as usize);
                assert_eq!(rows.iter().map(|s|(s.kind,s.id)).collect::<std::collections::HashSet<_>>().len(),rows.len());
                for kind in [ItemKind::Sorcery,ItemKind::Incantation,ItemKind::Armor] {
                    assert_eq!(rows.iter().filter(|s|CATALOG.iter().any(|c|c.kind==kind&&c.id==erdtree_tower::affixes::base(if s.kind==1{0x10000000|s.id as u32}else{0x40000000|s.id as u32})&&s.kind==if kind==ItemKind::Armor {1}else{3})).count(),3);
                }
                assert_eq!(rows.iter().filter(|s|s.kind==0&&s.quantity==1).count(),5);
                let spirit=rows.iter().filter(|s|s.kind==3&&erdtree_tower::rewards::lookup(0x40000000|s.id as u32).is_some_and(|c|c.kind==ItemKind::Spirit)).count();
                let crystal=rows.iter().any(|s|s.kind==3&&(11000..=11031).contains(&s.id));
                assert_eq!(spirit,usize::from(!crystal&&hash(seed ^ 0xa549)%4==0));
                for (group,quantity) in [("food",5),("pot",3),("knife",20)] {
                    let pool=extras(group);
                    let selected:Vec<_>=rows.iter().filter(|s|pool.iter().any(|p|p.kind==s.kind&&p.id==s.id)).collect();
                    assert_eq!(selected.len(),1+usize::from(group=="food"&&spirit==0&&!crystal)+usize::from(group=="food"&&!rows.iter().any(|s|s.kind==3&&s.id==8000)));
                    assert_eq!(selected[0].quantity,quantity);
                }
                let catalyst=match class {Class::Astrologer|Class::Prisoner=>Some("staff"),Class::Prophet|Class::Confessor=>Some("seal"),_=>None};
                if let Some(group)=catalyst {
                    assert_eq!(rows.iter().filter(|s|extras(group).iter().any(|p|p.kind==s.kind&&p.id as u32==erdtree_tower::affixes::base(s.id as u32))).count(),2);
                }
            }
        }
        let flags:Vec<_>=(0..STOCK_COUNT).flat_map(|i|stock_flag(i)..stock_flag(i)+8).collect();
        assert!(flags.iter().all(|f|*f<super::super::spirits::SUMMONED_FLAG));
        assert_eq!(flags.iter().collect::<std::collections::HashSet<_>>().len(),flags.len());
    }
    #[test]
    fn shop_spirits_cover_all_levels_in_every_act_and_keep_their_price() {
        use erdtree_tower::rewards::{lookup,ItemKind};
        for act in 1..=3 {
            let mut levels=std::collections::BTreeSet::new();
            for seed in 0..300 {
                let mut run=Run::new(seed,erdtree_tower::Class::Samurai);run.act=act;
                let first=stock(&run);let second=stock(&run);
                assert!(first.iter().zip(second.iter()).all(|(a,b)|a.id==b.id&&a.kind==b.kind));
                for item in first.iter().filter(|s|s.kind==3&&lookup(0x40000000|s.id as u32).is_some_and(|i|i.kind==ItemKind::Spirit)) {
                    assert_eq!(item.price,2000);assert!((0..=10).contains(&(item.id%1000)));levels.insert(item.id%1000);
                }
            }
            assert_eq!(levels,(0..=10).collect());
        }
    }
    #[test]
    fn same_shop_is_stable_next_shop_has_different_stock_and_all_categories() {
        let mut run = Run::new(42, erdtree_tower::Class::Samurai);
        let first = stock(&run);
        assert_eq!(first, stock(&run));
        assert_eq!(first.len(), STOCK_COUNT as usize);
        assert_eq!(first.iter().filter(|s|s.kind==2).count(),2);
        assert!(first.iter().all(|s|s.kind!=3 || !(9500000..9500008).contains(&s.id)));
        assert_eq!(first.iter().filter(|s| s.kind == 0&&s.quantity==1).count(), 5);
        assert_eq!(first.iter().filter(|s| s.kind == 4).count(), 2);
        let shop = run
            .nodes
            .iter()
            .position(|n| n.kind == NodeKind::Shop)
            .unwrap();
        run.visited.push(shop);
        assert_ne!(first, stock(&run));
        assert!(stock(&run).iter().all(|s| s.price > 0));
    }
    #[test]
    fn capped_blessings_are_not_sold_and_shop_remains_full() {
        let mut run = Run::new(1, erdtree_tower::Class::Samurai);
        run.blessings = BlessingKind::ALL
            .iter()
            .map(|&kind| Blessing { kind, level: 3 })
            .collect();
        let rows = stock(&run);
        assert_eq!(rows.len(), STOCK_COUNT as usize);
        assert_eq!(
            rows.iter()
                .map(|s| (s.kind, s.id))
                .collect::<std::collections::HashSet<_>>()
                .len(),
            STOCK_COUNT as usize
        );
        assert!(
            !rows
                .iter()
                .any(|s| (GOODS..GOODS + 8).contains(&(s.id as u32)))
        );
    }
}
