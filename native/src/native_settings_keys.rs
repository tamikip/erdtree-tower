// ERNativeUI 1.3 erui.h ERUI_KeyboardKey order, including INVALID as unbound.
pub const NAMES: [&str;73] = [
    "None","1","2","3","4","5","6","7","8","9","0","Backspace","Tab",
    "Q","W","E","R","T","Y","U","I","O","P","Enter","LeftCtrl",
    "A","S","D","F","G","H","J","K","L","LeftShift","Z","X","C","V","B","N","M","RightShift","LeftAlt","Space",
    "Num7","Num8","Num9","Num4","Num5","Num6","Num1","Num2","Num3","Num0","NumEnter","RightCtrl","RightAlt",
    "Home","Up","PageUp","Left","Right","End","Down","PageDown","Insert","Delete","NumMultiply","NumSubtract","NumAdd","NumDecimal","NumDivide",
];
pub fn code(name:&str)->Option<u32> {NAMES.iter().position(|key|*key==name).map(|i|i as u32)}
#[cfg(test)]
mod tests {
    #[test]
    fn native_keys_roundtrip_and_defaults_are_supported() {
        for (i,name) in super::NAMES.iter().enumerate() {
            assert_eq!(super::code(name),Some(i as u32));
            assert!(erdtree_tower::options::KEYS.contains(name));
        }
        assert_eq!(super::code("F8"),None);
        assert_eq!(super::code("F10"),None);
        for hotkey in crate::native_settings::NATIVE {assert!(super::code(hotkey.default_key()).is_some());}
        assert!(!crate::native_settings::NATIVE.contains(&erdtree_tower::options::Hotkey::Recover));
    }
}
