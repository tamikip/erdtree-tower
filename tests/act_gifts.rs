use erdtree_tower::{Class, Run};

#[test]
fn larval_gifts_start_in_act_two_and_survive_reload_without_duplicates() {
    let mut run=Run::new(42,Class::Samurai);
    assert!(!run.larval_gift.pending(1));
    for act in 2..=3 {
        run.act=act;
        assert!(run.larval_gift.pending(act));
        assert_eq!(run.larval_gift.begin(3),4);
        run=Run::decode(&run.encode().unwrap()).unwrap();
        assert_eq!(run.larval_gift.begin(3),4);
        assert!(!run.larval_gift.complete(act,3));
        assert!(run.larval_gift.complete(act,4));
        run=Run::decode(&run.encode().unwrap()).unwrap();
        assert!(!run.larval_gift.pending(act));
        assert!(!run.larval_gift.complete(act,4));
    }
    let mut legacy=serde_json::to_value(Run::new(1,Class::Astrologer)).unwrap();
    legacy.as_object_mut().unwrap().remove("larval_gift");
    assert!(!Run::decode(&legacy.to_string()).unwrap().larval_gift.pending(1));
}
