use std::time::Duration;

#[derive(Debug, PartialEq, Eq)]
pub enum ResultState {
    Waiting,
    PortalReady,
    Ready,
    PlayerDied,
}

#[derive(Default)]
pub struct Completion { armed:bool }
impl Completion {
    pub fn poll(&mut self,elapsed:Duration,dead:bool,interacted:bool)->ResultState {
        match poll(elapsed,dead,interacted&&self.armed) {
            ResultState::PortalReady if !self.armed=>{self.armed=true;ResultState::PortalReady},
            ResultState::PortalReady=>ResultState::Waiting,
            state=>state,
        }
    }
}

pub fn poll(elapsed: Duration, player_dead: bool, interacted: bool) -> ResultState {
    if player_dead {
        ResultState::PlayerDied
    } else if elapsed >= Duration::from_secs(3) && interacted {
        ResultState::Ready
    } else if elapsed >= Duration::from_secs(3) {
        ResultState::PortalReady
    } else {
        ResultState::Waiting
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portal_arms_once_and_requires_a_later_interaction() {
        let mut gate=Completion::default();
        assert_eq!(gate.poll(Duration::from_secs(2),false,true),ResultState::Waiting);
        assert_eq!(gate.poll(Duration::from_secs(3),false,true),ResultState::PortalReady);
        for _ in 0..1000 {assert_eq!(gate.poll(Duration::from_secs(4),false,false),ResultState::Waiting);}
        assert_eq!(gate.poll(Duration::from_secs(5),false,true),ResultState::Ready);
        assert_eq!(Completion::default().poll(Duration::from_secs(4),true,false),ResultState::PlayerDied);
    }
    #[test]
    fn waits_three_seconds_without_suppressing_player_death() {
        assert_eq!(
            poll(Duration::from_millis(2999), false, true),
            ResultState::Waiting
        );
        assert_eq!(poll(Duration::from_secs(3), false, false), ResultState::PortalReady);
        assert_eq!(poll(Duration::from_secs(400), false, false), ResultState::PortalReady);
        assert_eq!(poll(Duration::from_secs(4), false, true), ResultState::Ready);
        assert_eq!(poll(Duration::from_secs(3), true, true), ResultState::PlayerDied);
    }
}
