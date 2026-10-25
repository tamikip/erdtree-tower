#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Position {pub block:i32,pub xyz:[f32;3]}
#[derive(Clone,Copy,Debug,PartialEq)]
pub enum Action {None,Waiting,RequestRevival(Position),FinishRevival}
#[derive(Default)]
pub struct Death {epoch:Option<u64>,position:Option<Position>,requested:bool,finished:bool}
impl Death {
    pub fn epoch(&self)->Option<u64>{self.epoch}
    #[cfg(test)]
    pub fn observe(&mut self,epoch:Option<u64>,player:Option<(Position,bool)>,victory:bool,warping:bool)->Action {
        self.observe_confirmed(epoch,player,victory,warping,true)
    }
    pub fn observe_confirmed(&mut self,epoch:Option<u64>,player:Option<(Position,bool)>,victory:bool,warping:bool,recovered:bool)->Action {
        if self.epoch!=epoch { *self=Self{epoch,..Default::default()}; }
        if epoch.is_none(){return Action::None;}
        if let Some((pos,true))=player {
            if self.position.is_none()&&pos.xyz.iter().all(|n|n.is_finite()) {
                self.position=Some(pos);
            }
        }
        let Some(origin)=self.position else{return Action::None;};
        if self.finished{return Action::None;}
        if !victory||warping{return Action::Waiting;}
        if let Some((current,false))=player {
            let distance=current.xyz.iter().zip(origin.xyz).map(|(a,b)|(a-b).powi(2)).sum::<f32>();
            if recovered&&current.block==origin.block&&distance<=4.0 {
                self.finished=true;return Action::FinishRevival;
            }
        }
        if !self.requested {self.requested=true;return Action::RequestRevival(origin);}
        Action::Waiting
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waits_for_authoritative_victory_and_requests_only_once() {
        let mut d=Death::default();let p=Position{block:45,xyz:[1.0,2.0,3.0]};
        assert_eq!(d.observe(Some(1),Some((p,true)),false,false),Action::Waiting);
        assert_eq!(d.observe(Some(1),None,false,true),Action::Waiting);
        assert_eq!(d.observe(Some(1),Some((p,true)),true,false),Action::RequestRevival(p));
        assert_eq!(d.observe(Some(1),Some((p,true)),true,false),Action::Waiting);
        assert_eq!(d.observe(Some(1),Some((p,false)),true,false),Action::FinishRevival);
        assert_eq!(d.observe(Some(1),Some((p,false)),true,false),Action::None);
    }
    #[test]
    fn departure_is_not_revival_and_defeat_cancels_pending_request() {
        let mut d=Death::default();let p=Position{block:45,xyz:[0.0;3]};
        d.observe(Some(1),Some((p,true)),false,false);
        assert!(matches!(d.observe(Some(1),Some((Position{block:11,..p},false)),true,false),Action::RequestRevival(_)));
        assert_eq!(d.observe(None,None,false,false),Action::None);
        assert_eq!(d.observe(Some(2),Some((p,false)),true,false),Action::None);
    }
    #[test]
    fn alive_character_requires_native_spectator_exit_confirmation() {
        let mut d=Death::default();let p=Position{block:45,xyz:[0.0;3]};
        d.observe_confirmed(Some(1),Some((p,true)),false,false,false);
        assert_eq!(d.observe_confirmed(Some(1),Some((p,true)),true,false,false),Action::RequestRevival(p));
        assert_eq!(d.observe_confirmed(Some(1),Some((p,false)),true,false,false),Action::Waiting);
        assert_eq!(d.observe_confirmed(Some(1),Some((p,false)),true,false,true),Action::FinishRevival);
    }
}
