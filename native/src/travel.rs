use std::time::Duration;

pub const PREPARATION: Duration = Duration::from_secs(5);
const TIMEOUT: Duration = Duration::from_secs(60);
const SETTLE: Duration = Duration::from_millis(500);

pub fn combat_fp(class: erdtree_tower::Class, current: i32, maximum: i32) -> i32 {
    if class != erdtree_tower::Class::Astrologer { return current; }
    let maximum = maximum.max(0);
    current.max(0).saturating_add(maximum / 2).min(maximum)
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
}

impl Travel {
    pub fn new(node: usize) -> Self {
        Self {
            node,
            resources_restored: false,
            stable_since: None,
            ready_since: None,
        }
    }
    pub fn is_preparing(&self) -> bool {
        self.ready_since.is_some()
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
        assert_eq!(travel.poll(ms(12500), true, true), Progress::Countdown(5));
        assert_eq!(travel.poll(ms(17499), true, true), Progress::Countdown(1));
        assert_eq!(travel.poll(ms(17500), true, true), Progress::Spawn);
    }
    #[test]
    fn combat_restores_half_maximum_fp_with_cap_and_integer_rounding() {
        use erdtree_tower::Class;
        assert_eq!(combat_fp(Class::Astrologer, 0, 100), 50);
        assert_eq!(combat_fp(Class::Astrologer, 20, 100), 70);
        assert_eq!(combat_fp(Class::Astrologer, 80, 100), 100);
        assert_eq!(combat_fp(Class::Astrologer, 100, 100), 100);
        assert_eq!(combat_fp(Class::Astrologer, 10, 95), 57);
        assert_eq!(combat_fp(Class::Astrologer, 0, 0), 0);
        assert_eq!(combat_fp(Class::Astrologer, i32::MAX, i32::MAX), i32::MAX);
        for class in Class::ALL {
            if class != Class::Astrologer {
                assert_eq!(combat_fp(class, 0, 100), 0);
                assert_eq!(combat_fp(class, 20, 100), 20);
            }
        }
    }
    #[test]
    fn unloading_resets_countdown_and_timeout_never_spawns() {
        let mut travel = Travel::new(3);
        travel.poll(ms(0), true, true);
        travel.poll(ms(500), true, true);
        assert_eq!(travel.poll(ms(3000), false, false), Progress::Loading);
        travel.poll(ms(6000), true, true);
        assert_eq!(travel.poll(ms(6500), true, true), Progress::Countdown(5));
        assert_eq!(travel.poll(ms(60000), true, true), Progress::Failed);
    }
}
