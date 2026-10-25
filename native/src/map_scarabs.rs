use eldenring::cs::{FieldInsHandle,WorldChrMan};
use std::time::{Duration,Instant};
use super::native_talk::{flag,set_flag};

pub fn controls(entity:u32)->Result<(u32,u32,u32),String> {
    let slot=entity.checked_sub(48040000).filter(|s|*s<72).ok_or("粪金龟实体编号无效")?;
    let request=45007000+slot*4;Ok((request,request+1,request+2))
}
pub struct Hunt {
    base:u32,reset:bool,requested:bool,bugs:Vec<(FieldInsHandle,usize)>,
    clock:Option<Instant>,revealed:bool,killed:u8,finished:bool,
}
impl Hunt {
    pub fn new(base:u32)->Result<Self,String> {
        controls(base)?;controls(base+2)?;
        if (base-48040000)%3!=0{return Err("粪金龟组编号无效".into());}
        Ok(Self{base,reset:false,requested:false,bugs:vec![],clock:None,revealed:false,killed:0,finished:false})
    }
    pub fn ready(&self)->bool {self.bugs.len()==3&&!self.active()&&!self.finished}
    pub fn active(&self)->bool {self.clock.is_some()}
    pub fn result(&self)->Option<u8> {self.finished.then_some(self.killed)}
    pub fn start(&mut self)->Result<(),String> {
        if !self.ready(){return Err("粪金龟尚未就绪".into());}self.clock=Some(Instant::now());Ok(())
    }
    pub fn prepare(&mut self,world:&mut WorldChrMan)->Result<(),String> {
        if self.active()||self.finished||self.bugs.len()==3{return Ok(());}
        if !self.reset {
            self.retire(world);self.reset=true;return Ok(());
        }
        if !self.requested {
            for entity in self.base..self.base+3 {if flag(controls(entity)?.1)? {return Ok(());}}
            for entity in self.base..self.base+3 {set_flag(controls(entity)?.0,true)?;}
            self.requested=true;return Ok(());
        }
        let mut bugs=vec![];
        for entity in self.base..self.base+3 {
            if !flag(controls(entity)?.1)? {return Ok(());}
            let Some(chr)=world.chr_sets.iter().flatten().flat_map(|s|s.characters()).find(|c|
                c.event_entity_id==entity&&c.npc_id==4191&&c.npc_param_id==super::scarabs::NPC
                &&c.modules.data.hp>0&&!c.chr_flags1c5.death_flag()
                &&matches!(unsafe {c.chr_set_entry.as_ref()}.chr_load_status,eldenring::cs::ChrLoadStatus::Active|eldenring::cs::ChrLoadStatus::ReadyForActivation)) else{return Ok(());};
            bugs.push((chr.field_ins_handle,std::ptr::from_ref(chr) as usize));
        }
        self.bugs=bugs;super::log(format!("Party scarabs bound: base={}",self.base));Ok(())
    }
    pub fn poll(&mut self,world:&mut WorldChrMan)->Result<(),String> {
        let Some(clock)=self.clock else{return Ok(());};
        let phase=super::scarabs::stage(clock.elapsed());
        if self.finished||phase==super::scarabs::Stage::Waiting{return Ok(());}
        if !self.revealed {
            for entity in self.base..self.base+3 {set_flag(controls(entity)?.2,true)?;}
            self.revealed=true;super::log(format!("Party scarabs revealed: base={}",self.base));
            self.clock=Some(Instant::now()-Duration::from_secs(3));return Ok(());
        }
        for (slot,&(handle,address)) in self.bugs.iter().enumerate() {
            if self.killed&(1<<slot)!=0 {continue;}
            let chr=world.chr_ins_by_handle(&handle).filter(|c|std::ptr::from_ref(*c) as usize==address).ok_or("粪金龟场景实体已卸载")?;
            if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(){self.killed|=1<<slot;}
        }
        if phase==super::scarabs::Stage::Finished {
            for entity in self.base..self.base+3 {set_flag(controls(entity)?.2,false)?;}
            self.finished=true;
            super::log(format!("Party scarabs finished: base={} killed={}",self.base,self.killed));
        }
        Ok(())
    }
    pub fn retire(&self,_world:&mut WorldChrMan) {
        for entity in self.base..self.base+3 {
            if let Ok((request,_,visible))=controls(entity) {
                let _=set_flag(visible,false);let _=set_flag(request,false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn map_controls_do_not_overlap_enemy_or_combat_flags() {
        let mut flags=std::collections::HashSet::new();
        for group in 0..24 {
            assert!(Hunt::new(48040000+group*3).is_ok());
            for slot in 0..3 {
                let (request,ready,visible)=controls(48040000+group*3+slot).unwrap();
                for f in [request,ready,visible] {assert!(flags.insert(f));assert!((45007000..45007288).contains(&f));}
            }
        }
        assert!(Hunt::new(48040001).is_err());assert!(controls(48040072).is_err());
    }
}
