use std::time::Duration;

#[derive(Default)]
pub struct Rescue {
    clock: Option<std::time::Instant>,
    falling_since: Option<Duration>,
    requested: Option<Duration>,
    attempts: u8,
}

#[derive(Debug, PartialEq)]
pub enum Step { Wait, Request, Exhausted }

impl Rescue {
    pub fn tick(&mut self, falling:bool)->Step {
        let now=self.clock.get_or_insert_with(std::time::Instant::now).elapsed();
        self.poll(now,falling)
    }
    pub fn poll(&mut self, now: Duration, falling: bool) -> Step {
        if !falling { self.falling_since=None; return Step::Wait; }
        let since=*self.falling_since.get_or_insert(now);
        if now.saturating_sub(since)<Duration::from_secs(1)
            ||self.requested.is_some_and(|at|now.saturating_sub(at)<Duration::from_secs(15)) {
            return Step::Wait;
        }
        if self.attempts>=3 { return Step::Exhausted; }
        self.attempts+=1;
        self.requested=Some(now);
        Step::Request
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn t(s:u64)->Duration {Duration::from_secs(s)}
    #[test]
    fn void_respawn_is_rescued_before_the_next_fall_death() {
        let mut r=Rescue::default();
        assert_eq!(r.poll(t(0),false),Step::Wait);
        assert_eq!(r.poll(t(2),true),Step::Wait);
        assert_eq!(r.poll(t(3),true),Step::Request);
        assert_eq!(r.poll(t(4),true),Step::Wait);
    }
    #[test]
    fn loading_or_brief_fall_does_not_request_a_warp() {
        let mut r=Rescue::default();
        assert_eq!(r.poll(t(0),true),Step::Wait);
        assert_eq!(r.poll(t(1),false),Step::Wait);
        assert_eq!(r.poll(t(2),true),Step::Wait);
        assert_eq!(r.poll(t(3),false),Step::Wait);
    }
    #[test]
    fn failed_rescue_is_bounded_and_survives_loading_gaps() {
        let mut r=Rescue::default();
        r.poll(t(0),true);
        assert_eq!(r.poll(t(1),true),Step::Request);
        r.poll(t(2),false);
        r.poll(t(3),true);
        assert_eq!(r.poll(t(15),true),Step::Wait);
        assert_eq!(r.poll(t(16),true),Step::Request);
        assert_eq!(r.poll(t(31),true),Step::Request);
        assert_eq!(r.poll(t(46),true),Step::Exhausted);
    }
}
