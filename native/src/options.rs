// 启动参数：DLL 所在目录（模组目录）的 tower-options.ini，格式见 erdtree_tower::options。启动时读一次，改了下次启动游戏生效。
use std::{path::PathBuf,sync::{LazyLock,OnceLock}};
use hudhook::imgui::Key;
use erdtree_tower::options::{FILE,Hotkey,Options,parse};

static DIR:OnceLock<PathBuf>=OnceLock::new();

/// DllMain 里调用：记下 DLL 所在目录
pub fn init(dll:Option<PathBuf>) {
    if let Some(dir)=dll.and_then(|p|p.parent().map(PathBuf::from)) {let _=DIR.set(dir);}
    let options=&*OPTIONS;
    // DLC 开关只对单人生效：无缝联机时双方必须抽到同样的敌人和奖励，一律按有 DLC 处理
    let seamless=std::env::var("ERDTREE_TOWER_SEAMLESS_ENTRY").as_deref()==Ok("1");
    let dlc=if seamless {erdtree_tower::dlc::Mode::Full} else {options.dlc};
    erdtree_tower::dlc::set(dlc);
    crate::log(format!("OPTIONS file={:?} debug={} dlc={}{} prompts={} keys={}",path(),options.debug,dlc.id(),if seamless&&options.dlc!=dlc {"（无缝联机，忽略设置）"} else {""},options.prompts.id(),
        Hotkey::ALL.map(|h|format!("{}={}",h.id(),options.key(h))).join(",")));
}

/// 模组目录（DLL 所在目录）：tower-options.ini、联机大厅的 tower-lobby.json 都在这里
pub fn dir()->PathBuf {DIR.get().cloned().or_else(||crate::LOG_PATH.parent().map(PathBuf::from)).unwrap_or_default()}
fn path()->PathBuf {dir().join(FILE)}

static OPTIONS:LazyLock<Options>=LazyLock::new(||std::fs::read_to_string(path()).map(|text|parse(&text)).unwrap_or_default());

pub fn get()->&'static Options {&OPTIONS}
/// 绑定到某个功能的键在界面上的写法（如 Home、F10、J）
pub fn label(hotkey:Hotkey)->&'static str {crate::native_settings::live_key(hotkey).unwrap_or_else(||OPTIONS.key(hotkey))}
pub fn key(hotkey:Hotkey)->Key {to_key(label(hotkey)).unwrap_or_else(||to_key(hotkey.default_key()).unwrap_or(Key::Home))}
pub fn pressed(ui:&hudhook::imgui::Ui,hotkey:Hotkey)->bool {
    if label(hotkey)=="None" {return false;}
    if let Some(pressed)=crate::native_settings::take_pressed(hotkey) {
        return pressed || (crate::input::active() && ui.is_key_pressed_no_repeat(key(hotkey)));
    }
    ui.is_key_pressed_no_repeat(key(hotkey))
}

pub(crate) fn to_key(name:&str)->Option<Key> {
    use Key::*;
    const LETTERS:[Key;26]=[A,B,C,D,E,F,G,H,I,J,K,L,M,N,O,P,Q,R,S,T,U,V,W,X,Y,Z];
    const DIGITS:[Key;10]=[Alpha0,Alpha1,Alpha2,Alpha3,Alpha4,Alpha5,Alpha6,Alpha7,Alpha8,Alpha9];
    const PAD:[Key;10]=[Keypad0,Keypad1,Keypad2,Keypad3,Keypad4,Keypad5,Keypad6,Keypad7,Keypad8,Keypad9];
    const FUNCTION:[Key;12]=[F1,F2,F3,F4,F5,F6,F7,F8,F9,F10,F11,F12];
    Some(match name {
        "Backspace"=>Backspace,"Tab"=>Tab,"Enter"=>Enter,"LeftCtrl"=>LeftCtrl,"RightCtrl"=>RightCtrl,"LeftShift"=>LeftShift,"RightShift"=>RightShift,"LeftAlt"=>LeftAlt,"RightAlt"=>RightAlt,"Space"=>Space,
        "Up"=>UpArrow,"Down"=>DownArrow,"Left"=>LeftArrow,"Right"=>RightArrow,"NumEnter"=>KeypadEnter,"NumMultiply"=>KeypadMultiply,"NumSubtract"=>KeypadSubtract,"NumAdd"=>KeypadAdd,"NumDecimal"=>KeypadDecimal,"NumDivide"=>KeypadDivide,
        "Home"=>Home,"End"=>End,"Insert"=>Insert,"Delete"=>Delete,"PageUp"=>PageUp,"PageDown"=>PageDown,"Pause"=>Pause,"ScrollLock"=>ScrollLock,
        _ if name.len()==1&&name.as_bytes()[0].is_ascii_uppercase()=>LETTERS[(name.as_bytes()[0]-b'A') as usize],
        _ if name.len()==1&&name.as_bytes()[0].is_ascii_digit()=>DIGITS[(name.as_bytes()[0]-b'0') as usize],
        _ if name.starts_with("Num")=>PAD[name[3..].parse::<usize>().ok().filter(|n|*n<10)?],
        _ if name.starts_with('F')=>FUNCTION[name[1..].parse::<usize>().ok().filter(|n|(1..=12).contains(n))?-1],
        _=>return None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_bindable_name_maps_to_a_key() {
        for name in erdtree_tower::options::KEYS {assert!(name==&"None"||super::to_key(name).is_some(),"{name}");}
    }
}
