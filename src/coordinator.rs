use crate::{CampChoice, NodeKind, RuleResult, Run};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    Enter(usize),
    Camp(CampChoice),
    ClaimReward(usize),
    OpenChest,
    Buy(usize),
    LeaveShop,
    Event(bool),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameEffect {
    StartEncounter {
        seed: u64,
        node_id: usize,
        kind: NodeKind,
    },
    SetFlasks {
        previous: u8,
        next: u8,
    },
    UpgradeWeapon {
        previous: u8,
        next: u8,
    },
    SetBlessings(Vec<crate::Blessing>),
    SacrificeHealth {
        previous_max: u32,
        next_max: u32,
        next_hp: u32,
    },
}

pub trait GameBridge {
    // On failure, all game effects must be rolled back before returning.
    fn apply_atomic(&mut self, effects: &[GameEffect]) -> RuleResult<()>;
}

pub fn dispatch(run: &mut Run, intent: Intent, bridge: &mut impl GameBridge) -> RuleResult<()> {
    let mut next = run.clone();
    let mut effects = Vec::new();
    match intent {
        Intent::Enter(id) => {
            next.enter(id)?;
            let kind = next.nodes[id].kind;
            if kind.combat() {
                effects.push(GameEffect::StartEncounter {
                    seed: next.seed,
                    node_id: id,
                    kind,
                });
            }
        }
        Intent::Camp(choice) => next.camp(choice)?,
        Intent::ClaimReward(index) => next.claim_reward(index)?,
        Intent::OpenChest => next.open_chest()?,
        Intent::Buy(index) => next.buy(index)?,
        Intent::LeaveShop => next.leave_shop()?,
        Intent::Event(accept) => next.resolve_event(accept)?,
    }
    if next.flasks != run.flasks {
        effects.push(GameEffect::SetFlasks {
            previous: run.flasks,
            next: next.flasks,
        });
    }
    if next.weapon_level != run.weapon_level {
        effects.push(GameEffect::UpgradeWeapon {
            previous: run.weapon_level,
            next: next.weapon_level,
        });
    }
    if next.blessings != run.blessings {
        effects.push(GameEffect::SetBlessings(next.blessings.clone()));
    }
    if next.max_hp != run.max_hp {
        effects.push(GameEffect::SacrificeHealth {
            previous_max: run.max_hp,
            next_max: next.max_hp,
            next_hp: next.hp,
        });
    }
    bridge.apply_atomic(&effects)?;
    *run = next;
    Ok(())
}
