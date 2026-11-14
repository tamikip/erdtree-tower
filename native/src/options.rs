// 启动参数：DLL 所在目录（模组目录）的 tower-options.ini，格式见 erdtree_tower::options。启动时读一次，改了下次启动游戏生效。
use std::{path::PathBuf,sync::{LazyLock,OnceLock}};
use hudhook::imgui::Key;
use erdtree_tower::options::{FILE,Hotkey,Options,parse};

static DIR:OnceLock<PathBuf>=OnceLock::new();

/// DllMain 里调用：记下 DLL 所在目录
pub fn init(dll:Option<PathBuf>) {
    if let Some(dir)=dll.and_then(|p|p.parent().map(PathBuf::from)) {let _=DIR.set(dir);}
    let options=&*OPTIONS;
    erdtree_tower::localization::set(options.language);
    // 单人按本机设置；联机时由房主在握手里定下整队模式（都有 DLC 才按有 DLC，见 sync_dlc）
    crate::log(format!("OPTIONS renderer={}",options.renderer.id()));
    let dlc=options.dlc;
    erdtree_tower::dlc::set(dlc);
    crate::log(format!("OPTIONS file={:?} debug={} dlc={} prompts={} keys={}",path(),options.debug,dlc.id(),options.prompts.id(),
        Hotkey::ALL.map(|h|format!("{}={}",h.id(),options.key(h))).join(",")));
}

/// 模组目录（DLL 所在目录）：tower-options.ini、联机大厅的 tower-lobby.json 都在这里
pub fn dir()->PathBuf {DIR.get().cloned().or_else(||crate::LOG_PATH.parent().map(PathBuf::from)).unwrap_or_default()}
fn path()->PathBuf {dir().join(FILE)}

static OPTIONS:LazyLock<Options>=LazyLock::new(||std::fs::read_to_string(path()).map(|text|parse(&text)).unwrap_or_default());

pub fn get()->&'static Options {&OPTIONS}
/// 本机是否按「有 DLC」运行（启动器里 DLC 设为「有 DLC」）：联机握手时报给房主
pub fn local_dlc()->bool {OPTIONS.dlc==erdtree_tower::dlc::Mode::Full}
fn effective_dlc(local:erdtree_tower::dlc::Mode,party:Option<Option<bool>>)->erdtree_tower::dlc::Mode {
    use erdtree_tower::dlc::Mode;
    match party {None=>local,Some(Some(true)) if local==Mode::Full=>Mode::Full,Some(_)=>Mode::None}
}
/// 每帧调用：联机仅在全队明确确认有 DLC 时启用；未知状态和本机禁用均不放行。
pub fn sync_dlc(party:Option<Option<bool>>) {
    use erdtree_tower::dlc::Mode;
    let want=effective_dlc(OPTIONS.dlc,party);
    if erdtree_tower::dlc::mode()!=want {
        erdtree_tower::dlc::set(want);
        crate::log(format!("DLC mode -> {}（{}）",want.id(),if party.is_none(){"单人，按本机设置"}else if party==Some(None){"队伍 DLC 尚未确认，暂不启用 DLC 内容"}else if want==Mode::None{"队伍里有人没有 DLC，整队不出 DLC 内容"}else{"全队都有 DLC"}));
    }
}
#[test]
fn no_dlc_settings_are_never_promoted_by_unknown_party_state() {
    use erdtree_tower::dlc::Mode;
    for local in [Mode::Full,Mode::None,Mode::Weapons] {
        assert_eq!(effective_dlc(local,None),local);
        assert_eq!(effective_dlc(local,Some(None)),Mode::None);
        assert_eq!(effective_dlc(local,Some(Some(false))),Mode::None);
        assert_eq!(effective_dlc(local,Some(Some(true))),if local==Mode::Full{Mode::Full}else{Mode::None});
    }
}
/// 绑定到某个功能的键在界面上的写法（如 Home、F10、J）
pub fn label(hotkey:Hotkey)->&'static str {crate::native_settings::live_key(hotkey).unwrap_or_else(||OPTIONS.key(hotkey))}
pub fn key(hotkey:Hotkey)->Key {to_key(label(hotkey)).unwrap_or_else(||to_key(hotkey.default_key()).unwrap_or(Key::Home))}
pub fn pressed(ui:&hudhook::imgui::Ui,hotkey:Hotkey)->bool {
    if !crate::input::foreground() {return false;}
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
