use eldenring::cs::{ChrDebugSpawnRequest,ChrLoadStatus,FieldInsHandle,NpcParam,NpcThinkParam,SoloParamRepository,WorldChrMan};
use fromsoftware_shared::{F32Vector4,FromStatic};
use std::time::{Duration,Instant};

pub const NPC:i32=941912000;
pub const THINK:i32=41912000;
#[derive(Default)]
pub struct Hunt {
    entity_base:Option<i32>,
    bugs:Vec<(FieldInsHandle,usize)>,
    requested:Option<Instant>,
    clock:Option<Instant>,
    revealed:bool,
    killed:u8,
    rewarded:u8,
    finished:bool,
}
#[derive(Debug,PartialEq)]
pub(super) enum Stage { Waiting, Hunting, Finished }
pub(super) fn stage(elapsed:Duration)->Stage {
    if elapsed<Duration::from_secs(3) {Stage::Waiting}
    else if elapsed<Duration::from_secs(9) {Stage::Hunting}
    else {Stage::Finished}
}
impl Hunt {
    pub fn networked(base:i32)->Self {Self{entity_base:Some(base),..Default::default()}}
    pub fn result(&self)->Option<u8> {self.finished.then_some(self.killed)}
    pub fn active(&self)->bool {self.clock.is_some()}
    pub fn ready(&self)->bool {self.bugs.len()==3&&!self.active()&&!self.finished}
    pub fn start(&mut self)->Result<(),String> {
        if !self.ready(){return Err("粪金龟尚未就绪".into());}
        self.clock=Some(Instant::now());Ok(())
    }
    pub fn prepare(&mut self,world:&mut WorldChrMan)->Result<(),String> {
        if self.active()||self.finished||self.bugs.len()==3 {return Ok(());}
        if let Some(at)=self.requested {
            if let Some(created)=world.debug_chr_creator.last_created_chr {
                let chr=unsafe {created.as_ref()};
                let handle=chr.field_ins_handle;
                let address=std::ptr::from_ref(chr) as usize;
                if chr.npc_param_id==NPC && chr.modules.data.hp>0
                    && matches!(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status,ChrLoadStatus::Active|ChrLoadStatus::ReadyForActivation)
                    && !self.bugs.iter().any(|(h,_)|*h==handle) {
                    if let Some(chr)=world.chr_ins_by_handle_mut(&handle).filter(|c|std::ptr::from_ref(*c) as usize==address) {
                        chr.debug_flags.set_character_disabled(true);
                        chr.team_type=6;
                        self.bugs.push((handle,address));
                        self.requested=None;world.debug_chr_creator.last_created_chr=None;
                    }
                }
            }
            if self.requested.is_some()&&at.elapsed()>Duration::from_secs(15) {return Err("粪金龟加载失败，未收取卢恩".into());}
            return Ok(());
        }
        if world.debug_chr_creator.spawn {return Ok(());}
        let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"粪金龟参数未加载")?;
        if params.get::<NpcParam>(NPC as u32).is_none()||params.get::<NpcThinkParam>(THINK as u32).is_none() {return Err("缺少粪金龟事件资源，未收取卢恩".into());}
        let player=world.main_player.as_ref().ok_or("等待角色加载")?;
        let local=[-4.0+self.bugs.len() as f32*4.0,0.2,-2.0];
        let physics=&player.chr_ins.modules.physics;
        let pos=[physics.position.0-player.block_position.x+local[0],physics.position.1-player.block_position.y+local[1],physics.position.2-player.block_position.z+local[2]];
        let init=&mut world.debug_chr_creator.init_data;
        unsafe {
            let bytes=std::ptr::from_mut(init).cast::<u8>();
            bytes.add(0x10).cast::<F32Vector4>().write(F32Vector4(0.0,self.bugs.len() as f32*2.1,0.0,0.0));
            bytes.add(0xc8).write(0);
        }
        world.debug_chr_creator.last_created_chr=None;
        world.spawn_debug_character(&ChrDebugSpawnRequest {chr_id:4191,npc_param_id:NPC,npc_think_param_id:THINK,chara_init_param_id:0,event_entity_id:self.entity_base.unwrap_or(45011810)+self.bugs.len() as i32,talk_id:-1,pos_x:pos[0],pos_y:pos[1],pos_z:pos[2]});
        self.requested=Some(Instant::now());Ok(())
    }
    pub fn poll(&mut self,world:&mut WorldChrMan)->Result<(),String> {
        let Some(clock)=self.clock else{return Ok(());};
        let phase=stage(clock.elapsed());
        if self.finished||phase==Stage::Waiting {return Ok(());}
        if !self.revealed {
            for &(handle,address) in &self.bugs {
                let chr=world.chr_ins_by_handle_mut(&handle).filter(|c|std::ptr::from_ref(*c) as usize==address).ok_or("粪金龟已卸载")?;
                chr.debug_flags.set_character_disabled(false);
            }
            self.revealed=true;
            // Begin the full capture window only when all three become visible.
            self.clock=Some(Instant::now()-Duration::from_secs(3));
            return Ok(());
        }
        for (slot,&(handle,address)) in self.bugs.iter().enumerate() {
            if world.chr_ins_by_handle(&handle).is_some_and(|c|std::ptr::from_ref(c) as usize==address && (c.modules.data.hp<=0||c.chr_flags1c5.death_flag())) {
                self.killed|=1<<slot;
            }
        }
        if phase==Stage::Finished {self.retire(world);self.finished=true;}
        Ok(())
    }
    pub fn rewards(&self)->Vec<usize> {(0..3).filter(|i|self.finished&&self.killed&(1<<i)!=0&&self.rewarded&(1<<i)==0).collect()}
    pub fn rewarded(&mut self,slot:usize){self.rewarded|=1<<slot;}
    pub fn done(&self)->bool {self.finished&&self.rewards().is_empty()}
    pub fn retire(&self,world:&mut WorldChrMan) {
        if self.requested.is_some()&&world.debug_chr_creator.init_data.npc_param_id==NPC {world.debug_chr_creator.spawn=false;}
        for &(handle,address) in &self.bugs {
            if let Some(chr)=world.chr_ins_by_handle_mut(&handle).filter(|c|std::ptr::from_ref(*c) as usize==address) {
                chr.debug_flags.set_character_disabled(true);
                chr.debug_flags.set_force_unloaded(true);
            }
        }
        if self.requested.is_some() {
            if let Some(created)=world.debug_chr_creator.last_created_chr {
                let chr=unsafe {created.as_ref()};
                if chr.npc_param_id==NPC {
                    let handle=chr.field_ins_handle;
                    if let Some(chr)=world.chr_ins_by_handle_mut(&handle){chr.debug_flags.set_character_disabled(true);chr.debug_flags.set_force_unloaded(true);}
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn three_second_delay_and_six_second_capture_window_have_exact_boundaries() {
        for (ms,expected) in [(0,Stage::Waiting),(2999,Stage::Waiting),(3000,Stage::Hunting),(6000,Stage::Hunting),(8999,Stage::Hunting),(9000,Stage::Finished),(9999,Stage::Finished)] {
            assert_eq!(stage(Duration::from_millis(ms)),expected);
        }
    }
    #[test]
    fn only_killed_scarabs_pay_once_and_expired_scarabs_pay_nothing() {
        let mut hunt=Hunt::default();assert!(hunt.start().is_err());
        hunt.killed=5;hunt.finished=true;assert_eq!(hunt.rewards(),vec![0,2]);assert!(!hunt.done());
        hunt.rewarded(0);assert_eq!(hunt.rewards(),vec![2]);hunt.rewarded(0);assert_eq!(hunt.rewards(),vec![2]);
        hunt.rewarded(2);assert!(hunt.done());
        let empty=Hunt{finished:true,..Default::default()};assert!(empty.done());
    }
}
