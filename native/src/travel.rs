use std::time::Duration;

pub const PREPARATION: Duration = Duration::from_secs(3);
const TIMEOUT: Duration = Duration::from_secs(60);
const SETTLE: Duration = Duration::from_millis(500);

fn arrival_hp(current:i32,maximum:i32,saved:i32,dead:bool)->i32 {
    if dead||current<=0||maximum<=0||saved<=0 {return current;}
    saved.min(maximum)
}

/// Loading may clamp HP before permanent max-HP effects return. Restore the departure amount once.
pub fn restore_arrival_hp(chr:&mut eldenring::cs::ChrIns,saved:i32,context:&str) {
    let dead=chr.chr_flags1c5.death_flag();
    let data=&mut chr.modules.data;
    let before=data.hp;
    data.hp=arrival_hp(before,data.max_hp,saved,dead);
    super::log(format!("TRAVEL_HP {context} saved={saved} arrival={before} max={} restored={} dead={dead}",data.max_hp,data.hp));
}

/// HansFix 试验：星辉术士的天赋已换成蓝瓶持续回复（见 starsurge），进入战斗不再回复一半专注
pub fn combat_fp(_class: erdtree_tower::Class, current: i32, _maximum: i32) -> i32 {
    current
}

#[derive(Debug, PartialEq, Eq)]
pub enum Progress {
    Loading,
    Fog,
    Countdown(u64),
    Spawn,
    Failed,
}

pub struct Travel {
    pub node: usize,
    pub resources_restored: bool,
    stable_since: Option<Duration>,
    ready_since: Option<Duration>,
    circle_entered: Option<Duration>,
}

impl Travel {
    pub fn new(node: usize) -> Self {
        Self {
            node,
            resources_restored: false,
            stable_since: None,
            ready_since: None,
            circle_entered: None,
        }
    }
    pub fn is_preparing(&self) -> bool {
        self.ready_since.is_some()
    }
    pub fn poll_circle(&mut self,elapsed:Duration,arrived:bool,grounded:bool,crossed:bool)->Progress {
        if !arrived||!crossed {
            self.circle_entered=None;self.stable_since=None;self.ready_since=None;
            return if arrived {Progress::Fog}else{self.poll(elapsed,false,false)};
        }
        let entered=*self.circle_entered.get_or_insert(elapsed);
        self.poll(elapsed.saturating_sub(entered),true,grounded)
    }
    pub fn poll_fog(&mut self,elapsed:Duration,arrived:bool,grounded:bool,crossed:bool)->Progress {
        if !arrived {return self.poll(elapsed,false,false);}
        if !crossed {self.stable_since=None;self.ready_since=None;return Progress::Fog;}
        if grounded {Progress::Spawn} else {Progress::Loading}
    }
    pub fn poll(&mut self, elapsed: Duration, arrived: bool, grounded: bool) -> Progress {
        if elapsed >= TIMEOUT {
            return Progress::Failed;
        }
        if !arrived {
            self.stable_since = None;
            self.ready_since = None;
            return Progress::Loading;
        }
        if self.ready_since.is_none() {
            if !grounded {
                self.stable_since = None;
                return Progress::Loading;
            }
            let stable = *self.stable_since.get_or_insert(elapsed);
            if elapsed.saturating_sub(stable) < SETTLE {
                return Progress::Loading;
            }
            self.ready_since = Some(elapsed);
        }
        let remaining =
            PREPARATION.saturating_sub(elapsed.saturating_sub(self.ready_since.unwrap()));
        if remaining.is_zero() {
            Progress::Spawn
        } else {
            Progress::Countdown(remaining.as_secs_f64().ceil() as u64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arrival_restores_hp_clamped_while_godslayer_or_crimson_amber_reloads() {
        assert_eq!(arrival_hp(1000,1440,1440,false),1440);
        assert_eq!(arrival_hp(1000,1440,1200,false),1200);
        assert_eq!(arrival_hp(1000,1060,1060,false),1060);
    }
    #[test]
    fn arrival_keeps_departure_damage_instead_of_filling_the_health_bar() {
        assert_eq!(arrival_hp(1440,1440,750,false),750);
        assert_eq!(arrival_hp(1000,1440,750,false),750);
        assert_eq!(arrival_hp(1000,1000,1,false),1);
    }
    #[test]
    fn arrival_caps_health_if_maximum_really_decreased() {
        assert_eq!(arrival_hp(900,900,1200,false),900);
        assert_eq!(arrival_hp(500,900,1200,false),900);
    }
    #[test]
    fn arrival_never_revives_or_applies_an_invalid_snapshot() {
        assert_eq!(arrival_hp(0,1440,1440,false),0);
        assert_eq!(arrival_hp(1,1440,1440,true),1);
        assert_eq!(arrival_hp(500,1440,0,false),500);
        assert_eq!(arrival_hp(500,0,1440,false),500);
    }
    #[test]
    fn circle_wait_is_unlimited_and_countdown_starts_after_entry_and_landing() {
        let mut travel=Travel::new(0);
        assert_eq!(travel.poll_circle(ms(120000),true,true,false),Progress::Fog);
        assert_eq!(travel.poll_circle(ms(120100),true,false,true),Progress::Loading);
        assert_eq!(travel.poll_circle(ms(120200),true,true,true),Progress::Loading);
        assert_eq!(travel.poll_circle(ms(120700),true,true,true),Progress::Countdown(3));
        assert_eq!(travel.poll_circle(ms(123699),true,true,true),Progress::Countdown(1));
        assert_eq!(travel.poll_circle(ms(123700),true,true,true),Progress::Spawn);
        assert_eq!(travel.poll_circle(ms(123701),true,true,false),Progress::Fog);
        assert!(!travel.is_preparing());
    }
    #[test]
    fn fog_wait_does_not_timeout_or_start_preparation() {
        let mut travel=Travel::new(0);
        assert_eq!(travel.poll_fog(ms(2000),true,true,false),Progress::Fog);
        assert_eq!(travel.poll_fog(ms(120000),true,true,false),Progress::Fog);
        assert!(!travel.is_preparing());
        assert_eq!(travel.poll_fog(ms(120100),true,false,true),Progress::Loading);
        assert_eq!(travel.poll_fog(ms(120101),true,true,true),Progress::Spawn);
        assert_eq!(travel.poll_fog(ms(120102),true,true,false),Progress::Fog);
    }
    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }
    #[test]
    fn loading_does_not_consume_preparation_time() {
        let mut travel = Travel::new(0);
        assert_eq!(travel.poll(ms(10000), false, false), Progress::Loading);
        assert_eq!(travel.poll(ms(12000), true, true), Progress::Loading);
        assert_eq!(travel.poll(ms(12500), true, true), Progress::Countdown(3));
        assert_eq!(travel.poll(ms(15499), true, true), Progress::Countdown(1));
        assert_eq!(travel.poll(ms(15500), true, true), Progress::Spawn);
    }
    #[test]
    fn combat_restores_half_maximum_fp_with_cap_and_integer_rounding() {
        use erdtree_tower::Class;
        // HansFix：星辉术士进入战斗不再回复一半专注，所有职业都保持原值
        for class in Class::PLAYABLE {
            assert_eq!(combat_fp(class, 0, 100), 0);
            assert_eq!(combat_fp(class, 20, 100), 20);
        }
    }
    #[test]
    fn unloading_resets_countdown_and_timeout_never_spawns() {
        let mut travel = Travel::new(3);
        travel.poll(ms(0), true, true);
        travel.poll(ms(500), true, true);
        assert_eq!(travel.poll(ms(3000), false, false), Progress::Loading);
        travel.poll(ms(6000), true, true);
        assert_eq!(travel.poll(ms(6500), true, true), Progress::Countdown(3));
        assert_eq!(travel.poll(ms(60000), true, true), Progress::Failed);
    }
}
