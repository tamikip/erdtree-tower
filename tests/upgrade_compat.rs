//! 跨多个版本更新：一个在今天这批改动之前保存、正停在奖励页的旧存档（没有骨灰等级记录、余烬品质规则、强化石奖励这几项），
//! 新版要能直接读进来，奖励页保持原样，之后照旧规则继续；新开的旅程才用新规则。
use erdtree_tower::{Class,Phase,Reward,Run};

fn legacy_json(run:&Run)->String {
    let mut json=serde_json::to_value(run).unwrap();
    let map=json.as_object_mut().unwrap();
    for field in ["spirit_peak","spirit_rules","ember_rules","stone_rewards"] {map.remove(field);}
    json.to_string()
}

#[test]
fn saves_from_several_versions_back_load_with_their_reward_page_unchanged() {
    for seed in 0..300 {
        // 旧版的旅程：这几项都是旧值
        let mut old=Run::new(seed,Class::Samurai);
        old.spirit_peak=None;old.spirit_rules=0;old.ember_rules=0;old.stone_rewards=false;
        let node=old.start_nodes()[0];old.enter(node).unwrap();old.win_combat().unwrap();
        let Phase::Reward{ref offers,..}=old.phase else {panic!()};
        assert!(!offers.contains(&Reward::UpgradeStone));
        let loaded=Run::decode(&legacy_json(&old)).unwrap_or_else(|e|panic!("seed {seed}: {e}"));
        assert_eq!(loaded.phase,old.phase,"the pending reward page is kept as saved");
        assert_eq!((loaded.spirit_peak,loaded.spirit_rules,loaded.ember_rules,loaded.stone_rewards),(None,0,0,false),"old saves keep the old rules");
        let mut next=loaded.clone();next.claim_reward(0).unwrap();
        assert_eq!(Run::decode(&next.encode().unwrap()).unwrap(),next,"and keep saving and loading afterwards");
    }
    let fresh=Run::new(1,Class::Samurai);
    assert_eq!((fresh.spirit_peak,fresh.spirit_rules,fresh.ember_rules,fresh.stone_rewards),(Some(0),1,3,true),"new runs use the new rules");
    // 上一版（有骨灰记录、规则 0）停在奖励页的存档也照常读取
    let mut previous=Run::new(5,Class::Samurai);previous.spirit_rules=0;
    let node=previous.start_nodes()[0];previous.enter(node).unwrap();previous.win_combat().unwrap();
    let mut json=serde_json::to_value(&previous).unwrap();json.as_object_mut().unwrap().remove("spirit_rules");
    assert_eq!(Run::decode(&json.to_string()).unwrap().phase,previous.phase);
}
