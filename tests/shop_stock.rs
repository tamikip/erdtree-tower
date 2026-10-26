use erdtree_tower::{Class,NodeKind,Phase,Run,ShopStock};

fn at_shop()->Run {
    for seed in 0..200 {
        let mut run=Run::new(seed,Class::Samurai);
        for _ in 0..12 {
            let Some(id)=run.available().into_iter().find(|id|run.nodes[*id].kind==NodeKind::Shop) else {
                let id=run.available()[0];run.enter(id).unwrap();
                match run.nodes[id].kind {NodeKind::Monster|NodeKind::Elite|NodeKind::Boss=>{run.win_combat().unwrap();run.decline_reward().unwrap();},NodeKind::Camp=>run.leave_camp().unwrap(),NodeKind::Chest=>run.finish_chest().unwrap(),NodeKind::Event=>run.resolve_event(false).unwrap(),_=>unreachable!()}
                continue;
            };
            run.enter(id).unwrap();return run;
        }
    }
    panic!("no shop reachable");
}

#[test]
fn saved_shop_stock_survives_reload_and_is_cleared_on_leave() {
    let mut run=at_shop();
    let Phase::Node{id}=run.phase else{panic!("not at shop")};
    run.shop_stock=Some(ShopStock{node:id,items:vec![[0,1000000,3000,1];37]});
    let loaded=Run::decode(&run.encode().unwrap()).unwrap();
    assert_eq!(loaded.shop_stock,run.shop_stock);
    let mut left=loaded.clone();left.leave_shop().unwrap();assert!(left.shop_stock.is_none());
    assert!(!left.encode().unwrap().contains("shop_stock"));
    let mut wrong=run.clone();wrong.shop_stock=Some(ShopStock{node:id+1,items:vec![]});
    assert!(Run::decode(&wrong.encode().unwrap()).is_err());
}
