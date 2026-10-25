use eldenring::cs::{ChrIns,ChrInsExt,EquipParamGoods,SoloParamRepository,SpEffectParam};
use fromsoftware_shared::FromStatic;

pub fn clear(chr:&mut ChrIns)->Result<(),String> {
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"异常清除参数尚未加载")?;
    let mut effects=Vec::with_capacity(7);
    for id in [900,910,920,930,940,950,960] {
        let item=params.get::<EquipParamGoods>(id).ok_or("缺少异常治疗参数")?;
        let effect=item.ref_id_default();
        if item.ref_category()!=2||effect<0||params.get::<SpEffectParam>(effect as u32).is_none() {return Err("异常治疗参数不匹配".into());}
        effects.push(effect);
    }
    for effect in effects {chr.apply_speffect(effect,false);}
    Ok(())
}
