use std::{ffi::{c_void,CStr},ptr::NonNull};
use erdtree_tower::party_roster::Roster;

const CHANNEL:i32=0x4554;
const MAX_PACKET:usize=256*1024;
#[repr(C)]
struct Identity {bytes:[u8;136]}
#[repr(C)]
struct Message {data:*const u8,size:i32,connection:u32,peer:Identity}
type Interface=unsafe extern "C" fn()->*mut c_void;
type SetIdentity=unsafe extern "C" fn(*mut Identity,u64);
type GetIdentity=unsafe extern "C" fn(*const Identity)->u64;
type Send=unsafe extern "C" fn(*mut c_void,*const Identity,*const u8,u32,i32,i32)->i32;
type Receive=unsafe extern "C" fn(*mut c_void,i32,*mut *mut Message,i32)->i32;
type Release=unsafe extern "C" fn(*mut Message);
type Close=unsafe extern "C" fn(*mut c_void,*const Identity,i32)->bool;
type Accept=unsafe extern "C" fn(*mut c_void,*const Identity)->bool;
type ConnectionInfo=unsafe extern "C" fn(*mut c_void,*const Identity,*mut c_void,*mut RealtimeStatus)->i32;
// Steamworks SteamNetConnectionRealTimeStatus_t (NetworkingMessages002).
#[repr(C)]
#[derive(Default)]
struct RealtimeStatus {
    state:i32,ping:i32,quality:[f32;2],rates:[f32;4],counters:[i32;4],queue_time:i64,reserved:[u32;16],
}
// Steamworks SteamNetConnectionInfo_t：断线时读 end_reason / end_debug，弄清 Steam 连接为何中断。
#[repr(C)]
struct ConnInfo {
    identity:[u8;136],user_data:i64,listen:u32,addr:[u8;18],pad:u16,pop_remote:u32,pop_relay:u32,
    state:i32,end_reason:i32,end_debug:[u8;128],description:[u8;128],flags:i32,reserved:[u32;63],
}
const NET_LOG_INTERVAL:std::time::Duration=std::time::Duration::from_secs(30);
const MESSAGES_ENTRY:&CStr=c"SteamAPI_SteamNetworkingMessages_SteamAPI_v002";
fn messages_module(mut lookup:impl FnMut(&str,&CStr)->Option<(*mut c_void,*mut c_void)>)->Result<(*mut c_void,*mut c_void,&'static str),String> {
    for name in ["steam_api64_o.dll","steam_api64.dll"] {
        if let Some((module,entry))=lookup(name,MESSAGES_ENTRY).filter(|(module,entry)|!module.is_null()&&!entry.is_null()) {
            return Ok((module,entry,name));
        }
    }
    Err("未找到已加载的原始 Steam 消息接口；已停止调用不兼容的转发入口".into())
}
#[link(name="kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const i8)->*mut c_void;
}

pub struct Transport {
    interface:NonNull<c_void>,set:SetIdentity,get:GetIdentity,
    send:Send,receive:Receive,release:Release,close:Close,accept:Accept,roster:Roster,
    connection_info:Option<ConnectionInfo>,
    net_logged:std::cell::Cell<Option<std::time::Instant>>,
}
// Access remains serialized by the runtime lock; no interface call overlaps another.
unsafe impl std::marker::Send for Transport {}
pub struct Pending {
    pub roster:Roster,
    transport:Transport,
    handshake:erdtree_tower::party_handshake::Handshake,
    hello:std::time::Instant,
    started:std::time::Instant,
    packets:Vec<(u64,erdtree_tower::party_handshake::Payload)>,
}
impl Pending {
    pub fn waiting_reason(&self)->String {self.handshake.waiting_reason()}
    pub fn new(roster:Roster,nonce:u64,seed:u64,dlc:bool)->Result<Self,String> {
        let handshake=erdtree_tower::party_handshake::Handshake::new(roster.clone(),nonce,seed)?.with_dlc(dlc);
        Ok(Self{transport:Transport::load(roster.clone())?,roster,handshake,hello:std::time::Instant::now()-std::time::Duration::from_secs(1),started:std::time::Instant::now(),packets:vec![]})
    }
    pub fn poll(&mut self)->Result<Option<(u64,u64)>,String> {
        use erdtree_tower::party_handshake::Payload;
        if self.hello.elapsed()>=std::time::Duration::from_secs(1) {
            self.hello=std::time::Instant::now();
            if let Some((peer,message))=self.handshake.hello(){self.transport.send(peer,&Payload::Handshake(message).encode()?)?;}
        }
        for (peer,bytes) in self.transport.receive()? {
            match Payload::decode(&bytes).map_err(|error|format!("队员 {peer}：{error}"))? {
                Payload::Handshake(message)=>{
                    for (target,response) in self.handshake.receive_lobby(peer,message)? {
                        self.transport.send(target,&Payload::Handshake(response).encode()?)?;
                    }
                },
                packet=>{
                    if self.handshake.ready().is_none() {continue;}
                    if self.packets.len()>=64 {return Err("队伍握手缓存已满".into());}
                    self.packets.push((peer,packet));
                },
            }
        }
        let ready=self.handshake.ready();
        if ready.is_none()&&handshake_expired(self.started.elapsed()) {
            super::log(format!("Party handshake still waiting: host={} peers={}; retaining nonce and retrying",self.roster.local==self.roster.host,self.roster.members.len()-1));
            self.started=std::time::Instant::now();
        }
        Ok(ready)
    }
    pub fn finish(self)->(Transport,erdtree_tower::party_handshake::Handshake,Vec<(u64,erdtree_tower::party_handshake::Payload)>) {(self.transport,self.handshake,self.packets)}
}
impl Transport {
    pub fn load(roster:Roster)->Result<Self,String> {
        if !super::seamless::active() {return Err("无缝入口未加载".into());}
        let (module,entry,module_name)=messages_module(|name,symbol| {
            let wide:Vec<_>=name.encode_utf16().chain(std::iter::once(0)).collect();
            let module=unsafe {GetModuleHandleW(wide.as_ptr())};
            if module.is_null(){return None;}
            Some((module,unsafe {GetProcAddress(module,symbol.as_ptr())}))
        })?;
        let address=|name:&CStr|->Result<*mut c_void,String>{
            let p=unsafe {GetProcAddress(module,name.as_ptr())};
            if p.is_null(){Err(format!("Steam API 缺少 {}",name.to_string_lossy()))}else{Ok(p)}
        };
        unsafe {
            for symbol in [c"SteamAPI_SteamNetworkingIdentity_SetSteamID64",c"SteamAPI_SteamNetworkingIdentity_GetSteamID64",
                c"SteamAPI_ISteamNetworkingMessages_SendMessageToUser",c"SteamAPI_ISteamNetworkingMessages_ReceiveMessagesOnChannel",
                c"SteamAPI_SteamNetworkingMessage_t_Release",c"SteamAPI_ISteamNetworkingMessages_CloseChannelWithUser",
                c"SteamAPI_ISteamNetworkingMessages_AcceptSessionWithUser"] {address(symbol)?;}
            super::log(format!("Steam messages module: {module_name}; obtaining original v002 interface"));
            let interface:Interface=std::mem::transmute(entry);
            let interface=NonNull::new(interface()).ok_or("原始 Steam 消息接口未就绪，请确认 Steam 已登录并运行")?;
            super::log("Steam messages original interface ready");
            Ok(Self{
                interface,
                set:std::mem::transmute(address(c"SteamAPI_SteamNetworkingIdentity_SetSteamID64")?),
                get:std::mem::transmute(address(c"SteamAPI_SteamNetworkingIdentity_GetSteamID64")?),
                send:std::mem::transmute(address(c"SteamAPI_ISteamNetworkingMessages_SendMessageToUser")?),
                receive:std::mem::transmute(address(c"SteamAPI_ISteamNetworkingMessages_ReceiveMessagesOnChannel")?),
                release:std::mem::transmute(address(c"SteamAPI_SteamNetworkingMessage_t_Release")?),
                close:std::mem::transmute(address(c"SteamAPI_ISteamNetworkingMessages_CloseChannelWithUser")?),roster,
                accept:std::mem::transmute(address(c"SteamAPI_ISteamNetworkingMessages_AcceptSessionWithUser")?),
                connection_info:address(c"SteamAPI_ISteamNetworkingMessages_GetSessionConnectionInfo").ok().map(|p|std::mem::transmute::<*mut c_void,ConnectionInfo>(p)),
                net_logged:std::cell::Cell::new(None),
            })
        }
    }
    /// 队员退出后换成缩小后的名单（之后不再给他发消息，也不收他的）
    pub fn set_roster(&mut self,roster:Roster) {self.roster=roster;}
    pub fn send(&self,peer:u64,data:&[u8])->Result<(),String> {
        if !allowed(&self.roster,peer,data.len()){return Err("无效队伍消息目标或长度".into());}
        let mut identity=Identity{bytes:[0;136]};
        let result=unsafe {
            (self.set)(&mut identity,peer);
            (self.send)(self.interface.as_ptr(),&identity,data.as_ptr(),data.len() as u32,8|1,CHANNEL)
        };
        if result==1 {return Ok(());}
        super::log_repeating(format!("PARTY_NET send failed result={result} {}",self.diagnose(peer)));
        Err(format!("Steam 消息发送失败：{result}"))
    }
    /// 读 Steam 到该队员的连接状态：状态、断开原因与调试文本、延迟和连接质量。只用于写日志排查掉线。
    pub fn diagnose(&self,peer:u64)->String {
        let Some(query)=self.connection_info else{return format!("peer={peer} connection_info=unsupported");};
        if peer==self.roster.local {return format!("peer={peer} local");}
        let mut identity=Identity{bytes:[0;136]};
        let mut info:ConnInfo=unsafe {std::mem::zeroed()};
        let mut status=RealtimeStatus{ping:-1,..Default::default()};
        let state=unsafe {
            (self.set)(&mut identity,peer);
            query(self.interface.as_ptr(),&identity,(&mut info as *mut ConnInfo).cast(),&mut status)
        };
        let text=|bytes:&[u8]|{let end=bytes.iter().position(|&b|b==0).unwrap_or(bytes.len());String::from_utf8_lossy(&bytes[..end]).into_owned()};
        format!("peer={peer} state={state} info_state={} end_reason={} end_debug=\"{}\" relay={} ping={} quality={:.2}/{:.2} pending={}/{}",
            info.state,info.end_reason,text(&info.end_debug),info.pop_relay!=0,status.ping,status.quality[0],status.quality[1],status.counters[0],status.counters[1])
    }
    fn periodic_log(&self) {
        if self.net_logged.get().is_some_and(|at|at.elapsed()<NET_LOG_INTERVAL){return;}
        self.net_logged.set(Some(std::time::Instant::now()));
        for &peer in &self.roster.members {if peer!=self.roster.local {super::log(format!("PARTY_NET {}",self.diagnose(peer)));}}
    }
    pub fn latency_supported(&self)->bool {self.connection_info.is_some()}
    pub fn ping(&self,peer:u64)->Option<u32> {
        if !allowed(&self.roster,peer,1){return None;}
        let query=self.connection_info?;
        let mut identity=Identity{bytes:[0;136]};
        let mut status=RealtimeStatus{ping:-1,..Default::default()};
        let state=unsafe {
            (self.set)(&mut identity,peer);
            query(self.interface.as_ptr(),&identity,std::ptr::null_mut(),&mut status)
        };
        (state==3&&status.state==3&&status.ping>=0&&status.ping<=erdtree_tower::party_latency::MAX_MS as i32).then_some(status.ping as u32)
    }
    pub fn receive(&self)->Result<Vec<(u64,Vec<u8>)>,String> {
        self.periodic_log();
        for &peer in &self.roster.members {
            if peer==self.roster.local {continue;}
            let mut identity=Identity{bytes:[0;136]};
            unsafe {(self.set)(&mut identity,peer);(self.accept)(self.interface.as_ptr(),&identity);}
        }
        let mut messages=[std::ptr::null_mut();32];
        let count=unsafe {(self.receive)(self.interface.as_ptr(),CHANNEL,messages.as_mut_ptr(),messages.len() as i32)};
        if !(0..=messages.len() as i32).contains(&count){return Err("Steam 接收数量无效".into());}
        let mut packets=Vec::new();
        for &message in &messages[..count as usize] {
            if message.is_null(){continue;}
            let owned=OwnedMessage{message,release:self.release};
            let msg=unsafe {&*owned.message};
            let peer=unsafe {(self.get)(&msg.peer)};
            if msg.size>0&&!msg.data.is_null()&&allowed(&self.roster,peer,msg.size as usize) {
                packets.push((peer,unsafe {std::slice::from_raw_parts(msg.data,msg.size as usize)}.to_vec()));
            }
        }
        Ok(packets)
    }
}
fn handshake_expired(elapsed:std::time::Duration)->bool {elapsed>=std::time::Duration::from_secs(60)}
fn allowed(roster:&Roster,peer:u64,size:usize)->bool {
    peer!=roster.local&&roster.members.contains(&peer)&&size>0&&size<=MAX_PACKET
}
struct OwnedMessage {message:*mut Message,release:Release}
impl Drop for OwnedMessage {fn drop(&mut self){unsafe {(self.release)(self.message)}}}
impl Drop for Transport {
    fn drop(&mut self) {
        for &peer in &self.roster.members {
            if peer==self.roster.local {continue;}
            let mut identity=Identity{bytes:[0;136]};
            unsafe {(self.set)(&mut identity,peer);(self.close)(self.interface.as_ptr(),&identity,CHANNEL);}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connection_info_matches_steamworks_abi() {
        assert_eq!(std::mem::size_of::<ConnInfo>(),696);
        assert_eq!(std::mem::offset_of!(ConnInfo,user_data),136);
        assert_eq!(std::mem::offset_of!(ConnInfo,pop_remote),168);
        assert_eq!(std::mem::offset_of!(ConnInfo,state),176);
        assert_eq!(std::mem::offset_of!(ConnInfo,end_reason),180);
        assert_eq!(std::mem::offset_of!(ConnInfo,end_debug),184);
        assert_eq!(std::mem::offset_of!(ConnInfo,flags),440);
    }
    #[test]
    fn realtime_status_matches_steamworks_abi() {
        assert_eq!(std::mem::size_of::<RealtimeStatus>(),120);
        assert_eq!(std::mem::align_of::<RealtimeStatus>(),8);
        assert_eq!(std::mem::offset_of!(RealtimeStatus,ping),4);
        assert_eq!(std::mem::offset_of!(RealtimeStatus,queue_time),48);
        assert_eq!(std::mem::offset_of!(RealtimeStatus,reserved),56);
    }
    #[test]
    fn messages_module_uses_loaded_original_without_calling_wrapper_alias() {
        let pointer=NonNull::<u8>::dangling().as_ptr().cast::<c_void>();
        for available in ["steam_api64_o.dll","steam_api64.dll"] {
            let (module,entry,name)=messages_module(|name,symbol| {
                assert_eq!(symbol,MESSAGES_ENTRY);
                (name==available).then_some((pointer,pointer))
            }).unwrap();
            assert_eq!(module,pointer);assert_eq!(entry,pointer);assert_eq!(name,available);
        }
        let (_,_,preferred)=messages_module(|_,_|Some((pointer,pointer))).unwrap();
        assert_eq!(preferred,"steam_api64_o.dll");
        assert!(messages_module(|_,_|None).is_err());
        assert!(messages_module(|_,_|Some((pointer,std::ptr::null_mut()))).is_err());
    }
    #[test]
    fn handshake_wait_diagnostics_are_rate_limited() {
        assert!(!handshake_expired(std::time::Duration::from_secs(59)));
        assert!(handshake_expired(std::time::Duration::from_secs(60)));
    }
    #[test]
    fn only_current_remote_members_and_bounded_payloads_are_accepted() {
        let roster=Roster{local:1,host:1,members:vec![1,2]};
        assert!(allowed(&roster,2,MAX_PACKET));
        for (peer,size) in [(1,1),(3,1),(0,1),(2,0),(2,MAX_PACKET+1)] {assert!(!allowed(&roster,peer,size));}
        assert_eq!(std::mem::size_of::<Identity>(),136);
        assert_eq!(std::mem::offset_of!(Message,peer),16);
    }
    #[test]
    fn rejected_payloads_still_release_the_steam_message() {
        use std::sync::atomic::{AtomicUsize,Ordering};
        static RELEASES:AtomicUsize=AtomicUsize::new(0);
        unsafe extern "C" fn release(_: *mut Message){RELEASES.fetch_add(1,Ordering::SeqCst);}
        let mut message=Message{data:std::ptr::null(),size:-1,connection:0,peer:Identity{bytes:[0;136]}};
        {let _owned=OwnedMessage{message:&mut message,release};}
        assert_eq!(RELEASES.load(Ordering::SeqCst),1);
    }
}
