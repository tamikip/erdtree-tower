use erdtree_tower::{Run,Class,Reward,rewards::{self,ItemKind}};
#[test]
fn spirit_levels_are_valid_stable_and_cover_zero_to_ten() {
    let base=0x40000000|200000;
    for (min,max) in [(0,10)] {
        let mut seen=std::collections::BTreeSet::new();
        for seed in 0..1000 {
            let id=rewards::random_spirit_level(base,seed);
            assert_eq!(id,rewards::random_spirit_level(base,seed));
            assert!((min..=max).contains(&(id-base)));seen.insert(id-base);
            assert_eq!(rewards::lookup(id).unwrap().kind,ItemKind::Spirit);
        }
        assert_eq!(seen,(min..=max).collect());
    }
}
#[test]
fn offered_spirit_name_matches_actual_item_and_save_roundtrips() {
    let mut seen=false;
    for seed in 0..200 {
        let mut r=Run::new(seed,Class::Samurai);let id=r.available()[0];r.enter(id).unwrap();r.win_combat().unwrap();
        if let erdtree_tower::Phase::Reward{offers,..}=&r.phase {
            for reward in offers {if let Reward::Item(item)=reward {
                if let Some(i)=rewards::lookup(*item).filter(|i|i.kind==ItemKind::Spirit) {
                    seen=true;assert!((item&0x0fffffff)%1000<=10);
                    if (item&0x0fffffff)%1000!=0 {assert!(i.name.contains('+'));}
                }
            }}
        }
        assert_eq!(Run::decode(&r.encode().unwrap()).unwrap(),r);
    }
    assert!(seen);
}
#[test]
fn claimed_upgraded_spirits_are_not_offered_again() {
    for seed in 0..200 {
        let mut r=Run::new(seed,Class::Samurai);let node=r.available()[0];
        r.claimed_items=rewards::CATALOG.iter().filter(|i|i.kind==ItemKind::Spirit).flat_map(|i|(1..=10).map(move |level|i.id+level)).collect();
        for reward in rewards::offers(&r,node) {if let Reward::Item(id)=reward {assert!(!r.claimed_items.contains(&id));}}
    }
}
