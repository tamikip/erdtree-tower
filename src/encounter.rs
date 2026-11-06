#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observation {
    Alive,
    Dead,
    Missing,
    DifferentEntity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Waiting,
    Victory,
    PlayerDied,
    LostEntity,
    SpawnTimedOut,
}

#[derive(Clone, Debug)]
pub struct Encounter<H> {
    handle: Option<H>,
    terminal: Option<Outcome>,
}

impl<H: Copy + Eq> Default for Encounter<H> {
    fn default() -> Self {
        Self {
            handle: None,
            terminal: None,
        }
    }
}

impl<H: Copy + Eq> Encounter<H> {
    pub fn bind(&mut self, handle: H, observed_alive: bool) -> bool {
        if self.handle.is_some() || self.terminal.is_some() || !observed_alive {
            return false;
        }
        self.handle = Some(handle);
        true
    }

    pub fn handle(&self) -> Option<H> {
        self.handle
    }

    pub fn poll(
        &mut self,
        player_dead: bool,
        observation: Observation,
        spawn_expired: bool,
    ) -> Outcome {
        if let Some(terminal) = self.terminal {
            return terminal;
        }
        let outcome = if player_dead {
            Outcome::PlayerDied
        } else if self.handle.is_none() {
            if spawn_expired {
                Outcome::SpawnTimedOut
            } else {
                Outcome::Waiting
            }
        } else {
            match observation {
                Observation::Alive => Outcome::Waiting,
                Observation::Dead => Outcome::Victory,
                Observation::Missing | Observation::DifferentEntity => Outcome::LostEntity,
            }
        };
        if outcome != Outcome::Waiting {
            self.terminal = Some(outcome);
        }
        outcome
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {Weak,Strong,Boss}
/// 单人同时在场的敌人上限（Boss 组除外）
pub const SOLO_CAP:usize=2;
/// 后续波次：前一波的条件满足后再等这么久才出场
pub const WAVE_GAP:std::time::Duration=std::time::Duration::from_secs(2);
/// 后续波次的敌人出场后这么久内不出招（防止贴脸突袭）；同批、第一波不受影响
pub const WAVE_GRACE:std::time::Duration=std::time::Duration::from_millis(1500);
#[derive(Clone,Debug,Default)]
pub struct SpawnGap {
    blocked:bool,
    ready_since:Option<std::time::Duration>,
    admitted:bool,
}
impl SpawnGap {
    /// 同批直接出场；曾等待前批的敌人，条件满足后再等 WAVE_GAP。放行后不再重新拦截。
    pub fn allows(&mut self,now:std::time::Duration,eligible:bool)->bool {
        if self.admitted {return true;}
        if !eligible {self.blocked=true;self.ready_since=None;return false;}
        if self.blocked&&now.saturating_sub(*self.ready_since.get_or_insert(now))<WAVE_GAP{return false;}
        self.admitted=true;true
    }
    /// 已经放行、而且是等过前批才出场的（后续波次）：出场后有 WAVE_GRACE 的出招保护期
    pub fn later_wave(&self)->bool {self.admitted&&self.blocked}
}
/// 联机与 Boss 组一起出（联机的精英战仍按苦难律法「精英成双」等前面的精英，房主与成员按同一规则各自放出预置实体）；
/// 其余单人战斗同时在场最多 SOLO_CAP 只，普通战清完弱怪后才出强怪。
/// previous 为本组中排在 index 之前的敌人状态，未生成的敌人记为 Waiting。
pub fn may_spawn(networked:bool,monster_node:bool,roles:&[Role],index:usize,previous:&[Outcome])->bool {
    may_spawn_paced(networked,monster_node,false,roles,index,previous)
}
/// 精英战里精英怪池的精英怎么上场（苦难律法「精英成双」）：一只一只（前一只倒下）、前一只半血、一起上
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Pacing {Apart,HalfHealth,Together}
impl Pacing {
    /// 这只精英要不要等：前面的精英还没上场，或（一只一只时）还活着，或（半血时）生命还在一半以上
    pub fn waits(self,earlier:&[(Outcome,Option<(i32,i32)>)])->bool {
        earlier.iter().any(|(outcome,health)|*outcome!=Outcome::Victory&&match self {
            Pacing::Together=>false,
            Pacing::Apart=>true,
            Pacing::HalfHealth=>health.is_none_or(|(hp,max)|i64::from(hp)*2>i64::from(max)),
        })
    }
}
/// held：这只精英按 Pacing::waits 需要等前面的精英（由运行时按生命算好）
pub fn may_spawn_paced(networked:bool,monster_node:bool,held:bool,roles:&[Role],index:usize,previous:&[Outcome])->bool {
    if roles.iter().all(|r|*r==Role::Boss) {return true;}
    if held&&roles[index]==Role::Strong {return false;}
    if networked {return true;}
    if previous.iter().filter(|o|**o!=Outcome::Victory).count()>=SOLO_CAP {return false;}
    !(monster_node&&roles[index]!=Role::Weak&&roles[..index].iter().zip(previous).any(|(r,o)|*r==Role::Weak&&*o!=Outcome::Victory))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_wave_waits_two_seconds_after_spawn_conditions_clear() {
        use std::time::Duration as D;
        let mut gap=SpawnGap::default();
        assert!(!gap.allows(D::ZERO,false));
        assert!(!gap.allows(D::from_secs(10),false));
        assert!(!gap.allows(D::from_secs(20),true));
        assert!(!gap.allows(D::from_millis(21999),true));assert!(!gap.later_wave());
        assert!(gap.allows(D::from_secs(22),true));assert!(gap.later_wave(),"it waited for the earlier wave");
        assert!(gap.allows(D::from_secs(25),true));
        assert!(gap.allows(D::from_secs(26),false),"an admitted enemy must not be held again while loading or fighting");
    }
    #[test]
    fn first_wave_and_same_wave_members_have_no_extra_serial_delay() {
        use std::time::Duration as D;
        let mut first=SpawnGap::default();assert!(first.allows(D::ZERO,true));assert!(!first.later_wave(),"the first wave has no grace");
        let mut second=[SpawnGap::default(),SpawnGap::default()];
        for gap in &mut second {assert!(!gap.allows(D::ZERO,false));assert!(!gap.allows(D::from_secs(5),true));}
        for gap in &mut second {assert!(gap.allows(D::from_secs(8),true));}
    }
    #[test]
    fn interrupted_spawn_eligibility_restarts_the_wave_gap() {
        use std::time::Duration as D;
        let mut gap=SpawnGap::default();
        assert!(!gap.allows(D::ZERO,false));assert!(!gap.allows(D::from_secs(1),true));
        assert!(!gap.allows(D::from_secs(2),false));assert!(!gap.allows(D::from_secs(10),true));
        assert!(!gap.allows(D::from_millis(11999),true));assert!(gap.allows(D::from_secs(12),true));
    }

    #[test]
    fn requires_a_live_bound_enemy_before_victory() {
        let mut fight = Encounter::<u64>::default();
        assert_eq!(
            fight.poll(false, Observation::Dead, false),
            Outcome::Waiting
        );
        assert!(!fight.bind(7, false));
        assert!(fight.bind(7, true));
        assert_eq!(
            fight.poll(false, Observation::Dead, false),
            Outcome::Victory
        );
    }

    #[test]
    fn missing_or_replaced_enemies_are_not_kills() {
        for observation in [Observation::Missing, Observation::DifferentEntity] {
            let mut fight = Encounter::<u64>::default();
            assert!(fight.bind(7, true));
            assert_eq!(fight.poll(false, observation, false), Outcome::LostEntity);
            assert_eq!(
                fight.poll(false, Observation::Dead, false),
                Outcome::LostEntity
            );
        }
    }

    #[test]
    fn simultaneous_death_is_not_victory() {
        let mut fight = Encounter::<u64>::default();
        fight.bind(7, true);
        assert_eq!(
            fight.poll(true, Observation::Dead, false),
            Outcome::PlayerDied
        );
    }

    #[test]
    fn late_binding_after_timeout_is_rejected() {
        let mut fight = Encounter::<u64>::default();
        assert_eq!(
            fight.poll(false, Observation::Alive, true),
            Outcome::SpawnTimedOut
        );
        assert!(!fight.bind(7, true));
    }

    #[test]
    fn binding_cannot_be_replaced_and_timeout_does_not_end_active_fight() {
        let mut fight = Encounter::<u64>::default();
        assert!(fight.bind(7, true));
        assert!(!fight.bind(8, true));
        assert_eq!(fight.handle(), Some(7));
        assert_eq!(
            fight.poll(false, Observation::Alive, true),
            Outcome::Waiting
        );
    }

    #[test]
    fn solo_waves_cap_and_boss_groups() {
        use super::{Role::*,may_spawn,Outcome::*};
        let duo=[Boss,Boss];
        assert!(may_spawn(false,false,&duo,1,&[Waiting]),"boss components spawn together");
        let monsters=[Weak,Weak,Strong,Strong];
        assert!(may_spawn(false,true,&monsters,1,&[Waiting]));
        assert!(!may_spawn(false,true,&monsters,2,&[Waiting,Waiting]),"cap of two");
        assert!(!may_spawn(false,true,&monsters,2,&[Victory,Waiting]),"strong wave waits for weak enemies");
        assert!(may_spawn(false,true,&monsters,2,&[Victory,Victory]));
        assert!(may_spawn(false,true,&monsters,3,&[Victory,Victory,Waiting]),"strong enemies arrive together");
        let elite=[Strong,Strong,Weak,Weak];
        assert!(may_spawn(false,false,&elite,1,&[Waiting]),"elite and escorts together");
        assert!(!may_spawn(false,false,&elite,2,&[Waiting,Waiting]));
        assert!(may_spawn(false,false,&elite,2,&[Victory,Waiting]),"next enemy fills a freed slot");
        // 联机：没有同时在场上限，但精英战的精英照样按「精英成双」等前一只
        let pair=[Role::Strong,Role::Strong,Role::Weak];
        assert!(may_spawn_paced(true,false,false,&pair,1,&[Waiting]),"party: no cap");
        assert!(!may_spawn_paced(true,false,true,&pair,1,&[Waiting]),"party: the second elite waits");
        assert!(may_spawn_paced(true,false,true,&pair,2,&[Waiting,Waiting]),"escorts never wait");
        assert!(may_spawn_paced(true,false,true,&[Role::Boss,Role::Boss],1,&[Waiting]),"boss groups spawn together");
        assert!(may_spawn(true,true,&monsters,3,&[Waiting,Waiting,Waiting]),"party keeps group spawning");
        // 精英战：精英怪池的两只精英一只一只上；苦难律法「精英成双」时一起上；Boss 组（精英版双人组）不受影响
        let pair=[Role::Strong,Role::Strong,Role::Weak];
        assert!(!may_spawn_paced(false,false,true,&pair,1,&[Waiting]),"a held elite waits");
        assert!(may_spawn_paced(false,false,false,&pair,1,&[Waiting]),"paired elites arrive together");
        let alive_full=[(Waiting,Some((100,100)))];let alive_half=[(Waiting,Some((50,100)))];let not_yet=[(Waiting,None)];let dead=[(Victory,Some((0,100)))];
        assert!(Pacing::Apart.waits(&alive_half)&&!Pacing::Apart.waits(&dead));
        assert!(Pacing::HalfHealth.waits(&alive_full)&&Pacing::HalfHealth.waits(&not_yet)&&!Pacing::HalfHealth.waits(&alive_half)&&!Pacing::HalfHealth.waits(&dead));
        assert!(!Pacing::Together.waits(&alive_full));
        assert!(may_spawn_paced(false,false,true,&duo,1,&[Waiting]),"boss duos are untouched");
        assert!(!may_spawn_paced(true,false,true,&pair,1,&[Waiting]),"party elites follow the same pacing");
    }
}
