// 苦难律法：开局前自选的难度节点。节点沿路线解锁，同一路线效果逐级叠加；分数只用于评级与纪录。
/// art：图标在律法图标集（scene-assets/law-art/icons.rgba，scene-builder/law_icons.py 生成）里的序号，同一路线共用一个
pub struct Node {pub name:&'static str,pub branch:usize,pub parent:Option<usize>,pub points:u32,pub effect:&'static str,pub art:u8,pub col:u8,pub row:u8}
pub const BRANCHES:[&str;4]=["敌人","我方","经济","规则"];
const fn n(name:&'static str,branch:usize,parent:Option<usize>,points:u32,effect:&'static str,art:u8,col:u8,row:u8)->Node {Node{name,branch,parent,points,effect,art,col,row}}
pub const NODES:[Node;35]=[
    n("强敌 I",0,None,10,"敌人强度 +1 档：生命 +10%，攻击 +6%",0,1,0),
    n("强敌 II",0,Some(0),20,"再 +1 档：累计生命 +21%，攻击 +13%",0,2,0),
    n("强敌 III",0,Some(1),30,"再 +2 档：累计生命 +45%，攻击 +28%",0,3,0),
    n("词条蔓延 I",0,None,10,"Boss 词条点数预算 +2（受本幕上限约束）",1,1,1),
    n("词条蔓延 II",0,Some(3),20,"Boss 词条点数按最高的 6 点抽取",1,2,1),
    n("精英词条",0,None,20,"精英战的敌人也获得词条（按本幕 Boss 预算，额外卢恩为 Boss 的 1/3）",2,3,1),
    n("坚不可摧 I",0,None,10,"敌人受到的削韧 -30%",4,1,2),
    n("坚不可摧 II",0,Some(6),10,"敌人受到的削韧累计 -44%",4,2,2),
    n("节制 I",1,None,10,"红圣杯瓶上限 -1",7,1,3),
    n("节制 II",1,Some(8),10,"蓝圣杯瓶上限也 -1",7,2,3),
    n("节制 III",1,Some(9),20,"红蓝圣杯瓶上限再各 -1（共各 -2）",7,3,3),
    n("脆弱 I",1,None,10,"我方受到的伤害 +10%",8,1,4),
    n("脆弱 II",1,Some(11),10,"我方受到的伤害累计 +20%",8,2,4),
    n("脆弱 III",1,Some(12),20,"我方受到的伤害累计 +35%",8,3,4),
    n("孤狼",1,Some(13),30,"无法召唤骨灰，奖励与商店不再出现骨灰",9,4,4),
    n("疲惫 I",1,None,20,"精力恢复速度约 -20%",10,1,5),
    n("疲惫 II",1,Some(15),30,"精力恢复速度约 -40%",10,2,5),
    n("断念 I",1,None,10,"魔法、祷告与战技的专注消耗 +25%",11,1,6),
    n("断念 II",1,Some(17),10,"专注消耗累计 +50%",11,2,6),
    n("贫瘠 I",2,None,10,"击杀与战斗奖励的卢恩 -15%",12,1,7),
    n("贫瘠 II",2,Some(19),10,"击杀与战斗奖励的卢恩累计 -30%",12,2,7),
    n("奸商 I",2,None,10,"商店价格 +20%",13,3,7),
    n("奸商 II",2,Some(21),10,"商店价格累计 +40%",13,4,7),
    n("吝啬的赐福",2,None,30,"战后奖励三选一改为二选一",14,5,7),
    n("限时 I",3,None,10,"每场战斗限时 3 分钟，超时后每秒失去 2% 生命",15,1,8),
    n("限时 II",3,Some(24),10,"战斗限时缩短为 2 分钟",15,2,8),
    n("嫉妒的世界",3,None,30,"你喝圣杯瓶时，所有敌人回复 10% 生命",16,3,8),
    n("元素庇护 I",0,None,10,"敌人受到魔力、火、雷、圣伤害 -10%，不影响物理伤害",5,1,9),
    n("元素庇护 II",0,Some(27),10,"敌人受到四种元素伤害累计 -20%",5,2,9),
    n("异常耐性 I",0,None,10,"敌人中毒、腐败、出血、冻伤、睡眠、发狂、死亡耐性 +15%，原有免疫不变",6,1,10),
    n("异常耐性 II",0,Some(29),10,"七类异常耐性累计 +30%，原有免疫不变",6,2,10),
    n("词条强化",0,None,20,"Boss 与精英的词条数值约提高一半：减伤、攻击、回复、吸血、反弹、加速都更强",3,4,1),
    // 32 起超出原来的 32 位：律法以 64 位记录，前 32 个节点的律法码不变
    n("精英成双 I",0,None,20,"精英战第一只精英生命降到一半时，第二只上场（默认第一只倒下后才出来）",17,4,0),
    n("精英成双 II",0,Some(32),40,"精英战里的两只精英同时上场",17,5,0),
    n("精英横行",0,None,20,"从第 6 个点起，普通战斗房有 25% 概率替换为精英房，路线结果固定",18,5,1),
];
/// 停用的节点：位置与编号保留（存档、旅程档案与律法码里的位不变），界面不再显示，新开局选不上；
/// 已经带着它开局的旅程照旧生效。14 孤狼（2026-11-06 移除）
pub const RETIRED:[usize;1]=[14];
pub const GRADES:[(u32,&str);4]=[(70,"老练"),(140,"艰难"),(210,"噩梦"),(280,"危机")];

#[derive(Clone,Copy,Debug,Default,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
#[serde(transparent)]
pub struct Law(pub u64);
impl Law {
    pub fn is_empty(self)->bool {self.0==0}
    fn chain(self,ids:&[usize])->u8 {ids.iter().take_while(|&&id|self.has(id)).count() as u8}
    /// 强敌：敌人额外档位（0、1、2、4）
    pub fn enemy_tiers(self)->u8 {[0,1,2,4][usize::from(self.chain(&[0,1,2]))]}
    pub fn enemy_level(self)->u8 {self.chain(&[0,1,2])}
    /// 词条蔓延：Boss 词条预算（不超过组合表上限）
    pub fn affix_budget(self,base:u8,max:u8)->u8 {match self.chain(&[3,4]) {0=>base,1=>(base+2).min(max),_=>max}}
    pub fn elite_affixes(self)->bool {self.has(5)}
    pub fn elite_pressure(self)->bool {self.has(34)}
    /// 词条强化：Boss 词条使用强化后的数值
    pub fn empowered_affixes(self)->bool {self.has(31)}
    /// 精英成双：精英战里精英怪池的精英怎么上场
    pub fn elite_pacing(self)->crate::encounter::Pacing {
        use crate::encounter::Pacing;
        match self.chain(&[32,33]) {0=>Pacing::Apart,1=>Pacing::HalfHealth,_=>Pacing::Together}
    }
    pub fn toughness(self)->u8 {self.chain(&[6,7])}
    pub fn elemental_resistance(self)->u8 {self.chain(&[27,28])}
    pub fn status_resistance(self)->u8 {self.chain(&[29,30])}
    /// 节制：红、蓝圣杯瓶上限各减少多少
    pub fn flask_cut(self)->[u8;2] {[[0,0],[1,0],[1,1],[2,2]][usize::from(self.chain(&[8,9,10]))]}
    pub fn fragile(self)->u8 {self.chain(&[11,12,13])}
    pub fn lone_wolf(self)->bool {self.has(14)}
    pub fn fatigue(self)->u8 {self.chain(&[15,16])}
    pub fn focus(self)->u8 {self.chain(&[17,18])}
    /// 贫瘠：卢恩获取百分比
    pub fn poverty(self)->u8 {self.chain(&[19,20])}
    pub fn rune_percent(self)->u32 {[100,85,70][usize::from(self.poverty())]}
    pub fn runes(self,amount:u32)->u32 {if self.rune_percent()==100 {amount}else{(u64::from(amount)*u64::from(self.rune_percent())/100) as u32}}
    /// 奸商：商店价格百分比
    pub fn price_percent(self)->u32 {[100,120,140][usize::from(self.chain(&[21,22]))]}
    pub fn stingy(self)->bool {self.has(23)}
    /// 限时：每场战斗的秒数
    pub fn time_limit(self)->Option<u64> {match self.chain(&[24,25]) {0=>None,1=>Some(180),_=>Some(120)}}
    pub fn envy_world(self)->bool {self.has(26)}
    pub fn has(self,id:usize)->bool {id<NODES.len()&&self.0&(1u64<<id)!=0}
    pub fn available(self,id:usize)->bool {id<NODES.len()&&NODES[id].parent.is_none_or(|p|self.has(p))}
    /// 律法界面与调试菜单列出的节点（不含停用的）
    pub fn listed(id:usize)->bool {id<NODES.len()&&!RETIRED.contains(&id)}
    /// 去掉停用的节点（开局时、读律法码时用）
    pub fn current(self)->Self {Law(RETIRED.iter().fold(self.0,|mask,&id|mask&!(1u64<<id)))}
    pub fn valid(self)->bool {u64::from(self.0)>>NODES.len()==0&&(0..NODES.len()).all(|id|!self.has(id)||self.available(id))}
    /// 选择时连同这条路线上还没选的前置节点一起选上；取消会连带取消其后的节点
    pub fn toggle(self,id:usize)->Self {
        if id>=NODES.len() {return self;}
        if !self.has(id)&&!Self::listed(id) {return self;}
        if self.has(id) {
            let mut next=self.0&!(1u64<<id);
            loop {let before=next;for (i,node) in NODES.iter().enumerate(){if node.parent.is_some_and(|p|next&(1u64<<p)==0){next&=!(1u64<<i);}}if next==before{break;}}
            Law(next)
        } else {Law(self.0|self.path(id))}
    }
    /// 节点与它的全部前置节点（位掩码）
    fn path(self,id:usize)->u64 {
        let mut mask=0u64;let mut node=Some(id);
        while let Some(i)=node {mask|=1u64<<i;node=NODES[i].parent;}
        mask
    }
    /// 选择这个节点时一并选上的、还没选的前置节点（由近到远；已选或没有前置时为空）
    pub fn missing_prerequisites(self,id:usize)->Vec<usize> {
        let mut missing=Vec::new();let mut node=NODES.get(id).and_then(|n|n.parent);
        while let Some(i)=node {if !self.has(i){missing.push(i);}node=NODES[i].parent;}
        missing
    }
    pub fn score(self)->u32 {(0..NODES.len()).filter(|&id|self.has(id)).map(|id|NODES[id].points).sum()}
    pub fn grade(self)->Option<&'static str> {GRADES.iter().rev().find(|(min,_)|self.score()>=*min).map(|(_,name)|*name)}
    pub fn selected(self)->Vec<usize> {
        let mut ids:Vec<_>=(0..NODES.len()).filter(|&id|self.has(id)).collect();
        ids.sort_by_key(|&id|(std::cmp::Reverse(NODES[id].points),id));ids
    }
    /// 律法码：节点位图加 5 位校验，Crockford Base32，便于分享
    pub fn code(self)->String {
        const DIGITS:&[u8]=b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        let value=(self.0<<5)|checksum(self.0);
        (0..if value<(1u64<<35){7}else{8}).rev().map(|i|DIGITS[((value>>(i*5))&31) as usize] as char).collect()
    }
    pub fn parse(code:&str)->Option<Self> {
        const DIGITS:&str="0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        let code=code.trim().to_ascii_uppercase().replace(['O'],"0").replace(['I','L'],"1");
        if code.is_empty()||code.len()>8 {return None;}
        let value=code.chars().try_fold(0u64,|v,c|DIGITS.find(c).map(|d|v<<5|d as u64))?;
        let law=Law(value>>5);
        (value&31==checksum(law.0)&&law.valid()).then_some(law.current())
    }
}
/// 校验：前 32 位的算法不变（旧律法码照常通过）；用到更高的位时接着折算
fn checksum(mask:u64)->u64 {
    let low=(0..32u32).map(|i|((mask>>i)&1) as u32*(i+7)).fold(11u32,|a,b|a.wrapping_mul(31).wrapping_add(b));
    let all=if mask>>32==0 {low}else{(32..64u32).map(|i|((mask>>i)&1) as u32*(i+7)).fold(low,|a,b|a.wrapping_mul(31).wrapping_add(b))};
    u64::from(all%32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codes_from_before_the_64_bit_law_stay_valid_and_new_nodes_round_trip() {
        // 扩成 64 位之前的算法：32 位节点图 + 同样的校验
        let old_code=|mask:u32|{
            const DIGITS:&[u8]=b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
            let sum=u64::from((0..32).map(|i|((mask>>i)&1)*(i+7)).fold(11u32,|a,b|a.wrapping_mul(31).wrapping_add(b))%32);
            let value=(u64::from(mask)<<5)|sum;
            (0..if value<(1u64<<35){7}else{8}).rev().map(|i|DIGITS[((value>>(i*5))&31) as usize] as char).collect::<String>()
        };
        let mut law=Law::default();
        for id in [0,3,8,11,15,19,24,27,29,4,31,1,12,25] {
            law=law.toggle(id);
            assert_eq!(law.code(),old_code(law.0 as u32),"old codes are unchanged");
            assert_eq!(Law::parse(&old_code(law.0 as u32)),Some(law));
        }
        let half=Law::default().toggle(32);
        assert!(half.elite_pacing()==crate::encounter::Pacing::HalfHealth&&!half.has(2)&&!half.has(1)&&!half.has(0),"pacing is independent of enemy strength");
        let paired=Law::default().toggle(33);
        assert!(paired.elite_pacing()==crate::encounter::Pacing::Together&&paired.has(32));
        assert_eq!(Law::default().elite_pacing(),crate::encounter::Pacing::Apart);
        assert_eq!(Law::parse(&paired.code()),Some(paired));assert_eq!(Law::parse(&half.code()),Some(half));
        let all=Law((1u64<<NODES.len())-1);assert!(all.valid());assert_eq!(Law::parse(&all.code()),Some(all.current()));
        assert!(!Law(1u64<<NODES.len()).valid());
    }
    #[test]
    fn every_route_has_its_own_icon() {
        // 18 个图集图标，加上由精英徽记与上升箭头组成的精英横行图标。
        const ICONS:u8=19;
        let stem=|n:&Node|n.name.trim_end_matches(['I',' ']);
        assert!(NODES.iter().all(|n|n.art<ICONS));
        assert_eq!(NODES.iter().map(|n|n.art).collect::<std::collections::BTreeSet<_>>().len(),usize::from(ICONS),"every icon is used");
        for (i,a) in NODES.iter().enumerate() {for b in &NODES[..i] {assert_eq!(a.art==b.art,stem(a)==stem(b),"{} / {}",a.name,b.name);}}
    }
    #[test]
    fn graph_is_well_formed() {
        for (id,node) in NODES.iter().enumerate() {
            if let Some(p)=node.parent {assert!(p<id&&NODES[p].branch==node.branch&&NODES[p].row==node.row&&NODES[p].col+1==node.col,"{}",node.name);}
            if let Some(p)=node.parent {if Law::listed(id){assert_eq!(node.art,NODES[p].art,"different rules must not be chained");}}
            assert!(NODES.iter().enumerate().all(|(o,other)|o==id||(other.col,other.row)!=(node.col,node.row)));
        }
        assert_eq!(Law((1u64<<NODES.len())-1).score(),570);
    }
    #[test]
    fn selection_follows_paths_and_cascades() {
        let law=Law::default().toggle(2);assert_eq!(law,Law::default().toggle(0).toggle(1).toggle(2),"choosing a later node also chooses its prerequisites");
        assert_eq!(Law::default().missing_prerequisites(2),vec![1,0]);assert!(law.missing_prerequisites(2).is_empty());
        assert_eq!(Law::default().toggle(0).toggle(2).score(),60,"only the missing prerequisites are added");
        assert_eq!(Law::default().toggle(99),Law::default());
        let law=Law::default().toggle(0).toggle(1).toggle(2);assert_eq!(law.score(),60);
        assert_eq!(law.toggle(1),Law::default().toggle(0),"removing a node removes its successors");
        let all=(0..NODES.len()).fold(Law::default(),|l,id|l.toggle(id));
        assert_eq!(all.score(),540,"everything except the retired lone wolf");assert_eq!(all.grade(),Some("危机"));assert!(all.valid());
        assert_eq!(Law::default().grade(),None);assert_eq!(Law::default().toggle(24).toggle(25).toggle(26).toggle(23).grade(),Some("老练"));
        assert!(!Law(1<<1).valid());
    }
    #[test]
    fn retired_nodes_stay_readable_but_cannot_be_chosen() {
        let lone=Law(Law::default().toggle(13).0|1<<14);
        assert!(lone.valid()&&lone.lone_wolf(),"old runs and records still read it");
        assert_eq!(Law::default().toggle(14),Law::default());assert_eq!(Law::default().toggle(13).toggle(14),Law::default().toggle(13));
        assert_eq!(lone.toggle(14),Law::default().toggle(13),"it can still be removed");
        assert_eq!(Law::parse(&lone.code()),Some(Law::default().toggle(13)),"shared codes drop it");
        assert_eq!(lone.current(),Law::default().toggle(13));
        assert!(RETIRED.iter().all(|&id|!Law::listed(id))&&(0..NODES.len()).filter(|&id|Law::listed(id)).count()==NODES.len()-RETIRED.len());
        let mut run=crate::Run::new(3,crate::Class::Samurai);run.set_law(lone).unwrap();assert!(!run.law.lone_wolf(),"new runs drop it");
    }
    #[test]
    fn elite_pressure_survives_act_transitions_and_keeps_legacy_maps() {
        let mut run=crate::Run::new(42,crate::Class::Samurai);run.set_law(Law::default().toggle(34)).unwrap();
        for act in 1..=3 {
            assert_eq!(run.act,act);assert_eq!(run.nodes,crate::law_graph(run.seed,7,run.law));
            assert_eq!(crate::Run::decode(&run.encode().unwrap()).unwrap().nodes,run.nodes);
            if act<3 {let boss=run.nodes.iter().position(|n|n.kind==crate::NodeKind::Boss).unwrap();run.finish(boss);}
        }
        for schema in 1..=7 {
            let mut old=crate::Run::new(42,crate::Class::Samurai);old.schema=schema;old.nodes=crate::law_graph(42,schema,Law::default());
            assert_eq!(crate::Run::decode(&old.encode().unwrap()).unwrap().nodes,old.nodes);
        }
    }
    #[test]
    fn different_rules_can_be_selected_and_removed_independently() {
        for id in [5,21,23,26,31,32] {
            let law=Law::default().toggle(id);
            assert_eq!(law.0,1u64<<id);assert!(law.valid());assert!(law.missing_prerequisites(id).is_empty());
            assert_eq!(Law::parse(&law.code()),Some(law));
        }
        let economy=Law::default().toggle(20).toggle(22).toggle(23);
        let remaining=economy.toggle(19);
        assert_eq!(remaining.rune_percent(),100);assert_eq!(remaining.price_percent(),140);assert!(remaining.stingy());
        let enemies=Law::default().toggle(2).toggle(33).toggle(4).toggle(5).toggle(31);
        let remaining=enemies.toggle(0).toggle(3);
        assert_eq!(remaining.enemy_tiers(),0);assert!(remaining.elite_affixes()&&remaining.empowered_affixes());
        assert_eq!(remaining.elite_pacing(),crate::encounter::Pacing::Together);
        assert_eq!((Law::default().toggle(32).score(),Law::default().toggle(33).score()),(20,60));
    }
    #[test]
    fn effects_follow_each_route() {
        let law=|ids:&[usize]|ids.iter().fold(Law::default(),|l,&id|l.toggle(id));
        assert_eq!((law(&[]).enemy_tiers(),law(&[0]).enemy_tiers(),law(&[0,1]).enemy_tiers(),law(&[0,1,2]).enemy_tiers()),(0,1,2,4));
        assert_eq!((law(&[]).flask_cut(),law(&[8]).flask_cut(),law(&[8,9]).flask_cut(),law(&[8,9,10]).flask_cut()),([0,0],[1,0],[1,1],[2,2]));
        assert_eq!((law(&[]).affix_budget(2,6),law(&[3]).affix_budget(2,6),law(&[3,4]).affix_budget(2,6),law(&[3]).affix_budget(6,6)),(2,4,6,6));
        assert_eq!((law(&[19]).runes(1000),law(&[19,20]).runes(1000),law(&[]).runes(1000)),(850,700,1000));
        assert_eq!((law(&[21]).price_percent(),law(&[22]).price_percent()),(120,140));
        assert_eq!((law(&[24]).time_limit(),law(&[24,25]).time_limit(),law(&[]).time_limit()),(Some(180),Some(120),None));
        let all=law(&(0..NODES.len()).collect::<Vec<_>>());
        assert!(!all.lone_wolf()&&Law(1<<14).lone_wolf(),"retired: cannot be chosen, old runs keep it");
        assert!(all.stingy()&&all.envy_world()&&all.elite_affixes()&&all.empowered_affixes());
        assert!(!law(&[3,4]).empowered_affixes()&&law(&[3,4,5,31]).empowered_affixes());
        assert_eq!((all.fragile(),all.fatigue(),all.focus(),all.toughness()),(3,2,2,2));
    }
    #[test]
    fn codes_round_trip_and_reject_typos() {
        let laws=[Law::default(),Law::default().toggle(0).toggle(8).toggle(9),(0..NODES.len()).fold(Law::default(),|l,id|l.toggle(id))];
        for law in laws {let code=law.code();assert!((7..=8).contains(&code.len()));assert_eq!(Law::parse(&code),Some(law));assert_eq!(Law::parse(&code.to_lowercase()),Some(law));}
        let code=laws[1].code();let mut broken:Vec<char>=code.chars().collect();broken[6]=if broken[6]=='0'{'1'}else{'0'};
        assert_eq!(Law::parse(&broken.into_iter().collect::<String>()),None);
        assert_eq!(Law::parse(""),None);assert_eq!(Law::parse("ZZZZZZZZ"),None);
    }
    #[test]
    fn runs_keep_the_law_and_apply_its_rules() {
        use crate::{Class,Phase,Reward,Run,NodeKind,rewards::{lookup,ItemKind}};
        let law=|ids:&[usize]|ids.iter().fold(Law::default(),|l,&id|l.toggle(id));
        let plain=Run::new(7,Class::Samurai);
        assert!(!plain.encode().unwrap().contains("\"law\""),"saves without a law keep the old format");
        let mut run=plain.clone();run.set_law(law(&[8,9,10,19,20,21,22,23])).unwrap();
        assert_eq!((run.flask_limit,run.flasks),(6,6));
        assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
        assert!(Run::decode(&plain.encode().unwrap()).unwrap().law.is_empty());
        let mut bad=run.clone();bad.law=Law(1<<1);assert!(bad.validate().is_err());
        let mut started=plain.clone();let id=started.available()[0];started.enter(id).unwrap();assert!(started.set_law(law(&[0])).is_err());
        let mut lone=0;
        for seed in 0..300 {
            let mut free=Run::new(seed,Class::Astrologer);let mut bound=free.clone();bound.set_law(law(&[11,12,13,19,20,21,22,23])).unwrap();bound.law=Law(bound.law.0|1<<14);
            let id=free.available()[0];assert_eq!(free.nodes[id].kind,NodeKind::Monster);
            for r in [&mut free,&mut bound] {r.enter(id).unwrap();r.win_combat().unwrap();}
            let (Phase::Reward{offers:a,..},Phase::Reward{offers:b,..})=(&free.phase,&bound.phase) else{panic!()};
            assert_eq!(b.len(),2);
            assert!(b.iter().all(|r|!matches!(r,Reward::Item(i) if lookup(*i).is_some_and(|i|i.kind==ItemKind::Spirit))));
            lone+=a.iter().filter(|r|matches!(r,Reward::Item(i) if lookup(*i).is_some_and(|i|i.kind==ItemKind::Spirit))).count();
            for (x,y) in a.iter().zip(b) {if let (Reward::Gold(x),Reward::Gold(y))=(x,y){assert_eq!(*y,bound.law.runes(*x));}}
            assert_eq!(Run::decode(&bound.encode().unwrap()).unwrap(),bound);
            bound.claim_reward(1).unwrap();
        }
        assert!(lone>0,"without the law spirits still appear");
        let mut capped=plain.clone();capped.set_law(law(&[8])).unwrap();
        for _ in 0..4 {capped.grant(Reward::FlaskCapacity).unwrap();}
        assert_eq!(capped.flask_limit,17);assert!(capped.validate().is_ok());
        assert!(capped.align_party_start(capped.seed).is_ok(),"matching party routes preserve the selected law");
    }
}
