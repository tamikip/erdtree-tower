//! 联机大厅：房主由启动器“开房”启动游戏，启动器把房间信息写进模组目录的 tower-lobby.json，
//! 游戏里的 DLL 读到后用 Steam 大厅公开这个房间（游戏开着房间就在，关掉游戏 Steam 自动解散）；
//! 其他人在启动器里浏览房间，点“加入”时启动器把这个房间的无缝联机密码写好再启动游戏，玩家不用看也不用填密码。
//! 公开房间把无缝联机密码直接放在大厅数据里；私密房间发布盐和密码摘要，密码由启动器按“盐 + 房间密码”算出，
//! 不知道房间密码的人算不出来（算法见 launcher/Tower.Launcher/SteamLobby.cs）。
use serde::Deserialize;

/// 启动器写给 DLL 的房间文件（模组目录里）；不是房主启动时启动器会删掉它
pub const FILE:&str="tower-lobby.json";
/// DLL 写给启动器的房间状态（同一目录）：一行“状态 详情”，状态为 creating / open / failed；启动器据此显示“你开的房间”
pub const STATUS_FILE:&str="tower-lobby-status.txt";
/// 大厅筛选标记：只列出爬塔的房间
pub const TAG_KEY:&str="tower";
pub const TAG:&str="erdtree-tower";
pub const MAX_PLAYERS:u8=6;

#[derive(Debug,Clone,PartialEq,Eq,Deserialize)]
pub struct HostRoom {
    pub name:String,
    #[serde(default)] pub code:String,
    pub max:u8,
    #[serde(default)] pub locked:bool,
    #[serde(default)] pub salt:String,
    /// 公开房间的无缝联机密码；私密房间为空
    #[serde(default)] pub password:String,
    #[serde(default)] pub password_sha256:String,
    /// 模组版本（启动器的安装记录），版本不同的房间在启动器里不能加入
    pub version:String,
}
fn token(text:&str,min:usize,max:usize)->bool {(min..=max).contains(&text.len())&&text.bytes().all(|b|b.is_ascii_alphanumeric())}
impl HostRoom {
    pub fn parse(text:&str)->Result<Self,String> {
        let room:Self=serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e|format!("房间文件格式无效：{e}"))?;
        room.validate()?;Ok(room)
    }
    pub fn validate(&self)->Result<(),String> {
        let name=self.name.trim();
        if name.is_empty()||name.chars().count()>32||name.chars().any(char::is_control) {return Err("房间名应为 1–32 个字".into());}
        if !self.code.is_empty()&&(self.code.len()!=17||!self.code.starts_with('T')||!self.code[1..].bytes().all(|b|b.is_ascii_digit()||(b'A'..=b'F').contains(&b))) {return Err("房间码格式无效".into());}
        if !(2..=MAX_PLAYERS).contains(&self.max) {return Err("房间人数应为 2–6".into());}
        if !self.password_sha256.is_empty()&&(self.password_sha256.len()!=64||!self.password_sha256.bytes().all(|b|b.is_ascii_hexdigit())) {return Err("房间密码校验信息无效".into());}
        if self.version.is_empty()||self.version.len()>32||self.version.chars().any(char::is_control) {return Err("房间版本无效".into());}
        if self.locked {
            if !token(&self.salt,16,64)||!self.password.is_empty() {return Err("私密房间应只有盐、没有密码".into());}
        } else if !token(&self.password,8,64) {return Err("公开房间的联机密码无效".into());}
        Ok(())
    }
    /// 写进 Steam 大厅的数据（键值都很短，远小于 Steam 的上限）。open：房主已经用无缝联机的道具开启联机（会话为房主），这时别人才连得上；
    /// started：旅程已经出发（不在大赐福），中途加入会出问题，启动器里不允许加入
    pub fn data(&self,host:&str,players:u8,state:&str,open:bool,started:bool)->Vec<(&'static str,String)> {
        let host:String=host.chars().filter(|c|!c.is_control()).take(32).collect();
        let state:String=state.chars().filter(|c|!c.is_control()).take(32).collect();
        vec![(TAG_KEY,TAG.into()),("v",self.version.clone()),("name",self.name.trim().into()),("code",self.code.clone()),("host",host),
            ("pwd_sha256",self.password_sha256.clone()),
            ("max",self.max.to_string()),("players",players.clamp(1,self.max).to_string()),("state",state),
            ("lock",if self.locked{"1"}else{"0"}.into()),("salt",self.salt.clone()),("pwd",self.password.clone()),("open",if open{"1"}else{"0"}.into()),("started",if started{"1"}else{"0"}.into())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn room_code_is_a_discovery_identifier_not_the_connection_secret() {
        let mut room=HostRoom::parse(r#"{"name":"tower","code":"T0123456789ABCDEF","max":4,"password":"independentSecret","version":"v116"}"#).unwrap();
        let data=room.data("host",1,"hub",true,false);
        assert!(data.contains(&("code","T0123456789ABCDEF".into())));
        assert!(data.contains(&("pwd","independentSecret".into())));
        for code in ["1234","T0123456789ABCDEG","T0123456789ABCDEF0","T0123456789abcdef","中0123456789ABCDEF"] {
            room.code=code.into();assert!(room.validate().is_err());
        }
        room.code.clear();assert!(room.validate().is_ok());
    }
    #[test]
    fn private_verifier_is_published_and_room_capacity_is_six() {
        let mut room=HostRoom::parse(r#"{"name":"private","max":6,"locked":true,"salt":"0123456789abcdef","version":"v116","password_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#).unwrap();
        let data=room.data("host",1,"hub",true,false);
        assert!(data.contains(&("pwd_sha256","a".repeat(64))));
        assert!(data.contains(&("pwd",String::new())));
        for size in [1,7,8] {room.max=size;assert!(room.validate().is_err());}
        room.max=6;room.password_sha256="z".repeat(64);assert!(room.validate().is_err());
        room.password_sha256="a".repeat(63);assert!(room.validate().is_err());
    }
    #[test]
    fn host_room_files_are_validated_and_become_lobby_data() {
        let public=HostRoom::parse(r#"{"name":"一起爬塔","max":4,"password":"Ab12Cd34Ef56","version":"v104"}"#).unwrap();
        let data=public.data("褪色者",9,"第 2 幕",true,true);assert!(data.contains(&("started","1".into())));assert!(data.contains(&("open","1".into())));
        assert!(data.contains(&("tower","erdtree-tower".into())));
        assert!(data.contains(&("players","4".into())),"clamped to the room size");
        assert!(data.contains(&("pwd","Ab12Cd34Ef56".into())));assert!(data.contains(&("lock","0".into())));
        let private=HostRoom::parse("\u{feff}{\"name\":\"私密\",\"max\":2,\"locked\":true,\"salt\":\"0123456789abcdef\",\"version\":\"v104\"}").unwrap();
        assert!(private.data("x",1,"大赐福",false,false).contains(&("pwd",String::new())),"private rooms never publish the password");
        for bad in [r#"{"name":"","max":4,"password":"Ab12Cd34Ef56","version":"v1"}"#,
                    r#"{"name":"a","max":9,"password":"Ab12Cd34Ef56","version":"v1"}"#,
                    r#"{"name":"a","max":4,"password":"short","version":"v1"}"#,
                    r#"{"name":"a","max":4,"password":"has space 123","version":"v1"}"#,
                    r#"{"name":"a","max":4,"locked":true,"salt":"0123456789abcdef","password":"Ab12Cd34Ef56","version":"v1"}"#,
                    r#"{"name":"a","max":4,"password":"Ab12Cd34Ef56","version":""}"#] {
            assert!(HostRoom::parse(bad).is_err(),"{bad}");
        }
    }
}
