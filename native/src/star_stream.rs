use std::{ffi::c_void,sync::{Mutex,atomic::{AtomicUsize,Ordering}},time::{Instant,Duration}};
use eldenring::{cs::{ChrIns,CSCamera,CSCamExt},rotation::Quaternion};
use fromsoftware_shared::FromStatic;
use erdtree_tower::{Run,Phase};
use hudhook::mh::{MhHook,MH_EnableHook};

#[derive(Clone,Copy)]
struct Aim {owner:u64,direction:[f32;3],updated:Instant}
static AIM:Mutex<Option<Aim>>=Mutex::new(None);
static ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static POINT_TARGET:AtomicUsize=AtomicUsize::new(0);
#[repr(C,align(16))]
#[derive(Clone)]
struct SpawnData([u8;0x110]);
type Spawn=unsafe extern "C" fn(*mut c_void,*mut i32,*const SpawnData,*mut i32);
type PointTarget=unsafe extern "C" fn(*mut u8,*const [f32;4])->*mut u8;

fn channeling(id:i32)->bool {matches!(id,407045010|407045012|407045110|407045112)}
fn direction(vector:[f32;3])->Option<[f32;3]> {
    let length=vector.iter().map(|v|v*v).sum::<f32>().sqrt();
    (length.is_finite()&&length>0.001).then(||vector.map(|v|v/length))
}
fn body_yaw(dir:[f32;3])->f32 {(-dir[0]).atan2(-dir[2])}
fn horizontal_aim(vector:[f32;3])->Option<[f32;3]> {direction([vector[0],0.0,vector[2]])}
fn redirect(data:&mut SpawnData,aim:Aim)->bool {
    let owner=u64::from_le_bytes(data.0[0..8].try_into().unwrap());
    let bullet=i32::from_le_bytes(data.0[0x14..0x18].try_into().unwrap());
    if owner!=aim.owner||!(10420000..=10420004).contains(&bullet) {return false;}
    let Some(dir)=direction(aim.direction)else{return false;};
    data.0[0x20..0x28].fill(0xff);
    let right=direction([dir[2],0.0,-dir[0]]).unwrap_or([1.0,0.0,0.0]);
    let up=[dir[1]*right[2]-dir[2]*right[1],dir[2]*right[0]-dir[0]*right[2],dir[0]*right[1]-dir[1]*right[0]];
    for (offset,row) in [(0x50,right),(0x60,up),(0x70,dir)] {
        for (i,v) in row.into_iter().chain([0.0]).enumerate() {data.0[offset+i*4..offset+i*4+4].copy_from_slice(&v.to_le_bytes());}
    }
    true
}
unsafe extern "C" fn spawn(manager:*mut c_void,out:*mut i32,data:*const SpawnData,error:*mut i32) {
    let original:Spawn=unsafe {std::mem::transmute(ORIGINAL.load(Ordering::Acquire))};
    let aim=AIM.try_lock().ok().and_then(|s|*s).filter(|a|a.updated.elapsed()<Duration::from_millis(100));
    if let Some(aim)=aim.filter(|_|!data.is_null()&&!super::input::CAPTURE.load(Ordering::Relaxed)) {
        let mut local=unsafe {(*data).clone()};
        if redirect(&mut local,aim) {
            let mut point=[0.0,0.0,0.0,1.0];
            for (i,value) in point[..3].iter_mut().enumerate() {
                *value=f32::from_le_bytes(local.0[0x80+i*4..0x84+i*4].try_into().unwrap())+aim.direction[i]*100.0;
            }
            if point.iter().all(|v|v.is_finite()) {
                let target:PointTarget=unsafe {std::mem::transmute(POINT_TARGET.load(Ordering::Acquire))};
                local.0[0xb0..0x100].fill(0);
                unsafe {target(local.0.as_mut_ptr().add(0xb0),&point)};
                if local.0[0xb0]==4 {unsafe {original(manager,out,&local,error)};return;}
            }
        }
    }
    unsafe {original(manager,out,data,error)};
}
pub fn ready()->bool {ORIGINAL.load(Ordering::Acquire)!=0}
pub fn clear() {if let Ok(mut aim)=AIM.lock(){*aim=None;}}
pub unsafe fn install()->Result<(),String> {
    let target=super::items::checked_function(0x395290,&[0x48,0x89,0x4c,0x24,0x08,0x57,0x48,0x83,0xec,0x30])?;
    let address=super::items::checked_function(0x3a2cb0,&[0x40,0x53,0x55,0x56,0x57,0x48,0x81,0xec,0x98,0x07,0x00,0x00])? as *mut c_void;
    if size_of::<eldenring::cs::BulletSpawnData>()!=size_of::<SpawnData>() {return Err("Bullet spawn layout mismatch".into());}
    let hook=unsafe {MhHook::new(address,spawn as *mut c_void)}.map_err(|e|format!("Star stream hook: {e:?}"))?;
    POINT_TARGET.store(target as usize,Ordering::Release);
    ORIGINAL.store(hook.trampoline() as usize,Ordering::Release);
    if let Err(error)=unsafe {MH_EnableHook(address)}.ok() {
        ORIGINAL.store(0,Ordering::Release);return Err(format!("Star stream enable: {error:?}"));
    }
    super::log("Star stream ready; horizontal aim");
    Ok(())
}
pub fn tick(chr:&mut ChrIns,run:Option<&Run>) {
    if !ready()||!run.is_some_and(|r|r.star_stream&&!matches!(r.phase,Phase::Dead|Phase::Won))
        ||chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag()
        ||super::input::CAPTURE.load(Ordering::Relaxed) {return;}
    let Ok(camera)=(unsafe {CSCamera::instance()})else{return;};
    let forward=camera.pers_cam_1.forward();
    let Some(dir)=horizontal_aim([forward.0,forward.1,forward.2])else{return;};
    let owner=unsafe {std::ptr::read_unaligned((&chr.field_ins_handle as *const eldenring::cs::FieldInsHandle).cast::<u64>())};
    if let Ok(mut aim)=AIM.lock(){*aim=Some(Aim{owner,direction:dir,updated:Instant::now()});}
    if !channeling(chr.modules.action_request.action_request_queue.current_tae_id)||dir[0]*dir[0]+dir[2]*dir[2]<0.001 {return;}
    let (s,c)=(body_yaw(dir)*0.5).sin_cos();
    chr.modules.physics.orientation=Quaternion(0.0,s,0.0,c);
    chr.modules.physics.interpolated_orientation=chr.modules.physics.orientation;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pitch_does_not_change_beam_height_or_heading() {
        for pitch in [-0.99,0.0,0.99] {
            let dir=horizontal_aim([0.6,pitch,0.8]).unwrap();
            assert_eq!(dir,[0.6,0.0,0.8]);
        }
        assert!(horizontal_aim([0.0,1.0,0.0]).is_none());
    }
    #[test]
    fn body_faces_camera_aim_using_native_negative_z() {
        for dir in [[0.0,0.0,1.0],[1.0,0.0,0.0],[0.0,0.0,-1.0],[-1.0,0.0,0.0],[0.6,0.0,0.8]] {
            let yaw=body_yaw(dir);
            assert!((-yaw.sin()-dir[0]).abs()<0.0001);
            assert!((-yaw.cos()-dir[2]).abs()<0.0001);
        }
    }
    #[test]
    fn only_comet_emission_animations_can_turn() {
        for id in [407045010,407045012,407045110,407045112] {assert!(channeling(id));}
        for id in [0,45012,407045000,407045011,407045111,407145010,492045010,-1] {assert!(!channeling(id));}
    }
    #[test]
    fn redirects_only_local_comet_and_preserves_every_other_spawn_field() {
        let mut source=SpawnData([0x3a;0x110]);source.0[..8].copy_from_slice(&42u64.to_le_bytes());
        source.0[0x14..0x18].copy_from_slice(&10420001i32.to_le_bytes());
        let aim=Aim{owner:42,direction:[1.0,2.0,3.0],updated:Instant::now()};
        let mut data=source.clone();assert!(redirect(&mut data,aim));
        for i in 0..0x110 {if !(0x20..0x28).contains(&i)&&!(0x50..0x80).contains(&i) {assert_eq!(data.0[i],source.0[i]);}}
        let dir=direction(aim.direction).unwrap();
        for (i,v) in dir.iter().enumerate(){assert_eq!(f32::from_le_bytes(data.0[0x70+i*4..0x74+i*4].try_into().unwrap()),*v);}
        assert!(!redirect(&mut source.clone(),Aim{owner:43,..aim}));
        for id in [10419999,10420005,10000000] {
            let mut other=source.clone();other.0[0x14..0x18].copy_from_slice(&i32::to_le_bytes(id));
            let before=other.clone();assert!(!redirect(&mut other,aim));assert_eq!(other.0,before.0);
        }
        assert!(direction([0.0;3]).is_none());assert!(direction([f32::NAN,0.0,1.0]).is_none());
        for axis in [[0.0,1.0,0.0],[0.0,-1.0,0.0],[-1.0,0.0,0.0]] {assert_eq!(direction(axis),Some(axis));}
    }
    #[test]
    fn aim_basis_is_orthonormal_even_when_looking_vertical() {
        for dir in [[1.0,2.0,3.0],[0.0,1.0,0.0],[0.0,-1.0,0.0],[0.0,0.0,-1.0]] {
            let mut data=SpawnData([0;0x110]);data.0[0x14..0x18].copy_from_slice(&10420000i32.to_le_bytes());
            assert!(redirect(&mut data,Aim{owner:0,direction:dir,updated:Instant::now()}));
            let rows:[[_;3];3]=std::array::from_fn(|r|std::array::from_fn(|c|f32::from_le_bytes(data.0[0x50+r*16+c*4..0x54+r*16+c*4].try_into().unwrap())));
            for a in 0..3 {for b in 0..3 {let dot=(0..3).map(|i|rows[a][i]*rows[b][i]).sum::<f32>();assert!((dot-if a==b{1.0}else{0.0}).abs()<0.0001);}}
        }
    }
}
