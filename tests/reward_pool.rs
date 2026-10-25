use erdtree_tower::{Class,Phase,Reward,Run,rewards::{self,ItemKind}};

#[test]
fn random_rewards_never_include_numbered_talisman_upgrades() {
    for seed in 0..500 {
        let run=Run::new(seed,Class::Samurai);
        for reward in rewards::offers(&run,0) {if let Reward::Item(id)=reward {
            let item=rewards::lookup(id).unwrap();
            assert!(item.kind!=ItemKind::Talisman||!item.name.contains(['+','＋']),"{}",item.name);
        }}
    }
    assert!(rewards::lookup(0x20000000|1001).is_some());
}

#[test]
fn expanded_catalog_is_large_unique_and_keeps_previous_items() {
    let pool=&rewards::CATALOG;
    assert!(pool.len()>1000);
    for line in include_str!("../scene-assets/reward-catalog-v29.txt").lines() {
        let p:Vec<_>=line.split('|').collect();
        assert!(pool.iter().any(|i|(i.id&0x0fffffff).to_string()==p[1]&&i.name==p[2]));
    }
    assert_eq!(pool.iter().map(|i|i.id).collect::<std::collections::HashSet<_>>().len(),pool.len());
    for (kind,min) in [(ItemKind::Weapon,300),(ItemKind::Armor,400),(ItemKind::Talisman,100),(ItemKind::Sorcery,50),(ItemKind::Incantation,35),(ItemKind::Spirit,60)] {assert!(pool.iter().filter(|i|i.kind==kind).count()>=min);}
}

#[test]
fn class_pools_cover_equipment_and_only_matching_spells() {
    for class in Class::ALL {
        let mut seen=Vec::new();
        for seed in 0..200 {
            let mut run=Run::new(seed,class);run.enter(0).unwrap();run.win_combat().unwrap();
            let Phase::Reward{ref offers,..}=run.phase else {panic!()};
            assert_eq!(offers.len(),3);assert!(!offers.contains(&Reward::UpgradeWeapon));
            for reward in offers {
                if let Reward::Item(id)=reward {
                    let kind=rewards::lookup(*id).unwrap().kind;
                    assert!(kind.allowed(class));seen.push(kind);
                }
            }
            assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
        }
        assert!(seen.contains(&ItemKind::Weapon));assert!(seen.contains(&ItemKind::Armor));
        assert!(seen.iter().any(|kind|kind.label()=="骨灰"));
        assert_eq!(seen.contains(&ItemKind::Sorcery),class.sorcerer());
        assert_eq!(seen.contains(&ItemKind::Incantation),class.cleric());
    }
}
#[test]
fn item_claim_is_single_use_and_not_offered_again() {
    let mut run=(0..100).map(|seed|{let mut r=Run::new(seed,Class::Astrologer);r.enter(0).unwrap();r.win_combat().unwrap();r})
        .find(|r|matches!(&r.phase,Phase::Reward{offers,..} if offers.iter().any(|o|matches!(o,Reward::Item(_))))).unwrap();
    let Phase::Reward{ref offers,..}=run.phase else {panic!()};
    let index=offers.iter().position(|o|matches!(o,Reward::Item(_))).unwrap();
    let Reward::Item(id)=offers[index] else {panic!()};
    run.claim_reward(index).unwrap();assert!(run.claim_reward(index).is_err());
    assert_eq!(run.claimed_items,vec![id]);
    for node in 0..36 {assert!(!rewards::offers(&run,node).contains(&Reward::Item(id)));}
}
