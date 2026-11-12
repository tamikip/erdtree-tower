use std::ffi::c_void;

#[derive(Clone,Copy)]
struct Module {base:usize,timestamp:u32,size:u32}
#[derive(Clone,Copy)]
struct Hook {rva:usize,dispatcher:usize,length:usize,hash:u64}
const HOOKS:[Hook;2]=[
    Hook{rva:0x4b73d0,dispatcher:0x96ed0,length:0x63,hash:0x6407c0604523c26a},
    Hook{rva:0x4b8690,dispatcher:0x96480,length:0x65,hash:0x2a0354428ec85f58},
];
const HOOKS_203:[Hook;2]=[
    Hook{rva:0x4b73d0,dispatcher:0x983b0,length:0x63,hash:0x829e43a0c10f9dfd},
    Hook{rva:0x4b8690,dispatcher:0x97960,length:0x65,hash:0xeb454f899352a64a},
];
fn hooks(module:Module)->Option<(&'static str,&'static [Hook;2])> {
    match (module.timestamp,module.size) {
        (0x6ae0ad1a,0xd65000)=>Some(("2.0.2",&HOOKS)),
        (0x6af4b20a,0xd67000)=>Some(("2.0.3",&HOOKS_203)),
        _=>None,
    }
}
trait Memory {fn read(&self,address:usize,length:usize)->Option<Vec<u8>>;}
#[derive(Debug,PartialEq)]
enum Entry {Native,Seamless}
fn hash(bytes:&[u8])->u64 {bytes.iter().fold(0xcbf29ce484222325,|h,b|(h^u64::from(*b)).wrapping_mul(0x100000001b3))}
fn pointer(bytes:&[u8])->Option<usize>{usize::try_from(u64::from_le_bytes(bytes.try_into().ok()?)).ok()}
fn classify(memory:&impl Memory,address:usize,expected:&[u8],module:Option<Module>,hook:Hook)->Option<Entry> {
    if expected.len()<5{return None;}
    let entry=memory.read(address,expected.len())?;
    if entry==expected{return Some(Entry::Native);}
    let module=module?;
    let (_,known)=hooks(module)?;
    if !known.iter().any(|h|h.rva==hook.rva&&h.dispatcher==hook.dispatcher)||entry[0]!=0xe9||entry[5..]!=expected[5..]{return None;}
    let target=module.base.checked_add(hook.dispatcher)?;
    if hook.dispatcher.checked_add(hook.length)? > module.size as usize||hash(&memory.read(target,hook.length)?)!=hook.hash{return None;}
    let delta=i32::from_le_bytes(entry[1..5].try_into().ok()?) as isize;
    let mut next=address.checked_add(5)?.checked_add_signed(delta)?;
    let mut visited=Vec::new();
    for _ in 0..3 {
        if next<0x10000||visited.contains(&next){return None;}
        visited.push(next);
        let bytes=memory.read(next,28)?;
        if bytes[..2]==[0x48,0xb8]&&bytes[10..17]==[0x66,0x48,0x0f,0x6e,0xe8,0x49,0xbb]&&bytes[25..28]==[0x41,0xff,0xe3] {
            let context=pointer(&bytes[2..10])?;
            if pointer(&bytes[17..25])?!=target||!(0x10000..=0x7fffffff0000).contains(&context)||context%8!=0{return None;}
            memory.read(context,8)?;
            return Some(Entry::Seamless);
        }
        next=if bytes[..6]==[0xff,0x25,0,0,0,0] {pointer(&bytes[6..14])?}
            else if bytes[..2]==[0x48,0xb8]&&bytes[10..12]==[0xff,0xe0] {pointer(&bytes[2..10])?}
            else {return None;};
    }
    None
}

#[link(name="kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn ReadProcessMemory(process:*mut c_void,address:*const c_void,buffer:*mut c_void,size:usize,read:*mut usize)->i32;
}
struct Local;
impl Memory for Local {
    fn read(&self,address:usize,length:usize)->Option<Vec<u8>> {
        if address<0x10000||length==0||address.checked_add(length).is_none(){return None;}
        let mut bytes=vec![0;length];let mut read=0;
        let ok=unsafe {ReadProcessMemory(-1isize as *mut c_void,address as *const c_void,bytes.as_mut_ptr().cast(),length,&mut read)};
        (ok!=0&&read==length).then_some(bytes)
    }
}
fn module(memory:&impl Memory,base:usize)->Option<Module> {
    let dos=memory.read(base,64)?;
    if dos[..2]!=[b'M',b'Z']{return None;}
    let offset=u32::from_le_bytes(dos[0x3c..0x40].try_into().ok()?) as usize;
    if offset>0x100000{return None;}
    let nt=memory.read(base.checked_add(offset)?,0x58)?;
    if nt[..4]!=[b'P',b'E',0,0]{return None;}
    Some(Module{base,timestamp:u32::from_le_bytes(nt[8..12].try_into().ok()?),size:u32::from_le_bytes(nt[0x50..0x54].try_into().ok()?)})
}
// Only the pinned version's two verified detours are eligible. MinHook chains to
// the existing jump through its trampoline, retaining Seamless's callback.
pub(super) unsafe fn checked_function(rva:usize,expected:&[u8])->Result<usize,String> {
    let base=unsafe {GetModuleHandleW(std::ptr::null())} as usize;
    let address=base.checked_add(rva).ok_or("Spirit hook address overflow")?;
    let name:Vec<u16>="ersc.dll\0".encode_utf16().collect();
    let ersc=unsafe {GetModuleHandleW(name.as_ptr())} as usize;
    let module=module(&Local,ersc);
    let (version,selected)=module.and_then(hooks).unwrap_or(("unknown",&HOOKS));
    let hook=*selected.iter().find(|h|h.rva==rva).ok_or_else(||format!("Unsupported spirit hook {rva:x}"))?;
    match classify(&Local,address,expected,module,hook) {
        Some(Entry::Native)=>Ok(address),
        Some(Entry::Seamless)=>{super::super::log(format!("Spirit hook compatibility: retaining Seamless {version} chain at {rva:x} -> ersc+{:x}",hook.dispatcher));Ok(address)},
        None=>Err(format!("Spirit hook {rva:x}: unrecognized entry or Seamless build; original chain left intact (bytes={:02x?})",Local.read(address,15).unwrap_or_default())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    const GAME:usize=0x140000000;
    const RELAY:usize=GAME-0x1000;
    const STUB:usize=GAME-0x2000;
    const CONTEXT:usize=0x200000000;
    const EXPECTED:[u8;15]=[0x48,0x89,0x5c,0x24,0x08,0x48,0x89,0x74,0x24,0x10,0x57,0x48,0x83,0xec,0x20];
    #[derive(Clone,Default)]struct Fake(BTreeMap<usize,u8>);
    impl Fake {fn put(&mut self,address:usize,bytes:&[u8]){for(i,b)in bytes.iter().enumerate(){self.0.insert(address+i,*b);}}}
    impl Memory for Fake {fn read(&self,address:usize,length:usize)->Option<Vec<u8>>{(0..length).map(|i|self.0.get(&(address+i)).copied()).collect()}}
    fn fixture(indirect:bool)->(Fake,Module,Hook) {
        let module=Module{base:0x180000000,timestamp:0x6ae0ad1a,size:0xd65000};
        let mut hook=HOOKS[usize::from(indirect)];hook.length=4;hook.hash=hash(&[0xf3,0x0f,0x1e,0xfa]);
        let mut m=Fake::default();m.put(GAME,&EXPECTED);
        m.put(GAME,&[0xe9]);m.put(GAME+1,&((RELAY as i64-GAME as i64-5) as i32).to_le_bytes());
        let mut relay=[0xcc;28];
        if indirect {relay[..6].copy_from_slice(&[0xff,0x25,0,0,0,0]);relay[6..14].copy_from_slice(&(STUB as u64).to_le_bytes());}
        else {relay[..2].copy_from_slice(&[0x48,0xb8]);relay[2..10].copy_from_slice(&(STUB as u64).to_le_bytes());relay[10..12].copy_from_slice(&[0xff,0xe0]);}
        m.put(RELAY,&relay);
        let mut stub=[0;28];stub[..2].copy_from_slice(&[0x48,0xb8]);stub[2..10].copy_from_slice(&(CONTEXT as u64).to_le_bytes());
        stub[10..17].copy_from_slice(&[0x66,0x48,0x0f,0x6e,0xe8,0x49,0xbb]);stub[17..25].copy_from_slice(&((module.base+hook.dispatcher) as u64).to_le_bytes());stub[25..].copy_from_slice(&[0x41,0xff,0xe3]);
        m.put(STUB,&stub);m.put(CONTEXT,&[0;8]);m.put(module.base+hook.dispatcher,&[0xf3,0x0f,0x1e,0xfa]);(m,module,hook)
    }
    #[test]fn vanilla_stays_strict_and_needs_no_seamless(){let mut m=Fake::default();m.put(GAME,&EXPECTED);assert_eq!(classify(&m,GAME,&EXPECTED,None,HOOKS[0]),Some(Entry::Native));m.put(GAME+8,&[0xcc]);assert_eq!(classify(&m,GAME,&EXPECTED,None,HOOKS[0]),None);}
    #[test]fn both_observed_seamless_jump_shapes_are_accepted(){for indirect in [false,true]{let(m,module,hook)=fixture(indirect);assert_eq!(classify(&m,GAME,&EXPECTED,Some(module),hook),Some(Entry::Seamless));}}
    #[test]fn new_build_uses_new_dispatchers_and_rejects_old_chain(){
        for indirect in [false,true] {
            let(mut m,mut module,old)=fixture(indirect);
            module.timestamp=0x6af4b20a;module.size=0xd67000;
            assert_eq!(classify(&m,GAME,&EXPECTED,Some(module),old),None);
            let mut hook=HOOKS_203[usize::from(indirect)];hook.length=old.length;hook.hash=old.hash;
            m.put(STUB+17,&((module.base+hook.dispatcher) as u64).to_le_bytes());
            m.put(module.base+hook.dispatcher,&[0xf3,0x0f,0x1e,0xfa]);
            assert_eq!(classify(&m,GAME,&EXPECTED,Some(module),hook),Some(Entry::Seamless));
            m.put(module.base+hook.dispatcher,&[0xcc]);
            assert_eq!(classify(&m,GAME,&EXPECTED,Some(module),hook),None);
        }
    }
    #[test]fn unknown_build_or_modified_dispatcher_is_rejected(){let(m,mut module,hook)=fixture(false);module.timestamp+=1;assert_eq!(classify(&m,GAME,&EXPECTED,Some(module),hook),None);module.timestamp-=1;let mut changed=m.clone();changed.put(module.base+hook.dispatcher,&[0xcc]);assert_eq!(classify(&changed,GAME,&EXPECTED,Some(module),hook),None);module.size+=1;assert_eq!(classify(&m,GAME,&EXPECTED,Some(module),hook),None);}
    #[test]fn altered_game_tail_context_and_dispatch_target_are_rejected(){for address in [GAME+6,STUB+10,STUB+17]{let(mut m,module,hook)=fixture(false);m.put(address,&[0xcc]);assert_eq!(classify(&m,GAME,&EXPECTED,Some(module),hook),None);}let(mut m,module,hook)=fixture(false);m.0.remove(&CONTEXT);assert_eq!(classify(&m,GAME,&EXPECTED,Some(module),hook),None);}
    #[test]fn unreadable_or_cyclic_relays_fail_closed(){let(mut m,module,hook)=fixture(false);m.put(RELAY+2,&(RELAY as u64).to_le_bytes());assert_eq!(classify(&m,GAME,&EXPECTED,Some(module),hook),None);m.0.remove(&RELAY);assert_eq!(classify(&m,GAME,&EXPECTED,Some(module),hook),None);}

    #[test]
    fn minhook_chains_an_existing_jump_in_an_isolated_process() {
        let status=std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","spirits::seamless_hook::tests::native_chain_child","--ignored","--nocapture"]).status().unwrap();
        assert!(status.success(),"MinHook must preserve the foreign detour");
    }
    static ORIGINAL:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
    unsafe extern "C" fn outer(value:i32)->i32 {
        let original:unsafe extern "C" fn(i32)->i32=unsafe {std::mem::transmute(ORIGINAL.load(std::sync::atomic::Ordering::Acquire))};
        unsafe {original(value)+3}
    }
    #[test]
    #[ignore="run by the parent test in a private process"]
    fn native_chain_child() {
        unsafe extern "system" {
            fn VirtualAlloc(address:*mut c_void,size:usize,allocation:u32,protection:u32)->*mut c_void;
            fn VirtualFree(address:*mut c_void,size:usize,kind:u32)->i32;
        }
        unsafe {
            let page=VirtualAlloc(std::ptr::null_mut(),4096,0x3000,0x40).cast::<u8>();assert!(!page.is_null());
            std::ptr::write_bytes(page,0xcc,4096);
            std::ptr::copy_nonoverlapping([0xe9,59,0,0,0].as_ptr(),page,5);
            // A pre-existing foreign detour returns value + 7, not the bytes after the entry jump.
            std::ptr::copy_nonoverlapping([0x8d,0x41,7,0xc3].as_ptr(),page.add(64),4);
            let call:unsafe extern "C" fn(i32)->i32=std::mem::transmute(page);
            assert_eq!(call(10),17);
            hudhook::mh::MH_Initialize().ok().unwrap();
            let hook=hudhook::mh::MhHook::new(page.cast(),outer as *mut c_void).unwrap();
            ORIGINAL.store(hook.trampoline() as usize,std::sync::atomic::Ordering::Release);
            hudhook::mh::MH_EnableHook(page.cast()).ok().unwrap();
            assert_eq!(call(10),20);assert_eq!(call(-4),6);
            hudhook::mh::MH_DisableHook(page.cast()).ok().unwrap();assert_eq!(call(10),17);
            hudhook::mh::MH_Uninitialize().ok().unwrap();assert_ne!(VirtualFree(page.cast(),0,0x8000),0);
        }
    }
}
