use erdtree_tower::{Class,Run,Reward,Phase,NodeKind};

#[test]
fn old_flask_limits_migrate_without_free_refills_or_lost_bonuses() {
    for class in Class::ALL {for bonus in 0..=4 {
        let mut run=Run::new(42,class);run.flask_bonus=bonus;run.flask_limit=10+bonus*2;run.flasks=2;
        let mut json=serde_json::to_value(&run).unwrap();json["flask_limit"]=(5+bonus*2).into();
        let restored=Run::decode(&json.to_string()).unwrap();
        assert_eq!(restored.flask_limit,10+bonus*2);assert_eq!(restored.flasks,2);
        assert_eq!(restored.flask_bonus,bonus);
        json["flasks"]=(6+bonus*2).into();assert!(Run::decode(&json.to_string()).is_err());
    }}
}

#[test]
fn capped_growth_is_removed_and_elite_boss_pools_are_unchanged() {
    let mut flasks=0;let mut memory=0;
    for seed in 0..1000 {
        let mut run=Run::new(seed,Class::Samurai);
        let offered=erdtree_tower::rewards::offers(&run,0);
        flasks+=usize::from(offered.contains(&Reward::FlaskCapacity));
        memory+=usize::from(offered.contains(&Reward::MemorySlot));
        run.flask_bonus=4;run.flask_limit=18;run.memory_bonus=6;
        assert!(erdtree_tower::rewards::offers(&run,0).iter().all(|r|!matches!(r,Reward::FlaskCapacity|Reward::MemorySlot)));
        for (id,node) in run.nodes.iter().enumerate().filter(|(_,n)|matches!(n.kind,NodeKind::Elite|NodeKind::Boss)) {
            let expected=if node.kind==NodeKind::Boss{erdtree_tower::elite::boss_offers(&run,id)}else{erdtree_tower::elite::offers(&run,id)};
            let normalized:Vec<_>=erdtree_tower::rewards::offers(&run,id).into_iter().map(|r|match r {Reward::Item(i)=>Reward::Item(erdtree_tower::affixes::base(i)),_=>r}).collect();
            assert_eq!(normalized,expected);
        }
    }
    assert!((60..140).contains(&flasks)&&(60..140).contains(&memory));
    let mut run=Run::new(1,Class::Samurai);run.flask_bonus=5;
    assert!(run.validate().is_err());run.flask_bonus=0;run.memory_bonus=7;
    assert!(run.validate().is_err());
}

#[test]
fn growth_rewards_are_claimed_once_persist_and_do_not_refill() {
    for reward in [Reward::FlaskCapacity,Reward::MemorySlot] {
        let mut run=(0..500).find_map(|seed| {
            let mut r=Run::new(seed,Class::Astrologer);r.enter(0).unwrap();r.win_combat().unwrap();
            matches!(&r.phase,Phase::Reward{offers,..} if offers.contains(&reward)).then_some(r)
        }).unwrap();
        run.flasks=1;
        let Phase::Reward{ref offers,..}=run.phase else{unreachable!()};
        let index=offers.iter().position(|r|r==&reward).unwrap();
        run.claim_reward(index).unwrap();
        assert_eq!(run.flasks,1);
        assert_eq!(run.flask_limit,if reward==Reward::FlaskCapacity{12}else{10});
        assert_eq!(run.memory_bonus,u8::from(reward==Reward::MemorySlot));
        assert!(run.claim_reward(index).is_err());
        assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
        let bonus=(run.flask_bonus,run.memory_bonus);
        while run.act==1 {
            let id=run.available()[0];run.enter(id).unwrap();
            match run.nodes[id].kind {
                NodeKind::Monster|NodeKind::Elite|NodeKind::Boss=>{run.win_combat().unwrap();run.decline_reward().unwrap();},
                NodeKind::Camp=>run.leave_camp().unwrap(),NodeKind::Shop=>run.leave_shop().unwrap(),
                NodeKind::Chest=>run.finish_chest().unwrap(),NodeKind::Event=>run.resolve_event(false).unwrap(),
            }
        }
        assert_eq!((run.flask_bonus,run.memory_bonus),bonus);
        assert_eq!(run.flasks,run.flask_limit);
        let fresh=Run::new(1,run.class);
        assert_eq!((fresh.flask_bonus,fresh.memory_bonus),(0,0));
    }
}
