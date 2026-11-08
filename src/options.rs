//! 启动参数：模组目录（DLL 所在目录）里的 tower-options.ini，由启动器写入，也可以手动改；下次启动游戏生效。
//! 每行 `名字=值`，`#` 或 `;` 开头是注释，不认识的行原样忽略：
//!   debug=1              打开调试菜单（和放一个 tower-debug.enable 文件效果相同）
//!   dlc=full             DLC 内容：full 有 DLC；none 没有 DLC；weapons 没有 DLC 但保留 DLC 武器（实验）。只对单人生效，见 dlc.rs
//!   key.settle=Home      快速结算（暂离结束本局）
//!   key.overlay=Insert   显示/隐藏爬塔界面
//!   key.archive=J        旅程档案
//!   key.build=C          构筑面板（地图界面）
//!   key.debug=Delete     调试菜单；None 表示未绑定
//!   sfx=70               界面音效音量（0–100，0 为关闭）
//!   prompts=keyboard     界面按键提示：keyboard 键鼠；pad 手柄。固定显示所选的一种，不随最近用的设备切换

pub const FILE: &str = "tower-options.ini";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hotkey {
    Settle,
    Overlay,
    Archive,
    Build,
    Debug,
    /// 跳过本关：视为本场战斗胜利，直接到三选一（联机只有房主能用）
    Skip,
}

impl Hotkey {
    pub const ALL: [Hotkey; 6] = [Hotkey::Settle, Hotkey::Overlay, Hotkey::Archive, Hotkey::Build, Hotkey::Debug, Hotkey::Skip];
    pub fn id(self) -> &'static str {
        match self {
            Hotkey::Settle => "settle",
            Hotkey::Overlay => "overlay",
            Hotkey::Archive => "archive",
            Hotkey::Build => "build",
            Hotkey::Debug => "debug",
            Hotkey::Skip => "skip",
        }
    }
    pub fn default_key(self) -> &'static str {
        match self {
            Hotkey::Settle => "Home",
            Hotkey::Overlay => "Insert",
            Hotkey::Archive => "J",
            Hotkey::Build => "C",
            Hotkey::Debug => "Delete",
            Hotkey::Skip => "End",
        }
    }
}

/// 可以绑定的键（imgui 能识别的键盘键）；数字键写成 0–9，小键盘写成 Num0–Num9
pub const KEYS: &[&str] = &[
    "None", "Backspace", "Tab", "Enter", "LeftCtrl", "RightCtrl", "LeftShift", "RightShift", "LeftAlt", "RightAlt", "Space",
    "Up", "Down", "Left", "Right", "NumEnter", "NumMultiply", "NumSubtract", "NumAdd", "NumDecimal", "NumDivide",
    "Home", "End", "Insert", "Delete", "PageUp", "PageDown", "Pause", "ScrollLock",
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
    "Num0", "Num1", "Num2", "Num3", "Num4", "Num5", "Num6", "Num7", "Num8", "Num9",
];

/// 界面按键提示画成键鼠键帽还是手柄按键（启动器设置页选择）
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Prompts {
    #[default]
    Keyboard,
    Pad,
}

impl Prompts {
    pub fn id(self) -> &'static str {
        match self {
            Prompts::Keyboard => "keyboard",
            Prompts::Pad => "pad",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        [Prompts::Keyboard, Prompts::Pad].into_iter().find(|p| p.id().eq_ignore_ascii_case(value.trim()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub debug: bool,
    pub dlc: crate::dlc::Mode,
    keys: [&'static str; 6],
    /// 界面音效音量（0–100）
    pub sfx: u8,
    pub prompts: Prompts,
}

impl Default for Options {
    fn default() -> Self {
        Options { debug: false, dlc: crate::dlc::Mode::Full, keys: Hotkey::ALL.map(Hotkey::default_key), sfx: 70, prompts: Prompts::Keyboard }
    }
}

impl Options {
    pub fn key(&self, hotkey: Hotkey) -> &'static str {
        self.keys[hotkey as usize]
    }
}

fn canonical_key(name: &str) -> Option<&'static str> {
    KEYS.iter().copied().find(|k| k.eq_ignore_ascii_case(name.trim()))
}

pub fn parse(text: &str) -> Options {
    let mut options = Options::default();
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else { continue };
        let (name, value) = (name.trim().to_ascii_lowercase(), value.trim());
        if name == "debug" {
            options.debug = matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "on" | "yes");
        } else if name == "sfx" {
            if let Ok(volume) = value.trim().parse::<u32>() {
                options.sfx = volume.min(100) as u8;
            }
        } else if name == "prompts" {
            if let Some(prompts) = Prompts::parse(value) {
                options.prompts = prompts;
            }
        } else if name == "dlc" {
            if let Some(mode) = crate::dlc::Mode::parse(value) {
                options.dlc = mode;
            }
        } else if let Some(id) = name.strip_prefix("key.")
            && let Some(hotkey) = Hotkey::ALL.into_iter().find(|h| h.id() == id)
            && let Some(key) = canonical_key(value)
        {
            options.keys[hotkey as usize] = key;
        }
    }
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_empty_file_keeps_the_original_keys() {
        let options = parse("");
        assert!(!options.debug);
        assert_eq!(Hotkey::ALL.map(|h| options.key(h)), ["Home", "Insert", "J", "C", "Delete", "End"]);
    }

    #[test]
    fn launcher_file_rebinds_keys_and_enables_debug() {
        let text = "\u{feff}# 黄金树爬塔启动参数\r\ndebug = 1\r\ndlc=Weapons\r\nkey.settle=end\r\nKEY.Overlay= f9 \r\nkey.archive=num5\r\n; key.build=X\r\n";
        let options = parse(text);
        assert!(options.debug);
        assert_eq!(options.dlc, crate::dlc::Mode::Weapons);
        assert_eq!(options.key(Hotkey::Settle), "End");
        assert_eq!(options.key(Hotkey::Overlay), "F9");
        assert_eq!(options.key(Hotkey::Archive), "Num5");
        assert_eq!(options.key(Hotkey::Build), "C");
        assert_eq!(options.prompts, Prompts::Keyboard, "keyboard prompts unless chosen otherwise");
        assert_eq!(parse("prompts = Pad").prompts, Prompts::Pad);
    }

    #[test]
    fn unknown_keys_values_and_lines_are_ignored() {
        let options = parse("debug=maybe\ndlc=sometimes\nprompts=touch\nkey.settle=Ctrl\nkey.jump=F1\nnonsense\nkey.debug=");
        assert_eq!(options, Options::default());
    }
}
