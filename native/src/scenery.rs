use hudhook::imgui::ImColor32;
use erdtree_tower::{Run,themes::Theme};

pub fn theme(run:&Run)->&'static Theme {erdtree_tower::themes::for_run(run)}
fn rgb(c:u32)->ImColor32 {ImColor32::from_rgb((c>>16) as u8,(c>>8) as u8,c as u8)}
pub fn palette(run:&Run)->(ImColor32,ImColor32,ImColor32) {let t=theme(run);(rgb(t.paper),rgb(t.ink),rgb(t.accent))}
pub fn ink_rgba(run:&Run)->[f32;4] {let c=theme(run).ink;[((c>>16)&255) as f32/255.0,((c>>8)&255) as f32/255.0,(c&255) as f32/255.0,1.0]}

#[cfg(test)]
mod tests {
    #[test]
    fn every_theme_has_a_readable_palette() {
        for theme in erdtree_tower::themes::THEMES.iter() {
            let luma=|c:u32|((c>>16)&255)*3+((c>>8)&255)*6+(c&255);
            assert!(luma(theme.paper)>luma(theme.ink)*3,"{}: ink must stand out on paper",theme.name);
        }
    }
}
