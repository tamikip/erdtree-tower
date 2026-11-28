use eldenring::cs::{EquipGameData, EquipInventoryData, EquipInventoryDataListEntry, InventoryItemsData, PlayerGameData};
use std::{
    collections::HashMap,
    ffi::c_void,
    mem::{offset_of, size_of},
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetCurrentProcess() -> *mut c_void;
    fn ReadProcessMemory(process: *mut c_void, address: *const c_void, out: *mut c_void, len: usize, got: *mut usize) -> i32;
}

// UI jobs can be replaced while the text renderer is reading a description.
fn read<const N: usize>(address: usize) -> Option<[u8; N]> {
    if address < 0x10000 {
        return None;
    }
    let mut bytes = [0; N];
    let mut got = 0;
    let ok = unsafe { ReadProcessMemory(GetCurrentProcess(), address as *const c_void, bytes.as_mut_ptr().cast(), N, &mut got) };
    (ok != 0 && got == N).then_some(bytes)
}
fn ptr(address: usize) -> Option<usize> {
    Some(usize::from_le_bytes(read(address)?))
}
fn word(address: usize) -> Option<u32> {
    Some(u32::from_le_bytes(read(address)?))
}

fn selection(base: usize, query: u32) -> Option<(u32, u32, u32)> {
    // Verified retail 2.7.2 equipment list and inventory dialog, including two equal param IDs.
    let menu = ptr(base + 0x3d72840)?;
    let popup = ptr(menu + 0x80)?;
    let top = ptr(popup + 0x10)?;
    if ptr(top)? != base + 0x2b02710 {
        return None;
    }
    let mut dialog = ptr(top + 0x58)?;
    if ptr(dialog)? == base + 0x2af2b18 {
        dialog = ptr(dialog + 0x58)?;
    }
    let ty = ptr(dialog)?;
    if ty != base + 0x2af3f10 && ty != base + 0x2afccc8 {
        return None;
    }
    let detail = dialog + 0xe88;
    if ptr(detail)? != base + 0x2b24440 {
        return None;
    }
    let record = read::<12>(detail + 0xec8)?;
    let [slot, category, param] = [0, 4, 8].map(|i| u32::from_le_bytes([record[i], record[i + 1], record[i + 2], record[i + 3]]));
    let matches = match category {
        0 => query / 10000 == param / 10000,
        0x20000000 => query == param,
        _ => false,
    };
    if !matches {
        return None;
    }
    let game = ptr(base + 0x3d64fb8)?;
    let player = ptr(game + 8)?;
    let bag = player
        + offset_of!(PlayerGameData, equipment)
        + offset_of!(EquipGameData, equip_inventory_data)
        + offset_of!(EquipInventoryData, items_data);
    let normal = slot.checked_sub(word(bag + offset_of!(InventoryItemsData, key_items_capacity))?)?;
    if normal >= word(bag + offset_of!(InventoryItemsData, normal_items_len))? {
        return None;
    }
    let entry = ptr(bag + offset_of!(InventoryItemsData, normal_items_head))? + normal as usize * size_of::<EquipInventoryDataListEntry>();
    let id = word(entry + offset_of!(EquipInventoryDataListEntry, item_id))?;
    if id != (category | param) || word(entry + offset_of!(EquipInventoryDataListEntry, quantity))? == 0 {
        return None;
    }
    let character = word(player + offset_of!(PlayerGameData, character_id))?;
    let sort = word(entry + offset_of!(EquipInventoryDataListEntry, sort_id))?;
    let (character, sort) = super::card_affixes::identity(character, id, sort);
    Some((character, id, sort))
}

fn exact_description_item(item: u32, query: u32) -> bool {
    match item >> 28 {
        2 => item == (0x20000000 | query),
        0 => item == query,
        _ => false,
    }
}

fn same_description_item(item: u32, query: u32) -> bool {
    match item >> 28 {
        2 => item == (0x20000000 | query),
        0 => item / 10000 == query / 10000,
        _ => false,
    }
}

fn row_entry(row: &[u8]) -> Option<(u32, u32)> {
    let field = |offset| u32::from_le_bytes(row[offset..offset + 4].try_into().unwrap());
    let quantity = field(offset_of!(EquipInventoryDataListEntry, quantity));
    if quantity == 0 {
        return None;
    }
    let item = field(offset_of!(EquipInventoryDataListEntry, item_id));
    let sort = field(offset_of!(EquipInventoryDataListEntry, sort_id));
    Some((item, sort))
}

/// 当候选项非空且解析后的词条身份完全一致（单件或灵庙复制的多件同词条武器）时返回该身份；存在多种不同词条时返回 None 避免误猜。
fn common_identity(
    candidates: impl IntoIterator<Item = (u32, u32)>,
    character: u32,
    resolve: &impl Fn(u32, u32, u32) -> (u32, u32),
) -> Option<(u32, u32, u32)> {
    let mut matched: Option<(u32, u32, u32)> = None;
    for (item, sort) in candidates {
        let (rc, rs) = resolve(character, item, sort);
        match matched {
            None => matched = Some((rc, item, rs)),
            Some((mc, _, ms)) if mc == rc && ms == rs => {}
            Some(_) => return None,
        }
    }
    matched
}

const EQUIPPED_AFFIX_SLOTS: [usize; 10] = [0, 1, 2, 3, 4, 5, 17, 18, 19, 20];

fn resolve_fallback(
    rows: &[u8],
    key_cap: u32,
    equipped_slots: &[u32; 22],
    query: u32,
    character: u32,
    resolve: impl Fn(u32, u32, u32) -> (u32, u32),
) -> Option<(u32, u32, u32)> {
    let stride = size_of::<EquipInventoryDataListEntry>();
    let count = rows.len() / stride;

    let equipped: Vec<(u32, u32)> = EQUIPPED_AFFIX_SLOTS
        .into_iter()
        .filter_map(|slot| {
            let normal = equipped_slots[slot].checked_sub(key_cap)? as usize;
            if normal >= count {
                return None;
            }
            row_entry(&rows[normal * stride..(normal + 1) * stride])
        })
        .collect();

    let eq_exact: Vec<_> = equipped.iter().copied().filter(|&(item, _)| exact_description_item(item, query)).collect();
    if !eq_exact.is_empty() {
        return common_identity(eq_exact, character, &resolve);
    }

    let eq_variant: Vec<_> = equipped.iter().copied().filter(|&(item, _)| same_description_item(item, query)).collect();
    if !eq_variant.is_empty() {
        return common_identity(eq_variant, character, &resolve);
    }

    let bag_exact: Vec<_> = rows
        .chunks_exact(stride)
        .filter_map(row_entry)
        .filter(|&(item, _)| exact_description_item(item, query))
        .collect();
    if !bag_exact.is_empty() {
        return common_identity(bag_exact, character, &resolve);
    }

    let bag_variant: Vec<_> = rows
        .chunks_exact(stride)
        .filter_map(row_entry)
        .filter(|&(item, _)| same_description_item(item, query))
        .collect();
    common_identity(bag_variant, character, &resolve)
}

fn pause_equipment_menu_open(base: usize) -> bool {
    let Some(menu) = ptr(base + 0x3d72840) else { return false };
    let Some(popup) = ptr(menu + 0x80) else { return false };
    let Some(top) = ptr(popup + 0x10) else { return false };
    if ptr(top) != Some(base + 0x2b02710) {
        return false;
    }
    let Some(dialog) = ptr(top + 0x58) else { return false };
    let Some(vtable) = ptr(dialog) else { return false };
    [base + 0x2af2b18, base + 0x2afccc8, base + 0x2af3f10].contains(&vtable)
}

fn inventory_fallback(base: usize, query: u32) -> Option<(u32, u32, u32)> {
    if !pause_equipment_menu_open(base) && !super::native_talk::owned_menu_open() {
        return None;
    }
    let game = ptr(base + 0x3d64fb8)?;
    let player = ptr(game + 8)?;
    let equip = player + offset_of!(PlayerGameData, equipment);
    let bag = equip
        + offset_of!(EquipGameData, equip_inventory_data)
        + offset_of!(EquipInventoryData, items_data);
    let key_cap = word(bag + offset_of!(InventoryItemsData, key_items_capacity))?;
    let len = word(bag + offset_of!(InventoryItemsData, normal_items_len))?;
    let capacity = word(bag + offset_of!(InventoryItemsData, normal_items_capacity))?;
    if len == 0 || len > capacity || len > 16384 {
        return None;
    }
    let idx_bytes = read::<88>(equip + offset_of!(EquipGameData, equipment_item_idx_list))?;
    let equipped_slots: [u32; 22] = std::array::from_fn(|i| {
        u32::from_le_bytes(idx_bytes[i * 4..(i + 1) * 4].try_into().unwrap())
    });
    let head = ptr(bag + offset_of!(InventoryItemsData, normal_items_head))?;
    let mut rows = vec![0u8; len as usize * size_of::<EquipInventoryDataListEntry>()];
    let mut got = 0;
    if unsafe { ReadProcessMemory(GetCurrentProcess(), head as *const c_void, rows.as_mut_ptr().cast(), rows.len(), &mut got) } == 0
        || got != rows.len()
    {
        return None;
    }
    let character = word(player + offset_of!(PlayerGameData, character_id))?;
    let resolved = resolve_fallback(&rows, key_cap, &equipped_slots, query, character, super::card_affixes::identity)?;
    if ptr(game + 8)? != player
        || ptr(bag + offset_of!(InventoryItemsData, normal_items_head))? != head
        || word(bag + offset_of!(InventoryItemsData, normal_items_len))? != len
    {
        return None;
    }
    Some(resolved)
}

fn shop_fallback(query: u32) -> Option<(u32, u32, u32)> {
    if !super::native_talk::shop_menu_open() {
        return None;
    }
    let item = if erdtree_tower::talisman_passives::variant(query | 0x20000000).is_some() {
        query | 0x20000000
    } else {
        query
    };
    let (a, b) = super::card_affixes::shop_offer(item)?;
    Some((a, item, b))
}

fn replace_note(original: &str, character: u32, id: u32, sort: u32) -> Option<String> {
    let (start, lang) = ["随机附加词条 ×", "隨機附加詞條 ×", "Random affixes: "]
        .iter()
        .enumerate()
        .find_map(|(i, marker)| original.rfind(marker).map(|p| (p, i)))?;
    let lines = roll_lines(character, id, sort, lang)?;
    let suffix = original[start..].find(['<', '\n', '\r']).map_or("", |end| &original[start + end..]);
    Some(format!("{}{}{}", &original[..start], lines.join("\n"), suffix))
}

pub fn roll_lines(character: u32, id: u32, sort: u32, lang: usize) -> Option<Vec<String>> {
    let effects: Vec<u32> = if let Some(v) = erdtree_tower::talisman_passives::variant(id) {
        erdtree_tower::talisman_passives::instance_effects(v, character, sort)
            .map(|e| e as u32)
            .collect()
    } else {
        erdtree_tower::weapon_rolls::instance(character, sort, id)?.effects
    };
    static SPECIAL: LazyLock<Vec<Vec<&str>>> = LazyLock::new(|| {
        include_str!("../../scene-assets/weapon-trait-text.tsv")
            .lines()
            .map(|l| l.split('\t').collect())
            .collect()
    });
    let mut lines = Vec::new();
    for effect in effects {
        let label = if effect >= 9875100 {
            SPECIAL.get((effect - 9875100) as usize)?.get(lang)?.to_string()
        } else {
            let label = *super::reward_ui::TALISMAN_EFFECT_TEXT.get(&(effect as i32))?;
            match lang {
                1 => erdtree_tower::localization::to_traditional(label).into_owned(),
                2 => erdtree_tower::localization::translate_en(label).into_owned(),
                _ => label.into(),
            }
        };
        lines.push(label);
    }
    Some(lines)
}

type Lookup = unsafe extern "C" fn(usize, i32) -> *const u16;
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static BASE: AtomicUsize = AtomicUsize::new(0);
type TextKey = (usize, u32, u32, u32);
static TEXT: LazyLock<Mutex<HashMap<TextKey, Box<[u16]>>>> = LazyLock::new(Default::default);

unsafe extern "C" fn lookup(table: usize, query: i32) -> *const u16 {
    let original: Lookup = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    let raw = unsafe { original(table, query) };
    // HansFix 惜别泪滴：新增祷告在原版文本表里没有条目
    if raw.is_null() && query == super::tears::SPELL {
        if let Some(text) = super::tears::text(table, &|t, q| unsafe { original(t, q) }) {
            return text;
        }
    }
    if raw.is_null() || query < 0 {
        return raw;
    }
    let id = query as u32;
    if erdtree_tower::weapon_rolls::slot_count(id / 10000 * 10000).is_none()
        && erdtree_tower::talisman_passives::variant(id | 0x20000000).is_none()
    {
        return raw;
    }
    let base = BASE.load(Ordering::Relaxed);
    let selected = selection(base, id).or_else(|| {
        let len = (0..8192).position(|i| unsafe { *raw.add(i) == 0 })?;
        let text = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(raw, len) });
        if !["随机附加词条 ×", "隨機附加詞條 ×", "Random affixes: "]
            .iter()
            .any(|s| text.contains(s))
        {
            return None;
        }
        shop_fallback(id).or_else(|| inventory_fallback(base, id))
    });
    let Some((character, item, sort)) = selected else {
        return raw;
    };
    let Ok(mut cache) = TEXT.lock() else {
        return raw;
    };
    let key = (raw as usize, character, item, sort);
    if let Some(text) = cache.get(&key) {
        return text.as_ptr();
    }
    // Native FMG entries are NUL-terminated UTF-16 strings.
    let Some(len) = (0..8192).position(|i| unsafe { *raw.add(i) == 0 }) else {
        return raw;
    };
    let text = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(raw, len) });
    let Some(value) = replace_note(&text, character, item, sort) else {
        return raw;
    };
    if cache.len() >= 8192 {
        return raw;
    }
    if cache.len() < 128 {
        super::log(format!("ITEM_DETAIL item={item} sort={sort} character={character}"));
    }
    cache
        .entry(key)
        .or_insert_with(|| value.encode_utf16().chain([0]).collect())
        .as_ptr()
}

pub fn install() -> Result<(), String> {
    use hudhook::mh::{MH_EnableHook, MhHook};
    // Leaf FMG lookup, below MsgRepository (also used by transmog).
    let address = super::items::checked_function(
        0x2671c40,
        &[0x44, 0x8b, 0x41, 0x0c, 0x45, 0x33, 0xc9, 0x41, 0xff, 0xc8, 0x4c, 0x8b, 0xd9, 0x3b, 0x51, 0x2c],
    )? as *mut c_void;
    let hook = unsafe { MhHook::new(address, lookup as *mut c_void) }.map_err(|e| format!("{e:?}"))?;
    BASE.store(unsafe { GetModuleHandleW(std::ptr::null()) } as usize, Ordering::Relaxed);
    ORIGINAL.store(hook.trampoline() as usize, Ordering::Release);
    unsafe { MH_EnableHook(address) }.ok().map_err(|e| format!("{e:?}"))?;
    super::log("Native weapon and talisman descriptions enabled");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_row(item_id: u32, quantity: u32, sort_id: u32) -> Vec<u8> {
        let mut row = vec![0u8; size_of::<EquipInventoryDataListEntry>()];
        for (off, value) in [
            (offset_of!(EquipInventoryDataListEntry, item_id), item_id),
            (offset_of!(EquipInventoryDataListEntry, quantity), quantity),
            (offset_of!(EquipInventoryDataListEntry, sort_id), sort_id),
        ] {
            row[off..off + 4].copy_from_slice(&value.to_le_bytes());
        }
        row
    }

    #[test]
    fn unique_archers_talisman_is_resolved_without_guessing_duplicates() {
        let row = make_row(537971123, 1, 2868);
        let empty = [u32::MAX; 22];
        let identity = |c, _item, s| (c, s);
        assert_eq!(resolve_fallback(&row, 0, &empty, 1100211, 0, identity), Some((0, 537971123, 2868)));
        assert_eq!(resolve_fallback(&row, 0, &empty, 1100212, 0, identity), None);

        let mut two_diff = make_row(537971123, 1, 2868);
        two_diff.extend(make_row(537971123, 1, 2869));
        assert_eq!(resolve_fallback(&two_diff, 0, &empty, 1100211, 0, identity), None);

        let lines = roll_lines(0, 537971123, 2868, 0).unwrap();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn mausoleum_duplicated_weapons_with_shared_identity_resolve_cleanly() {
        // Two copies of 194070008 (later reinforced to +9/+10) with different sort_ids
        // both bound to the same inherited identity (42, 999) after Mausoleum copy.
        let mut rows = make_row(194070009, 1, 3001);
        rows.extend(make_row(194070010, 1, 3055));
        let mut equipped = [u32::MAX; 22];
        equipped[0] = 384;
        equipped[1] = 385;
        let resolve = |_c, _item, _s| (42, 999);
        assert_eq!(
            resolve_fallback(&rows, 384, &equipped, 194070000, 0, resolve),
            Some((42, 194070009, 999))
        );
        assert_eq!(
            resolve_fallback(&rows, 384, &equipped, 194070010, 0, resolve),
            Some((42, 194070010, 999))
        );
    }

    #[test]
    fn equipped_copy_takes_precedence_over_unequipped_duplicate_with_different_affixes() {
        let mut rows = make_row(194070008, 1, 3001);
        rows.extend(make_row(194070008, 1, 3055));
        let mut equipped = [u32::MAX; 22];
        equipped[0] = 385; // Only the second copy (sort=3055) is equipped.
        let identity = |c, _item, s| (c, s);
        assert_eq!(
            resolve_fallback(&rows, 384, &equipped, 194070000, 0, identity),
            Some((0, 194070008, 3055))
        );
    }

    #[test]
    fn selected_copy_uses_its_own_roll_and_keeps_native_text() {
        for (item, note) in [
            (537970914, "能提升血量的上限\n随机附加词条 ×2，装备后查看。"),
            (160030000, "补正组合：灵巧/智力\n随机附加词条 ×3，装备后查看。</font>"),
        ] {
            let a = replace_note(note, 0, item, 2598).unwrap();
            let b = replace_note(note, 0, item, 2599).unwrap();
            assert_ne!(a, b);
            assert_eq!(a, replace_note(note, 0, item, 2598).unwrap());
            assert!(a.starts_with(note.lines().next().unwrap()));
            assert_eq!(a.ends_with("</font>"), note.ends_with("</font>"));
            assert!(!a.contains("装备后查看"));
        }
    }
}
