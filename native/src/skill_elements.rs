// 战技元素锁：亵渎圣剑「掠夺火焰」、蕾拉娜的对剑「月与火的架式」等武器的招式攻击行只乘武器的某一种元素
// （火浪只乘火、对剑的月光招只乘魔/火焰招只乘火）。高级词条把这类武器的元素整体换成别的元素、或变成纯元素无物理时，
// 同一行 AtkParam 无法同时适配：按参数算，战技为原版的 33%～263%、对剑普攻/重击 76%～139%（掠夺火焰纯圣变体约 255%）。
// 这里在游戏读入参数后，把这些武器所有词条变体的基础攻击力按原版的物理/元素比例重新分配（总攻击力不变，
// 品质、补正、词条效果都保留），于是每一招都回到原版的伤害比例。只改本机内存，联机各人都装同一补丁即可一致。
// 作者回来后应改在 scene-builder/AdvancedAffixAssets.cs 里生成时就不换元素（见 skill-element-lock-vs-42e56f9 补丁）。
use eldenring::cs::{EquipParamWeapon, SoloParamRepository};
use fromsoftware_shared::FromStatic;
use std::sync::atomic::{AtomicBool, Ordering};

/// 原版武器 ID（各品质/词条变体经 affixes::base 映射回它）
pub const LOCKED: [u32; 12] = [
    3140000,  // 亵渎圣剑（掠夺火焰）
    3170000,  // 黄金律法大剑
    11150000, // 玛莉卡的石槌
    12200000, // 吞世权杖
    13020000, // 家族头连枷
    14120000, // 罗泽司的斧
    16040000, // 尊腐骑士矛
    16120000, // 死亡仪式矛
    18100000, // 罗蕾塔的战镰
    21060000, // 接肢飞龙
    2140000,  // 夜与火之剑（魔/火分招）
    67520000, // 蕾拉娜的对剑（魔/火分招，含普攻与重击）
];

/// 按原版比例重新分配总攻击力；总数保持不变（四舍五入的误差补到物理或最大的一项）
pub fn redistribute(vanilla: [u16; 5], current: [u16; 5]) -> [u16; 5] {
    let total: u32 = current.iter().map(|&v| u32::from(v)).sum();
    let base: u32 = vanilla.iter().map(|&v| u32::from(v)).sum();
    if total == 0 || base == 0 { return current; }
    let mut out = [0u16; 5];
    let mut used = 0u32;
    for i in 0..5 {
        let v = (u64::from(vanilla[i]) * u64::from(total) + u64::from(base) / 2) / u64::from(base);
        out[i] = v.min(u64::from(u16::MAX)) as u16;
        used += u32::from(out[i]);
    }
    let biggest = (0..5).max_by_key(|&i| vanilla[i]).unwrap();
    let fixed = i64::from(out[biggest]) + i64::from(total) - i64::from(used);
    out[biggest] = fixed.clamp(0, i64::from(u16::MAX)) as u16;
    out
}

fn bases(w: &eldenring::param::EQUIP_PARAM_WEAPON_ST) -> [u16; 5] {
    [w.attack_base_physics(), w.attack_base_magic(), w.attack_base_fire(), w.attack_base_thunder(), w.attack_base_dark()]
}

/// 每帧调用，参数就绪后只做一次
pub fn apply_once() {
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.load(Ordering::Relaxed) { return; }
    let Ok(params) = (unsafe { SoloParamRepository::instance_mut() }) else { return };
    let mut vanilla = Vec::new();
    for id in LOCKED {
        let Some(w) = params.get::<EquipParamWeapon>(id) else { return };
        vanilla.push((id, bases(w)));
    }
    let (mut rows, mut changed) = (0usize, 0usize);
    let mut examples = Vec::new();
    for (id, row) in params.rows_mut::<EquipParamWeapon>() {
        if id < 100_000_000 { continue; }
        let base = erdtree_tower::affixes::base(id / 10000 * 10000);
        let Some(&(_, original)) = vanilla.iter().find(|(b, _)| *b == base) else { continue };
        rows += 1;
        let now = bases(row);
        let fixed = redistribute(original, now);
        if fixed == now { continue; }
        row.set_attack_base_physics(fixed[0]); row.set_attack_base_magic(fixed[1]); row.set_attack_base_fire(fixed[2]);
        row.set_attack_base_thunder(fixed[3]); row.set_attack_base_dark(fixed[4]);
        changed += 1;
        if id % 10000 == 0 && examples.len() < 6 { examples.push(format!("{id}:{now:?}->{fixed:?}")); }
    }
    DONE.store(true, Ordering::Relaxed);
    crate::log(format!("SKILL_ELEMENT_LOCK variants={rows} rebalanced={changed} e.g. {}", examples.join(" ")));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_total_and_restores_vanilla_split() {
        // 亵渎圣剑原版 121 物理 + 78 火；纯圣变体 236 → 物理/火按原版比例
        let out = redistribute([121, 0, 78, 0, 0], [0, 0, 0, 0, 236]);
        assert_eq!(out.iter().map(|&v| u32::from(v)).sum::<u32>(), 236);
        assert_eq!(out, [143, 0, 93, 0, 0]);
        // 蕾拉娜的对剑原版 103/66/66；火属性变体 126/0/103
        let out = redistribute([103, 66, 66, 0, 0], [126, 0, 103, 0, 0]);
        assert_eq!(out.iter().map(|&v| u32::from(v)).sum::<u32>(), 229);
        assert_eq!(out, [101, 64, 64, 0, 0]);
        // 已经是原版比例的普通词条变体不动
        assert_eq!(redistribute([121, 0, 78, 0, 0], [125, 0, 81, 0, 0]), [125, 0, 81, 0, 0]);
        assert_eq!(redistribute([121, 0, 78, 0, 0], [0, 0, 0, 0, 0]), [0, 0, 0, 0, 0]);
    }
}
