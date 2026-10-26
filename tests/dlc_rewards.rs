use erdtree_tower::{affixes,rewards::{self,ItemKind},Class,Run};

#[test]
fn dlc_rewards_have_text_icons_and_affixed_equipment() {
    let details=include_str!("../scene-assets/reward-details.tsv");
    for (id,kind) in [(64500000,ItemKind::Weapon),(67520000,ItemKind::Weapon),(0x10000000|5260000,ItemKind::Armor),
        (0x20000000|8220,ItemKind::Talisman),(0x40000000|2004320,ItemKind::Sorcery),
        (0x40000000|2006700,ItemKind::Incantation),(0x40000000|2200000,ItemKind::Spirit),(0x40000000|2011070,ItemKind::Crystal)] {
        let item=rewards::lookup(id).expect("DLC item missing");
        assert_eq!(item.kind,kind);
        let row=details.lines().find(|l|l.starts_with(&format!("{id}\t"))).expect("DLC description missing");
        let icon=row.split('\t').nth(1).unwrap();
        assert_eq!(std::fs::metadata(format!("scene-assets/reward-icons/{icon}.rgba")).unwrap().len(),160*160*4);
        if matches!(kind,ItemKind::Weapon|ItemKind::Armor) {
            for version in [1,2] {
                let mut run=Run::new(42,Class::Samurai);run.affix_rules=version;
                let variant=affixes::roll(&run,id,0,true).unwrap();
                assert_ne!(variant,id);assert_eq!(affixes::base(variant),id);
            }
        }
    }
    assert_eq!(affixes::base(100000000),1000000);
    assert_eq!(affixes::base(160000000),1000000);
}
