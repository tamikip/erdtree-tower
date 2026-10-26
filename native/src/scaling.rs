use eldenring::cs::{ChrInsExt,FieldInsHandle,SoloParamRepository,SpEffectParam,WorldChrMan};
use fromsoftware_shared::FromStatic;
pub const GROWTH_FIRST:i32=9640000;
fn ready(step:u8)->bool {unsafe {SoloParamRepository::instance()}.is_ok_and(|p|p.get::<SpEffectParam>((GROWTH_FIRST+i32::from(step)) as u32).is_some())}
/// 新旅程按本幕进度给每个刷出的敌人施加一次递增效果；联机时只由房主施加
#[derive(Default)]
pub struct Growth {key:Option<(u64,u8,usize)>,applied:Vec<FieldInsHandle>,missing:bool}
impl Growth {
    pub fn tick(&mut self,world:&mut WorldChrMan,key:(u64,u8,usize),step:u8,handles:&[FieldInsHandle],authority:bool) {
        if self.key!=Some(key){*self=Self{key:Some(key),..Default::default()};}
        if step==0||!authority{return;}
        if !ready(step){if !self.missing{super::log(format!("TOWER_GROWTH missing effect {}",GROWTH_FIRST+i32::from(step)));self.missing=true;}return;}
        for handle in handles {
            if self.applied.contains(handle){continue;}
            let Some(chr)=world.chr_ins_by_handle_mut(handle) else{continue};
            if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(){continue;}
            chr.apply_speffect(GROWTH_FIRST+i32::from(step),false);self.applied.push(*handle);
        }
    }
}
