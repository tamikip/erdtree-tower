use std::time::Instant;
use eldenring::cs::{ChrIns,ChrInsExt,FieldInsHandle,SoloParamRepository,SpEffectParam,WorldChrMan};
use fromsoftware_shared::FromStatic;
use erdtree_tower::{Class,Phase,Run,law::Law};

/// 苦难律法效果：scene-builder law-patch 写入的 9660000 段
pub const FIRST:i32=9660000;
const OVERTIME:i32=FIRST+50;
const PLAYER:[i32;10]=[FIRST+11,FIRST+12,FIRST+13,FIRST+21,FIRST+22,FIRST+31,FIRST+32,FIRST+41,FIRST+42,OVERTIME];
fn ready(ids:&[i32])->bool {unsafe {SoloParamRepository::instance()}.is_ok_and(|p|ids.iter().all(|id|p.get::<SpEffectParam>(*id as u32).is_some()))}
fn player_effects(law:Law,overtime:bool)->Vec<i32> {
    let mut ids=Vec::new();
    for (level,base) in [(law.fragile(),10),(law.fatigue(),20),(law.focus(),30),(law.poverty(),40)] {if level>0 {ids.push(FIRST+base+i32::from(level));}}
    if overtime {ids.push(OVERTIME);}
    ids
}
fn enemy_effects(law:Law)->Vec<i32> {
    let mut ids=Vec::new();
    if law.enemy_level()>0 {ids.push(FIRST+i32::from(law.enemy_level()));}
    if law.toughness()>0 {ids.push(FIRST+5+i32::from(law.toughness()));}
    if law.elemental_resistance()>0 {ids.push(FIRST+60+i32::from(law.elemental_resistance()));}
    if law.status_resistance()>0 {ids.push(FIRST+70+i32::from(law.status_resistance()));}
    ids
}
pub fn lone_wolf(run:Option<&Run>)->bool {run.is_some_and(|r|r.law.lone_wolf())}
pub fn shop_price(law:Law,price:i32)->i32 {
    if price<=0||law.price_percent()==100 {return price;}
    (i64::from(price)*i64::from(law.price_percent())/100).min(i64::from(i32::MAX)) as i32
}
/// 玩家身上的律法效果：只在进行中的旅程里保留，超时失血只在限时战斗超时后施加
pub fn sync(chr:&mut ChrIns,run:Option<&Run>,overtime:bool) {
    let want=run.filter(|r|!matches!(r.phase,Phase::Dead|Phase::Won)).map_or_else(Vec::new,|r|player_effects(r.law,overtime));
    let present:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).collect();
    for id in PLAYER {if present.contains(&id)&&!want.contains(&id){chr.remove_speffect(id);}}
    if want.is_empty()||!ready(&want) {return;}
    for id in want {if !present.contains(&id){chr.apply_speffect(id,false);}}
}
/// 一场战斗内的律法：给每个刷出的敌人挂一次强化，限时计时，嫉妒的世界在玩家喝药时回复敌人
#[derive(Default)]
pub struct Fight {key:Option<(u64,u8,usize)>,applied:Vec<FieldInsHandle>,started:Option<Instant>,flasks:Option<u8>,limit:Option<u64>,missing:bool,drinks:erdtree_tower::party_law::DrinkDeltas}
impl Fight {
    pub fn tick(&mut self,world:&mut WorldChrMan,key:(u64,u8,usize),law:Law,handles:&[FieldInsHandle],authority:bool,flasks:Option<u8>,team:Option<&[(u64,u64)]>) {
        if self.key!=Some(key) {
            *self=Self{key:Some(key),started:Some(Instant::now()),limit:law.time_limit(),..Default::default()};
            if !law.is_empty() {super::log(format!("TOWER_LAW node={} act={} code={} score={}",key.2,key.1,law.code(),law.score()));}
        }
        let drank=team.map_or_else(||match (self.flasks,flasks){(Some(before),Some(after))=>u64::from(before.saturating_sub(after)),_=>0},|counts|self.drinks.take(counts));self.flasks=flasks;
        if law.is_empty()||!authority {return;}
        let effects=enemy_effects(law);
        if !ready(&effects) {if !self.missing{super::log("TOWER_LAW missing law effects; update regulation.bin");self.missing=true;}return;}
        for handle in handles {
            let Some(chr)=world.chr_ins_by_handle_mut(handle) else{continue};
            if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(){continue;}
            if !self.applied.contains(handle) {for id in &effects {chr.apply_speffect(*id,false);}self.applied.push(*handle);}
            if drank>0&&law.envy_world() {
                let data=&mut chr.modules.data;
                data.hp=(i64::from(data.hp)+i64::from(data.max_hp.max(0))/10*drank.min(32) as i64).min(i64::from(data.max_hp)) as i32;
            }
        }
        if drank>0&&law.envy_world() {super::log(format!("TOWER_LAW envy world healed enemies: flask_uses={drank}"));}
    }
    /// 限时战斗剩余秒数；超时后为负数
    pub fn clock(&self)->Option<i64> {
        let (started,limit)=(self.started?,self.limit?);
        Some(limit as i64-started.elapsed().as_secs() as i64)
    }
    pub fn overtime(&self)->bool {self.clock().is_some_and(|left|left<=0)}
}
/// 按职业记录的苦难律法最高分（只记登顶的旅程）
fn best_path(owner:(u32,i32))->std::path::PathBuf {super::hub::path(owner,"law-best")}
fn parse_best(text:&str)->Vec<(Class,u32)> {
    text.lines().filter_map(|line|{let (class,score)=line.split_once('|')?;Some((Class::PLAYABLE.into_iter().find(|c|format!("{c:?}")==class)?,score.trim().parse().ok()?))}).collect()
}
fn merge_best(mut records:Vec<(Class,u32)>,class:Class,score:u32)->Option<Vec<(Class,u32)>> {
    match records.iter_mut().find(|(c,_)|*c==class) {
        Some((_,best)) if *best>=score=>return None,
        Some((_,best))=>*best=score,
        None=>records.push((class,score)),
    }
    Some(records)
}
pub fn best(owner:(u32,i32),class:Class)->Option<u32> {
    parse_best(&std::fs::read_to_string(best_path(owner)).ok()?).into_iter().find(|(c,_)|*c==class).map(|(_,s)|s)
}
pub fn record(owner:(u32,i32),run:&Run)->Result<(),String> {
    if run.phase!=Phase::Won||run.law.is_empty()||run.debug {return Ok(());}
    let path=best_path(owner);
    let Some(records)=merge_best(parse_best(&std::fs::read_to_string(&path).unwrap_or_default()),run.class,run.law.score()) else{return Ok(());};
    let text:String=records.iter().map(|(c,s)|format!("{c:?}|{s}\n")).collect();
    let tmp=path.with_extension("tmp");
    std::fs::write(&tmp,text).and_then(|_|std::fs::rename(&tmp,&path)).map_err(|e|format!("苦难律法纪录写入失败：{e}"))?;
    super::log(format!("TOWER_LAW new best class={:?} score={}",run.class,run.law.score()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn law(ids:&[usize])->Law {ids.iter().fold(Law::default(),|l,&id|l.toggle(id))}
    #[test]
    fn effects_match_the_patched_rows() {
        assert!(player_effects(Law::default(),false).is_empty()&&enemy_effects(Law::default()).is_empty());
        assert_eq!(player_effects(law(&[11,12,13,15,17,18,19]),true),vec![FIRST+13,FIRST+21,FIRST+32,FIRST+41,OVERTIME]);
        assert_eq!(enemy_effects(law(&[0,1,2,6])),vec![FIRST+3,FIRST+6]);
        assert_eq!(enemy_effects(law(&[27,29])),vec![FIRST+61,FIRST+71]);
        assert_eq!(enemy_effects(law(&[27,28,29,30])),vec![FIRST+62,FIRST+72]);
        for id in player_effects(law(&(0..erdtree_tower::law::NODES.len()).collect::<Vec<_>>()),true) {assert!(PLAYER.contains(&id));}
        assert_eq!((shop_price(law(&[21]),1000),shop_price(law(&[22]),1000),shop_price(Law::default(),1000),shop_price(law(&[21]),-1)),(1200,1400,1000,-1));
    }
    #[test]
    fn best_scores_only_rise_per_class() {
        let records=merge_best(vec![],Class::Samurai,120).unwrap();
        assert!(merge_best(records.clone(),Class::Samurai,100).is_none());
        let records=merge_best(records,Class::Hero,40).unwrap();let records=merge_best(records,Class::Samurai,200).unwrap();
        let text:String=records.iter().map(|(c,s)|format!("{c:?}|{s}\n")).collect();
        assert_eq!(parse_best(&text),vec![(Class::Samurai,200),(Class::Hero,40)]);
        assert!(parse_best("garbage\nNobody|5\n").is_empty());
    }
}
