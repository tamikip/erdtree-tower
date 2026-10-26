use erdtree_tower::{affixes,rewards::ItemKind};

#[test]
fn random_ash_manifest_covers_every_weapon_without_polluting_plain_ui_text() {
    let text=std::fs::read_to_string("scene-assets/random-ashes.tsv").unwrap();
    let rows:std::collections::HashMap<u32,Vec<&str>>=text.lines().skip(1).map(|l|{let p:Vec<_>=l.split('\t').collect();(p[0].parse().unwrap(),p)}).collect();
    let mut randomized=0;
    for item in affixes::CATALOG.iter().filter(|i|i.kind==ItemKind::Weapon) {
        let row=&rows[&item.id];assert_eq!(row[1].parse::<u32>().unwrap(),affixes::base(item.id));
        if row[2]!="-1" {randomized+=1;assert!(!row[4].is_empty());}
        assert!(!item.name.contains("<font"));
    }
    assert!(randomized>1000);
    assert!(rows.values().any(|r|r[2].parse::<i32>().unwrap()>=400000),"DLC ashes missing");
}
