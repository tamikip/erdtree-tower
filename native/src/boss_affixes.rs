use std::time::{Duration,Instant};
use eldenring::cs::{ChrInsExt,FieldInsHandle,SoloParamRepository,SpEffectParam,WorldChrMan};
use fromsoftware_shared::FromStatic;
use erdtree_tower::boss_affixes::Affix;
pub const FIRST:i32=9620000;
/// 腐败的：Boss 每次打中玩家给玩家挂一次：原版“NPC：猩红腐败”（21001，每次积累 20，积满后照常发作、持续扣血）的复制，
/// 只打开了“作用于自身”。21001 本身不能作用于自身，直接挂给玩家会被游戏忽略（腐败条不涨）；更早自建的 FIRST+41 只有积累值、积满不发作
const ROT_BUILDUP:i32=FIRST+42;const ENVY:i32=FIRST+70;const PRIDE:i32=FIRST+80;const SHIELD:i32=FIRST+90;pub(crate) const REBORN:i32=FIRST+95;
/// 迅猛、荆棘的常驻光效，吸血回血时的闪光
const SWIFT:i32=FIRST+50;const THORNED:i32=FIRST+60;const LEECH_FLASH:i32=FIRST+65;
const EFFECTS:[i32;13]=[FIRST,FIRST+10,FIRST+20,FIRST+30,FIRST+40,ROT_BUILDUP,ENVY,PRIDE,SHIELD,REBORN,SWIFT,THORNED,LEECH_FLASH];
/// 苦难律法「词条强化」用的强化版效果：坚毅、铁壁、狂暴、再生、嫉妒、傲慢（各自的编号 +1）
const EMPOWERED:[i32;6]=[FIRST+1,FIRST+11,FIRST+21,FIRST+31,ENVY+1,PRIDE+1];
/// 由运行时计算的词条数值；词条强化时换成 STRONG
struct Tuning {
    /// 腐败的：每次打中积累几次（每次 20）
    rot_per_hit:u8,
    /// 吸血：每次打中玩家回复最大生命的千分比；荆棘：反弹受到伤害的百分比
    leech_permille:i64,thorn_percent:i64,
    /// 迅猛：动作播放速度倍率（出招、收招、移动都更快）
    swift_speed:f32,
    /// 黄金律法：护盾间隔（秒）；不屈：回复到最大生命的百分比；傲慢：多久没挨打后触发（秒）
    shield_every:u64,reborn_percent:i64,proud_after:u64,
}
const NORMAL:Tuning=Tuning{rot_per_hit:1,leech_permille:10,thorn_percent:10,swift_speed:1.1,shield_every:25,reborn_percent:40,proud_after:8};
const STRONG:Tuning=Tuning{rot_per_hit:2,leech_permille:15,thorn_percent:12,swift_speed:1.3,shield_every:25,reborn_percent:60,proud_after:6};
/// 荆棘：一次反弹最多扣这么多生命（同一帧打出的伤害合算一次）
const THORN_CAP:i64=200;
const BAR_MARKER:i32=FIRST+99;
/// text-update 为新增词条的组合也写好血条名称后加入的第二个标记
const BAR_MARKER_V2:i32=FIRST+98;
/// text-update 为之后追加的 Boss（名称序号 140 起）写好全部带词条名称后加入的第三个标记
const BAR_MARKER_V3:i32=FIRST+97;
const BAR_MARKER_V4:i32=FIRST+100;
static BAR_NAMES:std::sync::atomic::AtomicU8=std::sync::atomic::AtomicU8::new(0);
/// 名称资源版本：1 最早组合，2 全部组合，3 追加 Boss，4 精英与普通敌人
fn bar_names_version()->u8 {
    let known=BAR_NAMES.load(std::sync::atomic::Ordering::Relaxed);
    if known==4 {return 4;}
    let version=unsafe {SoloParamRepository::instance()}.map_or(0,|p|if p.get::<SpEffectParam>(BAR_MARKER_V4 as u32).is_some(){4}
        else if p.get::<SpEffectParam>(BAR_MARKER_V3 as u32).is_some(){3}
        else if p.get::<SpEffectParam>(BAR_MARKER_V2 as u32).is_some(){2}else if p.get::<SpEffectParam>(BAR_MARKER as u32).is_some(){1}else{0});
    BAR_NAMES.store(version,std::sync::atomic::Ordering::Relaxed);version
}
/// 调试菜单的“血条颜色标签测试”：打开后主 Boss 血条显示 text-update 写入的测试文字（带 <font color> 标签），
/// 用来确认游戏的血条是否支持颜色标签
pub const BAR_TAG_TEST_NAME:i32=9799999;
static BAR_TAG_TEST:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
pub fn bar_tag_test()->bool {BAR_TAG_TEST.load(std::sync::atomic::Ordering::Relaxed)}
pub fn set_bar_tag_test(on:bool) {BAR_TAG_TEST.store(on,std::sync::atomic::Ordering::Relaxed);super::log(format!("Boss bar color tag test: {on}"));}
/// text-update 写入带词条的血条名称时，同时在 regulation 中加入标记行；没有标记（或新组合还没生成名称）就沿用原名，避免名称缺失
pub fn bar_name(affixes:&[Affix],name:i32)->i32 {
    if bar_tag_test() {return BAR_TAG_TEST_NAME;}
    if affixes.is_empty() {return name;}
    let version=bar_names_version();
    let index=erdtree_tower::boss_affixes::combos().iter().position(|c|c==affixes);
    let late=name-super::enemies::BOSS_NAME_FIRST>=erdtree_tower::boss_affixes::LATE_BOSS_FIRST;
    let covered=if late {version>=3}else{match version {2..=4=>true,1=>index.is_some_and(|i|i<erdtree_tower::boss_affixes::legacy_combos()),_=>false}};
    if !covered {return name;}
    erdtree_tower::boss_affixes::bar_name(affixes,name,super::enemies::BOSS_NAME_FIRST).unwrap_or(name)
}
pub fn elite_bar_name(affixes:&[Affix],name:i32)->i32 {
    if affixes.is_empty()||bar_names_version()<4 {return name;}
    erdtree_tower::boss_affixes::pool_bar_name(affixes,name,super::enemies::POOL_NAME_FIRST)
        .or_else(||erdtree_tower::boss_affixes::bar_name(affixes,name,super::enemies::BOSS_NAME_FIRST)).unwrap_or(name)
}
/// 整场挂着的效果；强化版的编号是原编号 +1
fn resident(affix:Affix,empowered:bool)->Option<i32> {
    let strong=i32::from(empowered);
    match affix {Affix::Stalwart=>Some(FIRST+strong),Affix::Bulwark=>Some(FIRST+10+strong),Affix::Frenzied=>Some(FIRST+20+strong),
        Affix::Rot=>Some(FIRST+40),Affix::Swift=>Some(SWIFT),Affix::Thorned=>Some(THORNED),_=>None}
}
fn regeneration_effect(empowered:bool,authority:bool,quiet:Duration)->Option<i32> {
    (authority&&quiet>=Duration::from_secs(3)).then_some(FIRST+30+i32::from(empowered))
}
#[test]
fn regeneration_waits_three_seconds_and_only_the_authority_heals() {
    for empowered in [false,true] {
        assert_eq!(resident(Affix::Regenerating,empowered),None);
        for ms in [0,1000,2999] {assert_eq!(regeneration_effect(empowered,true,Duration::from_millis(ms)),None);}
        assert_eq!(regeneration_effect(empowered,true,Duration::from_secs(3)),Some(FIRST+30+i32::from(empowered)));
        assert_eq!(regeneration_effect(empowered,false,Duration::from_secs(60)),None);
        let start=Instant::now();let next_hit=start+Duration::from_secs(2);
        assert_eq!(regeneration_effect(empowered,true,(start+Duration::from_secs(4)).duration_since(next_hit)),None);
    }
}
pub fn validate(empowered:bool)->Result<(),String> {
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"Boss 词条参数未加载")?;
    if EFFECTS.iter().chain(EMPOWERED.iter().filter(|_|empowered)).any(|id|params.get::<SpEffectParam>(*id as u32).is_none()){return Err("缺少 Boss 词条参数，请使用完整启动包".into());}Ok(())
}
struct Foe {handle:FieldInsHandle,hp:i32,hurt:Instant,proud:bool,reborn:bool}
/// 联机：房主按所有玩家的掉血算吸血；成员只算自己的（腐败、荆棘），护盾的时机跟房主（看 Boss 身上的护盾效果）
#[derive(Default)]
pub struct Fight {key:Option<(u64,u8,usize)>,affixes:Vec<Affix>,empowered:bool,foes:Vec<Foe>,flasks:Option<u8>,shield_at:Option<Instant>,shield_until:Option<Instant>,rot_at:Option<Instant>,missing:bool,player_hp:Option<i32>,
    /// 房主：每个玩家上一帧的生命（吸血按全队受到的伤害）
    players:Vec<(FieldInsHandle,i32)>,drinks:erdtree_tower::party_law::DrinkDeltas}
impl Fight {
    /// empowered：苦难律法「词条强化」
    pub fn begin(&mut self,key:(u64,u8,usize),affixes:Vec<Affix>,empowered:bool) {
        if self.key==Some(key){return;}
        if !affixes.is_empty(){super::log(format!("BOSS_AFFIX node={} act={} affixes={}{}",key.2,key.1,erdtree_tower::boss_affixes::title(&affixes),if empowered{" (empowered)"}else{""}));}
        *self=Self{key:Some(key),affixes,empowered,..Default::default()};
    }
    fn tuning(&self)->&'static Tuning {if self.empowered {&STRONG}else{&NORMAL}}
    pub fn affixes(&self)->&[Affix] {&self.affixes}
    pub fn title(&self)->Option<String> {(!self.affixes.is_empty()).then(||erdtree_tower::boss_affixes::title(&self.affixes))}
    /// 屏幕上方的护盾提示：(文字, 是否为预警)。展开前 2 秒预警，展开中倒计时
    pub fn alert(&self)->Option<(String,bool)> {
        if !self.affixes.contains(&Affix::Golden) {return None;}
        let now=Instant::now();
        if let Some(until)=self.shield_until.filter(|until|now<*until) {
            return Some((format!("黄金护盾展开中 · 受到的伤害 -60% · 剩 {} 秒",until.duration_since(now).as_secs()+1),false));
        }
        self.shield_at.filter(|at|*at>now&&at.duration_since(now)<=Duration::from_secs(2)).map(|_|("黄金护盾即将展开".to_owned(),true))
    }
    /// authority：房主或单人（只有它改 Boss 的状态）；party：联机（荆棘只按自己与自己召唤物的伤害反弹）；team：联机时全队的喝药次数
    pub fn tick(&mut self,world:&mut WorldChrMan,handles:&[FieldInsHandle],authority:bool,party:bool,flasks:Option<u8>,team:Option<&[(u64,u64)]>) {
        if self.affixes.is_empty(){return;}
        if let Err(error)=validate(self.empowered){if !self.missing{super::log(error);self.missing=true;}return;}
        let now=Instant::now();let tuning=self.tuning();
        let (pride,envy)=if self.empowered {(PRIDE+1,ENVY+1)}else{(PRIDE,ENVY)};
        let drank=team.map_or_else(||u64::from(matches!((self.flasks,flasks),(Some(before),Some(after)) if after<before)),|counts|self.drinks.take(counts));self.flasks=flasks;
        // 护盾：房主按时机展开；成员看到 Boss 身上出现护盾效果时开始 5 秒倒计时（提示用）
        let golden=self.affixes.contains(&Affix::Golden);
        let shield=authority&&golden&&match self.shield_at {Some(at)=>now>=at,None=>{self.shield_at=Some(now+Duration::from_secs(tuning.shield_every));false}};
        if shield {self.shield_at=Some(now+Duration::from_secs(tuning.shield_every));self.shield_until=Some(now+Duration::from_secs(5));}
        if !authority&&golden&&self.shield_until.is_none_or(|until|now>=until)
            &&handles.iter().filter_map(|h|world.chr_ins_by_handle(h)).any(|c|c.special_effect.entries().any(|e|e.param_id==SHIELD)) {
            self.shield_until=Some(now+Duration::from_secs(5));
        }
        // 本机玩家这一帧受到的伤害（腐败）；房主再算全队受到的伤害（吸血）
        let player_hp=world.main_player.as_ref().map(|p|p.chr_ins.modules.data.hp);
        let taken=match (self.player_hp,player_hp) {(Some(before),Some(after)) if after<before=>before-after,_=>0};self.player_hp=player_hp;
        let team_taken=if authority&&party {
            let now_hp:Vec<_>=world.player_chr_set.characters().map(|p|(p.chr_ins.field_ins_handle,p.chr_ins.modules.data.hp)).collect();
            let lost=now_hp.iter().map(|(h,hp)|self.players.iter().find(|(p,_)|p==h).map_or(0,|(_,before)|(before-hp).max(0))).sum::<i32>();
            self.players=now_hp;lost
        }else{taken};
        let own:Vec<FieldInsHandle>=world.main_player.as_ref().map(|p|p.chr_ins.field_ins_handle).into_iter().chain(world.summon_buddy_chr_set.characters().map(|c|c.field_ins_handle)).collect();
        let mut reflected=0i64;
        for handle in handles {
            let Some(chr)=world.chr_ins_by_handle_mut(handle) else{continue};
            if chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(){continue;}
            let index=match self.foes.iter().position(|f|f.handle==*handle) {Some(i)=>i,None=>{self.foes.push(Foe{handle:*handle,hp:chr.modules.data.hp,hurt:now,proud:false,reborn:false});self.foes.len()-1}};
            let foe=&mut self.foes[index];
            let (hp,max)=(chr.modules.data.hp,chr.modules.data.max_hp);
            // 荆棘：单人按 Boss 受到的伤害；联机只按自己与自己召唤物打的（看 last_hit_by），由各自在本机反弹给自己
            if hp<foe.hp&&self.affixes.contains(&Affix::Thorned)&&(!party||own.contains(&chr.last_hit_by)) {reflected+=i64::from(foe.hp-hp)*tuning.thorn_percent/100;}
            if hp<foe.hp {foe.hurt=now;}
            // 减伤、攻击等效果房主和成员都在本机挂上：效果不随网络同步，只有房主挂时成员那边的 Boss 更“软”，
            // 成员算出的伤害偏高，本机副本先被打到 0、卡住不倒。护盾、吸血、涅槃这些改 Boss 状态的仍只由房主做
            let present:Vec<i32>=chr.special_effect.entries().map(|e|e.param_id).collect();
            for id in self.affixes.iter().filter_map(|a|resident(*a,self.empowered)) {if !present.contains(&id){chr.apply_speffect(id,false);}}
            let regeneration=self.affixes.contains(&Affix::Regenerating)
                .then(||regeneration_effect(self.empowered,authority,now.duration_since(foe.hurt))).flatten();
            for id in [FIRST+30,FIRST+31] {
                if regeneration==Some(id) {if !present.contains(&id){chr.apply_speffect(id,false);}}
                else if present.contains(&id){chr.remove_speffect(id);}
            }
            if !authority {foe.hp=hp;continue;}
            if hp<foe.hp&&foe.proud {chr.remove_speffect(pride);foe.proud=false;}
            if self.affixes.contains(&Affix::Leeching)&&team_taken>0&&max>0 {
                chr.modules.data.hp=(i64::from(hp)+i64::from(max)*tuning.leech_permille/1000).min(i64::from(max)) as i32;
                chr.apply_speffect(LEECH_FLASH,false);
            }
            let hp=chr.modules.data.hp;
            foe.hp=hp;
            // 迅猛：持续把动作播放速度保持在 1.1 倍（强化 1.3 倍）
            if self.affixes.contains(&Affix::Swift)&&chr.modules.behavior.animation_speed<tuning.swift_speed {chr.modules.behavior.animation_speed=tuning.swift_speed;}
            if self.affixes.contains(&Affix::Proud)&&!foe.proud&&now.duration_since(foe.hurt)>=Duration::from_secs(tuning.proud_after) {chr.apply_speffect(pride,false);foe.proud=true;}
            if self.affixes.contains(&Affix::Envious)&&drank>0 {chr.apply_speffect(envy,false);}
            if shield {chr.apply_speffect(SHIELD,false);}
            if self.affixes.contains(&Affix::Reborn)&&!foe.reborn&&max>0&&i64::from(hp)*10<=i64::from(max) {
                chr.modules.data.hp=(i64::from(max)*tuning.reborn_percent/100) as i32;foe.hp=chr.modules.data.hp;foe.reborn=true;chr.apply_speffect(REBORN,false);
                super::log(format!("BOSS_AFFIX reborn handle={handle} hp={}/{max}",foe.hp));
            }
        }
        let Some(player)=world.main_player.as_mut().filter(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()) else{return};
        // 荆棘：反弹给本机玩家，一次最多 200，不会致死
        let reflected=reflected.min(THORN_CAP);
        if reflected>0 {
            let data=&mut player.chr_ins.modules.data;
            data.hp=(i64::from(data.hp)-reflected).max(1) as i32;self.player_hp=Some(data.hp);
        }
        // 腐败的：攻击附带猩红腐败，玩家每次受伤积累一次（词条强化积累两次）；同一次攻击的多段伤害 0.4 秒内只算一次。每个玩家在本机给自己挂
        if self.affixes.contains(&Affix::Rot)&&taken>0&&self.rot_at.is_none_or(|at|now>=at) {
            for _ in 0..tuning.rot_per_hit {player.chr_ins.apply_speffect(ROT_BUILDUP,true);}
            self.rot_at=Some(now+Duration::from_millis(400));
        }
    }
}
