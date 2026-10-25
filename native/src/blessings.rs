use eldenring::cs::{ChrIns, ChrInsExt, EquipParamAccessory, ItemCategory, PlayerGameData, SoloParamRepository};
use erdtree_tower::{Blessing, BlessingKind, Run};
use fromsoftware_shared::FromStatic;
use std::collections::BTreeSet;

pub fn talisman(kind:BlessingKind,level:u8)->(u32,&'static str) {
    use BlessingKind::*;
    let rows=match kind {
        Guard=>[(4000,"龙徽盾护符"),(4001,"龙徽盾护符＋１"),(4002,"龙徽盾护符＋２")],
        Charge=>[(2130,"斧护符"),(2180,"钩爪护符"),(2060,"矛护符")],
        Dodge=>[(1150,"绿龟护符"),(1030,"众武护符"),(1032,"大壶众武护符")],
        Bleed=>[(2160,"鲜血君王的欢愉"),(2080,"带翼剑徽章"),(2081,"腐败翼剑徽章")],
        Sorcery=>[(3000,"魔法师球护符"),(3001,"魔法师块护符"),(3070,"拉达冈的肖像")],
        Flame=>[(2020,"火对蝎"),(4021,"火龙徽护符＋１"),(4022,"火龙徽护符＋２")],
        Vigor=>[(1000,"红琥珀链坠"),(1001,"红琥珀链坠＋１"),(1002,"红琥珀链坠＋２")],
        Endurance=>[(1020,"绿琥珀链坠"),(1021,"绿琥珀链坠＋１"),(1022,"绿琥珀链坠＋２")],
    };
    rows[level.clamp(1,3) as usize-1]
}
pub fn description(kind:BlessingKind,level:u8)->String {
    format!("护符：{}（放入背包，自行装备后生效）",talisman(kind,level).1)
}
fn pending(run:&Run)->Vec<u32> {
    run.blessings.iter().flat_map(|b|(1..=b.level).map(move |level|talisman(b.kind,level).0))
        .filter(|id|!run.delivered_talismans.contains(id)).collect()
}
fn owns(data:&PlayerGameData,id:u32)->bool {
    let matches=|e:&eldenring::cs::EquipInventoryData|e.items_data.items()
        .any(|i|i.quantity>0&&i.item_id.category()==ItemCategory::Accessory&&i.item_id.param_id()==id);
    matches(&data.equipment.equip_inventory_data)||data.storage.as_ref().is_some_and(|s|matches(s))
}
fn effects(kind: BlessingKind, level: u8) -> Vec<i32> {
    use BlessingKind::*;
    let tier = i32::from(level.clamp(1, 3) - 1);
    match kind {
        Guard => vec![340000 + tier * 10, 322000],
        Charge => [321300, 321800, 320600][..level.clamp(1, 3) as usize].to_vec(),
        Dodge => {
            if level == 1 {
                vec![311500]
            } else {
                vec![311500, if level == 2 { 310300 } else { 310320 }]
            }
        }
        Bleed => {
            if level == 1 {
                vec![321600]
            } else if level == 2 {
                vec![321600, 320800, 320801, 320802, 320803]
            } else {
                vec![321600, 320810, 320811, 320812, 320813]
            }
        }
        Sorcery => {
            if level == 1 {
                vec![330000]
            } else if level == 2 {
                vec![330010]
            } else {
                vec![330010, 330700]
            }
        }
        Flame => vec![320200, 340200 + tier * 10],
        Vigor => vec![310000 + tier * 10],
        Endurance => vec![310200 + tier * 10],
    }
}


pub struct Blessings;
impl Blessings {
    pub fn clear_run(chr:&mut ChrIns) {
        let ids:BTreeSet<_>=BlessingKind::ALL.iter().flat_map(|&kind|(1..=3).flat_map(move |level|effects(kind,level))).collect();
        for id in ids {chr.remove_speffect(id);}
    }
    pub fn validate(blessings:&[Blessing])->Result<(),String> {
        let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"护符参数未就绪")?;
        for b in blessings {
            for level in 1..=b.level {
                let id=talisman(b.kind,level).0;
                if params.get::<EquipParamAccessory>(id).is_none(){return Err(format!("原版护符 {id} 不存在"));}
            }
        }
        Ok(())
    }
    pub fn deliver(data:&mut PlayerGameData,run:&mut Run)->Result<(),String> {
        Self::validate(&run.blessings)?;
        for id in pending(run) {
            if !owns(data,id) {
                super::items::give(&[((0x20000000|id) as i32,1,-1)])?;
                if !owns(data,id){return Err(format!("护符 {id} 尚未进入背包，请重试"));}
            }
            run.delivered_talismans.push(id);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rewards_are_real_accessories_and_delivery_survives_resume() {
        let mut run=Run::new(42,erdtree_tower::Class::Samurai);
        run.blessings.push(Blessing{kind:BlessingKind::Charge,level:2});
        assert_eq!(pending(&run),vec![2130,2180]);
        run.delivered_talismans.push(2130);
        assert_eq!(pending(&Run::decode(&run.encode().unwrap()).unwrap()),vec![2180]);
        run.delivered_talismans.push(2180);
        assert!(pending(&run).is_empty());
        for kind in BlessingKind::ALL {
            for level in 1..=3 {
                assert!(talisman(kind,level).0<9500000);
                assert!(description(kind,level).contains("自行装备"));
            }
        }
    }
}
