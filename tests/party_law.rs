use erdtree_tower::{Class,Run,law::Law,party_game::{Gate,Report,Order}};

#[test]
fn host_law_must_be_applied_by_every_player_before_travel() {
    for count in 2..=8 {
        let mut host=Run::new(42,Class::Samurai);
        host.set_law(Law::default().toggle(0).toggle(8).toggle(27).toggle(28).toggle(29).toggle(30).toggle(34)).unwrap();
        host.align_party_start(53).unwrap();
        let mut guest=Run::new(53,Class::Hero);
        let mut gate=Gate::new((1..=count).collect());
        gate.report(1,Report::new(&host,0,false,false,true)).unwrap();
        for id in 2..=count {gate.report(id,Report::new(&guest,0,false,false,true)).unwrap();}
        let node=host.available()[0];assert!(gate.enter(&host,node).is_err());
        let Order::Law{epoch,seed,law}=gate.law_order(&host).unwrap().unwrap() else {panic!("missing law order")};
        assert_eq!((epoch,seed,law),(0,53,host.law));
        guest.set_law(law).unwrap();
        for id in 2..=count {gate.report(id,Report::new(&guest,0,false,false,true)).unwrap();}
        assert!(gate.law_order(&host).unwrap().is_none());
        assert!(gate.enter(&host,node).is_ok());assert_eq!(guest.flask_limit,host.flask_limit);assert_eq!(guest.nodes,host.nodes);
    }
}

#[test]
fn law_mismatch_after_progress_is_rejected_instead_of_rewriting_runs() {
    let mut host=Run::new(42,Class::Samurai);let mut guest=host.clone();
    host.set_law(Law::default().toggle(0)).unwrap();
    host.visited.push(host.available()[0]);guest.visited=host.visited.clone();
    let mut gate=Gate::new(vec![1,2]);
    gate.report(1,Report::new(&host,0,false,false,true)).unwrap();
    gate.report(2,Report::new(&guest,0,false,false,true)).unwrap();
    assert!(gate.law_order(&host).is_err());
}

#[test]
fn every_law_node_survives_party_alignment_and_wire_roundtrip() {
    for id in 0..erdtree_tower::law::NODES.len() {
        let mut law=Law::default();for node in 0..=id {law=law.toggle(node);}
        let mut run=Run::new(42,Class::Samurai);run.set_law(law).unwrap();run.align_party_start(53).unwrap();
        assert_eq!(run.law,law);assert_eq!(Run::decode(&run.encode().unwrap()).unwrap().law,law);
        let payload=erdtree_tower::party_handshake::Payload::Order{session:1,order:Order::Law{epoch:0,seed:53,law}};
        let decoded=erdtree_tower::party_handshake::Payload::decode(&payload.encode().unwrap()).unwrap();
        assert!(matches!(decoded,erdtree_tower::party_handshake::Payload::Order{order:Order::Law{law:actual,..},..} if actual==law));
    }
    let run=Run::new(42,Class::Samurai);let mut report=Report::new(&run,0,false,false,true);report.law=Law(1u64<<erdtree_tower::law::NODES.len());
    assert!(Gate::new(vec![1,2]).report(2,report).is_err());
}
