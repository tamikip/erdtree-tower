use std::{borrow::Cow,collections::HashMap,sync::{LazyLock,atomic::{AtomicBool,Ordering}}};

#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub enum Language {#[default] Auto,Chinese,English,TraditionalChinese}
impl Language {
    pub fn id(self)->&'static str {match self {Self::Auto=>"auto",Self::Chinese=>"zh-CN",Self::English=>"en-US",Self::TraditionalChinese=>"zh-TW"}}
    pub fn parse(value:&str)->Option<Self> {match value.trim().to_ascii_lowercase().as_str(){"auto"|"game"=>Some(Self::Auto),"zh-cn"|"zhocn"|"chinese"=>Some(Self::Chinese),"en-us"|"engus"|"english"|"en"=>Some(Self::English),"zh-tw"|"zhotw"|"zh-hant"|"traditional"|"tc"=>Some(Self::TraditionalChinese),_=>None}}
}
static ENGLISH:AtomicBool=AtomicBool::new(false);
static TRADITIONAL:AtomicBool=AtomicBool::new(false);
pub fn set(language:Language){ENGLISH.store(language==Language::English,Ordering::Relaxed);TRADITIONAL.store(language==Language::TraditionalChinese,Ordering::Relaxed);}
pub fn english()->bool {ENGLISH.load(Ordering::Relaxed)}
pub fn traditional()->bool {TRADITIONAL.load(Ordering::Relaxed)}

/// 简体 → 繁体（台湾正体）：OpenCC 的 s2twp 三步（简繁词组与单字、台湾常用词、台湾异体字），表见 localization/zh-TW，生成器 generate_zh_tw.py。
/// 每步都是最长匹配；只用于显示，存档键、物品编号、联机消息仍是简体原文。
struct Convert {table:HashMap<&'static str,&'static str>,longest:usize}
impl Convert {
    fn new(data:&'static str)->Self {
        let table:HashMap<_,_>=data.lines().filter_map(|l|l.split_once('\t')).collect();
        Self{longest:table.keys().map(|k|k.chars().count()).max().unwrap_or(1),table}
    }
    fn apply(&self,text:&str)->String {
        let chars:Vec<(usize,char)>=text.char_indices().collect();let mut out=String::with_capacity(text.len());let mut i=0;
        while i<chars.len() {
            let mut hit=None;
            for len in (1..=self.longest.min(chars.len()-i)).rev() {
                let start=chars[i].0;let end=chars.get(i+len).map_or(text.len(),|c|c.0);
                if let Some(value)=self.table.get(&text[start..end]) {hit=Some((len,*value));break;}
            }
            match hit {Some((len,value))=>{out.push_str(value);i+=len;},None=>{out.push(chars[i].1);i+=1;}}
        }
        out
    }
}
static TRADITIONAL_TABLES:LazyLock<[Convert;3]>=LazyLock::new(||[
    Convert::new(include_str!("../localization/zh-TW/st.tsv")),
    Convert::new(include_str!("../localization/zh-TW/phrases.tsv")),
    Convert::new(include_str!("../localization/zh-TW/variants.tsv")),
]);
pub fn to_traditional(text:&str)->Cow<'_,str> {
    if !cjk(text) {return Cow::Borrowed(text);}
    let [st,phrases,variants]=&*TRADITIONAL_TABLES;
    let converted=variants.apply(&phrases.apply(&st.apply(text)));
    if converted==text {Cow::Borrowed(text)}else{Cow::Owned(converted)}
}
fn cjk(text:&str)->bool {text.chars().any(|c|('\u{3400}'..='\u{9fff}').contains(&c))}

struct Pattern {parts:Vec<String>,names:Vec<String>,target:String,weight:usize}
impl Pattern {
    fn new(source:&str,target:&str)->Option<Self> {
        let mut parts=Vec::new();let mut names=Vec::new();let mut rest=source;
        while let Some(start)=rest.find('{') {
            if rest[start..].starts_with("{{"){return None;}
            let end=rest[start+1..].find('}')?+start+1;
            let field=rest[start+1..end].split(':').next()?;
            parts.push(rest[..start].to_owned());names.push(if field.is_empty(){names.len().to_string()}else{field.to_owned()});
            rest=&rest[end+1..];
        }
        parts.push(rest.to_owned());let weight=parts.iter().map(String::len).sum();
        (!names.is_empty()&&weight>=2).then_some(Self{parts,names,target:target.to_owned(),weight})
    }
    fn apply(&self,text:&str)->Option<String> {
        let mut rest=text.strip_prefix(&self.parts[0])?;let mut values=Vec::new();
        for index in 0..self.names.len() {
            let next=&self.parts[index+1];
            let end=if index+1==self.names.len() {rest.len().checked_sub(next.len()).filter(|_|rest.ends_with(next))?}
                else {if next.is_empty(){return None;}rest.find(next)?};
            if rest[..end].contains('\n')&&!self.parts.iter().any(|part|part.contains('\n')){return None;}
            values.push(&rest[..end]);rest=&rest[end+next.len()..];
        }
        if !rest.is_empty(){return None;}
        let mut result=self.target.clone();
        for (name,value) in self.names.iter().zip(values) {result=result.replace(&format!("{{{name}}}"),&translate_en_inner(value,true));}
        Some(result)
    }
}
struct Catalog {exact:HashMap<String,String>,patterns:Vec<Pattern>,prefixes:HashMap<String,Vec<String>>}
static CATALOG:LazyLock<Catalog>=LazyLock::new(||{
    let mut exact:HashMap<String,String>=serde_json::from_str(include_str!("../localization/official-en.json")).expect("Invalid official English catalog");
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/spells-en.json")).expect("Invalid spell English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/en-US.json")).expect("Invalid English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/core-en.json")).expect("Invalid core English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/embers-en.json")).expect("Invalid ember English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/rules-en.json")).expect("Invalid rule English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/ui-en.json")).expect("Invalid UI English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/items-en.json")).expect("Invalid item English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/enemy-aliases-en.json")).expect("Invalid enemy English aliases"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/enemies-en.json")).expect("Invalid enemy English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/descriptions-en.json")).expect("Invalid description English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/talismans-en.json")).expect("Invalid talisman English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/events-en.json")).expect("Invalid event English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/screens-en.json")).expect("Invalid screen English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/lore-en.json")).expect("Invalid lore English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/runtime-en.json")).expect("Invalid runtime English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/native-en.json")).expect("Invalid native English catalog"));
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/debug-en.json")).expect("Invalid debug English catalog"));
    let mut patterns:Vec<_>=exact.iter().filter_map(|(source,target)|Pattern::new(source,target)).collect();
    patterns.sort_by_key(|p|std::cmp::Reverse(p.weight));
    let mut prefixes:HashMap<String,Vec<String>>=HashMap::new();
    for source in exact.keys().filter(|s|s.len()>12&&s.contains('\n')) {
        prefixes.entry(source.split('\n').next().unwrap().to_owned()).or_default().push(source.clone());
    }
    for values in prefixes.values_mut(){values.sort_by_key(|s|std::cmp::Reverse(s.len()));}
    Catalog{exact,patterns,prefixes}
});
thread_local! {static TRANSLATIONS:std::cell::RefCell<HashMap<(String,bool),String>>=std::cell::RefCell::new(HashMap::new());}
fn translate_en_inner(text:&str,patterns:bool)->Cow<'_,str> {
    if !cjk(text){return Cow::Borrowed(text);}
    if let Some(translation)=CATALOG.exact.get(text){return Cow::Borrowed(translation);}
    let key=(text.to_owned(),patterns);
    if let Some(value)=TRANSLATIONS.with(|cache|cache.borrow().get(&key).cloned()){return Cow::Owned(value);}
    let result=translate_en_uncached(text,patterns);
    if text.len()<=8192 {
        TRANSLATIONS.with(|cache|{let mut cache=cache.borrow_mut();if cache.len()>=4096{cache.clear();}cache.insert(key,result.to_string());});
    }
    result
}
fn translate_en_uncached(text:&str,patterns:bool)->Cow<'_,str> {
    if !cjk(text){return Cow::Borrowed(text);}
    if let Some(translation)=CATALOG.exact.get(text){return Cow::Borrowed(translation);}
    if patterns&&(text.contains('。')||text.contains('\n')) {
        for pattern in &CATALOG.patterns {
            if let Some(result)=pattern.apply(text) {if !cjk(&result){return Cow::Owned(result);}}
        }
    }
    let trimmed=text.trim();
    if trimmed.len()!=text.len() {
        let start=text.len()-text.trim_start().len();
        return Cow::Owned(format!("{}{}{}",&text[..start],translate_en_inner(trimmed,patterns),&text[start+trimmed.len()..]));
    }
    if let Some((name,level))=text.rsplit_once(' ') {
        if name.ends_with("之誓")&&matches!(level,"I"|"II"|"III") {
            return Cow::Owned(format!("{} {level}",translate_en_inner(name,patterns)));
        }
    }
    if let Some(candidates)=CATALOG.prefixes.get(text.split('\n').next().unwrap_or(text)) {
        for source in candidates {
            if let Some(rest)=text.strip_prefix(source).filter(|s|s.starts_with('\n')) {
                return Cow::Owned(format!("{}{}",CATALOG.exact[source],translate_en_inner(rest,patterns)));
            }
        }
    }
    if let Some((head,tail))=text.split_once('\n') {
        return Cow::Owned(format!("{}\n{}",translate_en_inner(head,patterns),translate_en_inner(tail,patterns)));
    }
    if let Some((prefix,name))=text.split_once(' ') {
        if let Some(name)=CATALOG.exact.get(name) {
            let translated:Option<Vec<_>>=prefix.split('·').map(|part|CATALOG.exact.get(part)).collect();
            if let Some(prefix)=translated {return Cow::Owned(format!("{} {name}",prefix.iter().map(|s|s.as_str()).collect::<Vec<_>>().join(" · ")));}
        }
    }
    if let Some(end)=text.find('。') {
        let end=end+'。'.len_utf8();
        if !text[end..].trim().is_empty() {
            let tail=&text[end..];let trimmed=tail.trim_start();
            let separator=if tail.len()==trimmed.len(){" "}else{&tail[..tail.len()-trimmed.len()]};
            return Cow::Owned(format!("{}{}{}",translate_en_inner(&text[..end],patterns),separator,translate_en_inner(trimmed,patterns)));
        }
    }
    if text.contains(" → ") {
        return Cow::Owned(text.split(" → ").map(|part|translate_en_inner(part,patterns).into_owned()).collect::<Vec<_>>().join(" → "));
    }
    for marker in [" ★", " ×", "＋", " +"] {
        if let Some((name,suffix))=text.rsplit_once(marker) {
            let valid=!name.is_empty() && match marker {
                " ★"=>suffix.chars().all(|c|c=='★'),
                _=>!suffix.is_empty()&&suffix.chars().all(|c|c.is_ascii_digit()||('０'..='９').contains(&c)),
            };
            if valid {
                let suffix:String=suffix.chars().map(|c|if ('０'..='９').contains(&c){char::from_u32(c as u32-'０' as u32+'0' as u32).unwrap()}else{c}).collect();
                return Cow::Owned(format!("{}{}{}",translate_en_inner(name,patterns),if marker=="＋"{" +"}else{marker},suffix));
            }
        }
    }
    if patterns {for pattern in &CATALOG.patterns {if let Some(result)=pattern.apply(text){return Cow::Owned(result);}}}
    for (separator,joiner) in [("\n","\n"),("·"," · "),("／"," / "),("/","/"),("：",": "),("、",", ")] {
        if text.contains(separator) {
            let pieces:Vec<_>=text.split(separator).map(|p|translate_en_inner(p.trim(),patterns)).collect();
            if pieces.iter().zip(text.split(separator)).any(|(a,b)|a.as_ref()!=b.trim()) {
                return Cow::Owned(pieces.iter().map(|p|p.as_ref()).collect::<Vec<_>>().join(joiner));
            }
        }
    }
    Cow::Borrowed(text)
}
pub fn translate_en(text:&str)->Cow<'_,str> {translate_en_inner(text,true)}
/// Translate presentation text only; never use translated text as a save key or network identifier.
pub fn text(text:&str)->Cow<'_,str> {if english(){translate_en(text)}else if traditional(){to_traditional(text)}else{Cow::Borrowed(text)}}

/// Translate the original description before flattening it for compact UI panels.
/// Catalog keys and format patterns can contain newlines; flattening first loses them.
pub fn inline(text:&str)->String {inline_for(text,english())}
fn inline_for(text:&str,english:bool)->String {
    let translated=if english {translate_en(text)} else {Cow::Borrowed(text)};
    translated.replace('\n',if english {" "} else {"，"})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn traditional_conversion_uses_taiwan_forms_and_keeps_markup() {
        assert_eq!(to_traditional("联机大厅"),"聯機大廳");
        assert_eq!(to_traditional("软件 · 鼠标 · 内存"),"軟體 · 滑鼠 · 記憶體");
        assert_eq!(to_traditional("苦难律法 ##law-1 {0}%"),"苦難律法 ##law-1 {0}%");
        assert_eq!(to_traditional("发现 一发 头发"),"發現 一發 頭髮");
        assert_eq!(to_traditional("ABC 123"),"ABC 123");
        assert!(matches!(to_traditional("English only"),Cow::Borrowed(_)));
        assert_eq!(Language::parse("zh-TW"),Some(Language::TraditionalChinese));
        set(Language::TraditionalChinese);assert_eq!(text("开始旅程"),"開始旅程");assert!(traditional()&&!english());
        set(Language::Chinese);assert_eq!(text("开始旅程"),"开始旅程");
    }
    #[test]
    fn language_aliases_and_format_capture_preserve_values() {
        assert_eq!(Language::parse(" ENGUS "),Some(Language::English));assert_eq!(Language::parse("zh-CN"),Some(Language::Chinese));assert_eq!(Language::parse("unknown"),None);
        assert_eq!(translate_en("第 3 幕"),"Act 3");assert_eq!(translate_en("40 分"),"40 points");
        assert_eq!(translate_en("ABC##stable-id"),"ABC##stable-id");
        assert_eq!(translate_en("未知玩家名字"),"未知玩家名字");
    }
    #[test]
    fn composite_equipment_and_multiline_stats_are_fully_english() {
        assert_eq!(translate_en("防具类型：物理\n防御反击伤害 +8%"),"Armor Type: Physical\nGuard counter damage +8%");
        assert_eq!(translate_en("荆棘的 大树守卫"),"Thorned Tree Sentinel");
        assert_eq!(translate_en("大树守卫的骨灰＋12"),"Tree Sentinel Ashes +12");
        assert_eq!(translate_en("耐力恢复速度 +4"),"Stamina recovery speed +4");
    }
    #[test]
    fn decorated_names_preserve_stars_counts_and_upgrade_arrows() {
        assert_eq!(translate_en("结晶小刀 ★★"), "Crystal Knife ★★");
        assert_eq!(translate_en("混种战士 ×15"), "Misbegotten Warrior ×15");
        assert_eq!(translate_en("红琥珀链坠 → 红琥珀链坠＋２"), "Crimson Amber Medallion → Crimson Amber Medallion +2");
    }
    #[test]
    fn english_descriptions_keep_values_and_original_paragraphs() {
        assert_eq!(translate_en("物理 74　魔力 0　火 0　雷 0　圣 65"),"Physical 74 · Magic 0 · Fire 0 · Lightning 0 · Holy 65");
        assert_eq!(translate_en("可强化至 +39。每次召唤消耗一份骨灰。"),"Can be upgraded to +39. Each summon consumes one copy of the ashes.");
        assert_eq!(translate_en("坚毅的·铁壁的 “战士”荷莱·露"),"Steadfast · Fortified Hoarah Loux, Warrior");
        let source="粗糙的布质上衣。\n在交界地极为常见的防具。\n\n防御反击伤害 +8%";
        let translated=translate_en(source);
        assert!(!cjk(&translated));assert!(translated.contains("\n\nGuard counter damage +8%"));
    }
    #[test]
    fn every_oath_level_translates_the_rendered_title_and_effect() {
        for kind in crate::contracts::Kind::ALL {
            for (level,roman) in [(1,"I"),(2,"II"),(3,"III")] {
                let title=format!("{} {roman}",kind.name());
                assert_eq!(translate_en(&title),format!("{} {roman}",translate_en(kind.name())));
                let boon=kind.boon_at(level);
                assert!(!cjk(&translate_en(&boon)),"{title}: {boon}");
                assert!(!cjk(&translate_en(kind.bane_at())));
            }
        }
        assert_eq!(translate_en("胜利恢复最大生命 15%\n最大专注 20%"),"On victory, recover 15% max HP\nand 20% max FP");
    }
    #[test]
    fn english_catalog_has_no_untranslated_chinese() {
        assert!(CATALOG.exact.iter().all(|(_,value)|!cjk(value)));
    }

    #[test]
    fn inline_effects_translate_before_flattening_and_preserve_chinese() {
        let source=crate::embers::Ember::Bloodlord.effect();
        assert_eq!(inline_for(&source,true),"Each hit inflicts 40 hemorrhage buildup");
        assert_eq!(inline_for(&source,false),"每次命中积累，40 点出血");
        assert_eq!(inline_for("胜利恢复最大生命 15%\n最大专注 20%",true),
            "On victory, recover 15% max HP and 20% max FP");
        assert_eq!(inline_for("未知效果\n未翻译说明",true),"未知效果 未翻译说明");
        assert_eq!(inline_for("",true),"");
    }

    #[test]
    fn every_build_ember_and_oath_has_an_english_inline_description() {
        for ember in crate::embers::Ember::ALL.into_iter().chain([crate::embers::Ember::Stride]) {
            let source=ember.effect();
            let rendered=inline_for(&source,true);
            assert!(!cjk(&rendered),"{ember:?}: {rendered}");
            assert!(!rendered.contains('\n'),"{ember:?}: {rendered}");
            assert_eq!(rendered,translate_en(&source).replace('\n'," "));
            assert_eq!(inline_for(&source,false),source.replace('\n',"，"));
            assert!(!cjk(&translate_en(ember.name())),"{ember:?}");
            let reward=format!("余烬「{}」：{rendered}；本局生效",ember.name());
            assert_eq!(translate_en(&reward),format!("Ember: {} - {rendered}. Lasts for this run.",translate_en(ember.name())));
            for level in [0,1,7,123] {
                if let Some(bonus)=ember.stack_bonus(level) {
                    let name=format!("{} · {level} 层",ember.name());
                    let bonus=format!("当前加成：{bonus}");
                    assert!(!cjk(&translate_en(&name)),"{name}");
                    assert!(!cjk(&translate_en(&bonus)),"{bonus}");
                }
            }
        }
        for kind in crate::contracts::Kind::ALL {
            for level in 0..=3 {
                let (boon,bane)=if level==0 {(kind.boon().to_owned(),kind.bane())} else {(kind.boon_at(level),kind.bane_at())};
                let rendered=inline_for(&boon,true);
                assert!(!cjk(&rendered),"{kind:?} {level}: {rendered}");
                assert!(!rendered.contains('\n'));
                assert_eq!(inline_for(&boon,false),boon.replace('\n',"，"));
                assert!(!cjk(&translate_en(&format!("代价：{bane}"))),"{kind:?}: {bane}");
            }
        }
    }

    #[test]
    fn build_talisman_buffs_and_stat_labels_are_english() {
        let details:HashMap<u32,Vec<&str>>=include_str!("../scene-assets/reward-details.tsv").lines()
            .filter_map(|line| {
                let fields:Vec<_>=line.split('\t').collect();
                Some((fields.first()?.parse().ok()?,fields))
            }).collect();
        for id in crate::elite::BUFFS {
            let fields=&details[&(0x20000000|id)];
            assert!(!cjk(&translate_en(fields[2])),"{}",fields[2]);
            let info=fields[3].replace("\\n","\n");
            let rendered=inline_for(&info,true);
            assert!(!cjk(&rendered),"{id}: {rendered}");
            assert_eq!(inline_for(&info,false),info.replace('\n',"，"));
        }
        for stat in crate::stat_summary::STATS {
            assert!(!cjk(&translate_en(stat.group)),"{}",stat.group);
            assert!(!cjk(&translate_en(stat.label)),"{}",stat.label);
        }
    }
}
