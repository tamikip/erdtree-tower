use serde::{Deserialize,Serialize};
use crate::{Run,NodeKind,RuleResult,Reward};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum Progress {Choosing,Running,Complete,Skipped}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum Decision {Challenge,Skip,Engage,Complete}

impl Run {
    pub fn stormveil_decide(&mut self,decision:Decision)->RuleResult<()> {
        let node=self.active_node(NodeKind::Event)?;
        match (self.regional_event.stormveil,decision) {
            (Some(Progress::Choosing),Decision::Challenge)=>self.regional_event.stormveil=Some(Progress::Running),
            (Some(Progress::Choosing),Decision::Skip)=>{
                self.gold=self.gold.checked_add(800).ok_or("卢恩数量超限")?;
                self.regional_event.stormveil=Some(Progress::Skipped);
            },
            (Some(Progress::Running),Decision::Complete)=>{
                if self.regional_event.chariot {
                    self.regional_event.stormveil=Some(Progress::Complete);
                    return Ok(());
                }
                let mut pool:Vec<_>=crate::elite::BUFFS.iter().copied().filter(|id|!self.elite_buffs.contains(id)).collect();
                for slot in 0..2 {
                    if pool.is_empty(){break;}
                    let salt=if self.regional_event.chariot{0x3009_0000}else{0x570_0000};
                    let index=crate::mix(self.reward_seed()^node as u64^salt^slot) as usize%pool.len();
                    self.grant(Reward::TalismanBuff(pool.remove(index)))?;
                }
                self.regional_event.stormveil=Some(Progress::Complete);
            },
            _=>return Err("跑城事件状态不匹配或已经结算".into()),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Phase;
    fn event()->Run {
        let mut r=Run::new(42,crate::Class::Samurai);
        let id=r.nodes.iter().position(|n|n.kind==NodeKind::Event).unwrap();
        r.phase=Phase::Node{id};r.regional_event.stormveil=Some(Progress::Choosing);r
    }
    #[test]
    fn chariot_trial_persists_and_cannot_repeat_rewards() {
        let mut r=event();r.regional_event.chariot=true;
        assert!(r.valid_regional_event());
        r.stormveil_decide(Decision::Challenge).unwrap();
        let mut r:Run=serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert!(r.valid_regional_event());
        assert!(r.regional_event.chariot);
        assert!(r.leave_regional_event().is_err());
        r.stormveil_decide(Decision::Complete).unwrap();
        assert!(r.elite_buffs.is_empty());
        assert!(r.stormveil_decide(Decision::Complete).is_err());
        assert!(r.leave_regional_event().is_err());
        let item=crate::regional_events::descent_reward(r.seed,0);
        r.begin_chariot_reward(item,0).unwrap();
        assert!(r.valid_regional_event());
        assert!(r.begin_chariot_reward(item,0).is_err());
        r=serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        r.complete_chariot_reward().unwrap();
        assert!(r.valid_regional_event());
        assert!(r.begin_chariot_reward(item,1).is_err());
        r.leave_regional_event().unwrap();
        assert!(!r.regional_event.chariot);
        r.regional_event.chariot=true;assert!(!r.valid_regional_event());
        let mut r=event();r.regional_event.chariot=true;
        let gold=r.gold;r.stormveil_decide(Decision::Skip).unwrap();
        assert_eq!(r.gold,gold+800);assert!(r.elite_buffs.is_empty());
    }
    #[test]
    fn skip_grants_exactly_800_once_and_challenge_awards_two_distinct_buffs() {
        let mut r=event();let gold=r.gold;
        assert!(r.leave_regional_event().is_err());
        assert!(r.stormveil_decide(Decision::Complete).is_err());
        r.stormveil_decide(Decision::Skip).unwrap();assert_eq!(r.gold,gold+800);
        assert!(r.stormveil_decide(Decision::Skip).is_err());
        assert!(r.stormveil_decide(Decision::Challenge).is_err());
        r.leave_regional_event().unwrap();assert_eq!(r.phase,Phase::Map);
        let mut r=event();r.elite_buffs.push(crate::elite::BUFFS[0]);
        r.stormveil_decide(Decision::Challenge).unwrap();assert!(r.leave_regional_event().is_err());
        r=serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert!(r.valid_regional_event());r.stormveil_decide(Decision::Complete).unwrap();
        assert_eq!(r.elite_buffs.len(),3);assert_eq!(r.gold,gold);
        assert!(r.stormveil_decide(Decision::Complete).is_err());
        r.leave_regional_event().unwrap();assert!(r.regional_event.stormveil.is_none());
    }
}
