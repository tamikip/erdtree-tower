use serde::{Deserialize,Serialize};
use crate::RuleResult;

pub const RULES:[(&str,&str,i32);10]=[
    ("allow_invaders","允许入侵",1),("death_debuffs","死亡惩罚",1),("allow_summons","允许召唤",1),("always_spectate_on_death","死亡观战",1),
    ("enemy_health_scaling","敌人生命缩放",1000),("enemy_damage_scaling","敌人伤害缩放",1000),("enemy_posture_scaling","敌人削韧抗性缩放",1000),
    ("boss_health_scaling","Boss 生命缩放",1000),("boss_damage_scaling","Boss 伤害缩放",1000),("boss_posture_scaling","Boss 削韧抗性缩放",1000),
];
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Compatibility {
    pub tower_build:String,pub seamless_version:String,pub seamless_hash:String,pub assets:String,pub rules:[i32;10],
}
impl Compatibility {
    pub fn validate(&self)->RuleResult<()> {
        if [&self.tower_build,&self.seamless_hash,&self.assets].iter().any(|s|s.len()!=16||!s.bytes().all(|b|b.is_ascii_hexdigit()))
            ||self.seamless_version.is_empty()||self.seamless_version.len()>24||!self.seamless_version.bytes().all(|b|b.is_ascii_digit()||b==b'.')
            ||self.rules.iter().zip(RULES).any(|(&v,(_,_,max))|v<0||v>max){return Err("开局兼容信息无效，请更新同一测试构建后重启游戏".into());}
        Ok(())
    }
    pub fn check(&self,other:&Self)->RuleResult<()> {
        self.validate()?;other.validate()?;
        for (label,left,right) in [("爬塔构建",&self.tower_build,&other.tower_build),("无缝版本",&self.seamless_version,&other.seamless_version),
            ("无缝文件",&self.seamless_hash,&other.seamless_hash),("关键资源",&self.assets,&other.assets)] {
            if left!=right{return Err(format!("{label}不一致：本机 {left}，对端 {right}。请全队更新同一整合包后重启游戏。"));}
        }
        for (index,(_,label,_)) in RULES.iter().enumerate() {
            if self.rules[index]!=other.rules[index]{return Err(format!("{label}不一致：本机 {}，对端 {}。请统一联机设置后重启游戏。",self.rules[index],other.rules[index]));}
        }
        Ok(())
    }
}
pub fn parse_rules(text:&str)->RuleResult<[i32;10]> {
    let mut found=[None;10];
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line=line.split([';','#']).next().unwrap_or("").trim();
        let Some((key,value))=line.split_once('=') else{continue;};
        let Some(index)=RULES.iter().position(|(name,_,_)|*name==key.trim()) else{continue;};
        let (_,label,max)=RULES[index];
        if found[index].is_some(){return Err(format!("联机设置重复：{label}，请修复配置后重启游戏"));}
        let value=value.trim().parse::<i32>().ok().filter(|v|(0..=max).contains(v)).ok_or_else(||format!("联机设置无效：{label}"))?;
        found[index]=Some(value);
    }
    let mut values=[0;10];
    for (i,entry) in found.into_iter().enumerate(){values[i]=entry.ok_or_else(||format!("联机设置缺少：{}，请更新配置后重启游戏",RULES[i].1))?;}
    Ok(values)
}
