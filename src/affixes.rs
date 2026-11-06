use std::{collections::HashMap,sync::LazyLock};
use crate::{Run,Reward,rewards::{Item,ItemKind}};

struct Variant {item:Item,base:u32,tier:u8,profile:Option<u8>}
static VARIANTS:LazyLock<Vec<Variant>>=LazyLock::new(||include_str!("../scene-assets/affix-catalog.txt").lines().chain(include_str!("../scene-assets/affix-v2-catalog.txt").lines()).map(|line| {
    let p:Vec<_>=line.split('|').collect();let tag=if p[0]=="weapon"{0}else{0x10000000};
    Variant{item:Item{id:tag|p[1].parse::<u32>().unwrap(),kind:if tag==0{ItemKind::Weapon}else{ItemKind::Armor},name:p[4]},base:tag|p[2].parse::<u32>().unwrap(),tier:p[3].parse().unwrap(),profile:p.get(5).map(|v|v.parse().unwrap())}
}).collect());
pub static CATALOG:LazyLock<Vec<Item>>=LazyLock::new(||VARIANTS.iter().map(|v|Item{id:v.item.id,kind:v.item.kind,name:v.item.name}).collect());
static BY_ID:LazyLock<HashMap<u32,usize>>=LazyLock::new(||VARIANTS.iter().enumerate().map(|(i,v)|(v.item.id,i)).collect());
static BY_BASE:LazyLock<HashMap<u32,[u32;4]>>=LazyLock::new(|| {
    let mut map=HashMap::new();for v in VARIANTS.iter().filter(|v|v.profile.is_none()){map.entry(v.base).or_insert([0;4])[v.tier as usize]=v.item.id;}map
});
static ADVANCED:LazyLock<HashMap<u32,Vec<[u32;4]>>>=LazyLock::new(|| {
    let mut map=HashMap::new();for v in VARIANTS.iter(){if let Some(profile)=v.profile {
        let groups=map.entry(v.base).or_insert_with(Vec::new);groups.resize(groups.len().max(profile as usize+1),[0;4]);groups[profile as usize][v.tier as usize]=v.item.id;
    }}map
});
static BASE_RARITY:LazyLock<HashMap<u32,u8>>=LazyLock::new(||include_str!("../scene-assets/reward-rarities.tsv").lines().map(|l|{let (id,tier)=l.split_once('\t').unwrap();(id.parse().unwrap(),tier.parse().unwrap())}).collect());
pub fn lookup(id:u32)->Option<&'static Item>{BY_ID.get(&id).map(|i|&CATALOG[*i])}
pub fn base(id:u32)->u32 {BY_ID.get(&id).map_or(id,|i|VARIANTS[*i].base)}
pub fn tier(id:u32)->Option<u8>{BY_ID.get(&id).map(|i|VARIANTS[*i].tier)}
pub fn profile(id:u32)->Option<u8>{BY_ID.get(&id).and_then(|i|VARIANTS[*i].profile)}
/// 规则 4 的前缀概率（0..100 的掷骰值）：普通 25%、稀有 35%、史诗 28%、传说 12%
pub fn prefix_tier(value:u64)->u8 {match value {0..=24=>0,25..=59=>1,60..=87=>2,_=>3}}
/// 规则 5 起品质随旅程进度变化：第一幕第 1 层为 QUALITY_START，第三幕最后一层为 QUALITY_END，中间按层线性过渡（普通、稀有、史诗、传说，合计 100）
pub const QUALITY_START:[u32;4]=[45,35,17,3];
pub const QUALITY_END:[u32;4]=[20,34,33,13];
/// 当前所在层的品质概率（百分比，合计 100）
pub fn quality_odds(run:&Run)->[u32;4] {
    let (step,total)=run.journey_progress();
    // 对累计概率插值再取整，各档逐层单调变化
    let cumulative=|table:&[u32;4],tier:usize|table[..=tier].iter().sum::<u32>();
    let mut odds=[0;4];let mut below=0;
    for tier in 0..3 {
        let at=(cumulative(&QUALITY_START,tier)*(total-step)+cumulative(&QUALITY_END,tier)*step+total/2)/total;
        odds[tier]=at-below;below=at;
    }
    odds[3]=100-below;
    odds
}
fn layered_tier(run:&Run,value:u64)->u8 {
    let mut roll=value as u32;
    for (tier,odds) in quality_odds(run).into_iter().enumerate() {if roll<odds {return tier as u8;} roll-=odds;}
    3
}
pub fn roll(run:&Run,id:u32,entropy:u64,repeat:bool)->Option<u32> {roll_min(run,id,entropy,repeat,0)}
/// 规则 3 起品质只看前缀，按来源决定保底；Boss 新规则仅第一张保底传说。
/// 旧规则（affix_rules < 3）仍以原版稀有度为最低品质，忽略 min
pub fn roll_min(run:&Run,id:u32,entropy:u64,repeat:bool,min:u8)->Option<u32> {
    if id>>28==2{return Some(crate::talismans::roll(run,id,entropy,repeat));}
    if !run.affix_equipment{return Some(id);}
    let groups=if run.affix_rules>=2 {ADVANCED.get(&id).map(Vec::as_slice)}else{BY_BASE.get(&id).map(std::slice::from_ref)};
    let Some(groups)=groups else{return Some(id)};
    let start=if run.affix_rules>=2{crate::mix(run.reward_seed()^entropy^u64::from(id)^0xa4c4_7e99) as usize%groups.len()}else{0};
    let floor=if run.affix_rules>=3 {min.min(3)}else{BASE_RARITY.get(&id).copied().unwrap_or(0).min(3)};
    let value=crate::mix(run.reward_seed()^entropy^u64::from(id)^0xaff1_600d)%100;
    let selected=(if run.affix_rules>=5 {layered_tier(run,value)}else if run.affix_rules>=4 {prefix_tier(value)}else{match value {0..=44=>0,45..=74=>1,75..=92=>2,_=>3}}).max(floor);
    for n in 0..groups.len() {
        let variants=&groups[(start+n)%groups.len()];
        if let Some(id)=(0..4).map(|offset|(selected+offset)%4).filter(|t|*t>=floor).map(|t|variants[t as usize]).find(|variant|*variant!=0&&!run.claimed_items.contains(variant)){return Some(id);}
    }
    repeat.then_some(groups[start][selected as usize])
}
pub fn rewards(run:&Run,node:usize,mut offers:Vec<Reward>)->Vec<Reward> {
    if !run.affix_equipment{return offers;}
    let entropy=(node as u64).wrapping_mul(7919);
    let normal=run.nodes.get(node).is_some_and(|n|matches!(n.kind,crate::NodeKind::Monster|crate::NodeKind::Chest));
    if normal&&crate::mix(run.reward_seed()^entropy^0xa420)%4==0 {
        if let Some(slot)=offers.iter().position(|r|matches!(r,Reward::Item(id) if crate::rewards::lookup(*id).is_some_and(|i|i.kind==ItemKind::Weapon))) {
            let armor:Vec<_>=crate::rewards::CATALOG.iter().filter(|i|i.kind==ItemKind::Armor&&crate::dlc::item_allowed(i.id)).collect();
            let start=crate::mix(entropy^run.reward_seed()) as usize%armor.len();
            if let Some(item)=(0..armor.len()).map(|n|armor[(start+n)%armor.len()]).find(|i|roll(run,i.id,entropy^slot as u64,false).is_some()){offers[slot]=Reward::Item(item.id);}
        }
    }
    let kind=run.nodes.get(node).map(|n|n.kind);
    let repeat=kind==Some(crate::NodeKind::Boss);
    for (slot,reward) in offers.iter_mut().enumerate(){if let Reward::Item(id)=*reward {
        let min=match kind {Some(crate::NodeKind::Boss) if run.boss_reward_rules==0||slot==0=>3,Some(crate::NodeKind::Elite)=>2,_=>0};
        *reward=roll_min(run,id,entropy^slot as u64,repeat,min).map_or(Reward::Gold(1500),Reward::Item);
    }}
    offers
}
/// 盲盒：从本职业可用的武器与防具中抽一件，品质按盲盒档次随机（传说档武器至少为其原有品质）
pub fn mystery_box(run:&Run,node:usize,premium:bool)->Option<u32> {
    let roll=crate::mix(run.reward_seed()^(node as u64).wrapping_mul(7919)^0xb0b0_5e1d^u64::from(premium));
    let value=crate::mix(roll)%100;
    // 奖励装备的最低品质为稀有：普通盲盒 稀有 65／史诗 30／传说 5，高级盲盒 稀有 15／史诗 55／传说 30
    let tier:u8=if premium {match value {0..=14=>1,15..=69=>2,_=>3}} else {match value {0..=64=>1,65..=94=>2,_=>3}};
    // 先定品质，再只从能以该品质出现的装备里抽，说明里写的概率就是实际概率
    let pool:Vec<_>=crate::rewards::CATALOG.iter().filter(|i|(i.kind==ItemKind::Weapon&&i.random_eligible()||i.kind==ItemKind::Armor&&crate::dlc::item_allowed(i.id))&&i.kind.allowed(run.class)
        &&BY_BASE.contains_key(&i.id)&&(run.affix_rules>=3||BASE_RARITY.get(&i.id).copied().unwrap_or(0)<=tier)).collect();
    if pool.is_empty() {return None;}
    let start=(roll%pool.len() as u64) as usize;
    // 抽中的装备这局已经拿过时，顺延到下一件没拿过的
    (0..pool.len()).map(|n|pool[(start+n)%pool.len()].id).find_map(|base|variant(run,base,(roll>>32) as usize,tier).filter(|id|!run.claimed_items.contains(id)))
}
/// 重练炉：把武器换成同品质的另一把武器（基础武器不同、本职业可用），词条组合随机；同一座炉子每次（uses）抽到的不同
pub fn furnace(run:&Run,node:usize,current:u32,uses:u8)->Option<u32> {
    let (old_base,tier)=(base(current),tier(current)?);
    let roll=crate::mix(run.reward_seed()^(node as u64).wrapping_mul(7919)^0xf0_12ace^u64::from(current)^u64::from(uses).wrapping_mul(0x9e37_79b9));
    let pool:Vec<_>=crate::rewards::CATALOG.iter().filter(|i|i.kind==ItemKind::Weapon&&i.random_eligible()&&i.kind.allowed(run.class)
        &&i.id!=old_base&&BY_BASE.contains_key(&i.id)).map(|i|i.id).collect();
    if pool.is_empty() {return None;}
    let start=(roll%pool.len() as u64) as usize;
    let candidates=(0..pool.len()).filter_map(|n|variant(run,pool[(start+n)%pool.len()],(roll>>32) as usize,tier));
    // 这局还没拿过的优先
    let mut fallback=None;
    for id in candidates {if !run.claimed_items.contains(&id){return Some(id);} fallback.get_or_insert(id);}
    fallback
}
fn variant(run:&Run,base:u32,profile:usize,tier:u8)->Option<u32> {
    let id=if run.affix_rules>=2 {let groups=ADVANCED.get(&base)?;groups[profile%groups.len()][usize::from(tier)]}else{BY_BASE.get(&base)?[usize::from(tier)]};
    (id!=0).then_some(id)
}
/// 重铸：换成同一件装备在同品质下的另一种词条组合；有 1/5 的概率品质 +1（这时任意组合都可以）。
/// 精炼（guaranteed）必定品质 +1，传说品质不能精炼
pub fn reforge(run:&Run,node:usize,current:u32,guaranteed:bool)->Option<u32> {
    let (base,tier)=(base(current),tier(current)?);
    if guaranteed&&tier>=3 {return None;}
    let roll=crate::mix(run.reward_seed()^(node as u64).wrapping_mul(7919)^0x4e70_f09e^u64::from(current));
    let upgrade=guaranteed||tier<3&&roll%5==0;
    let target=if upgrade {tier+1}else{tier};
    let mut options:Vec<u32>=ADVANCED.get(&base).into_iter().flatten().map(|g|g[usize::from(target)])
        .chain(BY_BASE.get(&base).map(|g|g[usize::from(target)]).filter(|_|run.affix_rules<2))
        .filter(|id|*id!=0&&*id!=current).collect();
    options.dedup();
    (!options.is_empty()).then(||options[((roll>>8)%options.len() as u64) as usize])
}
#[test]
    fn quality_rises_gently_from_the_first_layer_to_the_last() {
        let mut run=Run::new(42,crate::Class::Samurai);
        let per_act=run.nodes.iter().map(|n|n.layer+1).max().unwrap();
        let mut previous=[0u32;4];
        for act in 1..=3u8 {for layer in 1..=per_act {
            run.act=act;run.reached_layer=layer;let odds=quality_odds(&run);
            assert_eq!(odds.iter().sum::<u32>(),100);
            if act==1&&layer==1 {assert_eq!(odds,QUALITY_START);}
            if act==3&&layer==per_act {assert_eq!(odds,QUALITY_END);}
            if previous!=[0;4] {assert!(odds[0]<=previous[0]&&odds[3]>=previous[3],"{odds:?} after {previous:?}");}
            previous=odds;
        }}
        run.act=1;run.reached_layer=0;assert_eq!(quality_odds(&run),QUALITY_START,"before entering a node");
        // 实际掷出的品质接近当前层的概率
        for (act,layer) in [(1u8,1usize),(3,per_act)] {
            run.act=act;run.reached_layer=layer;let odds=quality_odds(&run);
            let mut counts=[0u32;4];
            for value in 0..100u64 {counts[usize::from(layered_tier(&run,value))]+=1;}
            assert_eq!(counts,odds);
        }
    }
    
