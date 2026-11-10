use erdtree_tower::{Class,Run,Phase};
use eldenring::cs::{ChrIns,ChrInsExt,PlayerGameData,SoloParamRepository,SpEffectParam};
use fromsoftware_shared::FromStatic;
const FIRST:i32=9800000;

/// 开局附带的职业物品：(编号含类别位, 数量)
fn starting_gift(class:Class)->&'static [(u32,i32)] {
    match class {
        Class::Vagabond=>&[(0x40000000|10020,1)],
        // 召唤师带离群野狼与骸骨民兵的骨灰（狂战士的巨斧直接装在右手，见 loadout::starting_weapons）
        Class::Confessor=>&[(0x40000000|232000,1),(0x40000000|212000,1)],
        _=>&[],
    }
}
pub fn give_starting(data:&mut PlayerGameData,class:Class)->Result<(),String> {
    for &(id,quantity) in starting_gift(class) {
        super::random_ashes::give(&[(id as i32,quantity,-1)])?;
        if super::loot::count(data,id)!=quantity as u64 {return Err("职业初始物品发放不完整".into());}
    }
    Ok(())
}
/// 打赢一场战斗：祷誓者回复 10% 生命（狂战士的胜利回血已移除）
fn victory_hp(run:&Run,hp:i32,max_hp:i32)->i32 {
    let percent=match run.class {Class::Prophet=>10,_=>return hp};
    if hp<=0||max_hp<=0 {return hp;}
    (i64::from(hp)+i64::from(max_hp)*percent/100).min(i64::from(max_hp)) as i32
}
pub fn heal_victory(chr:&mut ChrIns,run:&Run) {
    if chr.chr_flags1c5.death_flag(){return;}
    let data=&mut chr.modules.data;
    data.hp=victory_hp(run,data.hp,data.max_hp);
}
/// 祷誓者「圣誓」：旅程中每秒回复 1% 最大专注（至少 1 点）
fn prophet_regen(fp:i32,max_fp:i32)->i32 {
    if max_fp<=0 {return fp;}
    (i64::from(fp.max(0))+(i64::from(max_fp)/100).max(1)).min(i64::from(max_fp)) as i32
}
static REGEN:std::sync::Mutex<Option<std::time::Instant>>=std::sync::Mutex::new(None);
fn regen(chr:&mut ChrIns,run:Option<&Run>) {
    let active=run.is_some_and(|r|r.class==Class::Prophet&&!matches!(r.phase,Phase::Dead|Phase::Won))&&!chr.chr_flags1c5.death_flag()&&chr.modules.data.hp>0;
    let Ok(mut last)=REGEN.lock() else{return};
    if !active {*last=None;return;}
    let now=std::time::Instant::now();
    let Some(at)=*last else{*last=Some(now);return};
    if now.duration_since(at)<std::time::Duration::from_secs(1) {return;}
    *last=Some(now);
    let data=&mut chr.modules.data;data.fp=prophet_regen(data.fp,data.max_fp);
}
/// 职业重做后的职业机制（scene-builder class-patch 写入的 9810000 段）
const CLASS:i32=9810000;
/// 运行时管理的常驻效果；举盾、致命一击后的效果由这些常驻效果自己触发，不在这里增删。
/// FIRST..=FIRST+4 是职业重做前的旧被动，已不再施加，只留在这里以便从角色身上清掉
fn all()->impl Iterator<Item=i32> {(FIRST..=FIRST+4).chain([CLASS,CLASS+3,CLASS+10,CLASS+11,CLASS+20,CLASS+23,CLASS+30,CLASS+40])}
/// 当前应挂的效果；low 为生命低于一半（狂战士的狂血）
fn desired(run:Option<&Run>,low:bool)->Vec<i32> {
    let Some(run)=run.filter(|r|!matches!(r.phase,Phase::Dead|Phase::Won)) else{return vec![]};
    match run.class {
        // 守誓骑士：举盾时减伤（常驻判定）与防御耗精；狂战士：削韧减免，低血时减伤；影刃：致命一击后增伤（常驻监视）与精力恢复
        Class::Vagabond=>vec![CLASS,CLASS+3],
        Class::Hero=>if low {vec![CLASS+10,CLASS+11]}else{vec![CLASS+11]},
        Class::Samurai=>vec![CLASS+20,CLASS+23],
        Class::Astrologer=>vec![CLASS+30],
        Class::Prophet=>vec![CLASS+40],
        Class::Confessor=>vec![],
    }
}
pub fn sync(chr:&mut ChrIns,run:Option<&Run>) {
    regen(chr,run);
    let Ok(params)=(unsafe {SoloParamRepository::instance()}) else{return;};
    let data=&chr.modules.data;
    let low=data.max_hp>0&&i64::from(data.hp)*2<i64::from(data.max_hp);
    let want=desired(run,low);
    let present:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).collect();
    for id in all() {
        if present.contains(&id)&&!want.contains(&id){chr.remove_speffect(id);}
        if want.contains(&id)&&!present.contains(&id)&&params.get::<SpEffectParam>(id as u32).is_some(){chr.apply_speffect(id,false);}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn class_gifts_are_exact() {
        assert_eq!(starting_gift(Class::Vagabond),&[(0x40000000|10020,1)]);
        assert_eq!(starting_gift(Class::Confessor),&[(0x40000000|232000,1),(0x40000000|212000,1)]);
        assert!(starting_gift(Class::Hero).is_empty());
        assert!(starting_gift(Class::Samurai).is_empty());
    }
    #[test]
    fn victory_heals_follow_the_class() {
        let mut run=Run::new(1,Class::Hero);
        assert_eq!(victory_hp(&run,300,1000),300,"berserker no longer heals on victory");
        run.class=Class::Samurai;assert_eq!(victory_hp(&run,300,1000),300);
        assert_eq!(victory_hp(&Run::new(1,Class::Prophet),0,1000),0);
        let cleric=Run::new(1,Class::Prophet);
        assert_eq!((victory_hp(&cleric,300,1000),victory_hp(&cleric,950,1000)),(400,1000));
        assert_eq!(victory_hp(&Run::new(1,Class::Astrologer),300,1000),300);
        assert_eq!((prophet_regen(10,300),prophet_regen(10,50),prophet_regen(299,300),prophet_regen(-5,100)),(13,11,300,1));
    }
    #[test]
    fn class_effects_follow_rules_and_end_with_run() {
        let mut run=Run::new(1,Class::Vagabond);
        assert_eq!(desired(Some(&run),false),vec![CLASS,CLASS+3]);
        run.class=Class::Hero;assert_eq!((desired(Some(&run),false),desired(Some(&run),true)),(vec![CLASS+11],vec![CLASS+10,CLASS+11]));
        run.class=Class::Confessor;assert!(desired(Some(&run),true).is_empty());
        run.class=Class::Hero;assert!(desired(Some(&run),true).iter().all(|id|!(FIRST..=FIRST+4).contains(id)),"the pre-rework passives are never applied");
        run.phase=Phase::Dead;assert!(desired(Some(&run),false).is_empty());
        assert!(desired(None,false).is_empty());
        assert!(desired(Some(&Run::new(1,Class::Samurai)),false).iter().all(|id|all().any(|x|x==*id)));
    }
}
