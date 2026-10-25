use erdtree_tower::{Class,Run,NodeKind};
#[test]
fn multiple_starts_merge_into_one_boss_without_crossing_edges() {
    for seed in 0..100 {
        let r=Run::new(seed,Class::Samurai);
        assert!((4..=5).contains(&r.available().len()));assert_eq!(r.floors(),16);
        for layer in 0..15 {assert!((4..=5).contains(&r.nodes.iter().filter(|n|n.layer==layer).count()));}
        assert_eq!(r.nodes.iter().filter(|n|n.kind==NodeKind::Boss).count(),1);
        for start in r.start_nodes() {
            let mut trial=r.clone();trial.enter(start).unwrap();trial.win_combat().unwrap();trial.decline_reward().unwrap();
            assert_eq!(trial.available(),trial.nodes[start].next);
            assert!(trial.available().iter().all(|i|*i>=3));
            assert!(Run::decode(&trial.encode().unwrap()).is_ok());
            let mut reachable=vec![false;r.nodes.len()];reachable[start]=true;
            for (id,node) in r.nodes.iter().enumerate(){if reachable[id]{for &next in &node.next{reachable[next]=true;}}}
            assert!(reachable[r.nodes.len()-1]);
        }
        for layer in 0..r.floors()-1 {
            let mut visible=vec![false;r.nodes.len()];for id in r.start_nodes(){visible[id]=true;}
            for (id,n) in r.nodes.iter().enumerate(){if visible[id]{for &t in &n.next{visible[t]=true;}}}
            let mut edges=Vec::new();for (_,n) in r.nodes.iter().enumerate().filter(|(id,n)|visible[*id]&&n.layer==layer){for &t in &n.next {edges.push((n.lane,r.nodes[t].lane));}}
            for &(a,b) in &edges {for &(c,d) in &edges {assert!(!(a<c&&b>d));}}
        }
    }
}
