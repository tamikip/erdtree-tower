use erdtree_tower::{Class,Run,Reward,rewards::{self,ItemKind}};

fn ruby(id:u32)->bool {matches!(id&0x0fffffff,11540|11600|11610|12500..=14403)}

#[test]
fn ruby_spells_have_unique_catalog_entries_and_require_a_staff() {
    let spells:Vec<_>=rewards::CATALOG.iter().filter(|i|ruby(i.id)).collect();
    assert_eq!(spells.len(),101);
    let mut ids=std::collections::HashSet::new();
    for spell in spells {
        assert!(ids.insert(spell.id));assert_eq!(spell.kind,ItemKind::Sorcery);
        assert!(!rewards::castable(spell.id,[false,true]));
        assert!(rewards::castable(spell.id,[true,false]));
    }
    for id in [12800,12900,13501] {assert!(rewards::lookup(0x40000000|id).is_none());}
}

#[test]
fn ruby_spells_can_be_won_and_saved_without_bypassing_catalyst_filters() {
    let mut found=false;
    for seed in 0..256 {
        let mut run=Run::new(seed,Class::Astrologer);run.casting_tools=Some([true,false]);
        for node in 0..run.nodes.len() {
            found|=rewards::offers(&run,node).iter().any(|r|matches!(r,Reward::Item(id) if ruby(*id)));
        }
        run.casting_tools=Some([false,true]);
        for node in 0..run.nodes.len() {
            assert!(!rewards::offers(&run,node).iter().any(|r|matches!(r,Reward::Item(id) if ruby(*id))));
        }
    }
    assert!(found);
    let mut run=Run::new(17,Class::Astrologer);run.claimed_items.push(0x40000000|13511);
    assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
}

#[test]
fn old_saves_keep_their_reward_pool_and_new_spell_claims_are_not_repeated() {
    let mut run=Run::new(3,Class::Astrologer);
    let mut json:serde_json::Value=serde_json::from_str(&run.encode().unwrap()).unwrap();
    json.as_object_mut().unwrap().remove("ruby_spells");
    let old=Run::decode(&json.to_string()).unwrap();assert!(!old.ruby_spells);
    for node in 0..old.nodes.len() {assert!(!rewards::offers(&old,node).iter().any(|r|matches!(r,Reward::Item(id) if ruby(*id))));}
    run.claimed_items.extend(rewards::CATALOG.iter().filter(|i|ruby(i.id)).map(|i|i.id));
    for node in 0..run.nodes.len() {assert!(!rewards::offers(&run,node).iter().any(|r|matches!(r,Reward::Item(id) if ruby(*id))));}
    assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
}
