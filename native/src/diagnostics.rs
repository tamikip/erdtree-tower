use std::{io::Write,path::{Path,PathBuf},sync::{Mutex,LazyLock,atomic::{AtomicU64,Ordering}},time::{Duration,Instant}};

static START:LazyLock<Instant>=LazyLock::new(Instant::now);
static SERIAL:Mutex<u64>=Mutex::new(0);
static FRAME:AtomicU64=AtomicU64::new(0);
static TASK:AtomicU64=AtomicU64::new(0);
const LIMIT:u64=8*1024*1024;

#[repr(C)]
#[derive(Default)]
struct SystemTime {year:u16,month:u16,weekday:u16,day:u16,hour:u16,minute:u16,second:u16,millis:u16}
#[link(name="kernel32")]
unsafe extern "system" {
    fn GetSystemTime(time:*mut SystemTime);
    fn GetModuleFileNameW(module:*mut std::ffi::c_void,path:*mut u16,size:u32)->u32;
    fn GetModuleHandleW(name:*const u16)->*mut std::ffi::c_void;
}
fn backup(path:&Path,index:u8)->PathBuf {path.with_file_name(format!("tower-preview.{index}.log"))}
fn rotate(path:&Path,limit:u64)->std::io::Result<()> {
    if !path.exists()||path.metadata()?.len()<limit{return Ok(());}
    let oldest=backup(path,3);if oldest.exists(){std::fs::remove_file(oldest)?;}
    for n in (1..3).rev(){let old=backup(path,n);if old.exists(){std::fs::rename(old,backup(path,n+1))?;}}
    std::fs::rename(path,backup(path,1))
}
pub fn write(message:impl std::fmt::Display) {
    let Ok(mut serial)=SERIAL.lock() else{return;};
    let path=&*super::LOG_PATH;
    if let Some(parent)=path.parent(){let _=std::fs::create_dir_all(parent);}
    if rotate(path,LIMIT).is_err(){return;}
    let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(path) else{return;};
    *serial+=1;
    let mut time=SystemTime::default();unsafe {GetSystemTime(&mut time);}
    let text=message.to_string().replace('\r',"\\r").replace('\n',"\\n");
    let _=writeln!(file,"{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z pid={} seq={} +{}ms {}",
        time.year,time.month,time.day,time.hour,time.minute,time.second,time.millis,std::process::id(),*serial,START.elapsed().as_millis(),text);
}
fn module_path(address:usize)->Option<PathBuf> {
    let mut buffer=vec![0u16;32768];
    let length=unsafe {GetModuleFileNameW(address as _,buffer.as_mut_ptr(),buffer.len() as u32)} as usize;
    (length>0&&length<buffer.len()).then(||PathBuf::from(String::from_utf16_lossy(&buffer[..length])))
}
pub fn startup(address:usize) {
    super::log(format!("SESSION_START build={} protocol={} dll={:?} exe={:?} log={:?}",env!("TOWER_BUILD_ID"),erdtree_tower::party_handshake::VERSION,module_path(address),module_path(0),*super::LOG_PATH));
    for name in ["ersc.dll","dxgi.dll","d3d12.dll","dinput8.dll","ReShade64.dll","nvngx_dlssg.dll"] {
        let wide:Vec<_>=name.encode_utf16().chain(Some(0)).collect();
        let handle=unsafe {GetModuleHandleW(wide.as_ptr())};
        if !handle.is_null(){super::log(format!("MODULE name={name} path={:?}",module_path(handle as usize)));}
    }
    super::log(format!("LAUNCH preview={} seamless={} probe={}",std::env::var("ERDTREE_TOWER_PREVIEW").as_deref()==Ok("1"),std::env::var("ERDTREE_TOWER_SEAMLESS_ENTRY").as_deref()==Ok("1"),std::env::var("ERDTREE_TOWER_SEAMLESS_PROBE").as_deref()==Ok("1")));
}
pub fn frame(){FRAME.store(START.elapsed().as_millis() as u64+1,Ordering::Relaxed);}
pub fn task(){TASK.store(START.elapsed().as_millis() as u64+1,Ordering::Relaxed);}
fn stalled(now:u64,last:u64)->bool {now>30_000&&(last==0||now.saturating_sub(last)>30_000)}
pub fn watchdog() {
    let _=std::thread::Builder::new().name("tower-diagnostics".into()).spawn(||loop {
        std::thread::sleep(Duration::from_secs(30));
        let now=START.elapsed().as_millis() as u64;
        let frame=FRAME.load(Ordering::Relaxed);let task=TASK.load(Ordering::Relaxed);
        super::log(format!("HEALTH uptime_ms={now} render_last_ms={frame} task_last_ms={task} render_stalled={} task_stalled={}",stalled(now,frame),stalled(now,task)));
    });
}
#[derive(Default)]
pub struct Changes {last:String,at:Option<Instant>}
impl Changes {
    pub fn due(&mut self,key:String,now:Instant)->bool {
        if key==self.last&&self.at.is_some_and(|t|now.duration_since(t)<Duration::from_secs(30)){return false;}
        self.last=key;self.at=Some(now);true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_changes_log_immediately_and_steady_state_has_heartbeat() {
        let now=Instant::now();let mut state=Changes::default();
        assert!(state.due("alive".into(),now));assert!(!state.due("alive".into(),now+Duration::from_secs(1)));
        assert!(state.due("dead".into(),now+Duration::from_secs(2)));
        assert!(!state.due("dead".into(),now+Duration::from_secs(31)));
        assert!(state.due("dead".into(),now+Duration::from_secs(32)));
        assert!(stalled(31_000,0));assert!(!stalled(31_000,30_999));assert!(stalled(70_000,30_999));
    }
    #[test]
    fn rotation_keeps_three_backups_in_order() {
        let dir=std::env::temp_dir().join(format!("tower-log-test-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&dir).unwrap();let path=dir.join("tower-preview.log");
        for n in 0..5 {std::fs::write(&path,n.to_string()).unwrap();rotate(&path,1).unwrap();}
        for (i,value) in [(1,"4"),(2,"3"),(3,"2")] {assert_eq!(std::fs::read_to_string(backup(&path,i)).unwrap(),value);std::fs::remove_file(backup(&path,i)).unwrap();}
        std::fs::remove_dir(dir).unwrap();
    }
}
