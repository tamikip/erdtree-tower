use std::{borrow::Cow,collections::HashMap,sync::{LazyLock,atomic::{AtomicBool,Ordering}}};

#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub enum Language {#[default] Auto,Chinese,English}
impl Language {
    pub fn id(self)->&'static str {match self {Self::Auto=>"auto",Self::Chinese=>"zh-CN",Self::English=>"en-US"}}
    pub fn parse(value:&str)->Option<Self> {match value.trim().to_ascii_lowercase().as_str(){"auto"|"game"=>Some(Self::Auto),"zh-cn"|"zhocn"|"chinese"=>Some(Self::Chinese),"en-us"|"engus"|"english"|"en"=>Some(Self::English),_=>None}}
}
static ENGLISH:AtomicBool=AtomicBool::new(false);
pub fn set(language:Language){ENGLISH.store(language==Language::English,Ordering::Relaxed);}
pub fn english()->bool {ENGLISH.load(Ordering::Relaxed)}
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
    exact.extend(serde_json::from_str::<HashMap<String,String>>(include_str!("../localization/native-settings-en.json")).expect("Invalid native settings English catalog"));
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
pub fn text(text:&str)->Cow<'_,str> {if english(){translate_en(text)}else{Cow::Borrowed(text)}}

#[cfg(test)]
mod tests {
    use super::*;
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
}
