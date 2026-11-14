use std::{ffi::c_void,sync::{Mutex,atomic::{AtomicUsize,Ordering}},time::{Duration,Instant}};
use eldenring::cs::{GameMan,PlayerIns,WorldChrMan};
use super::party_death::Position;

type Request=unsafe extern "C" fn(*mut u8,bool,bool);
static BASE:AtomicUsize=AtomicUsize::new(0);
static PROFILE:AtomicUsize=AtomicUsize::new(0);
static FINISHED:AtomicUsize=AtomicUsize::new(0);
static PENDING:Mutex<Option<Pending>>=Mutex::new(None);
struct Build {
    version:&'static str,timestamp:u32,image_size:u32,root:usize,
    object_delta:usize,component_delta:usize,service_delta:usize,forward_delta:usize,
    request:usize,callbacks:[usize;3],signatures:&'static [(usize,usize,u64)],
    root_locator:usize,root_bytes:[u8;14],
}
const BUILDS:[Build;3]=[
    Build{version:"2.0.1",timestamp:0x6a98a913,image_size:0xd31000,root:0x222030,
        object_delta:0xad0,component_delta:0x1848,service_delta:0xa88,forward_delta:0x480,
        request:0xd7ec0,callbacks:[0xd7fd0,0xd8180,0xd81f0],
        signatures:&[(0xd7bb0,0x303,0x3b87e0ebc95fb1c3),(0xd7ec0,0x108,0xe8e01219eedfa0cf),
            (0xd7fd0,0x1aa,0xa72e59a70f316a03),(0xd8180,0x69,0xe759d135c29ca357),(0xd81f0,0x123,0x6772508514e31e45)],
        root_locator:0x32c09,root_bytes:[0x48,0x8d,0x85,0xe0,0x2f,0,0,0x48,0x89,0x05,0x19,0xf4,0x1e,0]},
    // Verified against ersc.dll SHA256 0a093d8490967bf19c62f895fdf1fda21840282d2b2b371b9b4fc24a0cfe146e.
    Build{version:"2.0.2",timestamp:0x6ae0ad1a,image_size:0xd65000,root:0x224030,
        object_delta:0xb20,component_delta:0x19c0,service_delta:0xd30,forward_delta:0x3c0,
        request:0xda6d0,callbacks:[0xda7e0,0xda990,0xdaa00],
        signatures:&[(0xda3c0,0x303,0xce66d6b08685636a),(0xda6d0,0x108,0xf93589627953ac40),
            (0xda7e0,0x1aa,0xa72e59a70f316a03),(0xda990,0x69,0xe759d135c29ca357),(0xdaa00,0x123,0x65be19c77b1bafad),
            (0x36185,0x74,0xb7e4c3391be820e8)],
        root_locator:0x33049,root_bytes:[0x48,0x8d,0x85,0x90,0x36,0,0,0x48,0x89,0x05,0xd9,0x0f,0x1f,0]},
    // ersc.dll SHA256 2ebde62be864ba3d759728aac44c459f0f813e8b6d7a342304ce68f6513f280c.
    Build{version:"2.0.3",timestamp:0x6af4b20a,image_size:0xd67000,root:0x226030,
        object_delta:0x970,component_delta:0x1880,service_delta:0xbf8,forward_delta:0x3d0,
        request:0xdc190,callbacks:[0xdc2a0,0xdc450,0xdc4c0],
        signatures:&[(0xdbe80,0x303,0x77b92ef7f7c66dc5),(0xdc190,0x108,0x719d0205fb569cd3),
            (0xdc2a0,0x1aa,0xa72e59a70f316a03),(0xdc450,0x69,0xe759d135c29ca357),(0xdc4c0,0x123,0xf498f37eea7fbed4),
            (0x361d3,0x74,0x548231910211e55a)],
        root_locator:0x33089,root_bytes:[0x48,0x8d,0x85,0xb0,0x33,0,0,0x48,0x89,0x05,0x99,0x2f,0x1f,0]},
];
fn select_build(timestamp:u32,image_size:u32)->Option<&'static Build> {
    BUILDS.iter().find(|b|b.timestamp==timestamp&&b.image_size==image_size)
}
#[link(name="kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn ReadProcessMemory(process:*mut c_void,address:*const c_void,buffer:*mut c_void,size:usize,read:*mut usize)->i32;
}
trait Memory {
    fn bytes<const N:usize>(&self,address:usize)->Option<[u8;N]>;
    fn pointer(&self,address:usize)->Option<usize>{self.bytes::<8>(address).map(|b|u64::from_le_bytes(b) as usize)}
    fn word(&self,address:usize)->Option<u32>{self.bytes::<4>(address).map(u32::from_le_bytes)}
}
struct Local;
impl Memory for Local {
    fn bytes<const N:usize>(&self,address:usize)->Option<[u8;N]> {
        if address<0x10000||address.checked_add(N).is_none(){return None;}
        let mut bytes=[0;N];let mut read=0;
        let ok=unsafe {ReadProcessMemory(-1isize as *mut c_void,address as *const c_void,bytes.as_mut_ptr().cast(),N,&mut read)};
        (ok!=0&&read==N).then_some(bytes)
    }
}
#[derive(Clone,Copy)]
struct Spectator {root:usize,address:usize,target:usize,key:u32}
fn valid_address(address:usize)->bool {(0x10000..=0x7fffffff0000).contains(&address)}
fn locate(memory:&impl Memory,base:usize,world:usize,build:&Build)->Option<Spectator> {
    let root=memory.pointer(base+build.root)?;
    if !valid_address(root){return None;}
    let address=root.checked_sub(build.object_delta)?;
    if address<0x10000{return None;}
    // Verified against the pinned initializer and the live object; never scan or guess during recovery.
    let component=memory.pointer(address+8)?;
    if component!=address.checked_sub(build.component_delta)?
        ||memory.pointer(address+0x20)?!=address.checked_sub(build.service_delta)?
        ||memory.pointer(address+0x30)?!=address+build.forward_delta
        ||memory.pointer(address+0x38)?!=address.checked_sub(0xe0)?
        ||memory.pointer(memory.pointer(component)?)?!=world {return None;}
    let key=memory.word(address+0xb0)?;
    if key>2||memory.word(address+0xac)?==i32::MAX as u32||memory.pointer(address+0xc8)?!=3{return None;}
    let head=memory.pointer(address+0xc0)?;
    if !valid_address(head){return None;}
    if memory.bytes::<1>(head+0x19)?[0]!=1{return None;}
    let mut todo=vec![memory.pointer(head+8)?];let mut visited=Vec::new();let mut keys=0u8;
    while let Some(node)=todo.pop() {
        if node==head{continue;}
        if !valid_address(node)||visited.len()==3||visited.contains(&node)||memory.bytes::<1>(node+0x19)?[0]!=0{return None;}
        visited.push(node);
        let key=memory.word(node+0x20)?;
        let callback=*build.callbacks.get(key as usize)?;
        if keys&(1<<key)!=0||memory.pointer(node+0x30)?!=base+callback{return None;}
        keys|=1<<key;
        todo.extend([memory.pointer(node)?,memory.pointer(node+0x10)?]);
    }
    if keys!=7{return None;}
    Some(Spectator{root,address,target:memory.pointer(address+0x58)?,key})
}
struct Pending {player:usize,owner:u32,origin:Position,block_goal:Option<Position>,relocate:bool,fp:i32,started:Instant,sent:bool,stable:u8,reported_lookup:bool}
pub fn available()->bool {BASE.load(Ordering::Acquire)!=0}
pub fn verified_module()->Option<(&'static str,std::path::PathBuf)> {
    let base=BASE.load(Ordering::Acquire);if base==0{return None;}
    Some((BUILDS.get(PROFILE.load(Ordering::Acquire))?.version,super::diagnostics::module_path(base)?))
}
pub fn finished(player:usize)->bool {player!=0&&FINISHED.load(Ordering::Acquire)==player}
pub fn owned_address<T>(player:&fromsoftware_shared::OwnedPtr<T>)->usize {player.as_ptr() as usize}
pub fn cancel(){if let Ok(mut p)=PENDING.lock(){*p=None;}FINISHED.store(0,Ordering::Release);}
pub fn request(player:&PlayerIns,origin:Position)->Result<(),String> {
    request_inner(player,origin,false,None)
}
pub fn request_at_safe_block_goal(player:&PlayerIns,goal:Position)->Result<(),String> {
    let (block,physics)=positions(player);
    let origin=goal.in_physics_space(block,physics).ok_or("复活目标与当前地图不匹配")?;
    request_inner(player,origin,true,Some(goal))
}
fn positions(player:&PlayerIns)->(Position,Position) {
    let block=i32::from(player.current_block_id);let b=&player.block_position;let h=&player.chr_ins.modules.physics.position;
    (Position{block,xyz:[b.x,b.y,b.z]},Position{block,xyz:[h.0,h.1,h.2]})
}
fn request_inner(player:&PlayerIns,origin:Position,relocate:bool,block_goal:Option<Position>)->Result<(),String> {
    if !available(){return Err("无缝复活请求接口未通过版本校验".into());}
    if !origin.xyz.iter().all(|p|p.is_finite()){return Err("死亡位置无效，未请求复活".into());}
    let mut pending=PENDING.lock().map_err(|_|"复活状态锁不可用")?;
    if pending.is_none() {
        *pending=Some(Pending{player:player as *const _ as usize,owner:unsafe {player.player_game_data.as_ref()}.character_id,
            origin,block_goal,relocate,fp:player.chr_ins.modules.data.fp,started:Instant::now(),sent:false,stable:0,reported_lookup:false});
        FINISHED.store(0,Ordering::Release);
    }
    Ok(())
}
fn recovered(origin:Position,current:Position,alive:bool,sent:bool,target:usize,key:u32)->bool {
    sent&&alive&&target==usize::MAX&&key==0&&origin.block==current.block
        &&origin.xyz.iter().zip(current.xyz).map(|(a,b)|(a-b).powi(2)).sum::<f32>()<=4.0
}
fn safe_landing(goal:Position,current:Position,grounded:bool,falling:bool)->bool {
    grounded&&!falling&&goal.block==current.block
        &&goal.xyz.iter().zip(current.xyz).map(|(a,b)|(a-b).powi(2)).sum::<f32>()<=4.0
}
#[derive(Debug,PartialEq)]
enum Placement {Wait,Move,Ready}
fn placement(initial:bool,goal:Option<Position>,current:Position,grounded:bool,falling:bool,proxy_pending:bool)->Placement {
    if proxy_pending{return Placement::Wait;}
    let Some(goal)=goal else{return if initial{Placement::Move}else{Placement::Ready};};
    if goal.block<0||goal.block!=current.block||!goal.xyz.iter().chain(&current.xyz).all(|v|v.is_finite()){return Placement::Wait;}
    let distance=goal.xyz.iter().zip(current.xyz).map(|(a,b)|(a-b).powi(2)).sum::<f32>();
    if !distance.is_finite(){return Placement::Wait;}
    if initial||distance>4.0{return Placement::Move;}
    // Near the destination, let physics settle instead of resetting its position every tick.
    if safe_landing(goal,current,grounded,falling){Placement::Ready}else{Placement::Wait}
}
pub fn tick(world:&mut WorldChrMan,game:&GameMan) {
    let Ok(mut queue)=PENDING.try_lock() else{return;};
    let Some(pending)=queue.as_mut() else{return;};
    if pending.started.elapsed()>Duration::from_secs(30) {
        let position=world.main_player.as_ref().map(|p|(positions(p),p.chr_ins.modules.physics.standing_on_solid_ground,p.chr_ins.modules.physics.is_falling));
        super::log(format!("PARTY_REVIVE timeout: spectator exit or safe ground not confirmed; block_goal={:?} current={position:?}; no reward advancement",pending.block_goal));*queue=None;return;
    }
    if game.warp_requested{*queue=None;return;}
    let world_address=world as *const _ as usize;
    let Some(player)=world.main_player.as_mut() else{return;};
    if owned_address(player)!=pending.player||unsafe {player.player_game_data.as_ref()}.character_id!=pending.owner
        ||i32::from(player.current_block_id)!=pending.origin.block {
        super::log(format!("PARTY_REVIVE cancelled: expected_player={:#x} actual_player={:#x} expected_owner={} actual_owner={} expected_block={:#x} actual_block={:#x}",
            pending.player,owned_address(player),pending.owner,unsafe {player.player_game_data.as_ref()}.character_id,pending.origin.block,i32::from(player.current_block_id)));
        *queue=None;return;
    }
    let build=&BUILDS[PROFILE.load(Ordering::Acquire)];
    let Some(spectator)=locate(&Local,BASE.load(Ordering::Acquire),world_address,build) else{
        if !pending.reported_lookup {
            pending.reported_lookup=true;
            super::log(format!("PARTY_REVIVE waiting for validated object: version={} root={:?} world={world_address:#x}",build.version,Local.pointer(BASE.load(Ordering::Acquire)+build.root)));
        }
        return;
    };
    let alive=player.chr_ins.modules.data.hp>0&&!player.chr_ins.chr_flags1c5.death_flag();
    if !pending.sent {
        if alive||spectator.target==usize::MAX{return;}
        let base=BASE.load(Ordering::Acquire);
        if Local.pointer(base+build.root)!=Some(spectator.root){return;}
        let request:Request=unsafe {std::mem::transmute(base+build.request)};
        // The native mutex and atomic transition handle recovery; false keeps the request local.
        unsafe {request(spectator.address as *mut u8,true,false)};
        pending.sent=true;
        super::log(format!("PARTY_REVIVE native request submitted without detour target={:#x}",spectator.target));
        return;
    }
    let placement=placement(pending.relocate,pending.block_goal,positions(player).0,
        player.chr_ins.modules.physics.standing_on_solid_ground,player.chr_ins.modules.physics.is_falling,
        player.chr_ins.modules.physics.chr_proxy_pos_update_requested);
    if placement==Placement::Move&&alive&&pending.sent&&spectator.target==usize::MAX&&spectator.key==0 {
        if let Some(goal)=pending.block_goal {
            let (block,physics)=positions(player);
            let Some(origin)=goal.in_physics_space(block,physics) else{return;};
            pending.origin=origin;
        }
        let physics=&mut player.chr_ins.modules.physics;
        physics.position.0=pending.origin.xyz[0];physics.position.1=pending.origin.xyz[1];physics.position.2=pending.origin.xyz[2];
        physics.last_update_position=physics.position;physics.chr_proxy_pos_update_requested=true;
        if pending.relocate {super::log(format!("PARTY_REVIVE moved to safe goal: physics={:?} block={:?}; waiting for ground",pending.origin,pending.block_goal));}
        pending.relocate=false;pending.stable=0;
        return;
    }
    let (block,physics)=positions(player);
    let (origin,current)=pending.block_goal.map_or((pending.origin,physics),|goal|(goal,block));
    if placement!=Placement::Ready||!recovered(origin,current,alive,pending.sent,spectator.target,spectator.key){pending.stable=0;return;}
    player.chr_ins.modules.data.hp=1;
    player.chr_ins.modules.data.fp=pending.fp.min(player.chr_ins.modules.data.max_fp);
    pending.stable+=1;
    if pending.stable>=2 {
        FINISHED.store(pending.player,Ordering::Release);
        super::log("PARTY_REVIVE native exit confirmed at recovery goal; HP=1, FP preserved");
        *queue=None;
    }
}
pub unsafe fn install()->Result<(),String> {
    if std::env::var("ERDTREE_TOWER_DISABLE_REVIVAL").as_deref()==Ok("1"){return Err("Recovery disabled by diagnostic switch".into());}
    let name:Vec<_>="ersc.dll\0".encode_utf16().collect();
    let base=unsafe {GetModuleHandleW(name.as_ptr())} as usize;
    if base==0{return Err("Seamless not loaded".into());}
    let pe=Local.word(base+0x3c).filter(|p|*p<=0x1000).ok_or("Invalid Seamless PE header")? as usize;
    let build=select_build(Local.word(base+pe+8).ok_or("Missing Seamless timestamp")?,Local.word(base+pe+24+56).ok_or("Missing Seamless image size")?).ok_or("Unsupported Seamless build")?;
    validate(&Local,base,build)?;
    PROFILE.store(BUILDS.iter().position(|b|std::ptr::eq(b,build)).ok_or("Unknown recovery profile")?,Ordering::Release);
    BASE.store(base,Ordering::Release);
    super::log(format!("PARTY_REVIVE native request interface ready; Seamless {}; no spectator code hooks installed",build.version));
    Ok(())
}
fn validate(memory:&impl Memory,base:usize,build:&Build)->Result<(),String> {
    for &(rva,len,expected) in build.signatures {
        let mut hash=14695981039346656037u64;
        for i in 0..len {let b=memory.bytes::<1>(base+rva+i).ok_or("Unreadable recovery function")?[0];hash=(hash^u64::from(b)).wrapping_mul(1099511628211);}
        if hash!=expected{return Err(format!("Recovery signature mismatch at {rva:x}"));}
    }
    if memory.bytes::<14>(base+build.root_locator)!=Some(build.root_bytes){return Err("Recovery root locator mismatch".into());}
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn placement_waits_for_proxy_ack_then_allows_natural_landing() {
        let goal=Position{block:0x0d000000,xyz:[21.85,1009.5,300.43]};
        let remote=Position{xyz:[0.0,900.0,0.0],..goal};
        assert_eq!(placement(true,Some(goal),remote,false,false,true),Placement::Wait);
        assert_eq!(placement(true,Some(goal),remote,false,false,false),Placement::Move);
        for _ in 0..120 {
            assert_eq!(placement(false,Some(goal),remote,false,false,true),Placement::Wait);
        }
        let airborne=Position{xyz:[21.85,1010.0,300.43],..goal};
        assert_eq!(placement(false,Some(goal),airborne,false,true,false),Placement::Wait);
        assert_eq!(placement(false,Some(goal),airborne,false,false,false),Placement::Wait);
        assert_eq!(placement(false,Some(goal),goal,true,false,false),Placement::Ready);
        assert_eq!(placement(false,Some(goal),remote,false,true,false),Placement::Move);
    }
    #[test]
    fn placement_never_finishes_from_stale_ground_while_proxy_update_is_pending() {
        let goal=Position{block:45,xyz:[1.0,2.0,3.0]};
        for target in [None,Some(goal)] {
            assert_eq!(placement(false,target,goal,true,false,true),Placement::Wait);
            assert_eq!(placement(false,target,goal,true,false,false),Placement::Ready);
        }
    }
    #[test]
    fn placement_rejects_unloaded_or_invalid_coordinates_before_moving() {
        let goal=Position{block:45,xyz:[1.0,2.0,3.0]};
        for current in [Position{block:-1,..goal},Position{block:46,..goal},
            Position{xyz:[f32::NAN,0.0,0.0],..goal},Position{xyz:[f32::INFINITY,0.0,0.0],..goal}] {
            assert_eq!(placement(true,Some(goal),current,false,false,false),Placement::Wait);
        }
        assert_eq!(placement(false,None,goal,false,true,false),Placement::Ready);
    }
    #[test]
    fn safe_goal_waits_for_loaded_ground_and_rejects_falling() {
        let goal=Position{block:0x0a000000,xyz:[-202.71,81.864,314.077]};
        assert!(safe_landing(goal,goal,true,false));
        assert!(!safe_landing(goal,goal,false,false));
        assert!(!safe_landing(goal,goal,true,true));
        assert!(!safe_landing(goal,Position{block:0x0d000000,..goal},true,false));
        assert!(!safe_landing(goal,Position{xyz:[-114.71,65.864,298.077],..goal},true,false));
        assert!(!safe_landing(goal,Position{xyz:[f32::NAN,0.0,0.0],..goal},true,false));
    }
    #[test]
    fn build_selection_rejects_unknown_or_mixed_headers() {
        assert_eq!(select_build(0x6a98a913,0xd31000).unwrap().version,"2.0.1");
        assert_eq!(select_build(0x6ae0ad1a,0xd65000).unwrap().version,"2.0.2");
        assert_eq!(select_build(0x6af4b20a,0xd67000).unwrap().version,"2.0.3");
        assert!(select_build(0x6af4b20a,0xd65000).is_none());
        assert!(select_build(0x6ae0ad1a,0xd31000).is_none());
        assert!(select_build(0,0xd65000).is_none());
    }
    #[test]
    #[ignore = "requires locally installed Seamless DLL fixtures"]
    fn real_dll_signatures_match_supported_versions_and_reject_modified_code() {
        struct Image {data:Vec<u8>,sections:Vec<(usize,usize,usize)>}
        impl Memory for Image {
            fn bytes<const N:usize>(&self,address:usize)->Option<[u8;N]> {
                let rva=address.checked_sub(0x180000000)?;
                let &(start,len,raw)=self.sections.iter().find(|(s,n,_)|rva>=*s&&rva.checked_add(N).is_some_and(|end|end<=s+n))?;
                let _=len;
                self.data.get(raw+rva-start..raw+rva-start+N)?.try_into().ok()
            }
        }
        let selected=std::env::var("ERDTREE_REVIVAL_FIXTURE_VERSION").ok();
        assert!(selected.as_ref().is_none_or(|v|BUILDS.iter().any(|b|b.version==v)));
        for (env,build) in [("ERDTREE_REVIVAL_DLL_201",&BUILDS[0]),("ERDTREE_REVIVAL_DLL_202",&BUILDS[1]),("ERDTREE_REVIVAL_DLL_203",&BUILDS[2])] {
            if selected.as_ref().is_some_and(|v|v!=build.version){continue;}
            let data=std::fs::read(std::env::var(env).expect("set fixture path")).unwrap();
            let u16at=|at|u16::from_le_bytes(data[at..at+2].try_into().unwrap()) as usize;
            let u32at=|at|u32::from_le_bytes(data[at..at+4].try_into().unwrap()) as usize;
            let pe=u32at(0x3c);let section=pe+24+u16at(pe+20);
            assert_eq!(select_build(u32at(pe+8) as u32,u32at(pe+80) as u32).unwrap().version,build.version);
            let sections=(0..u16at(pe+6)).map(|i|{let s=section+i*40;(u32at(s+12),u32at(s+16),u32at(s+20))}).collect();
            let mut image=Image{data,sections};
            assert!(validate(&image,0x180000000,build).is_ok());
            let &(start,_,raw)=image.sections.iter().find(|(s,n,_)|build.request>=*s&&build.request<s+n).unwrap();
            image.data[raw+build.request-start]^=1;
            assert!(validate(&image,0x180000000,build).unwrap_err().contains("signature mismatch"));
        }
    }
    #[test]
    fn pending_identity_uses_owned_player_target_not_container_slot() {
        use std::{mem::ManuallyDrop,ptr::NonNull};
        let mut value=42u64;
        let raw=NonNull::from(&mut value);
        let owned=ManuallyDrop::new(unsafe {std::mem::transmute::<NonNull<u64>,fromsoftware_shared::OwnedPtr<u64>>(raw)});
        assert_ne!(&*owned as *const _ as usize,raw.as_ptr() as usize);
        assert_eq!(owned_address(&owned),raw.as_ptr() as usize);
    }
    #[derive(Default)]struct Fake(BTreeMap<usize,u8>);
    impl Memory for Fake {
        fn bytes<const N:usize>(&self,address:usize)->Option<[u8;N]> {
            let b=(0..N).map(|i|self.0.get(&(address+i)).copied()).collect::<Option<Vec<_>>>()?;b.try_into().ok()
        }
    }
    impl Fake {
        fn put(&mut self,at:usize,b:&[u8]){for (i,b) in b.iter().enumerate(){self.0.insert(at+i,*b);}}
        fn put_pointer(&mut self,at:usize,value:usize){self.put(at,&value.to_le_bytes());}
        fn put_word(&mut self,at:usize,value:u32){self.put(at,&value.to_le_bytes());}
    }
    fn fixture(build:&Build)->Fake {
        let mut m=Fake::default();let (base,object,head,world)=(0x180000000,0x30000,0x40000,0x50000);
        m.put_pointer(base+build.root,object+build.object_delta);m.put_pointer(object+8,object-build.component_delta);m.put_pointer(object-build.component_delta,0x60000);m.put_pointer(0x60000,world);
        m.put_pointer(object+0x20,object-build.service_delta);m.put_pointer(object+0x30,object+build.forward_delta);m.put_pointer(object+0x38,object-0xe0);
        m.put_word(object+0xb0,0);m.put_word(object+0xac,0);m.put_pointer(object+0x58,usize::MAX);m.put_pointer(object+0xc0,head);m.put_pointer(object+0xc8,3);
        m.put(head+0x19,&[1]);m.put_pointer(head+8,0x41000);
        for (key,callback) in build.callbacks.into_iter().enumerate() {
            let node=0x41000+key*0x100;
            m.put(node+0x19,&[0]);m.put_word(node+0x20,key as u32);m.put_pointer(node+0x30,base+callback);
            m.put_pointer(node,head);m.put_pointer(node+0x10,if key==2 {head}else{node+0x100});
        }
        m
    }
    #[test]fn locator_requires_matching_world_and_all_native_callbacks() {
        for build in &BUILDS {
            let m=fixture(build);assert_eq!(locate(&m,0x180000000,0x50000,build).unwrap().address,0x30000);
            assert!(locate(&m,0x180000000,0x50001,build).is_none());
            let mut m=fixture(build);m.put_pointer(0x41100+0x30,0x180000000+build.callbacks[2]);assert!(locate(&m,0x180000000,0x50000,build).is_none());
        }
    }
    #[test]fn corrupt_or_expired_locator_is_rejected_before_request() {
        for build in &BUILDS {for (at,value) in [(0x180000000+build.root,0),(0x180000000+build.root,usize::MAX),(0x30000+8,0x29999),(0x30000+0xc8,4),(0x30000+0xc0,usize::MAX),(0x41100+0x10,0x41000)] {
            let mut m=fixture(build);m.put_pointer(at,value);assert!(locate(&m,0x180000000,0x50000,build).is_none());
        }}
    }
    #[test]fn old_layout_is_rejected_for_new_version_even_if_root_is_present() {
        for old in &BUILDS {for new in &BUILDS {
            if old.version==new.version{continue;}
            let mut m=fixture(old);
            m.put_pointer(0x180000000+new.root,0x30000+new.object_delta);
            assert!(locate(&m,0x180000000,0x50000,new).is_none());
        }}
    }
    #[test]fn positive_hp_alone_does_not_confirm_spectator_exit() {
        let p=Position{block:45,xyz:[1.0,2.0,3.0]};
        assert!(recovered(p,p,true,true,usize::MAX,0));
        for (alive,sent,target,key) in [(false,true,usize::MAX,0),(true,false,usize::MAX,0),(true,true,42,0),(true,true,usize::MAX,2)] {
            assert!(!recovered(p,p,alive,sent,target,key));
        }
        assert!(!recovered(p,Position{block:11,..p},true,true,usize::MAX,0));
        assert!(!recovered(p,Position{xyz:[9.0,2.0,3.0],..p},true,true,usize::MAX,0));
    }
}
