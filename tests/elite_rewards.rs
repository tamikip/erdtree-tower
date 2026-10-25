use erdtree_tower::{Class,NodeKind,Phase,Reward,Run};
#[test]
fn elites_offer_legendary_buff_and_one_tear_and_persist_chosen_buff() {
    for class in Class::ALL {
        let mut run=Run::new(26,class);
        let id=run.nodes.iter().position(|n|n.kind==NodeKind::Elite).unwrap();
        let offers=erdtree_tower::rewards::offers(&run,id);
        assert!(matches!(offers[0],Reward::Item(id) if erdtree_tower::elite::LEGENDARY.contains(&erdtree_tower::affixes::base(id))||erdtree_tower::elite::EPIC.contains(&erdtree_tower::affixes::base(id))));
        assert!(matches!(offers[1],Reward::TalismanBuff(_)));
        assert_eq!(offers[2],Reward::SacredTear);
        if let Reward::TalismanBuff(id)=offers[1] {run.elite_buffs.push(id);}
        assert!(erdtree_tower::rewards::offers(&run,id).iter().all(|r|!matches!(r,Reward::TalismanBuff(id) if run.elite_buffs.contains(id))));
        run.phase=Phase::Map;
        assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
    }
}
#[test]
fn elite_epic_and_legendary_tiers_both_appear_without_changing_bosses() {
    let mut epic=0usize;
    for seed in 0..1000 {
        let run=Run::new(seed,Class::Samurai);
        let id=0;
        let offers=erdtree_tower::elite::offers(&run,id);
        if matches!(offers[0],Reward::Item(id) if erdtree_tower::elite::EPIC.contains(&id)){epic+=1;}
        assert_eq!(offers,erdtree_tower::elite::offers(&run,id));
        assert!(erdtree_tower::elite::boss_offers(&run,35).iter().all(|r|matches!(r,Reward::Item(id) if erdtree_tower::elite::LEGENDARY.contains(id))));
    }
    let expected=1000*erdtree_tower::elite::EPIC.len()/(erdtree_tower::elite::EPIC.len()+9);
    assert!(epic.abs_diff(expected)<40);
    assert_eq!(erdtree_tower::elite::EPIC.iter().chain(erdtree_tower::elite::LEGENDARY).collect::<std::collections::HashSet<_>>().len(),erdtree_tower::elite::EPIC.len()+erdtree_tower::elite::LEGENDARY.len());
    assert!(erdtree_tower::elite::EPIC.iter().all(|id|erdtree_tower::rewards::lookup(*id).is_some_and(|i|i.kind==erdtree_tower::rewards::ItemKind::Weapon)));
}
