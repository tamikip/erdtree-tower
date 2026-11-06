use std::{io::Write,path::{Path,PathBuf},sync::{Mutex,LazyLock,atomic::{AtomicU64,Ordering}},time::{Duration,Instant}};

static START:LazyLock<Instant>=LazyLock::new(Instant::now);
static SERIAL:Mutex<LogState>=Mutex::new(LogState{sequence:0,date:None});
static VERBOSE:LazyLock<bool>=LazyLock::new(||std::env::var("ERDTREE_TOWER_LOG_VERBOSE").as_deref()==Ok("1"));
static FRAME:AtomicU64=AtomicU64::new(0);
static TASK:AtomicU64=AtomicU64::new(0);
const LIMIT:u64=8*1024*1024;

#[repr(C)]
#[derive(Default)]
struct SystemTime {year:u16,month:u16,weekday:u16,day:u16,hour:u16,minute:u16,second:u16,millis:u16}
#[derive(Default)]
struct LogState {sequence:u64,date:Option<(u16,u16,u16)>}
impl LogState {
    fn record(&mut self,out:&mut impl Write,time:&SystemTime,uptime:u128,message:&str,empty:bool,verbose:bool)->std::io::Result<()> {
        self.sequence+=1;
        let date=(time.year,time.month,time.day);
        let text=message.replace('\r',"\\r").replace('\n',"\\n");
        if verbose {
            writeln!(out,"{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z pid={} seq={} +{}ms {}",
                time.year,time.month,time.day,time.hour,time.minute,time.second,time.millis,std::process::id(),self.sequence,uptime,text)?;
        } else {
            if empty||self.sequence==1||self.date!=Some(date) {
                writeln!(out,"LOG_CONTEXT {:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z pid={} seq={} uptime_ms={} format=compact-v1",
                    time.year,time.month,time.day,time.hour,time.minute,time.second,time.millis,std::process::id(),self.sequence,uptime)?;
            }
            writeln!(out,"[{:02}:{:02}:{:02}.{:03}Z] {}",time.hour,time.minute,time.second,time.millis,text)?;
        }
        self.date=Some(date);Ok(())
    }
}
pub fn verbose()->bool {*VERBOSE}
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
    let mut time=SystemTime::default();unsafe {GetSystemTime(&mut time);}
    let empty=file.metadata().is_ok_and(|m|m.len()==0);
    let _=serial.record(&mut file,&time,START.elapsed().as_millis(),&message.to_string(),empty,verbose());
}
pub(crate) fn module_path(address:usize)->Option<PathBuf> {
    let mut buffer=vec![0u16;32768];
    let length=unsafe {GetModuleFileNameW(address as _,buffer.as_mut_ptr(),buffer.len() as u32)} as usize;
    (length>0&&length<buffer.len()).then(||PathBuf::from(String::from_utf16_lossy(&buffer[..length])))
}
pub fn startup(address:usize) {
    super::log(format!("DIAGNOSTIC_SWITCHES no_attendant={} stall_victory={}",std::env::var("ERDTREE_TOWER_NO_ATTENDANT").as_deref()==Ok("1"),std::env::var("ERDTREE_TOWER_STALL_VICTORY").as_deref()==Ok("1")));
    let path=|address|module_path(address).map(|p|p.display().to_string()).unwrap_or_else(||"-".into());
    super::log(format!("SESSION_START build={} protocol={} dll=\"{}\" exe=\"{}\" log=\"{}\"",env!("TOWER_BUILD_ID"),erdtree_tower::party_handshake::VERSION,path(address),path(0),super::LOG_PATH.display()));
    for name in ["ersc.dll","dxgi.dll","d3d12.dll","dinput8.dll","ReShade64.dll","nvngx_dlssg.dll"] {
        let wide:Vec<_>=name.encode_utf16().chain(Some(0)).collect();
        let handle=unsafe {GetModuleHandleW(wide.as_ptr())};
        if !handle.is_null(){super::log(format!("MODULE name={name} path=\"{}\"",path(handle as usize)));}
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
        let render_stalled=stalled(now,frame);let task_stalled=stalled(now,task);
        if verbose()||render_stalled||task_stalled {
            super::log(format!("HEALTH uptime_ms={now} render_last_ms={frame} task_last_ms={task} render_stalled={render_stalled} task_stalled={task_stalled}"));
        } else {super::log("HEALTH render=ok task=ok");}
    });
}
#[derive(Default)]
pub struct Changes {last:String,at:Option<Instant>}
#[derive(Default)]
pub struct LoadTrace {started:Option<Instant>,last_stage:String,last_state:String,last_sample:Option<Instant>,triggered:bool}
impl LoadTrace {
    pub fn needs_detail(&self,now:Instant,trigger:bool,verbose:bool)->bool {
        verbose||(!(trigger&&!self.triggered)&&self.started.is_some_and(|start|now.saturating_duration_since(start)>=Duration::from_secs(10)))
    }
    #[cfg(test)]
    pub fn observe(&mut self,now:Instant,trigger:bool,stage:&str,detail:&str)->Option<String> {
        self.observe_state(now,trigger,stage,"",detail)
    }
    pub fn observe_state(&mut self,now:Instant,trigger:bool,stage:&str,state:&str,detail:&str)->Option<String> {
        if trigger&&!self.triggered {self.started=Some(now);self.last_stage.clear();self.last_state.clear();self.last_sample=None;}
        self.triggered=trigger;
        let start=self.started?;let elapsed=now.saturating_duration_since(start).as_millis();
        let terminal=matches!(stage,"combat_ready"|"room_ready")||elapsed>=180_000;
        if !terminal&&stage==self.last_stage&&state==self.last_state&&self.last_sample.is_some_and(|t|now.saturating_duration_since(t)<Duration::from_secs(5)){return None;}
        self.last_stage=stage.into();self.last_state=state.into();self.last_sample=Some(now);
        if terminal{self.started=None;}
        Some(format!("LOAD_TRACE elapsed_ms={elapsed} stage={stage} end={terminal} {detail}"))
    }
}
impl Changes {
    pub fn due(&mut self,key:String,now:Instant)->bool {
        if key==self.last&&self.at.is_some_and(|t|now.duration_since(t)<Duration::from_secs(30)){return false;}
        self.last=key;self.at=Some(now);true
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn compact_lines_keep_context_once_and_escape_multiline_errors() {
        let mut state=super::LogState::default();let mut output=Vec::new();
        let time=super::SystemTime{year:2026,month:11,day:7,hour:13,minute:1,second:2,millis:345,..Default::default()};
        state.record(&mut output,&time,100,"READY",true,false).unwrap();
        state.record(&mut output,&time,101,"ERROR first\nsecond\rthird",false,false).unwrap();
        let text=String::from_utf8(output).unwrap();
        assert_eq!(text.matches("LOG_CONTEXT").count(),1);
        assert!(text.contains("2026-11-07T13:01:02.345Z"));
        assert!(text.contains("[13:01:02.345Z] READY\n"));
        assert!(text.contains("[13:01:02.345Z] ERROR first\\nsecond\\rthird\n"));
        assert_eq!(text.lines().count(),3);
        assert_eq!(text.matches("pid=").count(),1);
        assert_eq!(text.matches("seq=").count(),1);
    }
    #[test]
    fn compact_context_repeats_after_midnight_and_rotation() {
        let mut state=super::LogState::default();let mut output=Vec::new();
        let mut time=super::SystemTime{year:2026,month:11,day:7,..Default::default()};
        state.record(&mut output,&time,0,"A",true,false).unwrap();
        time.day=8;
        state.record(&mut output,&time,1,"B",false,false).unwrap();
        state.record(&mut output,&time,2,"C",true,false).unwrap();
        let text=String::from_utf8(output).unwrap();
        assert_eq!(text.matches("LOG_CONTEXT").count(),3);
        assert!(text.contains("seq=3 uptime_ms=2"));
    }
    #[test]
    fn verbose_lines_preserve_full_timing_and_sequence() {
        let mut state=super::LogState::default();let mut output=Vec::new();
        let time=super::SystemTime{year:2026,month:11,day:7,..Default::default()};
        state.record(&mut output,&time,123,"EVENT",true,true).unwrap();
        let text=String::from_utf8(output).unwrap();
        assert!(text.starts_with("2026-11-07T00:00:00.000Z pid="));
        assert!(text.ends_with("seq=1 +123ms EVENT\n"));
    }
    #[test]
    fn slow_loads_expand_automatically_and_new_loads_reset() {
        use super::*;
        let now=Instant::now();let mut trace=LoadTrace::default();
        assert!(!trace.needs_detail(now,true,false));
        trace.observe(now,true,"warp","");
        assert!(!trace.needs_detail(now+Duration::from_millis(9999),true,false));
        assert!(trace.needs_detail(now+Duration::from_secs(10),true,false));
        assert!(trace.needs_detail(now,true,true));
        trace.observe(now+Duration::from_secs(11),false,"room_ready","");
        assert!(!trace.needs_detail(now+Duration::from_secs(12),true,false));
    }
    #[test]
    fn load_state_changes_are_immediate_but_position_does_not_flood_log() {
        use super::*;
        let now=Instant::now(); let mut trace=LoadTrace::default();
        assert!(trace.observe_state(now,true,"not_grounded","gravity=1","position=0").is_some());
        assert!(trace.observe_state(now+Duration::from_millis(250),true,"not_grounded","gravity=1","position=1").is_none());
        assert!(trace.observe_state(now+Duration::from_millis(500),true,"not_grounded","gravity=0","position=1").is_some());
        assert!(trace.observe_state(now+Duration::from_millis(750),true,"not_grounded","gravity=0","position=2").is_none());
        assert!(trace.observe_state(now+Duration::from_millis(5500),true,"not_grounded","gravity=0","position=3").is_some());
    }
    use super::*;
    #[test]
    fn load_trace_separates_map_readiness_and_battle_and_limits_repeated_logs() {
        let now=Instant::now();let mut trace=LoadTrace::default();
        assert!(trace.observe(now,false,"room_ready","").is_none());
        assert!(trace.observe(now,true,"warp","").unwrap().contains("elapsed_ms=0"));
        assert!(trace.observe(now+Duration::from_secs(1),true,"warp","").is_none());
        assert!(trace.observe(now+Duration::from_secs(5),false,"waiting_center","").unwrap().contains("end=false"));
        assert!(trace.observe(now+Duration::from_secs(10),false,"waiting_center","").is_some());
        assert!(trace.observe(now+Duration::from_secs(30),false,"combat_ready","").unwrap().contains("elapsed_ms=30000"));
        assert!(trace.observe(now+Duration::from_secs(31),false,"combat_ready","").is_none());
        assert!(trace.observe(now+Duration::from_secs(32),true,"warp","").unwrap().contains("elapsed_ms=0"));
        assert!(trace.observe(now+Duration::from_secs(212),false,"waiting_center","").unwrap().contains("end=true"));
    }
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
