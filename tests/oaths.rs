// 誓约重做：整局保留、三选一深化、史诗档适配装备；旧规则存档保持原行为
use erdtree_tower::{Class,Run,NodeKind,contracts::{Card,State,Kind,Oath,MAX_LEVEL}};

fn finish_act(run:&mut Run) {
    for _ in 0..run.floors() {
        let id=run.available()[0];run.enter(id).unwrap();
        match run.nodes[id].kind {
            NodeKind::Monster|NodeKind::Elite|NodeKind::Boss=>{run.win_combat().unwrap();run.decline_reward().unwrap();},
            NodeKind::Camp=>run.leave_camp().unwrap(),NodeKind::Shop=>run.leave_shop().unwrap(),
            NodeKind::Chest=>run.finish_chest().unwrap(),NodeKind::Event=>{run.resolve_event(false).unwrap();},
        }
    }
}

#[test]
fn opening_has_three_unique_choices_and_third_choice_roundtrips() {
    for seed in 0..64 {
        let mut state=State::start(seed,Class::Samurai,1,vec![],0);
        assert_eq!(state.options(),3);
        assert!(state.valid(1));
        assert!(!state.offers.contains(&Kind::Hunter));
        assert_eq!(state,State::start(seed,Class::Samurai,1,vec![],0));
        let third=state.offers[2];
        state.choose(2,seed,Class::Samurai).unwrap();
        assert_eq!(state.held,vec![Oath{kind:third,level:1}]);
        assert!(state.valid(1));
        let restored:State=serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(restored,state);
        assert!(state.choose(0,seed,Class::Samurai).is_err());
    }
}

#[test]
fn old_two_choice_saves_remain_valid_and_malformed_offers_are_rejected() {
    let mut state=State::start(31,Class::Samurai,1,vec![],0);
    state.offers.truncate(2);
    let old:State=serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
    assert!(old.valid(1));assert_eq!(old.options(),2);
    state.offers.push(state.offers[0]);assert!(!state.valid(1));
    state.offers.clear();assert!(!state.valid(1));
}

#[test]
fn oaths_persist_and_can_be_deepened_to_level_three() {
    let mut run=Run::new(31,Class::Samurai);run.enable_contracts();
    let state=run.contract.as_ref().unwrap();
    assert_eq!((state.rules,state.deepen),(1,None),"no deepen card before holding an oath");
    assert!(matches!(state.cards()[..],[Card::Oath(_),Card::Oath(_),Card::Oath(_)]),"first act offers three oaths");
    let seed=run.reward_seed();
    let state=run.contract.as_mut().unwrap();state.choose(0,seed,run.class).unwrap();let first=state.chosen.unwrap();
    assert_eq!(state.held,vec![Oath{kind:first,level:1}]);
    state.skip_gift().unwrap();
    finish_act(&mut run);
    let state=run.contract.as_ref().unwrap();
    assert_eq!(state.act,2);assert_eq!(state.held,vec![Oath{kind:first,level:1}],"oaths survive the act change");
    assert_eq!(state.deepen,Some(first));assert!(!state.offers.contains(&first));
    assert!(matches!(state.cards()[..],[Card::Oath(_),Card::Oath(_),Card::Deepen(k)] if k==first),"later acts add a deepen card");
    let seed=run.reward_seed();
    let state=run.contract.as_mut().unwrap();state.choose(2,seed,run.class).unwrap();
    assert!(state.deepened&&state.gear==[0;3]);assert!(state.begin_gift(0).is_err()&&state.skip_gift().is_err());
    assert!(run.contract_pending());
    assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
    run.contract.as_mut().unwrap().finish_deepen().unwrap();assert!(!run.contract_pending());
    assert_eq!(run.contract.as_ref().unwrap().level(first),Some(2));
    finish_act(&mut run);
    let seed=run.reward_seed();
    let state=run.contract.as_mut().unwrap();assert_eq!(state.deepen,Some(first));
    state.choose(2,seed,run.class).unwrap();state.finish_deepen().unwrap();
    assert_eq!(state.level(first),Some(MAX_LEVEL));
    assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
    // 满级后不再提供深化卡
    let maxed=State::start(1,Class::Samurai,3,vec![Oath{kind:first,level:3}],0);assert_eq!(maxed.deepen,None);
    let mut broken=run.clone();broken.contract.as_mut().unwrap().held[0].level=4;assert!(broken.validate().is_err());
}

#[test]
fn three_distinct_oaths_can_be_held_and_the_deepen_card_prefers_the_strongest() {
    let mut run=Run::new(77,Class::Astrologer);run.enable_contracts();
    let mut kinds=vec![];
    for act in 1..=3 {
        let seed=run.reward_seed();let state=run.contract.as_mut().unwrap();
        assert!(state.offers.iter().all(|k|!kinds.contains(k)));
        state.choose(1,seed,run.class).unwrap();kinds.push(state.chosen.unwrap());state.skip_gift().unwrap();
        assert_eq!(state.active().len(),act as usize);
        if act<3 {finish_act(&mut run);}
    }
    let held=vec![Oath{kind:Kind::Steel,level:1},Oath{kind:Kind::Blood,level:2},Oath{kind:Kind::Guard,level:1}];
    assert_eq!(State::start(5,Class::Samurai,3,held,0).deepen,Some(Kind::Blood));
}

#[test]
fn new_gifts_are_epic_and_old_saves_keep_the_old_rules() {
    let tiers:std::collections::HashMap<u32,u8>=include_str!("../scene-assets/reward-rarities.tsv").lines().map(|l| {
        let (id,tier)=l.split_once('\t').unwrap();(id.parse().unwrap(),tier.trim().parse().unwrap())
    }).collect();
    for kind in Kind::ALL {for seed in 0..64 {
        let mut s=State::start(seed,Class::Prophet,1,vec![],0);s.offers[0]=kind;s.offers[1]=*Kind::ALL.iter().find(|k|**k!=kind).unwrap();
        s.offers[2]=*Kind::ALL.iter().find(|k|!s.offers[..2].contains(k)).unwrap();
        s.choose(0,seed,Class::Prophet).unwrap();assert!(s.valid(1));
        assert!(s.gear.iter().all(|id|tiers[id]==2),"{kind:?} gift must be epic");
        if kind==Kind::Guard {assert!(matches!(s.gear[0]/1_000_000,31|32));}
    }}
    // 旧存档：没有 rules 字段，下一幕照旧换一份新契约
    let mut run=Run::new(12,Class::Samurai);
    run.contract=Some(State::new(run.reward_seed(),run.class,1,None,0));
    let seed=run.reward_seed();run.contract.as_mut().unwrap().choose(0,seed,run.class).unwrap();
    let old=run.contract.as_ref().unwrap();assert_eq!(old.rules,0);assert!(old.held.is_empty());
    assert!(old.gear.iter().all(|id|tiers[id]<=1));
    let mut json:serde_json::Value=serde_json::from_str(&run.encode().unwrap()).unwrap();
    for key in ["rules","held","deepen","deepened"] {json["contract"].as_object_mut().unwrap().remove(key);}
    let mut legacy=Run::decode(&json.to_string()).unwrap();assert_eq!(legacy.contract.as_ref().unwrap().rules,0);
    let chosen=legacy.contract.as_ref().unwrap().chosen;
    legacy.contract.as_mut().unwrap().skip_gift().unwrap();finish_act(&mut legacy);
    let next=legacy.contract.as_ref().unwrap();
    assert_eq!(next.rules,0);assert!(next.chosen.is_none()&&next.active().is_empty()&&!next.offers.contains(&chosen.unwrap()));
}

#[test]
fn level_text_matches_the_planned_numbers() {
    assert_eq!(Kind::Steel.boon_at(1),"蓄力重攻击伤害 +35%");assert_eq!(Kind::Steel.boon_at(3),"蓄力重攻击伤害 +55%");
    assert_eq!(Kind::Mystic.boon_at(2),"魔法／祷告伤害 +32%");
    assert_eq!(Kind::Blood.boon_at(3),"生命低于 50%\n伤害 +55%");
    assert_eq!(Kind::Feather.boon_at(3),"轻负重时伤害 +30%\n精力恢复 +7 点/秒");
    assert_eq!(Kind::Ascetic.boon_at(3),"胜利恢复最大生命 20%\n最大专注 26%");
    assert_eq!(Kind::Ascetic.boon_at(1),"胜利恢复最大生命 15%\n最大专注 20%");
    assert_eq!((Kind::Combo.bane_at(),Kind::Guard.bane_at(),Kind::Steel.bane_at()),("受到伤害 +5%","动作精力消耗 +10%","战技伤害 -20%"));
}

