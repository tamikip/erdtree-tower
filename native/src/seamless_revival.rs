use std::{ffi::c_void,sync::{Mutex,atomic::{AtomicUsize,Ordering}},time::{Duration,Instant}};
use eldenring::cs::{GameMan,PlayerIns,WorldChrMan};
use super::party_death::Position;

type Request=unsafe extern "C" fn(*mut u8,bool,bool);
static BASE:AtomicUsize=AtomicUsize::new(0);
static FINISHED:AtomicUsize=AtomicUsize::new(0);
static PENDING:Mutex<Option<Pending>>=Mutex::new(None);
const ROOT:usize=0x222030;
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
fn locate(memory:&impl Memory,base:usize,world:usize)->Option<Spectator> {
    let root=memory.pointer(base+ROOT)?;
    if !valid_address(root){return None;}
    let address=root.checked_sub(0xad0)?;
    if address<0x10000{return None;}
    // Verified against the pinned initializer and the live object; never scan or guess during recovery.
    let component=memory.pointer(address+8)?;
    if component!=address.checked_sub(0x1848)?
        ||memory.pointer(address+0x20)?!=address.checked_sub(0xa88)?
        ||memory.pointer(address+0x30)?!=address+0x480
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
        let callback=match key {0=>0xd7fd0,1=>0xd8180,2=>0xd81f0,_=>return None};
        if keys&(1<<key)!=0||memory.pointer(node+0x30)?!=base+callback{return None;}
        keys|=1<<key;
        todo.extend([memory.pointer(node)?,memory.pointer(node+0x10)?]);
    }
    if keys!=7{return None;}
    Some(Spectator{root,address,target:memory.pointer(address+0x58)?,key})
}
struct Pending {player:usize,owner:u32,origin:Position,fp:i32,started:Instant,sent:bool,stable:u8,reported_lookup:bool}
pub fn available()->bool {BASE.load(Ordering::Acquire)!=0}
pub fn finished(player:usize)->bool {player!=0&&FINISHED.load(Ordering::Acquire)==player}
pub fn owned_address<T>(player:&fromsoftware_shared::OwnedPtr<T>)->usize {player.as_ptr() as usize}
pub fn cancel(){if let Ok(mut p)=PENDING.lock(){*p=None;}FINISHED.store(0,Ordering::Release);}
pub fn request(player:&PlayerIns,origin:Position)->Result<(),String> {
    if !available(){return Err("无缝复活请求接口未通过版本校验".into());}
    if !origin.xyz.iter().all(|p|p.is_finite()){return Err("死亡位置无效，未请求复活".into());}
    let mut pending=PENDING.lock().map_err(|_|"复活状态锁不可用")?;
    if pending.is_none() {
        *pending=Some(Pending{player:player as *const _ as usize,owner:unsafe {player.player_game_data.as_ref()}.character_id,
            origin,fp:player.chr_ins.modules.data.fp,started:Instant::now(),sent:false,stable:0,reported_lookup:false});
        FINISHED.store(0,Ordering::Release);
    }
    Ok(())
}
fn recovered(origin:Position,current:Position,alive:bool,sent:bool,target:usize,key:u32)->bool {
    sent&&alive&&target==usize::MAX&&key==0&&origin.block==current.block
        &&origin.xyz.iter().zip(current.xyz).map(|(a,b)|(a-b).powi(2)).sum::<f32>()<=4.0
}
pub fn tick(world:&mut WorldChrMan,game:&GameMan) {
    let Ok(mut queue)=PENDING.try_lock() else{return;};
    let Some(pending)=queue.as_mut() else{return;};
    if pending.started.elapsed()>Duration::from_secs(30) {
        super::log("PARTY_REVIVE timeout: native spectator exit not confirmed; no reward advancement");*queue=None;return;
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
    let Some(spectator)=locate(&Local,BASE.load(Ordering::Acquire),world_address) else{
        if !pending.reported_lookup {
            pending.reported_lookup=true;
            super::log(format!("PARTY_REVIVE waiting for validated object: root={:?} world={world_address:#x}",Local.pointer(BASE.load(Ordering::Acquire)+ROOT)));
        }
        return;
    };
    let alive=player.chr_ins.modules.data.hp>0&&!player.chr_ins.chr_flags1c5.death_flag();
    if !pending.sent {
        if alive||spectator.target==usize::MAX{return;}
        let base=BASE.load(Ordering::Acquire);
        if Local.pointer(base+ROOT)!=Some(spectator.root){return;}
        let request:Request=unsafe {std::mem::transmute(base+0xd7ec0)};
        // The native mutex and atomic transition handle recovery; false keeps the request local.
        unsafe {request(spectator.address as *mut u8,true,false)};
        pending.sent=true;
        super::log(format!("PARTY_REVIVE native request submitted without detour target={:#x}",spectator.target));
        return;
    }
    let p=&player.chr_ins.modules.physics.position;
    let current=Position{block:i32::from(player.current_block_id),xyz:[p.0,p.1,p.2]};
    if !recovered(pending.origin,current,alive,pending.sent,spectator.target,spectator.key){pending.stable=0;return;}
    player.chr_ins.modules.data.hp=1;
    player.chr_ins.modules.data.fp=pending.fp.min(player.chr_ins.modules.data.max_fp);
    pending.stable+=1;
    if pending.stable>=2 {
        FINISHED.store(pending.player,Ordering::Release);
        super::log("PARTY_REVIVE native exit confirmed at death position; HP=1, FP preserved");
        *queue=None;
    }
}
pub unsafe fn install()->Result<(),String> {
    if std::env::var("ERDTREE_TOWER_DISABLE_REVIVAL").as_deref()==Ok("1"){return Err("Recovery disabled by diagnostic switch".into());}
    let name:Vec<_>="ersc.dll\0".encode_utf16().collect();
    let base=unsafe {GetModuleHandleW(name.as_ptr())} as usize;
    if base==0{return Err("Seamless not loaded".into());}
    let pe=Local.word(base+0x3c).filter(|p|*p<=0x1000).ok_or("Invalid Seamless PE header")? as usize;
    if Local.word(base+pe+8)!=Some(0x6a98a913)||Local.word(base+pe+24+56)!=Some(0xd31000){return Err("Unsupported Seamless build".into());}
    for (rva,len,expected) in [(0xd7bb0,0x303,0x3b87e0ebc95fb1c3u64),(0xd7ec0,0x108,0xe8e01219eedfa0cf),(0xd81f0,0x123,0x6772508514e31e45)] {
        let mut hash=14695981039346656037u64;
        for i in 0..len {let b=Local.bytes::<1>(base+rva+i).ok_or("Unreadable recovery function")?[0];hash=(hash^u64::from(b)).wrapping_mul(1099511628211);}
        if hash!=expected{return Err(format!("Recovery signature mismatch at {rva:x}"));}
    }
    if Local.bytes::<14>(base+0x32c09)!=Some([0x48,0x8d,0x85,0xe0,0x2f,0,0,0x48,0x89,0x05,0x19,0xf4,0x1e,0]){return Err("Recovery root locator mismatch".into());}
    BASE.store(base,Ordering::Release);
    super::log("PARTY_REVIVE native request interface ready; no spectator code hooks installed");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
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
    fn fixture()->Fake {
        let mut m=Fake::default();let (base,object,head,world)=(0x180000000,0x30000,0x40000,0x50000);
        m.put_pointer(base+ROOT,object+0xad0);m.put_pointer(object+8,object-0x1848);m.put_pointer(object-0x1848,0x60000);m.put_pointer(0x60000,world);
        m.put_pointer(object+0x20,object-0xa88);m.put_pointer(object+0x30,object+0x480);m.put_pointer(object+0x38,object-0xe0);
        m.put_word(object+0xb0,0);m.put_word(object+0xac,0);m.put_pointer(object+0x58,usize::MAX);m.put_pointer(object+0xc0,head);m.put_pointer(object+0xc8,3);
        m.put(head+0x19,&[1]);m.put_pointer(head+8,0x41000);
        for (key,callback) in [0xd7fd0,0xd8180,0xd81f0].into_iter().enumerate() {
            let node=0x41000+key*0x100;
            m.put(node+0x19,&[0]);m.put_word(node+0x20,key as u32);m.put_pointer(node+0x30,base+callback);
            m.put_pointer(node,head);m.put_pointer(node+0x10,if key==2 {head}else{node+0x100});
        }
        m
    }
    #[test]fn locator_requires_matching_world_and_all_native_callbacks() {
        let m=fixture();assert_eq!(locate(&m,0x180000000,0x50000).unwrap().address,0x30000);
        assert!(locate(&m,0x180000000,0x50001).is_none());
        let mut m=fixture();m.put_pointer(0x41100+0x30,0x1800d81f0);assert!(locate(&m,0x180000000,0x50000).is_none());
    }
    #[test]fn corrupt_or_expired_locator_is_rejected_before_request() {
        for (at,value) in [(0x180000000+ROOT,0),(0x180000000+ROOT,usize::MAX),(0x30000+8,0x29999),(0x30000+0xc8,4),(0x30000+0xc0,usize::MAX),(0x41100+0x10,0x41000)] {
            let mut m=fixture();m.put_pointer(at,value);assert!(locate(&m,0x180000000,0x50000).is_none());
        }
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
