use std::{path::Path,sync::{Once,OnceLock}};
use erdtree_tower::party_compat::{Compatibility,parse_rules};

const ASSETS:[&str;10]=["regulation.bin","arenas-resources.txt","enemy-pools.txt","boss-pool.txt","battlefields.txt",
    "party-dlc-slots.tsv","party-extra-slots.tsv","hidden-road-resources.txt","event-hosts.txt","party-services.txt"];
fn append(hash:&mut u64,bytes:&[u8]) {for &b in bytes{*hash=(*hash^u64::from(b)).wrapping_mul(0x100000001b3);}}
fn read(path:&Path)->Result<Vec<u8>,String> {
    std::fs::read(path).map_err(|_|format!("无法读取开局校验文件 {}，请修复整合包后重启游戏",path.file_name().unwrap_or_default().to_string_lossy()))
}
fn collect(build:&str,version:&str,module:&Path,assets:&Path)->Result<Compatibility,String> {
    let mut seamless_hash=0xcbf29ce484222325;append(&mut seamless_hash,&read(module)?);
    let config=read(&module.with_file_name("ersc_settings.ini"))?;
    let config=std::str::from_utf8(&config).map_err(|_|"无缝配置不是有效 UTF-8，请修复配置后重启游戏")?;
    let rules=parse_rules(config)?;
    let mut hash=0xcbf29ce484222325;
    for name in ASSETS {
        let bytes=read(&assets.join(name))?;
        append(&mut hash,name.as_bytes());append(&mut hash,&[0]);append(&mut hash,&(bytes.len() as u64).to_le_bytes());append(&mut hash,&bytes);
    }
    let result=Compatibility{tower_build:build.into(),seamless_version:version.into(),seamless_hash:format!("{seamless_hash:016x}"),assets:format!("{hash:016x}"),rules};
    result.validate()?;Ok(result)
}
pub fn profile()->Option<&'static Result<Compatibility,String>> {
    static START:Once=Once::new();static RESULT:OnceLock<Result<Compatibility,String>>=OnceLock::new();
    START.call_once(||{std::thread::spawn(||{
        let result=super::seamless_revival::verified_module().ok_or_else(||"无缝版本尚未通过接口校验，请安装受支持版本并重启游戏".to_owned())
            .and_then(|(version,module)|collect(env!("TOWER_BUILD_ID"),version,&module,&crate::LOG_PATH.with_file_name("scene-assets")));
        match &result {Ok(p)=>super::log(format!("PARTY_COMPAT ready build={} seamless={} assets={}",p.tower_build,p.seamless_version,p.assets)),Err(e)=>super::log(format!("PARTY_COMPAT failed: {e}"))}
        let _=RESULT.set(result);
    });});
    RESULT.get()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disk_profile_excludes_secrets_and_detects_changed_or_missing_assets() {
        let root=std::env::temp_dir().join(format!("tower-compat-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();
        let config="allow_invaders=0\ndeath_debuffs=0\nallow_summons=1\nalways_spectate_on_death=1\nenemy_health_scaling=100\nenemy_damage_scaling=20\nenemy_posture_scaling=30\nboss_health_scaling=100\nboss_damage_scaling=20\nboss_posture_scaling=80\n";
        let module=root.join("ersc.dll");std::fs::write(&module,b"fixture DLL, never executed").unwrap();
        for name in ASSETS {std::fs::write(root.join(name),name).unwrap();}
        std::fs::write(root.join("ersc_settings.ini"),format!("{config}cooppassword=first-secret\n")).unwrap();
        let before=collect("0123456789abcdef","2.0.3",&module,&root).unwrap();
        std::fs::write(root.join("ersc_settings.ini"),format!("{config}cooppassword=second-secret\nmod_language_override=english\n")).unwrap();
        assert_eq!(before,collect("0123456789abcdef","2.0.3",&module,&root).unwrap());
        std::fs::write(root.join("regulation.bin"),b"modified").unwrap();
        assert!(before.check(&collect("0123456789abcdef","2.0.3",&module,&root).unwrap()).unwrap_err().contains("关键资源"));
        std::fs::remove_file(root.join("regulation.bin")).unwrap();
        assert!(collect("0123456789abcdef","2.0.3",&module,&root).unwrap_err().contains("regulation.bin"));
        for name in ASSETS.into_iter().filter(|&n|n!="regulation.bin").chain(["ersc.dll","ersc_settings.ini"]) {std::fs::remove_file(root.join(name)).unwrap();}
        std::fs::remove_dir(root).unwrap();
    }
}
