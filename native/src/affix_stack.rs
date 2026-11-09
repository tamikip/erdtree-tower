// 六个武器装备栏与四件防具均计入词条。按原版已经挂上的效果补足份数，卸装时撤下补充效果。
use eldenring::cs::{ChrIns,ChrInsExt,EquipParamProtector,EquipParamWeapon,PlayerGameData,SoloParamRepository,SpEffectParam};
use fromsoftware_shared::FromStatic;

const AFFIX_FIRST:i32=9850000;
const AFFIX_SPAN:i32=210;
const FIRST:i32=9880000;
const SPAN:i32=1000;
const COPIES:usize=9;
pub const WEAPON_SLOTS:[usize;6]=[0,1,2,3,4,5];
fn affix(id:i32)->bool {(AFFIX_FIRST..AFFIX_FIRST+AFFIX_SPAN).contains(&id)}
fn ours(id:i32)->bool {(FIRST..FIRST+SPAN*(COPIES as i32+1)).contains(&id)&&(id-FIRST)%SPAN<AFFIX_SPAN}
fn copy(effect:i32,n:usize)->i32 {FIRST+n as i32*SPAN+effect-AFFIX_FIRST}
/// 身上的词条效果（含重复）→ 需要挂的复制效果
fn wanted(effects:&[i32],native:&[i32])->Vec<i32> {
    let mut counts=std::collections::BTreeMap::new();
    for &effect in effects.iter().filter(|e|affix(**e)) {*counts.entry(effect).or_insert(0usize)+=1;}
    counts.into_iter().flat_map(|(effect,count)|{
        let first=usize::from(native.contains(&effect));
        (first..count.min(COPIES+1)).map(move |n|copy(effect,n))
    }).collect()
}
/// 装备栏中的武器与防具，不包括背包、箭矢和护符。
fn equipped(data:&PlayerGameData,params:&SoloParamRepository)->Vec<i32> {
    let asm=&data.equipment.chr_asm;
    let mut effects=Vec::new();
    for slot in WEAPON_SLOTS {
        let Some(id)=asm.equipment_param_ids.get(slot).copied().filter(|id|*id>0) else {continue};
        let Some(weapon)=params.get::<EquipParamWeapon>(id as u32).or_else(||params.get::<EquipParamWeapon>(id as u32/100*100)) else {continue};
        effects.extend([weapon.resident_sp_effect_id(),weapon.resident_sp_effect_id1(),weapon.resident_sp_effect_id2()]);
    }
    for slot in 12..=15 {
        let Some(id)=asm.equipment_param_ids.get(slot).copied().filter(|id|*id>0) else {continue};
        let Some(armor)=params.get::<EquipParamProtector>(id as u32) else {continue};
        effects.extend([armor.resident_sp_effect_id(),armor.resident_sp_effect_id2(),armor.resident_sp_effect_id3()]);
    }
    effects
}
/// 每帧：按身上的重复词条挂上或撤下复制效果（参数缺失时不挂）
pub fn sync(chr:&mut ChrIns,data:&PlayerGameData) {
    if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag() {return;}
    let Ok(params)=(unsafe {SoloParamRepository::instance()}) else{return};
    let active:Vec<i32>=chr.special_effect.entries().map(|e|e.param_id).collect();
    let want=wanted(&equipped(data,params),&active);
    let present:Vec<i32>=active.into_iter().filter(|id|ours(*id)).collect();
    for &id in present.iter().filter(|id|!want.contains(id)) {chr.remove_speffect(id);}
    for &id in want.iter().filter(|id|!present.contains(id)) {
        if params.get::<SpEffectParam>(id as u32).is_some() {chr.apply_speffect(id,false);}
        else {super::log_repeating("AFFIX_EQUIPPED missing parameter rows; update regulation.bin with the DLL");}
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn inactive_weapons_receive_the_first_effect_copy() {
        let effect=9850041;
        assert_eq!(super::wanted(&[effect],&[]),vec![9880041]);
        assert!(super::wanted(&[effect],&[effect]).is_empty());
        assert_eq!(super::wanted(&[effect;6],&[]).len(),6);
        assert_eq!(super::wanted(&[effect;6],&[effect]).len(),5);
    }
    #[test]
    fn switching_hands_and_removing_equipment_do_not_duplicate_bonuses() {
        let effect=9850041;
        for count in 1..=10 {
            let equipped=vec![effect;count];
            let held=super::wanted(&equipped,&[effect,effect]);
            let sheathed=super::wanted(&equipped,&[]);
            assert_eq!(held.len()+1,count);
            assert_eq!(sheathed.len(),count);
            assert_eq!(&sheathed[1..],held.as_slice());
        }
        assert!(super::wanted(&[],&[effect]).is_empty());
        assert_eq!(super::wanted(&[effect,effect+1],&[effect]),vec![9880042]);
    }
    #[test]
    fn each_extra_piece_with_the_same_affix_adds_one_copy() {
        let attack_rare=9850000+4*10+1;let hp_common=9850000;
        assert!(super::wanted(&[attack_rare,hp_common,-1,0],&[attack_rare,hp_common]).is_empty(),"native effects are not duplicated");
        assert_eq!(super::wanted(&[attack_rare,attack_rare],&[attack_rare]),vec![9881041]);
        assert_eq!(super::wanted(&[attack_rare;3],&[attack_rare]),vec![9881041,9882041]);
        assert_eq!(super::wanted(&[attack_rare,attack_rare+1],&[attack_rare,attack_rare+1]),Vec::<i32>::new(),"different qualities already stack natively");
        assert_eq!(super::wanted(&[hp_common;12],&[hp_common]).len(),super::COPIES,"capped at ten equipment slots");
        assert!(super::wanted(&[7000,7000,12345,12345],&[]).is_empty(),"only tower affix effects");
        assert!(super::ours(9880041)&&super::ours(9881041)&&super::ours(9889209)&&!super::ours(9880250)&&!super::ours(9890000));
    }
}
