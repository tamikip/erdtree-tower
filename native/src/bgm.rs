#[derive(Default)]
pub struct Frame {pub active:bool}
impl Drop for Frame {
    fn drop(&mut self) {let _=super::native_talk::set_flag(45020969,self.active);}
}
pub fn playing(alive:bool,warping:bool,travelling:bool,solo:bool,party:bool)->bool {
    alive&&!warping&&!travelling&&(solo||party)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn music_requires_combat_and_stops_on_death_or_travel() {
        for solo in [false,true] {for party in [false,true] {
            assert_eq!(playing(true,false,false,solo,party),solo||party);
            assert!(!playing(false,false,false,solo,party));
            assert!(!playing(true,true,false,solo,party));
            assert!(!playing(true,false,true,solo,party));
        }}
        assert!(!Frame::default().active);
    }
}
