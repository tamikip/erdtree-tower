use serde::{Deserialize, Serialize};

pub mod coordinator;
pub mod party_rewards;
pub mod party_link;
pub mod party_game;
pub mod party_vote;
pub mod party_roster;
pub mod party_handshake;
pub mod party_events;
pub mod map_drawing;
pub mod contracts;
pub mod act_gifts;
pub mod elite;
pub mod encounter;
pub mod events;
pub mod regional_events;
pub mod stormveil;
pub mod rewards;
pub mod affixes;
pub mod boss_affixes;
pub mod scaling;

pub type RuleResult<T> = Result<T, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Class {
    Vagabond,
    Samurai,
    Astrologer,
    Prophet,
    Warrior,
    Hero,
    Bandit,
    Prisoner,
    Confessor,
    Wretch,
}

impl Class {
    pub const ALL: [Self; 10] = [Self::Vagabond,Self::Warrior,Self::Hero,Self::Bandit,Self::Astrologer,Self::Prophet,Self::Prisoner,Self::Samurai,Self::Confessor,Self::Wretch];
    pub fn sorcerer(self)->bool {matches!(self,Self::Astrologer|Self::Prisoner)}
    pub fn cleric(self)->bool {matches!(self,Self::Prophet|Self::Confessor)}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    Monster,
    Elite,
    Boss,
    Camp,
    Shop,
    Event,
    Chest,
}

impl NodeKind {
    pub fn combat(self) -> bool {
        matches!(self, Self::Monster | Self::Elite | Self::Boss)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Monster => "怪物",
            Self::Elite => "精英",
            Self::Boss => "Boss",
            Self::Camp => "篝火",
            Self::Shop => "商店",
            Self::Event => "事件",
            Self::Chest => "宝箱",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub kind: NodeKind,
    pub layer: usize,
    pub lane: usize,
    pub next: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlessingKind {
    Guard,
    Charge,
    Dodge,
    Bleed,
    Sorcery,
    Flame,
    Vigor,
    Endurance,
}

impl BlessingKind {
    pub const ALL: [Self; 8] = [
        Self::Guard,
        Self::Charge,
        Self::Dodge,
        Self::Bleed,
        Self::Sorcery,
        Self::Flame,
        Self::Vigor,
        Self::Endurance,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Guard => "铁壁反击",
            Self::Charge => "蓄势重击",
            Self::Dodge => "闪避锋芒",
            Self::Bleed => "鲜血脉动",
            Self::Sorcery => "魔力连携",
            Self::Flame => "余烬之火",
            Self::Vigor => "生命赐福",
            Self::Endurance => "耐力赐福",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blessing {
    pub kind: BlessingKind,
    pub level: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reward {
    Blessing(BlessingKind),
    UpgradeWeapon,
    Gold(u32),
    Item(u32),
    TalismanBuff(u32),
    SacredTear,
    FlaskCapacity,
    MemorySlot,
    StarStream,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Map,
    Combat { id: usize },
    Node { id: usize },
    Reward { id: usize, offers: Vec<Reward> },
    Dead,
    Won,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampChoice {
    Refill,
    RefillBlue,
    RefillRedOnly,
    RefillBlueOnly,
    UpgradeWeapon,
    UpgradeBlessing(usize),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CombatCheckpoint {
    id: usize,
    hp: u32,
    flasks: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    schema: u32,
    #[serde(default)]
    pub affix_equipment: bool,
    #[serde(default)]
    pub affix_rules: u8,
    #[serde(default)]
    pub ruby_spells: bool,
    #[serde(default)]
    pub boss_affixes: bool,
    #[serde(default)]
    pub scaling_rules: u8,
    #[serde(default="first_act")]
    pub act:u8,
    pub seed: u64,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub initial_seed: Option<u64>,
    #[serde(default)]
    pub completed_nodes: u32,
    #[serde(default)]
    pub completed_battles: u32,
    #[serde(default)]
    pub reached_layer: usize,
    #[serde(default,skip_serializing_if="Option::is_none")]
    personal_reward_seed: Option<u64>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub casting_tools: Option<[bool;2]>,
    pub class: Class,
    pub nodes: Vec<Node>,
    pub phase: Phase,
    pub visited: Vec<usize>,
    pub hp: u32,
    pub max_hp: u32,
    pub flasks: u8,
    #[serde(default)]
    pub physick_empty: bool,
    pub flask_limit: u8,
    #[serde(default)]
    pub flask_bonus: u8,
    #[serde(default)]
    pub memory_bonus: u8,
    #[serde(default)]
    pub star_stream: bool,
    pub gold: u32,
    pub weapon_level: u8,
    pub blessings: Vec<Blessing>,
    #[serde(default)]
    pub delivered_talismans: Vec<u32>,
    #[serde(default)]
    pub claimed_items: Vec<u32>,
    #[serde(default)]
    pub regional_event: regional_events::Progress,
    #[serde(default)]
    pub contract: Option<contracts::State>,
    #[serde(default)]
    pub larval_gift: act_gifts::LarvalGift,
    #[serde(default)]
    pub elite_buffs: Vec<u32>,
    shop_purchased: u8,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub shop_stock: Option<ShopStock>,
    checkpoint: Option<CombatCheckpoint>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShopStock {pub node:usize,pub items:Vec<[i32;4]>}

fn first_act()->u8 {1}
fn mix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn legacy_graph(seed: u64) -> Vec<Node> {
    (0..36)
        .map(|id| {
            let layer = id / 3;
            let lane = id % 3;
            let kind = match layer {
                0 => NodeKind::Monster,
                3 => [NodeKind::Shop, NodeKind::Chest, NodeKind::Event][lane],
                10 => NodeKind::Camp,
                11 => NodeKind::Boss,
                _ => [
                    NodeKind::Monster,
                    NodeKind::Camp,
                    NodeKind::Shop,
                    NodeKind::Event,
                    NodeKind::Chest,
                    NodeKind::Monster,
                    NodeKind::Elite,
                ][(mix(seed ^ id as u64) % 7) as usize],
            };
            let next = if layer == 11 {
                vec![]
            } else {
                (0..3)
                    .filter(|target| {
                        let right = layer < 2 || mix(seed ^ (layer as u64 * 997)) & 1 == 0;
                        *target == lane || (right && *target == lane + 1) || (!right && lane == *target + 1)
                    })
                    .map(|target| (layer + 1) * 3 + target)
                    .collect()
            };
            Node {
                kind,
                layer,
                lane,
                next,
            }
        })
        .collect()
}

#[cfg(test)]
mod map_compatibility {
    use super::*;
    #[test]
    fn old_act_keeps_twelve_floors_then_advances_to_sixteen() {
        for schema in 1..=3 {
            let mut run=Run::new(42,Class::Samurai);run.schema=schema;
            run.nodes=match schema {1=>legacy_graph(42),2=>graph(42),_=>balanced_graph(42)};
            let original=run.nodes.clone();
            for step in 0..12 {
                run=Run::decode(&run.encode().unwrap()).unwrap();
                assert_eq!(run.nodes,original);assert_eq!(run.visited.len(),step);
                let id=run.available()[0];run.enter(id).unwrap();
                match run.nodes[id].kind {
                    NodeKind::Monster|NodeKind::Elite|NodeKind::Boss=>{run.win_combat().unwrap();run.decline_reward().unwrap();},
                    NodeKind::Camp=>run.leave_camp().unwrap(),NodeKind::Shop=>run.leave_shop().unwrap(),
                    NodeKind::Chest=>run.finish_chest().unwrap(),NodeKind::Event=>run.resolve_event(false).unwrap(),
                }
            }
            assert_eq!(run.act,2);assert_eq!(run.floors(),16);assert_eq!(run.schema,6);
            assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
        }
    }
    #[test]
    fn legacy_map_roundtrips_without_rewriting_routes() {
        let mut run=Run::new(42,Class::Samurai);run.schema=1;run.nodes=legacy_graph(run.seed);
        assert_eq!(run.available(),vec![0]);
        run.enter(0).unwrap();run.win_combat().unwrap();
        assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
        run.decline_reward().unwrap();assert_eq!(run.available(),vec![3,4]);
        assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
    }
    #[test]
    fn increased_camps_preserve_old_saves_and_spacing() {
        let mut old_count=0;let mut new_count=0;
        for seed in 0..1000 {
            let mut old=Run::new(seed,Class::Samurai);old.schema=5;old.nodes=spaced_graph(seed);
            assert_eq!(Run::decode(&old.encode().unwrap()).unwrap(),old);
            let new=Run::new(seed,Class::Samurai);
            assert_eq!(Run::decode(&new.encode().unwrap()).unwrap(),new);
            old_count+=old.nodes.iter().filter(|n|n.kind==NodeKind::Camp&&n.layer<14).count();
            new_count+=new.nodes.iter().filter(|n|n.kind==NodeKind::Camp&&n.layer<14).count();
            for (id,node) in new.nodes.iter().enumerate() {
                assert_eq!(node.next,old.nodes[id].next);
                if matches!(node.kind,NodeKind::Camp|NodeKind::Chest){assert!(node.next.iter().all(|&next|!matches!(new.nodes[next].kind,NodeKind::Camp|NodeKind::Chest)));}
                if node.layer==14{assert_eq!(node.kind,NodeKind::Camp);}
            }
        }
        assert!(new_count*100>old_count*150,"camps: old={old_count}, new={new_count}");
        println!("Random camps over 1000 seeds: old={old_count}, new={new_count}");
    }
    #[test]
    fn spaced_routes_keep_links_and_boss_camp_and_old_saves() {
        for seed in 0..1000 {
            let mut old=Run::new(seed,Class::Samurai);old.schema=4;old.nodes=expedition_graph(seed);
            assert_eq!(Run::decode(&old.encode().unwrap()).unwrap(),old);
            let mut run=Run::new(seed,Class::Samurai);
            for act in 1..=3 {
                assert_eq!(run.act,act);run.validate().unwrap();
                assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
                let original=expedition_graph(run.seed);
                for (id,node) in run.nodes.iter().enumerate() {
                    assert_eq!(node.next,original[id].next);
                    if node.layer==14 {assert_eq!(node.kind,NodeKind::Camp);}
                    if matches!(node.kind,NodeKind::Chest|NodeKind::Camp) {
                        for &next in &node.next {assert!(!matches!(run.nodes[next].kind,NodeKind::Chest|NodeKind::Camp));}
                    }
                }
                if act<3 {let boss=run.nodes.len()-1;run.finish(boss);}
            }
        }
    }
    #[test]
    fn previous_routes_remain_stable_and_new_routes_reduce_services() {
        let service=|n:&&Node|matches!(n.kind,NodeKind::Camp|NodeKind::Shop|NodeKind::Chest);
        let mut old_count=0;let mut new_count=0;
        for seed in 0..200 {
            let mut old=Run::new(seed,Class::Samurai);old.schema=2;old.nodes=graph(seed);
            assert_eq!(Run::decode(&old.encode().unwrap()).unwrap(),old);
            let mut new=Run::new(seed,Class::Samurai);new.schema=3;new.nodes=balanced_graph(seed);assert_eq!(Run::decode(&new.encode().unwrap()).unwrap(),new);
            old_count+=old.nodes.iter().filter(service).count();new_count+=new.nodes.iter().filter(service).count();
            assert!(new.nodes.iter().filter(|n|n.layer==10).all(|n|n.kind==NodeKind::Camp));
        }
        assert!(new_count*100<old_count*85);
        assert!(new_count*100>old_count*60);
    }
}

fn graph(seed:u64)->Vec<Node> {
    let mut nodes=legacy_graph(seed);
    nodes.truncate(34);
    nodes[33].lane=1;
    let lanes=|layer:usize|->Vec<usize> {if layer==0||layer==2||layer<10&&mix(seed^layer as u64^319)%3==0 {vec![0,1,2]}else{vec![0,2]}};
    for layer in 0..10 {
        let from=lanes(layer);let to=lanes(layer+1);
        let right=mix(seed^(layer as u64*997))&1==0;
        let lane=if right {(mix(seed^layer as u64^71)%2) as usize}else{1+(mix(seed^layer as u64^71)%2) as usize};
        for node in &mut nodes[layer*3..layer*3+3] {
            let targets=if let Some(rank)=from.iter().position(|&l|l==node.lane) {
                match (from.len(),to.len()) {
                    (3,2)=>match rank {0=>vec![0],1=>vec![0,1],_=>vec![1]},
                    (2,3)=>if rank==0 {vec![0,1]}else{vec![1,2]},
                    _=>{let mut v=vec![rank];if rank==lane.min(from.len()-1) {let next=if right{rank+1}else{rank.saturating_sub(1)};if next<to.len()&&!v.contains(&next){v.push(next);}}v},
                }
            }else{vec![node.lane.min(to.len()-1)]};
            node.next=targets.into_iter().map(|rank|(layer+1)*3+to[rank]).collect();node.next.sort_unstable();
        }
        if from.len()==2 {nodes[layer*3+1].next=vec![*nodes[layer*3].next.last().unwrap()];}
    }
    for node in &mut nodes[30..33] {node.next=vec![33];}
    nodes
}

fn expedition_graph(seed:u64)->Vec<Node> {
    let mut nodes=Vec::new();let mut rows=Vec::new();
    let pool=[NodeKind::Monster,NodeKind::Monster,NodeKind::Monster,NodeKind::Monster,NodeKind::Elite,NodeKind::Event,NodeKind::Event,NodeKind::Camp,NodeKind::Shop,NodeKind::Chest];
    for layer in 0..16 {
        let count=if layer==15 {1}else{4+(mix(seed^layer as u64^319)%2) as usize};
        let start=nodes.len();rows.push((start,count));
        for lane in 0..count {
            let kind=match layer {0=>NodeKind::Monster,14=>NodeKind::Camp,15=>NodeKind::Boss,_=>pool[(mix(seed^nodes.len() as u64)%pool.len() as u64) as usize]};
            nodes.push(Node{kind,layer,lane,next:vec![]});
        }
    }
    for layer in 0..15 {
        let (start,n)=rows[layer];let (next,m)=rows[layer+1];
        for i in 0..n {
            nodes[start+i].next=(0..m).filter(|&j|i*m<(j+1)*n&&j*n<(i+1)*m).map(|j|next+j).collect();
        }
        if n==m {
            let lane=(mix(seed^layer as u64^997)%(n-1) as u64) as usize;
            nodes[start+lane].next.push(next+lane+1);
        }
    }
    nodes
}

fn balanced_graph(seed:u64)->Vec<Node> {
    let mut nodes=graph(seed);
    let pool=[NodeKind::Monster,NodeKind::Monster,NodeKind::Monster,NodeKind::Monster,NodeKind::Elite,NodeKind::Event,NodeKind::Event,NodeKind::Camp,NodeKind::Shop,NodeKind::Chest];
    for (id,node) in nodes.iter_mut().enumerate().filter(|(_,n)|(1..10).contains(&n.layer)) {
        node.kind=if node.layer==3 {[NodeKind::Shop,NodeKind::Monster,NodeKind::Event][node.lane]}else{pool[(mix(seed^id as u64)%pool.len() as u64) as usize]};
    }
    nodes
}

fn spaced_graph(seed:u64)->Vec<Node> {
    space_rest_nodes(expedition_graph(seed))
}

fn camp_graph(seed:u64)->Vec<Node> {
    let mut nodes=expedition_graph(seed);
    for (id,node) in nodes.iter_mut().enumerate() {
        if (1..14).contains(&node.layer)&&mix(seed^id as u64)%10==0 {node.kind=NodeKind::Camp;}
    }
    space_rest_nodes(nodes)
}

fn space_rest_nodes(mut nodes:Vec<Node>)->Vec<Node> {
    let rest=|kind|matches!(kind,NodeKind::Chest|NodeKind::Camp);
    for id in (0..nodes.len()).rev() {
        if rest(nodes[id].kind)&&nodes[id].next.iter().any(|&next|rest(nodes[next].kind)) {
            nodes[id].kind=NodeKind::Monster;
        }
    }
    nodes
}

impl Run {
    pub fn floors(&self)->usize {self.nodes.last().map_or(0,|n|n.layer+1)}
    pub fn new(seed: u64, class: Class) -> Self {
        Self {
            schema: 6,
            affix_equipment: true,
            affix_rules: 2,
            ruby_spells: false,
            boss_affixes: true,
            scaling_rules: 1,
            act:1,
            seed,
            initial_seed: Some(seed),
            completed_nodes: 0,
            completed_battles: 0,
            reached_layer: 0,
            personal_reward_seed: None,
            casting_tools: None,
            class,
            nodes: camp_graph(seed),
            phase: Phase::Map,
            visited: vec![],
            hp: 100,
            max_hp: 100,
            flasks: 10,
            flask_limit: 10,
            flask_bonus: 0,
            memory_bonus: 0,
            star_stream: false,
            gold: 0,
            weapon_level: 0,
            blessings: vec![],
            delivered_talismans: vec![],
            claimed_items: vec![],
            physick_empty: false,
            regional_event: Default::default(),
            contract: None,
            larval_gift: Default::default(),
            elite_buffs: vec![],
            shop_purchased: 0,
            shop_stock: None,
            checkpoint: None,
        }
    }

    pub fn new_personal(seed:u64,class:Class,reward_seed:u64)->Self {
        let mut run=Self::new(seed,class);
        run.personal_reward_seed=Some(reward_seed);
        run
    }

    pub fn reward_seed(&self)->u64 {
        self.personal_reward_seed.map_or(self.seed,|seed|mix(seed ^ (u64::from(self.act)<<48) ^ 0x726577617264))
    }

    pub fn align_party_start(&mut self,seed:u64)->RuleResult<()> {
        self.validate()?;
        if self.seed==seed {return Ok(());}
        if self.act!=1||!self.visited.is_empty()||self.phase!=Phase::Map {
            return Err("已有旅程不能替换路线，请从大赐福开始共同旅程".into());
        }
        let mut next=self.clone();next.seed=seed;next.initial_seed=Some(seed);next.schema=6;next.nodes=camp_graph(seed);
        next.validate()?;*self=next;Ok(())
    }

    pub fn available(&self) -> Vec<usize> {
        if self.phase != Phase::Map || self.contract_pending() {
            return vec![];
        }
        self.next_route()
    }

    fn next_route(&self) -> Vec<usize> {
        match self.visited.last() {
            Some(id) => self.nodes[*id].next.clone(),
            None => self.start_nodes(),
        }
    }

    pub fn start_nodes(&self)->Vec<usize> {if self.schema==1 {vec![0]}else if self.schema<4 {vec![0,1,2]}else{self.nodes.iter().enumerate().take_while(|(_,n)|n.layer==0).map(|(id,_)|id).collect()}}

    pub fn enter(&mut self, id: usize) -> RuleResult<()> {
        if !self.available().contains(&id) {
            return Err("节点不可达".into());
        }
        self.reached_layer=self.nodes[id].layer+1;
        self.shop_purchased = 0;self.shop_stock=None;
        if self.nodes[id].kind==NodeKind::Event {
            let event_roll=mix(self.seed ^ id as u64 ^ 0x10cced)%8;
            self.regional_event.scarabs=event_roll==3;
            self.regional_event.locked=event_roll==0;
            self.regional_event.trial=matches!(event_roll,4|6|7);
            self.regional_event.recovery=event_roll==5;

        }
        if self.nodes[id].kind.combat() {
            self.checkpoint = Some(CombatCheckpoint {
                id,
                hp: self.hp,
                flasks: self.flasks,
            });
            self.phase = Phase::Combat { id };
        } else {
            self.phase = Phase::Node { id };
        }
        Ok(())
    }

    fn active_node(&self, kind: NodeKind) -> RuleResult<usize> {
        match self.phase {
            Phase::Node { id } if self.nodes.get(id).is_some_and(|n| n.kind == kind) => Ok(id),
            _ => Err("当前节点不允许此操作".into()),
        }
    }

    fn finish(&mut self, id: usize) {
        self.completed_nodes=self.completed_nodes.saturating_add(1);
        if matches!(self.nodes[id].kind,NodeKind::Monster|NodeKind::Elite|NodeKind::Boss){self.completed_battles=self.completed_battles.saturating_add(1);}
        self.regional_event=Default::default();
        self.visited.push(id);
        self.checkpoint = None;
        self.shop_purchased = 0;self.shop_stock=None;
        if self.nodes[id].kind==NodeKind::Boss && self.act<3 {
            self.act+=1;
            self.reached_layer=0;
            self.hp=self.max_hp;
            self.flasks=self.flask_limit;
            self.seed=mix(self.seed^u64::from(self.act));
            if let Some(old)=&self.contract {self.contract=Some(contracts::State::new(self.reward_seed(),self.class,self.act,old.chosen,old.peak));}
            self.schema=6;
            self.nodes=camp_graph(self.seed);
            self.visited.clear();
            self.phase=Phase::Map;
            return;
        }
        self.phase = if self.nodes[id].kind == NodeKind::Boss {
            Phase::Won
        } else {
            Phase::Map
        };
    }

    pub fn use_flask(&mut self) -> RuleResult<()> {
        if !matches!(self.phase, Phase::Combat { .. }) || self.flasks == 0 {
            return Err("当前无法使用圣杯瓶".into());
        }
        self.flasks -= 1;
        Ok(())
    }

    // The game bridge owns healing and reports actual health; the rule engine never invents it.
    pub fn observe_hp(&mut self, hp: u32) -> RuleResult<()> {
        if !matches!(self.phase, Phase::Combat { .. }) || hp > self.max_hp {
            return Err("无效生命状态".into());
        }
        self.hp = hp;
        if hp == 0 {
            self.phase = Phase::Dead;
            self.checkpoint = None;
        }
        Ok(())
    }

    pub fn camp(&mut self, choice: CampChoice) -> RuleResult<()> {
        let id = self.active_node(NodeKind::Camp)?;
        match choice {
            CampChoice::Refill | CampChoice::RefillBlue | CampChoice::RefillRedOnly | CampChoice::RefillBlueOnly => {
                if self.flasks >= self.flask_limit && !self.physick_empty {
                    return Err("圣杯瓶已满".into());
                }
                self.flasks = self.flasks.saturating_add(3).min(self.flask_limit);
                self.physick_empty=false;
            }
            CampChoice::UpgradeWeapon => {
                if self.weapon_level >= 25 {
                    return Err("武器已达到上限".into());
                }
                self.weapon_level += 1;
            }
            CampChoice::UpgradeBlessing(index) => {
                let blessing = self.blessings.get_mut(index).ok_or("请选择已有赐福")?;
                if blessing.level >= 3 {
                    return Err("赐福已达到上限".into());
                }
                blessing.level += 1;
            }
        }
        self.finish(id);
        Ok(())
    }

    pub fn leave_camp(&mut self) -> RuleResult<()> {
        let id = self.active_node(NodeKind::Camp)?;
        self.finish(id);
        Ok(())
    }

    fn offers(&self, id: usize) -> Vec<Reward> {
        rewards::offers(self,id)
    }
    fn legacy_offers(&self, id: usize) -> Vec<Reward> {
        let start = (mix(self.seed ^ (id as u64 + 100)) % 8) as usize;
        let mut offers = Vec::new();
        for offset in 0..8 {
            let kind = BlessingKind::ALL[(start + offset) % 8];
            if !self
                .blessings
                .iter()
                .any(|b| b.kind == kind && b.level == 3)
            {
                offers.push(Reward::Blessing(kind));
                if offers.len() == 2 {
                    break;
                }
            }
        }
        while offers.len() < 2 {
            offers.push(Reward::Gold(75 + offers.len() as u32 * 25));
        }
        offers.push(if self.weapon_level < 25 {
            Reward::UpgradeWeapon
        } else {
            Reward::Gold(100)
        });
        offers
    }

    pub fn win_combat(&mut self) -> RuleResult<()> {
        let Phase::Combat { id } = self.phase else {
            return Err("当前没有进行中的战斗".into());
        };
        let kind = self.nodes.get(id).ok_or("无效节点")?.kind;
        if !kind.combat() || self.hp == 0 {
            return Err("战斗状态无效".into());
        }
        self.checkpoint = None;
        self.phase = Phase::Reward {
            id,
            offers: self.offers(id),
        };
        Ok(())
    }

    fn grant(&mut self, reward: Reward) -> RuleResult<()> {
        match reward {
            Reward::FlaskCapacity=>{
                if self.flask_bonus>=4{return Err("圣杯瓶容量已达上限".into());}
                self.flask_bonus+=1;self.flask_limit=10+self.flask_bonus*2;
            },
            Reward::MemorySlot=>{
                if self.memory_bonus>=6{return Err("记忆栏位已达上限".into());}
                self.memory_bonus+=1;
            },
            Reward::StarStream=>{
                if self.star_stream{return Err("已获得星流引导".into());}
                self.star_stream=true;
            },
            Reward::SacredTear=>{},
            Reward::TalismanBuff(id)=>{
                if !elite::BUFFS.contains(&id)||self.elite_buffs.contains(&id){return Err("护符效果已获得或无效".into());}
                self.elite_buffs.push(id);
            },
            Reward::Blessing(kind) => {
                if let Some(b) = self.blessings.iter_mut().find(|b| b.kind == kind) {
                    if b.level >= 3 {
                        return Err("赐福已达到上限".into());
                    }
                    b.level += 1;
                } else {
                    self.blessings.push(Blessing { kind, level: 1 });
                }
            }
            Reward::UpgradeWeapon => {
                if self.weapon_level >= 25 {
                    return Err("武器已达到上限".into());
                }
                self.weapon_level += 1;
            }
            Reward::Gold(amount) => self.gold = self.gold.saturating_add(amount),
            Reward::Item(id)=>{
                let item=rewards::lookup(id).ok_or("未知奖励物品")?;
                let boss=matches!(self.phase,Phase::Reward{id:node,..} if self.nodes[node].kind==NodeKind::Boss)&&elite::LEGENDARY.contains(&affixes::base(id));
                if (!item.kind.allowed(self.class)&&!boss)||(self.claimed_items.contains(&id)&&!(boss&&item.kind==rewards::ItemKind::Weapon)) {return Err("该奖励不可领取".into());}
                if !self.claimed_items.contains(&id){self.claimed_items.push(id);}
            },
        }
        Ok(())
    }

    pub fn claim_reward(&mut self, index: usize) -> RuleResult<()> {
        let Phase::Reward { id, ref offers } = self.phase else {
            return Err("当前没有待领取奖励".into());
        };
        let reward = offers.get(index).ok_or("无效奖励选项")?.clone();
        self.grant(reward)?;
        self.finish(id);
        Ok(())
    }

    pub fn decline_reward(&mut self) -> RuleResult<()> {
        let Phase::Reward { id, .. } = self.phase else {
            return Err("当前没有待领取奖励".into());
        };
        self.finish(id);
        Ok(())
    }

    pub fn finish_chest(&mut self) -> RuleResult<()> {
        let id = self.active_node(NodeKind::Chest)?;
        self.finish(id);
        Ok(())
    }

    pub fn open_chest(&mut self) -> RuleResult<()> {
        let id = self.active_node(NodeKind::Chest)?;
        self.phase = Phase::Reward {
            id,
            offers: self.offers(id),
        };
        Ok(())
    }

    pub fn shop_offer(&self, index: usize) -> RuleResult<(Reward, u32, bool)> {
        self.active_node(NodeKind::Shop)?;
        let stock = [
            (Reward::Blessing(BlessingKind::Vigor), 75),
            (Reward::UpgradeWeapon, 100),
            (Reward::Blessing(BlessingKind::Endurance), 75),
        ];
        let (reward, price) = stock.get(index).ok_or("无效商品")?;
        Ok((
            reward.clone(),
            *price,
            self.shop_purchased & (1 << index) != 0,
        ))
    }

    pub fn buy(&mut self, index: usize) -> RuleResult<()> {
        let (reward, price, purchased) = self.shop_offer(index)?;
        if purchased {
            return Err("商品已售出".into());
        }
        if self.gold < price {
            return Err("卢恩不足".into());
        }
        self.grant(reward)?;
        self.gold -= price;
        self.shop_purchased |= 1 << index;
        Ok(())
    }

    pub fn leave_shop(&mut self) -> RuleResult<()> {
        let id = self.active_node(NodeKind::Shop)?;
        self.finish(id);
        Ok(())
    }

    pub fn event_cost(&self) -> u32 {
        self.max_hp.saturating_mul(15).div_ceil(100)
    }

    pub fn resolve_event(&mut self, accept: bool) -> RuleResult<()> {
        let id = self.active_node(NodeKind::Event)?;
        if accept {
            let cost = self.event_cost();
            if self.hp <= cost || self.max_hp <= cost {
                return Err("生命不足，无法献祭".into());
            }
            self.grant(Reward::Blessing(BlessingKind::Bleed))?;
            self.max_hp -= cost;
            self.hp = (self.hp - cost).min(self.max_hp);
        }
        self.finish(id);
        Ok(())
    }

    pub fn encode(&self) -> RuleResult<String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
    pub fn resume_from_hub(&mut self) -> RuleResult<()> {
        self.validate()?;
        if matches!(self.phase,Phase::Dead|Phase::Won){return Err("已结束的旅程不能继续".into());}
        if let Phase::Combat { .. } = self.phase {
            self.checkpoint=None;
            self.phase=Phase::Map;
        }
        Ok(())
    }
    pub fn end_on_death(&mut self) {
        self.hp=0;self.phase=Phase::Dead;self.checkpoint=None;
    }

    pub fn decode(json: &str) -> RuleResult<Self> {
        if json.len() > 128 * 1024 {
            return Err("存档过大".into());
        }
        let mut run: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if run.ruby_spells {
            run.ruby_spells=false;
            if let Phase::Reward{id,..}=run.phase {run.phase=Phase::Reward{id,offers:run.offers(id)};}
            if run.shop_stock.as_ref().is_some_and(|s|s.items.iter().any(|[kind,id,..]|*kind==3&&rewards::RETIRED_SPELLS.contains(&(0x40000000|*id as u32)))) {run.shop_stock=None;}
        }
        if run.flask_bonus<=4&&run.flask_limit==5+run.flask_bonus*2 {
            if run.flasks>run.flask_limit||run.checkpoint.as_ref().is_some_and(|c|c.flasks>run.flask_limit) {return Err("旧存档圣杯瓶数量无效".into());}
            run.flask_limit=10+run.flask_bonus*2;
        }
        run.validate()?;
        if let Phase::Reward{id,ref offers}=run.phase {
            if *offers==rewards::before_claimed_bases(&run,id)||*offers==rewards::before_spirit_levels(&run,id)||*offers==rewards::before_currency_1500(&run,id)||*offers==rewards::before_chest_boosts(&run,id)||*offers==rewards::before_bosses(&run,id)||*offers==rewards::before_elites(&run,id)||*offers==rewards::before_base_talismans(&run,id)||*offers==rewards::before_spirit_weight(&run,id)||*offers==rewards::previous_offers(&run,id)||*offers==run.legacy_offers(id)||*offers==rewards::before_spirits(&run,id)||*offers==rewards::before_expansion(&run,id) {
                run.phase=Phase::Reward{id,offers:run.offers(id)};
            }
        }
        if let Some(checkpoint) = &run.checkpoint {
            run.hp = checkpoint.hp;
            run.flasks = checkpoint.flasks;
        }
        Ok(run)
    }

    pub fn validate(&self) -> RuleResult<()> {
        let fail = || Err("存档状态不合法或版本不受支持".into());
        if ![1,2,3,4,5,6].contains(&self.schema)||self.affix_rules>2||self.scaling_rules>1
            || self.contract.as_ref().is_some_and(|c|!c.valid(self.act))
            || !self.larval_gift.valid(self.act)
            || self.elite_buffs.iter().any(|id|!elite::BUFFS.contains(id))
            || self.elite_buffs.iter().collect::<std::collections::BTreeSet<_>>().len()!=self.elite_buffs.len()
            || !self.valid_regional_event()
            || !(1..=3).contains(&self.act)
            || self.nodes != match self.schema {1=>legacy_graph(self.seed),2=>graph(self.seed),3=>balanced_graph(self.seed),4=>expedition_graph(self.seed),5=>spaced_graph(self.seed),_=>camp_graph(self.seed)}
            || self.max_hp == 0
            || self.max_hp > 100
            || self.hp > self.max_hp
            || self.flask_bonus>4 || self.memory_bonus>6
            || self.flask_limit != 10+self.flask_bonus*2
            || self.flasks > self.flask_limit
            || self.weapon_level > 25
            || self.shop_purchased > 7
            || self.blessings.len() > 8
            || self.delivered_talismans.len() > 24
            || self.visited.len() > self.floors()
        {
            return fail();
        }
        for (i, b) in self.blessings.iter().enumerate() {
            if !(1..=3).contains(&b.level)
                || self.blessings[..i].iter().any(|other| other.kind == b.kind)
            {
                return fail();
            }
        }
        for (i,id) in self.delivered_talismans.iter().enumerate() {
            if *id==0 || *id>999999 || self.delivered_talismans[..i].contains(id) {return fail();}
        }
        if self.claimed_items.len()>rewards::CATALOG.len()+affixes::CATALOG.len() {return fail();}
        for (i,id) in self.claimed_items.iter().enumerate() {
            if !(rewards::lookup(*id).is_some_and(|item|item.kind.allowed(self.class)||elite::LEGENDARY.contains(&affixes::base(*id)))||rewards::RETIRED_SPELLS.contains(id))||self.claimed_items[..i].contains(id) {return fail();}
        }
        let mut route = self.start_nodes();
        for id in &self.visited {
            if !route.contains(id) {
                return fail();
            }
            route = self.nodes[*id].next.clone();
        }
        let current_id = match &self.phase {
            Phase::Combat { id } => {
                if !self.nodes.get(*id).is_some_and(|n| n.kind.combat()) {
                    return fail();
                }
                let Some(c) = &self.checkpoint else {
                    return fail();
                };
                if c.id != *id || c.hp == 0 || c.hp > self.max_hp || c.flasks > self.flask_limit {
                    return fail();
                }
                Some(*id)
            }
            Phase::Node { id } => {
                if self.nodes.get(*id).is_none_or(|n| n.kind.combat()) {
                    return fail();
                }
                Some(*id)
            }
            Phase::Reward { id, offers } => {
                if !self
                    .nodes
                    .get(*id)
                    .is_some_and(|n| n.kind.combat() || n.kind == NodeKind::Chest)
                    || (*offers != self.offers(*id) && *offers != rewards::before_claimed_bases(self,*id) && *offers != rewards::before_spirit_levels(self,*id) && *offers != rewards::before_currency_1500(self,*id) && *offers != rewards::before_chest_boosts(self,*id) && *offers != rewards::before_bosses(self,*id) && *offers != rewards::before_elites(self,*id) && *offers != rewards::before_base_talismans(self,*id) && *offers != self.legacy_offers(*id) && *offers != rewards::previous_offers(self,*id) && *offers != rewards::before_spirits(self,*id) && *offers != rewards::before_expansion(self,*id) && *offers != rewards::before_spirit_weight(self,*id))
                {
                    return fail();
                }
                Some(*id)
            }
            Phase::Won => {
                if self.visited.len() != self.floors() || !route.is_empty() || self.hp == 0 {
                    return fail();
                }
                None
            }
            Phase::Dead => {
                if self.hp != 0 {
                    return fail();
                }
                None
            }
            Phase::Map => {
                if route.is_empty() {
                    return fail();
                }
                None
            }
        };
        if current_id.is_some_and(|id| !route.contains(&id))
            || (!matches!(self.phase, Phase::Combat { .. }) && self.checkpoint.is_some())
            || (self.hp == 0 && self.phase != Phase::Dead)
            || (self.shop_purchased != 0
                && !matches!(self.phase, Phase::Node { id } if self.nodes[id].kind == NodeKind::Shop))
            || self.shop_stock.as_ref().is_some_and(|stock|stock.items.len()>64
                || !matches!(self.phase, Phase::Node { id } if id==stock.node&&self.nodes[id].kind == NodeKind::Shop))
        {
            return fail();
        }
        Ok(())
    }
}
