//! 黄金树之影 DLC 内容开关（启动参数 dlc=full|none|weapons，见 options.rs）。
//! 只影响“从池子里挑”：奖励、宝箱、商店、盲盒、精英与 Boss 奖励、随机战灰、敌人池；
//! 已经拿到的物品照常显示和使用，存档校验也不看这个开关。联机时由房主在握手里定下整队模式：
//! 所有人都有 DLC 才按 Full，有一个人没有就整队按 None，保证双方抽到同样的内容（party_handshake.rs）。
//!
//! 哪些物品属于 DLC 按编号判断（用 Paramdex 的物品名逐段核对过）：
//!   武器  去掉强化/质变位后 ≥ 60000000（DLC 新类别），或在原有类别的后半段（百万位以下 ≥ 500000，如 1500000 Main-gauche）
//!   防具  3000000 起（2000000–2030000 是本体的腐败决斗者、蘑菇冠等）
//!   护符  7000–99999（+3 琥珀坠饰、双头龟、Rellana's Cameo 等；模组自己的随机护符是 960100 起，不算）
//!   道具  2000000 起（DLC 法术、骨灰、结晶露滴、壶；本体最大 263010）
//!   战灰  100000 起（本体最大 85000）
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// 有 DLC：全部内容
    Full,
    /// 没有 DLC：去掉 DLC 敌人和全部 DLC 物品
    None,
    /// 没有 DLC，但保留 DLC 武器（实验：看没买 DLC 的账号能不能正常用 DLC 武器）
    Weapons,
}

impl Mode {
    pub fn id(self) -> &'static str {
        match self {
            Mode::Full => "full",
            Mode::None => "none",
            Mode::Weapons => "weapons",
        }
    }
    pub fn parse(text: &str) -> Option<Mode> {
        [Mode::Full, Mode::None, Mode::Weapons].into_iter().find(|m| m.id().eq_ignore_ascii_case(text.trim()))
    }
}

static MODE: AtomicU8 = AtomicU8::new(0);

#[cfg(test)]
thread_local! {
    /// 测试并行运行：各测试只改自己线程的模式，不影响别的测试
    static TEST_MODE: std::cell::Cell<Option<Mode>> = const { std::cell::Cell::new(None) };
}

/// 测试用：在当前线程临时换模式
#[cfg(test)]
pub fn with_mode<T>(mode: Mode, f: impl FnOnce() -> T) -> T {
    TEST_MODE.with(|m| m.set(Some(mode)));
    let result = f();
    TEST_MODE.with(|m| m.set(None));
    result
}

/// DLL 启动时按启动参数设置；联机时按整队模式切换（native options::sync_dlc）
pub fn set(mode: Mode) {
    MODE.store(mode as u8, Ordering::Relaxed);
}
pub fn mode() -> Mode {
    #[cfg(test)]
    if let Some(mode) = TEST_MODE.with(|m| m.get()) {
        return mode;
    }
    match MODE.load(Ordering::Relaxed) {
        1 => Mode::None,
        2 => Mode::Weapons,
        _ => Mode::Full,
    }
}

/// 带类别标记的物品编号（武器 0、防具 0x10000000、护符 0x20000000、道具 0x40000000）是否属于 DLC；词条装备看它的底子
pub fn is_dlc_item(id: u32) -> bool {
    let id = crate::affixes::base(id);
    let raw = id & 0x0fff_ffff;
    match id >> 28 {
        0 => is_dlc_weapon(raw),
        1 => (3_000_000..100_000_000).contains(&raw),
        2 => (7000..100_000).contains(&raw),
        4 => (2_000_000..100_000_000).contains(&raw),
        _ => false,
    }
}

fn is_dlc_weapon(raw: u32) -> bool {
    let base = raw - raw % 10_000;
    (60_000_000..100_000_000).contains(&base) || (base < 60_000_000 && base % 1_000_000 >= 500_000)
}

/// 战灰（EquipParamGem 编号）是否属于 DLC
pub fn is_dlc_gem(gem: u32) -> bool {
    gem >= 100_000
}

/// 这件物品能不能进当前的抽取池
pub fn item_allowed(id: u32) -> bool {
    match mode() {
        Mode::Full => true,
        Mode::Weapons if id >> 28 == 0 => true,
        _ => !is_dlc_item(id),
    }
}

/// 这个战灰能不能被随机抽到（兼容武器模式也只保留武器本身，DLC 战灰一并去掉）
pub fn gem_allowed(gem: u32) -> bool {
    mode() == Mode::Full || !is_dlc_gem(gem)
}

/// DLC 敌人能不能出现
pub fn enemies_allowed() -> bool {
    mode() == Mode::Full
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_items_are_classified_like_the_game() {
        // DLC：Backhand Blade、Main-gauche（+质变/强化）、Velvet Sword、Shadow Sunflower Blossom、Dryleaf Arts
        for weapon in [64_500_000, 1_500_000, 1_500_725, 2_510_000, 23_510_000, 60_500_000] {
            assert!(is_dlc_item(weapon), "{weapon}");
        }
        // 本体：匕首、长剑（含质变强化）、Lazuli 一带的防具、本体护符、本体法术
        for base in [1_000_000, 2_000_000, 2_000_825, 4_100_000, 0x1000_0000 | 2_030_000, 0x2000_0000 | 6110, 0x4000_0000 | 4200, 0x4000_0000 | 263_010] {
            assert!(!is_dlc_item(base), "{base:#x}");
        }
        // DLC：Dane's Hat、Oathseeker Knight Helm、+3 琥珀坠饰、Rellana's Cameo、Rellana's Twin Moons、骨灰
        for dlc in [0x1000_0000 | 3_000_000, 0x1000_0000 | 5_000_000, 0x2000_0000 | 7000, 0x2000_0000 | 8220, 0x4000_0000 | 2_004_320, 0x4000_0000 | 2_201_001] {
            assert!(is_dlc_item(dlc), "{dlc:#x}");
        }
        // 模组自己的随机护符不是 DLC
        assert!(!is_dlc_item(0x2000_0000 | 960_100));
        assert!(is_dlc_gem(200_000) && is_dlc_gem(409_000) && !is_dlc_gem(85_000));
    }

    #[test]
    fn every_reward_catalog_entry_is_classified_without_panicking() {
        let dlc = crate::rewards::CATALOG.iter().filter(|i| is_dlc_item(i.id)).count();
        assert!(dlc > 0 && dlc < crate::rewards::CATALOG.len());
    }

    /// 一大批对局里所有奖励页、盲盒、试炼奖励：无 DLC 时一件 DLC 物品都不出；兼容武器模式只放行 DLC 武器
    fn drawn_items(mode: Mode) -> Vec<u32> {
        with_mode(mode, || {
            let mut items = Vec::new();
            for seed in 0..160 {
                for class in [crate::Class::Samurai, crate::Class::Astrologer, crate::Class::Prophet] {
                    let run = crate::Run::new(seed, class);
                    for node in 0..run.nodes.len() {
                        for reward in crate::rewards::offers(&run, node) {
                            if let crate::Reward::Item(id) = reward {
                                items.push(id);
                            }
                        }
                        items.extend(crate::affixes::mystery_box(&run, node, seed % 2 == 0));
                    }
                    items.push(crate::regional_events::descent_reward(seed, 3));
                }
            }
            items
        })
    }

    #[test]
    fn without_the_dlc_no_dlc_item_is_ever_drawn() {
        let full = drawn_items(Mode::Full);
        assert!(full.iter().any(|id| is_dlc_item(*id)), "DLC items are part of the normal pools");
        assert!(drawn_items(Mode::None).iter().all(|id| !is_dlc_item(*id)));
        let weapons = drawn_items(Mode::Weapons);
        assert!(weapons.iter().all(|id| !is_dlc_item(*id) || crate::affixes::base(*id) >> 28 == 0));
        assert!(weapons.iter().any(|id| is_dlc_item(*id)), "DLC weapons stay in the compatibility mode");
    }

    #[test]
    fn modes_round_trip_through_their_ids() {
        for mode in [Mode::Full, Mode::None, Mode::Weapons] {
            assert_eq!(Mode::parse(mode.id()), Some(mode));
        }
        assert_eq!(Mode::parse("NONE"), Some(Mode::None));
        assert_eq!(Mode::parse("maybe"), None);
    }
}
