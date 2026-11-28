//! 闪退保底（HansFix r18）：游戏本体里两处已知会因为「对象已经半销毁」而读空指针的位置，
//! 出错时让游戏跳过这一小步继续运行，而不是整个闪退。
//! 只拦截这两个精确地址、而且只在寄存器确实是空指针时才拦；其它任何异常照常交给游戏处理。
//! 启动时先核对这两处的机器码，游戏版本不同（字节对不上）就自动不启用。
//!
//! A. eldenring.exe+0x11A8200：引用计数对象释放回对象池。退出联机 / 回圆桌卸载角色时，
//!    对象所属的池已经先被销毁（池指针为空），读 [池+0x88] 闪退。
//!    处理：跳过「放回对象池」这一步直接返回（只少回收一个小对象，不影响游戏）。
//! B. eldenring.exe+0x4B93BD：游戏每帧遍历一组角色，读角色的模块容器 [角色+0x190]。
//!    联机结算时队友的骨灰正在销毁，模块容器已经是空的，读 [空+0x28] 闪退
//!    （和 r16 我们自己代码里的骨灰闪退是同一个根源，这次发生在游戏本体里）。
//!    处理：跳过这个角色，继续处理链表里的下一个。
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32,AtomicUsize,Ordering};

unsafe extern "system" {
    fn AddVectoredExceptionHandler(first:u32,handler:unsafe extern "system" fn(*mut c_void)->i32)->*mut c_void;
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
}

struct Site {name:&'static str,fault:usize,check:&'static [(usize,&'static [u8])],register:usize,resume:usize}
// CONTEXT (x64) 偏移：Rax 0x78、Rcx 0x80、Rip 0xF8
const RAX:usize=0x78;const RCX:usize=0x80;const RIP:usize=0xF8;
const SITES:[Site;2]=[
    Site{name:"pool_release",fault:0x11A8269,register:RCX,resume:0x11A827A,check:&[
        (0x11A8200,&[0x40,0x53,0x48,0x83,0xec,0x20,0xb8,0xff,0xff,0x00,0x00,0x48,0x8b,0xd9,0x66,0x01,0x41,0x08,0x75,0x66,0x80,0x79,0x0a,0x00]),
        (0x11A8266,&[0x48,0x8b,0x09,0x48,0x8b,0x81,0x88,0x00,0x00,0x00,0x48,0x89,0x03,0x48,0x89,0x99,0x88,0x00,0x00,0x00,0x48,0x83,0xc4,0x20,0x5b,0xc3])]},
    Site{name:"chr_modules_list",fault:0x4B93E2,register:RAX,resume:0x4B9435,check:&[
        (0x4B93D0,&[0x48,0x8b,0x6f,0x10]),
        (0x4B93DB,&[0x48,0x8b,0x85,0x90,0x01,0x00,0x00,0x48,0x8b,0x48,0x28]),
        (0x4B9435,&[0x48,0x8b,0x3f,0x48,0x3b,0xfe,0x75,0x93])]},
];

static EXE:AtomicUsize=AtomicUsize::new(0);
static ENABLED:AtomicU32=AtomicU32::new(0);
static HITS:[AtomicU32;2]=[AtomicU32::new(0),AtomicU32::new(0)];

pub fn install() {
    let exe=unsafe {GetModuleHandleW(std::ptr::null())} as usize;
    if exe==0 {return;}
    let mut mask=0u32;
    let mut report=Vec::new();
    for (i,site) in SITES.iter().enumerate() {
        let ok=site.check.iter().all(|(rva,bytes)|unsafe {std::slice::from_raw_parts((exe+rva) as *const u8,bytes.len())}==*bytes);
        if ok {mask|=1<<i;}
        report.push(format!("{}={}",site.name,if ok{"on"}else{"off(version mismatch)"}));
    }
    EXE.store(exe,Ordering::Relaxed);
    ENABLED.store(mask,Ordering::Relaxed);
    // 在闪退追踪之后注册、同样放在最前面，于是比追踪先执行：拦下的异常不会被记成闪退
    if mask!=0 {unsafe {AddVectoredExceptionHandler(1,handler);}}
    let text=format!("CRASH_GUARD installed {}",report.join(" "));
    crate::crash_trace::write(&format!("{text}\n"));
    crate::log(text);
}

unsafe extern "system" fn handler(pointers:*mut c_void)->i32 {
    const CONTINUE_SEARCH:i32=0;const CONTINUE_EXECUTION:i32=-1;
    if pointers.is_null() {return CONTINUE_SEARCH;}
    let (record,context)=unsafe {(*(pointers as *const *const u8),*((pointers as *const *mut u8).add(1)))};
    if record.is_null()||context.is_null() {return CONTINUE_SEARCH;}
    if unsafe {(record as *const u32).read_unaligned()}!=0xC0000005 {return CONTINUE_SEARCH;}
    let exe=EXE.load(Ordering::Relaxed);let mask=ENABLED.load(Ordering::Relaxed);
    let rip=unsafe {(context.add(RIP) as *const usize).read_unaligned()};
    for (i,site) in SITES.iter().enumerate() {
        if mask&(1<<i)==0||rip!=exe+site.fault {continue;}
        // 只有寄存器真的是空（或极小的无效地址）才拦，其它情况照常闪退、留给追踪记录
        let value=unsafe {(context.add(site.register) as *const usize).read_unaligned()};
        if value>=0x10000 {return CONTINUE_SEARCH;}
        unsafe {(context.add(RIP) as *mut usize).write_unaligned(exe+site.resume);}
        let n=HITS[i].fetch_add(1,Ordering::Relaxed)+1;
        if n<=5||n.is_power_of_two() {crate::crash_trace::write(&format!("CRASH_GUARD skipped {} hit={n} (would have crashed)\n",site.name));}
        return CONTINUE_EXECUTION;
    }
    CONTINUE_SEARCH
}

