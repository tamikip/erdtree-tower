use eldenring::cs::{ChrIns,ChrInsExt,SoloParamRepository,SpEffectParam};
use fromsoftware_shared::FromStatic;
use erdtree_tower::{Run,Phase,elite::BUFFS};
const FIRST:i32=9700000;
fn desired(run:Option<&Run>)->Vec<i32> {
    run.filter(|r|!matches!(r.phase,Phase::Dead|Phase::Won)).map_or_else(Vec::new,|r|r.elite_buffs.iter().filter_map(|id|BUFFS.iter().position(|b|b==id).map(|i|FIRST+i as i32)).collect())
}
pub fn validate()->Result<(),String> {
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"护符赐福参数未加载")?;
    if (0..BUFFS.len()).any(|i|params.get::<SpEffectParam>((FIRST+i as i32) as u32).is_none()){return Err("缺少精英护符赐福参数，请重启更新后的启动器".into());}Ok(())
}
pub fn sync(chr:&mut ChrIns,run:Option<&Run>) {
    if validate().is_err(){return;}
    let want=desired(run);let present:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).collect();
    for id in (0..BUFFS.len()).map(|i|FIRST+i as i32) {
        if present.contains(&id)&&!want.contains(&id){chr.remove_speffect(id);}
        if want.contains(&id)&&!present.contains(&id){chr.apply_speffect(id,false);}
    }
}
#[test]
fn elite_buffs_survive_act_changes_and_clear_after_run() {
    let mut run=Run::new(42,erdtree_tower::Class::Samurai);run.elite_buffs.push(BUFFS[0]);
    assert_eq!(desired(Some(&run)),vec![FIRST]);run.act=2;assert_eq!(desired(Some(&run)),vec![FIRST]);
    run.phase=Phase::Dead;assert!(desired(Some(&run)).is_empty());run.phase=Phase::Won;assert!(desired(Some(&run)).is_empty());assert!(desired(None).is_empty());
}
