#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Position {pub block:i32,pub xyz:[f32;3]}
impl Position {
    pub fn in_physics_space(self,current_block:Self,current_physics:Self)->Option<Self> {
        let xyz=std::array::from_fn(|i|self.xyz[i]+current_physics.xyz[i]-current_block.xyz[i]);
        (self.block==current_block.block&&self.block==current_physics.block&&self.block>=0
            &&self.xyz.iter().chain(&current_block.xyz).chain(&current_physics.xyz).chain(&xyz).all(|v|v.is_finite())).then_some(Self{block:self.block,xyz})
    }
}
#[derive(Clone,Copy,Debug,PartialEq)]
pub enum Action {None,Waiting,RequestRevival(Position),FinishRevival}
#[derive(Default)]
pub struct Death {epoch:Option<u64>,position:Option<Position>,requested:bool,finished:bool}
impl Death {
    pub fn awaiting_revival(&self)->bool {self.position.is_some()&&!self.finished}
    pub fn redirect(&mut self,destination:Position) {
        if !self.finished&&self.position.is_some_and(|p|p.block==destination.block)
            &&destination.xyz.iter().all(|n|n.is_finite()){self.position=Some(destination);}
    }
    pub fn epoch(&self)->Option<u64>{self.epoch}
    #[cfg(test)]
    pub fn observe(&mut self,epoch:Option<u64>,player:Option<(Position,bool)>,victory:bool,warping:bool)->Action {
        self.observe_confirmed(epoch,player,victory,warping,true)
    }
    pub fn observe_confirmed(&mut self,epoch:Option<u64>,player:Option<(Position,bool)>,victory:bool,warping:bool,recovered:bool)->Action {
        if self.epoch!=epoch||self.finished&&player.is_some_and(|(_,dead)|dead) { *self=Self{epoch,..Default::default()}; }
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
    fn another_death_in_the_same_mausoleum_visit_can_recover_again() {
        let mut death=Death::default();let epoch=Some(7);
        let entrance=Position{block:0x0d000000,xyz:[5.86,1.50,4.43]};
        assert_eq!(death.observe_confirmed(epoch,Some((entrance,true)),true,false,false),Action::RequestRevival(entrance));
        assert_eq!(death.observe_confirmed(epoch,Some((entrance,false)),true,false,true),Action::FinishRevival);
        assert_eq!(death.observe_confirmed(epoch,Some((entrance,false)),true,false,true),Action::None);
        let fallen=Position{xyz:[0.0,-20.0,30.0],..entrance};
        assert_eq!(death.observe_confirmed(epoch,Some((fallen,true)),true,false,true),Action::RequestRevival(fallen));
        death.redirect(entrance);
        assert_eq!(death.observe_confirmed(epoch,Some((entrance,false)),true,false,false),Action::Waiting);
        assert_eq!(death.observe_confirmed(epoch,Some((entrance,false)),true,false,true),Action::FinishRevival);
    }
    #[test]
    fn safe_entry_tracks_world_origin_changes_instead_of_reusing_havok_coordinates() {
        let block=0x0d000000;
        let entrance=Position{block,xyz:[21.856926,1009.5039,300.43274]};
        let current=Position{block,xyz:[15.329869,1009.58844,329.86993]};
        let physics=Position{block,xyz:[-0.670131,1.58844,1.86993]};
        let goal=entrance.in_physics_space(current,physics).unwrap();
        assert!((goal.xyz[2]+27.56726).abs()<0.001,"the captured entry's old Havok Z=4.43 belongs to a different origin");
        let shifted=entrance.in_physics_space(current,Position{xyz:[physics.xyz[0],physics.xyz[1],physics.xyz[2]+32.0],..physics}).unwrap();
        assert!((shifted.xyz[2]-goal.xyz[2]-32.0).abs()<0.001);
        assert!(entrance.in_physics_space(Position{block:45,..current},physics).is_none());
        assert!(entrance.in_physics_space(current,Position{xyz:[f32::NAN,0.0,0.0],..physics}).is_none());
    }
    #[test]
    fn trial_death_uses_safe_goal_and_waits_for_native_confirmation() {
        let mut d=Death::default();let fall=Position{block:35,xyz:[0.0,-500.0,0.0]};
        let goal=Position{block:35,xyz:[125.644,-408.535,-61.213]};
        d.observe_confirmed(Some(1),Some((fall,true)),false,false,false);
        assert!(d.awaiting_revival());
        assert_eq!(d.observe_confirmed(Some(1),Some((fall,true)),true,false,false),Action::RequestRevival(fall));
        d.redirect(goal);
        assert_eq!(d.observe_confirmed(Some(1),Some((goal,false)),true,false,false),Action::Waiting);
        assert!(d.awaiting_revival());
        assert_eq!(d.observe_confirmed(Some(1),Some((goal,false)),true,false,true),Action::FinishRevival);
        assert!(!d.awaiting_revival());
        assert_eq!(d.observe_confirmed(Some(1),Some((goal,false)),true,false,true),Action::None);
    }
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
