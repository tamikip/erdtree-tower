use std::time::Duration;
use erdtree_tower::NodeKind;

pub struct Check {
    pub solo:bool,pub alive:bool,pub idle:bool,pub preparing:bool,
    pub kind:Option<NodeKind>,pub elapsed:Duration,
}
impl Check {
    pub fn allowed(&self)->Result<(),&'static str> {
        if !self.solo {return Err("联机中不执行本地重传，避免队伍入场状态不同步。");}
        if !self.alive||!self.idle||self.preparing {return Err("请等待当前战斗、领奖、复活或传送完成。");}
        if !self.kind.is_some_and(NodeKind::combat) {return Err("入场自救仅适用于等待进入战斗场地。");}
        if self.elapsed<Duration::from_secs(15) {return Err("请先等待场地加载至少 15 秒，再尝试入场自救。");}
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn valid()->Check {Check{solo:true,alive:true,idle:true,preparing:false,kind:Some(NodeKind::Monster),elapsed:Duration::from_secs(15)}}
    #[test] fn recovery_never_retargets_service_rooms_or_missing_travel(){
        for kind in [None,Some(NodeKind::Shop),Some(NodeKind::Camp),Some(NodeKind::Event),Some(NodeKind::Chest)] {assert!(Check{kind,..valid()}.allowed().is_err());}
        for kind in [NodeKind::Monster,NodeKind::Elite,NodeKind::Boss] {assert!(Check{kind:Some(kind),..valid()}.allowed().is_ok());}
    }
    #[test] fn recovery_rejects_party_death_rewards_preparation_and_repeated_requests(){
        for check in [Check{solo:false,..valid()},Check{alive:false,..valid()},Check{idle:false,..valid()},Check{preparing:true,..valid()},Check{elapsed:Duration::from_secs(14),..valid()}] {assert!(check.allowed().is_err());}
    }
}
