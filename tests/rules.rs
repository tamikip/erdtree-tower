use erdtree_tower::*;

fn camp(flasks: u8) -> Run {
    let mut run = Run::new(42, Class::Samurai);
    run.flasks = flasks;
    run.phase = Phase::Node { id: run.nodes.iter().position(|n|n.kind==NodeKind::Camp).unwrap() };
    run
}

#[test]
fn starts_with_ten_flasks_and_class() {
    let r = Run::new(1, Class::Astrologer);
    assert_eq!(r.flasks, 10);
    assert_eq!(r.flask_limit, 10);
    assert_eq!(r.class, Class::Astrologer);
    assert_eq!(r.phase, Phase::Map);
}

#[test]
fn camp_refills_three_without_healing() {
    let mut r = camp(2);
    r.hp = 30;
    r.camp(CampChoice::Refill).unwrap();
    assert_eq!((r.flasks, r.hp), (5, 30));
    assert_eq!(r.phase, Phase::Map);
}

#[test]
fn refill_caps_at_ten() {
    let mut r = camp(9);
    r.camp(CampChoice::Refill).unwrap();
    assert_eq!(r.flasks, 10);
}

#[test]
fn blue_supply_consumes_the_same_camp_choice() {
    let mut r = camp(0);
    r.camp(CampChoice::RefillBlue).unwrap();
    assert_eq!(r.flasks, 3);
    assert!(r.camp(CampChoice::Refill).is_err());
    assert!(r.camp(CampChoice::UpgradeWeapon).is_err());
}

#[test]
fn pure_color_supply_restores_physick_and_spends_one_camp_service() {
    for choice in [CampChoice::RefillRedOnly,CampChoice::RefillBlueOnly] {
        let mut run=camp(1);run.physick_empty=true;
        run.camp(choice.clone()).unwrap();
        assert_eq!(run.flasks,4);assert!(!run.physick_empty);
        assert!(run.camp(choice).is_err());
    }
}

#[test]
fn full_flask_refill_does_not_consume_camp() {
    let mut r = camp(10);
    let before = r.clone();
    assert!(r.camp(CampChoice::Refill).is_err());
    assert_eq!(r, before);
}

#[test]
fn empty_physick_can_use_camp_with_full_red_blue_flasks() {
    let mut run=camp(10);run.physick_empty=true;
    run.camp(CampChoice::Refill).unwrap();
    assert_eq!(run.flasks,10);assert!(!run.physick_empty);
    assert!(run.camp(CampChoice::Refill).is_err());
}

#[test]
fn camp_choice_is_exclusive() {
    let mut r = camp(1);
    r.camp(CampChoice::UpgradeWeapon).unwrap();
    assert_eq!((r.weapon_level, r.flasks), (1, 1));
    assert!(r.camp(CampChoice::Refill).is_err());
}

#[test]
fn camp_upgrades_owned_blessing_only() {
    let mut r = camp(1);
    r.blessings.push(Blessing {
        kind: BlessingKind::Guard,
        level: 1,
    });
    let before = r.clone();
    assert!(r.camp(CampChoice::UpgradeBlessing(1)).is_err());
    assert_eq!(r, before);
    r.camp(CampChoice::UpgradeBlessing(0)).unwrap();
    assert_eq!(r.blessings[0].level, 2);
    assert_eq!(r.flasks, 1);
}

#[test]
fn capped_upgrades_leave_camp_available() {
    let mut r = camp(1);
    r.weapon_level = 25;
    assert!(r.camp(CampChoice::UpgradeWeapon).is_err());
    assert!(matches!(r.phase, Phase::Node { .. }));
    r.blessings.push(Blessing {
        kind: BlessingKind::Guard,
        level: 3,
    });
    assert!(r.camp(CampChoice::UpgradeBlessing(0)).is_err());
    assert!(matches!(r.phase, Phase::Node { .. }));
}

#[test]
fn combat_does_not_refill_or_heal() {
    let mut r = Run::new(1, Class::Vagabond);
    r.enter(0).unwrap();
    r.use_flask().unwrap();
    r.observe_hp(55).unwrap();
    r.win_combat().unwrap();
    assert_eq!((r.hp, r.flasks), (55, 9));
    assert!(matches!(r.phase, Phase::Reward { .. }));
}

#[test]
fn combat_reward_cannot_be_farmed() {
    let mut r = Run::new(1, Class::Vagabond);
    r.enter(0).unwrap();
    r.win_combat().unwrap();
    let before = r.clone();
    assert!(r.win_combat().is_err());
    assert_eq!(r, before);
    r.claim_reward(0).unwrap();
    let before = r.clone();
    assert!(r.claim_reward(0).is_err());
    assert_eq!(r, before);
    assert!(r.enter(0).is_err());
}

#[test]
fn cannot_skip_layers_or_select_unconnected_node() {
    let mut r = Run::new(1, Class::Vagabond);
    assert!(r.enter(r.start_nodes().len()).is_err());
    r.enter(0).unwrap();
    r.win_combat().unwrap();
    r.claim_reward(0).unwrap();
    let blocked=(0..r.nodes.len()).find(|id|!r.available().contains(id)).unwrap();
    assert!(r.enter(blocked).is_err());
    r.enter(r.available()[0]).unwrap();
}

#[test]
fn seed_fixes_routes_and_rewards() {
    let mut a = Run::new(123, Class::Vagabond);
    let mut b = a.clone();
    a.enter(0).unwrap();
    b.enter(0).unwrap();
    a.win_combat().unwrap();
    b.win_combat().unwrap();
    assert_eq!(a, b);
}

#[test]
fn graph_is_connected_and_has_fixed_endpoints() {
    for seed in 0..1000 {
        let r = Run::new(seed, Class::Vagabond);
        assert_eq!(r.floors(),16);
        assert_eq!(r.nodes[0].kind, NodeKind::Monster);
        for node in r.nodes.iter().filter(|n|n.layer==14) {
            assert_eq!(node.kind, NodeKind::Camp);
        }
        for node in r.nodes.iter().filter(|n|n.layer==15) {
            assert_eq!(node.kind, NodeKind::Boss);
        }
        let mut reached = vec![0usize];
        for layer in 1..r.floors() {
            let mut next = Vec::new();
            for id in &reached {
                for target in &r.nodes[*id].next {
                    assert_eq!(r.nodes[*target].layer, layer);
                    if !next.contains(target) {
                        next.push(*target);
                    }
                }
            }
            assert!(!next.is_empty());
            reached = next;
        }
    }
}

#[test]
fn route_branches_do_not_cross_between_layers() {
    for seed in 0..100 {
        let run = Run::new(seed, Class::Samurai);
        for layer in 0..run.floors()-1 {
            let nodes:Vec<_> = run.nodes.iter().filter(|n|n.layer==layer).collect();
            assert!(nodes.iter().all(|n| !n.next.is_empty() && n.next.len() <= 2));
            for a in &nodes { for b in &nodes {
                if a.lane < b.lane {
                    for x in &a.next { for y in &b.next { assert!(x <= y); } }
                }
            } }
        }
    }
}

#[test]
fn all_seven_node_types_exist() {
    let types: std::collections::HashSet<_> = (0..100)
        .flat_map(|seed| Run::new(seed, Class::Vagabond).nodes)
        .map(|n| n.kind)
        .collect();
    assert_eq!(types.len(), 7);
}

#[test]
fn death_is_terminal() {
    let mut r = Run::new(1, Class::Vagabond);
    r.enter(0).unwrap();
    r.observe_hp(0).unwrap();
    assert_eq!(r.phase, Phase::Dead);
    assert!(r.win_combat().is_err());
    assert!(r.enter(3).is_err());
    assert!(r.use_flask().is_err());
}

#[test]
fn save_resumes_reward_without_reroll() {
    let mut r = Run::new(1, Class::Vagabond);
    r.enter(0).unwrap();
    r.win_combat().unwrap();
    let json = r.encode().unwrap();
    assert_eq!(Run::decode(&json).unwrap(), r);
}

#[test]
fn malformed_and_invalid_saves_are_rejected() {
    assert!(Run::decode("{}").is_err());
    let mut r = Run::new(1, Class::Vagabond);
    r.flasks = 11;
    assert!(Run::decode(&r.encode().unwrap()).is_err());
    r.flasks = 5;
    r.nodes[0].next = vec![999];
    assert!(Run::decode(&r.encode().unwrap()).is_err());
}

#[test]
fn pending_combat_save_restarts_exact_encounter_checkpoint() {
    let mut r = Run::new(1, Class::Vagabond);
    r.enter(0).unwrap();
    r.use_flask().unwrap();
    r.observe_hp(45).unwrap();
    let restored = Run::decode(&r.encode().unwrap()).unwrap();
    assert_eq!((restored.hp, restored.flasks), (100, 10));
    assert_eq!(restored.phase, Phase::Combat { id: 0 });
}

#[test]
fn shop_requires_money_and_stock_is_single_use() {
    let mut r = Run::new(1, Class::Vagabond);
    let id = r
        .nodes
        .iter()
        .position(|n| n.kind == NodeKind::Shop)
        .unwrap();
    r.phase = Phase::Node { id };
    let before = r.clone();
    assert!(r.buy(0).is_err());
    assert_eq!(r, before);
    r.gold = 300;
    r.buy(0).unwrap();
    assert_eq!(r.gold, 225);
    assert!(r.buy(0).is_err());
    r.leave_shop().unwrap();
    assert!(r.buy(1).is_err());
}

#[test]
fn chest_reward_only_once() {
    let mut r = Run::new(1, Class::Vagabond);
    let id = r
        .nodes
        .iter()
        .position(|n| n.kind == NodeKind::Chest)
        .unwrap();
    r.phase = Phase::Node { id };
    r.open_chest().unwrap();
    assert!(r.open_chest().is_err());
    r.claim_reward(0).unwrap();
    assert_eq!(r.phase, Phase::Map);
}

#[test]
fn event_cost_is_visible_and_does_not_kill_player() {
    let mut r = Run::new(1, Class::Vagabond);
    let id = r
        .nodes
        .iter()
        .position(|n| n.kind == NodeKind::Event)
        .unwrap();
    r.phase = Phase::Node { id };
    r.hp = 10;
    assert_eq!(r.event_cost(), 15);
    assert!(r.resolve_event(true).is_err());
    r.resolve_event(false).unwrap();
    assert_eq!(r.hp, 10);
}

#[test]
fn boss_victory_finishes_after_reward() {
    let mut r = Run::new(1, Class::Vagabond);
    r.act=3;
    r.phase = Phase::Combat { id: r.nodes.len()-1 };
    r.win_combat().unwrap();
    r.claim_reward(0).unwrap();
    assert_eq!(r.phase, Phase::Won);
}

#[test]
fn failed_reward_selection_is_atomic() {
    let mut r = Run::new(1, Class::Vagabond);
    r.enter(0).unwrap();
    r.win_combat().unwrap();
    let before = r.clone();
    assert!(r.claim_reward(3).is_err());
    assert_eq!(r, before);
}

#[test]
fn flask_consumption_is_only_legal_in_combat() {
    let mut r = Run::new(1, Class::Vagabond);
    assert!(r.use_flask().is_err());
    r.enter(0).unwrap();
    for _ in 0..10 {
        r.use_flask().unwrap();
    }
    assert!(r.use_flask().is_err());
    assert_eq!(r.flasks, 0);
}

#[test]
fn complete_runs_remain_valid_at_every_checkpoint() {
    for seed in 0..1000 {
        let mut r = Run::new(seed, Class::Prophet);
        for _ in 0..48 {
            r.validate().unwrap();
            r.enter(r.available()[0]).unwrap();
            let id = match r.phase {
                Phase::Combat { id } | Phase::Node { id } => id,
                _ => unreachable!(),
            };
            r = Run::decode(&r.encode().unwrap()).unwrap();
            match r.nodes[id].kind {
                NodeKind::Monster | NodeKind::Elite | NodeKind::Boss => {
                    r.win_combat().unwrap();
                    r = Run::decode(&r.encode().unwrap()).unwrap();
                    r.claim_reward(0).unwrap();
                }
                NodeKind::Camp => {
                    if r.weapon_level < 25 {
                        r.camp(CampChoice::UpgradeWeapon).unwrap();
                    } else {
                        let index = r.blessings.iter().position(|b| b.level < 3).unwrap();
                        r.camp(CampChoice::UpgradeBlessing(index)).unwrap();
                    }
                }
                NodeKind::Shop => r.leave_shop().unwrap(),
                NodeKind::Event => r.resolve_event(false).unwrap(),
                NodeKind::Chest => {
                    r.open_chest().unwrap();
                    r.claim_reward(0).unwrap();
                }
            }
            r = Run::decode(&r.encode().unwrap()).unwrap();
        }
        assert_eq!(r.phase, Phase::Won);
    }
}

#[test]
fn save_rejects_changed_offers_and_unreachable_active_nodes() {
    let mut r = Run::new(1, Class::Vagabond);
    r.enter(0).unwrap();
    r.win_combat().unwrap();
    if let Phase::Reward { ref mut offers, .. } = r.phase {
        offers[0] = Reward::Gold(u32::MAX);
    }
    assert!(Run::decode(&r.encode().unwrap()).is_err());
    r.phase = Phase::Node { id: r.nodes.iter().position(|n|n.kind==NodeKind::Camp).unwrap() };
    assert!(Run::decode(&r.encode().unwrap()).is_err());
}

#[test]
fn saving_a_dead_run_never_restores_combat_checkpoint() {
    let mut r = Run::new(1, Class::Vagabond);
    r.enter(0).unwrap();
    r.observe_hp(0).unwrap();
    let restored = Run::decode(&r.encode().unwrap()).unwrap();
    assert_eq!((restored.phase, restored.hp), (Phase::Dead, 0));
}

#[test]
fn purchasing_capped_blessing_does_not_charge_or_consume_stock() {
    let mut r = Run::new(1, Class::Vagabond);
    r.phase = Phase::Node { id: r.nodes.iter().position(|n|n.kind==NodeKind::Shop).unwrap() };
    r.gold = 300;
    r.blessings.push(Blessing {
        kind: BlessingKind::Vigor,
        level: 3,
    });
    let before = r.clone();
    assert!(r.buy(0).is_err());
    assert_eq!(r, before);
}
