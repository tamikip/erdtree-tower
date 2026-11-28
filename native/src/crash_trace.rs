//! 闪退追踪（HansFix 诊断）：游戏闪退时把原因写进日志旁的 crash-trace.txt。
//! - Rust panic：记下消息和代码位置（在钩子回调里 panic 会直接让游戏退出，没有任何提示）。
//! - 访问违例等硬件异常：记下出错地址属于哪个模块、偏移多少，以及栈上属于本 DLL 的返回地址
//!   （配合编译出来的 PDB 就能还原成函数名）。
//! 只记录，不处理：异常照常交给游戏自己的处理流程。
use std::ffi::c_void;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32,AtomicUsize,Ordering};

unsafe extern "system" {
    fn AddVectoredExceptionHandler(first:u32,handler:unsafe extern "system" fn(*mut c_void)->i32)->*mut c_void;
    fn GetModuleHandleExW(flags:u32,address:*const c_void,module:*mut *mut c_void)->i32;
    fn GetModuleFileNameW(module:*mut c_void,name:*mut u16,size:u32)->u32;
    fn GetCurrentThreadId()->u32;
}

static PATH:OnceLock<PathBuf>=OnceLock::new();
static OUR_BASE:AtomicUsize=AtomicUsize::new(0);
static OUR_SIZE:AtomicUsize=AtomicUsize::new(0);
static LOGGED:AtomicU32=AtomicU32::new(0);
/// 最多记这么多次异常（游戏偶尔会自己触发并处理掉一些异常）
const MAX_EXCEPTIONS:u32=12;

pub fn install(module_base:usize,log_dir:PathBuf) {
    if PATH.set(log_dir.join("crash-trace.txt")).is_err() {return;}
    OUR_BASE.store(module_base,Ordering::Relaxed);
    // PE 头里的 SizeOfImage
    let size=unsafe {
        let nt=module_base+(((module_base+0x3c) as *const u32).read_unaligned() as usize);
        ((nt+0x50) as *const u32).read_unaligned() as usize
    };
    OUR_SIZE.store(size,Ordering::Relaxed);
    let previous=std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info|{
        let thread=std::thread::current();
        write(&format!("PANIC thread={:?} tid={} {info}\n{}\n",thread.name(),unsafe {GetCurrentThreadId()},std::backtrace::Backtrace::force_capture()));
        previous(info);
    }));
    unsafe {AddVectoredExceptionHandler(1,handler);}
    write(&format!("CRASH_TRACE installed module_base={module_base:#x} size={size:#x}\n"));
}

pub(crate) fn write(text:&str) {
    let Some(path)=PATH.get() else{return};
    if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|d|d.as_millis());
        let _=write!(file,"[unix_ms={now}] {text}");
        let _=file.flush();
    }
}

/// 地址属于哪个模块：（模块文件名, 偏移）
fn module_of(address:usize)->Option<(String,usize)> {
    let mut module=std::ptr::null_mut();
    // FROM_ADDRESS | UNCHANGED_REFCOUNT
    if unsafe {GetModuleHandleExW(0x4|0x2,address as *const c_void,&mut module)}==0||module.is_null() {return None;}
    let mut name=[0u16;260];
    let len=unsafe {GetModuleFileNameW(module,name.as_mut_ptr(),name.len() as u32)} as usize;
    let full=String::from_utf16_lossy(&name[..len.min(name.len())]);
    let short=full.rsplit(['\\','/']).next().unwrap_or(&full).to_string();
    Some((short,address-module as usize))
}

fn stack_base()->usize {
    let base:usize;
    unsafe {std::arch::asm!("mov {}, gs:[0x08]",out(reg) base,options(nostack,readonly,preserves_flags));}
    base
}

unsafe extern "system" fn handler(pointers:*mut c_void)->i32 {
    const CONTINUE_SEARCH:i32=0;
    if pointers.is_null() {return CONTINUE_SEARCH;}
    let (record,context)=unsafe {(*(pointers as *const *const u8),*((pointers as *const *const u8).add(1)))};
    if record.is_null()||context.is_null() {return CONTINUE_SEARCH;}
    let code=unsafe {(record as *const u32).read_unaligned()};
    // 只关心真正会导致闪退的：访问违例、非法指令、除零、栈溢出、堆损坏、快速失败
    if !matches!(code,0xC0000005|0xC000001D|0xC0000094|0xC0000374|0xC0000409|0xC0000096) {return CONTINUE_SEARCH;}
    if LOGGED.fetch_add(1,Ordering::Relaxed)>=MAX_EXCEPTIONS {return CONTINUE_SEARCH;}
    let address=unsafe {(record.add(0x10) as *const usize).read_unaligned()};
    let params=unsafe {(record.add(0x18) as *const u32).read_unaligned()};
    let info:[usize;2]=unsafe {[(record.add(0x20) as *const usize).read_unaligned(),(record.add(0x28) as *const usize).read_unaligned()]};
    let rip=unsafe {(context.add(0xF8) as *const usize).read_unaligned()};
    let rsp=unsafe {(context.add(0x98) as *const usize).read_unaligned()};
    let mut text=format!("EXCEPTION code={code:#x} tid={} address={address:#x} module={:?}",unsafe {GetCurrentThreadId()},module_of(address));
    if code==0xC0000005&&params>=2 {text+=&format!(" access={} target={:#x}",match info[0]{0=>"read",1=>"write",8=>"execute",_=>"?"},info[1]);}
    text+=&format!(" rip={rip:#x}\n");
    // 栈上属于本 DLL 的返回地址（从近到远），以及前几个属于其它模块的
    let (base,size)=(OUR_BASE.load(Ordering::Relaxed),OUR_SIZE.load(Ordering::Relaxed));
    let top=stack_base().min(rsp.saturating_add(0x4000));
    let mut ours=Vec::new();let mut others=Vec::new();
    let mut at=rsp&!7;
    while at+8<=top&&ours.len()<24 {
        let value=unsafe {(at as *const usize).read_unaligned()};
        if value>=base&&value<base+size {ours.push(value-base);}
        else if others.len()<10&&value>0x10000&&value<0x7FFF_FFFF_FFFF&&let Some(m)=module_of(value)&&m.0.to_ascii_lowercase().ends_with(".exe") {others.push(m.1);}
        at+=8;
    }
    text+=&format!("  our_dll_offsets={:x?}\n  exe_offsets={:x?}\n",ours,others);
    write(&text);
    CONTINUE_SEARCH
}
