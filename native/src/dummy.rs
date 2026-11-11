// 营地侧边的测伤假人：落地后固定姿态，保留伤害结算；停手时隐藏统计面板。
// 按它每帧掉的生命统计伤害（出血、中毒等状态伤害也算）：从第一击开始一轮，停手 4 秒后结算；万一一击打空血条，按满血计并在 1 秒后重新生成。
// 出血、中毒、腐败、冻伤按最大生命的百分比扣血，所以假人的生命取本幕 Boss 的典型生命（health），每帧补满，DPS 与实战一致。
// 联机时也生成：调试生成的角色只存在于本机，队友看不到、打不到，所以每人各有一个、各算各的。
use eldenring::cs::{ChrDebugSpawnRequest,ChrIns,ChrLoadStatus,FieldInsHandle,NpcParam,NpcThinkParam,SoloParamRepository,WorldChrMan};
use fromsoftware_shared::{F32Vector4,FromStatic};
use std::time::{Duration,Instant};

/// 葛瑞克士兵（模型 4311）在爬塔里的敌人参数
const MODEL:i32=4311;
const NPC:i32=900000000+MODEL*10000;
const THINK:i32=MODEL*10000;
/// 停手多久结算一轮（秒）
const IDLE:f64=4.0;
/// 假人的生命：各幕单体 Boss 打到幕末的典型生命（中位数约 3100 / 8100 / 22800，取宽一些，避免一击打空）
pub const HEALTH:[i32;3]=[5_000,12_000,30_000];
pub fn health(act:u8)->i32 {HEALTH[usize::from(act.clamp(1,3)-1)]}

fn damage_since_refill(previous:i32,hp:i32,max:i32)->u32 {
    previous.min(max.max(0)).saturating_sub(hp.max(0)).max(0) as u32
}

fn placement(me:[f32;3],yaw:f32)->[f32;3] {
    let (sin,cos)=yaw.sin_cos();
    [me[0]-sin*2.0+cos*8.0,me[1]+0.1,me[2]-cos*2.0-sin*8.0]
}
fn refill(chr:&mut ChrIns,health:i32) {
    let data=&mut chr.modules.data;
    data.base_hp=health;data.max_uncapped_hp=health;data.max_hp=health;data.hp=health;
}

/// 一轮的统计
#[derive(Clone,Copy,Debug,Default,PartialEq)]
pub struct Round {pub total:u64,pub hits:u32,pub seconds:f64,pub biggest:u32}
impl Round {
    /// 不到 1 秒按 1 秒算，避免只打一下时数字虚高
    pub fn dps(&self)->f64 {self.total as f64/self.seconds.max(1.0)}
}
/// 伤害计：当前一轮与上一轮的结果
#[derive(Clone,Copy,Debug,Default,PartialEq)]
pub struct Meter {current:Round,first:Option<f64>,last:Option<f64>,pub last_hit:u32,pub finished:Option<Round>}
impl Meter {
    pub fn hit(&mut self,damage:u32,now:f64) {
        if damage==0 {return;}
        if self.first.is_none() {self.current=Round::default();self.first=Some(now);}
        self.current.total+=u64::from(damage);self.current.hits+=1;self.current.biggest=self.current.biggest.max(damage);
        self.last_hit=damage;self.last=Some(now);
    }
    /// 停手超过 IDLE 秒时结算本轮（用时为第一击到最后一击）
    pub fn tick(&mut self,now:f64) {
        if let (Some(first),Some(last))=(self.first,self.last) && now-last>=IDLE {
            self.finished=Some(Round{seconds:last-first,..self.current});
            self.first=None;self.last=None;
        }
    }
    /// 进行中的一轮（用时为第一击到现在）
    pub fn active(&self,now:f64)->Option<Round> {self.first.map(|first|Round{seconds:now-first,..self.current})}
}

/// 界面上显示的内容
#[derive(Clone,Copy,Debug,Default)]
pub struct View {pub active:Option<Round>,pub last_hit:u32,pub overflow:bool,pub health:i32,pub act:u8}

#[derive(Default)]
pub struct Dummy {
    bound:Option<(FieldInsHandle,usize)>,
    requested:Option<Instant>,
    respawn_at:Option<Instant>,
    failed:bool,
    last_hp:i32,
    overflow:bool,
    /// 当前所在幕（决定假人的生命）
    act:u8,
    meter:Meter,
    spot:Option<([f32;3],f32)>,
    anchor:Option<[f32;3]>,
    display:Option<(f64,f64,View)>,
}
impl Dummy {
    pub fn target(&self,world:&WorldChrMan)->Option<FieldInsHandle> {
        let (handle,address)=self.bound?;
        world.chr_ins_by_handle(&handle).filter(|c|std::ptr::from_ref(*c) as usize==address).map(|_|handle)
    }
    pub fn view(&mut self,now:f64)->View {
        let visible=self.meter.last.is_some_and(|last|now-last<IDLE);
        let Some(first)=self.meter.first.filter(|_|visible) else {self.display=None;return View::default();};
        if let Some((start,at,view))=self.display {
            if start==first&&now>=at&&now-at<0.5 {return view;}
        }
        let view=View{active:self.meter.active(now),last_hit:self.meter.last_hit,overflow:self.overflow,health:health(self.act),act:self.act.clamp(1,3)};
        self.display=Some((first,now,view));view
    }
    fn hold(&mut self,chr:&mut ChrIns,origin:[f32;3]) {
        chr.debug_flags.set_disabled_secondary_actions(true);
        chr.debug_flags.set_disabled_movement(true);
        let physics=&mut chr.modules.physics;
        if self.anchor.is_none()&&physics.standing_on_solid_ground&&!physics.is_falling {
            self.anchor=Some([physics.position.0-origin[0],physics.position.1-origin[1],physics.position.2-origin[2]]);
        }
        let Some(at)=self.anchor else{return;};
        physics.motion_multiplier=0.0;
        physics.position.0=origin[0]+at[0];physics.position.1=origin[1]+at[1];physics.position.2=origin[2]+at[2];
        physics.last_update_position=physics.position;physics.chr_proxy_pos_update_requested=true;
        if let Some((_,yaw))=self.spot {
            let (sin,cos)=(yaw*0.5).sin_cos();
            physics.orientation=eldenring::rotation::Quaternion(0.0,sin,0.0,cos);
            physics.interpolated_orientation=physics.orientation;
        }
        chr.modules.behavior.animation_speed=0.0;
        chr.modules.action_flag.action_modifiers_flags.set_super_armor(true);
        chr.modules.action_flag.action_modifiers_flags.set_poise_forced_break(false);
        chr.modules.super_armor.sa_durability=chr.modules.super_armor.sa_durability_max;
        chr.modules.super_armor.poise_broken_state=false;
    }
    pub fn tick(&mut self,world:&mut WorldChrMan,now:f64,act:u8)->Result<(),String> {
        self.meter.tick(now);self.act=act;let health=health(act);
        let Some(player)=world.main_player.as_ref() else{return Ok(());};
        let p=player.chr_ins.modules.physics.position;
        let origin=[p.0-player.block_position.x,p.1-player.block_position.y,p.2-player.block_position.z];
        if let Some((handle,address))=self.bound {
            let Some(chr)=world.chr_ins_by_handle_mut(&handle).filter(|c|std::ptr::from_ref(*c) as usize==address) else {
                self.bound=None;self.anchor=None;return Ok(());
            };
            let hp=chr.modules.data.hp;
            let damage=damage_since_refill(self.last_hp,hp,chr.modules.data.max_hp);
            if damage>0 {
                if self.meter.first.is_none(){self.overflow=false;}
                self.meter.hit(damage,now);
            }
            if hp<=0||chr.chr_flags1c5.death_flag() {
                // 一击打空血条：按满血计，撤下后重新生成
                self.overflow=true;
                chr.debug_flags.set_character_disabled(true);chr.debug_flags.set_force_unloaded(true);
                self.bound=None;self.anchor=None;self.respawn_at=Some(Instant::now()+Duration::from_secs(1));
                return Ok(());
            }
            refill(chr,health);self.last_hp=health;
            self.hold(chr,origin);
            return Ok(());
        }
        if let Some(at)=self.requested {
            if let Some(created)=world.debug_chr_creator.last_created_chr {
                let chr=unsafe {created.as_ref()};
                let (handle,address)=(chr.field_ins_handle,std::ptr::from_ref(chr) as usize);
                if chr.npc_param_id==NPC&&chr.modules.data.hp>0
                    &&matches!(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status,ChrLoadStatus::Active|ChrLoadStatus::ReadyForActivation)
                    &&let Some(chr)=world.chr_ins_by_handle_mut(&handle).filter(|c|std::ptr::from_ref(*c) as usize==address) {
                    refill(chr,health);self.last_hp=health;
                    self.hold(chr,origin);
                    self.bound=Some((handle,address));self.requested=None;world.debug_chr_creator.last_created_chr=None;
                    super::log(format!("Damage dummy bound: handle={handle}"));
                    return Ok(());
                }
            }
            if at.elapsed()>Duration::from_secs(15) {self.requested=None;self.failed=true;return Err("测伤假人生成超时".into());}
            return Ok(());
        }
        if self.failed||self.respawn_at.is_some_and(|at|Instant::now()<at)||world.debug_chr_creator.spawn {return Ok(());}
        let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"敌人参数未加载")?;
        if params.get::<NpcParam>(NPC as u32).is_none()||params.get::<NpcThinkParam>(THINK as u32).is_none() {self.failed=true;return Err("缺少测伤假人的敌人参数".into());}
        let player=world.main_player.as_ref().ok_or("等待角色加载")?;
        let physics=&player.chr_ins.modules.physics;
        if !physics.standing_on_solid_ground||physics.is_falling {return Ok(());}
        // 前方 2 米、右侧 8 米，避开营地中央；重生沿用原位置。
        let me=[physics.position.0-player.block_position.x,physics.position.1-player.block_position.y,physics.position.2-player.block_position.z];
        let (at,yaw)=*self.spot.get_or_insert_with(|| {
            let at=placement(me,physics.orientation.to_euler_angles().1);
            (at,super::enemies::facing(at,me))
        });
        let origin=[physics.position.0-me[0],physics.position.1-me[1],physics.position.2-me[2]];
        let init=&mut world.debug_chr_creator.init_data;
        unsafe {
            let bytes=std::ptr::from_mut(init).cast::<u8>();
            bytes.add(0x10).cast::<F32Vector4>().write(F32Vector4(0.0,yaw,0.0,0.0));
            bytes.add(0xc8).write(0);
        }
        world.debug_chr_creator.last_created_chr=None;
        world.spawn_debug_character(&ChrDebugSpawnRequest{chr_id:MODEL,npc_param_id:NPC,npc_think_param_id:THINK,chara_init_param_id:0,event_entity_id:0,talk_id:-1,
            pos_x:origin[0]+at[0],pos_y:origin[1]+at[1],pos_z:origin[2]+at[2]});
        self.requested=Some(Instant::now());self.respawn_at=None;
        Ok(())
    }
    pub fn retire(&self,world:&mut WorldChrMan) {
        if self.requested.is_some()&&world.debug_chr_creator.init_data.npc_param_id==NPC {world.debug_chr_creator.spawn=false;}
        if let Some((handle,address))=self.bound
            &&let Some(chr)=world.chr_ins_by_handle_mut(&handle).filter(|c|std::ptr::from_ref(*c) as usize==address) {
            chr.debug_flags.set_character_disabled(true);chr.debug_flags.set_force_unloaded(true);
        }
        if self.requested.is_some()&&let Some(created)=world.debug_chr_creator.last_created_chr {
            let chr=unsafe {created.as_ref()};
            if chr.npc_param_id==NPC {
                let handle=chr.field_ins_handle;
                if let Some(chr)=world.chr_ins_by_handle_mut(&handle){chr.debug_flags.set_character_disabled(true);chr.debug_flags.set_force_unloaded(true);}
            }
        }
    }
}

/// 营地左侧的统计面板（画在前景层，不拦截输入）
pub fn draw_hud(ui:&hudhook::imgui::Ui,view:&View) {
    let Some(round)=view.active else{return;};
    use crate::fancy_text::{Effect,Layer,Style,draw,measure};
    use hudhook::imgui::ImColor32;
    let size=ui.io().display_size;let s=(size[1]/1080.0).max(0.6);
    let (x,y)=(size[0]*0.025,size[1]*0.24);
    let (gold,white,dim,warn)=([232,196,120],[236,228,212],[160,150,132],[232,140,96]);
    let number=|v:u64|{let s=v.to_string();let mut out=String::new();for (i,c) in s.chars().enumerate(){if i>0&&(s.len()-i)%3==0{out.push(',');}out.push(c);}out};
    let mut lines:Vec<(String,[u8;3],f32)>=vec![("测伤假人".into(),gold,26.0)];
            lines.push((format!("DPS  {:.1}",round.dps()),white,30.0));
            lines.push((format!("本轮 {} 伤害 · {} 击 · {:.1} 秒",number(round.total),round.hits,round.seconds),white,19.0));
            lines.push((format!("最近一击 {} · 最高 {}",number(u64::from(view.last_hit)),number(u64::from(round.biggest))),dim,19.0));
            lines.push((format!("生命 {}（第 {} 幕 Boss 典型值，出血等按比例结算）",number(view.health as u64),view.act),dim,17.0));
            lines.push(("停手 4 秒后隐藏".into(),dim,17.0));
    if view.overflow {lines.push(("有一击超过假人生命上限，按满血计".into(),warn,17.0));}
    let height:f32=lines.iter().map(|(_,_,k)|k*s+6.0*s).sum::<f32>()+20.0*s;
    let width=lines.iter().map(|(t,_,k)|measure(t,k*s)).fold(0.0f32,f32::max)+32.0*s;
    let back=ui.get_foreground_draw_list();
    back.add_rect([x,y],[x+width,y+height],ImColor32::from_rgba(10,9,8,170)).filled(true).rounding(6.0*s).build();
    back.add_rect([x,y],[x+width,y+height],ImColor32::from_rgba(180,150,90,120)).rounding(6.0*s).build();
    drop(back);
    let mut line_y=y+10.0*s;
    for (text,color,k) in &lines {
        draw(Layer::Foreground,[x+16.0*s,line_y],k*s,text,&Style{color:*color,accent:*color,effect:Effect::Plain,glow:false},0.0,1.0);
        line_y+=k*s+6.0*s;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn display_refreshes_twice_per_second_without_dropping_damage() {
        let mut dummy=Dummy::default();
        dummy.meter.hit(100,1.0);
        let initial=dummy.view(1.0).active.unwrap();
        dummy.meter.hit(250,1.125);
        assert_eq!(dummy.view(1.49).active.unwrap(),initial);
        assert_eq!(dummy.meter.active(1.49).unwrap().total,350);
        let updated=dummy.view(1.5);
        assert_eq!(updated.active.unwrap().total,350);
        assert_eq!(updated.last_hit,250);
        assert!(dummy.view(5.125).active.is_none());
        dummy.meter.tick(5.125);dummy.meter.hit(20,5.25);
        assert_eq!(dummy.view(5.25).active.unwrap().total,20);
    }
    use super::*;
    #[test]
    fn health_cap_adjustments_do_not_count_as_damage() {
        assert!(HEALTH.iter().all(|h|*h<=524_287)&&HEALTH.windows(2).all(|w|w[0]<w[1]));
        assert_eq!((health(0),health(1),health(3),health(9)),(HEALTH[0],HEALTH[0],HEALTH[2],HEALTH[2]));
        let mut meter=Meter::default();
        for frame in 0..626 {meter.hit(damage_since_refill(10_000_000,524_287,524_287),frame as f64/60.0);}
        assert_eq!(meter,Meter::default());
        for full in HEALTH {
            assert_eq!(damage_since_refill(full,full,full),0);
            assert_eq!(damage_since_refill(full,full-1234,full),1234);
            assert_eq!(damage_since_refill(full,0,full),full as u32);
        }
        assert_eq!(damage_since_refill(HEALTH[0],900,1000),100);
        assert_eq!(damage_since_refill(HEALTH[0],0,0),0);
    }
    #[test]
    fn hud_only_shows_during_damage_rounds() {
        let mut dummy=Dummy::default();
        assert!(dummy.view(0.0).active.is_none());
        dummy.meter.hit(10,1.0);
        assert!(dummy.view(4.9).active.is_some());
        assert!(dummy.view(5.0).active.is_none());
        dummy.meter.tick(5.1);
        assert!(dummy.view(6.0).active.is_none());
        dummy.meter.hit(20,7.0);
        assert_eq!(dummy.view(7.0).active.unwrap().total,20);
    }
    #[test]
    fn lateral_placement_rotates_with_player_heading() {
        let me=[10.0,2.0,20.0];
        assert_eq!(placement(me,0.0),[18.0,2.1,18.0]);
        let p=placement(me,std::f32::consts::FRAC_PI_2);
        assert!((p[0]-8.0).abs()<0.001&&(p[2]-12.0).abs()<0.001);
    }
    #[test]
    fn rounds_start_on_the_first_hit_and_settle_after_four_idle_seconds() {
        let mut meter=Meter::default();
        meter.hit(100,10.0);meter.hit(300,11.5);meter.hit(0,12.0);
        let round=meter.active(13.0).unwrap();
        assert_eq!((round.total,round.hits,round.biggest),(400,2,300));
        assert!((round.dps()-400.0/3.0).abs()<1e-9);
        meter.tick(15.0);assert!(meter.active(15.0).is_some()&&meter.finished.is_none());
        meter.tick(15.6);
        let done=meter.finished.unwrap();assert!(meter.active(16.0).is_none());
        assert_eq!((done.total,done.hits),(400,2));assert!((done.seconds-1.5).abs()<1e-9);
        assert!((done.dps()-400.0/1.5).abs()<1e-9);
        meter.hit(50,20.0);
        assert_eq!(meter.active(20.0).unwrap().total,50);assert_eq!(meter.finished.unwrap().total,400);
    }
    #[test]
    fn a_single_hit_counts_over_at_least_one_second() {
        let mut meter=Meter::default();meter.hit(900,0.0);meter.tick(5.0);
        assert_eq!(meter.finished.unwrap().dps(),900.0);
    }
}
