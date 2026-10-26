use std::{sync::{LazyLock,Mutex},time::{Duration,Instant}};
use erdtree_tower::{music::{self,Track},Phase,Run};

#[derive(Default)]
pub struct Frame {pub active:bool,pub track:Option<&'static Track>}
#[derive(Default)]
struct Playback {initialized:bool,current:Option<u32>,pending:Option<(u32,Duration)>}
impl Playback {
    fn sync(&mut self,now:Duration,desired:Option<u32>,mut write:impl FnMut(u32,bool)->Result<(),String>)->Result<bool,String> {
        if !self.initialized {
            write(45020969,false)?;
            for track in music::CATALOG.iter(){write(track.flag,false)?;}
            self.initialized=true;
        }
        if self.current==desired&&self.pending.is_none(){return Ok(false);}
        let mut changed=false;
        if let Some(old)=self.current {write(old,false)?;self.current=None;changed=true;}
        let Some(next)=desired else {self.pending=None;return Ok(changed);};
        match self.pending {
            Some((flag,at)) if flag==next&&now.saturating_sub(at)>=Duration::from_millis(350)=>{
                write(next,true)?;self.current=Some(next);self.pending=None;Ok(true)
            },
            Some((flag,_)) if flag==next=>Ok(changed),
            _=>{self.pending=Some((next,now));Ok(changed)},
        }
    }
}
impl Drop for Frame {
    fn drop(&mut self) {
        static PLAYBACK:Mutex<Playback>=Mutex::new(Playback{initialized:false,current:None,pending:None});
        static CLOCK:LazyLock<Instant>=LazyLock::new(Instant::now);
        let Ok(mut playback)=PLAYBACK.try_lock() else{return;};
        if playback.current.is_some_and(|id|!super::native_talk::flag(id).unwrap_or(false)){playback.current=None;}
        let result=playback.sync(CLOCK.elapsed(),self.track.map(|t|t.flag),|id,on| {
            super::native_talk::set_flag(id,on)?;
            if super::native_talk::flag(id)?!=on{return Err("音乐选择标记未生效".into());}Ok(())
        });
        match result {
            Ok(true)=>if let Some(track)=music::CATALOG.iter().find(|t|Some(t.flag)==playback.current){super::log(format!("BGM start: id={} track={} pool={:?}",track.id,track.name,track.pool));}else{super::log("BGM stopped");},
            Err(error) if self.track.is_some()=>super::log_repeating(format!("BGM: {error}")),
            _=>{},
        }
    }
}
pub fn selection(run:Option<&Run>,block:i32,alive:bool,warping:bool,travelling:bool,combat:bool,wave:u8)->Option<&'static Track> {
    if !alive||warping||travelling||!music::supported(block){return None;}
    if block==super::hub::BLOCK{return Some(music::hub());}
    let run=run?;
    let id=match run.phase {
        Phase::Combat{id}=>{if !combat{return None;}id},
        Phase::Node{id}|Phase::Reward{id,..}=>id,
        Phase::Map=>*run.visited.last()?,
        Phase::Dead|Phase::Won=>return None,
    };
    let challenge=matches!(run.phase,Phase::Node{..})&&(run.regional_event.descent_started||run.regional_event.stormveil==Some(erdtree_tower::stormveil::Progress::Running));
    Some(music::for_node(run,id,combat||challenge,wave))
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
    #[test]
    fn switching_has_a_silent_gap_and_never_enables_two_tracks() {
        let mut playback=Playback::default();let mut enabled=std::collections::HashSet::new();
        let mut write=|flag,on|{if on{enabled.insert(flag);}else{enabled.remove(&flag);}assert!(enabled.len()<=1);Ok(())};
        let a=music::CATALOG[0].flag;let b=music::CATALOG[1].flag;
        assert!(!playback.sync(Duration::ZERO,Some(a),&mut write).unwrap());
        assert!(playback.sync(Duration::from_millis(350),Some(a),&mut write).unwrap());
        assert!(playback.sync(Duration::from_secs(1),Some(b),&mut write).unwrap());
        assert_eq!(playback.current,None);
        assert!(!playback.sync(Duration::from_millis(1200),Some(b),&mut write).unwrap());
        assert!(playback.sync(Duration::from_millis(1350),Some(b),&mut write).unwrap());
        assert_eq!(playback.current,Some(b));
        assert!(playback.sync(Duration::from_secs(2),None,&mut write).unwrap());
        assert_eq!(playback.current,None);assert!(enabled.is_empty());
    }
    #[test]
    fn failed_start_is_retried_and_pending_music_can_be_cancelled() {
        let mut playback=Playback::default();let flag=music::CATALOG[0].flag;
        playback.sync(Duration::ZERO,Some(flag),|_,_|Ok(())).unwrap();
        assert!(playback.sync(Duration::from_secs(1),Some(flag),|_,on|if on{Err("unavailable".into())}else{Ok(())}).is_err());
        assert_eq!(playback.current,None);
        playback.sync(Duration::from_secs(2),Some(flag),|_,_|Ok(())).unwrap();assert_eq!(playback.current,Some(flag));
        playback.sync(Duration::from_secs(3),None,|_,_|Ok(())).unwrap();
        playback.sync(Duration::from_secs(4),Some(flag),|_,_|Ok(())).unwrap();
        playback.sync(Duration::from_millis(4100),None,|_,_|Ok(())).unwrap();assert!(playback.pending.is_none());
    }
    #[test]
    fn level_contexts_choose_combat_ambient_and_challenge_music_safely() {
        use erdtree_tower::{Class,NodeKind,music::Pool};
        let mut run=Run::new(42,Class::Vagabond);let block=0x2d020000;
        assert_eq!(selection(None,super::super::hub::BLOCK,true,false,false,false,0).unwrap().id,music::hub().id);
        run.phase=Phase::Combat{id:0};
        assert!(selection(Some(&run),block,true,false,false,false,0).is_none());
        assert_eq!(selection(Some(&run),block,true,false,false,true,0).unwrap().pool,Pool::Battle);
        for (alive,warp,travel) in [(false,false,false),(true,true,false),(true,false,true)] {assert!(selection(Some(&run),block,alive,warp,travel,true,0).is_none());}
        assert!(selection(Some(&run),0x3c000000,true,false,false,true,0).is_none());
        run.phase=Phase::Node{id:0};run.nodes[0].kind=NodeKind::Camp;
        assert_eq!(selection(Some(&run),block,true,false,false,false,0).unwrap().pool,Pool::Calm);
        run.nodes[0].kind=NodeKind::Event;run.regional_event.stormveil=Some(erdtree_tower::stormveil::Progress::Running);
        assert_eq!(selection(Some(&run),0x0a000000,true,false,false,false,0).unwrap().pool,Pool::Battle);
        run.regional_event.stormveil=Some(erdtree_tower::stormveil::Progress::Complete);
        assert_eq!(selection(Some(&run),0x0a000000,true,false,false,false,0).unwrap().pool,Pool::Explore);
    }
}
