use erdtree_tower::{Class,Run,Reward,Phase,NodeKind};

#[test]
fn stream_reward_is_personal_unique_and_persists_only_for_the_run() {
    let mut run=(0..1000).find_map(|seed| {
        let mut r=Run::new(seed,Class::Astrologer);r.enter(0).unwrap();r.win_combat().unwrap();
        matches!(&r.phase,Phase::Reward{offers,..} if offers.contains(&Reward::StarStream)).then_some(r)
    }).unwrap();
    let Phase::Reward{ref offers,..}=run.phase else{unreachable!()};
    let index=offers.iter().position(|r|*r==Reward::StarStream).unwrap();
    run.claim_reward(index).unwrap();
    assert!(run.star_stream);assert!(run.claim_reward(index).is_err());
    assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
    while run.act==1 {
        let id=run.available()[0];run.enter(id).unwrap();
        match run.nodes[id].kind {
            NodeKind::Monster|NodeKind::Elite|NodeKind::Boss=>{run.win_combat().unwrap();run.decline_reward().unwrap();},
            NodeKind::Camp=>run.leave_camp().unwrap(),NodeKind::Shop=>run.leave_shop().unwrap(),
            NodeKind::Chest=>run.finish_chest().unwrap(),NodeKind::Event=>run.resolve_event(false).unwrap(),
        }
    }
    assert!(run.star_stream);
    assert!(!Run::new(run.seed,run.class).star_stream);
    for seed in 0..1000 {
        let mut fresh=Run::new(seed,Class::Astrologer);fresh.star_stream=true;
        assert!(!erdtree_tower::rewards::offers(&fresh,0).contains(&Reward::StarStream));
        let warrior=Run::new(seed,Class::Samurai);
        assert!(!erdtree_tower::rewards::offers(&warrior,0).contains(&Reward::StarStream));
    }
}
