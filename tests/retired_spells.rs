use erdtree_tower::{Class,NodeKind,Phase,Reward,Run,ShopStock,rewards};

fn retired()->Vec<u32> {let mut ids:Vec<_>=rewards::RETIRED_SPELLS.iter().copied().collect();ids.sort();ids}
fn legacy(run:&Run)->String {
    let mut json:serde_json::Value=serde_json::from_str(&run.encode().unwrap()).unwrap();
    json["ruby_spells"]=serde_json::Value::Bool(true);json.to_string()
}

#[test]
fn retired_spells_are_not_offered_anywhere() {
    assert_eq!(retired().len(),101);
    assert!(rewards::CATALOG.iter().all(|i|!rewards::RETIRED_SPELLS.contains(&i.id)));
    for class in [Class::Astrologer,Class::Prisoner] {for seed in 0..100 {
        let run=Run::new(seed,class);assert!(!run.ruby_spells);
        for node in 0..run.nodes.len() {assert!(rewards::offers(&run,node).iter().all(|r|!matches!(r,Reward::Item(id) if rewards::RETIRED_SPELLS.contains(id))));}
    }}
}

#[test]
fn saves_from_the_ruby_build_still_load() {
    let mut run=Run::new(11,Class::Astrologer);
    run.claimed_items.extend(retired().into_iter().take(3));
    let id=run.available()[0];run.enter(id).unwrap();
    if run.nodes[id].kind.combat() {run.win_combat().unwrap();}
    if let Phase::Reward{id,..}=run.phase {run.phase=Phase::Reward{id,offers:vec![Reward::Item(retired()[0]),Reward::Gold(1500),Reward::Gold(1500)]};}
    let loaded=Run::decode(&legacy(&run)).unwrap();
    assert!(!loaded.ruby_spells);
    assert_eq!(loaded.claimed_items,run.claimed_items);
    if let Phase::Reward{id,offers}=&loaded.phase {assert_eq!(offers,&rewards::offers(&loaded,*id));}
}

fn at_shop()->Run {
    for seed in 0..200 {
        let mut run=Run::new(seed,Class::Astrologer);
        for _ in 0..12 {
            if let Some(id)=run.available().into_iter().find(|id|run.nodes[*id].kind==NodeKind::Shop) {run.enter(id).unwrap();return run;}
            let id=run.available()[0];run.enter(id).unwrap();
            match run.nodes[id].kind {NodeKind::Monster|NodeKind::Elite|NodeKind::Boss=>{run.win_combat().unwrap();run.decline_reward().unwrap();},NodeKind::Camp=>run.leave_camp().unwrap(),NodeKind::Chest=>run.finish_chest().unwrap(),NodeKind::Event=>run.resolve_event(false).unwrap(),_=>unreachable!()}
        }
    }
    panic!("no shop reachable");
}

#[test]
fn saved_shop_stock_with_retired_spells_is_rebuilt() {
    let mut run=at_shop();let Phase::Node{id}=run.phase else{panic!("not at shop")};
    run.shop_stock=Some(ShopStock{node:id,items:vec![[3,(retired()[0]&0x0fffffff) as i32,3000,1];37]});
    assert!(Run::decode(&legacy(&run)).unwrap().shop_stock.is_none());
    run.shop_stock=Some(ShopStock{node:id,items:vec![[0,1000000,3000,1];37]});
    assert_eq!(Run::decode(&legacy(&run)).unwrap().shop_stock,run.shop_stock);
}
