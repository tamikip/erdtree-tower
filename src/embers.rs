use serde::{Deserialize,Serialize};

/// 余烬：誓约三选一里的机制增益，没有代价，不能深化；选过的整局生效
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash,Serialize,Deserialize)]
pub enum Ember {Execution,Bloodlord,Frost,Deathflame,Miquella,Rise,DragonHeart,Revenge,Frenzy,Echo,Feast,Thorns,SpiritLink,SpiritBurst,SpiritFeast,SpiritCommand,SpiritTwin,SoulEater,Chain,Godslayer,SpiritKeeper,Barrier,Meditation,Glintstone,FullMoon,Footwork,SpiritResonance,Pouch,Arts,Bulwark,Breaker,Harvest,LastStand,Martyr,Mercy,Battlemage,Stride,FrostShackle}

// 运行时数值（参数类的数值见 scene-builder/EmberAssets.cs，说明文字同步引用这些常量）
pub const EXECUTION_SECONDS:u32=10;
pub const EXECUTION_STAMINA_CUT:u32=50;
pub const BUILDUP:u32=40;
pub const DEATHFLAME_RADIUS:f32=4.0;
pub const DEATHFLAME_PERCENT:u32=8;
pub const LIFESTEAL_PERCENT:u32=8;
pub const RISE_BELOW_PERCENT:u32=10;
pub const RISE_TO_PERCENT:u32=30;
pub const DRAGON_RESTORE_PERCENT:u32=50;
pub const DRAGON_COOLDOWN_SECONDS:u64=30;
pub const REVENGE_SECONDS:u64=3;
pub const REVENGE_PERCENT:u32=30;
pub const FRENZY_STEP_PERCENT:u32=2;
pub const FRENZY_STACKS:u8=40;
/// 圣杯余响：喝圣杯瓶后的增益（秒数、造成的伤害提高、受到的伤害降低，百分比）
pub const ECHO_SECONDS:u64=20;
pub const ECHO_ATTACK_PERCENT:u32=15;
pub const ECHO_CUT_PERCENT:u32=15;
pub const FEAST_PER_FIGHT:u8=1;
pub const THORNS_PERCENT:u32=40;
// 召唤流余烬（只在身上带着骨灰时出现）
pub const SPIRIT_SHARE_PERCENT:u32=30;
pub const SPIRIT_BURST_RADIUS:f32=6.0;
pub const SPIRIT_BURST_PERCENT:u32=20;
pub const SPIRIT_FEAST_HEAL_PERCENT:u32=20;
pub const SPIRIT_FEAST_FP_PERCENT:u32=20;
pub const SPIRIT_COMMAND_PERCENT:u32=20;
/// 灵唤师的共鸣：召唤物在场时，玩家自己造成的伤害提高（召唤物的伤害不变）
pub const SPIRIT_RESONANCE_PERCENT:u32=40;
// 叠层余烬：层数由战斗中的事件累积，整局保留（Run::ember_stacks），不设上限
/// 噬魂者：击倒普通敌人、精英、Boss 分别得魂点，每满 SOUL_POINTS 点一层
pub const SOUL_POINTS:u32=5;
pub const SOUL_KILL:[u32;3]=[1,3,10];
pub const SOUL_STEP_PERCENT:u32=3;
pub const CHAIN_STEP_PERCENT:u32=5;
pub const CHAIN_ROOM_LIMIT:u8=3;
pub const SLAYER_HP_PERCENT:u32=5;
pub const SLAYER_ATTACK_PERCENT:u32=4;
pub const KEEPER_ATTACK_PERCENT:u32=3;
/// 灵魂收容的减伤逐层乘算（每层受到的伤害 ×0.98），层数再多也不会到 100%
pub const KEEPER_CUT_PERCENT:u32=2;
// 法师余烬（只在身上带着法杖或圣印记时出现）。“施法时”：按住施法键期间，以及松开后 CAST_LINGER_MS 毫秒内（出招动作还没放完）
pub const CAST_LINGER_MS:u64=1200;
pub const BARRIER_CUT_PERCENT:u32=50;
pub const GLINTSTONE_FP_PERCENT:u32=25;
pub const MOON_FP_PERCENT:u32=70;
pub const MOON_ATTACK_PERCENT:u32=50;
/// 战技的精粹：战技造成的伤害提高、战技消耗的专注降低（百分比）
pub const ARTS_ATTACK_PERCENT:u32=25;
pub const ARTS_FP_CUT_PERCENT:u32=50;
/// 直接扣敌人生命的余烬对 Boss 每次最多扣最大生命的 1%
pub const BOSS_CAP_PERCENT:u32=1;
/// 处刑者的血宴：按敌人类型分别乘 1.02、1.05、1.10；点数仅用于显示，旧存档点数按原规则保留。
pub const HARVEST_POINTS:[u32;3]=[2,5,10];
/// 红狮子的不退：生命低于这个比例时，强韧度提高（受到的削韧 ×100/(100+LAST_STAND_POISE_PERCENT)，原版牛羊护符 +33% 即 ×0.75）
pub const LAST_STAND_HP_PERCENT:u32=40;
/// 殉道者的苦修：失去最大生命的 1% 回复最大专注的 MARTYR_FP_PERCENT/100 %（100 即 1:1，按两边的上限换算）
pub const MARTYR_FP_PERCENT:u32=200;
/// 黄金树的恩泽：治疗类祷告（含给队友的、持续回复的）回复量提高的百分比
pub const MERCY_HEAL_PERCENT:i32=50;
/// 战斗法师：受到的伤害按智力、信仰较高者每点降低 0.4%（千分比 4），最低保留 5%；一局最多转换 BATTLEMAGE_WEAPONS 把武器
pub const BATTLEMAGE_CUT_PERMILLE:u32=4;
pub const BATTLEMAGE_WEAPONS:usize=8;
/// 智力 / 信仰（取高）为 points 时受到的伤害倍率
pub fn battlemage_cut(points:u32)->f32 {(1.0-(points.min(1000)*BATTLEMAGE_CUT_PERMILLE) as f32/1000.0).max(0.05)}
impl crate::Run {
    /// 战斗法师：在铁匠处给一把近战武器（武器参数行 row）附加智力（0）或信仰（1）S 补正；同一把再选一次就换成另一种
    pub fn battlemage_convert(&mut self,row:u32,stat:u8)->crate::RuleResult<()> {
        if !self.embers.contains(&Ember::Battlemage) {return Err("没有持有余烬「战斗法师」".into());}
        if stat>1 {return Err("无效的补正".into());}
        if let Some(entry)=self.battlemage.iter_mut().find(|(r,_)|*r==row) {entry.1=stat;return Ok(());}
        if self.battlemage.len()>=BATTLEMAGE_WEAPONS {return Err(format!("一局最多转换 {BATTLEMAGE_WEAPONS} 把武器"));}
        self.battlemage.push((row,stat));Ok(())
    }
}
/// 受到伤害时按比例回复的专注（taken：这一帧掉的生命）
pub fn martyr_fp(taken:i32,max_hp:i32,max_fp:i32)->i32 {
    if taken<=0||max_hp<=0||max_fp<=0 {return 0;}
    (i64::from(taken)*i64::from(max_fp)*i64::from(MARTYR_FP_PERCENT)/(i64::from(max_hp)*100)).min(i64::from(max_fp)) as i32
}
pub const LAST_STAND_POISE_PERCENT:u32=200;
pub const HARVEST_STEP_PERCENT:u32=1;
/// 倍率计算的点数上限：只防浮点溢出（1.01^1000 约 2 万倍），正常一局远远到不了，规则上视为不设上限
pub const HARVEST_POINT_CAP:u32=1000;

impl Ember {
    pub const ALL:[Self;37]=[Self::Execution,Self::Bloodlord,Self::Frost,Self::Deathflame,Self::Miquella,Self::Rise,Self::DragonHeart,Self::Revenge,Self::Frenzy,Self::Echo,Self::Feast,Self::Thorns,
        Self::SpiritLink,Self::SpiritBurst,Self::SpiritFeast,Self::SpiritCommand,Self::SpiritTwin,Self::SoulEater,Self::Chain,Self::Godslayer,Self::SpiritKeeper,
        Self::Barrier,Self::Meditation,Self::Glintstone,Self::FullMoon,Self::Footwork,Self::SpiritResonance,Self::Pouch,Self::Arts,Self::Bulwark,Self::Breaker,Self::Harvest,Self::LastStand,Self::Martyr,Self::Mercy,Self::Battlemage,Self::FrostShackle];
    // Retained only for old saves and archived runs.
    pub fn retired(self)->bool {matches!(self,Self::Stride|Self::SpiritTwin)}
    pub fn index(self)->usize {match self {Self::Stride=>36,Self::FrostShackle=>37,_=>Self::ALL.iter().position(|e|*e==self).unwrap()}}
    pub fn icon_index(self)->usize {if self==Self::FrostShackle{Self::Frost.index()}else{self.index()}}
    pub fn name(self)->&'static str {match self {
        Self::FrostShackle=>"寒霜枷锁",
        Self::Execution=>"无声处刑",Self::Bloodlord=>"鲜血君王的赐福",Self::Frost=>"永冻吐息",Self::Deathflame=>"死诞之焰",
        Self::Miquella=>"米凯拉之刃",Self::Rise=>"荷莱·露的再起",Self::DragonHeart=>"古龙之心",Self::Revenge=>"以血还血",
        Self::Frenzy=>"癫火狂热",Self::Echo=>"圣杯余响",Self::Feast=>"噬神之蛇的饱食",Self::Thorns=>"荆棘之躯",
        Self::SpiritLink=>"仿身泪滴的替身",Self::SpiritBurst=>"灵骸迸裂",Self::SpiritFeast=>"众灵的盛宴",Self::SpiritCommand=>"灵唤铃的号令",
        Self::SpiritTwin=>"双生的灵唤铃",Self::SoulEater=>"噬魂者",Self::Chain=>"处刑之链",Self::Godslayer=>"屠神者",Self::SpiritKeeper=>"灵魂收容",
        Self::Barrier=>"托普斯的力场",Self::Meditation=>"亚兹勒的冥想",Self::Glintstone=>"辉石的余韵",Self::FullMoon=>"蕾娜拉的满月",
        Self::Footwork=>"黑刀刺客的身法",Self::SpiritResonance=>"灵唤师的共鸣",Self::Pouch=>"行商的无底囊",Self::Arts=>"战技的精粹",Self::Bulwark=>"坩埚骑士的铁壁",Self::Breaker=>"鲜血骑士的破势",Self::Harvest=>"处刑者的血宴",Self::LastStand=>"红狮子的不退",Self::Martyr=>"殉道者的苦修",Self::Mercy=>"黄金树的恩泽",Self::Battlemage=>"战斗法师",Self::Stride=>"已移除的余烬",
    }}
    pub fn effect(self)->String {match self {
        Self::FrostShackle=>"敌人触发冻伤后，持续 10 秒\n行动速度 -30%，造成伤害 -20%\n再次触发刷新持续时间，不叠加".into(),
        Self::Execution=>format!("致命一击后 {EXECUTION_SECONDS} 秒内\n精力消耗 -{EXECUTION_STAMINA_CUT}%"),
        Self::Bloodlord=>format!("每次命中积累\n{BUILDUP} 点出血"),
        Self::Frost=>format!("每次命中积累\n{BUILDUP} 点冻伤"),
        Self::Deathflame=>format!("敌人倒下时，对 {DEATHFLAME_RADIUS} 米内其他敌人\n造成其最大生命 {DEATHFLAME_PERCENT}% 的伤害\n对 Boss 最多 {BOSS_CAP_PERCENT}%"),
        Self::Miquella=>format!("造成伤害的 {LIFESTEAL_PERCENT}%\n转为生命"),
        Self::Rise=>format!("每场战斗一次：生命低于 {RISE_BELOW_PERCENT}% 时\n回复到 {RISE_TO_PERCENT}%"),
        Self::DragonHeart=>format!("精力耗尽时回复 {DRAGON_RESTORE_PERCENT}%\n冷却 {DRAGON_COOLDOWN_SECONDS} 秒"),
        Self::Revenge=>format!("受伤后 {REVENGE_SECONDS} 秒内\n下一次命中伤害 +{REVENGE_PERCENT}%"),
        Self::Frenzy=>format!("每次命中攻击 +{FRENZY_STEP_PERCENT}%，最多 +{}%\n受伤时清零",FRENZY_STEP_PERCENT*u32::from(FRENZY_STACKS)),
        Self::Echo=>format!("喝圣杯瓶后 {ECHO_SECONDS} 秒内\n造成的伤害提高 {ECHO_ATTACK_PERCENT}%\n受到的伤害降低 {ECHO_CUT_PERCENT}%"),
        Self::Feast=>format!("敌人倒下时补回 1 次红圣杯瓶\n每场战斗最多 {FEAST_PER_FIGHT} 次"),
        Self::Thorns=>format!("受到伤害的 {THORNS_PERCENT}%\n反弹给最近的敌人\n对 Boss 每次最多 {BOSS_CAP_PERCENT}%"),
        Self::SpiritLink=>format!("召唤物在场时\n你受到伤害的 {SPIRIT_SHARE_PERCENT}%\n改由召唤物承受"),
        Self::SpiritBurst=>format!("召唤物倒下或消失时，对 {SPIRIT_BURST_RADIUS} 米内敌人\n造成其最大生命 {SPIRIT_BURST_PERCENT}% 的伤害\n对 Boss 最多 {BOSS_CAP_PERCENT}%"),
        Self::SpiritFeast=>format!("敌人倒下时\n召唤物回复 {SPIRIT_FEAST_HEAL_PERCENT}% 生命\n你回复 {SPIRIT_FEAST_FP_PERCENT}% 专注"),
        Self::SpiritCommand=>format!("召唤物造成的伤害 +{SPIRIT_COMMAND_PERCENT}%\n受到的伤害 -{SPIRIT_COMMAND_PERCENT}%"),
        Self::SpiritTwin=>"每场战斗可以再召唤一次骨灰\n两组召唤物同时在场\n胜利时各组存活则返还骨灰".into(),
        Self::SoulEater=>format!("击倒得魂：小怪{} 精英{} Boss{}\n每 {SOUL_POINTS} 魂一层，每层攻击 +{SOUL_STEP_PERCENT}%\n层数不设上限，整局保留",SOUL_KILL[0],SOUL_KILL[1],SOUL_KILL[2]),
        Self::Chain=>format!("每次致命一击叠一层，每关最多 {CHAIN_ROOM_LIMIT} 层\n每层致命一击伤害 +{CHAIN_STEP_PERCENT}%\n总层数不设上限，整局保留"),
        Self::Godslayer=>format!("每赢一场精英战或 Boss 战叠一层\n每层最大生命 +{SLAYER_HP_PERCENT}%、攻击 +{SLAYER_ATTACK_PERCENT}%\n层数不设上限，整局保留"),
        Self::SpiritKeeper=>format!("召唤物在场时击倒敌人叠一层\n每层召唤物伤害 +{KEEPER_ATTACK_PERCENT}%\n受到的伤害 -{KEEPER_CUT_PERCENT}%（逐层乘算）\n层数不设上限，整局保留"),
        Self::Barrier=>format!("施放法术或祷告时\n受到的伤害降低 {BARRIER_CUT_PERCENT}%"),
        Self::Meditation=>"施放法术或祷告时霸体\n不会被打断".into(),
        Self::Glintstone=>format!("法术与祷告消耗的专注\n降低 {GLINTSTONE_FP_PERCENT}%"),
        Self::Footwork=>"翻滚、后撤步与致命一击\n不消耗精力".into(),
        Self::Pouch=>"飞刀、投掷壶等投掷道具\n使用后不再消耗".into(),
        Self::Arts=>format!("战技伤害 +{ARTS_ATTACK_PERCENT}%\n战技消耗的专注 -{ARTS_FP_CUT_PERCENT}%"),
        Self::Bulwark=>"使用大盾格挡时不消耗精力\n不影响负重、翻滚与移动速度".into(),
        Self::Stride=>"已移除，不再提供效果".into(),
        Self::Battlemage=>format!("可在铁匠处免费让近战武器\n获得智力或信仰 S 补正\n受到的伤害降低\n智力、信仰取高 ×{}%",BATTLEMAGE_CUT_PERMILLE as f32/10.0),
        Self::Mercy=>format!("治疗类祷告的回复量\n提高 {MERCY_HEAL_PERCENT}%"),
        Self::Martyr=>format!("受到伤害时回复专注\n失去最大生命的 1%\n回复最大专注的 {}%",MARTYR_FP_PERCENT as f32/100.0),
        Self::LastStand=>format!("生命低于 {LAST_STAND_HP_PERCENT}% 时\n韧性（强韧度）提高 {LAST_STAND_POISE_PERCENT}%"),
        Self::Harvest=>format!("致命一击击杀敌人时\n最大生命提高：小怪 {}%\n精英 {}%、Boss {}%\n在当前最大生命上逐次乘算\n不设上限，整局保留",HARVEST_POINTS[0],HARVEST_POINTS[1],HARVEST_POINTS[2]),
        Self::Breaker=>"敌人出血、冻伤、睡眠、发狂发作时\n压低架势，5 秒内下一次削韧命中触发失衡\nBoss 与普通敌人均无触发冷却".into(),
        Self::FullMoon=>format!("专注不低于 {MOON_FP_PERCENT}% 时\n造成的伤害提高 {MOON_ATTACK_PERCENT}%"),
        Self::SpiritResonance=>format!("召唤物在场时\n你造成的伤害 +{SPIRIT_RESONANCE_PERCENT}%"),
    }}
    /// 品质：余烬只有史诗（2）与传说（3），按强度定，决定抽到的概率与奖励卡的颜色
    pub fn quality(self)->u8 {match self {
        Self::Rise|Self::SpiritTwin|Self::Miquella|Self::Feast|Self::Godslayer|Self::Meditation|Self::SpiritResonance|Self::Breaker|Self::Harvest|Self::Battlemage=>3,
        _=>2,
    }}
    /// 只影响敌人的余烬：联机客机上不生效（敌人由房主计算）
    /// 叠层余烬（层数由战斗中的事件累积，不设上限）
    pub fn stacking(self)->bool {matches!(self,Self::SoulEater|Self::Chain|Self::Godslayer|Self::SpiritKeeper|Self::Harvest)}
    /// 当前层数的效果，显示在构筑面板里
    pub fn stack_bonus(self,stacks:u32)->Option<String> {
        let r=stack_rates(self,stacks)?;
        let percent=|rate:f32|((rate-1.0)*100.0).round() as i64;
        Some(match self {
            Self::SoulEater=>format!("攻击 +{}%",percent(r.attack)),
            Self::Chain=>format!("致命一击伤害 +{}%",percent(r.attack)),
            Self::Godslayer=>format!("最大生命 +{}%、攻击 +{}%",percent(r.hp),percent(r.attack)),
            Self::Harvest=>format!("最大生命 +{}%",percent(r.hp)),
            Self::SpiritKeeper=>format!("召唤物伤害 +{}%、受到的伤害 -{}%",percent(r.attack),-percent(r.cut)),
            _=>return None,
        })
    }
    pub fn affects_enemies(self)->bool {matches!(self,Self::Bloodlord|Self::Frost|Self::Deathflame|Self::Thorns|Self::SpiritBurst)}
    /// 召唤流余烬：只有身上带着骨灰时才会出现在奖励里
    pub fn needs_spirit(self)->bool {matches!(self,Self::SpiritLink|Self::SpiritBurst|Self::SpiritFeast|Self::SpiritCommand|Self::SpiritTwin|Self::SpiritKeeper|Self::SpiritResonance)}
    /// 法师余烬：只有身上带着法杖或圣印记时才会出现在奖励里
    pub fn needs_casting(self)->bool {matches!(self,Self::Barrier|Self::Meditation|Self::Glintstone|Self::FullMoon)}
    /// 这局的构筑能不能抽到它
    /// 职业专属余烬（余烬规则 3 起）：只有这些职业抽得到；None 为通用余烬，所有职业都能抽到
    pub fn classes(self)->Option<&'static [crate::Class]> {
        use crate::Class::*;
        match self {
            Self::Bulwark|Self::Chain=>Some(&[Vagabond]),
            Self::Frenzy|Self::Arts|Self::LastStand=>Some(&[Hero]),
            Self::Mercy=>Some(&[Prophet,Confessor]),
            Self::Stride=>Some(&[Astrologer,Prophet,Confessor]),
            Self::Bloodlord|Self::Frost|Self::Footwork|Self::Breaker=>Some(&[Samurai]),
            Self::Barrier|Self::Meditation|Self::Glintstone|Self::FullMoon|Self::Martyr|Self::Battlemage=>Some(&[Astrologer,Prophet]),
            Self::SpiritLink|Self::SpiritBurst|Self::SpiritFeast|Self::SpiritCommand|Self::SpiritTwin|Self::SpiritKeeper|Self::SpiritResonance=>Some(&[Confessor]),
            _=>None,
        }
    }
    /// class：余烬规则 3 起按职业限制（None 不限职业，旧存档）
    fn fits(self,spirits:bool,casting:bool,class:Option<crate::Class>)->bool {
        (spirits||!self.needs_spirit())&&(casting||!self.needs_casting())&&class.is_none_or(|class|self.classes().is_none_or(|classes|classes.contains(&class)))
    }
}
/// 作用在敌人身上的余烬效果。联机成员在本机算出（只算自己与自己召唤物的命中、击倒），随汇报发给房主，由房主执行。
/// slot：这场战斗敌人名单（enemies::node_roster）里的位置，房主与成员一致
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum Strike {
    /// 命中：鲜血君王的赐福、永冻吐息的异常积累
    Buildup{slot:u8,ember:Ember},
    /// 按目标最大生命的百分比扣血（Boss 有上限）：死诞之焰、圣杯余响、灵骸迸裂
    Wound{slot:u8,ember:Ember},
    /// 荆棘之躯：按成员受到的伤害反弹
    Thorns{slot:u8,taken:i32},
}
/// 荆棘之躯一次反弹按受到的伤害计，超过这个数视为无效请求
pub const THORNS_TAKEN_LIMIT:i32=100_000;
/// 汇报里最多带的敌人侧效果条数（最近的若干条；房主按序号执行没执行过的）
pub const STRIKE_WINDOW:usize=32;
impl Strike {
    pub fn slot(self)->u8 {match self {Self::Buildup{slot,..}|Self::Wound{slot,..}|Self::Thorns{slot,..}=>slot}}
    pub fn ember(self)->Ember {match self {Self::Buildup{ember,..}|Self::Wound{ember,..}=>ember,Self::Thorns{..}=>Ember::Thorns}}
    /// 成员确实带着这个余烬，且效果类型与余烬对得上
    pub fn valid(self,held:&[Ember])->bool {
        held.contains(&self.ember())&&match self {
            Self::Buildup{ember,..}=>matches!(ember,Ember::Bloodlord|Ember::Frost),
            Self::Wound{ember,..}=>wound_percent(ember).is_some(),
            Self::Thorns{taken,..}=>(1..=THORNS_TAKEN_LIMIT).contains(&taken),
        }
    }
}
/// 按目标最大生命百分比扣血的余烬
pub fn wound_percent(ember:Ember)->Option<u32> {match ember {
    Ember::Deathflame=>Some(DEATHFLAME_PERCENT),Ember::SpiritBurst=>Some(SPIRIT_BURST_PERCENT),_=>None,
}}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct StrikeRequest {pub seq:u32,pub strike:Strike}
/// 战斗中让叠层余烬加层的事件
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Gain {
    /// 击倒敌人：rank 0 普通、1 精英、2 Boss；spirit 为召唤物当时在场
    Kill{rank:u8,spirit:bool},
    Critical,
    /// 赢下一场精英战或 Boss 战
    Slain,
    /// 致命一击击杀敌人（rank 同 Kill）：处刑者的血宴
    CriticalKill{rank:u8},
}
/// 叠层余烬在某个层数下的倍率（攻击倍率、最大生命倍率、受到伤害的倍率），运行时写进效果参数
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Rates {pub attack:f32,pub hp:f32,pub cut:f32}
pub fn stack_rates(ember:Ember,stacks:u32)->Option<Rates> {
    let step=|percent:u32|1.0+(percent as f32)*(stacks as f32)/100.0;
    let plain=Rates{attack:1.0,hp:1.0,cut:1.0};
    Some(match ember {
        Ember::SoulEater=>Rates{attack:step(SOUL_STEP_PERCENT),..plain},
        Ember::Chain=>Rates{attack:step(CHAIN_STEP_PERCENT),..plain},
        Ember::Godslayer=>Rates{attack:step(SLAYER_ATTACK_PERCENT),hp:step(SLAYER_HP_PERCENT),..plain},
        Ember::Battlemage=>Rates{cut:battlemage_cut(stacks),..plain},
        Ember::Harvest=>Rates{hp:(1.0+HARVEST_STEP_PERCENT as f32/100.0).powi(stacks.min(HARVEST_POINT_CAP) as i32),..plain},
        Ember::SpiritKeeper=>Rates{attack:step(KEEPER_ATTACK_PERCENT),cut:(1.0-KEEPER_CUT_PERCENT as f32/100.0).powi(stacks.min(10_000) as i32),..plain},
        _=>return None,
    })
}
fn is_zero(value:&u32)->bool {*value==0}
fn no_harvest_kills(value:&[u32;3])->bool {*value==[0;3]}
/// 叠层余烬的进度，整局保留；只记拿到余烬之后的事件
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
pub struct Stacks {
    /// 噬魂者的魂点（层数 = 魂点 / SOUL_POINTS）
    #[serde(default)] pub souls:u32,
    #[serde(default)] pub chain:u32,
    #[serde(default)] pub slayer:u32,
    #[serde(default)] pub keeper:u32,
    /// 处刑者的血宴的点数（层数 = 点数）
    #[serde(default,skip_serializing_if="is_zero")] pub harvest:u32,
    /// 新规则按敌人类型分别记击杀数；旧存档的 harvest 点数保留原倍率。
    #[serde(default,skip_serializing_if="no_harvest_kills")] pub harvest_kills:[u32;3],
    #[serde(default,skip_serializing_if="Option::is_none")] chain_room:Option<(u64,u8,usize,u8)>,
}
impl Stacks {
    pub fn rates(&self,ember:Ember)->Option<Rates> {
        let mut rates=stack_rates(ember,self.level(ember))?;
        if ember==Ember::Harvest {
            let new_points=self.harvest_kills.iter().zip(HARVEST_POINTS).map(|(&n,p)|u64::from(n)*u64::from(p)).sum::<u64>();
            let legacy=u64::from(self.harvest).saturating_sub(new_points).min(u64::from(HARVEST_POINT_CAP));
            let mut hp=1.01f64.powi(legacy as i32);
            for (n,p) in self.harvest_kills.into_iter().zip(HARVEST_POINTS) {hp*=(1.0+f64::from(p)/100.0).powi(n.min(HARVEST_POINT_CAP) as i32);}
            rates.hp=hp.min(1.01f64.powi(HARVEST_POINT_CAP as i32)) as f32;
        }
        Some(rates)
    }
    pub fn bonus(&self,ember:Ember)->Option<String> {
        if ember==Ember::Harvest {Some(format!("最大生命 +{}%",((self.rates(ember)?.hp-1.0)*100.0).round() as i64))}
        else {ember.stack_bonus(self.level(ember))}
    }
    pub fn gain_in_room(&mut self,held:&[Ember],event:Gain,key:(u64,u8,usize))->Vec<Ember> {
        if !matches!(event,Gain::Critical)||!held.contains(&Ember::Chain) {return self.gain(held,event);}
        let (seed,act,id)=key;
        if self.chain_room.map(|(s,a,n,_)|(s,a,n))!=Some(key) {self.chain_room=Some((seed,act,id,0));}
        if self.chain_room.is_some_and(|(_,_,_,count)|count>=CHAIN_ROOM_LIMIT) {return Vec::new();}
        let raised=self.gain(held,event);
        if raised.contains(&Ember::Chain) {self.chain_room.as_mut().unwrap().3+=1;}
        raised
    }
    pub fn is_empty(&self)->bool {*self==Self::default()}
    pub fn level(&self,ember:Ember)->u32 {match ember {
        Ember::SoulEater=>self.souls/SOUL_POINTS,Ember::Chain=>self.chain,Ember::Godslayer=>self.slayer,Ember::SpiritKeeper=>self.keeper,Ember::Harvest=>self.harvest,_=>0,
    }}
    /// 记一次事件；返回因此升层的余烬（用于提示）
    pub fn gain(&mut self,held:&[Ember],event:Gain)->Vec<Ember> {
        let before=*self;
        let has=|e|held.contains(&e);
        match event {
            Gain::Kill{rank,spirit}=>{
                if has(Ember::SoulEater) {self.souls=self.souls.saturating_add(SOUL_KILL[usize::from(rank.min(2))]);}
                if has(Ember::SpiritKeeper)&&spirit {self.keeper=self.keeper.saturating_add(1);}
            },
            Gain::Critical=>if has(Ember::Chain) {self.chain=self.chain.saturating_add(1);},
            Gain::Slain=>if has(Ember::Godslayer) {self.slayer=self.slayer.saturating_add(1);},
            Gain::CriticalKill{rank}=>if has(Ember::Harvest) {let i=usize::from(rank.min(2));self.harvest=self.harvest.saturating_add(HARVEST_POINTS[i]);self.harvest_kills[i]=self.harvest_kills[i].saturating_add(1);},
        }
        held.iter().copied().filter(|e|self.level(*e)>before.level(*e)).collect()
    }
}
/// 按种子抽一张没拿过的余烬；spirits 为 false 时不出召唤流余烬，casting 为 false 时不出法师余烬
pub fn offer(seed:u64,held:&[Ember],spirits:bool,casting:bool,class:Option<crate::Class>)->Option<Ember> {
    offer_in(seed,held,spirits,casting,class,false)
}
fn pool(held:&[Ember],spirits:bool,casting:bool,class:Option<crate::Class>,legacy:bool)->Vec<Ember> {
    Ember::ALL.into_iter().chain(legacy.then_some(Ember::Stride)).filter(|e|(legacy||!e.retired())&&!held.contains(e)&&e.fits(spirits,casting,class)).collect()
}
fn offer_in(seed:u64,held:&[Ember],spirits:bool,casting:bool,class:Option<crate::Class>,legacy:bool)->Option<Ember> {
    let pool=pool(held,spirits,casting,class,legacy);
    (!pool.is_empty()).then(||pool[(crate::mix(seed^0xe3be_12a5) % pool.len() as u64) as usize])
}
/// 余烬规则 1 起按品质抽：先按 odds（普通、稀有、史诗、传说，合计 100）掷品质，再在这个品质里挑没拿过的；
/// 这个品质已经没有可抽的，就依次往低一档、再往高一档找
pub fn offer_by_quality(seed:u64,held:&[Ember],spirits:bool,casting:bool,class:Option<crate::Class>,odds:[u32;4])->Option<Ember> {
    offer_quality_in(seed,held,spirits,casting,class,odds,false)
}
fn offer_quality_in(seed:u64,held:&[Ember],spirits:bool,casting:bool,class:Option<crate::Class>,odds:[u32;4],legacy:bool)->Option<Ember> {
    let pool=pool(held,spirits,casting,class,legacy);
    if pool.is_empty() {return None;}
    let roll=(crate::mix(seed^0x51a1_17e5)%100) as u32;
    let mut below=0;let rolled=odds.iter().position(|chance|{below+=chance;roll<below}).unwrap_or(3) as u8;
    let order=(0..=rolled).rev().chain(rolled+1..4);
    for tier in order {
        let candidates:Vec<_>=pool.iter().copied().filter(|e|e.quality()==tier).collect();
        if !candidates.is_empty() {return Some(candidates[(crate::mix(seed^0xe3be_12a5) % candidates.len() as u64) as usize]);}
    }
    None
}
/// 余烬的品质概率：只有史诗与传说，传说的机会按装备品质概率里“传说 : 史诗”的比例换算（第一幕开头约 15%，第三幕末约 28%）
pub fn odds(items:[u32;4])->[u32;4] {
    let legendary=(items[3]*100+(items[2]+items[3])/2)/(items[2]+items[3]).max(1);
    [0,0,100-legendary,legendary]
}
/// 普通战奖励有 1/4 的概率把最后一个奖励换成余烬，精英战必定有一张；Boss 与宝箱不出余烬
pub const MONSTER_CHANCE:u64=4;
pub fn reward(run:&crate::Run,node:usize)->Option<Ember> {
    reward_in(run,node,false)
}
pub(crate) fn before_stride_removal(run:&crate::Run,node:usize)->Option<Ember> {
    reward_in(run,node,true)
}
fn reward_in(run:&crate::Run,node:usize,legacy:bool)->Option<Ember> {
    if !run.ember_rewards {return None;}
    let roll=crate::mix(run.reward_seed()^(node as u64).wrapping_mul(7919)^0xe3be_12a5);
    match run.nodes.get(node)?.kind {
        crate::NodeKind::Elite=>{},
        crate::NodeKind::Monster if roll%MONSTER_CHANCE==0=>{},
        _=>return None,
    }
    // 规则 0（旧存档）不分品质，等概率
    if run.ember_rules>=1 {offer_quality_in(roll>>8,&run.embers,carries_spirit(run),carries_catalyst(run),class_pool(run),odds(crate::affixes::quality_odds(run)),legacy)}
    else {offer_in(roll>>8,&run.embers,carries_spirit(run),carries_catalyst(run),None,legacy)}
}
/// 宝箱房（余烬规则 2 起）：打开宝箱直接得到一枚随机余烬（按品质概率，没拿过的，召唤流、法师余烬照常看构筑）；
/// 能抽的都拿完了返回 None，宝箱照旧给物品
pub fn chest_ember(run:&crate::Run,node:usize)->Option<Ember> {
    if !run.ember_rewards||run.ember_rules<2||run.nodes.get(node).is_none_or(|n|n.kind!=crate::NodeKind::Chest) {return None;}
    let seed=crate::mix(run.reward_seed()^(node as u64).wrapping_mul(7919)^0xc4e5_e3be);
    offer_by_quality(seed,&run.embers,carries_spirit(run),carries_catalyst(run),class_pool(run),odds(crate::affixes::quality_odds(run)))
}
/// 余烬规则 3 起：只抽通用余烬与本职业的专属余烬
fn class_pool(run:&crate::Run)->Option<crate::Class> {(run.ember_rules>=3).then_some(run.class)}
/// 身上带着骨灰：召唤师（开局就有骨灰），或拿到过任何骨灰奖励
pub fn carries_spirit(run:&crate::Run)->bool {
    run.summoner()||run.claimed_items.iter().any(|id|crate::rewards::lookup(*id).is_some_and(|i|i.kind==crate::rewards::ItemKind::Spirit))
}
/// 身上带着法杖或圣印记：星辉术士、祷誓者（开局就有），或上一场胜利时背包里有法杖、圣印记
pub fn carries_catalyst(run:&crate::Run)->bool {
    matches!(run.class,crate::Class::Astrologer|crate::Class::Prophet)||run.casting_tools.is_some_and(|tools|tools[0]||tools[1])
}
/// 借用原版护符的图标
pub fn icon_talisman(ember:Ember)->u32 {match ember {
    Ember::FrostShackle=>1140,
    Ember::Execution=>5050,Ember::Bloodlord=>2160,Ember::Frost=>1140,Ember::Deathflame=>1190,Ember::Miquella=>1250,Ember::Rise=>3090,
    Ember::DragonHeart=>4020,Ember::Revenge=>2060,Ember::Frenzy=>2080,Ember::Echo=>5000,Ember::Feast=>5030,Ember::Thorns=>1210,
    Ember::SpiritLink=>6090,Ember::SpiritBurst=>6020,Ember::SpiritFeast=>2210,Ember::SpiritCommand=>3060,Ember::SpiritTwin=>6090,
    Ember::SoulEater=>2050,Ember::Chain=>2090,Ember::Godslayer=>1001,Ember::SpiritKeeper=>2210,
    Ember::Barrier=>3000,Ember::Meditation=>6020,Ember::Glintstone=>1010,Ember::FullMoon=>2110,Ember::Footwork=>5050,Ember::SpiritResonance=>3060,Ember::Pouch=>5000,Ember::Arts=>6020,Ember::Bulwark=>4100,Ember::Breaker=>2160,Ember::Harvest=>2090,Ember::LastStand=>1210,Ember::Martyr=>2010,Ember::Mercy=>2190,Ember::Battlemage=>1010,Ember::Stride=>1170,
}}
/// 扣敌人生命的量：按目标最大生命的百分比，Boss 另有上限
pub fn enemy_damage(max_hp:i32,percent:u32,boss:bool)->i32 {
    let percent=if boss {percent.min(BOSS_CAP_PERCENT)}else{percent};
    (i64::from(max_hp.max(0))*i64::from(percent)/100) as i32
}
/// 荆棘：按受到的伤害反弹，对 Boss 不超过其最大生命的 1%
pub fn thorns_damage(taken:i32,target_max:i32,boss:bool)->i32 {
    let damage=(i64::from(taken.max(0))*i64::from(THORNS_PERCENT)/100) as i32;
    if boss {damage.min(enemy_damage(target_max,BOSS_CAP_PERCENT,true))}else{damage}
}

#[cfg(test)]
mod tests {
    #[test]
    fn frost_shackle_keeps_existing_ids_and_is_universal_epic() {
        use super::*;
        assert_eq!(Ember::Battlemage.index(),35);assert_eq!(Ember::Stride.index(),36);
        assert_eq!(Ember::FrostShackle.index(),37);assert_eq!(Ember::FrostShackle.icon_index(),Ember::Frost.index());
        assert_eq!(Ember::FrostShackle.quality(),2);assert!(Ember::FrostShackle.classes().is_none());
        assert_eq!(Ember::ALL.iter().map(|e|e.index()).collect::<std::collections::HashSet<_>>().len(),Ember::ALL.len());
        let held:Vec<_>=Ember::ALL.into_iter().filter(|e|*e!=Ember::FrostShackle).collect();
        assert_eq!(offer(1,&held,false,false,None),Some(Ember::FrostShackle));
    }
    #[test]
    fn twin_is_temporarily_retired_without_renumbering_other_embers() {
        use super::*;
        assert_eq!(Ember::SpiritTwin.index(),16);assert_eq!(Ember::SoulEater.index(),17);
        assert!(!pool(&[],true,true,Some(crate::Class::Confessor),false).contains(&Ember::SpiritTwin));
        let mut run=crate::Run::new(1,crate::Class::Confessor);run.embers.push(Ember::SpiritTwin);
        assert!(!crate::Run::decode(&run.encode().unwrap()).unwrap().embers.contains(&Ember::SpiritTwin));
    }
    use super::*;
    #[test]
    fn retired_stride_is_not_offered_and_old_saves_still_load() {
        assert!(!Ember::ALL.contains(&Ember::Stride));
        let mut run=crate::Run::new(3,crate::Class::Astrologer);
        run.embers.push(Ember::Stride);
        let decoded=crate::Run::decode(&run.encode().unwrap()).unwrap();
        assert!(!decoded.embers.contains(&Ember::Stride));
        assert_eq!(decoded.class,run.class);
        let mut saw_retired=false;
        for seed in 0..100 {
            let mut run=crate::Run::new(seed,crate::Class::Astrologer);
            let Some(id)=run.nodes.iter().position(|node|node.kind==crate::NodeKind::Elite) else{continue};
            run.debug_jump(id).unwrap();run.enter(id).unwrap();run.win_combat().unwrap();
            let previous=before_stride_removal(&run,id).unwrap();saw_retired|=previous.retired();
            if let crate::Phase::Reward{offers,..}=&mut run.phase {*offers.last_mut().unwrap()=crate::Reward::Ember(previous);}
            let decoded=crate::Run::decode(&run.encode().unwrap()).unwrap();
            let crate::Phase::Reward{ref offers,..}=decoded.phase else{panic!("expected rewards");};
            assert_eq!(*offers,decoded.offers_for_test(id));
            assert!(!offers.iter().any(|r|matches!(r,crate::Reward::Ember(e) if e.retired())));
        }
        assert!(saw_retired);
    }
    #[test]
    fn every_quality_has_embers_and_rolls_follow_the_odds() {
        assert!(Ember::ALL.iter().all(|e|matches!(e.quality(),2|3)),"embers are only epic or legendary");
        assert_eq!(Ember::ALL.iter().filter(|e|e.quality()==3).count(),10);
        assert_eq!(odds(crate::affixes::QUALITY_START),[0,0,85,15]);assert_eq!(odds(crate::affixes::QUALITY_END),[0,0,72,28]);
        let mut counts=[0u32;4];
        for seed in 0..20000u64 {counts[usize::from(offer_by_quality(seed,&[],true,true,None,[0,0,50,50]).unwrap().quality())]+=1;}
        assert!(counts[0]==0&&counts[1]==0&&(9000..11000).contains(&counts[2])&&(9000..11000).contains(&counts[3]),"{counts:?}");
        let mut late=[0u32;4];
        for seed in 0..20000u64 {late[usize::from(offer_by_quality(seed,&[],true,true,None,[0,0,0,100]).unwrap().quality())]+=1;}
        assert_eq!(late[3],20000,"legendary odds give legendary embers while any are left");
        // 这一档拿完了：先往低一档找
        let held:Vec<_>=Ember::ALL.into_iter().filter(|e|e.quality()==3).collect();
        assert!((0..500).all(|seed|offer_by_quality(seed,&held,true,true,None,[0,0,0,100]).unwrap().quality()==2));
        let mut all=vec![];while let Some(e)=offer_by_quality(all.len() as u64*13,&all,true,true,None,[0,0,85,15]) {all.push(e);}
        assert_eq!(all.len(),Ember::ALL.iter().filter(|e|!e.retired()).count());
    }
    #[test]
    fn battlemage_cuts_damage_by_the_higher_mind_stat_and_records_weapons() {
        assert_eq!(Ember::Battlemage.effect(),"可在铁匠处免费让近战武器\n获得智力或信仰 S 补正\n受到的伤害降低\n智力、信仰取高 ×0.4%");
        assert!((battlemage_cut(40)-0.84).abs()<1e-6&&(battlemage_cut(80)-0.68).abs()<1e-6&&(battlemage_cut(0)-1.0).abs()<1e-6&&(battlemage_cut(500)-0.05).abs()<1e-6);
        let mut run=crate::Run::new(3,crate::Class::Astrologer);
        assert!(run.battlemage_convert(2000000,0).is_err(),"needs the ember");
        run.embers.push(Ember::Battlemage);
        run.battlemage_convert(2000000,0).unwrap();run.battlemage_convert(2000000,1).unwrap();assert_eq!(run.battlemage,vec![(2000000,1)],"switches the stat");
        assert!(run.battlemage_convert(3000000,2).is_err());
        for i in 1..BATTLEMAGE_WEAPONS as u32 {run.battlemage_convert(3000000+i*100,0).unwrap();}
        assert!(run.battlemage_convert(9000000,0).is_err(),"at most eight weapons");
        assert_eq!(crate::Run::decode(&run.encode().unwrap()).unwrap().battlemage,run.battlemage);
        assert!(Ember::Battlemage.quality()==3&&Ember::Battlemage.classes()==Some(&[crate::Class::Astrologer,crate::Class::Prophet][..]));
    }
    #[test]
    fn mercy_boosts_healing_incantations_for_clerics() {
        assert_eq!(Ember::Mercy.effect(),"治疗类祷告的回复量\n提高 50%");
        assert!(Ember::Mercy.quality()==2&&Ember::Mercy.classes()==Some(&[crate::Class::Prophet,crate::Class::Confessor][..]));
    }
    #[test]
    fn martyr_restores_fp_in_proportion_to_the_health_lost() {
        assert_eq!(Ember::Martyr.effect(),"受到伤害时回复专注\n失去最大生命的 1%\n回复最大专注的 2%");
        assert_eq!(martyr_fp(200,1000,150),60,"20% of health lost restores 40% of focus");
        assert_eq!(martyr_fp(5000,1000,150),150,"never more than the whole pool");
        assert_eq!((martyr_fp(0,1000,150),martyr_fp(100,0,150),martyr_fp(100,1000,0)),(0,0,0));
        assert!(Ember::Martyr.quality()==2&&Ember::Martyr.classes()==Some(&[crate::Class::Astrologer,crate::Class::Prophet][..]));
    }
    #[test]
    fn last_stand_is_a_berserker_ember_with_the_tuned_text() {
        assert_eq!(Ember::LastStand.effect(),"生命低于 40% 时\n韧性（强韧度）提高 200%");
        assert!(Ember::LastStand.quality()==2&&Ember::LastStand.classes()==Some(&[crate::Class::Hero][..])&&!Ember::LastStand.stacking());
    }
    #[test]
    fn harvest_compounds_on_execution_kills_by_enemy_rank() {
        let held=[Ember::Harvest];let mut stacks=Stacks::default();
        assert!(stacks.gain(&held,Gain::Kill{rank:2,spirit:false}).is_empty(),"ordinary kills do not count");
        assert!(stacks.gain(&held,Gain::Critical).is_empty());
        assert_eq!(stacks.gain(&held,Gain::CriticalKill{rank:0}),vec![Ember::Harvest]);assert_eq!(stacks.harvest,2);
        stacks.gain(&held,Gain::CriticalKill{rank:1});stacks.gain(&held,Gain::CriticalKill{rank:2});assert_eq!(stacks.harvest,17);
        let hp=stacks.rates(Ember::Harvest).unwrap().hp;assert!((hp-1.02*1.05*1.10).abs()<1e-4,"compounds: {hp}");
        assert!(stack_rates(Ember::Harvest,100).unwrap().hp>2.7,"1.01^100 is about 2.7x, not 2x");
        assert!(stack_rates(Ember::Harvest,u32::MAX).unwrap().hp.is_finite());
        assert_eq!(Ember::Harvest.stack_bonus(9).unwrap(),"最大生命 +9%");
        let restored:Stacks=serde_json::from_str(&serde_json::to_string(&stacks).unwrap()).unwrap();
        assert_eq!(restored,stacks);assert_eq!(restored.rates(Ember::Harvest),stacks.rates(Ember::Harvest));
        let mut legacy:Stacks=serde_json::from_str(r#"{"harvest":9}"#).unwrap();
        let old_hp=legacy.rates(Ember::Harvest).unwrap().hp;
        assert!((old_hp-1.01f32.powi(9)).abs()<1e-4);
        legacy.gain(&held,Gain::CriticalKill{rank:2});
        assert!((legacy.rates(Ember::Harvest).unwrap().hp-old_hp*1.10).abs()<1e-4);
        let mut other=Stacks::default();assert!(other.gain(&[Ember::Chain],Gain::CriticalKill{rank:2}).is_empty(),"needs the ember");
        assert!(Ember::Harvest.stacking()&&Ember::Harvest.quality()==3&&Ember::Harvest.classes().is_none());
        assert_eq!(serde_json::to_string(&Stacks::default()).unwrap().contains("harvest"),false,"old saves stay unchanged");
    }
    #[test]
    fn class_embers_only_reach_their_own_classes() {
        use crate::Class;
        let general=Ember::ALL.iter().filter(|e|e.classes().is_none()).count();
        assert_eq!(general,14);
        for class in Class::PLAYABLE {
            let mut drawn=vec![];
            while let Some(e)=offer(drawn.len() as u64*29,&drawn,true,true,Some(class)) {
                assert!(e.classes().is_none_or(|c|c.contains(&class)),"{class:?} drew {e:?}");drawn.push(e);
            }
            let own=Ember::ALL.iter().filter(|e|!e.retired()&&e.classes().is_some_and(|c|c.contains(&class))).count();
            assert_eq!(drawn.len(),general+own,"{class:?}");assert!(own>=2);
        }
        assert!(Ember::ALL.iter().all(|e|!e.needs_spirit()||e.classes()==Some(&[Class::Confessor][..])));
        assert!(Ember::ALL.iter().all(|e|!e.needs_casting()||e.classes()==Some(&[Class::Astrologer,Class::Prophet][..])));
        // 新旅程按职业限制，旧存档（余烬规则 2）不限
        let mut run=crate::Run::new(7,Class::Hero);assert_eq!(class_pool(&run),Some(Class::Hero));
        run.ember_rules=2;assert_eq!(class_pool(&run),None);
    }
    #[test]
    fn offers_skip_held_embers_and_run_out_cleanly() {
        let mut held=vec![];
        for _ in 0..Ember::ALL.iter().filter(|e|!e.retired()).count() {let e=offer(held.len() as u64*31,&held,true,true,None).unwrap();assert!(!held.contains(&e));held.push(e);}
        assert_eq!(offer(1,&held,true,true,None),None);
        assert_eq!(offer(5,&[],true,true,None),offer(5,&[],true,true,None));
        // 没有骨灰、也没有法杖圣印记时只出 25 枚（不分职业时），抽完就没有了
        let mut plain=vec![];
        while let Some(e)=offer(plain.len() as u64*17,&plain,false,false,None) {assert!(!e.needs_spirit()&&!e.needs_casting());plain.push(e);}
        assert_eq!(plain.len(),26);
    }
    #[test]
    fn enemy_damage_is_capped_for_bosses() {
        assert_eq!(enemy_damage(10000,DEATHFLAME_PERCENT,false),800);
        assert_eq!(enemy_damage(10000,DEATHFLAME_PERCENT,true),100);
        assert_eq!(thorns_damage(400,10000,false),160);assert_eq!(thorns_damage(4000,10000,true),100);
        assert_eq!(enemy_damage(-5,DEATHFLAME_PERCENT,false),0);
    }
    #[test]
    fn member_strikes_need_the_matching_ember() {
        let held=[Ember::Bloodlord,Ember::Deathflame,Ember::Thorns];
        assert!(Strike::Buildup{slot:0,ember:Ember::Bloodlord}.valid(&held));
        assert!(!Strike::Buildup{slot:0,ember:Ember::Frost}.valid(&held),"frost not held");
        assert!(!Strike::Buildup{slot:0,ember:Ember::Deathflame}.valid(&held),"not a buildup ember");
        assert!(Strike::Wound{slot:1,ember:Ember::Deathflame}.valid(&held));
        assert!(!Strike::Wound{slot:1,ember:Ember::Bloodlord}.valid(&held),"bloodlord does not wound");
        assert!(!Strike::Wound{slot:1,ember:Ember::Echo}.valid(&held));
        assert!(Strike::Thorns{slot:2,taken:300}.valid(&held));
        assert!(!Strike::Thorns{slot:2,taken:0}.valid(&held)&&!Strike::Thorns{slot:2,taken:THORNS_TAKEN_LIMIT+1}.valid(&held));
        assert!(Ember::ALL.into_iter().filter(|e|e.affects_enemies()).all(|e|wound_percent(e).is_some()||matches!(e,Ember::Bloodlord|Ember::Frost|Ember::Thorns)),"every enemy-side ember can be requested");
    }
    #[test]
    fn embers_show_up_in_combat_rewards_and_persist() {
        use crate::{Class,NodeKind,Phase,Reward,Run};
        let (mut elites,mut monsters,mut monster_embers)=(0,0,0);
        for seed in 0..400 {
            let mut run=Run::new(seed,Class::Samurai);
            for (id,node) in run.nodes.clone().iter().enumerate() {
                let ember=reward(&run,id);
                match node.kind {
                    NodeKind::Elite=>{elites+=1;assert!(ember.is_some());},
                    NodeKind::Monster=>{monsters+=1;if ember.is_some(){monster_embers+=1;}},
                    _=>assert!(ember.is_none()),
                }
            }
            let id=run.available()[0];run.enter(id).unwrap();run.win_combat().unwrap();
            let Phase::Reward{offers,..}=run.phase.clone() else{panic!()};
            assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
            if let Some(index)=offers.iter().position(|r|matches!(r,Reward::Ember(_))) {
                let Reward::Ember(ember)=offers[index] else{unreachable!()};
                assert_eq!(index,offers.len()-1);
                run.claim_reward(index).unwrap();assert_eq!(run.embers,vec![ember]);
                assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
                for later in 0..run.nodes.len() {assert_ne!(reward(&run,later),Some(ember),"held embers never repeat");}
                let mut twice=run.clone();twice.embers.push(ember);assert!(twice.validate().is_err());
            }
        }
        assert!(elites>0);
        let rate=monster_embers as f64/monsters as f64;assert!((0.18..0.32).contains(&rate),"monster ember rate {rate}");
        let mut old=Run::new(3,Class::Samurai);old.ember_rewards=false;
        assert!((0..old.nodes.len()).all(|id|reward(&old,id).is_none()));
        let mut json:serde_json::Value=serde_json::from_str(&Run::new(3,Class::Samurai).encode().unwrap()).unwrap();
        json.as_object_mut().unwrap().remove("ember_rewards");
        assert!(!Run::decode(&json.to_string()).unwrap().ember_rewards,"saves from before embers keep their reward pages");
    }
    #[test]
    fn summon_embers_need_a_spirit_ash() {
        use crate::{Class,Run};
        let (mut plain,mut summoner)=(0,0);
        for seed in 0..300 {
            let mut run=Run::new(seed,Class::Samurai);
            for id in 0..run.nodes.len() {if reward(&run,id).is_some_and(Ember::needs_spirit) {plain+=1;}}
            run.claimed_items.push(0x40000000|232000);// 离群野狼的骨灰
            assert!(carries_spirit(&run));
            // 新旅程：召唤流余烬只有召唤师抽得到，影刃捡到骨灰也没有
            for id in 0..run.nodes.len() {if reward(&run,id).is_some_and(Ember::needs_spirit) {plain+=1;}}
            let mut old=run.clone();old.ember_rules=2;
            for id in 0..old.nodes.len() {if reward(&old,id).is_some_and(Ember::needs_spirit) {summoner+=1;}}
            let confessor=Run::new(seed,Class::Confessor);
            for id in 0..confessor.nodes.len() {if reward(&confessor,id).is_some_and(Ember::needs_spirit) {summoner+=1;}}
        }
        assert_eq!(plain,0,"no summon embers without a spirit ash, nor for other classes in new runs");
        assert!(summoner>0);
    }
    #[test]
    fn chest_rooms_grant_one_random_ember() {
        use crate::{Class,NodeKind,Phase,Run};
        let mut seen=std::collections::HashSet::new();let mut checked=0;
        for seed in 0..80 {
            let run=Run::new(seed,Class::Samurai);
            for id in (0..run.nodes.len()).filter(|id|run.nodes[*id].kind==NodeKind::Chest) {
                let ember=chest_ember(&run,id).unwrap();
                assert!(!ember.needs_spirit()&&!ember.needs_casting(),"gates still apply");
                assert_eq!(chest_ember(&run,id),Some(ember),"same chest, same ember");
                seen.insert(ember);checked+=1;
            }
            assert!(run.nodes.iter().enumerate().filter(|(_,n)|n.kind!=NodeKind::Chest).all(|(id,_)|chest_ember(&run,id).is_none()));
        }
        assert!(checked>20&&seen.len()>8,"random across chests");
        // 领取：进背包、节点结束；拿完了就没有，宝箱照旧给物品；旧存档（余烬规则 1）不变
        // 照正常路线走到第一个宝箱房
        let (mut run,chest)=(0..60).find_map(|seed|{
            let mut run=Run::new(seed,Class::Samurai);
            for _ in 0..run.floors()-1 {
                let available=run.available();
                let id=available.iter().copied().find(|id|run.nodes[*id].kind==NodeKind::Chest).unwrap_or(available[0]);
                run.enter(id).unwrap();
                match run.nodes[id].kind {
                    NodeKind::Chest=>return Some((run,id)),
                    NodeKind::Monster|NodeKind::Elite=>{run.win_combat().unwrap();run.decline_reward().unwrap();},
                    NodeKind::Camp=>run.leave_camp().unwrap(),NodeKind::Shop=>run.leave_shop().unwrap(),
                    NodeKind::Event=>run.resolve_event(false).unwrap(),_=>return None,
                }
            }
            None
        }).unwrap();
        assert_eq!(run.phase,Phase::Node{id:chest});
        let expected=chest_ember(&run,chest).unwrap();
        let mut opened=run.clone();assert_eq!(opened.claim_chest_ember().unwrap(),Some(expected));
        assert_eq!(opened.embers,vec![expected]);assert!(!matches!(opened.phase,Phase::Node{..}),"the chest node is finished");
        assert_eq!(Run::decode(&opened.encode().unwrap()).unwrap(),opened);
        run.embers.clear();let mut full=run.clone();full.embers=Ember::ALL.into_iter().filter(|e|e.fits(false,false,class_pool(&run))).collect();
        assert_eq!(full.claim_chest_ember().unwrap(),None);assert_eq!(full.phase,Phase::Node{id:chest},"nothing changes without an ember");
        let mut old=run.clone();old.ember_rules=1;assert_eq!(old.claim_chest_ember().unwrap(),None);
    }
    #[test]
    fn mage_embers_need_a_staff_or_seal() {
        use crate::{Class,Run};
        let (mut warrior,mut picked_up,mut casters)=(0,0,0);
        for seed in 0..300 {
            let mut run=Run::new(seed,Class::Samurai);
            assert!(!carries_catalyst(&run));
            for id in 0..run.nodes.len() {if reward(&run,id).is_some_and(Ember::needs_casting) {warrior+=1;}}
            run.casting_tools=Some([false,true]);
            assert!(carries_catalyst(&run));
            // 新旅程：法师余烬只有星辉术士、祷誓者抽得到；旧规则（余烬规则 2）捡到法杖的影刃照旧能抽到
            for id in 0..run.nodes.len() {if reward(&run,id).is_some_and(Ember::needs_casting) {warrior+=1;}}
            run.ember_rules=2;
            for id in 0..run.nodes.len() {if reward(&run,id).is_some_and(Ember::needs_casting) {picked_up+=1;}}
            for class in [Class::Astrologer,Class::Prophet] {
                let run=Run::new(seed,class);
                for id in 0..run.nodes.len() {if reward(&run,id).is_some_and(Ember::needs_casting) {casters+=1;}}
            }
        }
        assert_eq!(warrior,0,"no mage embers without a catalyst");
        assert!(picked_up>0&&casters>0);
        let mut plain=vec![];
        while let Some(e)=offer(plain.len() as u64*17,&plain,true,false,None) {assert!(!e.needs_casting());plain.push(e);}
        assert_eq!(plain.len(),Ember::ALL.iter().filter(|e|!e.retired()&&!e.needs_casting()).count());
        assert!(Ember::ALL.into_iter().filter(|e|e.needs_casting()).all(|e|e.quality()==if e==Ember::Meditation {3}else{2}&&!e.needs_spirit()&&!e.affects_enemies()));
    }
    #[test]
    fn chain_room_limit_survives_reload_and_preserves_total() {
        let held=[Ember::Chain];
        let mut stacks=Stacks::default();
        for _ in 0..9 {stacks.gain_in_room(&held,Gain::Critical,(42,0,3));}
        assert_eq!(stacks.chain,3);
        let mut stacks:Stacks=serde_json::from_str(&serde_json::to_string(&stacks).unwrap()).unwrap();
        assert!(stacks.gain_in_room(&held,Gain::Critical,(42,0,3)).is_empty());
        for _ in 0..8 {stacks.gain_in_room(&held,Gain::Critical,(42,0,4));}
        assert_eq!(stacks.chain,6);
        stacks.gain_in_room(&held,Gain::Critical,(42,1,4));
        assert_eq!(stacks.chain,7);
        let mut old:Stacks=serde_json::from_str(r#"{"chain":50}"#).unwrap();
        for _ in 0..8 {old.gain_in_room(&held,Gain::Critical,(42,0,3));}
        assert_eq!(old.chain,53);
        let mut empty=Stacks::default();
        for _ in 0..8 {empty.gain_in_room(&[],Gain::Critical,(42,0,3));}
        assert_eq!(empty.gain_in_room(&held,Gain::Critical,(42,0,3)),vec![Ember::Chain]);
    }
    #[test]
    fn stacks_grow_only_from_their_own_events_without_a_cap() {
        let mut stacks=Stacks::default();
        assert!(stacks.gain(&[Ember::Frenzy],Gain::Critical).is_empty());assert!(stacks.is_empty(),"no stacking ember held");
        let held=[Ember::SoulEater,Ember::Chain,Ember::Godslayer,Ember::SpiritKeeper];
        for _ in 0..4 {assert!(stacks.gain(&held,Gain::Kill{rank:0,spirit:false}).is_empty());}
        assert_eq!(stacks.gain(&held,Gain::Kill{rank:0,spirit:false}),vec![Ember::SoulEater]);
        assert_eq!(stacks.gain(&held,Gain::Kill{rank:2,spirit:true}),vec![Ember::SoulEater,Ember::SpiritKeeper]);
        assert_eq!((stacks.level(Ember::SoulEater),stacks.level(Ember::SpiritKeeper)),(3,1));
        assert_eq!(stacks.gain(&held,Gain::Kill{rank:1,spirit:false}),vec![],"3 souls: 18 in total");
        assert_eq!(stacks.gain(&held,Gain::Critical),vec![Ember::Chain]);
        assert_eq!(stacks.gain(&held,Gain::Slain),vec![Ember::Godslayer]);
        for _ in 0..100 {for event in [Gain::Kill{rank:2,spirit:true},Gain::Critical,Gain::Slain] {stacks.gain(&held,event);}}
        assert_eq!(held.map(|e|stacks.level(e)),[203,101,101,101],"no cap");
        assert_eq!(stacks.gain(&held,Gain::Critical),vec![Ember::Chain]);
        assert!(Ember::ALL.iter().all(|e|e.stacking()==e.stack_bonus(1).is_some()));
        assert_eq!(Ember::Godslayer.stack_bonus(8).unwrap(),"最大生命 +40%、攻击 +32%");
        assert_eq!(Ember::Chain.stack_bonus(50).unwrap(),"致命一击伤害 +250%");
        assert_eq!(Ember::SpiritKeeper.stack_bonus(10).unwrap(),"召唤物伤害 +30%、受到的伤害 -18%");
        let deep=stack_rates(Ember::SpiritKeeper,500).unwrap();assert!(deep.cut>0.0&&deep.cut<0.001,"damage cut never reaches zero");
        assert_eq!(stack_rates(Ember::SoulEater,0).unwrap(),Rates{attack:1.0,hp:1.0,cut:1.0});
        let mut full=Stacks{chain:u32::MAX,..Default::default()};assert!(full.gain(&held,Gain::Critical).is_empty(),"saturates instead of overflowing");
    }
    #[test]
    fn buffed_ember_values_and_growth_match_the_requested_balance() {
        assert_eq!((SOUL_POINTS,SOUL_STEP_PERCENT),(5,3));
        assert_eq!((SLAYER_HP_PERCENT,SLAYER_ATTACK_PERCENT),(5,4));
        assert_eq!(EXECUTION_SECONDS,10);
        assert_eq!((CHAIN_STEP_PERCENT,CHAIN_ROOM_LIMIT),(5,3));
        assert_eq!(HARVEST_POINTS,[2,5,10]);
        assert_eq!(u32::from(FRENZY_STACKS)*FRENZY_STEP_PERCENT,80);
        assert_eq!(BUILDUP,40);
        assert_eq!((BARRIER_CUT_PERCENT,MOON_ATTACK_PERCENT),(50,50));
        assert_eq!((SPIRIT_BURST_PERCENT,SPIRIT_FEAST_HEAL_PERCENT,SPIRIT_FEAST_FP_PERCENT),(20,20,20));
        assert_eq!(wound_percent(Ember::SpiritBurst),Some(20));
        assert_eq!(BOSS_CAP_PERCENT,1,"the existing boss cap is unchanged");
        let mut stacks=Stacks::default();
        for _ in 0..5 {stacks.gain(&[Ember::SoulEater],Gain::Kill{rank:0,spirit:false});}
        assert_eq!(stacks.level(Ember::SoulEater),1);
        assert!((stacks.rates(Ember::SoulEater).unwrap().attack-1.03).abs()<1e-6);
        for (rank,expected) in [(0,1.02),(1,1.05),(2,1.10)] {
            let mut stacks=Stacks::default();stacks.gain(&[Ember::Harvest],Gain::CriticalKill{rank});
            assert!((stacks.rates(Ember::Harvest).unwrap().hp-expected).abs()<1e-6);
        }
    }
    #[test]
    fn card_text_uses_the_tuned_numbers() {
        assert_eq!(Ember::Execution.effect(),"致命一击后 10 秒内\n精力消耗 -50%");
        assert_eq!(Ember::Barrier.effect(),"施放法术或祷告时\n受到的伤害降低 50%");
        assert_eq!(Ember::FullMoon.effect(),"专注不低于 70% 时\n造成的伤害提高 50%");
        assert_eq!(Ember::SpiritResonance.effect(),"召唤物在场时\n你造成的伤害 +40%");
        assert_eq!(Ember::Arts.effect(),"战技伤害 +25%\n战技消耗的专注 -50%");
        assert!(Ember::Arts.quality()==2&&!Ember::Arts.needs_spirit()&&!Ember::Arts.needs_casting());
        assert!(Ember::Pouch.quality()==2&&!Ember::Pouch.needs_spirit()&&!Ember::Pouch.needs_casting()&&!Ember::Pouch.affects_enemies());
        assert_eq!(Ember::Bulwark.effect(),"使用大盾格挡时不消耗精力\n不影响负重、翻滚与移动速度");
        assert!(Ember::Bulwark.quality()==2&&!Ember::Bulwark.needs_spirit()&&!Ember::Bulwark.needs_casting());
        assert_eq!(Ember::Breaker.effect(),"敌人出血、冻伤、睡眠、发狂发作时\n压低架势，5 秒内下一次削韧命中触发失衡\nBoss 与普通敌人均无触发冷却");
        assert!(Ember::Breaker.quality()==3&&!Ember::Breaker.needs_spirit()&&!Ember::Breaker.needs_casting());
        assert!(Ember::SpiritResonance.needs_spirit()&&Ember::SpiritResonance.quality()==3&&!Ember::SpiritResonance.affects_enemies());
        assert_eq!(Ember::Frenzy.effect(),"每次命中攻击 +2%，最多 +80%\n受伤时清零");
        assert!(Ember::ALL.iter().all(|e|e.effect().lines().all(|l|l.chars().count()<=20)),"card lines must fit");
    }
}
