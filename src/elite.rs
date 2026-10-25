use crate::{Reward,Run,rewards};
pub static EPIC:std::sync::LazyLock<Vec<u32>>=std::sync::LazyLock::new(||include_str!("../scene-assets/epic-weapons.txt").lines().map(|s|s.trim().parse().expect("Invalid epic weapon ID")).collect());
pub const LEGENDARY:&[u32]=&[2140000,3090000,3150000,3170000,4080000,4100000,7100000,12200000,16090000,0x40000000|4200,0x40000000|4210,0x40000000|4220,0x40000000|4361,0x40000000|6110,0x40000000|6720,0x40000000|7090];
pub const BUFFS:&[u32]=&[1000,1010,1020,1030,1040,1060,1070,1080,1090,1150,2060,2130,2180,2200,3000,3040];
pub const TEAR:u32=10020;
pub fn boss_offers(run:&Run,node:usize)->Vec<Reward> {
    let mut items:Vec<_>=LEGENDARY.iter().copied().filter(|id|!run.claimed_items.contains(id)||rewards::lookup(*id).is_some_and(|i|i.kind==rewards::ItemKind::Weapon)).collect();
    items.sort_by_key(|id|(run.claimed_items.contains(id),crate::mix(run.reward_seed()^(node as u64).wrapping_mul(7919)^u64::from(*id)^0xb055)));
    items.into_iter().take(3).map(Reward::Item).collect()
}
pub fn offers(run:&Run,node:usize)->Vec<Reward> {
    let mut result=legendary_offers(run,node);
    let items:Vec<_>=EPIC.iter().chain(LEGENDARY).copied().filter(|id|!run.claimed_items.contains(id)&&rewards::lookup(*id).is_some_and(|i|i.kind.allowed(run.class))).collect();
    let roll=crate::mix(run.reward_seed()^node as u64^0xe91c);
    if !items.is_empty() {
        result[0]=Reward::Item(items[roll as usize%items.len()]);
    }
    result
}
pub(crate) fn legendary_offers(run:&Run,node:usize)->Vec<Reward> {
    let items:Vec<_>=LEGENDARY.iter().copied().filter(|id|!run.claimed_items.contains(id)&&rewards::lookup(*id).is_some_and(|i|i.kind.allowed(run.class))).collect();
    let buffs:Vec<_>=BUFFS.iter().copied().filter(|id|!run.elite_buffs.contains(id)).collect();
    let roll=crate::mix(run.reward_seed()^node as u64^0xe117e);
    vec![if items.is_empty(){Reward::Gold(5000)}else{Reward::Item(items[roll as usize%items.len()])},
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
            let mut run=Run::new(seed,Class::Samurai);
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
