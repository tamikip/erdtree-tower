use serde::{Deserialize,Serialize};
use crate::{Phase,Run,RuleResult};

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Point {pub seed:u64,pub initial_seed:u64,pub act:u8,pub route:u64,pub visited:Vec<usize>,pub retry:Option<usize>,pub law:String}
impl Point {
    pub fn of(run:&Run)->Option<Self> {
        if run.phase!=Phase::Map||run.contract_pending(){return None;}
        let route=serde_json::to_vec(&run.nodes).ok()?.into_iter().fold(0xcbf29ce484222325u64,|h,b|(h^u64::from(b)).wrapping_mul(0x100000001b3));
        Some(Self{seed:run.seed,initial_seed:run.initial_seed.unwrap_or(run.seed),act:run.act,route,visited:run.visited.clone(),retry:run.retry_node,law:run.law.code()})
    }
}
pub fn prepare(run:&Run,gold:u32,flasks:Option<u8>)->RuleResult<Run> {
    if matches!(run.phase,Phase::Dead|Phase::Won){return Err("已结束的旅程不能暂存".into());}
    let mut next=run.clone();
    if let Phase::Combat{id}=next.phase {next.resume_from_hub()?;next.retry_node=Some(id);}
    next.gold=gold;
    if let Some(flasks)=flasks {next.flasks=flasks.min(next.flask_limit);}
    next.validate()?;Ok(next)
}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {pub version:u8,pub owner:(u32,i32),pub run:String}
impl Snapshot {
    pub fn encode(owner:(u32,i32),run:&Run)->RuleResult<String> {
        run.validate()?;
        serde_json::to_string(&Self{version:1,owner,run:run.encode()?}).map_err(|e|e.to_string())
    }
    pub fn decode(text:&str,owner:(u32,i32))->RuleResult<Run> {
        if text.len()>256*1024 {return Err("暂存记录过大".into());}
        let saved:Self=serde_json::from_str(text).map_err(|e|format!("暂存记录损坏：{e}"))?;
        if saved.version!=1||saved.owner!=owner {return Err("暂存记录版本或角色不匹配".into());}
        let run=Run::decode(&saved.run)?;
        if matches!(run.phase,Phase::Combat{..}|Phase::Dead|Phase::Won){return Err("暂存记录不是可恢复状态".into());}
        Ok(run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combat_restarts_same_room_without_restoring_spent_currency_or_flasks() {
        let mut run=Run::new(42,crate::Class::Samurai);let id=run.available()[0];run.enter(id).unwrap();
        let saved=prepare(&run,123,Some(1)).unwrap();let loaded=Snapshot::decode(&Snapshot::encode((7,0),&saved).unwrap(),(7,0)).unwrap();
        assert_eq!(loaded.phase,Phase::Map);assert_eq!(loaded.available(),vec![id]);
        assert_eq!(loaded.gold,123);assert_eq!(loaded.flasks,1);assert_eq!(loaded.visited,run.visited);
    }
    #[test]
    fn pending_rewards_and_receipts_survive_without_duplicate_claims() {
        let mut run=Run::new(42,crate::Class::Samurai);let id=run.available()[0];run.enter(id).unwrap();run.win_combat().unwrap();
        let saved=prepare(&run,run.gold,None).unwrap();assert_eq!(saved.phase,run.phase);
        let mut loaded=Snapshot::decode(&Snapshot::encode((7,0),&saved).unwrap(),(7,0)).unwrap();
        loaded.claim_reward(0).unwrap();let saved=prepare(&loaded,loaded.gold,None).unwrap();
        let mut again=Snapshot::decode(&Snapshot::encode((7,0),&saved).unwrap(),(7,0)).unwrap();
        assert!(again.claim_reward(0).is_err());assert_eq!(again.claimed_items,loaded.claimed_items);
    }
    #[test]
    fn owner_corruption_and_progress_mismatch_are_rejected() {
        let run=Run::new(42,crate::Class::Samurai);let text=Snapshot::encode((7,0),&run).unwrap();
        assert!(Snapshot::decode(&text,(8,0)).is_err());assert!(Snapshot::decode(&text[..text.len()/2],(7,0)).is_err());
        let other=Run::new(43,crate::Class::Samurai);assert_ne!(Point::of(&run),Point::of(&other));
        let mut changed_route=run.clone();changed_route.nodes[0].next.clear();assert_ne!(Point::of(&run),Point::of(&changed_route));
        let mut ended=run.clone();ended.end_on_death();assert!(prepare(&ended,0,None).is_err());
    }
}
