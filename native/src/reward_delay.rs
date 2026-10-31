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
    /// 传送门已经亮起：此后的交互才算数
    pub fn armed(&self)->bool {self.armed}
    pub fn poll(&mut self,elapsed:Duration,dead:bool,interacted:bool)->ResultState {
        match poll(elapsed,dead,interacted&&self.armed) {
            ResultState::PortalReady if !self.armed=>{self.armed=true;ResultState::PortalReady},
            ResultState::PortalReady=>ResultState::Waiting,
            state=>state,
        }
    }
}

/// 打完怪后多久亮起传送门：战斗场地的传送门这时才从场地正中的地面钻出来（地图事件见 scene-builder/PortalReveal.cs）
pub const PORTAL_DELAY:Duration=Duration::from_secs(1);
pub fn poll(elapsed: Duration, player_dead: bool, interacted: bool) -> ResultState {
    if player_dead {
        ResultState::PlayerDied
    } else if elapsed >= PORTAL_DELAY && interacted {
        ResultState::Ready
    } else if elapsed >= PORTAL_DELAY {
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
        assert_eq!(gate.poll(Duration::from_millis(900),false,true),ResultState::Waiting);
        assert_eq!(gate.poll(Duration::from_secs(1),false,true),ResultState::PortalReady);
        for _ in 0..1000 {assert_eq!(gate.poll(Duration::from_secs(2),false,false),ResultState::Waiting);}
        assert_eq!(gate.poll(Duration::from_secs(3),false,true),ResultState::Ready);
        assert_eq!(Completion::default().poll(Duration::from_secs(2),true,false),ResultState::PlayerDied);
    }
    #[test]
    fn waits_one_second_without_suppressing_player_death() {
        assert_eq!(poll(Duration::from_millis(999), false, true), ResultState::Waiting);
        assert_eq!(poll(Duration::from_secs(1), false, false), ResultState::PortalReady);
        assert_eq!(poll(Duration::from_secs(400), false, false), ResultState::PortalReady);
        assert_eq!(poll(Duration::from_secs(2), false, true), ResultState::Ready);
        assert_eq!(poll(Duration::from_secs(1), true, true), ResultState::PlayerDied);
    }
}
