use std::time::{Duration,Instant};
use eldenring::cs::{ChrInsExt,FieldInsHandle,SoloParamRepository,SpEffectParam,WorldChrMan};
use fromsoftware_shared::FromStatic;
use erdtree_tower::boss_affixes::Affix;
pub const FIRST:i32=9620000;
const ROT_BUILDUP:i32=FIRST+41;const ENVY:i32=FIRST+70;const PRIDE:i32=FIRST+80;const SHIELD:i32=FIRST+90;const REBORN:i32=FIRST+95;
const EFFECTS:[i32;10]=[FIRST,FIRST+10,FIRST+20,FIRST+30,FIRST+40,ROT_BUILDUP,ENVY,PRIDE,SHIELD,REBORN];
const ROT_RADIUS:f32=5.0;
fn resident(affix:Affix)->Option<i32> {
    match affix {Affix::Stalwart=>Some(FIRST),Affix::Bulwark=>Some(FIRST+10),Affix::Frenzied=>Some(FIRST+20),Affix::Regenerating=>Some(FIRST+30),Affix::Rot=>Some(FIRST+40),_=>None}
}
pub fn validate()->Result<(),String> {
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"Boss 词条参数未加载")?;
    if EFFECTS.iter().any(|id|params.get::<SpEffectParam>(*id as u32).is_none()){return Err("缺少 Boss 词条参数，请使用完整启动包".into());}Ok(())
}
struct Foe {handle:FieldInsHandle,hp:i32,hurt:Instant,proud:bool,reborn:bool}
#[derive(Default)]
pub struct Fight {key:Option<(u64,u8,usize)>,affixes:Vec<Affix>,foes:Vec<Foe>,flasks:Option<u8>,shield_at:Option<Instant>,rot_at:Option<Instant>,missing:bool}
impl Fight {
    pub fn begin(&mut self,key:(u64,u8,usize),affixes:Vec<Affix>) {
        if self.key==Some(key){return;}
        if !affixes.is_empty(){super::log(format!("BOSS_AFFIX node={} act={} affixes={}",key.2,key.1,erdtree_tower::boss_affixes::title(&affixes)));}
        *self=Self{key:Some(key),affixes,..Default::default()};
    }
    pub fn title(&self)->Option<String> {(!self.affixes.is_empty()).then(||erdtree_tower::boss_affixes::title(&self.affixes))}
    pub fn tick(&mut self,world:&mut WorldChrMan,handles:&[FieldInsHandle],authority:bool,flasks:Option<u8>) {
        if self.affixes.is_empty(){return;}
        if let Err(error)=validate(){if !self.missing{super::log(error);self.missing=true;}return;}
        let now=Instant::now();
        let drank=matches!((self.flasks,flasks),(Some(before),Some(after)) if after<before);self.flasks=flasks;
        let shield=self.affixes.contains(&Affix::Golden)&&match self.shield_at {Some(at)=>now>=at,None=>{self.shield_at=Some(now+Duration::from_secs(20));false}};
        if shield {self.shield_at=Some(now+Duration::from_secs(20));}
        let player=world.main_player.as_ref().map(|p|p.chr_ins.modules.physics.position);
        let mut nearest=f32::MAX;
        for handle in handles {
            let Some(chr)=world.chr_ins_by_handle_mut(handle) else{continue};
            if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(){continue;}
            if let Some(p)=player {let q=chr.modules.physics.position;nearest=nearest.min(((p.0-q.0).powi(2)+(p.1-q.1).powi(2)+(p.2-q.2).powi(2)).sqrt());}
            if !authority {continue;}
            let index=match self.foes.iter().position(|f|f.handle==*handle) {Some(i)=>i,None=>{self.foes.push(Foe{handle:*handle,hp:chr.modules.data.hp,hurt:now,proud:false,reborn:false});self.foes.len()-1}};
            let foe=&mut self.foes[index];
            let present:Vec<i32>=chr.special_effect.entries().map(|e|e.param_id).collect();
            for id in self.affixes.iter().filter_map(|a|resident(*a)) {if !present.contains(&id){chr.apply_speffect(id,false);}}
            let (hp,max)=(chr.modules.data.hp,chr.modules.data.max_hp);
            if hp<foe.hp {foe.hurt=now;if foe.proud{chr.remove_speffect(PRIDE);foe.proud=false;}}
            foe.hp=hp;
            if self.affixes.contains(&Affix::Proud)&&!foe.proud&&now.duration_since(foe.hurt)>=Duration::from_secs(8) {chr.apply_speffect(PRIDE,false);foe.proud=true;}
            if self.affixes.contains(&Affix::Envious)&&drank {chr.apply_speffect(ENVY,false);}
            if shield {chr.apply_speffect(SHIELD,false);}
            if self.affixes.contains(&Affix::Reborn)&&!foe.reborn&&max>0&&i64::from(hp)*10<=i64::from(max) {
                chr.modules.data.hp=(i64::from(max)*2/5) as i32;foe.hp=chr.modules.data.hp;foe.reborn=true;chr.apply_speffect(REBORN,false);
                super::log(format!("BOSS_AFFIX reborn handle={handle} hp={}/{max}",foe.hp));
            }
        }
        if self.affixes.contains(&Affix::Rot)&&nearest<=ROT_RADIUS&&self.rot_at.is_none_or(|at|now>=at) {
            if let Some(player)=world.main_player.as_mut().filter(|p|p.chr_ins.modules.data.hp>0) {player.chr_ins.apply_speffect(ROT_BUILDUP,true);}
            self.rot_at=Some(now+Duration::from_secs(1));
        }
    }
}
