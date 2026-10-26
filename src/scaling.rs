use crate::Run;

/// 本幕进度对应的递增档：首层 0，Boss 层 GROWTH_STEPS；仅新旅程启用
pub const GROWTH_STEPS:u8=11;
pub fn growth_step(run:&Run,node:usize)->u8 {
    if run.scaling_rules==0 {return 0;}
    let (Some(node),Some(last))=(run.nodes.get(node),run.nodes.iter().map(|n|n.layer).max()) else{return 0};
    if last==0 {return 0;}
    ((node.layer.min(last)*usize::from(GROWTH_STEPS)+last/2)/last) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Class,NodeKind};
    #[test]
    fn growth_rises_through_the_act_and_peaks_at_the_boss() {
        for seed in 0..50 {
            let run=Run::new(seed,Class::Samurai);
            let boss=run.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap();
            assert_eq!(growth_step(&run,boss),GROWTH_STEPS);
            for id in run.start_nodes() {assert_eq!(growth_step(&run,id),0);}
            for (a,b) in run.nodes.iter().enumerate().flat_map(|(a,n)|n.next.iter().map(move|b|(a,*b))) {assert!(growth_step(&run,a)<=growth_step(&run,b));}
        }
    }
    #[test]
    fn old_runs_keep_flat_enemy_strength() {
        let mut run=Run::new(3,Class::Samurai);let boss=run.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap();
        let mut json:serde_json::Value=serde_json::from_str(&run.encode().unwrap()).unwrap();json.as_object_mut().unwrap().remove("scaling_rules");
        let old=Run::decode(&json.to_string()).unwrap();assert_eq!(growth_step(&old,boss),0);
        run.scaling_rules=0;assert_eq!(growth_step(&run,boss),0);
    }
}
