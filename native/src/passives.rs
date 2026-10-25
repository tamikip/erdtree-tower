use erdtree_tower::{Class,Run,Phase};
use eldenring::cs::{ChrIns,ChrInsExt,PlayerGameData,SoloParamRepository,SpEffectParam};
use fromsoftware_shared::FromStatic;
const FIRST:i32=9800000;

fn starting_gift(class:Class)->Option<(u32,i32)> {
    match class {Class::Vagabond=>Some((10020,1)),Class::Wretch=>Some((8185,5)),_=>None}
}
pub fn give_starting(data:&mut PlayerGameData,class:Class)->Result<(),String> {
    if let Some((id,quantity))=starting_gift(class) {
        let full=0x40000000|id;
        super::items::give(&[(full as i32,quantity,-1)])?;
        if super::loot::count(data,full)!=quantity as u64 {return Err("职业初始物品发放不完整".into());}
    }
    Ok(())
}
fn victory_hp(class:Class,hp:i32,max_hp:i32)->i32 {
    if class!=Class::Warrior || hp<=0 || max_hp<=0 {return hp;}
    (i64::from(hp)+i64::from(max_hp)*8/100).min(i64::from(max_hp)) as i32
}
pub fn heal_victory(chr:&mut ChrIns,class:Class) {
    if chr.chr_flags1c5.death_flag(){return;}
    let data=&mut chr.modules.data;
    data.hp=victory_hp(class,data.hp,data.max_hp);
}
pub fn shop_price(class:Class,base:i32)->i32 {
    if class==Class::Prisoner && base>0 {(i64::from(base)*80/100).max(1) as i32}else{base}
}
fn desired(run:Option<&Run>)->Option<i32> {
    run.filter(|r|!matches!(r.phase,Phase::Dead|Phase::Won)).and_then(|r|match r.class {Class::Prophet=>Some(FIRST),Class::Confessor=>Some(FIRST+1),Class::Hero=>Some(FIRST+2),Class::Bandit=>Some(FIRST+3),Class::Samurai=>Some(FIRST+4),_=>None})
}
pub fn sync(chr:&mut ChrIns,run:Option<&Run>) {
    let Ok(params)=(unsafe {SoloParamRepository::instance()}) else{return;};
    let want=desired(run);
    let present:Vec<_>=chr.special_effect.entries().map(|e|e.param_id).collect();
    for id in FIRST..=FIRST+4 {
        if present.contains(&id)&&want!=Some(id){chr.remove_speffect(id);}
        if want==Some(id)&&!present.contains(&id)&&params.get::<SpEffectParam>(id as u32).is_some(){chr.apply_speffect(id,false);}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn class_gifts_are_exact_and_other_classes_receive_none() {
        assert_eq!(starting_gift(Class::Vagabond),Some((10020,1)));
        assert_eq!(starting_gift(Class::Wretch),Some((8185,5)));
        for class in Class::ALL {if !matches!(class,Class::Vagabond|Class::Wretch){assert_eq!(starting_gift(class),None);}}
    }
    #[test]
    fn warrior_heals_eight_percent_without_overheal_or_reviving() {
        assert_eq!(victory_hp(Class::Warrior,300,1000),380);
        assert_eq!(victory_hp(Class::Warrior,980,1000),1000);
        assert_eq!(victory_hp(Class::Warrior,0,1000),0);
        assert_eq!(victory_hp(Class::Samurai,300,1000),300);
        assert_eq!(victory_hp(Class::Warrior,1,419),34);
    }
    #[test]
    fn prisoner_discount_covers_all_prices_once() {
        for (base,expected) in [(5000,4000),(6000,4800),(3000,2400),(8000,6400),(1,1)] {
            assert_eq!(shop_price(Class::Prisoner,base),expected);
            assert_eq!(shop_price(Class::Confessor,base),base);
        }
    }
    #[test]
    fn added_passives_follow_class_and_end_with_run() {
        for (class,offset) in [(Class::Hero,2),(Class::Bandit,3),(Class::Samurai,4)] {
            let mut run=Run::new(1,class);
            assert_eq!(desired(Some(&run)),Some(FIRST+offset));
            run.phase=Phase::Dead;assert_eq!(desired(Some(&run)),None);
            run.phase=Phase::Won;assert_eq!(desired(Some(&run)),None);
        }
    }
    #[test]
    fn effects_follow_class_and_clear_after_run() {
        let mut run=Run::new(1,Class::Prophet);
        assert_eq!(desired(Some(&run)),Some(FIRST));
        run.class=Class::Confessor;assert_eq!(desired(Some(&run)),Some(FIRST+1));
        run.class=Class::Warrior;assert_eq!(desired(Some(&run)),None);
        run.class=Class::Prophet;run.phase=Phase::Dead;assert_eq!(desired(Some(&run)),None);
        run.phase=Phase::Won;assert_eq!(desired(Some(&run)),None);assert_eq!(desired(None),None);
    }
}
