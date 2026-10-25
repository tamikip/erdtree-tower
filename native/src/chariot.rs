use std::time::Instant;
use eldenring::cs::WorldChrMan;

const ENTITY:u32=45011880;
const MAP:i32=0x2d010000;
const RADIUS:f32=18.0;

fn nearest(from:[f32;2],targets:&[[f32;2]])->Option<[f32;2]> {
    targets.iter().copied().filter(|p|p.iter().all(|v|v.is_finite())&&p[0].hypot(p[1])<=40.)
        .min_by(|a,b|((a[0]-from[0]).hypot(a[1]-from[1])).total_cmp(&(b[0]-from[0]).hypot(b[1]-from[1])))
}
fn advance(pose:[f32;3],target:Option<[f32;2]>,dt:f32)->[f32;3] {
    let Some(target)=target else{return pose;};
    let dt=dt.clamp(0.,0.05);
    let wanted=(pose[0]-target[0]).atan2(pose[1]-target[1]);
    let turn=(wanted-pose[2]+std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)-std::f32::consts::PI;
    let yaw=pose[2]+turn.clamp(-dt*1.5,dt*1.5);
    let distance=6.*dt;
    let mut p=[pose[0]-yaw.sin()*distance,pose[1]-yaw.cos()*distance,yaw];
    let radius=p[0].hypot(p[1]);
    if radius>RADIUS {p[0]*=RADIUS/radius;p[1]*=RADIUS/radius;}
    p
}

pub fn valid_pose(p:[f32;3])->bool {p.iter().all(|v|v.is_finite())&&p[0].hypot(p[1])<=RADIUS+0.01&&p[2].abs()<100.}

#[derive(Default)]
pub struct Chase {last:Option<Instant>,pose:Option<[f32;3]>,saved:Option<(eldenring::cs::FieldInsHandle,f32)>}
impl Chase {
    pub fn tick(&mut self,world:&mut WorldChrMan,mut link:Option<&mut super::party::Link>,enabled:bool)->Result<(),String> {
        let now=Instant::now();
        let active=enabled&&world.main_player.as_ref().is_some_and(|p|i32::from(p.current_block_id)==MAP)
            &&link.as_ref().is_none_or(|l|l.stopped.is_none()&&!l.defeated);
        if !active {
            if let Some((handle,speed))=self.saved.take(){if let Some(c)=world.chr_ins_by_handle_mut(&handle){if c.event_entity_id==ENTITY{c.modules.physics.motion_multiplier=speed;}}}
            if self.pose.is_some(){super::log("Chariot pursuit stopped; native motion restored");}
            self.last=None;self.pose=None;return Ok(());
        }
        let dt=self.last.replace(now).map_or(0.,|at|now.duration_since(at).as_secs_f32());
        let player=world.main_player.as_ref().unwrap();
        let origin=[player.chr_ins.modules.physics.position.0-player.block_position.x,player.chr_ins.modules.physics.position.2-player.block_position.z];
        let Some(car)=world.chr_sets.iter().flatten().flat_map(|s|s.characters()).find(|c|c.event_entity_id==ENTITY&&c.npc_id==8120&&!c.debug_flags.character_disabled()) else{return Ok(());};
        let handle=car.field_ins_handle;
        let initial=[car.modules.physics.position.0-origin[0],car.modules.physics.position.2-origin[1],car.modules.physics.orientation.to_euler_angles().1];
        if self.pose.is_none()&&!valid_pose(initial){return Err("战车初始位置异常，已停止追赶控制".into());}
        if self.saved.as_ref().is_none_or(|(h,_)|*h!=handle){self.saved=Some((handle,car.modules.physics.motion_multiplier));self.pose=None;}
        if self.pose.is_none(){super::log(format!("Chariot pursuit active: controller={} position={initial:?}",if link.as_ref().is_none_or(|l|l.host()){"host"}else{"replica"}));}
        let mut pose=self.pose.unwrap_or(initial);
        if link.as_ref().is_none_or(|l|l.host()) {
            let targets:Vec<_>=world.player_chr_set.characters().map(|p|&*p).chain(world.main_player.iter().map(|p|&**p))
                .filter(|p|i32::from(p.current_block_id)==MAP&&p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()&&!p.chr_ins.debug_flags.character_disabled())
                .map(|p|[p.block_position.x,p.block_position.z]).collect();
            pose=advance(pose,nearest([pose[0],pose[1]],&targets),dt);
            if let Some(l)=link.as_mut(){l.chariot_pose(pose)?;}
        }else if let Some((_,p,_))=link.as_ref().and_then(|l|l.event.chariot_pose) {
            // Follow the host only; never extrapolate a disconnected host's vehicle.
            let t=(dt*15.).clamp(0.,1.);
            pose=[pose[0]+(p[0]-pose[0])*t,pose[1]+(p[1]-pose[1])*t,p[2]];
        }
        self.pose=Some(pose);
        let Some(car)=world.chr_ins_by_handle_mut(&handle) else{return Ok(());};
        let physics=&mut car.modules.physics;
        physics.motion_multiplier=0.;
        physics.position.0=pose[0]+origin[0];physics.position.2=pose[1]+origin[1];
        physics.chr_proxy_pos_update_requested=true;
        let (s,c)=(pose[2]*0.5).sin_cos();
        physics.orientation=eldenring::rotation::Quaternion(0.,s,0.,c);
        physics.interpolated_orientation=physics.orientation;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selects_nearest_valid_player_and_retargets() {
        assert_eq!(nearest([0.,0.],&[[10.,0.],[3.,0.],[f32::NAN,0.],[100.,0.]]),Some([3.,0.]));
        assert_eq!(nearest([0.,0.],&[[10.,0.]]),Some([10.,0.]));
        assert_eq!(nearest([0.,0.],&[]),None);
    }
    #[test]
    fn bounded_turn_speed_and_no_teleport_after_stall() {
        let p=advance([0.,0.,0.],Some([0.,-10.]),0.05);
        assert!((p[1]+0.3).abs()<0.001);
        let p=advance([0.,0.,0.],Some([10.,0.]),10.);
        assert!(p[0].hypot(p[1])<=0.301&&p[2].abs()<=0.079);
        let p=advance([18.,0.,-std::f32::consts::FRAC_PI_2],Some([30.,0.]),0.05);
        assert!(p[0].hypot(p[1])<=18.001);
        assert_eq!(advance([1.,2.,3.],None,0.05),[1.,2.,3.]);
    }
}
