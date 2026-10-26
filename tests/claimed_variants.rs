use erdtree_tower::{Class,NodeKind,Reward,Run,affixes,rewards};

fn owned_variant(run:&Run,base:u32)->u32 {affixes::roll(run,base,0,true).filter(|id|*id!=base).expect("Legendary weapon must have an affix variant")}

#[test]
fn claimed_affix_variant_excludes_its_base_from_boss_pages() {
    for seed in 0..400 {
        let mut run=Run::new(seed,Class::Samurai);
        let variant=owned_variant(&run,3150000);run.claimed_items.push(variant);
        let node=run.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap();
        assert!(rewards::offers(&run,node).iter().all(|r|!matches!(r,Reward::Item(id) if affixes::base(*id)==3150000)),"seed {seed}");
    }
}
