use erdtree_tower::{coordinator::*, *};

#[derive(Default)]
struct RecordingBridge {
    fail: bool,
    calls: Vec<Vec<GameEffect>>,
}

impl GameBridge for RecordingBridge {
    fn apply_atomic(&mut self, effects: &[GameEffect]) -> RuleResult<()> {
        self.calls.push(effects.to_vec());
        if self.fail {
            Err("game operation failed".into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn failed_encounter_creation_preserves_route_and_checkpoint() {
    let mut r = Run::new(42, Class::Samurai);
    let before = r.clone();
    let mut bridge = RecordingBridge {
        fail: true,
        ..Default::default()
    };
    assert!(dispatch(&mut r, Intent::Enter(0), &mut bridge).is_err());
    assert_eq!(r, before);
    assert_eq!(
        bridge.calls[0],
        vec![GameEffect::StartEncounter {
            seed: 42,
            node_id: 0,
            kind: NodeKind::Monster,
        }]
    );
}

#[test]
fn failed_refill_preserves_camp_choice() {
    let mut r = Run::new(42, Class::Samurai);
    r.flasks = 2;
    r.phase = Phase::Node { id: r.nodes.iter().position(|n|n.kind==NodeKind::Camp).unwrap() };
    let before = r.clone();
    let mut bridge = RecordingBridge {
        fail: true,
        ..Default::default()
    };
    assert!(dispatch(&mut r, Intent::Camp(CampChoice::Refill), &mut bridge).is_err());
    assert_eq!(r, before);
    assert_eq!(
        bridge.calls[0],
        vec![GameEffect::SetFlasks {
            previous: 2,
            next: 5
        }]
    );
    bridge.fail = false;
    dispatch(&mut r, Intent::Camp(CampChoice::Refill), &mut bridge).unwrap();
    assert_eq!((r.phase, r.flasks), (Phase::Map, 5));
}

#[test]
fn failed_upgrade_does_not_charge_shop() {
    let mut r = Run::new(42, Class::Samurai);
    r.gold = 200;
    r.phase = Phase::Node { id: r.nodes.iter().position(|n|n.kind==NodeKind::Shop).unwrap() };
    let before = r.clone();
    let mut bridge = RecordingBridge {
        fail: true,
        ..Default::default()
    };
    assert!(dispatch(&mut r, Intent::Buy(1), &mut bridge).is_err());
    assert_eq!(r, before);
    assert_eq!(
        bridge.calls[0],
        vec![GameEffect::UpgradeWeapon {
            previous: 0,
            next: 1
        }]
    );
}

#[test]
fn invalid_choice_never_calls_game_bridge() {
    let mut r = Run::new(42, Class::Samurai);
    let mut bridge = RecordingBridge::default();
    assert!(dispatch(&mut r, Intent::Enter(33), &mut bridge).is_err());
    assert!(bridge.calls.is_empty());
}

#[test]
fn event_effects_are_one_transaction() {
    let mut r = Run::new(42, Class::Samurai);
    r.phase = Phase::Node { id: r.nodes.iter().position(|n|n.kind==NodeKind::Event).unwrap() };
    let mut bridge = RecordingBridge::default();
    dispatch(&mut r, Intent::Event(true), &mut bridge).unwrap();
    assert_eq!(bridge.calls.len(), 1);
    assert_eq!(bridge.calls[0].len(), 2);
    assert!(matches!(bridge.calls[0][0], GameEffect::SetBlessings(_)));
    assert_eq!(
        bridge.calls[0][1],
        GameEffect::SacrificeHealth {
            previous_max: 100,
            next_max: 85,
            next_hp: 85,
        }
    );
}
