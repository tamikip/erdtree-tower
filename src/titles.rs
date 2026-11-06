//! 称号：局外的外观奖励，不加数值。按旅程档案（tower-runs.jsonl）里的成绩解锁，每人自选一个；
//! 联机时由房主同步给全队，显示在原版的玩家头顶名牌上（名字前加“称号·”，HUD 补丁按称号给名字上特效）。
//! 名牌由游戏用原版字体绘制，称号只能用 GB2312 一级常用字（见测试），否则会像“涅槃的”一样显示乱码。
use crate::journal::{Outcome,Record};

/// 名字特效，与 scene-builder/native_name_tag_glow.py 的 STYLES 一一对应（顺序不能变）
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Style {Ash,Gold,Ember,Blood,Star,Frost,Moon}
impl Style {
    pub const ALL:[Style;7]=[Style::Ash,Style::Gold,Style::Ember,Style::Blood,Style::Star,Style::Frost,Style::Moon];
    pub fn name(self)->&'static str {
        match self {Style::Ash=>"灰烬",Style::Gold=>"黄金流光",Style::Ember=>"余火",Style::Blood=>"鲜血",Style::Star=>"星辉",Style::Frost=>"寒霜",Style::Moon=>"月光"}
    }
    /// (主色, 亮色)：称号在两者之间流动
    pub fn colors(self)->(u32,u32) {
        match self {
            Style::Ash=>(0xB8B0A0,0xEDE6D6),Style::Gold=>(0xE8B84C,0xFFF4C8),Style::Ember=>(0xE86A2C,0xFFC070),
            Style::Blood=>(0xB01E2E,0xFF5A6A),Style::Star=>(0x8C7CFF,0x7CF0FF),Style::Frost=>(0x6CB8F0,0xE0F6FF),Style::Moon=>(0x9CA8D8,0xF0F2FF),
        }
    }
}

pub struct Title {pub id:u8,pub text:&'static str,pub style:Style,pub unlock:&'static str}
/// 编号写进存档与联机同步，只能追加
pub const TITLES:[Title;7]=[
    Title{id:1,text:"褪色者",style:Style::Ash,unlock:"默认拥有"},
    Title{id:2,text:"登顶者",style:Style::Gold,unlock:"登顶 1 次"},
    Title{id:3,text:"艰难之王",style:Style::Ember,unlock:"苦难律法 140 分以上登顶"},
    Title{id:4,text:"长夜之王",style:Style::Blood,unlock:"苦难律法 210 分以上登顶"},
    Title{id:5,text:"危机之王",style:Style::Star,unlock:"苦难律法 280 分以上登顶"},
    Title{id:6,text:"百战之身",style:Style::Frost,unlock:"完成 20 局旅程"},
    Title{id:7,text:"孤狼",style:Style::Moon,unlock:"立下「孤狼」登顶（该律法已停用，之前解锁的保留）"},
];
/// 称号与名字之间的分隔符（原版 Boss 名里也用它，原版字体有）
pub const SEPARATOR:char='·';

pub fn lookup(id:u8)->Option<&'static Title> {TITLES.iter().find(|t|t.id==id)}
/// 按旅程档案判断是否解锁（调试旅程不算）
pub fn unlocked(id:u8,records:&[Record])->bool {
    let real=||records.iter().filter(|r|!r.debug);
    let won=||real().filter(|r|r.result==Outcome::Won);
    match id {
        1=>true,
        2=>won().next().is_some(),
        3=>won().any(|r|r.law.score()>=140),
        4=>won().any(|r|r.law.score()>=210),
        5=>won().any(|r|r.law.score()>=280),
        6=>real().count()>=20,
        7=>won().any(|r|r.law.lone_wolf()),
        _=>false,
    }
}
/// 选用的称号：没有解锁或编号无效时不显示（0 表示不戴称号）
pub fn worn(id:u8,records:&[Record])->Option<&'static Title> {lookup(id).filter(|t|unlocked(t.id,records))}
/// 去掉名字前已经加上的称号（只认已知称号，玩家自己起的名字里带“·”不受影响）
pub fn strip(name:&str)->&str {
    TITLES.iter().find_map(|t|name.strip_prefix(t.text).and_then(|rest|rest.strip_prefix(SEPARATOR))).unwrap_or(name)
}
/// 名牌上显示的名字：称号·名字（先去掉旧称号，换称号或摘掉称号都不会叠加）
pub fn decorate(name:&str,title:Option<&Title>)->String {
    let name=strip(name);
    title.map_or_else(||name.to_owned(),|t|format!("{}{SEPARATOR}{name}",t.text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Class,Run,journal::Record};
    fn level_one(c:char)->bool {
        // 称号用到的字，都已核对是 GB2312 一级常用字（区码 0xB0–0xD7，原版文本量大，原版字体都有）；
        // 新称号的字先用 python 的 "字".encode("gb2312") 核对区码再加进来
        const ALLOWED:&str="褪色者登顶艰难之王长夜危机百战身孤狼";
        ALLOWED.contains(c)
    }
    #[test]
    fn titles_use_only_common_characters_and_unique_ids() {
        for (i,t) in TITLES.iter().enumerate() {
            assert!(t.text.chars().all(level_one),"{} uses a character the native font may lack",t.text);
            assert!(!t.text.contains(SEPARATOR)&&TITLES[..i].iter().all(|o|o.id!=t.id&&o.text!=t.text));
            assert!(t.id>0);
        }
        assert_eq!(Style::ALL.len(),7);
    }
    #[test]
    fn decorating_replaces_the_previous_title_and_keeps_player_dots() {
        let gold=lookup(2);let wolf=lookup(7);
        assert_eq!(decorate("Tarnished",gold),"登顶者·Tarnished");
        assert_eq!(decorate("登顶者·Tarnished",wolf),"孤狼·Tarnished");
        assert_eq!(decorate("孤狼·Tarnished",None),"Tarnished");
        assert_eq!(decorate("A·B",gold),"登顶者·A·B","a dot in the player's own name is kept");
        assert_eq!(strip("A·B"),"A·B");
    }
    #[test]
    fn unlocks_follow_the_archive() {
        let run=Run::new(1,Class::Hero);
        let mut won=Record::new(&run,vec![],vec![],0);won.result=Outcome::Won;
        let lost=Record::new(&run,vec![],vec![],0);
        assert!(unlocked(1,&[])&&!unlocked(2,&[lost.clone()])&&unlocked(2,&[won.clone()]));
        assert!(!unlocked(3,&[won.clone()]));
        // 孤狼已停用，选不上；之前带着它登顶的档案照样解锁称号
        let mut hard=won.clone();hard.law=crate::law::Law((0..crate::law::NODES.len()).fold(crate::law::Law::default(),|l,id|l.toggle(id)).0|1<<14);
        assert!(unlocked(3,std::slice::from_ref(&hard))&&unlocked(5,std::slice::from_ref(&hard))&&unlocked(7,std::slice::from_ref(&hard)));
        let mut debug=hard.clone();debug.debug=true;assert!(!unlocked(5,&[debug]),"debug runs do not count");
        assert!(!unlocked(6,&vec![lost.clone();19])&&unlocked(6,&vec![lost;20]));
        assert!(worn(4,&[won.clone()]).is_none()&&worn(2,&[won]).is_some()&&worn(0,&[]).is_none());
    }
}
