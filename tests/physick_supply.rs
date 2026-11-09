use erdtree_tower::{Class,Run,NodeKind,physick_supply::Supply};

#[test]
fn each_shop_gives_one_bottle_even_after_reload() {
    let mut run=Run::new(42,Class::Samurai);
    let id=run.nodes.iter().position(|n|n.kind==NodeKind::Shop).unwrap();
    assert!(run.physick_drop(id,false));
    assert!(run.physick_supply.queue(run.act,id));
    assert!(!run.physick_supply.queue(run.act,id));
    run.physick_supply.prepare(4);
    assert_eq!(run.physick_supply.pending.as_ref().unwrap().target,Some(5));
    let mut restored:Run=serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    restored.physick_supply.prepare(5);
    assert_eq!(restored.physick_supply.pending.as_ref().unwrap().target,Some(5));
    restored.physick_supply.complete();
    assert!(!restored.physick_supply.queue(restored.act,id));
    assert!(restored.physick_supply.queue(restored.act+1,id));
}

#[test]
fn battle_drop_is_stable_twenty_five_percent_and_requires_victory() {
    let mut drops=0;
    for seed in 0..10000 {
        let run=Run::new(seed,Class::Samurai);
        let id=run.nodes.iter().position(|n|n.kind==NodeKind::Monster).unwrap();
        assert!(!run.physick_drop(id,false));
        drops+=usize::from(run.physick_drop(id,true));
        let mut other=run.clone();other.class=Class::Astrologer;
        assert_eq!(run.physick_drop(id,true),other.physick_drop(id,true));
        for (id,node) in run.nodes.iter().enumerate().filter(|(_,n)|!n.kind.combat()&&n.kind!=NodeKind::Shop) {
            assert!(!run.physick_drop(id,true),"{:?}",node.kind);
        }
        assert!(!run.physick_drop(usize::MAX,true));
    }
    assert!((2300..2700).contains(&drops),"{drops}");
}

#[test]
fn old_saves_start_without_supply_receipts() {
    let run=Run::new(42,Class::Samurai);
    let mut json=serde_json::to_value(run).unwrap();
    json.as_object_mut().unwrap().remove("physick_supply");
    let restored:Run=serde_json::from_value(json).unwrap();
    assert_eq!(restored.physick_supply,Supply::default());
}

#[test]
fn full_native_bottle_banks_rewards_without_retrying_item_grants() {
    let mut supply=Supply::default();
    for node in 0..5 {assert!(supply.queue(1,node));assert!(supply.credit());assert!(!supply.begin_load(1));}
    assert_eq!(supply.reserve,5);
    for _ in 0..10000 {assert!(!supply.begin_load(1));assert!(!supply.credit());}
    assert!(supply.begin_load(0));
    for _ in 0..10000 {assert!(!supply.begin_load(0));}
    let mut restored:Supply=serde_json::from_str(&serde_json::to_string(&supply).unwrap()).unwrap();
    assert!(!restored.begin_load(0),"an unconfirmed grant cannot repeat after reload");
    assert!(restored.confirm_load(1));assert_eq!(restored.reserve,4);
    assert!(!restored.confirm_load(1));
    assert!(restored.begin_load(0),"next consumed bottle draws one reserve");
}

#[test]
fn legacy_failed_target_is_credited_once_instead_of_spammed() {
    let mut supply=Supply::default();
    supply.queue(1,4);supply.prepare(1);
    assert_eq!(supply.pending.as_ref().unwrap().target,Some(2));
    assert!(supply.credit());assert_eq!(supply.reserve,1);
    assert!(!supply.credit());assert!(!supply.queue(1,4));assert!(!supply.begin_load(1));
}

#[test]
fn full_inventory_keeps_the_drop_pending_and_retries_without_new_target() {
    let mut supply=Supply::default();
    assert!(supply.queue(1,3));
    supply.prepare(999);
    assert_eq!(supply.pending.as_ref().unwrap().target,None);
    assert!(!supply.queue(1,3));
    supply.prepare(998);
    assert_eq!(supply.pending.as_ref().unwrap().target,Some(999));
    supply.prepare(999);
    assert_eq!(supply.pending.as_ref().unwrap().target,Some(999));
    assert!(supply.valid());
    supply.complete();
    assert!(!supply.queue(1,3));
}
