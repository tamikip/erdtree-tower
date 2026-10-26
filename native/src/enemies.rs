use erdtree_tower::NodeKind;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Enemy {
    pub name: &'static str,
    pub model: i32,
    pub npc: i32,
    pub think: u32,
    pub act:u8,
    pub kind:NodeKind,
    pub bar:u32,
}
fn enemy(name: &'static str, model: i32,act:u8,kind:NodeKind) -> Enemy {
    Enemy {
        name,
        model,
        npc: 900000000 + model * 10000,
        think: model as u32 * 10000,
        act,kind,bar:0,
    }
}
static PREVIEW_NAMES:LazyLock<Vec<&'static str>>=LazyLock::new(|| {
    let mut names=Vec::new();
    for name in POOLS.iter().map(|e|e.name).chain(BOSSES.iter().map(|(_,e)|e.name)) {if !names.contains(&name){names.push(name);}}
    names
});
pub fn preview_texts(group:&[Enemy])->Vec<i32> {
    let names=&*PREVIEW_NAMES;
    let mut counts:Vec<(&str,usize)>=Vec::new();
    for enemy in group {
        if let Some((_,count))=counts.iter_mut().find(|(name,_)|*name==enemy.name){*count+=1;}else{counts.push((enemy.name,1));}
    }
    counts.into_iter().map(|(name,count)|9510000+names.iter().position(|n|*n==name).expect("Preview enemy missing") as i32*16+count.min(15) as i32).collect()
}
static POOLS:LazyLock<Vec<Enemy>>=LazyLock::new(||include_str!("../../scene-assets/enemy-pools.txt").lines().map(|line| {
    let p:Vec<_>=line.split('|').collect();
    let kind=match p[1]{"weak"=>NodeKind::Monster,"elite"=>NodeKind::Elite,"boss"=>NodeKind::Boss,_=>panic!("Invalid enemy tier")};
    enemy(p[4],p[2].parse().unwrap(),p[0].parse().unwrap(),kind)
}).collect());
pub fn roster(kind: NodeKind, seed: u64, ordinal: usize, layer: usize,act:u8) -> Vec<Enemy> {
    if kind == NodeKind::Boss { return boss_roster(kind,seed,ordinal,act); }
    if kind == NodeKind::Elite && act!=1 && (seed.wrapping_add(ordinal as u64))%4!=0 {return boss_roster(kind,seed/4,ordinal,act);}
    let seed=if kind==NodeKind::Elite {seed/4}else{seed};
    let stage = layer.min(11) / 3;
    let (strong, weak) = match (kind, stage) {
        (NodeKind::Elite, 0) => (1, 0),
        (NodeKind::Elite, 1) => (1, 1),
        (NodeKind::Elite, 2) => (2, 1),
        (NodeKind::Elite, _) => (2, 2),
        (_, 0) => (0, 2),
        (_, 1) => (0, 3),
        (_, 2) => (1, 2),
        _ => (2, 2),
    };
    (0..strong).map(|i| if kind==NodeKind::Elite&&act==1 {
        let models=[3010,3800,4351,4352,4353,4354,4355,4290,3020,4340,2270,4490];
        select(POOLS.iter().copied().filter(|e|e.act==1&&e.kind==NodeKind::Elite&&models.contains(&e.model)).collect(),seed,ordinal*strong+i)
    } else if kind==NodeKind::Monster {
        select(POOLS.iter().copied().filter(|e|e.act==act&&e.kind==NodeKind::Elite&&e.model!=4290).collect(),seed,ordinal*strong+i)
    } else {choose(NodeKind::Elite, seed, ordinal * strong + i,act)})
        .chain((0..weak).map(|i| choose(NodeKind::Monster, seed, ordinal * weak + i,act))).collect()
}
static BOSSES:LazyLock<Vec<(usize,Enemy)>>=LazyLock::new(||include_str!("../../scene-assets/boss-pool.txt").lines().map(|line| {
    let p:Vec<_>=line.split('|').collect();let index=p[2].parse::<u32>().unwrap();
    (p[1].parse().unwrap(),Enemy{name:p[7],model:p[3].parse().unwrap(),npc:950000000+index as i32*2,think:p[5].parse().unwrap(),act:p[0].parse().unwrap(),kind:NodeKind::Boss,bar:if p[9]=="1" {index+1}else{0}})
}).collect());
static MAP_ENTITIES:LazyLock<Vec<(Enemy,Vec<u32>)>>=LazyLock::new(||{
    let mut variants=std::collections::BTreeMap::new();
    for &spec in POOLS.iter() {variants.insert((spec.act,spec.npc),(spec,4));}
    for &(_,spec) in BOSSES.iter() {
        variants.insert((spec.act,spec.npc),(spec,1));
        let elite=Enemy{npc:spec.npc+1,kind:NodeKind::Elite,bar:0,..spec};
        variants.insert((elite.act,elite.npc),(elite,1));
    }
    let mut offsets=[0u32;4];
    variants.into_values().map(|(spec,copies)|{
        let offset=&mut offsets[usize::from(spec.act)];
        let ids=(0..copies).map(|_|{let id=48000000+u32::from(spec.act)*10000+*offset;*offset+=1;id}).collect();
        (spec,ids)
    }).collect()
});
pub fn map_entities(group:&[Enemy])->Result<Vec<u32>,String> {
    let mut used=std::collections::HashMap::new();
    group.iter().map(|spec|{
        if group.first().is_some_and(|first|first.act!=spec.act){return Err("战斗实体跨幕配置错误".into());}
        let (_,ids)=MAP_ENTITIES.iter().find(|(candidate,_)|
            (candidate.act,candidate.npc,candidate.model,candidate.think)==(spec.act,spec.npc,spec.model,spec.think))
            .ok_or_else(||format!("缺少战斗实体配置：{}",spec.name))?;
        let slot=used.entry(spec.npc).or_insert(0usize);
        let id=*ids.get(*slot).ok_or_else(||format!("战斗实体数量超限：{}",spec.name))?;
        *slot+=1;Ok(id)
    }).collect()
}
pub fn map_controls(entity:u32)->Option<(u32,u32)> {
    let relative=entity.checked_sub(48000000)?;
    let act=relative/10000;let slot=relative%10000;
    if !(1..=3).contains(&act)||slot>=300{return None;}
    let offset=(act-1)*300+slot;
    Some((45008000+offset,45009000+offset))
}
pub const MAP_COMBAT_FLAG:u32=45007998;
fn map_position_flags(entity:u32,slot:usize)->Option<[(u32,bool);4]> {
    map_controls(entity)?;if slot>=12{return None;}
    let base=45004000+((entity-48000000)/10000-1)*10000+(entity%10000)*4;
    Some(std::array::from_fn(|bit|(base+bit as u32,slot&(1<<bit)!=0)))
}
pub fn position_map_entity(entity:u32,slot:usize)->Result<(),String> {
    use fromsoftware_shared::FromStatic;
    let bits=map_position_flags(entity,slot).ok_or("战斗站位编号无效")?;
    let flags=&mut unsafe {eldenring::cs::CSEventFlagMan::instance_mut()}.map_err(|_|"事件系统未就绪")?.virtual_memory_flag;
    if bits.iter().any(|(flag,_)|flags.flag_block_descriptors.find(&(flag/1000)).is_none()){return Err("战斗站位旗标组缺失".into());}
    for (flag,value) in bits {flags.set_flag(flag,value);if flags.get_flag(flag)!=value{return Err("战斗站位写入失败".into());}}
    Ok(())
}
pub fn request_map_entity(entity:u32,enabled:bool)->Result<bool,String> {
    use fromsoftware_shared::FromStatic;
    let (request,ready)=map_controls(entity).ok_or("战斗实体编号无效")?;
    let flags=&mut unsafe {eldenring::cs::CSEventFlagMan::instance_mut()}.map_err(|_|"事件系统未就绪")?.virtual_memory_flag;
    for flag in [request,ready] {
        if flags.flag_block_descriptors.find(&(flag/1000)).is_none(){return Err(format!("战斗旗标组缺失：{flag}"));}
    }
    flags.set_flag(request,enabled);
    if flags.get_flag(request)!=enabled{return Err(format!("战斗激活请求写入失败：{entity}"));}
    Ok(flags.get_flag(ready))
}
static BOSS_LEVELS:LazyLock<std::collections::HashMap<&'static str,u16>>=LazyLock::new(||
    include_str!("../../reference/本体Boss等级与抗性.csv").lines().skip(1).filter(|line|!line.is_empty()).map(|line| {
        let mut fields=line.split(',');
        let name=fields.next().expect("Boss name missing");
        fields.next().expect("Boss region missing");
        let level=fields.next().expect("Boss level missing").parse().expect("Invalid boss level");
        (name,level)
    }).collect());
fn recommended_level(name:&str)->Option<u16> {BOSS_LEVELS.get(name).copied()}
fn boss_eligible(kind:NodeKind,enemy:&Enemy)->bool {
    let minimum=match (kind,enemy.act) {
        (NodeKind::Boss,1)=>35,(NodeKind::Boss,2)=>70,(NodeKind::Boss,3)=>100,
        (NodeKind::Elite,1)=>35,(NodeKind::Elite,2)=>60,(NodeKind::Elite,3)=>90,
        _=>return false,
    };
    recommended_level(enemy.name).is_some_and(|level|level>=minimum)
}
fn boss_roster(kind:NodeKind,seed:u64,ordinal:usize,act:u8)->Vec<Enemy> {
    let mut encounters=Vec::new();
    for (id,_) in BOSSES.iter().filter(|(id,e)|e.act==act&&boss_eligible(kind,e)&&encounter_allowed(kind,*id)) {if !encounters.contains(id){encounters.push(*id);}}
    encounters.sort_by_key(|&id|mix(seed ^ (id as u64).wrapping_mul(0x9e3779b97f4a7c15)));
    let chosen=encounters[ordinal%encounters.len()];
    BOSSES.iter().filter(|(id,_)|*id==chosen).map(|(_,e)|{
        let mut enemy=*e;enemy.kind=kind;if kind==NodeKind::Elite {enemy.npc+=1;enemy.bar=0;}enemy
    }).collect()
}
fn encounter_allowed(kind:NodeKind,id:usize)->bool {
    !BOSSES.iter().any(|(group,e)|*group==id&&e.name=="铃珠猎人")
        &&!BOSSES.iter().any(|(group,e)|*group==id&&matches!(e.model,4570|4580))
        &&!BOSSES.iter().any(|(group,e)|*group==id&&kind==NodeKind::Elite&&e.act==3&&e.model==4911)
        &&!BOSSES.iter().any(|(group,e)|*group==id&&kind==NodeKind::Boss&&e.act==1&&e.name=="死亡仪式鸟")
        &&(kind!=NodeKind::Boss||BOSSES.iter().filter(|(group,_)|*group==id).count()==1)
}
pub fn node_roster(run:&erdtree_tower::Run,id:usize)->Vec<Enemy> {
    let node=&run.nodes[id];
    let ordinal=run.visited.iter().take_while(|&&visited|visited!=id)
        .filter(|&&visited|run.nodes[visited].kind==node.kind).count();
    roster(node.kind,run.seed,ordinal,node.layer*11/(run.floors()-1),run.act)
}
pub fn placement(index: usize, count: usize) -> [f32; 3] {
    if count == 1 { return [0.0, 0.05, 0.0]; }
    let angle = std::f32::consts::TAU * index as f32 / count as f32;
    let radius=if count>4 {6.0}else{2.8};
    [angle.cos() * radius, 0.05, angle.sin() * radius]
}
pub fn facing(from:[f32;3],target:[f32;3])->f32 {
    (from[0]-target[0]).atan2(from[2]-target[2])
}
pub fn group_outcome(outcomes: &[erdtree_tower::encounter::Outcome], dead: bool) -> erdtree_tower::encounter::Outcome {
    use erdtree_tower::encounter::Outcome::*;
    if dead { return PlayerDied; }
    if outcomes.contains(&LostEntity) { return LostEntity; }
    if outcomes.contains(&SpawnTimedOut) { return SpawnTimedOut; }
    if !outcomes.is_empty() && outcomes.iter().all(|o| *o == Victory) { Victory } else { Waiting }
}
pub fn can_spawn_next(networked:bool,previous:&[erdtree_tower::encounter::Outcome])->bool {
    networked||previous.iter().all(|o|*o==erdtree_tower::encounter::Outcome::Victory)
}
pub fn choose(kind: NodeKind, seed: u64, ordinal: usize,act:u8) -> Enemy {
    select(POOLS.iter().copied().filter(|e|e.act==act&&e.kind==kind).collect(),seed,ordinal)
}
fn select(mut pool:Vec<Enemy>,seed:u64,ordinal:usize)->Enemy {
    pool.retain(|e|!matches!(e.model,4570|4580));
    pool.sort_by_key(|e|mix(seed ^ (e.model as u64).wrapping_mul(0x9e3779b97f4a7c15)));
    pool[ordinal % pool.len()]
}

fn mix(mut value:u64)->u64 {
    value=(value^(value>>30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value=(value^(value>>27)).wrapping_mul(0x94d049bb133111eb);
    value^(value>>31)
}

#[cfg(test)]
mod tests {
    #[test]
    fn solo_spawn_waits_for_confirmed_kills_and_party_keeps_group_spawning() {
        use erdtree_tower::encounter::Outcome::*;
        assert!(super::can_spawn_next(false,&[]));
        assert!(super::can_spawn_next(false,&[Victory,Victory]));
        for pending in [Waiting,LostEntity,SpawnTimedOut,PlayerDied] {
            assert!(!super::can_spawn_next(false,&[Victory,pending]));
            assert!(super::can_spawn_next(true,&[Victory,pending]));
        }
        assert_eq!(super::group_outcome(&[Victory,Waiting,Waiting],false),Waiting);
        assert_eq!(super::group_outcome(&[Victory,Victory,Victory],false),Victory);
    }
    #[test]
    fn wormfaces_are_excluded_from_all_encounters() {
        for act in 1..=3 {for kind in [NodeKind::Monster,NodeKind::Elite,NodeKind::Boss] {for seed in 0..300 {
            assert!(roster(kind,seed,seed as usize%40,11,act).iter().all(|e|!matches!(e.model,4570|4580)));
        }}}
    }
    #[test]
    fn ordinary_rooms_exclude_bloodhound_knights_at_every_stage() {
        for act in 1..=3 {for layer in 0..12 {for ordinal in 0..40 {
            for seed in [0,42,999] {
                let group=super::roster(NodeKind::Monster,seed,ordinal,layer,act);
                assert!(group.iter().all(|enemy|enemy.model!=4290));
                assert_eq!(group.len(),match layer/3 {0=>2,1|2=>3,_=>4});
            }
        }}}
        assert!((0..40).any(|n|super::roster(NodeKind::Elite,42,n,0,1).iter().any(|e|e.model==4290)));
    }
    #[test]
    fn preview_groups_duplicates_and_preserves_spawn_names() {
        use super::*;
        let first=POOLS[0];let other=*POOLS.iter().find(|e|e.name!=first.name).unwrap();
        let one=preview_texts(&[first]);
        let repeated=preview_texts(&[first,other,first]);
        assert_eq!(repeated.len(),2);
        assert_eq!(repeated[0],one[0]+1);
        assert_eq!(repeated[1],preview_texts(&[other])[0]);
        assert!(preview_texts(&[]).is_empty());
        for act in 1..=3 {for seed in 0..100 {for kind in [NodeKind::Monster,NodeKind::Elite,NodeKind::Boss] {
            let group=roster(kind,seed,0,11,act);
            assert_eq!(preview_texts(&group).len(),group.iter().map(|e|e.name).collect::<std::collections::HashSet<_>>().len());
        }}}
    }
    use super::*;
    #[test]
    fn map_control_flags_stay_in_verified_groups_and_reject_invalid_ids() {
        let mut used=std::collections::HashSet::new();
        for (_,ids) in MAP_ENTITIES.iter(){for &id in ids {
            let (request,ready)=map_controls(id).unwrap();
            assert_eq!(request/1000,45008);assert_eq!(ready/1000,45009);
            assert!(used.insert(request));assert!(used.insert(ready));
        }}
        for id in [0,48000000,48010300,48020300,48040000,u32::MAX] {assert_eq!(map_controls(id),None);}
    }
    #[test]
    fn formation_flags_encode_each_slot_without_colliding_with_activation() {
        let mut bases=std::collections::HashSet::new();
        for (_,ids) in MAP_ENTITIES.iter(){for &entity in ids {
            for slot in 0..12 {
                let bits=map_position_flags(entity,slot).unwrap();
                assert_eq!(bits.iter().enumerate().filter(|(_,(_,set))|*set).map(|(i,_)|1usize<<i).sum::<usize>(),slot);
                for (flag,_) in bits {assert!([45004,45005,45014,45015,45024,45025].contains(&(flag/1000)));if slot==0{assert!(bases.insert(flag));}}
            }
            assert!(map_position_flags(entity,12).is_none());
        }}
    }
    #[test]
    fn map_entities_assign_distinct_copies_and_reject_unknown_or_exhausted_slots() {
        let spec=choose(NodeKind::Monster,42,0,1);
        let ids=map_entities(&[spec;4]).unwrap();
        assert_eq!(ids.iter().copied().collect::<std::collections::HashSet<_>>().len(),4);
        assert_eq!(ids,map_entities(&[spec;4]).unwrap());
        assert!(map_entities(&[spec;5]).is_err());
        assert!(map_entities(&[Enemy{npc:-1,..spec}]).is_err());
        assert!(map_entities(&[Enemy{think:0,..spec}]).is_err());
        assert!(map_entities(&[spec,Enemy{act:2,..spec}]).is_err());
    }
    #[test]
    fn all_generated_combat_rosters_have_map_entity_slots() {
        for act in 1..=3 {for seed in 0..300 {for kind in [NodeKind::Monster,NodeKind::Elite,NodeKind::Boss] {
            let group=roster(kind,seed,seed as usize%24,seed as usize%12,act);
            let ids=map_entities(&group).unwrap();
            assert_eq!(ids.len(),group.len());
            assert_eq!(ids.iter().copied().collect::<std::collections::HashSet<_>>().len(),group.len());
        }}}
    }
    #[test]
    fn map_entity_manifest_for_scene_crosscheck() {
        for (spec,ids) in MAP_ENTITIES.iter() {for (slot,id) in ids.iter().enumerate() {
            println!("PARTY_SLOT={}|{}|{}|{}|{}|{}",spec.act,spec.npc,spec.model,spec.think,slot,id);
        }}
        assert_eq!(MAP_ENTITIES.iter().map(|(_,ids)|ids.len()).sum::<usize>(),684);
    }
    #[test]
    fn first_act_elites_always_use_curated_combatants_not_boss_labels() {
        let allowed=[3010,3800,4351,4352,4353,4354,4355,4290,3020,4340,2270,4490];
        let mut seen=std::collections::HashSet::new();
        for seed in 0..100 {for ordinal in 0..24 {for layer in 0..12 {
            let group=roster(NodeKind::Elite,seed,ordinal,layer,1);
            assert!(group.iter().any(|e|e.kind==NodeKind::Elite));
            for enemy in group.iter().filter(|e|e.kind==NodeKind::Elite) {
                assert!(allowed.contains(&enemy.model),"Unexpected elite: {}",enemy.name);
                assert_eq!(enemy.npc,900000000+enemy.model*10000);
                seen.insert(enemy.model);
            }
            assert_eq!(group,roster(NodeKind::Elite,seed,ordinal,layer,1));
        }}}
        assert_eq!(seen,allowed.into_iter().collect());
    }
    #[test]
    fn first_act_ordinary_pool_excludes_passive_and_fodder_types() {
        let removed=[4090,3070,3660,3661,4000,4230,4080,4161,4200,4250,4491,4070];
        let pool:Vec<_>=POOLS.iter().filter(|e|e.act==1&&e.kind==NodeKind::Monster).collect();
        assert_eq!(pool.len(),16);
        assert!(pool.iter().all(|e|!removed.contains(&e.model)));
        for model in [4311,3000,3080,3450,3650,4101,4210] {
            assert!(pool.iter().any(|e|e.model==model));
        }
    }
    #[test]
    fn expanded_pools_deal_distinct_early_groups_and_shuffle_across_runs() {
        for (act,weak,elite) in [(1,16,19),(2,18,13),(3,16,13)] {
            assert_eq!(POOLS.iter().filter(|e|e.act==act&&e.kind==NodeKind::Monster).count(),weak);
            assert_eq!(POOLS.iter().filter(|e|e.act==act&&e.kind==NodeKind::Elite).count(),elite);
            for seed in 0..100 {
                let groups:Vec<_>=(0..4).flat_map(|n|roster(NodeKind::Monster,seed,n,0,act)).collect();
                assert_eq!(groups.iter().map(|e|e.model).collect::<std::collections::HashSet<_>>().len(),8);
            }
            let sequences:std::collections::HashSet<_>=(0..100).map(|seed|(0..6).map(|n|choose(NodeKind::Monster,seed*100,n,act).model).collect::<Vec<_>>()).collect();
            assert!(sequences.len()>90);
        }
    }
    #[test]
    fn eligible_encounters_are_reachable_as_boss_and_elite() {
        assert_eq!(BOSSES.len(),140);
        assert_eq!(BOSSES.iter().map(|(id,_)|*id).collect::<std::collections::HashSet<_>>().len(),105);
        for act in 1..=3 {
            for kind in [NodeKind::Boss,NodeKind::Elite] {
                if act==1&&kind==NodeKind::Elite {continue;}
                let expected:std::collections::HashSet<_>=BOSSES.iter().filter(|(id,e)|e.act==act&&boss_eligible(kind,e)&&encounter_allowed(kind,*id)).map(|(_,e)|e.npc).collect();
                let actual:std::collections::HashSet<_>=(0..10000).flat_map(|seed|roster(kind,seed,0,11,act)).filter(|e|e.npc>=950000000).map(|e|e.npc-if kind==NodeKind::Elite {1}else{0}).collect();
                assert_eq!(actual,expected);
            }
            for seed in 0..100 {
                let group=roster(NodeKind::Boss,seed,0,11,act);
                assert!(group.iter().all(|e|!e.name.contains('\u{fffd}')));
                assert!(group.iter().enumerate().all(|(slot,e)|e.bar==0||slot<3));
                assert!(group.iter().any(|e|e.bar>0));
            }
        }
    }
    #[test]
    fn boss_nodes_only_spawn_one_enemy_including_adds() {
        for act in 1..=3 {for seed in 0..500 {for ordinal in 0..24 {
            let group=roster(NodeKind::Boss,seed,ordinal,11,act);
            assert_eq!(group.len(),1,"act={act}, seed={seed}: {group:?}");
            assert!(group[0].bar>0);
        }}}
    }
    #[test]
    fn great_wyrm_is_reserved_for_act_three_boss_rooms() {
        let (id,enemy)=BOSSES.iter().find(|(_,e)|e.act==3&&e.model==4911).unwrap();
        assert!(boss_eligible(NodeKind::Boss,enemy)&&encounter_allowed(NodeKind::Boss,*id));
        assert!(!encounter_allowed(NodeKind::Elite,*id));
        assert!((0..2000).any(|seed|roster(NodeKind::Boss,seed,0,11,3).iter().any(|e|e.model==4911)));
        for kind in [NodeKind::Monster,NodeKind::Elite] {for seed in 0..2000 {
            assert!(roster(kind,seed,0,11,3).iter().all(|e|e.model!=4911));
        }}
    }
    #[test]
    fn bell_bearing_hunter_is_excluded_from_boss_and_elite_encounters() {
        let hunters:Vec<_>=BOSSES.iter().filter(|(_,e)|e.name=="铃珠猎人").collect();
        assert!(!hunters.is_empty());
        for (id,_) in hunters {for kind in [NodeKind::Boss,NodeKind::Elite] {
            assert!(!encounter_allowed(kind,*id));
        }}
        for act in 1..=3 {for kind in [NodeKind::Monster,NodeKind::Elite,NodeKind::Boss] {
            for ordinal in 0..300 {assert!(roster(kind,42,ordinal,11,act).iter().all(|e|e.name!="铃珠猎人"));}
        }}
    }
    #[test]
    fn bosses_respect_reference_levels_and_exclude_unrated_champions() {
        for act in 1..=3 {
            let minimum=[35,70,100][usize::from(act-1)];
            let expected:std::collections::HashSet<_>=BOSSES.iter().filter(|(id,e)|e.act==act&&recommended_level(e.name).is_some_and(|level|level>=minimum)&&encounter_allowed(NodeKind::Boss,*id)).map(|(id,_)|*id).collect();
            assert!(expected.len()>=5);
            let mut seen=std::collections::HashSet::new();
            for seed in 0..2000 {
                let group=roster(NodeKind::Boss,seed,0,11,act);
                for enemy in &group {
                    assert!(recommended_level(enemy.name).is_some_and(|level|level>=minimum));
                    seen.insert(BOSSES.iter().find(|(_,e)|e.npc==enemy.npc).unwrap().0);
                }
            }
            assert_eq!(seen,expected);
        }
        for (_,e) in BOSSES.iter() {
            if recommended_level(e.name).is_none_or(|level|level<35) {
                assert!(!boss_eligible(NodeKind::Boss,e));
                assert!(!boss_eligible(NodeKind::Elite,e));
            }
        }
    }
    #[test]
    fn boss_preview_matches_spawn_and_stays_stable_after_victory() {
        for act in 1..=3 {for seed in 0..100 {
            let mut run=erdtree_tower::Run::new(seed,erdtree_tower::Class::Samurai);
            run.act=act;
            let id=run.nodes.iter().position(|n|n.kind==NodeKind::Boss).unwrap();
            let expected=roster(NodeKind::Boss,seed,0,11,act);
            assert_eq!(node_roster(&run,id),expected);
            run.visited.push(0);
            assert_eq!(node_roster(&run,id),expected);
            run.visited.push(id);
            assert_eq!(node_roster(&run,id),expected);
        }}
    }
    #[test]
    fn native_resistance_elites_are_reachable_in_each_act() {
        for (act, model) in [(1,2270),(2,4470),(3,4770)] {
            let count=POOLS.iter().filter(|e|e.act==act&&e.kind==NodeKind::Elite).count();
            assert!((0..count*100).any(|seed|roster(NodeKind::Elite,seed as u64,0,0,act)[0].model==model));
        }
    }
    #[test]
    fn first_act_boss_varies_across_runs_but_retry_is_stable() {
        let picks:std::collections::HashSet<_>=(0..100).map(|seed|{
            let first=roster(NodeKind::Boss,seed,5,11,1);
            assert_eq!(first,roster(NodeKind::Boss,seed,5,11,1));
            assert!((1..=12).contains(&first.len()));first[0].model
        }).collect();
        assert!(picks.len()>=4);
    }
    #[test]
    fn enemies_face_player_using_native_negative_z_forward() {
        for target in [[0.0,0.0,-10.0],[10.0,0.0,0.0],[-5.0,0.0,8.0]] {
            for count in 1..=4 {for i in 0..count {
                let from=placement(i,count);let yaw=facing(from,target);
                let dx=target[0]-from[0];let dz=target[2]-from[2];let length=dx.hypot(dz);
                assert!((-yaw.sin()-dx/length).abs()<0.0001);
                assert!((-yaw.cos()-dz/length).abs()<0.0001);
            }}
        }
    }
    #[test]
    fn group_requires_every_kill_and_never_rewards_missing_enemies() {
        use erdtree_tower::encounter::Outcome::*;
        assert_eq!(group_outcome(&[Victory, Waiting],false),Waiting);
        assert_eq!(group_outcome(&[Victory, LostEntity],false),LostEntity);
        assert_eq!(group_outcome(&[Victory, SpawnTimedOut],false),SpawnTimedOut);
        assert_eq!(group_outcome(&[Victory, Victory],false),Victory);
        assert_eq!(group_outcome(&[Victory, Victory],true),PlayerDied);
        assert_eq!(group_outcome(&[],false),Waiting);
    }
    #[test]
    fn late_floors_add_strong_enemies_and_groups_stay_bounded() {
        for act in 1..=3 {for seed in 0..100 {
            for kind in [NodeKind::Monster, NodeKind::Elite] {
                let mut last_budget = 0;
                for layer in 0..12 {
                    let group = roster(kind, seed, 3, layer,act);
                    assert!((1..=if kind==NodeKind::Elite {12}else{4}).contains(&group.len()));
                    let budget: usize = group.iter().map(|e| if e.kind==NodeKind::Elite {3} else {1}).sum();
                    assert!(budget >= last_budget);
                    last_budget = budget;
                    assert_eq!(group, roster(kind, seed, 3, layer,act));
                    assert!(group.iter().all(|e|e.act==act));
                }
            }
        }
        }
        assert_eq!(roster(NodeKind::Monster, 1, 0, 0,1).len(), 2);
        assert!(roster(NodeKind::Monster, 1, 0, 8,1).iter().any(|e| e.kind==NodeKind::Elite));
        for count in 1..=4 {
            let positions: Vec<_> = (0..count).map(|i| placement(i, count)).collect();
            assert!(positions.iter().all(|p| p[0].hypot(p[2]) < 3.0));
            for i in 0..count { for j in i+1..count {
                assert!((positions[i][0]-positions[j][0]).hypot(positions[i][2]-positions[j][2]) > 3.0);
            } }
        }
    }
    #[test]
    fn pool_rotates_without_adjacent_duplicates_and_is_reproducible() {
        for act in 1..=3 {for kind in [NodeKind::Monster, NodeKind::Elite,NodeKind::Boss] {
            let count = POOLS.iter().filter(|e|e.act==act&&e.kind==kind&&!matches!(e.model,4570|4580)).count();
            assert!(count>0);
            let picks: Vec<_> = (0..count).map(|n| choose(kind, 42, n,act)).collect();
            for (i, pick) in picks.iter().enumerate() {
                assert_eq!(*pick, choose(kind, 42, i,act));
                assert!(!picks[..i].contains(pick));
                assert_eq!(pick.npc, 900000000 + pick.model * 10000);
            }
        }}
        let mut models=std::collections::HashSet::new();
        assert!(POOLS.iter().all(|e|models.insert(e.model)));
    }
}
