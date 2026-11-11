use std::{ffi::c_void, path::Path, sync::{Mutex, OnceLock}};
use erdtree_tower::options::{Hotkey, KEYS, FILE};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use crate::native_settings_keys;

static CONFIG: OnceLock<std::path::PathBuf> = OnceLock::new();
static SAVED: Mutex<[u8; 8]> = Mutex::new([0; 8]);
static LIVE_KEYS: OnceLock<[AtomicU8; 5]> = OnceLock::new();
/// 游戏设置页（外部 tower_native_settings.dll，接口 v3 固定 5 个按键槽）管的键；「跳过本关」只在启动器里改键，由 imgui 读
const NATIVE: [Hotkey; 5] = [Hotkey::Settle, Hotkey::Overlay, Hotkey::Archive, Hotkey::Build, Hotkey::Debug];
static BINDINGS: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicU8 = AtomicU8::new(0);
static FRAME: AtomicU8 = AtomicU8::new(0);
pub fn begin_frame() {FRAME.store(PENDING.swap(0,Ordering::AcqRel),Ordering::Release);}
pub fn take_pressed(key:Hotkey)->Option<bool> {(BINDINGS.load(Ordering::Acquire)&&NATIVE.contains(&key)).then(||FRAME.fetch_and(!(1 << key as u8),Ordering::AcqRel)&(1 << key as u8)!=0)}
unsafe extern "C" fn activate(slot:u32) {if slot<5 && !crate::input::active() {PENDING.fetch_or(1 << slot,Ordering::AcqRel);}}
pub fn live_key(key: Hotkey) -> Option<&'static str> { LIVE_KEYS.get().filter(|_|NATIVE.contains(&key)).and_then(|keys| KEYS.get(keys.get(key as usize)?.load(Ordering::Acquire) as usize).copied()) }
const NAMES: [&str; 8] = ["debug", "sfx", "dlc", "key.settle", "key.overlay", "key.archive", "key.build", "key.debug"];

#[link(name="kernel32")]
unsafe extern "system" {
    fn LoadLibraryW(path: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const i8) -> *mut c_void;
    fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
}
fn wide(path: &Path) -> Vec<u16> { path.as_os_str().to_string_lossy().encode_utf16().chain(Some(0)).collect() }

fn update(text: &str, before: &[u8; 8], after: &[u8; 8]) -> Result<String, &'static str> {
    if after[0] > 1 || after[1] > 100 || after[2] > 2 || after[3..].iter().any(|&v| v as usize >= KEYS.len()) { return Err("invalid settings"); }
    let values = [after[0].to_string(), after[1].to_string(), ["full", "none", "weapons"][after[2] as usize].to_owned(),
        KEYS[after[3] as usize].to_owned(), KEYS[after[4] as usize].to_owned(), KEYS[after[5] as usize].to_owned(), KEYS[after[6] as usize].to_owned(), KEYS[after[7] as usize].to_owned()];
    let mut seen = [false; 8];
    let mut output = String::new();
    if text.starts_with('\u{feff}') { output.push('\u{feff}'); }
    for line in text.trim_start_matches('\u{feff}').lines() {
        let index = line.split_once('=').and_then(|(key, _)| NAMES.iter().position(|name| key.trim().eq_ignore_ascii_case(name)));
        if let Some(i) = index.filter(|&i| before[i] != after[i]) {
            if seen[i] { continue; }
            output.push_str(&format!("{}={}\r\n", NAMES[i], values[i])); seen[i] = true;
        } else { output.push_str(line); output.push_str("\r\n"); }
    }
    for i in 0..8 { if before[i] != after[i] && !seen[i] { output.push_str(&format!("{}={}\r\n", NAMES[i], values[i])); } }
    Ok(output)
}

unsafe extern "C" fn save(values: *const u8) -> i32 {
    let result = std::panic::catch_unwind(|| -> Result<(), String> {
        if values.is_null() { return Err("null settings".into()); }
        let mut after: [u8; 8] = unsafe { std::slice::from_raw_parts(values, 8) }.try_into().unwrap();
        let mut before = SAVED.lock().map_err(|e| e.to_string())?;
        after[3..].copy_from_slice(&before[3..]);
        let path = CONFIG.get().ok_or("configuration path missing")?;
        persist(path, &before, &after)?;
        *before = after;
        Ok(())
    });
    match result { Ok(Ok(())) => { crate::log("NATIVE_SETTINGS saved; restart required"); 1 }, other => { crate::log(format!("NATIVE_SETTINGS save failed: {other:?}")); 0 } }
}

fn persist(path: &Path, before: &[u8;8], after: &[u8;8]) -> Result<(), String> {
    let text = match std::fs::read_to_string(path) { Ok(text) => text, Err(e) if e.kind()==std::io::ErrorKind::NotFound => String::new(), Err(e) => return Err(e.to_string()) };
    let output = update(&text, before, after)?;
    let temporary = path.with_extension("ini.native-ui.tmp");
    std::fs::write(&temporary, output).map_err(|e| e.to_string())?;
    if unsafe { MoveFileExW(wide(&temporary).as_ptr(), wide(path).as_ptr(), 1 | 8) } == 0 { return Err(std::io::Error::last_os_error().to_string()); }
    Ok(())
}
fn binding_values(before: &[u8;8], codes: &[u32]) -> Result<[u8;8], String> {
    if codes.len()!=5 {return Err("invalid binding count".into());}
    let mut after=*before;
    for (slot,&code) in codes.iter().enumerate(){
        let name=native_settings_keys::NAMES.get(code as usize).ok_or("invalid native key")?;
        after[slot+3]=KEYS.iter().position(|key|key==name).ok_or("unknown native key")? as u8;
    }
    Ok(after)
}
unsafe extern "C" fn save_bindings(codes:*const u32)->i32 {
    let result=std::panic::catch_unwind(||->Result<(),String>{
        if codes.is_null(){return Err("null bindings".into());}
        let codes=unsafe{std::slice::from_raw_parts(codes,5)};
    let mut before = SAVED.lock().map_err(|e| e.to_string())?;
    let after=binding_values(&before,codes)?;
    let path = CONFIG.get().ok_or("配置文件未初始化")?;
    persist(path, &before, &after).map_err(|error| { crate::log(format!("NATIVE_SETTINGS key save failed: {error}")); "保存失败，请查看日志；原按键保持不变".to_owned() })?;
    *before = after;
    if let Some(keys) = LIVE_KEYS.get() { for i in 0..5 {keys[i].store(after[i+3], Ordering::Release);} }
    PENDING.store(0,Ordering::Release); FRAME.store(0,Ordering::Release);
    crate::log(format!("NATIVE_SETTINGS bindings saved and active: {:?}",codes));
    Ok(())
    });
    match result {Ok(Ok(()))=>1,other=>{crate::log(format!("NATIVE_SETTINGS bindings save failed: {other:?}"));0}}
}

pub fn init(module: Option<std::path::PathBuf>) {
    let Some(root) = module.as_deref().and_then(Path::parent) else { return; };
    let _ = CONFIG.set(root.join(FILE));
    let options = crate::options::get();
    let mut initial = [0u8; 8];
    initial[0] = u8::from(options.debug); initial[1] = options.sfx;
    initial[2] = match options.dlc.id() { "none" => 1, "weapons" => 2, _ => 0 };
    for (i, key) in NATIVE.iter().enumerate() { initial[i+3] = KEYS.iter().position(|name| *name == options.key(*key)).unwrap_or(0) as u8; }
    let folder = root.join("native-ui");
    for name in ["ERNativeUI.dll", "tower_native_settings.dll"] {
        if !folder.join(name).is_file() { crate::log(format!("NATIVE_SETTINGS unavailable: {name} missing")); return; }
    }
    let previous=initial;
    let defaults: [u32;5]=NATIVE.map(|key|native_settings_keys::code(key.default_key()).unwrap());
    for i in 0..5 {if native_settings_keys::code(KEYS[initial[i+3] as usize]).is_none(){initial[i+3]=KEYS.iter().position(|key|*key==NATIVE[i].default_key()).unwrap() as u8;}}
    if previous!=initial {
        if let Err(error)=persist(&root.join(FILE),&previous,&initial){crate::log(format!("NATIVE_SETTINGS binding migration failed: {error}"));return;}
        crate::log("NATIVE_SETTINGS unsupported F/Pause/ScrollLock bindings migrated to native defaults");
    }
    *SAVED.lock().unwrap()=initial;
    let _=LIVE_KEYS.set(std::array::from_fn(|i|AtomicU8::new(initial[i+3])));
    let codes: [u32;5]=std::array::from_fn(|i|native_settings_keys::code(KEYS[initial[i+3] as usize]).unwrap());
    unsafe {
        let host = LoadLibraryW(wide(&folder.join("ERNativeUI.dll")).as_ptr());
        if host.is_null() { crate::log(format!("NATIVE_SETTINGS host load failed: {}", std::io::Error::last_os_error())); return; }
        let client = LoadLibraryW(wide(&folder.join("tower_native_settings.dll")).as_ptr());
        if client.is_null() { crate::log(format!("NATIVE_SETTINGS client load failed: {}", std::io::Error::last_os_error())); return; }
        let language_symbol=GetProcAddress(client,c"tower_native_settings_set_language".as_ptr());
        if !language_symbol.is_null() {
            let configure:unsafe extern "C" fn(u32)=std::mem::transmute(language_symbol);
            configure(match crate::options::get().language {erdtree_tower::localization::Language::Auto=>0,erdtree_tower::localization::Language::Chinese=>1,erdtree_tower::localization::Language::English=>2});
        }
        let symbol = GetProcAddress(client, c"tower_native_settings_init_v3".as_ptr());
        if symbol.is_null() { crate::log("NATIVE_SETTINGS entry missing"); return; }
        type Init = unsafe extern "C" fn(*const u8,*const u32,*const u32,unsafe extern "C" fn(*const u8)->i32,unsafe extern "C" fn(*const u32)->i32,unsafe extern "C" fn(u32))->u32;
        let start: Init = std::mem::transmute(symbol);
        let result = start(initial.as_ptr(),codes.as_ptr(),defaults.as_ptr(),save,save_bindings,activate);
        if result==0&&crate::options::get().language==erdtree_tower::localization::Language::Auto {
            let query=GetProcAddress(client,c"tower_native_settings_language".as_ptr());
            if !query.is_null() {
                let language:unsafe extern "C" fn()->u32=std::mem::transmute(query);
                let selected=if language()==1 {erdtree_tower::localization::Language::English}else{erdtree_tower::localization::Language::Chinese};
                erdtree_tower::localization::set(selected);crate::log(format!("LANGUAGE game={}",selected.id()));
            }
        }
        BINDINGS.store(result==0,Ordering::Release);
        crate::log(format!("NATIVE_SETTINGS registration result={result}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn initial_keys() -> [u8;8] {
        let mut values = [0;8];
        for (i, key) in NATIVE.iter().enumerate() { values[i+3]=KEYS.iter().position(|name| *name==key.default_key()).unwrap() as u8; }
        values
    }
    #[test]
    fn native_bindings_clear_and_preserve_other_settings() {
        let mut before = initial_keys(); before[1]=70;
        let after = binding_values(&before, &[0,66,31,37,67]).unwrap();
        assert!(binding_values(&before,&[999,66,31,37,67]).is_err());
        assert_eq!(&after[..3], &before[..3]);
        assert_eq!(&after[4..], &before[4..]);
        let text = update("sfx=99\nkey.settle=Home\ncustom=keep", &before, &after).unwrap();
        let options = erdtree_tower::options::parse(&text);
        assert_eq!(options.key(Hotkey::Settle), "None");
        assert_eq!(options.sfx, 99);
        assert!(text.contains("custom=keep"));
    }
    #[test]
    fn atomic_key_save_roundtrips_and_reports_failed_writes() {
        let root=std::env::temp_dir().join(format!("tower-key-test-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();
        let path=root.join("options.ini");
        let before=initial_keys();
        let after=binding_values(&before,&[63,66,31,37,67]).unwrap();
        std::fs::write(&path,"# retained\nkey.settle=Home\n").unwrap();
        persist(&path,&before,&after).unwrap();
        assert_eq!(erdtree_tower::options::parse(&std::fs::read_to_string(&path).unwrap()).key(Hotkey::Settle),"End");
        assert!(!path.with_extension("ini.native-ui.tmp").exists());
        assert!(persist(&root.join("missing/options.ini"),&before,&after).is_err());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn preserves_unknown_and_external_settings_and_removes_changed_duplicates() {
        let before = [0; 8]; let mut after = before; after[1] = 85;
        let output = update("\u{feff}# settings\r\ncustom=yes\r\ndebug=1\r\nsfx=10\r\nSFX=20\r\n", &before, &after).unwrap();
        assert_eq!(output, "\u{feff}# settings\r\ncustom=yes\r\ndebug=1\r\nsfx=85\r\n");
    }
    #[test]
    fn adds_missing_settings_and_rejects_invalid_indices() {
        let before = [0;8]; let mut after = before; after[2]=2;
        assert_eq!(update("", &before, &after).unwrap(), "dlc=weapons\r\n");
        after[3]=255; assert!(update("", &before, &after).is_err());
    }
}
