use crate::{NodeKind,Run};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum Affix {Stalwart,Bulwark,Frenzied,Regenerating,Rot,Envious,Proud,Golden,Reborn,Leeching,Thorned,Swift}
impl Affix {
    /// 前 9 个是最早的词条（Boss 词条规则 0 只用这些），顺序不能变：血条名称按组合序号取
    pub const ALL:[Affix;12]=[Affix::Stalwart,Affix::Bulwark,Affix::Frenzied,Affix::Regenerating,Affix::Rot,Affix::Envious,Affix::Proud,Affix::Golden,Affix::Reborn,
        Affix::Leeching,Affix::Thorned,Affix::Swift];
    pub fn index(self)->usize {Self::ALL.iter().position(|a|*a==self).unwrap()}
    pub fn name(self)->&'static str {
        match self {Affix::Stalwart=>"坚毅的",Affix::Bulwark=>"铁壁的",Affix::Frenzied=>"狂暴的",Affix::Regenerating=>"再生的",Affix::Rot=>"腐败的",
            Affix::Envious=>"嫉妒的",Affix::Proud=>"傲慢的",Affix::Golden=>"黄金律法的",Affix::Reborn=>"不屈的",
            Affix::Leeching=>"吸血的",Affix::Thorned=>"荆棘的",Affix::Swift=>"迅猛的"}
    }
    pub fn description(self)->&'static str {
        match self {
            Affix::Stalwart=>"韧性翻倍；用重攻击和跳劈更快打出硬直",
            Affix::Bulwark=>"受到的伤害 -25%",
            Affix::Frenzied=>"攻击 +30%，但受到的伤害 +10%",
            Affix::Regenerating=>"连续 3 秒未受伤后，每秒回复 0.3% 最大生命；受伤后重新计时",
            Affix::Rot=>"攻击附带猩红腐败：每次打中你都会积累腐败；备好药",
            Affix::Envious=>"你喝圣杯瓶后，攻击 +30%，持续 15 秒",
            Affix::Proud=>"8 秒没挨打时攻击 +20%，被打中后清除",
            Affix::Golden=>"每 25 秒展开 5 秒金色护盾，受到的伤害 -60%",
            Affix::Reborn=>"生命首次降到 10% 以下时回复至 40%，仅一次",
            Affix::Leeching=>"每次打中你，回复其最大生命 1%",
            Affix::Thorned=>"受到伤害的 10% 反弹给你，一次最多 200（不会致死）",
            Affix::Swift=>"动作速度 +10%：出招、收招、移动都更快，压得更紧",
        }
    }
    /// 最早出现的幕：再生的第二幕起，吸血的第三幕起
    pub fn first_act(self)->u8 {match self {Affix::Regenerating=>2,Affix::Leeching=>3,_=>1}}
    pub fn cost(self)->u8 {match self {Affix::Bulwark|Affix::Regenerating|Affix::Envious|Affix::Golden|Affix::Thorned=>2,Affix::Reborn=>3,_=>1}}
    /// 互斥组：同组只能出一个。规则 0 的分组（只有防御、续航两组）用于还原最早的组合表
    fn group(self)->u8 {
        match self {
            Affix::Bulwark|Affix::Golden=>1,
            Affix::Regenerating|Affix::Reborn|Affix::Leeching=>2,
            Affix::Frenzied|Affix::Envious|Affix::Proud|Affix::Swift=>5,
            _=>0,
        }
    }
    fn legacy_group(self)->u8 {match self {Affix::Bulwark|Affix::Golden=>1,Affix::Regenerating|Affix::Reborn=>2,_=>0}}
}
pub const MAX_AFFIXES:usize=3;
/// 已移除、不再抽到的词条
pub const RETIRED:[Affix;1]=[Affix::Proud];
/// 最早的 9 个词条的组合（规则 0）：序号与已生成的血条名称一一对应
pub const LEGACY_AFFIXES:usize=9;
pub const BAR_NAME_FIRST:i32=9700000;
pub const BAR_NAME_STRIDE:i32=200;
/// 新增词条后的组合接在最早的组合之后，血条名称从这里起每个组合 144 个编号（Boss 名称序号最大 139）
pub const BAR_NAME_SECOND:i32=9722200;
pub const BAR_NAME_SECOND_STRIDE:i32=144;
/// 之后追加的 Boss（追忆、DLC）从这个名称序号开始；它们的带词条名称要等 text-update 写入第三个标记后才显示
pub const LATE_BOSS_FIRST:i32=140;
/// 新增词条后的组合里，名称序号 144 起的 Boss 放在第三段，每个组合 64 个编号（序号 144–207）
pub const BAR_NAME_THIRD:i32=9740000;
pub const BAR_NAME_THIRD_STRIDE:i32=64;
pub const POOL_BAR_NAME_FIRST:i32=98000000;
pub const POOL_BAR_NAME_STRIDE:i32=256;
pub fn pool_bar_name(affixes:&[Affix],name:i32,first:i32)->Option<i32> {
    let offset=name.checked_sub(first)?;
    if !(0..POOL_BAR_NAME_STRIDE).contains(&offset) {return None;}
    let index=combos().iter().position(|c|c==affixes)?;
    Some(POOL_BAR_NAME_FIRST+index as i32*POOL_BAR_NAME_STRIDE+offset)
}
fn allowed(affixes:&[Affix],group:fn(Affix)->u8)->bool {
    affixes.len()<=MAX_AFFIXES&&points(affixes)<=budget(3)&&affixes.iter().filter(|a|group(**a)!=0).all(|a|affixes.iter().filter(|b|group(**b)==group(*a)).count()==1)
}
fn enumerate(pool:&[Affix],group:fn(Affix)->u8)->Vec<Vec<Affix>> {
    let mut combos=Vec::new();let n=pool.len();
    for size in 1..=MAX_AFFIXES {for mask in 1u32..(1<<n) {
        if mask.count_ones() as usize!=size {continue;}
        let combo:Vec<_>=pool.iter().copied().enumerate().filter(|(i,_)|mask&(1<<i)!=0).map(|(_,a)|a).collect();
        if allowed(&combo,group){combos.push(combo);}
    }}
    combos
}
/// 先是最早的组合（顺序不变），再接上含新词条、按新分组允许的组合
static COMBOS:std::sync::LazyLock<(Vec<Vec<Affix>>,usize)>=std::sync::LazyLock::new(|| {
    let mut combos=enumerate(&Affix::ALL[..LEGACY_AFFIXES],Affix::legacy_group);let legacy=combos.len();
    for combo in enumerate(&Affix::ALL,Affix::group) {if !combos.contains(&combo){combos.push(combo);}}
    (combos,legacy)
});
pub fn combos()->&'static [Vec<Affix>] {&COMBOS.0}
/// 最早的组合数：之后的组合需要重新生成血条名称（text-update 写入第二个标记）后才显示带词条的名字
pub fn legacy_combos()->usize {COMBOS.1}
/// 所有带词条的血条名称编号
pub fn bar_name_range()->std::ops::Range<i32> {BAR_NAME_FIRST..BAR_NAME_THIRD+(combos().len()-legacy_combos()) as i32*BAR_NAME_THIRD_STRIDE}
pub fn combo_table()->String {combos().iter().enumerate().map(|(i,c)|format!("{i}|{}\n",title(c))).collect()}
/// 血条名称：最早的组合 9700000 + 序号 × 200 + Boss 名称序号；之后的组合 9722200 + (序号 − 最早组合数) × 144 + Boss 名称序号，
/// 名称序号 144 起放在第三段 9740000 + (序号 − 最早组合数) × 64 + (Boss 名称序号 − 144)
pub fn bar_name(affixes:&[Affix],boss_name:i32,first:i32)->Option<i32> {
    let index=combos().iter().position(|c|c==affixes)?;let offset=boss_name-first;
    let legacy=legacy_combos();
    if index<legacy {return (0..BAR_NAME_STRIDE).contains(&offset).then(||BAR_NAME_FIRST+index as i32*BAR_NAME_STRIDE+offset);}
    let late=(index-legacy) as i32;
    if (0..BAR_NAME_SECOND_STRIDE).contains(&offset) {return Some(BAR_NAME_SECOND+late*BAR_NAME_SECOND_STRIDE+offset);}
    (BAR_NAME_SECOND_STRIDE..BAR_NAME_SECOND_STRIDE+BAR_NAME_THIRD_STRIDE).contains(&offset).then(||BAR_NAME_THIRD+late*BAR_NAME_THIRD_STRIDE+offset-BAR_NAME_SECOND_STRIDE)
}
pub fn budget(act:u8)->u8 {match act {1=>2,2=>4,_=>6}}
/// 本场的词条预算；苦难律法可以提高 Boss 预算并让精英战也带词条
pub fn node_budget(run:&Run,node:usize)->u8 {
    match run.nodes.get(node).map(|n|n.kind) {
        Some(NodeKind::Boss)=>run.law.affix_budget(budget(run.act),budget(3)),
        Some(NodeKind::Elite) if run.law.elite_affixes()=>budget(run.act),
        _=>0,
    }
}
pub fn roll(run:&Run,node:usize)->Vec<Affix> {
    let budget=node_budget(run,node);
    if !run.boss_affixes||budget==0 {return vec![];}
    // 规则 0（旧存档）只用最早的 9 个词条和原来的分组
    let (count,group):(usize,fn(Affix)->u8)=if run.boss_affix_rules>=1||run.law.swift_affixes() {(Affix::ALL.len(),Affix::group)}else{(LEGACY_AFFIXES,Affix::legacy_group)};
    // 傲慢的已移除：不再抽到（枚举、组合表与血条名称编号保留，旧组合的序号不变）；再生的第二幕起、吸血的第三幕起才会出现
    let mut pool:Vec<_>=Affix::ALL[..count].iter().copied().filter(|a|*a!=Affix::Swift&&!RETIRED.contains(a)&&run.act>=a.first_act()).collect();
    pool.sort_by_key(|a|crate::mix(run.seed^(node as u64).wrapping_mul(7919)^(u64::from(run.act)<<40)^a.index() as u64^0xb055_af1c));
    let (mut left,mut chosen)=(budget,Vec::new());
    let swift=run.law.swift_affixes()&&match run.nodes[node].kind {
        NodeKind::Boss=>true,
        NodeKind::Elite=>run.law.elite_affixes()&&crate::mix(run.seed^(node as u64).wrapping_mul(7919)^(u64::from(run.act)<<40)^0x5a1f_25)%4==0,
        _=>false,
    };
    if swift {chosen.push(Affix::Swift);left=left.saturating_sub(Affix::Swift.cost());}
    for affix in pool {
        if chosen.len()==MAX_AFFIXES {break;}
        if affix.cost()<=left&&(group(affix)==0||!chosen.iter().any(|c:&Affix|group(*c)==group(affix))) {left-=affix.cost();chosen.push(affix);}
    }
    chosen.sort_by_key(|a|a.index());chosen
}
pub fn points(affixes:&[Affix])->u8 {affixes.iter().map(|a|a.cost()).sum()}
pub fn bonus_runes(run:&Run,node:usize)->u32 {
    let base=match run.act {1=>9000,2=>18000,_=>30000}/if run.nodes.get(node).is_some_and(|n|n.kind==NodeKind::Elite){3}else{1};
    run.law.runes(base*15*u32::from(points(&roll(run,node)))/100)
}
pub fn title(affixes:&[Affix])->String {affixes.iter().map(|a|a.name()).collect::<Vec<_>>().join("·")}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Class;
    fn boss(run:&Run)->usize {run.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap()}
    #[test]
    fn swift_is_opt_in_guaranteed_for_bosses_and_quarter_of_enabled_elites() {
        let mut run=Run::new(1,Class::Samurai);let node=boss(&run);let mut hits=0;
        for seed in 0..1024 {
            run.seed=seed;run.nodes[node].kind=NodeKind::Boss;run.law=crate::law::Law::default();
            assert!(!roll(&run,node).contains(&Affix::Swift));
            run.law=run.law.toggle(35);let affixes=roll(&run,node);
            assert_eq!(affixes.iter().filter(|a|**a==Affix::Swift).count(),1);
            assert!(combos().contains(&affixes)&&points(&affixes)<=node_budget(&run,node));
            run.nodes[node].kind=NodeKind::Elite;assert!(roll(&run,node).is_empty());
            run.law=run.law.toggle(5);let affixes=roll(&run,node);
            let expected=crate::mix(seed^(node as u64).wrapping_mul(7919)^(u64::from(run.act)<<40)^0x5a1f_25)%4==0;
            assert_eq!(affixes.contains(&Affix::Swift),expected);hits+=usize::from(expected);
            assert!(combos().contains(&affixes));
        }
        assert!((210..=300).contains(&hits),"{hits}/1024");
        let law=crate::law::Law::default().toggle(35);assert_eq!(law.code().len(),9);assert_eq!(crate::law::Law::parse(&law.code()),Some(law));
    }
    #[test]
    fn elite_pool_titles_are_unique_and_do_not_overlap_boss_titles() {
        let builder=include_str!("../scene-builder/AffixTextFix.cs");
        assert!(builder.contains(&format!("PoolBarNameFirst={POOL_BAR_NAME_FIRST}")));
        assert!(builder.contains(&format!("PoolBarNameStride={POOL_BAR_NAME_STRIDE}")));
        let pool:std::collections::HashSet<_>=include_str!("../scene-assets/enemy-pools.txt").lines().map(|l|l.split('|').nth(4).unwrap()).collect();
        assert!(pool.len()<=POOL_BAR_NAME_STRIDE as usize);
        let mut names=std::collections::HashSet::new();
        for combo in combos() {for offset in 0..POOL_BAR_NAME_STRIDE {
            let id=pool_bar_name(combo,9610000+offset,9610000).unwrap();
            assert!(!bar_name_range().contains(&id));
            assert!(names.insert(id));
        }}
        assert_eq!(pool_bar_name(&[],9610000,9610000),None);
        assert_eq!(pool_bar_name(&[Affix::Swift],9609999,9610000),None);
        assert_eq!(pool_bar_name(&[Affix::Swift],9610256,9610000),None);
    }
    #[test]
    fn rolls_respect_budget_groups_and_are_stable() {
        let mut seen=std::collections::HashSet::new();
        for seed in 0..300 {for act in 1..=3 {
            let mut run=Run::new(seed,Class::Samurai);run.act=act;let node=boss(&run);
            let affixes=roll(&run,node);
            assert_eq!(affixes,roll(&run,node));
            assert!(!affixes.is_empty()&&affixes.len()<=MAX_AFFIXES&&points(&affixes)<=budget(act));
            assert!(allowed(&affixes,Affix::group));
            assert_eq!(bonus_runes(&run,node),[9000,18000,30000][usize::from(act)-1]*15*u32::from(points(&affixes))/100);
            seen.extend(affixes);
        }}
        assert_eq!(seen.len(),Affix::ALL.len()-RETIRED.len()-1,"Swift requires its law");
        assert!(!seen.contains(&Affix::Swift));
        assert!(RETIRED.iter().all(|a|!seen.contains(a)));
        // 再生的第二幕起、吸血的第三幕起
        for seed in 0..600 {for act in 1..=3 {
            let mut run=Run::new(seed,Class::Samurai);run.act=act;let affixes=roll(&run,boss(&run));
            assert!(affixes.iter().all(|a|act>=a.first_act()),"act {act}: {affixes:?}");
        }}
    }
    #[test]
    fn every_roll_has_a_bar_name_and_the_combo_table_is_current() {
        assert_eq!(include_str!("../scene-assets/boss-affix-combos.txt").replace("\r\n","\n"),combo_table(),"regenerate scene-assets/boss-affix-combos.txt from boss_affixes::combo_table()");
        assert_eq!(legacy_combos(),111,"the earliest combos keep their indices");
        assert_eq!(BAR_NAME_FIRST+legacy_combos() as i32*BAR_NAME_STRIDE,BAR_NAME_SECOND);
        assert!(BAR_NAME_SECOND+(combos().len()-legacy_combos()) as i32*BAR_NAME_SECOND_STRIDE<=BAR_NAME_THIRD);
        assert!(BAR_NAME_THIRD+(combos().len()-legacy_combos()) as i32*BAR_NAME_THIRD_STRIDE<9799999,"stays below the colour tag test name");
        let mut names=std::collections::HashSet::new();
        for seed in 0..500 {for act in 1..=3 {for rules in [0,1] {
            let mut run=Run::new(seed,Class::Samurai);run.act=act;run.boss_affix_rules=rules;let affixes=roll(&run,boss(&run));
            let index=combos().iter().position(|c|c==&affixes).expect("every roll has a combo");
            if rules==0 {assert!(index<legacy_combos(),"old saves only roll the earliest combos");}
            let name=bar_name(&affixes,9600139,9600000).unwrap();names.insert(name);
            let (first,stride)=if index<legacy_combos() {(BAR_NAME_FIRST,BAR_NAME_STRIDE)}else{(BAR_NAME_SECOND,BAR_NAME_SECOND_STRIDE)};
            assert_eq!((name-first)%stride,139);
        }}}
        assert_eq!(bar_name(&[],9600005,9600000),None);assert_eq!(bar_name(&[Affix::Rot],9600000+BAR_NAME_STRIDE,9600000),None);
        assert_eq!(bar_name(&[Affix::Swift],9600000+BAR_NAME_SECOND_STRIDE+BAR_NAME_THIRD_STRIDE,9600000),None);
        // 所有组合、所有 Boss（含之后追加的 140–152 号）的名称编号互不重叠
        let late=include_str!("../scene-assets/boss-pool.txt").lines().count() as i32-1;
        assert!(late>=152&&late<BAR_NAME_SECOND_STRIDE+BAR_NAME_THIRD_STRIDE);
        let all:std::collections::HashSet<_>=combos().iter().flat_map(|c|[0,139,140,143,144,late].map(|o|bar_name(c,9600000+o,9600000).unwrap())).collect();
        assert_eq!(all.len(),combos().len()*6);
        assert!(all.iter().all(|id|bar_name_range().contains(id)));
    }
    #[test]
    fn only_new_runs_and_boss_nodes_get_affixes() {
        let mut run=Run::new(5,Class::Samurai);let node=boss(&run);
        assert!(run.nodes.iter().enumerate().filter(|(_,n)|n.kind!=NodeKind::Boss).all(|(id,_)|roll(&run,id).is_empty()));
        run.boss_affixes=false;assert!(roll(&run,node).is_empty());assert_eq!(bonus_runes(&run,node),0);
    }
    #[test]
    fn saves_without_the_flag_keep_old_boss_rules() {
        let run=Run::new(9,Class::Samurai);
        let mut json:serde_json::Value=serde_json::from_str(&run.encode().unwrap()).unwrap();
        json.as_object_mut().unwrap().remove("boss_affixes");
        let old=Run::decode(&json.to_string()).unwrap();
        assert!(!old.boss_affixes&&roll(&old,boss(&old)).is_empty());
        assert!(Run::decode(&run.encode().unwrap()).unwrap().boss_affixes);
    }
    #[test]
    fn law_raises_boss_budget_and_spreads_affixes_to_elites() {
        use crate::law::Law;
        let law=|ids:&[usize]|ids.iter().fold(Law::default(),|l,&id|l.toggle(id));
        for seed in 0..200 {
            let mut run=Run::new(seed,Class::Samurai);let node=boss(&run);
            let elite=run.nodes.iter().position(|n|n.kind==NodeKind::Elite);
            if let Some(e)=elite {assert!(roll(&run,e).is_empty());}
            run.set_law(law(&[3])).unwrap();assert_eq!(node_budget(&run,node),4);assert!(points(&roll(&run,node))<=4);
            run.law=law(&[3,4]);assert_eq!(node_budget(&run,node),6);
            let affixes=roll(&run,node);assert!(combos().contains(&affixes),"every law roll keeps a bar name");
            run.law=law(&[3,4,5]);
            if let Some(e)=elite {let affixes=roll(&run,e);assert!(!affixes.is_empty()&&points(&affixes)<=2);assert_eq!(bonus_runes(&run,e),3000*15*u32::from(points(&affixes))/100);}
            run.law=law(&[19,20]);assert_eq!(bonus_runes(&run,node),9000*15*u32::from(points(&roll(&run,node)))/100*70/100);
        }
    }
    #[test]
    fn old_saves_keep_the_original_nine_affixes() {
        let mut new_seen=std::collections::HashSet::new();
        for seed in 0..300 {for act in 1..=3 {
            let mut run=Run::new(seed,Class::Samurai);run.act=act;let node=boss(&run);
            new_seen.extend(roll(&run,node).into_iter().filter(|a|a.index()>=LEGACY_AFFIXES));
            run.boss_affix_rules=0;
            assert!(roll(&run,node).iter().all(|a|a.index()<LEGACY_AFFIXES));
            let mut json:serde_json::Value=serde_json::from_str(&run.encode().unwrap()).unwrap();
            json.as_object_mut().unwrap().remove("boss_affix_rules");
            assert_eq!(Run::decode(&json.to_string()).unwrap().boss_affix_rules,0);
        }}
        assert_eq!(new_seen.len(),2,"Swift is excluded without its law");
    }
    #[test]
    fn party_members_share_affixes_but_not_reward_seeds() {
        let host=Run::new_personal(77,Class::Samurai,1);let guest=Run::new_personal(77,Class::Astrologer,2);
        assert_eq!(roll(&host,boss(&host)),roll(&guest,boss(&guest)));
    }
}
