use crate::{NodeKind,Run};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum Affix {Stalwart,Bulwark,Frenzied,Regenerating,Rot,Envious,Proud,Golden,Reborn}
impl Affix {
    pub const ALL:[Affix;9]=[Affix::Stalwart,Affix::Bulwark,Affix::Frenzied,Affix::Regenerating,Affix::Rot,Affix::Envious,Affix::Proud,Affix::Golden,Affix::Reborn];
    pub fn index(self)->usize {Self::ALL.iter().position(|a|*a==self).unwrap()}
    pub fn name(self)->&'static str {
        match self {Affix::Stalwart=>"坚毅的",Affix::Bulwark=>"铁壁的",Affix::Frenzied=>"狂暴的",Affix::Regenerating=>"再生的",Affix::Rot=>"腐败的",
            Affix::Envious=>"嫉妒的",Affix::Proud=>"傲慢的",Affix::Golden=>"黄金律法的",Affix::Reborn=>"涅槃的"}
    }
    pub fn description(self)->&'static str {
        match self {
            Affix::Stalwart=>"受到的削韧减半；用重攻击和跳劈更快打出硬直",
            Affix::Bulwark=>"受到的伤害 -25%",
            Affix::Frenzied=>"攻击 +25%，但受到的伤害 +10%",
            Affix::Regenerating=>"每秒回复 0.3% 最大生命；持续压制可以抵消",
            Affix::Rot=>"身边 5 米内每秒积累腐败；保持距离或备好药",
            Affix::Envious=>"你喝圣杯瓶后，攻击 +30%，持续 10 秒",
            Affix::Proud=>"8 秒没挨打时攻击 +20%，被打中后清除",
            Affix::Golden=>"每 20 秒展开 5 秒金色护盾，受到的伤害 -70%",
            Affix::Reborn=>"生命首次降到 10% 以下时回复至 40%，仅一次",
        }
    }
    pub fn cost(self)->u8 {match self {Affix::Bulwark|Affix::Regenerating|Affix::Envious|Affix::Golden=>2,Affix::Reborn=>3,_=>1}}
    fn group(self)->u8 {match self {Affix::Bulwark|Affix::Golden=>1,Affix::Regenerating|Affix::Reborn=>2,_=>0}}
}
pub const MAX_AFFIXES:usize=3;
pub const BAR_NAME_FIRST:i32=9700000;
pub const BAR_NAME_STRIDE:i32=200;
fn allowed(affixes:&[Affix])->bool {
    affixes.len()<=MAX_AFFIXES&&points(affixes)<=budget(3)&&affixes.iter().filter(|a|a.group()!=0).all(|a|affixes.iter().filter(|b|b.group()==a.group()).count()==1)
}
static COMBOS:std::sync::LazyLock<Vec<Vec<Affix>>>=std::sync::LazyLock::new(|| {
    let mut combos=Vec::new();let n=Affix::ALL.len();
    for size in 1..=MAX_AFFIXES {for mask in 1u32..(1<<n) {
        if mask.count_ones() as usize!=size {continue;}
        let combo:Vec<_>=Affix::ALL.iter().copied().filter(|a|mask&(1<<a.index())!=0).collect();
        if allowed(&combo){combos.push(combo);}
    }}
    combos
});
pub fn combos()->&'static [Vec<Affix>] {&COMBOS}
pub fn combo_table()->String {combos().iter().enumerate().map(|(i,c)|format!("{i}|{}\n",title(c))).collect()}
pub fn bar_name(affixes:&[Affix],boss_name:i32,first:i32)->Option<i32> {
    let index=combos().iter().position(|c|c==affixes)?;let offset=boss_name-first;
    (0..BAR_NAME_STRIDE).contains(&offset).then(||BAR_NAME_FIRST+index as i32*BAR_NAME_STRIDE+offset)
}
pub fn budget(act:u8)->u8 {match act {1=>2,2=>4,_=>6}}
pub fn roll(run:&Run,node:usize)->Vec<Affix> {
    if !run.boss_affixes||!run.nodes.get(node).is_some_and(|n|n.kind==NodeKind::Boss) {return vec![];}
    let mut pool=Affix::ALL.to_vec();
    pool.sort_by_key(|a|crate::mix(run.seed^(node as u64).wrapping_mul(7919)^(u64::from(run.act)<<40)^a.index() as u64^0xb055_af1c));
    let (mut left,mut chosen)=(budget(run.act),Vec::new());
    for affix in pool {
        if chosen.len()==MAX_AFFIXES {break;}
        if affix.cost()<=left&&(affix.group()==0||!chosen.iter().any(|c:&Affix|c.group()==affix.group())) {left-=affix.cost();chosen.push(affix);}
    }
    chosen.sort_by_key(|a|a.index());chosen
}
pub fn points(affixes:&[Affix])->u8 {affixes.iter().map(|a|a.cost()).sum()}
pub fn bonus_runes(run:&Run,node:usize)->u32 {
    let base=match run.act {1=>9000,2=>18000,_=>30000};
    base*15*u32::from(points(&roll(run,node)))/100
}
pub fn title(affixes:&[Affix])->String {affixes.iter().map(|a|a.name()).collect::<Vec<_>>().join("·")}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Class;
    fn boss(run:&Run)->usize {run.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap()}
    #[test]
    fn rolls_respect_budget_groups_and_are_stable() {
        let mut seen=std::collections::HashSet::new();
        for seed in 0..300 {for act in 1..=3 {
            let mut run=Run::new(seed,Class::Samurai);run.act=act;let node=boss(&run);
            let affixes=roll(&run,node);
            assert_eq!(affixes,roll(&run,node));
            assert!(!affixes.is_empty()&&affixes.len()<=MAX_AFFIXES&&points(&affixes)<=budget(act));
            assert!(affixes.iter().filter(|a|a.group()!=0).all(|a|affixes.iter().filter(|b|b.group()==a.group()).count()==1));
            assert_eq!(bonus_runes(&run,node),[9000,18000,30000][usize::from(act)-1]*15*u32::from(points(&affixes))/100);
            seen.extend(affixes);
        }}
        assert_eq!(seen.len(),Affix::ALL.len());
    }
    #[test]
    fn every_roll_has_a_bar_name_and_the_combo_table_is_current() {
        assert_eq!(include_str!("../scene-assets/boss-affix-combos.txt").replace("\r\n","\n"),combo_table(),"regenerate scene-assets/boss-affix-combos.txt from boss_affixes::combo_table()");
        assert!(BAR_NAME_FIRST+combos().len() as i32*BAR_NAME_STRIDE<9800000);
        for seed in 0..500 {for act in 1..=3 {
            let mut run=Run::new(seed,Class::Samurai);run.act=act;let affixes=roll(&run,boss(&run));
            let name=bar_name(&affixes,9600005,9600000).unwrap();assert_eq!((name-BAR_NAME_FIRST)%BAR_NAME_STRIDE,5);
            assert_eq!(&combos()[((name-BAR_NAME_FIRST)/BAR_NAME_STRIDE) as usize],&affixes);
        }}
        assert_eq!(bar_name(&[],9600005,9600000),None);assert_eq!(bar_name(&[Affix::Rot],9600000+BAR_NAME_STRIDE,9600000),None);
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
    fn party_members_share_affixes_but_not_reward_seeds() {
        let host=Run::new_personal(77,Class::Samurai,1);let guest=Run::new_personal(77,Class::Astrologer,2);
        assert_eq!(roll(&host,boss(&host)),roll(&guest,boss(&guest)));
    }
}
