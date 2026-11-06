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
    /// 停用：保留名称编号与联机实体，但不再抽到
    pub retired:bool,
    /// DLC 敌人：在原有联机实体之后追加槽位
    pub dlc:bool,
    /// 仅单人：地图里没有预置实体（boss-pool.txt 第 11 列为 solo 或 trial），不分配联机槽位，联机时抽不到
    pub solo:bool,
    /// 后追加的实体与名称，不参与旧槽位排序。
    pub appended:bool,
}
/// 联机时只能抽到地图里预先摆放过的敌人；由运行时每帧同步
static PARTY:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
pub fn set_party(active:bool) {PARTY.store(active,std::sync::atomic::Ordering::Relaxed);}
fn party()->bool {PARTY.load(std::sync::atomic::Ordering::Relaxed)}
/// 停用的不抽；仅单人的（地图里没有联机实体）联机时不抽；没有 DLC 时不抽 DLC 敌人
fn selectable(e:&Enemy,party:bool)->bool {!e.retired&&!(party&&e.solo)&&(!e.dlc||erdtree_tower::dlc::enemies_allowed())}
/// Boss 与精英版 Boss 的参数编号段（BossAssets.Npc）；DLC 怪物池的编号也在 950000000 之上，不能只比大小
pub fn boss_npc(npc:i32)->bool {(950000000..950100000).contains(&npc)}
fn enemy(name: &'static str, model: i32,act:u8,kind:NodeKind) -> Enemy {
    Enemy {
        name,
        model,
        npc: 900000000 + model * 10000,
        think: model as u32 * 10000,
        act,kind,bar:0,retired:false,dlc:false,solo:false,appended:false,
    }
}
/// 预览名称追加在普通怪之后的 Boss 序号起点（DLC 的融泥骑士、黄金河马、贝勒、拉塔恩、梅瑟莫）
const LATE_BOSS_FIRST:i32=153;
static PREVIEW_NAMES:LazyLock<Vec<&'static str>>=LazyLock::new(|| {
    let mut names=Vec::new();
    // 追加顺序固定：旧敌人、旧 Boss、DLC 敌人、新 Boss、新普通怪、再之后追加的 Boss（序号 153 起），保持名称编号不变。
    let late=|e:&Enemy|e.npc>=950000000+LATE_BOSS_FIRST*2;
    for name in POOLS.iter().filter(|e|!e.dlc&&!e.appended).map(|e|e.name).chain(BOSSES.iter().filter(|(_,e)|!e.appended).map(|(_,e)|e.name))
        .chain(POOLS.iter().filter(|e|e.dlc).map(|e|e.name)).chain(BOSSES.iter().filter(|(_,e)|e.appended&&!late(e)).map(|(_,e)|e.name))
        .chain(POOLS.iter().filter(|e|e.appended).map(|e|e.name)).chain(BOSSES.iter().filter(|(_,e)|late(e)).map(|(_,e)|e.name)) {if !names.contains(&name){names.push(name);}}
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
    // party 为已补齐联机实体的追加敌人；solo 保留用于未移植敌人。DLC 与追加敌人的停用标记在第 7 列（保留联机实体编号）。
    let flag=p.get(5).copied().unwrap_or("");let extra=p.get(6).copied().unwrap_or("");
    assert!(matches!(flag,""|"retired"|"dlc"|"solo"|"party")&&matches!(extra,"")||(matches!(flag,"dlc"|"party")&&extra=="retired"),"Invalid enemy pool flag");
    Enemy{retired:flag=="retired"||extra=="retired",dlc:flag=="dlc",solo:flag=="solo",appended:matches!(flag,"solo"|"party"),..enemy(p[4],p[2].parse().unwrap(),p[0].parse().unwrap(),kind)}
}).collect());
pub const POOL_NAME_FIRST:i32=9610000;
pub const BOSS_NAME_FIRST:i32=9600000;
static POOL_NAMES:LazyLock<Vec<&'static str>>=LazyLock::new(||{let mut names=Vec::new();for e in POOLS.iter(){if !names.contains(&e.name){names.push(e.name);}}names});
pub fn pool_name_count()->usize {POOL_NAMES.len()}
pub fn boss_name_count()->usize {BOSSES.len()}
pub fn bar_text(spec:&Enemy)->Option<i32> {
    if spec.bar>0 {return Some(BOSS_NAME_FIRST+spec.bar as i32-1);}
    if boss_npc(spec.npc) {return Some(BOSS_NAME_FIRST+(spec.npc-950000000)/2);}
    POOL_NAMES.iter().position(|n|*n==spec.name).map(|i|POOL_NAME_FIRST+i as i32)
}
pub fn bar_rank(spec:&Enemy)->u8 {if spec.bar>0 {0}else if boss_npc(spec.npc)||spec.kind==NodeKind::Boss {1}else if spec.kind==NodeKind::Elite {2}else{3}}
/// 只在精英战出现、不进普通房“强怪”位置的精英：猎犬骑士、咒剑士（DLC）
const NOT_IN_ORDINARY_ROOMS:[i32;2]=[4290,5040];
pub fn roster(kind: NodeKind, seed: u64, ordinal: usize, layer: usize,act:u8) -> Vec<Enemy> {roster_in(kind,seed,ordinal,layer,act,party())}
#[cfg(test)]
pub(super) fn party_roster_for_test(kind:NodeKind,seed:u64,ordinal:usize,layer:usize,act:u8)->Vec<Enemy>{roster_in(kind,seed,ordinal,layer,act,true)}
fn roster_in(kind: NodeKind, seed: u64, ordinal: usize, layer: usize,act:u8,party:bool) -> Vec<Enemy> {
    if kind == NodeKind::Boss { return boss_roster(kind,seed,ordinal,act,party); }
    if kind == NodeKind::Elite && act!=1 && (seed.wrapping_add(ordinal as u64))%4!=0 {return boss_roster(kind,seed/4,ordinal,act,party);}
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
        let models=[3010,3800,4351,4352,4353,4354,4355,4290,4340,2270,4490];
        select(POOLS.iter().copied().filter(|e|e.act==1&&e.kind==NodeKind::Elite&&(models.contains(&e.model)||e.dlc))
            .chain(BOSSES.iter().filter(|(_,e)|e.act==1&&elite_only(e)).map(|(_,e)|Enemy{npc:e.npc+1,kind:NodeKind::Elite,bar:0,..*e})).collect(),seed,ordinal*strong+i,party)
    } else if kind==NodeKind::Monster {
        select(POOLS.iter().copied().filter(|e|e.act==act&&e.kind==NodeKind::Elite&&!NOT_IN_ORDINARY_ROOMS.contains(&e.model)).collect(),seed,ordinal*strong+i,party)
    } else {choose_in(NodeKind::Elite, seed, ordinal * strong + i,act,party)})
        .chain((0..weak).map(|i| choose_in(NodeKind::Monster, seed, ordinal * weak + i,act,party))).collect()
}
/// Boss 第 11 列：party 为追加的多人 Boss；solo 仅单人；trial 仅调试。
static BOSSES:LazyLock<Vec<(usize,Enemy)>>=LazyLock::new(||include_str!("../../scene-assets/boss-pool.txt").lines().map(|line| {
    let p:Vec<_>=line.split('|').collect();let index=p[2].parse::<u32>().unwrap();
    let flag=p.get(10).copied().unwrap_or("");
    assert!(matches!(flag,""|"solo"|"trial"|"party"),"Invalid boss pool flag");
    (p[1].parse().unwrap(),Enemy{name:p[7],model:p[3].parse().unwrap(),npc:950000000+index as i32*2,think:p[5].parse().unwrap(),act:p[0].parse().unwrap(),kind:NodeKind::Boss,
        retired:flag=="trial",dlc:false,solo:matches!(flag,"solo"|"trial"),appended:!flag.is_empty(),bar:if p[9]=="1" {index+1}else{0}})
}).collect());
static MAP_ENTITIES:LazyLock<Vec<(Enemy,Vec<u32>)>>=LazyLock::new(||{
    let mut variants=std::collections::BTreeMap::new();
    for &spec in POOLS.iter().filter(|e|!e.dlc&&!e.appended) {variants.insert((spec.act,spec.npc),(spec,4));}
    for &(_,spec) in BOSSES.iter().filter(|(_,e)|!e.appended) {
        variants.insert((spec.act,spec.npc),(spec,1));
        let elite=Enemy{npc:spec.npc+1,kind:NodeKind::Elite,bar:0,..spec};
        variants.insert((elite.act,elite.npc),(elite,1));
    }
    let mut offsets=[0u32;4];
    let mut entries:Vec<(Enemy,Vec<u32>)>=variants.into_values().map(|(spec,copies)|{
        let offset=&mut offsets[usize::from(spec.act)];
        let ids=(0..copies).map(|_|{let id=48000000+u32::from(spec.act)*10000+*offset;*offset+=1;id}).collect();
        (spec,ids)
    }).collect();
    for line in include_str!("../../scene-assets/party-dlc-slots.tsv").lines().chain(include_str!("../../scene-assets/party-extra-slots.tsv").lines()) {
        let p:Vec<u32>=line.split('|').map(|v|v.parse().unwrap()).collect();
        let spec=POOLS.iter().find(|e|e.act as u32==p[0]&&e.npc as u32==p[1]).copied().unwrap_or_else(|| {
            let source=BOSSES.iter().find(|(_,e)|e.npc as u32==p[1]||e.npc as u32+1==p[1]).expect("Missing appended boss").1;
            let elite=p[1]==source.npc as u32+1;
            Enemy{act:p[0] as u8,npc:p[1] as i32,kind:if elite{NodeKind::Elite}else{NodeKind::Boss},bar:if elite{0}else{source.bar},..source}
        });
        assert_eq!((spec.model as u32,spec.think),(p[2],p[3]));
        if let Some((_,ids))=entries.iter_mut().find(|(e,_)|e.act==spec.act&&e.npc==spec.npc) {assert_eq!(ids.len(),p[4] as usize);ids.push(p[5]);}
        else {assert_eq!(p[4],0);entries.push((spec,vec![p[5]]));}
    }
    entries
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
/// 挪到后一幕、只作为精英出现的组合（单人）：英雄石像鬼双人组从第二幕移到第三幕。地图里没有第三幕的联机实体，联机时不抽这一组
const RELOCATED_ELITES:[(i32,u8);2]=[(950000152,3),(950000154,3)];
/// 精英版只出第一只的组合（组里第一只的 npc）：英雄石像鬼双人组的精英版只出大剑那一只
const SINGLE_ELITES:[(i32,&str);1]=[(950000152,"英雄石像鬼")];
fn relocated(enemy:&Enemy)->Option<u8> {RELOCATED_ELITES.iter().find(|(npc,_)|*npc==enemy.npc).map(|(_,act)|*act)}
fn encounter_act(enemy:&Enemy)->u8 {if enemy.npc==950000196 {3}else{relocated(enemy).unwrap_or(enemy.act)}}
fn elite_only(enemy:&Enemy)->bool {
    include_str!("../../scene-assets/elite-only-bosses.txt").lines().any(|line|line.parse::<i32>().is_ok_and(|index|enemy.npc==950000000+index*2))
}
fn boss_eligible(kind:NodeKind,enemy:&Enemy)->bool {
    if elite_only(enemy) {return kind==NodeKind::Elite;}
    if enemy.appended&&!enemy.solo&&kind!=NodeKind::Boss {return false;}
    if enemy.npc==950000196||relocated(enemy).is_some() {return kind==NodeKind::Elite;}
    let minimum=match (kind,enemy.act) {
        (NodeKind::Boss,1)=>35,(NodeKind::Boss,2)=>70,(NodeKind::Boss,3)=>100,
        (NodeKind::Elite,1)=>35,(NodeKind::Elite,2)=>60,(NodeKind::Elite,3)=>90,
        _=>return false,
    };
    recommended_level(enemy.name).is_some_and(|level|level>=minimum)
}
/// 试验中的 Boss 不会被抽到；仅单人的 Boss 联机时抽不到
fn boss_roster(kind:NodeKind,seed:u64,ordinal:usize,act:u8,party:bool)->Vec<Enemy> {
    let mut encounters=Vec::new();
    for (id,_) in BOSSES.iter().filter(|(id,e)|!e.retired&&!(party&&(e.solo||relocated(e).is_some()||PARTY_UNSYNCED.contains(&e.name)))&&encounter_act(e)==act&&boss_eligible(kind,e)&&encounter_allowed(kind,*id)) {if !encounters.contains(id){encounters.push(*id);}}
    // 第三幕「双王共命」：每个双王组合是第三幕 Boss 战的一个选项，与每个单只 Boss 机会相同；组合里的 Boss 不再单独出现（机制见 dual_bosses.rs）
    let mut options:Vec<Vec<usize>>=Vec::new();
    let pairs:Vec<[usize;2]>=if party&&dual(kind,act) {DUAL_PAIRS.iter().filter_map(|pair|{
        let ids=pair.map(|name|encounters.iter().copied().find(|id|BOSSES.iter().any(|(g,e)|g==id&&e.name==name)));
        Some([ids[0]?,ids[1]?])
    }).collect()}else{Vec::new()};
    for &id in &encounters {if !pairs.iter().any(|p|p.contains(&id)) {options.push(vec![id]);}}
    for pair in &pairs {options.push(pair.to_vec());}
    options.sort_by_key(|o|mix(seed ^ (o.iter().fold(0u64,|k,&id|k.wrapping_mul(0x100000001b3)^id as u64)).wrapping_mul(0x9e3779b97f4a7c15)));
    let chosen=&options[ordinal%options.len()];
    let mut group:Vec<Enemy>=chosen.iter().flat_map(|chosen|BOSSES.iter().filter(move |(id,_)|id==chosen)).map(|(_,e)|*e).collect();
    if kind==NodeKind::Elite && let Some(&(_,name))=group.first().and_then(|first|SINGLE_ELITES.iter().find(|(npc,_)|*npc==first.npc)) {
        group.truncate(1);group[0].name=name;
    }
    group.into_iter().map(|mut enemy|{
        enemy.act=act;enemy.kind=kind;if kind==NodeKind::Elite {enemy.npc+=1;enemy.bar=0;}enemy
    }).collect()
}
/// Windows party-boss-model-sync 已验证模型与 AI；当前无待同步的联机 Boss。
const PARTY_UNSYNCED:[&str;0]=[];
/// 第三幕双王共命的组合（Boss 名）。原来唯一的一组卡利亚双骑（“圣树骑士”罗蕾塔 + “双月骑士”蕾拉娜）随罗蕾塔移出第三幕一起停用，蕾拉娜改为单独出场；机制（dual_bosses.rs）保留
pub const DUAL_PAIRS:[[&str;2];0]=[];
/// 这一幕的 Boss 战可能是双王
pub fn dual(kind:NodeKind,act:u8)->bool {kind==NodeKind::Boss&&act==3}
/// 能当 Boss 的组合只当 Boss：同一幕的精英战不再抽到它们的精英版
fn boss_capable(id:usize)->bool {
    BOSSES.iter().any(|(group,e)|*group==id&&!e.retired&&boss_eligible(NodeKind::Boss,e))&&encounter_allowed(NodeKind::Boss,id)
}
/// 这一幕的怪物池里已有同名的精英或小怪（例如第一幕的猎犬骑士、第二幕的鲜血贵族）：不再作为这一幕的 Boss
fn pooled(enemy:&Enemy)->bool {POOLS.iter().any(|p|p.act==enemy.act&&!p.retired&&p.kind!=NodeKind::Boss&&p.name==enemy.name)}
fn encounter_allowed(kind:NodeKind,id:usize)->bool {
    if kind==NodeKind::Elite&&boss_capable(id) {return false;}
    if kind==NodeKind::Boss&&BOSSES.iter().any(|(group,e)|*group==id&&pooled(e)) {return false;}
    // 铃珠猎人、艾斯缇（黑暗弃子）不进 Boss 与精英战；数据行与联机实体保留，名称编号不变
    !BOSSES.iter().any(|(group,e)|*group==id&&matches!(e.name,"铃珠猎人"|"艾斯缇"))
        &&!BOSSES.iter().any(|(group,e)|*group==id&&matches!(e.model,4570|4580|4050|4602|4480))
        &&!BOSSES.iter().any(|(group,e)|*group==id&&kind==NodeKind::Elite&&e.act==3&&e.model==4911)
        &&!BOSSES.iter().any(|(group,e)|*group==id&&kind==NodeKind::Boss&&e.act==1&&e.name=="死亡仪式鸟")
        // 第三幕不再出现“圣树骑士”罗蕾塔与混种圣骑士（Boss 与精英版都不抽）
        &&!BOSSES.iter().any(|(group,e)|*group==id&&e.act==3&&matches!(e.name,"“圣树骑士”罗蕾塔"|"混种圣骑士"))
        &&(kind!=NodeKind::Boss||BOSSES.iter().filter(|(group,_)|*group==id).count()==1)
}
/// 调试菜单：所有还会抽到的怪物池敌人
pub fn debug_pool()->Vec<Enemy> {POOLS.iter().copied().filter(|e|!e.retired).collect()}
/// 调试菜单：每组 Boss 战（同一组的所有部件）
pub fn debug_encounters()->Vec<Vec<Enemy>> {
    let mut groups:Vec<(usize,Vec<Enemy>)>=Vec::new();
    for &(id,spec) in BOSSES.iter() {
        match groups.iter_mut().find(|(g,_)|*g==id) {Some((_,parts))=>parts.push(spec),None=>groups.push((id,vec![spec]))}
    }
    let mut result:Vec<Vec<Enemy>>=groups.into_iter().map(|(_,parts)|parts).collect();
    for pair in DUAL_PAIRS {
        let parts:Option<Vec<_>>=pair.into_iter().map(|name|BOSSES.iter().find(|(_,e)|e.name==name&&encounter_act(e)==3).map(|(_,e)|{let mut spec=*e;spec.act=3;spec.kind=NodeKind::Boss;spec})).collect();
        if let Some(parts)=parts {if !result.contains(&parts){result.push(parts);}}
    }
    result
}
pub fn party_only_group(group:&[Enemy])->bool {
    group.len()==2&&DUAL_PAIRS.iter().any(|pair|pair.iter().all(|name|group.iter().any(|e|e.name==*name)))
}
pub fn debug_group_name(group:&[Enemy])->String {
    let mut names=Vec::new();for e in group {if !names.contains(&e.name){names.push(e.name);}}
    format!("{}{}",if party_only_group(group){"卡利亚双骑·双Boss·双王共命·联机独享｜"}else{""},names.join("＋"))
}
pub fn node_roster(run:&erdtree_tower::Run,id:usize)->Vec<Enemy> {
    let node=&run.nodes[id];
    if run.debug&&node.kind==NodeKind::Boss&&let Some(test)=run.boss_test.filter(|t|t.act==run.act) {
        let group=debug_encounters().get(usize::from(test.encounter)).cloned().unwrap_or_default();
        if party()||!party_only_group(&group){return group;}
    }
    let ordinal=run.visited.iter().take_while(|&&visited|visited!=id)
        .filter(|&&visited|run.nodes[visited].kind==node.kind).count();
    roster(node.kind,run.seed,ordinal,node.layer*11/(run.floors()-1),run.act)
}
pub fn boss_test_roster(encounter:u16,party:bool,dlc:bool)->Result<Vec<Enemy>,String> {
    let group=debug_encounters().get(usize::from(encounter)).cloned().ok_or("Boss 测试编号不存在")?;
    boss_test_allowed(&group,party,dlc)?;Ok(group)
}
pub fn boss_test_allowed(group:&[Enemy],party:bool,dlc:bool)->Result<(),String> {
    if group.is_empty(){return Err("Boss 组合为空".into());}
    if !party&&party_only_group(group){return Err("该双 Boss 组合仅限联机".into());}
    if group.iter().any(|e|e.dlc&&!dlc){return Err("需要启用 DLC 敌人资源".into());}
    if party&&(group.iter().any(|e|e.solo||relocated(e).is_some())||map_entities(&group).is_err()) {
        return Err("该组合缺少联机预置槽位，仅限单人测试".into());
    }
    Ok(())
}
pub fn placement(index: usize, count: usize) -> [f32; 3] {
    if count == 1 { return [0.0, 0.05, 0.0]; }
    let angle = std::f32::consts::TAU * index as f32 / count as f32;
    let radius=if count>4 {6.0}else{2.8};
    [angle.cos() * radius, 0.05, angle.sin() * radius]
}
/// 双王：左右各 8 米
pub fn dual_placement(index:usize)->[f32;3] {[if index==0 {-8.0}else{8.0},0.05,0.0]}
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
pub fn role(spec:&Enemy)->erdtree_tower::encounter::Role {
    use erdtree_tower::encounter::Role;
    if spec.kind==NodeKind::Elite {Role::Strong}else if spec.kind==NodeKind::Boss||boss_npc(spec.npc) {Role::Boss}else{Role::Weak}
}
#[cfg(test)]
pub fn choose(kind: NodeKind, seed: u64, ordinal: usize,act:u8) -> Enemy {choose_in(kind,seed,ordinal,act,party())}
fn choose_in(kind: NodeKind, seed: u64, ordinal: usize,act:u8,party:bool) -> Enemy {
    select(POOLS.iter().copied().filter(|e|e.act==act&&e.kind==kind).collect(),seed,ordinal,party)
}
fn select(mut pool:Vec<Enemy>,seed:u64,ordinal:usize,party:bool)->Enemy {
    pool.retain(|e|selectable(e,party));
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
    fn debug_catalog_includes_dynamic_knight_pair_with_party_slots() {
        use super::*;
        for pair in DUAL_PAIRS {
            let groups=debug_encounters();
            let index=groups.iter().position(|g|g.len()==2&&g.iter().map(|e|e.name).eq(pair)).expect("dynamic boss pair must be listed");
            let group=boss_test_roster(index as u16,true,true).unwrap();
            assert!(group.iter().all(|e|e.act==3&&e.kind==NodeKind::Boss));
            let ids=map_entities(&group).unwrap();assert_eq!(ids.len(),2);assert_ne!(ids[0],ids[1]);
            let mut run=erdtree_tower::Run::new(91,erdtree_tower::Class::Samurai);
            assert!(boss_test_roster(index as u16,false,true).is_err());
            let node=run.debug_boss_test(index as u16,3).unwrap();assert_eq!(node_roster(&run,node).len(),1,"old party test saves cannot spawn the pair in solo");
            for name in pair {assert!((0..200).any(|ordinal|boss_roster(NodeKind::Boss,91,ordinal,3,false).iter().any(|e|e.name==name)),"solo keeps {name}");}
            assert!((0..200).any(|ordinal|party_only_group(&boss_roster(NodeKind::Boss,91,ordinal,3,true))),"party keeps the pair");
        }
    }
    #[test]
    fn boss_test_catalog_preserves_every_group_and_checks_party_slots() {
        use super::*;
        let groups=debug_encounters();
        let originals=BOSSES.iter().map(|(id,_)|*id).collect::<std::collections::HashSet<_>>().len();
        assert_eq!(groups[..originals].iter().map(Vec::len).sum::<usize>(),BOSSES.len());
        for (index,id) in BOSSES.iter().map(|(id,_)|*id).collect::<Vec<_>>().into_iter().fold(Vec::new(),|mut ids,id|{if !ids.contains(&id){ids.push(id);}ids}).into_iter().enumerate(){
            assert_eq!(groups[index],BOSSES.iter().filter(|(group,_)|*group==id).map(|(_,e)|*e).collect::<Vec<_>>(),"existing debug IDs must not move");
        }
        assert!(!groups.is_empty()&&groups.len()<4096);
        for (index,group) in groups.iter().enumerate(){
            if party_only_group(group){assert!(boss_test_roster(index as u16,false,true).is_err());continue;}
            assert_eq!(boss_test_roster(index as u16,false,true).unwrap(),*group);
            let mut run=erdtree_tower::Run::new(91,erdtree_tower::Class::Samurai);
            let node=run.debug_boss_test(index as u16,group[0].act).unwrap();
            assert_eq!(node_roster(&run,node),*group);
            if group.iter().any(|e|e.dlc){assert!(boss_test_roster(index as u16,false,false).is_err());}
            if let Ok(party)=boss_test_roster(index as u16,true,true){assert!(map_entities(&party).is_ok());assert!(party.iter().all(|e|!e.solo));}
            else {println!("BOSS_TEST_SOLO_ONLY #{index}: {:?}",group.iter().map(|e|e.name).collect::<Vec<_>>());}
        }
        assert!(boss_test_roster(u16::MAX,false,true).is_err());
        println!("BOSS_TEST_CATALOG groups={} parts={} party={}",groups.len(),BOSSES.len(),(0..groups.len()).filter(|&i|boss_test_roster(i as u16,true,true).is_ok()).count());
    }
    #[test]
    fn every_enemy_gets_a_tower_bar_name_and_bosses_keep_theirs() {
        let names=include_str!("../../scene-builder/AffixTextFix.cs");assert!(names.contains(&format!("PoolNameFirst={}",super::POOL_NAME_FIRST)));
        for e in super::POOLS.iter() {let text=super::bar_text(e).unwrap();assert!((super::POOL_NAME_FIRST..super::POOL_NAME_FIRST+super::pool_name_count() as i32).contains(&text));}
        for (_,boss) in super::BOSSES.iter() {
            let text=super::bar_text(boss).unwrap();assert!((super::BOSS_NAME_FIRST..super::BOSS_NAME_FIRST+super::boss_name_count() as i32).contains(&text));
            if boss.bar>0 {assert_eq!(text,super::BOSS_NAME_FIRST+boss.bar as i32-1);assert_eq!(super::bar_rank(boss),0);}
            let elite=super::Enemy{npc:boss.npc+1,kind:NodeKind::Elite,bar:0,..*boss};assert_eq!(super::bar_text(&elite),Some(super::BOSS_NAME_FIRST+(boss.npc-950000000)/2));
        }
        assert!(super::BOSS_NAME_FIRST+(super::boss_name_count() as i32)<=super::POOL_NAME_FIRST);
    }
    #[test]
    fn spawn_roles_follow_enemy_kind() {
        use erdtree_tower::encounter::{Outcome::*,Role};
        for (_,boss) in super::BOSSES.iter() {assert_eq!(super::role(boss),Role::Boss);assert_eq!(super::role(&super::Enemy{npc:boss.npc+1,kind:NodeKind::Elite,..*boss}),Role::Strong);}
        for e in super::POOLS.iter() {assert_eq!(super::role(e),match e.kind {NodeKind::Monster=>Role::Weak,NodeKind::Elite=>Role::Strong,_=>Role::Boss});}
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
    fn reported_stalling_models_keep_slots_but_are_never_selected() {
        for model in [4050,4602] {assert!(MAP_ENTITIES.iter().any(|(e,_)|e.model==model));}
        for party in [false,true] {for act in 1..=3 {for kind in [NodeKind::Monster,NodeKind::Elite,NodeKind::Boss] {for seed in 0..300 {
            assert!(roster_in(kind,seed,seed as usize%40,11,act,party).iter().all(|e|!matches!(e.model,4050|4602)));
        }}}}
    }
    #[test]
    fn ordinary_rooms_exclude_bloodhound_knights_at_every_stage() {
        for act in 1..=3 {for layer in 0..12 {for ordinal in 0..40 {
            for seed in [0,42,999] {
                let group=super::roster(NodeKind::Monster,seed,ordinal,layer,act);
                assert!(group.iter().all(|enemy|!super::NOT_IN_ORDINARY_ROOMS.contains(&enemy.model)));
                assert_eq!(group.len(),match layer/3 {0=>2,1|2=>3,_=>4});
            }
        }}}
        assert!((0..40).any(|n|super::roster(NodeKind::Elite,42,n,0,1).iter().any(|e|e.model==4290)));
        // 咒剑士仍在第一幕的精英战里（DLC 敌人开启时）
        assert!((0..400).any(|n|super::roster_in(NodeKind::Elite,42,n,0,1,false).iter().any(|e|e.model==5040)));
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
    fn sentinel_and_starscourge_join_their_elite_acts() {
        for networked in [false,true] {for (act,name,model) in [(1,"大树守卫",3251),(2,"“碎星”拉塔恩",4730)] {
            let mut seen=false;
            for ordinal in 0..300 {
                let group=roster_in(NodeKind::Elite,91,ordinal,11,act,networked);
                if let Some(enemy)=group.iter().find(|e|e.name==name) {
                    assert_eq!((enemy.act,enemy.kind,enemy.model),(act,NodeKind::Elite,model));
                    assert_eq!(role(enemy),erdtree_tower::encounter::Role::Strong);
                    if model==4730 {assert_eq!(group.len(),1);assert!(!enemy.dlc);}
                    if networked {map_entities(&group).unwrap();}
                    seen=true;
                }
                assert!(roster_in(NodeKind::Boss,91,ordinal,11,act,networked).iter().all(|e|e.name!=name));
                assert!(roster_in(NodeKind::Monster,91,ordinal,11,act,networked).iter().all(|e|e.name!=name));
            }
            assert!(seen,"missing act {act} elite {name}, party={networked}");
        }}
    }
    #[test]
    fn boss_derived_elites_follow_sequential_pacing_and_wave_gap() {
        use erdtree_tower::{encounter::{Outcome,Pacing,Role,SpawnGap,may_spawn_paced},law::Law};
        use std::time::Duration as D;
        for networked in [false,true] {
            let mut groups=0;
            for act in [2,3] {for ordinal in 0..200 {
                let group=boss_roster(NodeKind::Elite,91,ordinal,act,networked);
                if group.len()<2 {continue;}
                groups+=1;
                let roles:Vec<_>=group.iter().map(role).collect();
                assert!(roles.iter().all(|r|*r==Role::Strong),"elite group bypassed pacing: {:?}",group.iter().map(|e|e.name).collect::<Vec<_>>());
                let alive=[(Outcome::Waiting,Some((100,100)))];
                let dead=[(Outcome::Victory,None)];
                let eligible=|pacing:Pacing,earlier:&[(Outcome,Option<(i32,i32)>) ]|may_spawn_paced(networked,false,pacing.waits(earlier),&roles,1,&[earlier[0].0]);
                let pacing=Law::default().elite_pacing();
                let mut gap=SpawnGap::default();
                assert!(!gap.allows(D::ZERO,eligible(pacing,&alive)));
                assert!(!gap.allows(D::from_secs(10),eligible(pacing,&dead)));
                assert!(!gap.allows(D::from_millis(11999),eligible(pacing,&dead)));
                assert!(gap.allows(D::from_secs(12),eligible(pacing,&dead)));
                let half=Law::default().toggle(32).elite_pacing();
                assert!(!eligible(half,&alive));
                assert!(eligible(half,&[(Outcome::Waiting,Some((50,100)))]));
                assert!(eligible(Law::default().toggle(33).elite_pacing(),&alive));
            }}
            assert!(groups>0,"must exercise real multi-enemy elite groups");
        }
    }
    #[test]
    fn boss_encounters_keep_simultaneous_spawning() {
        use erdtree_tower::encounter::{Outcome,Role,may_spawn_paced};
        for group in debug_encounters() {
            let roles:Vec<_>=group.iter().map(role).collect();
            assert!(roles.iter().all(|r|*r==Role::Boss));
            for i in 0..group.len() {for networked in [false,true] {
                assert!(may_spawn_paced(networked,false,true,&roles,i,&vec![Outcome::Waiting;i]));
            }}
        }
    }
    #[test]
    fn all_generated_party_combat_rosters_have_map_entity_slots() {
        for act in 1..=3 {for seed in 0..300 {for kind in [NodeKind::Monster,NodeKind::Elite,NodeKind::Boss] {
            let group=party_roster_for_test(kind,seed,seed as usize%24,seed as usize%12,act);
            assert!(group.iter().all(|e|!e.retired));
            let ids=map_entities(&group).unwrap();
            assert_eq!(ids.len(),group.len());
            assert_eq!(ids.iter().copied().collect::<std::collections::HashSet<_>>().len(),group.len());
        }}}
    }
    #[test]
    fn a_single_gargoyle_is_an_act_three_elite_in_solo_and_absent_in_party() {
        let mut seen=false;
        for party in [false,true] {for act in 1..=3 {for kind in [NodeKind::Monster,NodeKind::Elite,NodeKind::Boss] {for seed in 0..1500 {
            let group=roster_in(kind,seed,seed as usize%24,11,act,party);
            assert!(group.iter().all(|e|e.name!="英雄石像鬼双人组"),"the elite gargoyles come alone");
            if group.iter().any(|e|e.name=="英雄石像鬼") {
                assert!(!party);assert_eq!((act,kind),(3,NodeKind::Elite));
                assert_eq!(group.iter().map(|e|(e.npc,e.act)).collect::<Vec<_>>(),[(950000153,3)],"one greatsword gargoyle");seen=true;
            }
        }}}}
        assert!(seen);
    }
    #[test]
    fn act_two_casters_are_available_in_solo_and_coop_with_existing_slots() {
        for model in [3702,4110,5311] {
            let spec=*POOLS.iter().find(|e|e.model==model).unwrap();
            assert_eq!((spec.act,spec.kind),(2,NodeKind::Monster));
            assert!(!spec.retired);assert!(!spec.solo);
            assert_eq!(spec.dlc,model==5311);
            assert_eq!(map_entities(&[spec;4]).unwrap().len(),4);
            if spec.dlc&&!erdtree_tower::dlc::enemies_allowed(){continue;}
            for party in [false,true] {
                assert!((0..800).any(|seed|roster_in(NodeKind::Monster,seed,seed as usize%24,0,2,party).iter().any(|e|e.model==model)),"caster {model}, party={party}");
            }
        }
    }
    #[test]
    fn trivial_weak_enemies_leave_the_act_two_and_three_pools() {
        for act in 2..=3 {for seed in 0..600 {
            for e in roster_in(NodeKind::Monster,seed,seed as usize%24,seed as usize%12,act,false) {
                assert!(!e.retired);
                if e.kind==NodeKind::Monster {assert!(!["辉石矿工","巨蚁","罪人","圣树士兵","腐败野狗","梅瑟莫士兵"].contains(&e.name),"{}",e.name);}
            }
        }}
        assert_eq!(POOLS.iter().filter(|e|e.retired&&e.kind==NodeKind::Monster&&e.act>=2).count(),29);
    }
    #[test]
    fn slow_enemies_leave_the_act_two_and_three_pools() {
        // 机动性差（不动或移动很慢）的怪：泥人、大型腐烂尸体、腐烂尸体、大型腐烂软泥、各种米兰达花、巨大神谕众使者
        let slow=[3750,3662,3661,4171,4480,4482,5380,3630];
        for act in 2..=3 {for seed in 0..600 {for kind in [NodeKind::Monster,NodeKind::Elite] {for party in [false,true] {
                let group=roster_in(kind,seed,seed as usize%24,seed as usize%12,act,party);
                assert!(group.iter().all(|e|!slow.contains(&e.model)),"act={act} kind={kind:?} seed={seed} party={party}: {group:?}");
        }}}}
        for act in 2..=3 {assert!(POOLS.iter().filter(|e|e.act==act&&e.kind==NodeKind::Monster&&selectable(e,true)).count()>=7,"early fights still draw a varied set of weak enemies");}
        // 第二幕的巨型螯虾改为精英：普通战里不再作为小怪出现，精英战与后几层的强怪可以抽到
        assert!(POOLS.iter().any(|e|e.act==2&&e.model==4420&&e.kind==NodeKind::Elite));
    }
    #[test]
    fn appended_weak_enemies_join_solo_and_party_rosters() {
        let solo:Vec<_>=POOLS.iter().filter(|e|e.appended).collect();
        assert_eq!(solo.len(),9);assert!(solo.iter().all(|e|(e.kind==NodeKind::Monster||e.model==4420)&&e.act>=2&&(!e.retired||e.model==3661)&&!e.dlc&&!e.solo));
        let mut seen=std::collections::HashSet::new();
        for act in 2..=3 {for seed in 0..800 {
            let group=roster_in(NodeKind::Monster,seed,seed as usize%24,seed as usize%12,act,true);
            assert_eq!(group,roster_in(NodeKind::Monster,seed,seed as usize%24,seed as usize%12,act,false));
            map_entities(&group).unwrap();
            for e in group {if e.appended {seen.insert(e.model);}}
        }}
        assert_eq!(seen.len(),8,"the appended rotting corpse is retired as too slow");
        // 追加的普通怪排在追加的 Boss 之后，再之后是序号 153 起的 DLC Boss
        let names=&*PREVIEW_NAMES;assert_eq!(&names[names.len()-15..names.len()-6],&solo.iter().map(|e|e.name).collect::<Vec<_>>()[..]);
        assert_eq!(&names[names.len()-6..],&["融泥骑士","黄金河马","“狂龙”贝勒","约定之王拉塔恩","“穿刺者”梅瑟莫","“碎星”拉塔恩"]);
    }
    #[test]
    fn appended_bosses_have_party_slots_and_reach_boss_rosters() {
        let trial:Vec<_>=BOSSES.iter().filter(|(_,e)|e.appended).collect();
        assert_eq!(trial.len(),19);
        assert!(trial.iter().all(|(_,e)|!e.solo&&!e.retired&&e.bar>0&&recommended_level(e.name).is_some()));
        let mut seen=std::collections::HashSet::new();
        for act in 1..=3 {for seed in 0..400 {for kind in [NodeKind::Elite,NodeKind::Boss] {
            assert!(roster_in(kind,seed,seed as usize%24,0,act,false).iter().all(|e|!e.retired));
            let group=roster_in(kind,seed,seed as usize%24,0,act,true);map_entities(&group).unwrap();
            for e in group {if e.appended&&boss_npc(e.npc) {assert_eq!(kind,if e.model==4730 {NodeKind::Elite}else{NodeKind::Boss});assert!(!PARTY_UNSYNCED.contains(&e.name),"unsynced bosses stay out of party fights");seen.insert(e.npc-if kind==NodeKind::Elite {1}else{0});}}
        }}}
        let eligible:std::collections::HashSet<_>=trial.iter().filter(|(id,e)|[NodeKind::Boss,NodeKind::Elite].into_iter().any(|kind|boss_eligible(kind,e)&&encounter_allowed(kind,*id))).map(|(_,e)|e.npc).collect();
        assert_eq!(seen,eligible);
        // 原有 13 个追加 Boss 的名称编号保留；后加的 5 个仍位于列表末尾。
        let names=&*PREVIEW_NAMES;assert_eq!(&names[names.len()-28..names.len()-15],&trial.iter().filter(|(_,e)|e.npc<950000000+LATE_BOSS_FIRST*2).map(|(_,e)|e.name).collect::<Vec<_>>()[..]);
    }
    #[test]
    fn synced_rennala_reaches_party_roster_with_second_phase_ai() {
        let queen=(0..2000).find_map(|seed|roster_in(NodeKind::Boss,seed,0,0,1,true).into_iter().find(|e|e.name=="“满月女王”蕾娜菈")).expect("synced Rennala must be available in multiplayer");
        assert_eq!((queen.model,queen.think),(2031,20310000));
        assert!(map_entities(&[queen]).is_ok());
    }
    #[test]
    fn map_entity_manifest_for_scene_crosscheck() {
        for (spec,ids) in MAP_ENTITIES.iter() {for (slot,id) in ids.iter().enumerate() {
            println!("PARTY_SLOT={}|{}|{}|{}|{}|{}",spec.act,spec.npc,spec.model,spec.think,slot,id);
        }}
        assert_eq!(MAP_ENTITIES.iter().map(|(_,ids)|ids.len()).sum::<usize>(),812);
    }
    #[test]
    fn first_act_elites_use_curated_combatants_and_tree_sentinel() {
        let allowed=[3010,3800,4351,4352,4353,4354,4355,4290,4340,2270,4490,5040,5192,3251];
        let mut seen=std::collections::HashSet::new();
        for seed in 0..100 {for ordinal in 0..24 {for layer in 0..12 {
            let group=roster_in(NodeKind::Elite,seed,ordinal,layer,1,false);
            assert!(group.iter().any(|e|e.kind==NodeKind::Elite));
            for enemy in group.iter().filter(|e|e.kind==NodeKind::Elite) {
                assert!(allowed.contains(&enemy.model),"Unexpected elite: {}",enemy.name);
                assert_eq!(enemy.npc,if enemy.model==3251 {950000071}else{900000000+enemy.model*10000});
                seen.insert(enemy.model);
            }
            assert_eq!(group,roster_in(NodeKind::Elite,seed,ordinal,layer,1,false));
        }}}
        assert_eq!(seen,allowed.into_iter().collect());
    }
    #[test]
    fn act_three_has_no_loretta_or_misbegotten_crusader() {
        let mut lerana=false;
        for party in [false,true] {for kind in [NodeKind::Monster,NodeKind::Elite,NodeKind::Boss] {for seed in 0..300 {for ordinal in 0..24 {
            let group=roster_in(kind,seed,ordinal,11,3,party);
            assert!(group.iter().all(|e|!matches!(e.name,"“圣树骑士”罗蕾塔"|"混种圣骑士")),"{group:?}");
            if party&&kind==NodeKind::Boss&&group.iter().any(|e|e.name=="“双月骑士”蕾拉娜") {assert_eq!(group.len(),1);map_entities(&group).unwrap();lerana=true;}
        }}}}
        assert!(lerana,"Lerana stays as a single party boss");
    }
    #[test]
    fn large_exile_soldiers_never_appear() {
        for party in [false,true] {for kind in [NodeKind::Monster,NodeKind::Elite] {for seed in 0..300 {for ordinal in 0..24 {
            assert!(roster_in(kind,seed,ordinal,seed as usize%12,1,party).iter().all(|e|e.model!=3020));
        }}}}
    }
    #[test]
    fn first_act_ordinary_pool_excludes_passive_and_fodder_types() {
        let removed=[4090,3070,3660,3661,4000,4230,4080,4161,4200,4250,4491,4070];
        let pool:Vec<_>=POOLS.iter().filter(|e|e.act==1&&e.kind==NodeKind::Monster).collect();
        assert_eq!(pool.len(),18);
        assert!(pool.iter().all(|e|!removed.contains(&e.model)));
        for model in [4311,3000,3080,3450,3650,4101,4210] {
            assert!(pool.iter().any(|e|e.model==model));
        }
        assert!(pool.iter().find(|e|e.model==4210).is_some_and(|e|e.retired),"hawks stay in the name table but are no longer drawn");
    }
    #[test]
    fn expanded_pools_deal_distinct_early_groups_and_shuffle_across_runs() {
        for (act,weak,elite) in [(1,18,21),(2,25,20),(3,22,18)] {
            assert_eq!(POOLS.iter().filter(|e|e.act==act&&e.kind==NodeKind::Monster).count(),weak);
            assert_eq!(POOLS.iter().filter(|e|e.act==act&&e.kind==NodeKind::Elite).count(),elite);
            for seed in 0..100 {
                let groups:Vec<_>=(0..4).flat_map(|n|roster(NodeKind::Monster,seed,n,0,act)).collect();
                // 前四场普通战的 8 只小怪互不相同；第二幕只剩 7 种可抽的小怪（巨型螯虾改为精英），会有一种重复一次
                let available=POOLS.iter().filter(|e|e.act==act&&e.kind==NodeKind::Monster&&selectable(e,false)).count();
                assert_eq!(groups.iter().map(|e|e.model).collect::<std::collections::HashSet<_>>().len(),available.min(8));
                assert!(groups.iter().all(|e|!e.retired));
            }
            let sequences:std::collections::HashSet<_>=(0..100).map(|seed|(0..6).map(|n|choose(NodeKind::Monster,seed*100,n,act).model).collect::<Vec<_>>()).collect();
            assert!(sequences.len()>90);
        }
    }
    #[test]
    fn eligible_encounters_are_reachable_as_boss_and_elite() {
        assert_eq!(BOSSES.len(),159);
        assert_eq!(BOSSES.iter().map(|(id,_)|*id).collect::<std::collections::HashSet<_>>().len(),124);
        for act in 1..=3 {
            for kind in [NodeKind::Boss,NodeKind::Elite] {
                if act==1&&kind==NodeKind::Elite {continue;}
                let expected:std::collections::HashSet<_>=BOSSES.iter().filter(|(id,e)|encounter_act(e)==act&&boss_eligible(kind,e)&&encounter_allowed(kind,*id)).map(|(_,e)|e.npc).collect();
                let actual:std::collections::HashSet<_>=(0..10000).flat_map(|seed|roster(kind,seed,0,11,act)).filter(|e|boss_npc(e.npc)).map(|e|e.npc-if kind==NodeKind::Elite {1}else{0}).collect();
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
    fn an_act_never_offers_the_same_enemy_as_boss_and_elite() {
        for act in 1..=3u8 {
            let mut bosses=std::collections::HashSet::new();let mut elites=std::collections::HashSet::new();
            for seed in 0..3000 {
                for e in roster(NodeKind::Boss,seed,0,11,act) {bosses.insert(e.name);}
                for e in roster(NodeKind::Elite,seed,0,11,act) {elites.insert(e.name);}
                for e in roster(NodeKind::Monster,seed,0,11,act) {elites.insert(e.name);}
            }
            let both:Vec<_>=bosses.intersection(&elites).collect();
            assert!(both.is_empty(),"act {act}: {both:?}");
            assert!(bosses.len()>=10,"act {act} keeps enough bosses: {}",bosses.len());
        }
    }
    #[test]
    fn boss_nodes_only_spawn_one_enemy_including_adds() {
        for act in 1..=3 {for seed in 0..500 {for ordinal in 0..24 {for party in [false,true] {
            let group=roster_in(NodeKind::Boss,seed,ordinal,11,act,party);
            // 第三幕可能是双王组合：两只不同的 Boss，各有自己的血条
            if group.len()==2 {
                assert!(party,"dynamic dual bosses are multiplayer-only");
                assert_eq!(act,3);assert_ne!(group[0].npc,group[1].npc);
                assert!(DUAL_PAIRS.iter().any(|p|p.contains(&group[0].name)&&p.contains(&group[1].name)));
                if party {map_entities(&group).unwrap();}
            } else {
                assert_eq!(group.len(),1,"act={act}, seed={seed}: {group:?}");
                assert!(!party||DUAL_PAIRS.iter().all(|p|!p.contains(&group[0].name)||act!=3),"party pair members only appear together");
            }
            assert!(group.iter().all(|e|e.bar>0));
        }}}}
    }
    #[test]
    fn late_dlc_bosses_have_party_slots_and_stay_boss_fights_only() {
        let late:Vec<_>=BOSSES.iter().filter(|(_,e)|e.npc>=950000000+LATE_BOSS_FIRST*2&&!elite_only(e)).map(|(_,e)|(e.act,e.name)).collect();
        assert_eq!(late,vec![(2,"融泥骑士"),(2,"黄金河马"),(3,"“狂龙”贝勒"),(3,"约定之王拉塔恩"),(3,"“穿刺者”梅瑟莫")]);
        for (act,name) in late {
            assert!((0..4000).any(|seed|roster_in(NodeKind::Boss,seed,0,11,act,false).iter().any(|e|e.name==name)),"{name} is drawn as a boss");
            assert!((0..4000).any(|seed|roster_in(NodeKind::Boss,seed,0,11,act,true).iter().any(|e|e.name==name)),"{name} is drawn in multiplayer");
            let enemy=BOSSES.iter().find(|(_,e)|e.name==name).unwrap().1;
            assert!(!enemy.solo);
            let ids=map_entities(&[enemy]).unwrap();assert_eq!(ids.len(),1);
            assert!(super::super::map_guards::entity(ids[0]).is_ok());
            for seed in 0..1500 {
                for kind in [NodeKind::Elite,NodeKind::Monster] {assert!(roster_in(kind,seed,seed as usize%24,11,act,false).iter().all(|e|e.name!=name),"{name} never appears as an elite");}
                map_entities(&roster_in(NodeKind::Boss,seed,0,11,act,true)).unwrap();
            }
        }
    }
    #[test]
    fn the_carian_pair_is_one_act_three_option_among_the_single_bosses() {
        for party in [false,true] {
            let mut seen=std::collections::HashMap::new();
            for seed in 0..4000 {let group=roster_in(NodeKind::Boss,seed,0,11,3,party);*seen.entry(group.iter().map(|e|e.name).collect::<Vec<_>>()).or_insert(0)+=1;}
            if !party {assert!(seen.keys().all(|names|names.len()==1));continue;}
            let pair=seen.iter().find(|(names,_)|names.len()==2).map(|(_,n)|*n).expect("the pair is reachable");
            let options=seen.len() as i32;
            assert!((pair as i32-4000/options).abs()<4000/options/2,"pair drawn {pair} times among {options} options");
        }
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
        let astel=BOSSES.iter().find(|(_,e)|e.name=="艾斯缇").unwrap();
        for kind in [NodeKind::Boss,NodeKind::Elite] {assert!(!encounter_allowed(kind,astel.0));}
        for seed in 0..3000 {for party in [false,true] {assert!(roster_in(NodeKind::Boss,seed,0,11,2,party).iter().all(|e|e.name!="艾斯缇"));}}
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
            let expected:std::collections::HashSet<_>=BOSSES.iter().filter(|(id,e)|e.npc!=950000196&&!elite_only(e)&&e.act==act&&recommended_level(e.name).is_some_and(|level|level>=minimum)&&encounter_allowed(NodeKind::Boss,*id)).map(|(id,_)|*id).collect();
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
                assert_eq!(boss_eligible(NodeKind::Elite,e),elite_only(e));
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
            let count = POOLS.iter().filter(|e|e.act==act&&e.kind==kind&&!e.retired).count();
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
    #[test]
    fn dlc_enemies_reach_multiplayer_and_keep_existing_name_and_entity_ids() {
        let dlc:Vec<_>=POOLS.iter().filter(|e|e.dlc).collect();
        assert_eq!(dlc.len(),20);
        assert!(dlc.iter().all(|e|e.model>=5000&&!boss_npc(e.npc)&&bar_rank(e)>=2&&role(e)!=erdtree_tower::encounter::Role::Boss));
        // 名称编号：DLC 敌人排在已有名称之后
        let first_dlc=POOL_NAMES.iter().position(|n|dlc.iter().any(|e|e.name==*n)).unwrap();
        assert!(POOL_NAMES[first_dlc..first_dlc+dlc.len()].iter().all(|n|dlc.iter().any(|e|e.name==*n)));
        let first_preview=PREVIEW_NAMES.iter().position(|n|dlc.iter().any(|e|e.name==*n)).unwrap();
        assert_eq!(first_preview,PREVIEW_NAMES.len()-dlc.len()-28);
        // 联机实体只包含地图里摆放过的敌人
        assert_eq!(MAP_ENTITIES.iter().filter(|(spec,_)|spec.dlc).map(|(_,ids)|ids.len()).sum::<usize>(),80);
        for act in 1..=3 {for seed in 0..200 {for kind in [NodeKind::Monster,NodeKind::Elite] {
            assert!(roster_in(kind,seed,0,11,act,true).iter().all(|e|!e.retired));
        }}}
        let solo:std::collections::HashSet<_>=(1..=3).flat_map(|act|(0..400).flat_map(move|seed|[NodeKind::Monster,NodeKind::Elite].into_iter().flat_map(move|kind|roster_in(kind,seed,0,11,act,false)))).filter(|e|e.dlc).map(|e|e.model).collect();
        assert_eq!(solo,dlc.iter().filter(|e|!e.retired).map(|e|e.model).collect(),"every active DLC enemy is reachable solo");
        let party:std::collections::HashSet<_>=(1..=3).flat_map(|act|(0..400).flat_map(move|seed|[NodeKind::Monster,NodeKind::Elite].into_iter().flat_map(move|kind|roster_in(kind,seed,0,11,act,true)))).filter(|e|e.dlc).map(|e|e.model).collect();
        assert_eq!(party,solo,"every DLC enemy must also be reachable in multiplayer");
        for act in 1..=3 {
            let old:Vec<_>=MAP_ENTITIES.iter().filter(|(e,_)|e.act==act&&!e.dlc&&!e.appended&&!(e.act==3&&e.npc==950000197)).flat_map(|(_,ids)|ids).copied().collect();
            assert_eq!(old,(0..old.len()).map(|n|48000000+u32::from(act)*10000+n as u32).collect::<Vec<_>>());
        }
        let lines:Vec<_>=include_str!("../../scene-assets/party-dlc-slots.tsv").lines().collect();assert_eq!(lines.len(),80);
        for line in lines {
            let p:Vec<u32>=line.split('|').map(|s|s.parse().unwrap()).collect();
            let (enemy,ids)=MAP_ENTITIES.iter().find(|(e,_)|e.act as u32==p[0]&&e.npc as u32==p[1]).unwrap();
            assert!(enemy.dlc);assert_eq!(enemy.model as u32,p[2]);assert_eq!(enemy.think,p[3]);assert_eq!(ids[p[4] as usize],p[5]);assert_eq!(super::super::map_guards::entity(p[5]).unwrap(),p[6]);
        }
    }
    #[test]
    fn full_grown_fallingstar_is_only_an_act_three_elite() {
        let mut seen=false;let mut regular=false;
        for party in [false,true] {for act in 1..=3 {for kind in [NodeKind::Monster,NodeKind::Elite,NodeKind::Boss] {for seed in 0..1000 {
            let group=roster_in(kind,seed,0,11,act,party);
            for e in &group {
                if e.npc==950000196||e.npc==950000197 {
                    assert_eq!((act,kind,e.act,e.npc),(3,NodeKind::Elite,3,950000197));
                    assert!(map_entities(&group).is_ok());seen=true;
                }
                if e.npc==950000146||e.npc==950000147 {assert_eq!(act,2);regular=true;}
            }
        }}}}
        assert!(seen&&regular);
    }
}
