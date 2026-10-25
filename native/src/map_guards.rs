use std::time::{Duration,Instant};
use eldenring::cs::{FieldInsHandle,WorldChrMan,ChrLoadStatus};
use super::{enemies::{self,Enemy},native_talk::{flag,set_flag}};

pub const COMBAT:u32=45007997;
pub fn entity(original:u32)->Result<u32,String> {
    enemies::map_controls(original).ok_or("事件守卫配置无效")?;Ok(original+3000000)
}
fn offset(id:u32)->Result<u32,String> {
    let original=id.checked_sub(3000000).ok_or("事件守卫编号无效")?;
    entity(original)?;Ok(((original-48000000)/10000-1)*300+original%10000)
}
fn controls(id:u32)->Result<(u32,u32),String> {let n=offset(id)?;Ok((45003000+n,45002000+n))}
fn positions(id:u32,slot:usize)->Result<[(u32,bool);4],String> {
    if slot>=12{return Err("事件守卫站位超限".into());}
    let n=offset(id)?*4;
    let first=if n<1800 {45012000+n}else{45022000+n-1800};
    Ok(std::array::from_fn(|bit|(first+bit as u32,slot&(1<<bit)!=0)))
}
fn engaged(started:bool,elapsed:Duration)->bool {started&&elapsed>=Duration::from_secs(5)}
pub struct Guards {
    specs:Vec<Enemy>,ids:Vec<u32>,bound:Vec<(FieldInsHandle,usize)>,reset:bool,requested:bool,
    clock:Option<Instant>,active:bool,
}
impl Guards {
    pub fn new(specs:Vec<Enemy>)->Result<Self,String> {
        if specs.is_empty()||specs.len()>12{return Err("事件守卫数量无效".into());}
        let ids=enemies::map_entities(&specs)?.into_iter().map(entity).collect::<Result<Vec<_>,_>>()?;
        Ok(Self{specs,ids,bound:vec![],reset:false,requested:false,clock:None,active:false})
    }
    pub fn ready(&self)->bool {self.bound.len()==self.ids.len()}
    pub fn prepare(&mut self,world:&WorldChrMan)->Result<(),String> {
        if self.ready(){return Ok(());}
        if !self.reset {self.retire();self.reset=true;return Ok(());}
        if !self.requested {
            for &id in &self.ids {if flag(controls(id)?.1)?{return Ok(());}}
            for (slot,&id) in self.ids.iter().enumerate() {
                for (f,value) in positions(id,slot)?{set_flag(f,value)?;}
                set_flag(controls(id)?.0,true)?;
            }
            self.requested=true;return Ok(());
        }
        let mut bound=vec![];
        for (&id,spec) in self.ids.iter().zip(&self.specs) {
            if !flag(controls(id)?.1)?{return Ok(());}
            let Some(chr)=world.chr_sets.iter().flatten().flat_map(|s|s.characters()).find(|c|
                c.event_entity_id==id&&c.npc_id==spec.model&&c.npc_param_id==spec.npc
                &&c.modules.data.hp>0&&!c.chr_flags1c5.death_flag()&&!c.debug_flags.character_disabled()
                &&matches!(unsafe {c.chr_set_entry.as_ref()}.chr_load_status,ChrLoadStatus::Active|ChrLoadStatus::ReadyForActivation)) else{return Ok(());};
            bound.push((chr.field_ins_handle,std::ptr::from_ref(chr) as usize));
        }
        self.bound=bound;super::log(format!("Party event guards bound: {:?}",self.ids));Ok(())
    }
    pub fn poll(&mut self,world:&WorldChrMan,started:bool,host:bool)->Result<bool,String> {
        if !self.ready(){return Ok(false);}
        if started {self.clock.get_or_insert_with(Instant::now);}
        if !engaged(started,self.clock.map_or(Duration::ZERO,|t|t.elapsed())){return Ok(false);}
        if !self.active {set_flag(COMBAT,true)?;self.active=true;super::log("Party event guards engaged after shared ready barrier");}
        if !host{return Ok(false);}
        let mut dead=0;
        for (&(handle,address),(&id,spec)) in self.bound.iter().zip(self.ids.iter().zip(&self.specs)) {
            let chr=world.chr_ins_by_handle(&handle).filter(|c|std::ptr::from_ref(*c) as usize==address&&c.event_entity_id==id&&c.npc_id==spec.model&&c.npc_param_id==spec.npc)
                .ok_or("事件守卫已卸载，未结算")?;
            if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(){dead+=1;}
        }
        Ok(dead==self.bound.len())
    }
    pub fn retire(&self) {
        let _=set_flag(COMBAT,false);
        for &id in &self.ids {if let Ok((request,_))=controls(id){let _=set_flag(request,false);}}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guard_ids_and_controls_are_separate_from_battles_and_scarabs() {
        let mut flags=std::collections::HashSet::new();
        for act in 1..=3 {for slot in 0..300 {
            let original=48000000+act*10000+slot;
            let id=entity(original).unwrap();let (request,ready)=controls(id).unwrap();
            assert!(flags.insert(request));assert!(flags.insert(ready));
            assert!(!(45007000..45010000).contains(&request));
            assert!(matches!(ready/1000,45002));
            for (flag,_) in positions(id,11).unwrap(){assert!(flags.insert(flag));assert!(matches!(flag/1000,45012|45013|45022|45023));}
            assert_ne!(id,original);
        }}
        for id in [0,48040000,48010300]{assert!(entity(id).is_err());}
        assert!(positions(51010000,12).is_err());
    }
    #[test]
    fn guards_cannot_fight_before_both_clients_bind_and_countdown_finishes() {
        assert!(!engaged(false,Duration::from_secs(20)));
        assert!(!engaged(true,Duration::from_millis(4999)));
        assert!(engaged(true,Duration::from_secs(5)));
    }
    #[test]
    fn event_rosters_for_all_acts_have_unique_guard_slots_in_the_event_map() {
        for act in 1..=3 {for seed in 0..256 {for layer in 0..12 {
            for kind in [erdtree_tower::NodeKind::Monster,erdtree_tower::NodeKind::Elite] {
                let guards=Guards::new(enemies::roster(kind,seed,17,layer,act)).unwrap();
                assert_eq!(guards.ids.iter().collect::<std::collections::HashSet<_>>().len(),guards.ids.len());
                assert!(guards.ids.iter().all(|&id|controls(id).is_ok()));
                assert!(!guards.ready());assert!(!guards.active);
            }
        }}}
    }
}
