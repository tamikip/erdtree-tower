use crate::{BlessingKind, NodeKind, Reward, RuleResult, Run};

pub struct Story {
    pub title: &'static str,
    pub text: &'static str,
    pub choices: [&'static str; 2],
}
pub const STORIES: [Story; 6] = [
    Story {
        title: "鲜血祭坛",
        text: "祭坛上的指痕尚未干涸。一个声音从石缝中传来：‘黄金早已离去，鲜血仍能作价。’两枚护符静候你的选择。",
        choices: [
            "损失最大生命的 15%，获得鲜血脉动",
            "损失最大生命的 20%，获得蓄势重击",
        ],
    },
    Story {
        title: "流浪祈祷者",
        text: "祈祷者拢紧破旧的斗篷：‘我已走不动下一段路。若你愿分些卢恩，这枚护符便替我陪你走下去。’",
        choices: ["支付 1200 卢恩，获得生命护符", "支付 1200 卢恩，获得耐力护符"],
    },
    Story {
        title: "月光泉",
        text: "月光在泉中分成红与蓝。守泉人的石像刻着一句话：‘投入黄金，只取一捧；贪求两者的人，终会一无所得。’",
        choices: [
            "支付 800 卢恩，恢复 40% 最大生命",
            "支付 500 卢恩，恢复 60% 最大专注",
        ],
    },
    Story {
        title: "遗弃行囊",
        text: "一只行囊压在碎石下，主人已不知去向。纸条上写着：‘后来的人，请取走一份，另一份留给再后来的人。’",
        choices: ["取得 1200 卢恩", "补充 2 瓶圣杯瓶"],
    },
    Story {
        title: "禁书抄写员",
        text: "抄写员合上禁书，露出藏在书脊中的护符：‘知识未必救你。但这两条道路，总有一条能让敌人先倒下。’",
        choices: [
            "支付 1000 卢恩，获得魔法护符",
            "支付 1000 卢恩，获得火焰护符",
        ],
    },
    Story {
        title: "受伤的铁匠",
        text: "铁匠按住伤口，熄灭的炉旁还有余温：‘帮我撑过这一刻，钱袋归你。或者付些报酬，我还能再落几锤。’",
        choices: [
            "损失最大生命的 25%，取得 2000 卢恩",
            "支付 1200 卢恩，右手武器升至背包最高等效等级，再普通 +3 / 失色 +1",
        ],
    },
];

pub fn outcome(story:usize,choice:usize)->&'static str {
    if choice==2{return "你没有接受交易，沿着来路继续前行。";}
    match story {
        0=>"鲜血渗入石槽，一枚护符落入掌心。可在装备菜单自行佩戴。",
        1=>"祈祷者收下卢恩，将护符交给你：愿你走到我未曾抵达的地方。",
        2=>"投入的卢恩沉入泉底，温热的泉水抚平了旅途的疲惫。",
        3=>if choice==0{"你收起钱袋，把补给留给下一位旅人。"}else{"你补充了圣杯瓶，将钱袋放回行囊。"},
        4=>"抄写员收好卢恩，把护符递给你。它的力量需要佩戴后才会生效。",
        _=>if choice==0{"铁匠终于稳住呼吸，将装着卢恩的钱袋推到你身边。"}else{"最后一声锤响落下，铁匠将强化后的武器交还给你。"},
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct StoryEffect {
    pub hp_cost_percent: u32,
    pub heal_percent: u32,
    pub fp_percent: u32,
    pub refill: bool,
}

impl Run {
    pub fn story_index(&self) -> RuleResult<usize> {
        let id = self.active_node(NodeKind::Event)?;
        Ok((crate::mix(self.seed) as usize + id) % STORIES.len())
    }
    pub fn resolve_story(&mut self, choice: usize) -> RuleResult<StoryEffect> {
        let id = self.active_node(NodeKind::Event)?;
        if choice > 2 {
            return Err("无效事件选项".into());
        }
        let mut next = self.clone();
        let mut effect = StoryEffect::default();
        if choice < 2 {
            let mut price = 0;
            match self.story_index()? {
                0 => {
                    effect.hp_cost_percent = if choice == 0 { 15 } else { 20 };
                    next.grant(Reward::Blessing(if choice == 0 {
                        BlessingKind::Bleed
                    } else {
                        BlessingKind::Charge
                    }))?;
                }
                1 => {
                    price = 1200;
                    next.grant(Reward::Blessing(if choice == 0 {
                        BlessingKind::Vigor
                    } else {
                        BlessingKind::Endurance
                    }))?;
                }
                2 => {
                    if choice == 0 {
                        price = 800;
                        effect.heal_percent = 40;
                        if next.hp == next.max_hp {
                            return Err("生命已满".into());
                        }
                    } else {
                        price = 500;
                        effect.fp_percent = 60;
                    }
                }
                3 => {
                    if choice == 0 {
                        next.grant(Reward::Gold(1200))?;
                    } else {
                        if next.flasks == next.flask_limit {
                            return Err("圣杯瓶已满".into());
                        }
                        effect.refill = true;
                        next.flasks = (next.flasks + 2).min(next.flask_limit);
                    }
                }
                4 => {
                    price = 1000;
                    next.grant(Reward::Blessing(if choice == 0 {
                        BlessingKind::Sorcery
                    } else {
                        BlessingKind::Flame
                    }))?;
                }
                _ => {
                    if choice == 0 {
                        effect.hp_cost_percent = 25;
                        next.grant(Reward::Gold(2000))?;
                    } else {
                        price = 1200;
                        next.grant(Reward::UpgradeWeapon)?;
                    }
                }
            }
            if next.gold < price {
                return Err("卢恩不足，事件未消耗".into());
            }
            next.gold -= price;
            let cost = next
                .max_hp
                .saturating_mul(effect.hp_cost_percent)
                .div_ceil(100);
            if next.hp <= cost {
                return Err("生命不足，事件不会致死".into());
            }
            next.hp = (next.hp - cost)
                .saturating_add(next.max_hp * effect.heal_percent / 100)
                .min(next.max_hp);
        }
        next.finish(id);
        *self = next;
        Ok(effect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Class, Phase};
    fn story(index: usize) -> Run {
        for seed in 0..1000 {
            let mut r = Run::new(seed, Class::Samurai);
            r.nodes[0].kind = NodeKind::Event;
            r.phase = Phase::Node { id: 0 };
            r.gold = 5000;
            r.hp = 50;
            r.flasks = 2;
            if r.story_index().unwrap() == index {
                return r;
            }
        }
        panic!("missing story");
    }
    #[test]
    fn events_use_rune_prices_and_failed_payment_leaves_no_reward() {
        let mut r=story(1);r.gold=1199;let old=r.clone();
        assert!(r.resolve_story(0).is_err());assert_eq!(r,old);
        r.gold=1200;r.resolve_story(0).unwrap();assert_eq!(r.gold,0);
        let mut r=story(3);r.gold=0;r.resolve_story(0).unwrap();assert_eq!(r.gold,1200);
        assert!(STORIES.iter().all(|s|s.choices.iter().all(|c|!c.contains("塔币"))));
    }
    #[test]
    fn all_story_choices_resolve_once_and_leave_is_free() {
        for index in 0..STORIES.len() {
            for choice in 0..=2 {
                let mut r = story(index);
                let old = r.clone();
                r.resolve_story(choice).unwrap();
                assert_eq!(r.phase, Phase::Map);
                assert_eq!(r.visited, vec![0]);
                assert!(r.resolve_story(choice).is_err());
                if choice == 2 {
                    assert_eq!((r.gold, r.hp, r.flasks), (old.gold, old.hp, old.flasks));
                }
            }
        }
    }
    #[test]
    fn failed_payment_and_lethal_sacrifice_are_atomic() {
        let mut r = story(1);
        r.gold = 0;
        let old = r.clone();
        assert!(r.resolve_story(0).is_err());
        assert_eq!(r, old);
        let mut r = story(0);
        r.hp = 15;
        let old = r.clone();
        assert!(r.resolve_story(0).is_err());
        assert_eq!(r, old);
    }

    #[test]
    fn capped_blessing_and_full_resources_preserve_event() {
        let mut r = story(0);
        r.blessings.push(crate::Blessing {
            kind: BlessingKind::Bleed,
            level: 3,
        });
        let old = r.clone();
        assert!(r.resolve_story(0).is_err());
        assert_eq!(r, old);
        let mut r = story(2);
        r.hp = r.max_hp;
        let old = r.clone();
        assert!(r.resolve_story(0).is_err());
        assert_eq!(r, old);
        let mut r = story(3);
        r.flasks = r.flask_limit;
        let old = r.clone();
        assert!(r.resolve_story(1).is_err());
        assert_eq!(r, old);
    }
}
