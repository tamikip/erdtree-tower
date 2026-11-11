// 调试菜单的规则部分：只在路线图界面使用，用过的旅程打上 debug 标记（放宽誓约进度校验，不计入律法纪录）
use crate::{Phase,Run,RuleResult,contracts::{Kind,Oath,MAX_LEVEL},embers::Ember,law::Law};

#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
pub struct BossTest {pub encounter:u16,pub act:u8}

impl Run {
    pub fn debug_event_test(&mut self,draw:crate::events::Draw)->RuleResult<usize> {
        let mut next=self.clone();next.debug_begin()?;
        if let Some(contract)=next.contract.as_mut(){if contract.pending(){contract.skip()?;}}
        let node=next.nodes.iter().position(|n|n.kind==crate::NodeKind::Event).ok_or("本幕没有问号房")?;
        next.debug_jump(node)?;next.boss_test=None;next.mausoleum_test=None;
        next.event_test=Some((next.act,node,draw));next.validate()?;*self=next;Ok(node)
    }
    pub fn debug_mausoleum_test(&mut self)->RuleResult<usize> {
        let mut next=self.clone();next.debug_begin()?;
        if let Some(contract)=next.contract.as_mut(){if contract.pending(){contract.skip()?;}}
        let node=next.nodes.iter().position(|n|n.kind==crate::NodeKind::Event).ok_or("本幕没有问号房")?;
        next.debug_jump(node)?;next.boss_test=None;next.event_test=None;next.mausoleum_test=Some((next.act,node));
        next.validate()?;*self=next;Ok(node)
    }
    pub fn debug_boss_test(&mut self,encounter:u16,act:u8)->RuleResult<usize> {
        if encounter>=4096{return Err("Boss 测试编号无效".into());}
        let mut next=self.clone();
        next.debug_act(act)?;
        if let Some(contract)=next.contract.as_mut(){contract.skip()?;}
        let node=next.nodes.iter().position(|n|n.kind==crate::NodeKind::Boss).ok_or("本幕没有 Boss 节点")?;
        next.debug_jump(node)?;
        next.boss_test=Some(BossTest{encounter,act});next.validate()?;
        *self=next;Ok(node)
    }
    fn debug_begin(&mut self)->RuleResult<()> {
        if self.phase!=Phase::Map {return Err("请在路线图界面使用调试功能".into());}
        self.debug=true;Ok(())
    }
    /// 把路线改成刚好能进入目标节点：沿路线图找一条通往它的路径，路径上的节点记为已走过
    pub fn debug_jump(&mut self,target:usize)->RuleResult<()> {
        if target>=self.nodes.len() {return Err("节点不存在".into());}
        self.debug_begin()?;
        let starts=self.start_nodes();
        let path=if starts.contains(&target) {vec![]} else {
            let mut previous=vec![None;self.nodes.len()];let mut queue:std::collections::VecDeque<_>=starts.iter().copied().collect();
            let mut seen:std::collections::HashSet<_>=starts.iter().copied().collect();let mut found=None;
            while let Some(id)=queue.pop_front() {
                if self.nodes[id].next.contains(&target) {found=Some(id);break;}
                for &next in &self.nodes[id].next {if seen.insert(next) {previous[next]=Some(id);queue.push_back(next);}}
            }
            let mut path=vec![found.ok_or("本幕路线到不了这个节点")?];
            while let Some(before)=previous[*path.last().unwrap()] {path.push(before);}
            path.reverse();path
        };
        self.visited=path;
        self.reached_layer=self.visited.last().map_or(0,|&id|self.nodes[id].layer+1);
        self.regional_event=Default::default();self.shop_purchased=0;self.shop_stock=None;self.checkpoint=None;
        self.validate()
    }
    /// 切换到指定幕的起点：按初始种子推算该幕的路线，生命与圣杯瓶回满，誓约按新一幕重新出卡
    pub fn debug_act(&mut self,act:u8)->RuleResult<()> {
        if !(1..=3).contains(&act) {return Err("幕数无效".into());}
        let mut seed=self.initial_seed.ok_or("旧存档没有初始种子，不能切换幕")?;
        self.debug_begin()?;
        for step in 2..=act {seed=crate::mix(seed^u64::from(step));}
        if act<self.act {self.larval_gift=Default::default();}
        self.act=act;self.seed=seed;self.schema=7;self.nodes=crate::law_graph(seed,self.schema,self.law);
        self.visited.clear();self.reached_layer=0;self.phase=Phase::Map;
        self.hp=self.max_hp;self.flasks=self.flask_limit;
        self.regional_event=Default::default();self.shop_purchased=0;self.shop_stock=None;self.checkpoint=None;
        if let Some(old)=&self.contract {self.contract=Some(old.next_act(self.reward_seed(),self.class,act));}
        self.validate()
    }
    /// 设置誓约等级（0 为移除）；旧规则的誓约先转成新规则
    pub fn debug_oath(&mut self,kind:Kind,level:u8)->RuleResult<()> {
        if level>MAX_LEVEL {return Err("誓约等级无效".into());}
        self.debug_begin()?;
        self.enable_contracts();
        let contract=self.contract.as_mut().unwrap();
        if contract.rules==0 {
            contract.rules=1;contract.held=contract.chosen.map(|kind|Oath{kind,level:1}).into_iter().collect();
        }
        contract.held.retain(|o|o.kind!=kind);
        if level>0 {contract.held.push(Oath{kind,level});}
        contract.deepen=contract.held.iter().rev().filter(|o|o.level<MAX_LEVEL).max_by_key(|o|o.level).map(|o|o.kind);
        self.validate()
    }
    pub fn debug_ember(&mut self,ember:Ember,held:bool)->RuleResult<()> {
        self.debug_begin()?;
        self.ember_rewards=true;self.embers.retain(|e|*e!=ember);
        if held {self.embers.push(ember);}
        self.validate()
    }
    /// 直接替换苦难律法，并按节制重新计算圣杯瓶上限
    pub fn debug_law(&mut self,law:Law)->RuleResult<()> {
        if !law.valid() {return Err("苦难律法无效".into());}
        self.debug_begin()?;
        let full=self.flasks==self.flask_limit;
        self.law=law;self.flask_limit=self.full_flask_limit();
        self.flasks=if full {self.flask_limit}else{self.flasks.min(self.flask_limit)};
        self.validate()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Class,NodeKind,Phase,Run,contracts::Kind,embers::Ember,law::Law};
    #[test]
    fn mausoleum_test_keeps_seed_and_build_and_tests_both_choices() {
        for seed in 0..100 {
            let mut run=Run::new(seed,Class::Samurai);run.event_rules=3;run.gold=1234;
            let before=run.clone();let node=run.debug_mausoleum_test().unwrap();
            assert_eq!((run.seed,&run.nodes,run.gold,run.event_rules),(before.seed,&before.nodes,1234,3));
            assert_eq!(run.event_draw(node),crate::events::Draw::Mausoleum);
            assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
            run.enter(node).unwrap();let mut skip=run.clone();skip.choose_trial(crate::regional_events::Trial::Skip).unwrap();
            assert_eq!(skip.gold,2034);skip.leave_regional_event().unwrap();
            run.choose_trial(crate::regional_events::Trial::Mausoleum).unwrap();
            assert!(run.regional_event.mausoleum.as_ref().unwrap().accepted);
            assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
            let before=run.clone();assert!(run.debug_mausoleum_test().is_err());assert_eq!(run,before);
        }
    }
    #[test]
    fn boss_test_prepares_an_available_boss_and_survives_save() {
        for act in 1..=3 {
            let mut run=Run::new(55,Class::Samurai);
            let node=run.debug_boss_test(7,act).unwrap();
            assert!(run.debug&&run.available().contains(&node));
            assert_eq!(run.nodes[node].kind,NodeKind::Boss);
            assert_eq!(run.boss_test,Some(super::BossTest{encounter:7,act}));
            assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
            run.enter(node).unwrap();assert!(run.debug_boss_test(8,act).is_err());
        }
        let mut run=Run::new(1,Class::Samurai);let before=run.clone();
        assert!(run.debug_boss_test(7,4).is_err());assert_eq!(run,before);
        assert!(!run.encode().unwrap().contains("boss_test"));
    }
    #[test]
    fn jumping_makes_any_node_reachable_and_keeps_saves_loadable() {
        for seed in 0..40 {
            let run=Run::new(seed,Class::Samurai);
            for target in 0..run.nodes.len() {
                let mut jumped=run.clone();jumped.debug_jump(target).unwrap();
                assert!(jumped.available().contains(&target)||jumped.contract_pending(),"node {target} reachable");
                assert!(jumped.debug);
                assert_eq!(Run::decode(&jumped.encode().unwrap()).unwrap(),jumped);
            }
            let boss=run.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap();
            let mut fight=run.clone();fight.contract.as_mut().map(|c|c.skip());fight.debug_jump(boss).unwrap();
            fight.enter(boss).unwrap();fight.win_combat().unwrap();fight.decline_reward().unwrap();
            assert_eq!(fight.act,2,"winning the jumped boss still advances the act");
        }
        let mut fighting=Run::new(1,Class::Samurai);let id=fighting.available()[0];fighting.enter(id).unwrap();
        assert!(fighting.debug_jump(5).is_err(),"only from the map");
    }
    #[test]
    fn switching_acts_matches_the_normal_seed_chain_both_ways() {
        let mut played=Run::new(77,Class::Astrologer);played.enable_contracts();
        let mut acts=vec![(played.seed,played.nodes.clone())];
        for _ in 0..2 {
            played.contract.as_mut().unwrap().skip().unwrap();
            let boss=played.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap();
            played.debug_jump(boss).unwrap();played.enter(boss).unwrap();played.win_combat().unwrap();played.decline_reward().unwrap();
            acts.push((played.seed,played.nodes.clone()));
        }
        let mut run=Run::new(77,Class::Astrologer);run.enable_contracts();
        for act in [3,1,2] {
            run.debug_act(act).unwrap();
            assert_eq!((run.act,run.seed,&run.nodes),(act,acts[usize::from(act)-1].0,&acts[usize::from(act)-1].1));
            assert_eq!((run.phase.clone(),run.visited.len(),run.hp,run.flasks),(Phase::Map,0,run.max_hp,run.flask_limit));
            assert_eq!(run.contract.as_ref().unwrap().act,act);
            assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
        }
    }
    #[test]
    fn oaths_embers_and_law_can_be_set_directly() {
        let mut run=Run::new(5,Class::Samurai);
        run.debug_oath(Kind::Steel,3).unwrap();run.debug_oath(Kind::Blood,2).unwrap();run.debug_oath(Kind::Mystic,1).unwrap();
        let contract=run.contract.as_ref().unwrap();
        assert_eq!((contract.level(Kind::Steel),contract.level(Kind::Blood),contract.deepen),(Some(3),Some(2),Some(Kind::Blood)));
        run.debug_oath(Kind::Blood,0).unwrap();assert_eq!(run.contract.as_ref().unwrap().level(Kind::Blood),None);
        run.debug_ember(Ember::Frenzy,true).unwrap();run.debug_ember(Ember::Rise,true).unwrap();run.debug_ember(Ember::Frenzy,false).unwrap();
        assert_eq!(run.embers,vec![Ember::Rise]);
        let law=[8,9,10,24].into_iter().fold(Law::default(),|l,i|l.toggle(i));
        run.debug_law(law).unwrap();assert_eq!((run.flask_limit,run.flasks),(6,6));
        assert!(run.debug_law(Law(1<<1)).is_err());
        assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
        let plain=Run::new(5,Class::Samurai);assert!(!plain.debug&&!plain.encode().unwrap().contains("\"debug\""));
    }
}
