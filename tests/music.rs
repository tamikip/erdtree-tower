use erdtree_tower::{music::{self,Pool},Class,Run};
use std::collections::HashSet;

#[test]
fn soundtrack_has_unique_audible_mappings_in_all_four_pools() {
    assert_eq!(music::CATALOG.len(),100);
    let mut ids=HashSet::new();let mut names=HashSet::new();let mut flags=HashSet::new();
    for track in music::CATALOG.iter() {
        assert!(ids.insert(track.id));assert!(names.insert(track.name));assert!(flags.insert(track.flag));
        assert_eq!(track.flag/1000,45020969/1000);
        assert!(!track.name.contains("Silent")&&!track.name.contains("Reserved")&&!track.name.ends_with("None"));
    }
    for pool in [Pool::Explore,Pool::Calm,Pool::Battle,Pool::Boss] {
        assert!(music::CATALOG.iter().any(|t|t.pool==pool));
    }
    assert_eq!(music::hub().name,"Bgm_111_Hub");
}

#[test]
fn each_pool_is_exhausted_before_repeating_and_seed_changes_the_order() {
    for pool in [Pool::Explore,Pool::Calm,Pool::Battle,Pool::Boss] {
        let count=music::CATALOG.iter().filter(|t|t.pool==pool).count();
        let tracks:Vec<_>=(0..count).map(|n|music::choose(42,n,pool).id).collect();
        assert_eq!(tracks.iter().copied().collect::<HashSet<_>>().len(),count);
        for n in 0..count {assert_eq!(music::choose(42,n,pool).id,music::choose(42,n+count,pool).id);}
        assert!((0..count).any(|n|music::choose(42,n,pool).id!=music::choose(43,n,pool).id));
    }
}

#[test]
fn retry_save_and_personal_rewards_do_not_change_a_level_track() {
    let run=Run::new(42,Class::Astrologer);
    let restored=Run::decode(&run.encode().unwrap()).unwrap();
    let personal=Run::new_personal(42,Class::Astrologer,999);
    for n in 0..run.nodes.len() {
        for combat in [false,true] {
            let id=music::for_node(&run,n,combat,0).id;
            assert_eq!(id,music::for_node(&restored,n,combat,0).id);
            assert_eq!(id,music::for_node(&personal,n,combat,0).id);
        }
    }
}
