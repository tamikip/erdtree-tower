use serde::{Deserialize, Serialize};
use crate::{NodeKind, Phase, RuleResult, Run};

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    #[serde(default)] pub trial:bool,
    pub opened: u8, pub guards: bool,
    #[serde(default)] pub guard_slot:Option<u8>,
    #[serde(default)] pub locked: bool,
    #[serde(default)] pub receipt: Option<Receipt>,
    #[serde(default)] pub scarabs: bool,
    #[serde(default)] pub attempts: u8,
    #[serde(default)] pub descent: bool,
    #[serde(default)] pub descent_started: bool,
    #[serde(default)] pub recovery: bool,
    #[serde(default)] pub stormveil: Option<crate::stormveil::Progress>,
    #[serde(default)] pub chariot: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt { pub item:u32, pub key_target:u64, pub reward_target:u64 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome { Explosion, Guards, Item }
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum Trial {Descent,Stormveil,Chariot,Skip}
pub fn chest_roll(roll: u64) -> Outcome {
    match roll % 100 { 0..=19 => Outcome::Explosion, 20..=39 => Outcome::Guards, _ => Outcome::Item }
}
pub fn kind(seed: u64, node: usize) -> u8 { (crate::mix(seed ^ node as u64) % 2) as u8 }
pub fn descent_reward(seed:u64,node:usize)->u32 {
    let pool:Vec<_>=crate::elite::LEGENDARY.iter().copied().filter(|id|*id>>28==0).collect();
    pool[crate::mix(seed^(node as u64).wrapping_mul(7919)^0xde5ce17) as usize%pool.len()]
}
pub fn outcome(seed: u64, node: usize, chest: usize) -> Outcome {
    chest_roll(crate::mix(seed ^ (node as u64).wrapping_mul(7919) ^ (chest as u64 + 1).wrapping_mul(104729)))
}
impl Run {
    pub fn choose_trial(&mut self,choice:Trial)->RuleResult<()> {
        self.active_node(NodeKind::Event)?;
        if !self.regional_event.trial {return Err("试炼已经选择".into());}
        let gold=if choice==Trial::Skip {self.gold.checked_add(800).ok_or("卢恩数量超限")?}else{self.gold};
        self.regional_event=Progress::default();
        match choice {
            Trial::Descent=>{self.regional_event.descent=true;self.regional_event.descent_started=true;},
            Trial::Stormveil|Trial::Chariot=>{
                self.regional_event.stormveil=Some(crate::stormveil::Progress::Running);
                self.regional_event.chariot=choice==Trial::Chariot;
            },
            Trial::Skip=>{self.gold=gold;self.regional_event.stormveil=Some(crate::stormveil::Progress::Skipped);}
        }
        Ok(())
    }
    pub fn begin_chariot_reward(&mut self,item:u32,owned:u64)->RuleResult<()> {
        self.active_node(NodeKind::Event)?;
        let p=&mut self.regional_event;
        if !p.chariot||p.stormveil!=Some(crate::stormveil::Progress::Complete)||p.opened!=0||p.receipt.is_some(){return Err("挑战尚未完成或奖励已经领取".into());}
        if item>>28!=0||!crate::elite::LEGENDARY.contains(&crate::affixes::base(item)){return Err("宝箱奖励必须是传说武器".into());}
        p.receipt=Some(Receipt{item,key_target:0,reward_target:owned.checked_add(1).ok_or("物品数量异常")?});Ok(())
    }
    pub fn complete_chariot_reward(&mut self)->RuleResult<()> {
        self.active_node(NodeKind::Event)?;
        let p=&mut self.regional_event;
        if !p.chariot||p.stormveil!=Some(crate::stormveil::Progress::Complete)||p.receipt.is_none(){return Err("尚未领取挑战奖励".into());}
        p.opened=1;p.receipt=None;Ok(())
    }
    pub fn accept_descent(&mut self)->RuleResult<()> {
        self.active_node(NodeKind::Event)?;
        if !self.regional_event.descent||self.regional_event.descent_started||self.regional_event.opened!=0 {return Err("当前无法参加试炼".into());}
        self.regional_event.descent_started=true;Ok(())
    }
    pub fn recover_at_spring(&mut self,choice:usize)->RuleResult<()> {
        self.active_node(NodeKind::Event)?;
        if !self.regional_event.recovery||self.regional_event.opened!=0||choice>1 {return Err("泉水已使用或选项无效".into());}
        self.regional_event.opened=1<<choice;
        if choice==0 {self.hp=self.max_hp;}
        Ok(())
    }
    pub fn begin_descent(&mut self,item:u32,owned:u64)->RuleResult<()> {
        self.active_node(NodeKind::Event)?;
        if !self.regional_event.descent||!self.regional_event.descent_started||self.regional_event.opened!=0||self.regional_event.receipt.is_some(){return Err("试炼未开始或奖励已领取".into());}
        if item>>28!=0||!crate::elite::LEGENDARY.contains(&crate::affixes::base(item)){return Err("试炼奖励必须是传说武器".into());}
        self.regional_event.receipt=Some(Receipt{item,key_target:0,reward_target:owned.checked_add(1).ok_or("物品数量异常")?});Ok(())
    }
    pub fn complete_descent(&mut self)->RuleResult<()> {
        self.active_node(NodeKind::Event)?;
        if !self.regional_event.descent||self.regional_event.receipt.is_none(){return Err("尚未领取试炼奖励".into());}
        self.regional_event.opened=1;self.regional_event.receipt=None;Ok(())
    }
    pub fn pay_scarabs(&mut self, runes:u32)->RuleResult<u32> {
        self.active_node(NodeKind::Event)?;
        if !self.regional_event.scarabs || self.regional_event.attempts>=3 {return Err("本次游玩次数已用完".into());}
        let remaining=runes.checked_sub(1000).ok_or("需要 1000 卢恩")?;
        self.regional_event.attempts+=1;
        self.gold=remaining;
        Ok(remaining)
    }
    pub fn begin_locked(&mut self,item:u32,keys:u64,owned:u64)->RuleResult<()> {
        self.active_node(NodeKind::Event)?;
        if !self.regional_event.locked || self.regional_event.opened!=0 || self.regional_event.receipt.is_some() {return Err("宝箱已开启".into());}
        if keys==0 {return Err("需要石剑钥匙 ×1".into());}
        let reward_target=owned.checked_add(1).ok_or("物品数量异常")?;
        self.regional_event.receipt=Some(Receipt{item,key_target:keys-1,reward_target});Ok(())
    }
    pub fn complete_locked(&mut self)->RuleResult<()> {
        self.active_node(NodeKind::Event)?;
        if !self.regional_event.locked || self.regional_event.receipt.is_none(){return Err("宝箱尚未解锁".into());}
        self.regional_event.opened=1;self.regional_event.receipt=None;Ok(())
    }
    pub fn open_regional_event(&mut self, slot: usize) -> RuleResult<Outcome> {
        let node=self.active_node(NodeKind::Event)?;
        if self.regional_event.trial || self.regional_event.locked || self.regional_event.scarabs || self.regional_event.descent || self.regional_event.recovery || self.regional_event.stormveil.is_some() {return Err("请与事件交互物交互".into());}
        let altar=kind(self.seed,node)==1;
        if slot >= 3 || self.regional_event.guards || (altar && self.regional_event.opened != 0) || self.regional_event.opened & (1 << slot) != 0 {
            return Err("此处已领取，或需先击败守卫".into());
        }
        let result=if altar {if slot==1 {Outcome::Guards}else{Outcome::Item}} else {outcome(self.seed,node,slot)};
        self.regional_event.opened |= 1 << slot;
        self.regional_event.guards=result==Outcome::Guards;
        self.regional_event.guard_slot=(result==Outcome::Guards).then_some(slot as u8);
        Ok(result)
    }
    pub fn leave_regional_event(&mut self) -> RuleResult<()> {
        if self.regional_event.trial {return Err("请先完成试炼选择".into());}
        let node=self.active_node(NodeKind::Event)?;
        if self.regional_event.chariot&&self.regional_event.stormveil==Some(crate::stormveil::Progress::Complete)&&self.regional_event.opened==0{return Err("请先领取传说宝箱".into());}
        if matches!(self.regional_event.stormveil,Some(crate::stormveil::Progress::Choosing|crate::stormveil::Progress::Running)){return Err("请先完成队伍选择或抵达终点".into());}
        if self.regional_event.guards || self.regional_event.receipt.is_some() || (self.regional_event.descent_started&&self.regional_event.opened==0) {return Err("当前事件尚未完成".into());}
        self.finish(node);Ok(())
    }
    pub(crate) fn valid_regional_event(&self) -> bool {
        if self.regional_event.trial {
            return self.regional_event==Progress{trial:true,..Progress::default()}
                &&match self.phase {Phase::Dead=>true,Phase::Node{id}=>self.nodes.get(id).is_some_and(|n|n.kind==NodeKind::Event),_=>false};
        }
        if self.regional_event.chariot&&self.regional_event.stormveil.is_none(){return false;}
        if self.regional_event.stormveil.is_some() {
            let p=&self.regional_event;
            let reward=if p.chariot&&p.stormveil==Some(crate::stormveil::Progress::Complete) {
                p.opened<=1&&(p.opened==0||p.receipt.is_none())&&p.receipt.as_ref().is_none_or(|r|r.key_target==0&&r.reward_target>0&&r.item>>28==0&&crate::elite::LEGENDARY.contains(&crate::affixes::base(r.item)))
            }else{p.receipt.is_none()&&p.opened==0};
            return !p.locked&&!p.scarabs&&!p.descent&&!p.descent_started&&!p.recovery&&!p.guards&&p.guard_slot.is_none()&&reward&&p.attempts==0
                &&match self.phase {Phase::Dead=>true,Phase::Node{id}=>self.nodes.get(id).is_some_and(|n|n.kind==NodeKind::Event),_=>false};
        }
        if self.regional_event.descent_started&&!self.regional_event.descent{return false;}
        if self.regional_event.recovery {
            let p=&self.regional_event;
            return !p.locked&&!p.scarabs&&!p.descent&&!p.guards&&p.guard_slot.is_none()&&p.receipt.is_none()&&p.attempts==0&&p.opened<=2
                &&match self.phase {Phase::Dead=>true,Phase::Node{id}=>self.nodes.get(id).is_some_and(|n|n.kind==NodeKind::Event),_=>false};
        }
        if self.regional_event.descent {
            let p=&self.regional_event;
            return !p.locked&&!p.scarabs&&!p.guards&&p.guard_slot.is_none()&&p.attempts==0&&p.opened<=1
                &&(p.opened==0||p.receipt.is_none())&&p.receipt.as_ref().is_none_or(|r|r.key_target==0&&r.reward_target>0&&r.item>>28==0&&crate::elite::LEGENDARY.contains(&crate::affixes::base(r.item)))
                &&match self.phase {Phase::Dead=>true,Phase::Node{id}=>self.nodes.get(id).is_some_and(|n|n.kind==NodeKind::Event),_=>false};
        }
        if self.regional_event.guard_slot.is_some_and(|s|s>=3||!self.regional_event.guards||self.regional_event.opened&(1<<s)==0){return false;}
        if self.regional_event.scarabs {
            return !self.regional_event.locked && !self.regional_event.guards && self.regional_event.opened==0
                && self.regional_event.receipt.is_none() && self.regional_event.attempts<=3
                && match self.phase {Phase::Dead=>true,Phase::Node{id}=>self.nodes.get(id).is_some_and(|n|n.kind==NodeKind::Event),_=>false};
        }
        if self.regional_event.attempts!=0 {return false;}
        if self.regional_event.locked {
            return !self.regional_event.guards && self.regional_event.opened<=1
                && (self.regional_event.opened==0 || self.regional_event.receipt.is_none())
                && self.regional_event.receipt.as_ref().is_none_or(|r|r.reward_target>0 && (r.item==0x40002724 || crate::rewards::lookup(r.item).is_some_and(|i|matches!(i.kind,crate::rewards::ItemKind::Weapon|crate::rewards::ItemKind::Crystal))))
                && match self.phase {Phase::Dead=>true,Phase::Node{id}=>self.nodes.get(id).is_some_and(|n|n.kind==NodeKind::Event),_=>false};
        }
        if self.regional_event.receipt.is_some(){return false;}
        if self.regional_event==Progress::default(){return true;}
        match self.phase {
            Phase::Dead => self.regional_event.opened<=7,
            Phase::Node{id} if self.nodes.get(id).is_some_and(|n|n.kind==NodeKind::Event) => {
                self.regional_event.opened<=7
            }
            _=>false,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spring_is_free_personal_exclusive_and_survives_reload() {
        let mut run=Run::new(42,crate::Class::Samurai);
        let id=run.nodes.iter().position(|n|n.kind==NodeKind::Event).unwrap();
        run.phase=Phase::Node{id};run.regional_event.recovery=true;run.gold=0;run.hp=1;
        let original=run.clone();
        for choice in 0..2 {
            let mut player=original.clone();player.recover_at_spring(choice).unwrap();
            assert_eq!(player.gold,0);assert_eq!(player.flasks,original.flasks);
            assert_eq!(player.hp,if choice==0{player.max_hp}else{1});
            assert!(player.valid_regional_event());
            let mut loaded:Run=serde_json::from_str(&serde_json::to_string(&player).unwrap()).unwrap();
            assert!(loaded.recover_at_spring(1-choice).is_err());
            assert!(loaded.open_regional_event(0).is_err());
            loaded.leave_regional_event().unwrap();assert_eq!(loaded.regional_event,Progress::default());
        }
        assert_eq!(original.regional_event.opened,0);
        assert!(run.recover_at_spring(2).is_err());
    }
    #[test]
    fn descent_reward_is_persisted_once_and_exit_requires_claim() {
        let mut run=Run::new(42,crate::Class::Samurai);
        let id=run.nodes.iter().position(|n|n.kind==NodeKind::Event).unwrap();
        run.phase=Phase::Node{id};run.regional_event.descent=true;
        let mut skipped=run.clone();skipped.leave_regional_event().unwrap();
        assert!(run.begin_descent(3150000,2).is_err());
        run.accept_descent().unwrap();assert!(run.accept_descent().is_err());
        run=serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
        assert!(run.regional_event.descent_started);
        assert!(run.leave_regional_event().is_err());
        assert!(run.open_regional_event(0).is_err());
        assert!(run.begin_descent(0x40002724,2).is_err());
        for seed in 0..1000 {let item=descent_reward(seed,id);assert_eq!(item>>28,0);assert!(crate::elite::LEGENDARY.contains(&item));}
        run.begin_descent(3150000,2).unwrap();
        let saved=serde_json::to_string(&run.regional_event).unwrap();
        run.regional_event=serde_json::from_str(&saved).unwrap();
        assert!(run.valid_regional_event());
        assert_eq!(run.regional_event.receipt.as_ref().unwrap().reward_target,3);
        assert!(run.begin_descent(3150000,3).is_err());
        run.complete_descent().unwrap();assert!(run.valid_regional_event());
        assert!(run.begin_descent(3150000,3).is_err());
        run.leave_regional_event().unwrap();assert_eq!(run.regional_event,Progress::default());
    }
    #[test]
    fn scarab_payment_is_exact_and_three_attempts_survive_reload() {
        let mut run=Run::new(42,crate::Class::Samurai);
        let id=run.nodes.iter().position(|n|n.kind==NodeKind::Event).unwrap();
        run.phase=Phase::Node{id};run.regional_event.scarabs=true;
        assert!(run.pay_scarabs(999).is_err());assert_eq!(run.regional_event.attempts,0);
        for attempt in 1..=3 {
            assert_eq!(run.pay_scarabs(4000-(attempt-1)*1000).unwrap(),4000-attempt*1000);
            run.regional_event=serde_json::from_str(&serde_json::to_string(&run.regional_event).unwrap()).unwrap();
            assert_eq!(run.regional_event.attempts,attempt as u8);assert!(run.valid_regional_event());
        }
        assert!(run.pay_scarabs(10000).is_err());assert!(run.open_regional_event(0).is_err());
        run.leave_regional_event().unwrap();assert_eq!(run.regional_event,Progress::default());
    }
    #[test]
    fn altar_choices_are_exclusive_and_only_combat_summons_guards() {
        let mut run=Run::new(42,crate::Class::Samurai);
        let id=run.nodes.iter().enumerate().find(|(id,n)|n.kind==NodeKind::Event && kind(run.seed,*id)==1).unwrap().0;
        run.phase=Phase::Node{id};
        for slot in 0..3 {
            let mut chosen=run.clone();
            assert_eq!(chosen.open_regional_event(slot).unwrap(),if slot==1 {Outcome::Guards}else{Outcome::Item});
            assert_eq!(chosen.regional_event.guards,slot==1);
            assert!(chosen.valid_regional_event());
            for other in 0..3 {assert!(chosen.open_regional_event(other).is_err());}
            if slot==1 {assert!(chosen.leave_regional_event().is_err());chosen.regional_event.guards=false;}
            for other in 0..3 {assert!(chosen.open_regional_event(other).is_err());}
            chosen.leave_regional_event().unwrap();
        }
    }
    #[test]
    fn locked_chest_receipt_is_single_use_and_survives_reload() {
        let mut run=Run::new(42,crate::Class::Samurai);
        let id=run.nodes.iter().position(|n|n.kind==NodeKind::Event).unwrap();
        run.phase=Phase::Node{id};run.regional_event.locked=true;
        assert!(run.open_regional_event(0).is_err());
        assert!(run.begin_locked(0x40002724,0,0).is_err());
        run.begin_locked(0x40002724,2,3).unwrap();
        assert!(run.begin_locked(0x40002724,2,3).is_err());
        assert!(run.leave_regional_event().is_err());
        let saved=serde_json::to_string(&run.regional_event).unwrap();
        run.regional_event=serde_json::from_str(&saved).unwrap();
        let receipt=run.regional_event.receipt.as_ref().unwrap();
        assert_eq!((receipt.key_target,receipt.reward_target),(1,4));
        run.complete_locked().unwrap();
        assert!(run.begin_locked(0x40002724,2,3).is_err());
        run.leave_regional_event().unwrap();
        assert_eq!(run.regional_event,Progress::default());
        assert_eq!(serde_json::from_str::<Progress>(r#"{"opened":0,"guards":false}"#).unwrap(),Progress::default());
    }
    #[test]
    fn exact_chest_probabilities_and_stable_rolls() {
        for (result,count) in [(Outcome::Explosion,20),(Outcome::Guards,20),(Outcome::Item,60)] {
            assert_eq!((0..100).filter(|&n|chest_roll(n)==result).count(),count);
        }
        assert_eq!(outcome(42,12,0),outcome(42,12,0));
    }
    #[test]
    fn chests_and_altar_cannot_be_farmed_or_escape_guards() {
        for seed in 0..30 {
            let mut run=Run::new(seed,crate::Class::Samurai);
            let id=run.nodes.iter().position(|n|n.kind==NodeKind::Event).unwrap();
            run.phase=Phase::Node{id};
            let altar=kind(seed,id)==1;
            for slot in 0..if altar {1}else{3} {
                let result=run.open_regional_event(slot).unwrap();
                assert_eq!(run.regional_event.guard_slot,(result==Outcome::Guards).then_some(slot as u8));
                assert!(run.open_regional_event(slot).is_err());
                let saved=serde_json::to_string(&run.regional_event).unwrap();
                assert_eq!(serde_json::from_str::<Progress>(&saved).unwrap(),run.regional_event);
                if result==Outcome::Guards {assert!(run.leave_regional_event().is_err());run.regional_event.guards=false;run.regional_event.guard_slot=None;}
            }
            run.leave_regional_event().unwrap();
            assert_eq!(run.regional_event,Progress::default());
        }
    }
    #[test]
    fn active_event_progress_survives_full_save_and_legacy_saves_default_empty() {
        let mut run=Run::new(26,crate::Class::Samurai);
        loop {
            let id=run.available()[0];run.enter(id).unwrap();
            match run.nodes[id].kind {
                NodeKind::Event=>break,
                NodeKind::Monster|NodeKind::Elite|NodeKind::Boss=>{run.win_combat().unwrap();run.decline_reward().unwrap();},
                NodeKind::Camp=>run.leave_camp().unwrap(),NodeKind::Shop=>run.leave_shop().unwrap(),NodeKind::Chest=>run.finish_chest().unwrap(),
            }
        }
        run.regional_event=Progress::default();
        run.open_regional_event(0).unwrap();
        let mut loaded=Run::decode(&run.encode().unwrap()).unwrap();
        assert_eq!(loaded,run);
        assert!(loaded.open_regional_event(0).is_err());
        let original=Run::new(1,crate::Class::Samurai);
        let mut json=serde_json::to_value(&original).unwrap();
        json.as_object_mut().unwrap().remove("regional_event");
        assert_eq!(Run::decode(&json.to_string()).unwrap(),original);
    }
}
