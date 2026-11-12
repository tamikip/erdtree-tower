use crate::{Reward,Run,rewards};
pub static EPIC:std::sync::LazyLock<Vec<u32>>=std::sync::LazyLock::new(||include_str!("../scene-assets/epic-weapons.txt").lines().map(|s|s.trim().parse().expect("Invalid epic weapon ID")).collect());
pub const LEGENDARY:&[u32]=&[2140000,3090000,3150000,3170000,4080000,4100000,7100000,12200000,16090000,0x40000000|4200,0x40000000|4210,0x40000000|4220,0x40000000|4361,0x40000000|6110,0x40000000|6720,0x40000000|7090];
pub const BUFFS:&[u32]=&[1000,1010,1020,1030,1040,1060,1070,1080,1090,1150,2060,2130,2180,2200,3000,3040];
pub const TEAR:u32=10020;
/// Boss 奖励规则 2（新旅程）：第一格的保底改为从全部武器里抽一把（照普通奖励的规则：约七成是职业擅长的类型，有武器偏好时按偏好），
/// 品质固定为传说（affixes::rewards 对 Boss 第一格保底传说）；不再限定那 9 件传说装备。没拿过的武器优先
fn boss_weapon(run:&Run,node:usize)->u32 {
    let all:Vec<u32>=rewards::CATALOG.iter().filter(|i|i.kind==rewards::ItemKind::Weapon&&i.random_eligible()).map(|i|i.id).collect();
    let owned:std::collections::HashSet<u32>=run.claimed_items.iter().map(|id|crate::affixes::base(*id)).collect();
    let fresh:Vec<u32>=all.iter().copied().filter(|id|!owned.contains(id)).collect();
    let mut pool=if fresh.is_empty(){all}else{fresh};
    let roll=crate::mix(run.reward_seed()^(node as u64).wrapping_mul(7919)^0xb055_7e9a);
    if !run.preferences().is_empty() {return pool[crate::weapon_training::pick_many(&pool,roll,run.preferences(),Some(run.class))];}
    if crate::mix(roll^0xc1a5)%10<7 {
        let favored:Vec<_>=pool.iter().copied().filter(|id|run.class.weapon_types().contains(&crate::weapon_training::kind(*id))).collect();
        if !favored.is_empty() {pool=favored;}
    }
    pool[(roll>>8) as usize%pool.len()]
}
pub fn boss_offers(run:&Run,node:usize)->Vec<Reward> {
    if run.boss_reward_rules==0 {return old_boss_offers(run,node);}
    let items=old_boss_items(run,node);
    let mut guaranteed=items.iter().copied().find(|id|run.casting_tools.is_none_or(|tools|rewards::castable(*id,tools))).expect("Boss pool contains repeatable weapons");
    if run.boss_reward_rules>=2 {guaranteed=boss_weapon(run,node);}
    else if !run.preferences().is_empty()&&guaranteed>>28==0 {
        let owned=run.claimed_items.contains(&guaranteed);
        let weapons:Vec<_>=items.into_iter().filter(|id|id>>28==0&&run.claimed_items.contains(id)==owned).collect();
        guaranteed=weapons[crate::weapon_training::pick_many(&weapons,crate::mix(run.reward_seed()^node as u64^0xb055),run.preferences(),None)];
    }
    let mut result=vec![Reward::Item(guaranteed)];
    for reward in rewards::boss_regular_offers(run,node) {
        if result.len()==3 {break;}
        if !result.contains(&reward){result.push(reward);}
    }
    while result.len()<3 {result.push(Reward::Gold(1500));}
    result
}
fn old_boss_items(run:&Run,node:usize)->Vec<u32> {
    let mut items:Vec<_>=LEGENDARY.iter().copied().filter(|id|crate::dlc::item_allowed(*id)).filter(|id|!run.claimed_items.contains(id)||rewards::lookup(*id).is_some_and(|i|i.kind==rewards::ItemKind::Weapon)).collect();
    items.sort_by_key(|id|(run.claimed_items.contains(id),crate::mix(run.reward_seed()^(node as u64).wrapping_mul(7919)^u64::from(*id)^0xb055)));
    items
}
fn old_boss_offers(run:&Run,node:usize)->Vec<Reward> {
    old_boss_items(run,node).into_iter().take(3).map(Reward::Item).collect()
}
pub fn offers(run:&Run,node:usize)->Vec<Reward> {
    let mut result=legendary_offers(run,node);
    let items:Vec<_>=EPIC.iter().chain(LEGENDARY).copied().filter(|id|crate::dlc::item_allowed(*id)&&!run.claimed_items.contains(id)&&rewards::lookup(*id).is_some_and(|i|i.kind.allowed(run.class)&&!(run.law.lone_wolf()&&i.kind==rewards::ItemKind::Spirit))).collect();
    let roll=crate::mix(run.reward_seed()^node as u64^0xe91c);
    if !items.is_empty() {
        result[0]=Reward::Item(crate::weapon_training::reward(run,&items,roll));
    }
    result
}
pub(crate) fn legendary_offers(run:&Run,node:usize)->Vec<Reward> {
    let items:Vec<_>=LEGENDARY.iter().copied().filter(|id|crate::dlc::item_allowed(*id)&&!run.claimed_items.contains(id)&&rewards::lookup(*id).is_some_and(|i|i.kind.allowed(run.class)&&!(run.law.lone_wolf()&&i.kind==rewards::ItemKind::Spirit))).collect();
    let buffs:Vec<_>=BUFFS.iter().copied().filter(|id|!run.elite_buffs.contains(id)).collect();
    let roll=crate::mix(run.reward_seed()^node as u64^0xe117e);
    vec![if items.is_empty(){Reward::Gold(5000)}else{Reward::Item(crate::weapon_training::reward(run,&items,roll))},
        if buffs.is_empty(){Reward::Gold(5000)}else{Reward::TalismanBuff(buffs[(roll>>32) as usize%buffs.len()])},Reward::SacredTear]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_elite_page_migrates_and_new_page_roundtrips() {
        use crate::{Class,NodeKind,Phase};
        let mut checked=0;
        for seed in 0..40 {
            let mut run=Run::new(seed,Class::Samurai);run.ember_rewards=false;run.stone_rewards=false;
            for _ in 0..11 {
                let id=run.available()[0];run.enter(id).unwrap();
                match run.nodes[id].kind {
                    NodeKind::Elite=>{
                        run.win_combat().unwrap();
                        run.phase=Phase::Reward{id,offers:legendary_offers(&run,id)};
                        let loaded=Run::decode(&run.encode().unwrap()).unwrap();
                        assert_eq!(loaded.phase,Phase::Reward{id,offers:rewards::offers(&run,id)});
                        assert_eq!(Run::decode(&loaded.encode().unwrap()).unwrap(),loaded);
                        checked+=1;break;
                    },
                    NodeKind::Monster=>{run.win_combat().unwrap();run.decline_reward().unwrap();},
                    NodeKind::Camp=>run.leave_camp().unwrap(),NodeKind::Shop=>run.leave_shop().unwrap(),
                    NodeKind::Chest=>run.finish_chest().unwrap(),NodeKind::Event=>run.resolve_event(false).unwrap(),_=>break,
                }
            }
        }
        assert!(checked>0);
    }
}
