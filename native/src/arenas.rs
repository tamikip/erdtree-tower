/// 龙王圆形平台中心圈的半径（米）
pub const CIRCLE_RADIUS:f32=8.0;
#[derive(Clone, Copy)]
pub struct Arena {
    pub block: i32,
    pub entry: u32,
    pub stone: u32,
    pub name: &'static str,
    pub center:[f32;3],
    pub yaw:f32,
    arrival:Option<[f32;3]>,
}
const LEGACY: [Arena; 3] = [
    Arena { block: 0x2d020000, entry: 45022600, stone: 45020100, name: "宁姆格福竞技场",center:[0.0;3],yaw:0.0,arrival:None },
    Arena { block: 0x2d010000, entry: 45012600, stone: 45010100, name: "盖利德竞技场",center:[0.0;3],yaw:0.0,arrival:None },
    Arena { block: 0x2d000000, entry: 45002600, stone: 45000100, name: "王城竞技场",center:[0.0;3],yaw:0.0,arrival:None },
];
static ARENAS:std::sync::LazyLock<Vec<Arena>>=std::sync::LazyLock::new(||include_str!("../../scene-assets/battlefields.txt").lines().map(|line| {
    let p:Vec<_>=line.split('|').collect();let map:Vec<u8>=p[1][1..].split('_').map(|v|v.parse().unwrap()).collect();
    let basis=p[2].parse::<u32>().unwrap();
    Arena{block:i32::from_be_bytes([map[0],map[1],map[2],map[3]]),entry:basis+2600,stone:basis+100,name:p[3],center:[p[4].parse().unwrap(),p[5].parse().unwrap(),p[6].parse().unwrap()],yaw:p[7].parse::<f32>().unwrap().to_radians(),arrival:Some(std::array::from_fn(|i|p[8+i].parse().unwrap()))}
}).collect());
impl Arena {
    pub fn uses_circle(self)->bool {self.block==0x0d000000&&is_built(self.block)}
    /// 走进圈开战的场地：本机角色是否已在中心圈内（与地图里的中心区域同为半径 CIRCLE_RADIUS 的球，scene-builder DragonlordEntrance）。
    /// 地图事件按实体 10000 判断，联机时对队员来说那是房主，队员要等房主进圈才触发；这里按本机位置判断
    pub fn inside_circle(self,block_position:[f32;3])->bool {
        let center=self.position([0.0,0.15,0.0]);
        self.uses_circle()&&(block_position[0]-center[0]).hypot(block_position[2]-center[2])<=CIRCLE_RADIUS&&(block_position[1]-center[1]).abs()<=CIRCLE_RADIUS
    }
    pub fn combat_entry(self)->u32 {self.entry+351}
    pub fn attendant_position(self)->[f32;3] {self.arrival.unwrap_or_else(||self.position([28.35,0.35,-4.06]))}
    pub fn position(self,local:[f32;3])->[f32;3] {
        let (sin,cos)=self.yaw.sin_cos();
        [self.center[0]+local[0]*cos+local[2]*sin,self.center[1]+local[1],self.center[2]-local[0]*sin+local[2]*cos]
    }
    pub fn boss_entity(self,slot:usize)->i32 {self.stone as i32-100+850+slot as i32}
    pub fn below_floor(self,y:f32)->bool {y<self.center[1]-1.5}
}
pub fn for_act(act: u8) -> Arena {
    LEGACY[usize::from(act.clamp(1,3)-1)]
}
pub fn for_block(block: i32) -> Option<Arena> { ARENAS.iter().chain(LEGACY.iter()).chain(BUILT.iter().map(|(_,a)|a)).copied().find(|a| a.block == block) }

/// 主题幕的战斗场地：scene-builder arena-build（Windows）把原版地图改造成战场后写出 scene-assets/arenas-built.txt，每行
/// 键|地图|名称|中心 x y z|朝向（度）|场边引导者 x y z。改造时复用宁姆格福竞技场（m45_02）的进场设施与编号，
/// 所以入口点、召唤石等编号与第一幕竞技场相同。场地表编入 DLL，进场前验证实际地图与事件文件。
const SITE_BASIS:u32=45020000;
static BUILT:std::sync::LazyLock<Vec<(&'static str,Arena)>>=std::sync::LazyLock::new(|| {
    let text=include_str!("../../scene-assets/arenas-built.txt");
    let rows:Vec<_>=text.lines().filter(|l|!l.trim().is_empty()&&!l.starts_with('#')).filter(|line| {
        let p:Vec<_>=line.split('|').collect();
        p.len()==13&&p[12]=="1"&&site_info(p[0]).is_some_and(|(act,dlc)|p[10].parse::<u8>().ok()==Some(act)&&(p[11]=="1")==dlc)
    }).filter_map(parse_built).collect();
    crate::log(format!("Built arenas: {}",rows.iter().map(|(k,_)|*k).collect::<Vec<_>>().join(",")));
    rows
});
fn parse_built(line:&str)->Option<(&'static str,Arena)> {
    let p:Vec<&str>=line.split('|').collect();
    if p.len()<10 {return None;}
    let map:Vec<u8>=p[1].strip_prefix('m')?.split('_').map(|v|v.parse().ok()).collect::<Option<_>>()?;
    if map.len()!=4 {return None;}
    let f=|i:usize|p.get(i).and_then(|v|v.parse::<f32>().ok());
    let leak=|v:&str|->&'static str {Box::leak(v.to_owned().into_boxed_str())};
    Some((leak(p[0]),Arena{block:i32::from_be_bytes([map[0],map[1],map[2],map[3]]),entry:SITE_BASIS+2600,stone:SITE_BASIS+100,name:leak(p[2]),
        center:[f(3)?,f(4)?,f(5)?],yaw:f(6)?.to_radians(),arrival:Some([f(7)?,f(8)?,f(9)?])}))
}
/// 调试菜单“下一战指定场地”（单人）：场地键，空为按主题
static OVERRIDE:std::sync::Mutex<Option<&'static str>>=std::sync::Mutex::new(None);
pub fn set_override(key:Option<&'static str>) {if let Ok(mut o)=OVERRIDE.lock(){*o=key;}crate::log(format!("Arena override: {key:?}"));}
pub fn override_key()->Option<&'static str> {OVERRIDE.lock().ok().and_then(|o|*o)}
/// v89 的战场（battlefields.txt，已随资源发布）：arena-sites.txt 里地图写 battlefield、锚点写战场序号的场地直接用它们。
/// 第 1 处史东薇尔庭院已被跑酷试炼改写，不再引用
#[cfg(test)]
static PREBUILT:std::sync::LazyLock<Vec<(&'static str,Arena)>>=std::sync::LazyLock::new(||include_str!("../../scene-assets/arena-sites.txt").lines()
    .filter_map(|l|{let p:Vec<&str>=l.split('|').collect();(p.len()>=5&&p[1]=="battlefield").then(||(p[0],ARENAS[p[2].parse::<usize>().unwrap()-1]))}).collect());
/// 可用的场地键与名称（调试菜单用）
pub fn built()->Vec<(&'static str,&'static str)> {BUILT.iter().map(|(k,a)|(*k,a.name)).collect()}
/// 由 arena-build 生成的场地：复用宁姆格福竞技场的编号与场边服务
pub fn is_built(block:i32)->bool {BUILT.iter().any(|(_,a)|a.block==block)}
const RESOURCES:&str=include_str!("../../scene-assets/arenas-resources.txt");
/// 清单里的一行（路径|大小|FNV-1a 64）：文件缺失返回 Some(true)，与 DLL 不一致返回 Some(false)，一致返回 None
fn resource_problem(row:&str)->Option<bool> {
    let p:Vec<_>=row.split('|').collect();
    let Ok(bytes)=std::fs::read(crate::LOG_PATH.with_file_name("scene-assets").join(p[0])) else {return Some(true);};
    let hash=bytes.iter().fold(14695981039346656037u64,|h,b|(h^u64::from(*b)).wrapping_mul(1099511628211));
    (p.len()!=3||p[1].parse::<usize>().ok()!=Some(bytes.len())||u64::from_str_radix(p[2],16).ok()!=Some(hash)).then_some(false)
}
/// 开局前把整合包里的全部场地资源核对一遍：返回缺失或与 DLL 不一致的文件（不用等走到那一战才发现）
pub fn mismatched_resources()->Vec<String> {
    RESOURCES.lines().filter(|l|l.split('|').count()==3).filter(|row|resource_problem(row).is_some()).map(|row|row.split('|').next().unwrap_or("").to_owned()).collect()
}
pub fn require_resources(arena:Arena)->Result<(),String> {
    if !is_built(arena.block){return Ok(());}
    let [a,b,c,d]=arena.block.to_be_bytes();let map=format!("m{a:02}_{b:02}_{c:02}_{d:02}");
    for path in [format!("map/mapstudio/{map}.msb.dcx"),format!("event/{map}.emevd.dcx"),"asset/aeg/aeg990/aeg990_446.geombnd.dcx".to_owned()] {
        let row=RESOURCES.lines().find(|l|l.split('|').next()==Some(path.as_str())).ok_or("场地资源清单缺失")?;
        match resource_problem(row) {
            Some(true)=>return Err(format!("缺少场地资源：{path}，请更新完整整合包。")),
            Some(false)=>return Err(format!("场地资源与 DLL 不匹配：{path}，请全队更新同一版整合包。")),
            None=>{},
        }
    }
    use fromsoftware_shared::FromStatic;
    let params=unsafe {eldenring::cs::SoloParamRepository::instance()}.map_err(|_|"游戏参数尚未就绪")?;
    let arch=params.get::<eldenring::cs::AssetEnvironmentGeometryParam>(990446).ok_or("缺少传送门装饰参数，请更新完整整合包。")?;
    if arch.behavior_type()!=0||arch.hit_create_type()!=0||arch.collision_type()!=0 {return Err("传送门碰撞参数不匹配，请更新完整整合包。".into());}
    Ok(())
}
fn site_info(key:&str)->Option<(u8,bool)> {
    include_str!("../../scene-assets/arena-sites.txt").lines().find_map(|line| {
        let p:Vec<_>=line.split('|').collect();
        if p.first()!=Some(&key)||p.len()<5{return None;}
        let act=if p[1]=="battlefield"{p[2]}else{*p.get(9)?}.parse::<u8>().ok()?;
        (1..=3).contains(&act).then_some((act,p[4]=="1"))
    })
}
#[cfg(test)]
fn automatic(key:&str)->bool {
    include_str!("../../scene-assets/arena-sites.txt").lines().find(|l|l.split('|').next()==Some(key))
        .is_some_and(|l|l.split('|').nth(10)!=Some("0"))
}
#[cfg(test)]
fn select_site(seed:u64,act:u8,dlc:bool,available:&[(&str,Arena)])->Arena {
    let candidates:Vec<_>=available.iter().filter(|(key,_)|automatic(key)&&site_info(key).is_some_and(|(a,d)|a==act&&(!d||dlc))).collect();
    let theme=erdtree_tower::themes::for_act_with(seed,act,dlc);
    if let Some((_,arena))=candidates.iter().find(|(key,_)|*key==theme.arena){return *arena;}
    if candidates.is_empty(){return for_act(act);}
    candidates[erdtree_tower::themes::arena_index(seed,act,candidates.len()).unwrap()].1
}
/// 暂时统一到龙王平台；三幕联机预置均在同一地图，忽略主题和调试覆盖。
pub fn for_run(_run:&erdtree_tower::Run,_party:bool)->Arena {
    BUILT.iter().find(|(key,_)|*key=="farum_dragonlord").expect("built dragonlord arena missing").1
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_dragonlord_circle_is_checked_against_the_local_player() {
        let run=erdtree_tower::Run::new(42,erdtree_tower::Class::Hero);let arena=super::for_run(&run,true);
        assert!(arena.uses_circle());
        let at=|x:f32,z:f32|arena.position([x,0.15,z]);
        assert!(arena.inside_circle(at(0.0,0.0))&&arena.inside_circle(at(5.0,5.0)));
        assert!(!arena.inside_circle([21.857,1009.726,300.433]),"players land about 30 m out and still walk in");
        assert!(!arena.inside_circle(at(9.0,0.0))&&!arena.inside_circle(at(1.0,20.0)));
        let mut high=at(0.0,0.0);high[1]+=20.0;assert!(!arena.inside_circle(high),"not while falling from the spawn height");
        let colosseum=super::for_block(0x2d020000).unwrap();assert!(!colosseum.inside_circle(colosseum.position([0.0,0.0,0.0])),"fog-gate arenas keep their gate");
    }
    #[test]
    fn all_acts_and_modes_use_dragonlord_platform() {
        for seed in [0,1,42,u64::MAX] {
            let mut run=erdtree_tower::Run::new(seed,erdtree_tower::Class::Hero);
            for act in 1..=3 {
                run.act=act;
                for party in [false,true] {
                    let arena=super::for_run(&run,party);
                    assert_eq!(arena.block,0x0d000000);
                    assert!(arena.name.contains("龙王"));
                    assert_eq!(arena.entry,45022600);
                }
            }
        }
    }
    #[test]
    fn prebuilt_sites_use_the_v89_battlefields_with_matching_act_numbers() {
        let sites:Vec<_>=super::PREBUILT.iter().collect();
        assert!(sites.iter().all(|(_,a)|a.block!=0x0a000000),"Stormveil now belongs to the parkour trial");
        for theme in erdtree_tower::themes::THEMES.iter() {
            if let Some((_,arena))=sites.iter().find(|(k,_)|*k==theme.arena) {assert_eq!(arena.stone,super::for_act(theme.act).stone,"{}",theme.name);}
        }
        assert_eq!(sites.len(),2);
    }
    #[test]
    fn built_site_rows_parse_and_reuse_the_first_arena_numbers() {
        let (key,arena)=super::parse_built("leyndell_morgott|m11_00_00_00|王城·恶兆王的战场|12.5|60|-300|90|20|60.5|-310").unwrap();
        assert_eq!((key,arena.block,arena.entry,arena.stone),("leyndell_morgott",0x0b000000,45022600,45020100));
        assert_eq!(arena.combat_entry(),45022951);assert_eq!(arena.attendant_position(),[20.0,60.5,-310.0]);
        assert!((arena.yaw-std::f32::consts::FRAC_PI_2).abs()<1e-5);
        assert!(super::parse_built("broken|m11_00|x").is_none());
        assert!(super::parse_built("x|m11_00_00_00|名|a|0|0|0|0|0|0").is_none());
        let run=erdtree_tower::Run::new(1,erdtree_tower::Class::Hero);
        assert!(super::is_built(super::for_run(&run,true).block),"multiplayer uses a built combat map");
    }
    #[test]
    fn underground_arenas_use_their_own_floor_height() {
        let (_,arena)=super::parse_built("siofra_ancestor|m12_08_00_00|test|0|-1736|0|0|0|-1735.6|0").unwrap();
        assert!(!arena.below_floor(-1736.0));
        assert!(!arena.below_floor(-1737.4));
        assert!(arena.below_floor(-1737.6));
        assert!(super::for_act(1).below_floor(-1.6));
    }
    #[test]
    fn expanded_pool_is_deterministic_act_scoped_and_respects_dlc() {
        let sites:Vec<_>=include_str!("../../scene-assets/arena-sites.txt").lines().filter_map(|line| {
            let p:Vec<_>=line.split('|').collect();
            if p.len()<10{return None;}
            Some(super::parse_built(&format!("{}|{}|{}|0|0|0|0|1|0|0",p[0],p[1],p[3])).unwrap())
        }).collect();
        assert_eq!(sites.len(),16);
        for act in 1..=3 {
            let mut seen=std::collections::BTreeSet::new();
            for seed in 0..1000 {
                let arena=super::select_site(seed,act,true,&sites);
                assert_eq!(arena.block,super::select_site(seed,act,true,&sites).block);
                let key=sites.iter().find(|(_,a)|a.block==arena.block).unwrap().0;
                assert_eq!(super::site_info(key).unwrap().0,act);
                seen.insert(key);
                let base=super::select_site(seed,act,false,&sites);
                let key=sites.iter().find(|(_,a)|a.block==base.block).unwrap().0;
                assert!(!super::site_info(key).unwrap().1);
            }
            assert!(!seen.contains("anticipation_scion"));
            assert_eq!(seen.len(),sites.iter().filter(|(k,_)|super::automatic(k)&&super::site_info(k).unwrap().0==act).count());
        }
        assert_eq!(super::select_site(1,2,true,&[]).block,super::for_act(2).block);
    }
    #[test]
    fn combat_arrival_uses_exterior_gate_region_without_moving_services() {
        for act in 1..=3 {
            let arena=super::for_act(act);
            assert_eq!(arena.combat_entry(),arena.entry+351);
            assert_eq!(arena.attendant_position(),[28.35,0.35,-4.06]);
        }
    }
    #[test]
    fn attendants_use_verified_entry_height_instead_of_flat_arena_offsets() {
        for (index,line) in include_str!("../../scene-assets/battlefields.txt").lines().enumerate() {
            let fields:Vec<_>=line.split('|').collect();
            let expected:[f32;3]=std::array::from_fn(|i|fields[8+i].parse().unwrap());
            let arena=super::ARENAS[index];
            assert_eq!(arena.attendant_position(),expected);
            assert_ne!(arena.attendant_position(),arena.position([-16.0,0.1,10.0]));
        }
        let legacy=super::for_block(0x2d020000).unwrap();
        assert_eq!(legacy.attendant_position(),[28.35,0.35,-4.06]);
    }
    #[test]
    fn acts_have_distinct_matching_arena_resources() {
        let arenas = [super::for_act(1),super::for_act(2),super::for_act(3)];
        assert_eq!(arenas[0].block,0x2d020000);
        assert_eq!(arenas[0].entry,45022600);
        assert_eq!(arenas[0].attendant_position(),[28.35,0.35,-4.06]);
        for (i, a) in arenas.iter().enumerate() {
            assert_eq!(super::for_act(i as u8 + 1).block, a.block);
            assert_eq!(super::for_block(a.block).unwrap().stone, a.stone);
            assert_eq!(a.entry - a.stone, 2500);
            assert_eq!(a.position([0.0;3]),a.center);
            assert_eq!(a.boss_entity(0),a.entry as i32-1750);
            assert_eq!(a.block,[0x2d020000,0x2d010000,0x2d000000][i]);
            let front=a.position([0.0,0.0,10.0]);let back=a.position([0.0,0.0,-10.0]);
            assert!((front[0]+back[0]-2.0*a.center[0]).abs()<0.001);
            assert!((front[2]+back[2]-2.0*a.center[2]).abs()<0.001);
            assert!(arenas[..i].iter().all(|b| b.block != a.block));
        }
        assert!(super::for_block(0x0b0a0000).is_none());
    }
}
