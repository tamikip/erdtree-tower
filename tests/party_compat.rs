use erdtree_tower::{party_compat::{Compatibility,parse_rules},party_handshake::{Handshake,Message},party_roster::Roster};
fn wire(message:Message)->Message {
    use erdtree_tower::party_handshake::Payload;
    let Payload::Handshake(message)=Payload::decode(&Payload::Handshake(message).encode().unwrap()).unwrap() else{panic!("handshake frame")};
    message
}

fn profile()->Compatibility {
    Compatibility{tower_build:"0123456789abcdef".into(),seamless_version:"2.0.3".into(),seamless_hash:"123456789abcdef0".into(),assets:"23456789abcdef01".into(),rules:[0,0,1,1,100,20,30,100,20,80]}
}
fn state(local:u64,profile:Compatibility)->Handshake {
    Handshake::new(Roster{local,host:1,members:vec![1,2]},100+local,42).unwrap().with_compatibility(profile).unwrap()
}
#[test]
fn compatible_party_still_waits_for_ack_before_committing() {
    let mut host=state(1,profile());let mut guest=state(2,profile());
    let offer=host.receive_lobby(2,wire(guest.hello().unwrap().1)).unwrap().remove(0).1;
    assert!(host.ready().is_none());
    let ack=guest.receive_lobby(1,wire(offer)).unwrap().remove(0).1;
    assert!(guest.ready().is_none());
    let commit=host.receive_lobby(2,wire(ack)).unwrap().remove(0).1;
    guest.receive_lobby(1,wire(commit)).unwrap();assert_eq!(host.ready(),guest.ready());
    assert_eq!(host.ready(),Some((101,42)));
}
#[test]
fn each_incompatibility_is_reported_before_any_offer_or_commit() {
    for (field,label) in [("tower_build","爬塔构建"),("seamless_version","无缝版本"),("seamless_hash","无缝文件"),("assets","关键资源"),("rules","敌人生命缩放")] {
        let mut other=profile();
        match field {"tower_build"=>other.tower_build="ffffffffffffffff".into(),"seamless_version"=>other.seamless_version="2.0.2".into(),
            "seamless_hash"=>other.seamless_hash="ffffffffffffffff".into(),"assets"=>other.assets="ffffffffffffffff".into(),_=>other.rules[4]+=1}
        let mut host=state(1,profile());let guest=state(2,other);
        let error=host.receive_lobby(2,guest.hello().unwrap().1).unwrap_err();assert!(error.contains(label),"{error}");
        assert!(host.ready().is_none());assert!(host.receive(2,Message::Ack{nonce:102,session:101}).is_err());
    }
}
#[test]
fn guest_checks_host_profile_and_missing_profile_does_not_pass() {
    let mut host=state(1,profile());let mut guest=state(2,profile());
    let mut offer=host.receive_lobby(2,guest.hello().unwrap().1).unwrap().remove(0).1;
    if let Message::Offer{compatibility:Some(p),..}=&mut offer {p.assets="ffffffffffffffff".into();}
    assert!(guest.receive_lobby(1,offer).unwrap_err().contains("关键资源"));assert!(guest.ready().is_none());
    let legacy=Handshake::new(Roster{local:2,host:1,members:vec![1,2]},102,42).unwrap();
    assert!(state(1,profile()).receive_lobby(2,legacy.hello().unwrap().1).unwrap_err().contains("兼容信息"));
}
#[test]
fn rules_ignore_secrets_and_presentation_but_reject_missing_or_ambiguous_values() {
    let text="allow_invaders=0\ndeath_debuffs=0\nallow_summons=1\nalways_spectate_on_death=1\nenemy_health_scaling=100\nenemy_damage_scaling=20\nenemy_posture_scaling=30\nboss_health_scaling=100\nboss_damage_scaling=20\nboss_posture_scaling=80\n";
    let rules=parse_rules(text).unwrap();assert_eq!(rules,profile().rules);
    assert_eq!(parse_rules(&format!("\u{feff}{text}cooppassword=private-secret\nmod_language_override=english\n")).unwrap(),rules);
    assert!(parse_rules(&text.replace("allow_summons=1\n","")).is_err());
    assert!(parse_rules(&format!("{text}allow_summons=0\n")).is_err());
    assert!(parse_rules(&text.replace("enemy_damage_scaling=20","enemy_damage_scaling=NaN")).is_err());
}
#[test]
fn rejection_is_bound_to_the_current_peer_and_nonce() {
    let mut host=state(1,profile());let mut guest=state(2,profile());
    assert!(guest.receive_lobby(1,Message::Reject{nonce:999,reason:"stale".into()}).unwrap().is_empty());
    assert!(guest.receive_lobby(3,Message::Reject{nonce:102,reason:"outsider".into()}).unwrap().is_empty());
    assert!(guest.receive_lobby(1,Message::Reject{nonce:102,reason:"关键资源不一致".into()}).unwrap_err().contains("关键资源"));
    host.receive_lobby(2,guest.hello().unwrap().1).unwrap();
    assert!(host.receive_lobby(2,Message::Reject{nonce:102,reason:"构建不一致".into()}).unwrap_err().contains("构建"));
    assert!(host.ready().is_none());assert!(guest.ready().is_none());
}
#[test]
fn one_incompatible_member_does_not_release_the_compatible_members() {
    let members=vec![1,2,3];
    let mut host=Handshake::new(Roster{local:1,host:1,members:members.clone()},101,42).unwrap().with_compatibility(profile()).unwrap();
    let mut good=Handshake::new(Roster{local:2,host:1,members:members.clone()},102,42).unwrap().with_compatibility(profile()).unwrap();
    let mut bad_profile=profile();bad_profile.rules[8]=21;
    let bad=Handshake::new(Roster{local:3,host:1,members},103,42).unwrap().with_compatibility(bad_profile).unwrap();
    let offer=host.receive_lobby(2,good.hello().unwrap().1).unwrap().remove(0).1;
    let ack=good.receive_lobby(1,offer).unwrap().remove(0).1;
    assert!(host.receive_lobby(2,ack).unwrap().is_empty());
    let hello=bad.hello().unwrap().1;
    let error=host.receive_lobby(3,hello.clone()).unwrap_err();assert!(error.contains("Boss 伤害缩放"));
    let rejects=host.rejections(3,&hello,&format!("队员 3：{error}"));
    assert_eq!(rejects.len(),2);
    for (id,reject) in rejects {
        if id==2 {assert!(good.receive_lobby(1,wire(reject)).unwrap_err().contains("队员 3"));}
        else {assert!(matches!(reject,Message::Reject{nonce:103,..}));}
    }
    assert!(host.ready().is_none());assert!(good.ready().is_none());
}
#[test]
fn compatibility_does_not_remove_existing_dlc_negotiation() {
    let mut host=state(1,profile()).with_dlc(true);let mut guest=state(2,profile()).with_dlc(false);
    let offer=host.receive_lobby(2,guest.hello().unwrap().1).unwrap().remove(0).1;
    let ack=guest.receive_lobby(1,offer).unwrap().remove(0).1;
    let commit=host.receive_lobby(2,ack).unwrap().remove(0).1;guest.receive_lobby(1,commit).unwrap();
    assert_eq!(host.party_dlc(),Some(false));assert_eq!(guest.party_dlc(),Some(false));
}
