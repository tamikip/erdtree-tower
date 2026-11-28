use std::{
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use eldenring::{
    cs::{
        BlockId, CSDebugChrCreatorInitData, ChrDebugSpawnRequest, ChrInsExt, ChrLoadStatus, EventWorldType,
        FieldInsHandle, GameMan, NpcParam, NpcThinkParam, SoloParamRepository, WorldChrMan,
    },
    position::HavokPosition,
};
use erdtree_tower::{
    CampChoice, Phase, Run,
    encounter::{Encounter, Observation, Outcome},
};
use fromsoftware_shared::{F32Vector4, FromStatic};
use super::{loot,native_talk::{flag,set_flag}};

fn encounter_state<T:Default>(cache:&mut Option<((u64,u8,i32),T)>,scope:Option<(u64,u8,i32)>,available:bool)->Option<&mut T> {
    let Some(scope)=scope else {*cache=None;return None;};
    if cache.as_ref().is_some_and(|(previous,_)|*previous!=scope) {*cache=None;}
    if !available {return None;}
    Some(&mut cache.get_or_insert_with(||(scope,T::default())).1)
}

const SPAWN_TIMEOUT: Duration = Duration::from_secs(15);
// [fix] A bound foe that never enters the fight (e.g. Kaiden placed outside the arena) used to stall
// victory forever. After the others die, an untouched full-HP survivor is retired after this long.
const STALL_TIMEOUT: Duration = Duration::from_secs(60);
pub static QUICK_END:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
/// 开局前在后台核对一次全部场地资源（第一次调用时开始），结果出来后返回缺失或不一致的文件
fn resource_check()->Option<&'static Vec<String>> {
    static RESULT:std::sync::OnceLock<Vec<String>>=std::sync::OnceLock::new();
    static STARTED:std::sync::Once=std::sync::Once::new();
    STARTED.call_once(||{std::thread::spawn(||{let bad=super::arenas::mismatched_resources();super::log(format!("RESOURCE_CHECK mismatched={bad:?}"));let _=RESULT.set(bad);});});
    RESULT.get()
}
/// 出发被拒（场地文件缺失或不一致）：再弹一次醒目提示，把这个文件排在最前
fn flag_resource_alert(state:&mut Runtime,error:&str) {
    let file=error.split_once('：').map_or(error,|(_,rest)|rest.split('，').next().unwrap_or(rest)).to_owned();
    let (id,mut files)=state.view.resource_alert.take().unwrap_or((0,Vec::new()));
    files.retain(|f|*f!=file);files.insert(0,file);
    state.view.resource_alert=Some((id+1,files));
}
/// 「跳过本关」键：渲染线程置位，游戏任务线程处理
pub static RECOVER:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
pub static SKIP:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
pub static PLAYER_READY:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
fn can_end_run(phase:Option<&Phase>,host:bool,blocked:bool)->bool {
    host&&!blocked&&phase.is_some_and(|p|!matches!(p,Phase::Dead|Phase::Won))
}
fn victory_healing_allowed(player:&eldenring::cs::PlayerIns)->bool {
    player.chr_ins.modules.data.hp>0&&!player.chr_ins.chr_flags1c5.death_flag()
        &&!super::seamless_revival::finished(player as *const _ as usize)
}
/// 战斗胜利的回复（祷誓者 10% 生命、苦修之誓）：写日志，返回给玩家看的提示（没有回复时为空）
/// 地图、奖励、誓约界面打开时玩家没法吃药解毒（界面关不掉），战斗里带出来的中毒、腐败会一路掉血到死：
/// 界面锁住操作期间（不在战斗、不在房间、不在传送）每秒清一次异常
fn clear_ailments_under_overlay(state:&mut Runtime,world:&mut WorldChrMan,game:&GameMan) {
    let journey=state.view.run.as_ref().is_some_and(|r|!matches!(r.phase,Phase::Dead|Phase::Won|Phase::Combat{..}));
    if !overlay_cleanse_due(super::input::CAPTURE.load(std::sync::atomic::Ordering::Relaxed)&&journey&&!game.warp_requested,state.ailment_cleared,Instant::now()) {return;}
    let Some(player)=world.main_player.as_mut().filter(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()) else {return;};
    state.ailment_cleared=Some(Instant::now());
    if let Err(error)=super::ailments::clear(&mut player.chr_ins){super::log_repeating(error);}
}
/// 「跳过本关」：战斗中视为胜利（敌人撤下、不掉卢恩），传送门不用摸，直接到三选一；联机只有房主能用，房主通知全队
fn request_skip(state:&mut Runtime)->String {
    if state.party.as_ref().is_some_and(|p|!p.host()) {return "只有房主可以跳过本关。".into();}
    if state.skip_target.is_some(){return "跳过请求已记录，等待场景或物品处理完成。".into();}
    let Some(run)=state.view.run.as_ref() else{return "尚未开始旅程，请先开始一局。".into();};
    let travelling=state.travel.as_ref().map(|t|t.0.node).or_else(||state.room.as_ref().map(|r|r.node));
    let target=match erdtree_tower::skip::Target::new(run,travelling){Ok(t)=>t,Err(e)=>return e};
    state.skip_target=Some(target);
    "已记录跳过请求，场景就绪后直接跳过当前节点。".into()
}
fn apply_skip(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan)->Result<bool,String> {
    let Some(target)=state.skip_target else{return Ok(false);};
    if QUICK_END.load(std::sync::atomic::Ordering::Relaxed)||game.warp_requested||game.save_slot<0
        ||state.material_award.is_some()||state.party_pending.is_some(){return Ok(false);}
    let Some(player)=world.main_player.as_ref().filter(|p|i32::from(p.current_block_id)>=0&&p.chr_ins.modules.data.hp>0) else{return Ok(false);};
    let owner=(unsafe {player.player_game_data.as_ref()}.character_id,game.save_slot);
    if state.hub.as_ref().is_some_and(|h|h.owner!=owner){return Err("角色已切换，取消旧跳过请求".into());}
    if super::hub::path(owner,"party-award").exists(){return Ok(false);}
    let Some(old)=state.view.run.as_ref() else{return Ok(false);};
    let block=i32::from(player.current_block_id);
    if state.travel.is_some()&&block!=super::arenas::for_run(old,state.party.is_some()).block
        ||state.room.as_ref().is_some_and(|room|block!=room.block){return Ok(false);}
    if old.regional_event.receipt.is_some()||old.regional_event.mausoleum.as_ref().is_some_and(|m|m.receipt.is_some()){return Ok(false);}
    let mut next=old.clone();
    if !next.skip_current(target)?{state.skip_target=None;return Ok(true);}
    let victory_unsettled=state.reward_wait.is_none()&&state.pending.as_ref().is_none_or(|p|!p.completed);
    if let Some(room)=state.room.as_mut(){room.suspend_dialogues()?;}
    if let Some(link)=state.party.as_mut().filter(|link|link.host()) {
        if link.event.forced_skip!=Some(target){link.skip_node(target)?;}
    }else if state.party.is_none()&&game.event_world_type==EventWorldType::Remote{return Ok(false);}
    record_checked(&next)?;
    if target.rewards&&victory_unsettled {
        let healing=heal_victory(world,&next);if !healing.is_empty(){super::log(healing);}
        if let Some(plan)=keep_summoned_ash(world,&next){state.ash_refunds=plan;state.ash_guard=Some(Instant::now());}
        give_elite_tear(world,&next,target.node);
        let bonus=erdtree_tower::boss_affixes::bonus_runes(&next,target.node);
        if let Some(player)=world.main_player.as_mut().filter(|_|bonus>0) {
            let data=unsafe {player.player_game_data.as_mut()};data.rune_count=data.rune_count.saturating_add(bonus).min(999_999_999);
        }
        record_fight(&next,Some(target.node),true);
    }
    stop_map_combat(state,world);
    if let Some(pending)=state.pending.take(){state.lingering=Some((pending,Instant::now()));}
    if let Some(room)=state.room.take(){room.retire(world);}
    if let Some(scene)=state.party_scene.take(){scene.retire(world);}
    if let Some(attendant)=state.attendant.take(){attendant.retire(world);}
    world.summon_buddy_manager.disappear_requested=true;super::spirits::set_active(false);
    if let Err(error)=clear_chariot(){super::log(error);}
    for flag in [45020970,45020971,45020972,35007990,45011859,super::map_guards::COMBAT] {
        if let Err(error)=super::native_talk::set_flag(flag,false){super::log(error);}
    }
    state.travel=None;state.reward_wait=None;state.reward_portal=Default::default();state.event_start=None;state.survival_engaged=None;
    super::custom_event_ui::close();super::custom_event_ui::set_survival_hud(None);
    state.descent=None;state.stormveil=None;state.chariot_clock=None;state.party_spirits=None;
    state.skip_target=None;state.skip_fight=None;state.skip_portal=false;state.command=None;
    state.view.run=Some(next);state.view.room=None;state.view.shared_event=false;state.view.busy=false;
    state.view.travelling=false;state.view.enemy_hp=None;state.view.ballots.clear();
    super::log(format!("SKIP completed: seed={} act={} node={} rewards={}",target.seed,target.act,target.node,target.rewards));
    report(state,if target.rewards{"已跳过战斗，请选择奖励。"}else{"已跳过当前节点，未额外发放奖励，请选择下一条路线。"});
    Ok(true)
}
/// 跳过请求在这一帧生效：本机还活着、敌人已经出现；联机还要等全队开战（房主才能判定胜利）
fn skip_fight_due(requested:Option<usize>,node:Option<usize>,bound:bool,dead:bool,party:Option<(bool,bool)>)->bool {
    requested.is_some()&&requested==node&&bound&&!dead&&party.is_none_or(|(host,fighting)|host&&fighting)
}
fn overlay_cleanse_due(captured:bool,last:Option<Instant>,now:Instant)->bool {
    captured&&last.is_none_or(|t|now.saturating_duration_since(t)>=Duration::from_secs(1))
}
fn heal_victory(world:&mut WorldChrMan,run:&Run)->String {
    let Some(player)=world.main_player.as_mut() else{return String::new();};
    let data=&player.chr_ins.modules.data;let (hp,fp)=(data.hp,data.fp);
    if !victory_healing_allowed(player) {
        super::log(format!("VICTORY_HEAL skipped hp={hp} dead={} revived={}",player.chr_ins.chr_flags1c5.death_flag(),super::seamless_revival::finished(&**player as *const _ as usize)));
        return String::new();
    }
    super::passives::heal_victory(&mut player.chr_ins,run);super::contracts::heal_victory(&mut player.chr_ins,run);
    let data=&player.chr_ins.modules.data;
    super::log(format!("VICTORY_HEAL class={:?} hp={hp}->{}/{} fp={fp}->{}/{}",run.class,data.hp,data.max_hp,data.fp,data.max_fp));
    let mut parts=vec![];
    if data.hp>hp {parts.push(format!("生命 +{}",data.hp-hp));}
    if data.fp>fp {parts.push(format!("专注 +{}",data.fp-fp));}
    if parts.is_empty() {String::new()}else{format!("胜利回复：{}。",parts.join("，"))}
}

#[derive(Default)]
struct HostDeath {epoch:Option<u64>,block:Option<i32>}
impl HostDeath {
    fn observe(&mut self,epoch:Option<u64>,player:Option<(i32,bool)>,warping:bool)->bool {
        if self.epoch!=epoch||epoch.is_none(){self.epoch=epoch;self.block=None;}
        if epoch.is_none(){return false;}
        if let Some((block,dead))=player {
            if dead&&self.block.is_none(){self.block=Some(block);}
            if !warping&&!dead&&self.block==Some(block){self.block=None;}
        }
        self.block.is_some_and(|block|warping||player.is_some_and(|(current,_)|current!=block))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Enter(usize),
    KeepCurrency,
    Camp(CampChoice),
    Story(usize),
    Claim(usize),
    Contract(usize),
    Gear(usize),
    SkipGear,
    SkipContract,
}

#[derive(Clone)]
pub struct View {
    pub shared_event:bool,
    pub ballots:Vec<erdtree_tower::party_vote::Ballot>,
    pub latencies:Vec<erdtree_tower::party_latency::Row>,
    pub run: Option<Run>,
    pub status: String,
    pub busy: bool,
    pub enemy_hp: Option<(i32, i32)>,
    pub prompt: u64,
    pub ready: bool,
    pub travelling: bool,
    pub room: Option<super::rooms::RoomView>,
    pub runes: u32,
    pub boss_affixes: Option<String>,
    /// Boss 词条的时机提示（黄金律法的护盾）：(文字, 是否为预警)
    pub boss_alert: Option<(String,bool)>,
    /// 当前 Boss 的词条（屏幕上方逐个上色、带动态效果）
    pub boss_affix_list: Vec<erdtree_tower::boss_affixes::Affix>,
    /// 苦难律法限时战斗的剩余秒数，超时后为负
    pub law_clock: Option<i64>,
    /// 身上词条装备的名称与词条（路线图的构筑面板）
    pub build: Vec<super::reward_ui::Gear>,
    /// 篝火营地的测伤假人统计（单人、在营地时才有）
    pub dummy: Option<super::dummy::View>,
    /// 构筑面板的数值加成（身上生效的特效按项连乘）
    pub stats: super::stats::Summary,
    /// 整合包的场地文件缺失或与 DLL 不一致：(提示序号, 文件)。开局检查一次、出发被拒时再提示，界面据此弹出醒目提示
    pub resource_alert: Option<(u64,Vec<String>)>,
}
fn initial_view()->View {
    View{shared_event:false,ballots:vec![],latencies:vec![],run:None,status:"进入角色后，从金色入口开始旅程。".into(),
        busy:false,enemy_hp:None,prompt:0,ready:false,travelling:false,room:None,runes:0,boss_affixes:None,boss_alert:None,boss_affix_list:vec![],law_clock:None,build:vec![],dummy:None,stats:Default::default(),resource_alert:None}
}

#[derive(Default)]
struct SpawnCheck {
    /// 单人调试刷怪的预期区块局部坐标，用于兜底同帧浮点原点切换导致的 32m 刷飞
    target_local: Option<[f32;3]>,
    bound_at: Option<Instant>,
    verified: bool,
    fixes: u8,
}
struct Foe {
    spec: super::enemies::Enemy,
    fight: Encounter<FieldInsHandle>,
    requested: Option<Instant>,
    entity_address: Option<usize>,
    /// 双王共命：在这里（尸体的位置）重新生成
    respawn: Option<[f32;3]>,
    spawn_gap:erdtree_tower::encounter::SpawnGap,
    /// 后续波次的出招保护期：从这只敌人绑定上算起（WAVE_GRACE 内禁止攻击，到点放开后记 None 并标记 graced）
    grace:Option<Instant>,graced:bool,
    spawn_protection:super::spawn_grace::HitProtection,
    spawn_check:SpawnCheck,
}
type MemberSpawnGaps=((u64,u64,u8,usize),Vec<erdtree_tower::encounter::SpawnGap>);
struct Pending {
    entity_base:Option<i32>,
    networked:bool,
    /// 是否跳过单人同场上限（SOLO_CAP=2）与分波等待，一次性全部出场（用于「坚持 30 秒」大混战）
    all_at_once:bool,
    spirits:super::spirits::Spirits,
    foes: Vec<Foe>,
    next_run: Run,
    block: i32,
    completed: bool,
    // [fix] Since when every remaining live foe has sat untouched at full HP after the others died.
    stall: Option<Instant>,
    godskin_pace:GodskinPace,
}
struct PartyScene {run:Run,block:i32,wave:u8,started:Instant,guards:Option<super::map_guards::Guards>,hunt:Option<super::map_scarabs::Hunt>}
impl PartyScene {
    fn retire(&self,world:&mut WorldChrMan) {
        if let Some(guards)=&self.guards {guards.retire();}
        if let Some(hunt)=&self.hunt {hunt.retire(world);}
    }
}

fn tick_party_scene(state:&mut Runtime,world:&mut WorldChrMan,game:&GameMan)->Result<(),String> {
    use erdtree_tower::party_events::Kind;
    state.view.shared_event=false;
    let Some(scene)=state.party_scene.as_mut() else{return Ok(());};
    let link=state.party.as_mut().ok_or("事件队伍通道未加载")?;
    link.arrived=!game.warp_requested&&world.main_player.as_ref().is_some_and(|p|i32::from(p.current_block_id)==scene.block&&p.chr_ins.modules.physics.standing_on_solid_ground);
    let Some(job)=link.event.job else{return Ok(());};
    if link.event.applied==Some(job.wave){return Ok(());}
    if matches!(job.kind,Kind::Scarabs(_)){return tick_party_scarabs(state,world,game);}
    let Kind::Guards(slot)=job.kind else{return Ok(());};
    let Phase::Node{id}=scene.run.phase else{return Err("事件场景进度无效".into());};
    let survival=scene.run.event_draw(id)==erdtree_tower::events::Draw::Survival;
    state.view.shared_event=true;state.command=None;
    super::custom_event_ui::close();
    if let Some(room)=state.room.as_mut(){room.suspend_dialogues()?;}
    state.pad_capture.update(false);super::input::CAPTURE.store(false,std::sync::atomic::Ordering::Relaxed);
    if scene.wave!=job.wave {
        if let Some(guards)=scene.guards.take(){guards.retire();}
        scene.wave=job.wave;scene.started=Instant::now();
    }
    if link.event.done.is_some() {
        let kills=if survival {scene.guards.as_ref().map_or(0,|g|g.dead_count(world))}else{0};
        if let Some(mut guards)=scene.guards.take() {
            if survival {guards.force_retire(world);} else {guards.retire();}
        }
        if survival {super::custom_event_ui::set_survival_hud(None);}
        let run=state.view.run.as_mut().ok_or("个人事件进度未加载")?;
        if job.player==link.config.player&&(run.phase!=(Phase::Node{id})||!run.regional_event.guards){return Err("个人守卫事件与结算不匹配".into());}
        if survival {
            if run.phase==(Phase::Node{id})&&run.regional_event.opened!=3 {
                let mut next=run.clone();
                if next.regional_event.opened==0&&!next.regional_event.guards {next.begin_survival()?;}
                if next.regional_event.guards {
                    let (reward,names)=grant_survival_rewards(world,&mut next,id,kills)?;
                    state.view.runes=next.gold;
                    record_checked(&next)?;*run=next;
                    let full=erdtree_tower::new_events::survival_full_clear(kills);
                    let loot_desc=if names.is_empty(){String::new()}else{format!("，战利品：{}",names.join("、"))};
                    let msg=format!("生存大混战胜利（击杀 {kills}/{}，{}）：获得 {reward} 卢恩{loot_desc}。请从传送门离开。",erdtree_tower::new_events::SURVIVAL_COUNT,if full{"全清额外奖励"}else{"成功生还"});
                    super::log(format!("SURVIVAL_PARTY victory kills={kills} full={full} runes={reward} items={names:?}"));
                    if let Some(room)=state.room.as_mut(){room.view.message=msg.clone();}
                    state.view.status=msg;state.view.prompt=state.view.prompt.wrapping_add(1);
                }
            }
            if matches!(link.event.request,Some(Kind::Guards(_))){link.event.request=None;}
        } else {
            if run.phase==(Phase::Node{id})&&run.regional_event.guard_slot==Some(slot) {
                run.regional_event.guards=false;run.regional_event.guard_slot=None;record_checked(run)?;
                if matches!(link.event.request,Some(Kind::Guards(_))){link.event.request=None;}
            }
        }
        let healed=heal_victory(world,&scene.run);if !healed.is_empty(){super::log(&healed);state.view.status=healed;state.view.prompt=state.view.prompt.wrapping_add(1);}
        // 召唤师「群灵」：与单人的守卫战相同，胜利时召唤物还活着就补回这次用掉的骨灰
        if let Some(plan)=keep_summoned_ash(world,&scene.run) {state.ash_refunds=plan;state.ash_guard=Some(Instant::now());}
        world.summon_buddy_manager.disappear_requested=true;
        super::log("ASH_GROUP event guard victory: native summon dismissal requested");
        link.event.applied=Some(job.wave);state.view.shared_event=false;return Ok(());
    }
    if game.warp_requested||world.main_player.as_ref().is_none_or(|p|i32::from(p.current_block_id)!=scene.block){return Ok(());}
    if scene.guards.is_none(){
        let layer=scene.run.nodes[id].layer*11/(scene.run.floors()-1);
        let guards=if survival {
            super::map_guards::Guards::new(super::enemies::survival_roster(scene.run.seed,id,layer,scene.run.act))?
        }else{
            let altar=scene.run.altar(id);
            let roster=|s|super::enemies::event_roster(scene.run.seed,id,layer,scene.run.act,altar,s);
            let earlier=(0..if altar {0} else {slot}).flat_map(roster).collect::<Vec<_>>();
            super::map_guards::Guards::after(&earlier,roster(slot))?
        };
        scene.guards=Some(guards);
    }
    let guards=scene.guards.as_mut().unwrap();
    guards.prepare(world)?;
    if guards.ready(){link.event.bound=Some(job.wave);}
    if link.host()&&!link.event.hunting&&link.gate.event_bound(job.wave){link.event_hunt(job.wave)?;}
    let defeated=guards.poll(world,link.event.hunting,link.host())?;
    if survival {
        let total=guards.total_count();
        let hud=if let Some(elapsed)=guards.engaged_elapsed() {
            super::custom_event_ui::SurvivalHud {
                remaining_secs:(erdtree_tower::new_events::SURVIVAL_SECONDS as f32-elapsed.as_secs_f32()).max(0.0),
                kills:guards.dead_count(world),total,preparing_secs:None,
            }
        } else {
            let prep=super::travel::PREPARATION.saturating_sub(scene.started.elapsed()).as_secs().max(1);
            super::custom_event_ui::SurvivalHud {
                remaining_secs:erdtree_tower::new_events::SURVIVAL_SECONDS as f32,
                kills:0,total,preparing_secs:Some(prep),
            }
        };
        super::custom_event_ui::set_survival_hud(Some(hud));
    }
    let survived=survival&&guards.engaged_elapsed().is_some_and(|e|e>=Duration::from_secs(erdtree_tower::new_events::SURVIVAL_SECONDS));
    if link.host()&&(defeated||survived)&&link.gate.event_bound(job.wave){link.event_result(job.wave,0)?;}
    if !link.event.hunting&&scene.started.elapsed()>Duration::from_secs(90){
        if survival {super::custom_event_ui::set_survival_hud(None);}
        guards.retire();return Err("事件守卫未能全员加载，已停止激活；请双方更新同一补丁后重新组队".into());
    }
    Ok(())
}

fn tick_party_scarabs(state:&mut Runtime,world:&mut WorldChrMan,game:&GameMan)->Result<(),String> {
    let scene=state.party_scene.as_mut().ok_or("事件场景未加载")?;
    let link=state.party.as_mut().ok_or("事件队伍未加载")?;
    let job=link.event.job.ok_or("事件波次未加载")?;
    let erdtree_tower::party_events::Kind::Scarabs(attempt)=job.kind else{return Err("事件类型不匹配".into());};
    let Phase::Node{id}=scene.run.phase else{return Err("事件节点无效".into());};
    let base=link.event.scarab_base(job.player,attempt)?;
    state.view.shared_event=true;state.command=None;
    if scene.wave!=job.wave {
        if let Some(hunt)=scene.hunt.take(){hunt.retire(world);}
        scene.wave=job.wave;scene.started=Instant::now();
        scene.hunt=Some(super::map_scarabs::Hunt::new(base)?);
    }
    if let Some(killed)=link.event.done {
        let mine=link.event.scarab_rewards(link.config.player);
        let payer=job.player==link.config.player;
        super::log(format!("Party scarabs settle: wave={} payer={payer} killed={killed:03b} mine={mine:03b} owners={:?} aborted={}",job.wave,link.event.scarab_owners,link.event.aborted));
        if payer||mine!=0&&!link.event.aborted {
            let run=state.view.run.as_mut().ok_or("个人事件未加载")?;
            if run.phase!=(Phase::Node{id})||!run.regional_event.scarabs||payer&&run.regional_event.attempts!=attempt{return Err("个人粪金龟次数与结算不匹配".into());}
            if payer&&link.event.aborted {
                let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
                let data=unsafe {player.player_game_data.as_mut()};
                let refunded=data.rune_count.checked_add(erdtree_tower::regional_events::SCARAB_PRICE).ok_or("退款超过卢恩上限")?;
                write_award_journal(&award_path()?,run)?;
                let mut next=run.clone();next.regional_event.attempts-=1;next.gold=refunded;
                data.rune_count=refunded;state.view.runes=refunded;record_checked(&next)?;*run=next;
                finish_party_award()?;
            }else if mine!=0 {
                write_award_journal(&award_path()?,run)?;
                let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
                for slot in 0..3 {if mine&(1<<slot)!=0 {
                    // 不是付钱的人：种子里再混进付钱的人，免得两个人各自第 1 次的两波抽到同一件
                    let seed=if payer {run.seed^0x5ca4ab}else{run.seed^0x5ca4ab^job.player.rotate_left(17)};
                    super::loot::regional_reward(unsafe {player.player_game_data.as_mut()},run,seed,id*16+usize::from(attempt)*3+slot)?;
                }}
                record_checked(run)?;finish_party_award()?;
            }
        }
        if state.ash_guard.is_none()&&let Some(run)=state.view.run.as_ref()&&let Some(plan)=keep_summoned_ash(world,run) {
            state.ash_refunds=plan;state.ash_guard=Some(Instant::now());
        }
        world.summon_buddy_manager.disappear_requested=true;
        if let Some(hunt)=scene.hunt.take(){hunt.retire(world);}
        link.event.applied=Some(job.wave);state.view.shared_event=false;return Ok(());
    }
    if game.warp_requested||world.main_player.as_ref().is_none_or(|p|i32::from(p.current_block_id)!=scene.block){return Ok(());}
    let hunt=scene.hunt.as_mut().unwrap();
    hunt.prepare(world)?;
    if hunt.ready(){link.event.bound=Some(job.wave);}
    if link.host()&&!link.event.hunting&&link.gate.event_bound(job.wave){link.event_hunt(job.wave)?;}
    if link.event.hunting {
        if !hunt.active(){hunt.start()?;}
        let members:Vec<_>=std::iter::once(link.config.player).chain(link.config.peers.iter().map(|p|p.player)).collect();
        hunt.poll(world,link.config.player,&members)?;
        link.event.scarab_claim=Some(hunt.claim(job.wave,link.config.player));
        if link.host(){if let Some(killed)=hunt.result(){
            if let Some(owners)=link.gate.scarab_owners(job.wave,killed,hunt.owners()) {
                super::log(format!("SCARAB_OWNERS reconciled wave={} killed={killed:03b} host={:?} final={owners:?}",job.wave,hunt.owners()));
                link.event_result_owned(job.wave,killed,owners)?;
            }
        }}
    }
    if scene.started.elapsed()>Duration::from_secs(90)&&link.host()&&link.event.done.is_none(){link.event_abort(job.wave)?;}
    if scene.started.elapsed()>Duration::from_secs(120)&&link.event.done.is_none(){return Err("未收到粪金龟结算，已暂停重发".into());}
    Ok(())
}

struct MaterialAward {
    action:Action,old:Run,owner:(u32,i32),item:u32,before:u64,started:Instant,path:std::path::PathBuf,
}
impl MaterialAward {
    fn matches(&self,owner:(u32,i32),run:Option<&Run>)->bool {
        owner==self.owner&&run.is_some_and(|r|(r.seed,r.act,&r.phase)==(self.old.seed,self.old.act,&self.old.phase))
    }
}
struct Runtime {
    archery:super::shadow_archery::State,
    suspension:super::suspension::Flow,
    skip_target:Option<erdtree_tower::skip::Target>,
    material_award:Option<MaterialAward>,
    radahn_probe:Option<super::radahn_probe::Probe>,
    ember_notice:super::ember_notice::Notice,
    load_trace:super::diagnostics::LoadTrace,
    landing_trace:super::landing_probe::Trace,
    drawing:erdtree_tower::map_drawing::Sync,
    drawing_owner:Option<(u32,i32)>,
    rest_end:super::rest_end::RestEnd,
    diagnostics:super::diagnostics::Changes,
    diagnostic_probe:Option<Instant>,
    host_death:HostDeath,
    party_death:super::party_death::Death,
    party_death_wave:Option<(u64,u8)>,
    party_spirits:Option<((u64,u8,i32),super::spirits::Spirits)>,
    party_scene:Option<PartyScene>,
    party_pending:Option<super::steam_messages::Pending>,
    reconnect:Option<super::reconnect::Pending>,
    party:Option<super::party::Link>,
    departure:Option<super::party::Departure>,
    party_error:Option<String>,
    attendant:Option<super::rooms::Room>,
    hub: Option<super::hub::Hub>,
    last_save: Option<Instant>,
    last_saved: Option<Run>,
    view: View,
    command: Option<Action>,
    identity: Option<(u32, i32)>,
    hub_failed: Option<((u32,i32),Instant)>,
    ashes_checked: Option<Instant>,
    /// 召唤师胜利后的一小段时间：召唤物离场后才变成“用掉”的骨灰也补回，而不是清理掉
    ash_guard: Option<Instant>,
    ash_tracking:bool,
    ash_before:Vec<(u32,u32)>,
    ash_refunds:Vec<super::spirits::refund::Refund>,
    ash_revival_refund:bool,
    combat_scene:Option<(u64,u8,i32,usize)>,
    larval_retry: Option<Instant>,
    /// 本次游戏里已经为第几幕发过泪滴幼体：计数万一读不到，也只发一个，不会每 2 秒再发
    larval_sent: Option<(u64,u8)>,
    deepen_retry: Option<Instant>,
    build_checked: Option<Instant>,
    boss_fight: super::boss_affixes::Fight,
    dual_fight: super::dual_bosses::Fight,
    boss_guard: super::dual_bosses::LockGuard,
    /// 双生的灵唤铃：待补回的骨灰与开始补的时间
    twin_refund: Option<(u32,Instant)>,
    growth: super::scaling::Growth,
    law_fight: super::law::Fight,
    weapon_traits: super::weapon_traits::State,
    ember_fight: super::embers::Fight,
    ember_practice: super::embers::Fight,
    bout: super::debug_menu::Bout,
    pending: Option<Pending>,
    spawn_used: bool,
    dummy: Option<super::dummy::Dummy>,
    last_player_probe: Option<(u32, i32, i32, u8, bool)>,
    pad_capture: super::input::PadCapture,
    travel: Option<(super::travel::Travel, Instant, i32, i32)>,
    flasks: Option<super::flasks::Flasks>,
    flask_grant_requested: Option<Instant>,
    reward_wait: Option<(Instant, Run)>,
    reward_portal:super::reward_delay::Completion,
    /// 联机成员收到胜利后待撤下的敌人实体：尸体留到传送时再撤；(联机场次, 实体, 收到胜利的时间, 是否已撤下不同步的活敌人)
    victory_cleanup:Option<(u64,Vec<u32>,Instant,bool)>,
    /// 本局结束（死亡或通关）时这场战斗的敌人：先留在场上，复活或 15 秒后再撤，不在死亡瞬间消失
    lingering:Option<(Pending,Instant)>,
    /// 同一次死亡只扣一次赐福，等待原生复活后自动返回原房间。
    death_retry:bool,
    /// 上次在地图 / 奖励界面里清除中毒、腐败等异常的时间
    ailment_cleared:Option<Instant>,
    /// 跳过本关：这个战斗节点一开战（联机为全队开战）就判定胜利
    skip_fight:Option<usize>,
    /// 跳过本关：传送门一亮就直接领奖（联机由房主通知全队）
    skip_portal:bool,
    retry_ready_since:Option<Instant>,
    resume_combat:Option<usize>,
    retry_rescue:super::retry_rescue::Rescue,
    member_spawn_gaps:Option<MemberSpawnGaps>,
    /// 徘徊赐福实物道具：背包数量跟随旅程剩余次数
    retry_item:super::retry_item::Sync,
    action_guard: super::input::ActionGuard,
    last_blessing_sync: Option<Instant>,
    room: Option<super::rooms::Room>,
    event_start: Option<Instant>,
    survival_engaged: Option<Instant>,
    descent: Option<(Instant,i32,i32,bool)>,
    stormveil: Option<(Instant,i32,i32,bool)>,
    stormveil_probe:Option<Instant>,
    stormveil_barrier:super::parkour_barrier::Barrier,
    chariot_clock:Option<ChariotClock>,
    chariot_chase:super::chariot::Chase,
}

fn fresh_runtime()->Runtime {
    super::suspension::notice("");super::suspension::REQUEST.store(false,std::sync::atomic::Ordering::Relaxed);
    super::seamless_revival::cancel();
    let (party,party_error)=match super::party::Link::load(){Ok(p)=>(p,None),Err(e)=>(None,Some(e))};
    Runtime {
        archery:Default::default(),
        suspension:Default::default(),
        skip_target:None,
        material_award:None,
        radahn_probe:None,
        drawing:Default::default(),drawing_owner:None,
        rest_end:Default::default(),
        diagnostics:Default::default(),
        diagnostic_probe:None,
        host_death:Default::default(),
        party_death:Default::default(),
        party_death_wave:None,
        party_spirits:None,
        party_scene:None,
        party,party_error,party_pending:None,reconnect:None,departure:None,
        attendant:None,
        hub: None,
        last_save: None,
        last_saved: None,
        view: initial_view(),
        command: None,
        identity: None,
        hub_failed: None,
        ashes_checked: None,
        ash_guard: None,
        ash_tracking:false,
        ember_notice:Default::default(),
        ash_before:vec![],
        ash_refunds:vec![],
        ash_revival_refund:false,
        combat_scene:None,
        larval_retry: None,
        larval_sent: None,
        deepen_retry: None,
        build_checked: None,
        boss_fight: Default::default(),
        dual_fight: Default::default(),
        boss_guard: Default::default(),
        twin_refund: None,
        growth: Default::default(),
        law_fight: Default::default(),
        weapon_traits: Default::default(),
        ember_fight: Default::default(),
        ember_practice: Default::default(),
        bout: Default::default(),
        pending: None,
        spawn_used: false,
        dummy: None,
        last_player_probe: None,
        load_trace:Default::default(),
        landing_trace:Default::default(),
        pad_capture: Default::default(),
        travel: None,
        flasks: None,
        flask_grant_requested: None,
        reward_wait: None,
        reward_portal:Default::default(),
        victory_cleanup:None,
        lingering:None,
        death_retry:false,
        ailment_cleared:None,
        skip_fight:None,skip_portal:false,
        retry_ready_since:None,
        resume_combat:None,
        retry_rescue:Default::default(),
        member_spawn_gaps:None,
        retry_item:Default::default(),
        action_guard: Default::default(),
        last_blessing_sync: None,
        room: None,
        event_start: None,
        survival_engaged: None,
        descent: None,
        stormveil: None,
        stormveil_probe:None,
        stormveil_barrier:Default::default(),
        chariot_clock:None,
        chariot_chase:Default::default(),
    }
}
static STATE: LazyLock<Mutex<Runtime>> = LazyLock::new(||Mutex::new(fresh_runtime()));
static UI:LazyLock<Mutex<UiBridge>>=LazyLock::new(||Mutex::new(UiBridge{view:Some(initial_view()),owner:None,pending:None}));

#[derive(Clone,PartialEq,Eq)]
struct ActionContext {
    owner:Option<(u32,i32)>,seed:u64,act:u8,phase:Phase,visited:Vec<usize>,
    contract:Option<erdtree_tower::contracts::State>,
}
impl ActionContext {
    fn from_view(view:&View,owner:Option<(u32,i32)>)->Option<Self> {
        let run=view.run.as_ref()?;
        Some(Self{owner,seed:run.seed,act:run.act,phase:run.phase.clone(),visited:run.visited.clone(),contract:run.contract.clone()})
    }
}
struct QueuedAction {action:Action,context:ActionContext}
struct UiBridge {view:Option<View>,owner:Option<(u32,i32)>,pending:Option<QueuedAction>}
fn accepts_ui_action(view:&View)->bool {
    view.ready&&!view.busy&&!view.travelling&&!view.shared_event&&view.room.is_none()
}
impl UiBridge {
    fn publish(&mut self,mut view:View,owner:Option<(u32,i32)>) {
        if self.pending.as_ref().is_some_and(|pending|!accepts_ui_action(&view)
            ||ActionContext::from_view(&view,owner).as_ref()!=Some(&pending.context)) {self.pending=None;}
        view.busy|=self.pending.is_some();
        self.view=Some(view);self.owner=owner;
    }
    fn submit(&mut self,action:Action)->bool {
        let Some(view)=self.view.as_mut().filter(|view|accepts_ui_action(view)) else{return false;};
        if self.pending.is_some(){return false;}
        let Some(context)=ActionContext::from_view(view,self.owner) else{return false;};
        self.pending=Some(QueuedAction{action,context});view.busy=true;true
    }
    fn take(&mut self,current:&View,owner:Option<(u32,i32)>,occupied:bool)->Option<Action> {
        let pending=self.pending.take()?;
        (!occupied&&accepts_ui_action(current)
            &&ActionContext::from_view(current,owner).as_ref()==Some(&pending.context)).then_some(pending.action)
    }
}
fn owner_changed(previous:Option<(u32,i32)>,current:(u32,i32))->bool {previous.is_some_and(|owner|owner!=current)}

pub fn view() -> Option<View> {
    UI.lock().ok().and_then(|ui|ui.view.clone())
}
pub fn with_view<R>(f:impl FnOnce(&View)->R)->Option<R> {
    UI.lock().ok().and_then(|ui|ui.view.as_ref().map(f))
}

#[derive(Default)]
struct MapBridge {
    owner:Option<(u32,i32)>,
    canvas:Option<erdtree_tower::map_drawing::Canvas>,
    pending:std::collections::VecDeque<(erdtree_tower::map_drawing::Scope,Option<(u32,i32)>,erdtree_tower::map_drawing::Edit)>,
}
static MAP:LazyLock<Mutex<MapBridge>>=LazyLock::new(||Mutex::new(MapBridge::default()));
pub fn map_canvas()->Option<erdtree_tower::map_drawing::Canvas> {MAP.lock().ok().and_then(|map|map.canvas.clone())}
pub fn submit_map_edit(scope:erdtree_tower::map_drawing::Scope,edit:erdtree_tower::map_drawing::Edit)->bool {
    let allowed=UI.lock().is_ok_and(|ui|ui.view.as_ref().is_some_and(|v|accepts_ui_action(v)
        &&v.run.as_ref().is_some_and(|r|r.phase==Phase::Map&&erdtree_tower::map_drawing::Scope::of(r)==scope)));
    if !allowed||!edit.valid(){return false;}
    let Ok(mut map)=MAP.lock() else{return false;};
    if map.pending.len()>=erdtree_tower::map_drawing::MAX_PENDING||map.canvas.as_ref().is_none_or(|c|c.scope!=scope){return false;}
    let owner=map.owner;map.pending.push_back((scope,owner,edit));true
}
fn sync_map(state:&mut Runtime) {
    let scope=state.view.run.as_ref().map(erdtree_tower::map_drawing::Scope::of);
    if state.drawing_owner!=state.identity {
        state.drawing.set_scope(None);
        if let Some(link)=&mut state.party {link.drawing.set_scope(None);}
        state.drawing_owner=state.identity;
    }
    state.drawing.set_scope(scope);
    if let Some(link)=&mut state.party {link.drawing.set_scope(scope);}
    let Ok(mut map)=MAP.lock() else{return;};
    for (queued,owner,edit) in map.pending.drain(..) {
        if Some(queued)!=scope||owner!=state.identity||!accepts_ui_action(&state.view)
            ||state.view.run.as_ref().is_none_or(|r|r.phase!=Phase::Map){continue;}
        let accepted=if let Some(link)=&mut state.party {
            link.stopped.is_none()&&link.drawing.submit(link.config.player,edit,link.host())
        }else{state.drawing.submit(1,edit,true)};
        if !accepted {super::log("MAP_DRAWING edit rejected: canvas or queue full");}
    }
    map.owner=state.identity;
    map.canvas=state.party.as_ref().map_or_else(||state.drawing.canvas.clone(),|link|link.drawing.preview(link.config.player));
}

pub fn submit(action: Action) {
    let accepted=UI.lock().is_ok_and(|mut ui|ui.submit(action));
    super::log(format!("ACTION {} action={action:?}",if accepted{"queued"}else{"rejected"}));
}

fn normalized_hp(hp: i32, max_hp: i32) -> u32 {
    if hp <= 0 || max_hp <= 0 {
        return 0;
    }
    ((hp as u64 * 100).div_ceil(max_hp as u64)).min(100) as u32
}

fn start_allowed(run: &Run, id: usize, busy: bool) -> Result<(), String> {
    if busy || !run.available().contains(&id) {
        return Err("节点不可达，或仍在处理上一场战斗。".into());
    }
    Ok(())
}

static SESSION_PATH: LazyLock<Mutex<Option<std::path::PathBuf>>> = LazyLock::new(||Mutex::new(None));
fn record(run: &Run) {
    if let Err(error)=record_checked(run){super::log(format!("Session recording failed: {error}"));}
}
fn record_fp(fp:i32) {
    static LAST:Mutex<Option<(std::path::PathBuf,i32)>>=Mutex::new(None);
    let Some(path)=SESSION_PATH.lock().ok().and_then(|p|p.clone()).map(|p|p.with_extension("fp")) else{return;};
    let Ok(mut last)=LAST.lock() else{return;};
    if last.as_ref().is_some_and(|(p,v)|*p==path&&*v==fp){return;}
    if std::fs::write(&path,fp.to_string()).is_ok(){*last=Some((path,fp));}
}
pub(super) fn record_checked(run: &Run)->Result<(),String> {
    run.encode().and_then(|data| {
        let path = SESSION_PATH.lock().ok().and_then(|p|p.clone()).unwrap_or_else(||super::LOG_PATH.with_file_name("tower-session.json"));
        let temporary=path.with_extension("tmp");
        if let Ok(player)=unsafe {eldenring::cs::PlayerIns::local_player()} {record_fp(player.chr_ins.modules.data.fp);}
        std::fs::write(&temporary,data).and_then(|_|std::fs::rename(temporary,path)).map_err(|error|error.to_string())
    })
}

fn report(state: &mut Runtime, message: impl Into<String>) {
    let message = message.into();
    if state.view.status == message {
        return;
    }
    state.view.status = message;
    state.view.prompt = state.view.prompt.wrapping_add(1);
    super::log(&state.view.status);
}

fn publish_bound_combat(view:&mut View,next:&Run,bound:bool)->bool {
    if !bound||!view.travelling||!matches!(next.phase,Phase::Combat{..}) {return false;}
    view.run=Some(next.clone());view.travelling=false;view.busy=false;true
}

fn stop_map_combat(state:&Runtime,world:&mut WorldChrMan) {
    if let Some(hunter)=state.room.as_ref().and_then(|room|room.hunter.as_ref()){hunter.retire();}
    let _=super::native_talk::set_flag(30097990,false);
    let _=super::native_talk::set_flag(10007990,false);
    let _=super::native_talk::set_flag(45007990,false);
    let _=super::native_talk::set_flag(45007991,false);
    let _=super::native_talk::set_flag(45007992,false);
    let _=super::native_talk::set_flag(45007993,false);
    if let Some(scene)=&state.party_scene {scene.retire(world);}
    let _=super::native_talk::set_flag(super::enemies::MAP_COMBAT_FLAG,false);
    if let Some(pending)=&state.pending {retire_group(pending,world);}
    if let Some(run)=&state.view.run && let Phase::Combat{id}=run.phase {
        if let Ok(ids)=super::enemies::map_entities(&super::enemies::node_roster(run,id)) {
            for entity in ids {if let Err(error)=super::enemies::request_map_entity(entity,false){super::log(error);}}
        }
    }
}
pub struct ShopDeparture {
    seed:u64,act:u8,visited:Vec<usize>,node:usize,last_reason:Option<&'static str>,queued_at:Instant,
}
fn departure_grounded(standing_solid:bool,touching_solid:bool,touching_ground:bool,falling:bool,allow_dynamic_fallback:bool)->bool {
    standing_solid || (!falling && (touching_solid || touching_ground || allow_dynamic_fallback))
}
fn shop_departure_wait(warp:bool,player:Option<(bool,bool,bool)>)->Option<&'static str> {
    match (warp,player) {
        (true,_)=>Some("warp"),
        (_,None)=>Some("loading"),
        (_,Some((false,_,_)))=>Some("dead"),
        (_,Some((_,false,_)))=>Some("airborne"),
        (_,Some((_,_,true)))=>Some("invincible"),
        _=>None,
    }
}
#[test]
fn shop_entry_waits_until_all_transient_checks_clear() {
    assert_eq!(shop_departure_wait(true,None),Some("warp"));
    assert_eq!(shop_departure_wait(false,None),Some("loading"));
    assert_eq!(shop_departure_wait(false,Some((true,false,true))),Some("airborne"));
    assert_eq!(shop_departure_wait(false,Some((true,true,true))),Some("invincible"));
    assert_eq!(shop_departure_wait(false,Some((true,true,false))),None);
    assert!(departure_grounded(true,false,false,false,false));
    assert!(departure_grounded(false,true,false,false,false),"mausoleum/dynamic collision while not falling");
    assert!(departure_grounded(false,false,true,false,false),"touching ground while not falling");
    assert!(!departure_grounded(false,false,false,false,false));
    assert!(departure_grounded(false,false,false,false,true),"fallback after brief non-falling wait");
    assert!(!departure_grounded(false,true,true,true,true),"never allow warp while genuinely falling");
}
fn enter_party_node(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan,node:usize,allow_dynamic_fallback:bool)->Result<(),String>{
    super::native_talk::set_flag(super::enemies::MAP_COMBAT_FLAG,false)?;
    if let Some(scene)=state.party_scene.take(){scene.retire(world);}
    state.view.shared_event=false;
    request_travel_with(state,world,game,node,allow_dynamic_fallback)
}
fn tick_shop_departure(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan)->Result<(),String>{
    let Some(link)=state.party.as_mut() else{return Ok(());};
    if link.stopped.is_some()||link.defeated||state.skip_target.is_some(){link.event.shop_departure=None;return Ok(());}
    let Some(pending)=link.event.shop_departure.as_mut() else{return Ok(());};
    let allow_dynamic_fallback=pending.queued_at.elapsed()>=Duration::from_millis(1500);
    let player=world.main_player.as_ref().filter(|p|i32::from(p.current_block_id)>=0).map(|p|{
        let phys=&p.chr_ins.modules.physics;
        let grounded=departure_grounded(phys.standing_on_solid_ground,phys.touching_solid_ground,phys.is_touching_ground,phys.is_falling,allow_dynamic_fallback);
        (p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag(),grounded,p.chr_ins.chr_flags1c5.is_invincible())
    });
    if let Some(reason)=shop_departure_wait(game.warp_requested,player){
        if pending.last_reason!=Some(reason){
            pending.last_reason=Some(reason);
            super::log(format!("PARTY_DEPARTURE_WAIT epoch={} node={} reason={reason}",link.epoch,pending.node));
            report(state,"已确认出发，等待角色站稳后自动传送……");
        }
        return Ok(());
    }
    let pending=link.event.shop_departure.take().unwrap();
    if !state.view.run.as_ref().is_some_and(|run|run.seed==pending.seed&&run.act==pending.act&&run.phase==Phase::Map
        &&run.visited==pending.visited&&run.available().contains(&pending.node)){
        return Err("队友路线与房主不同，未执行节点传送".into());
    }
    enter_party_node(state,world,game,pending.node,allow_dynamic_fallback)
}
fn apply_party_order(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan,order:erdtree_tower::party_game::Order)->Result<(),String> {
    use erdtree_tower::party_game::Order;
    if matches!(order,Order::Enter{epoch,..}|Order::BossTest{epoch,..}|Order::EventTest{epoch,..} if epoch==state.party.as_ref().unwrap().epoch){return Ok(());}
    if let Order::EventTest{seed,act,visited,draw,..}=&order {
        if !super::debug_menu::enabled()||!state.view.run.as_ref().is_some_and(|r|r.seed==*seed&&r.act==*act&&r.visited==*visited){return Err("事件测试未获本机调试授权或旅程不同步".into());}
        prepare_event_test(state,world,game,*draw)?;
    }
    if let Order::BossTest{seed,test,..}=&order {
        if !super::debug_menu::enabled()||!state.view.run.as_ref().is_some_and(|r|r.seed==*seed){return Err("Boss 测试未获本机调试授权或旅程不同步".into());}
        prepare_boss_test(state,world,game,*test)?;
    }
    if let Order::Enter{act,ref visited,node,..}=order {
        let run=state.view.run.as_ref().ok_or("队友旅程尚未开启")?;
        if run.act!=act||&run.visited!=visited||!run.available().contains(&node){return Err("队友路线与房主不同，未执行传送".into());}
    }
    if !state.party.as_mut().unwrap().accept(&order)?{return Ok(());}
    match order {
        Order::SkipNode{target,..}=>{state.skip_target=Some(target);state.command=None;Ok(())},
        Order::BossTest{test,..}=>begin_boss_test(state,world,game,test),
        Order::EventTest{draw,..}=>begin_event_test(state,world,game,draw),
        Order::MausoleumOpen{..}=>{
            if super::mausoleum::active(state.view.run.as_ref()) {
                let mut next=state.view.run.clone().unwrap();
                let copy=next.regional_event.mausoleum.as_mut().unwrap();
                copy.observe_flow(true,true);super::mausoleum::restore_flow(copy)?;
                record_checked(&next)?;state.view.run=Some(next);
            }
            Ok(())
        },
        Order::Law{seed,law,endless,..}=>{
            let run=state.view.run.as_ref().ok_or("律法同步时旅程尚未开启")?;
            if run.seed!=seed{return Err("律法同步路线不一致".into());}
            if run.law==law&&run.endless==endless{return Ok(());}
            let mut next=run.clone();next.set_law(law)?;next.set_endless(endless)?;
            if let Some(flasks)=state.flasks.as_mut() {
                let data=unsafe {world.main_player.as_mut().ok_or("律法同步时角色未加载")?.player_game_data.as_mut()};
                flasks.set_bonus(data,next.class,next.flask_bonus,next.total_flask_cut())?;
                flasks.restore(data,next.flasks,next.flask_blue)?;
                flasks.stamp(&mut next);next.flasks=next.flasks.min(next.flask_limit);next.clamp_flask_blue();
            }
            record_checked(&next)?;state.view.run=Some(next);
            report(state,format!("已同步房主苦难律法：{} 分，律法码 {}{}",law.score(),law.code(),if endless{"（无尽模式）"}else{""}));Ok(())
        },
        Order::ForceEnd=>{
            super::log("HOME_SYNC received host force end; scheduling terminal settlement");
            QUICK_END.store(true,std::sync::atomic::Ordering::Relaxed);Ok(())
        },
        Order::Reseed{seed,..}=>{
            // 房主用固定种子开局：已经开了新旅程（还没出发）的队员按新种子重排路线；还在大赐福的开局时自然对齐
            if let Some(run)=state.view.run.as_ref() {
                let mut next=run.clone();next.align_party_start(seed)?;
                record_checked(&next)?;state.view.run=Some(next);
            }
            report(state,format!("房主设定了本局种子 {seed}，路线已同步。"));Ok(())
        },
        Order::NewRun{..}=>{
            if state.view.run.is_some()||!state.hub.as_ref().is_some_and(|h|h.active) {return Err("新局同步时角色尚未返回大赐福".into());}
            stop_map_combat(state,world);
            state.pending=None;state.party_scene=None;state.party_spirits=None;state.room=None;state.attendant=None;
            state.reward_wait=None;state.reward_portal=Default::default();state.travel=None;state.event_start=None;state.survival_engaged=None;
            super::custom_event_ui::close();super::custom_event_ui::set_survival_hud(None);
            state.spawn_used=false;state.command=None;state.view.ballots.clear();state.view.shared_event=false;
            state.view.busy=false;state.view.travelling=false;state.view.enemy_hp=None;
            report(state,"队伍已重置，可以开始新的旅程。");Ok(())
        },
        Order::Enter{node,act,visited,..}=>{
            let seed=state.view.run.as_ref().unwrap().seed;
            // 全节点（房主与客机）统一走平滑等待出发：若刚离开灵庙等动态碰撞地面、正在翻滚或切菜单，
            // 等待站稳后自动传送，不再直接报错触发全队炸房。
            state.party.as_mut().unwrap().event.shop_departure=Some(ShopDeparture{seed,act,visited,node,last_reason:None,queued_at:Instant::now()});
            tick_shop_departure(state,world,game)
        },
        Order::Defeat{..}=>{
            stop_map_combat(state,world);
            // 全队倒下：房主与成员都记下这场战斗（单人在战斗结算时记）
            if let Some(run)=state.view.run.as_ref()&&let Phase::Combat{id}=run.phase {record_fight(run,Some(id),false);}
            if let Some(run)=state.view.run.as_mut(){run.end_on_death();record_checked(run)?;}
            Ok(())
        },
        Order::Stop(error)=>{super::dance_ui::cancel();super::custom_event_ui::close();super::custom_event_ui::set_survival_hud(None);state.survival_engaged=None;stop_map_combat(state,world);Err(error)},
        _=>Ok(()),
    }
}

fn release_closed_party(closed:bool,at_hub:bool,hub_active:bool,phase:Option<&Phase>)->bool {
    closed&&at_hub&&match phase {None=>hub_active,Some(Phase::Dead|Phase::Won)=>true,_=>false}
}
fn rebuild_lobby(changed:bool,at_hub:bool,hub_active:bool,phase:Option<&Phase>)->bool {
    changed&&at_hub&&hub_active&&phase.is_none()
}
fn lobby_reconnect_error(error:Option<&str>)->bool {
    // [fix] A teammate fault also only blocks until everyone is back at an idle hub with no run:
    // rebuild_lobby() still requires that, so the host is no longer locked out until a restart.
    error.is_some_and(|error|error.contains(erdtree_tower::party_handshake::RECONNECTED)||error.starts_with(super::party::TEAMMATE_FAULT))
}
fn lobby_roster_changed(link:Option<&erdtree_tower::party_roster::Roster>,pending:Option<&erdtree_tower::party_roster::Roster>,current:Option<&erdtree_tower::party_roster::Roster>)->bool {
    link.or(pending).zip(current).is_some_and(|(old,current)|old!=current)
}
fn remote_route_prompt(view:&View,next:&[erdtree_tower::party_vote::Ballot],local:u64)->bool {
    view.room.is_none()&&!view.travelling&&!view.shared_event
        &&view.run.as_ref().is_some_and(|run|run.phase==Phase::Map)
        &&next.iter().any(|ballot|ballot.player!=local&&ballot.node.is_some()
            &&!view.ballots.iter().any(|old|old==ballot))
}
fn recover_party_at_hub(state:&mut Runtime,world:&WorldChrMan,game:&GameMan) {
    use eldenring::cs::{CSSessionManager,LobbyState};
    if super::party::manual_config().is_some()||!super::seamless::active()
        ||(state.party.is_none()&&state.party_pending.is_none()&&state.party_error.is_none()){return;}
    let Ok(session)=(unsafe {CSSessionManager::instance()}) else{return;};
    let at_hub=!game.warp_requested&&world.main_player.as_ref().is_some_and(|p|
        i32::from(p.current_block_id)==super::hub::BLOCK&&p.chr_ins.modules.data.hp>0
        &&p.chr_ins.modules.physics.standing_on_solid_ground);
    let roster=super::seamless::roster(session).ok();
    let changed=lobby_roster_changed(state.party.as_ref().and_then(|p|p.roster()),state.party_pending.as_ref().map(|p|&p.roster),roster.as_ref());
    let reconnect=lobby_reconnect_error(state.party_error.as_deref()
        .or_else(||state.party.as_ref().and_then(|p|p.stopped.as_deref()))
        .or_else(||state.party_pending.as_ref().and_then(|p|p.failure())));
    if rebuild_lobby(changed||reconnect,at_hub,state.hub.as_ref().is_some_and(|h|h.active),state.view.run.as_ref().map(|r|&r.phase)) {
        state.party=None;state.party_pending=None;state.party_error=None;
        state.command=None;state.view.ballots.clear();state.view.busy=false;
        super::log(format!("Lobby recovery before run: roster_changed={changed} peer_reconnected={reconnect}; rebuilding Steam party handshake"));
        return;
    }
    let alone_host=roster.is_some_and(|r|r.local==r.host&&r.members.len()==1);
    if !release_closed_party(session.lobby_state==LobbyState::None||alone_host,at_hub,
        state.hub.as_ref().is_none_or(|h|h.active),state.view.run.as_ref().map(|r|&r.phase)){return;}
    state.party=None;state.party_pending=None;state.party_error=None;
    state.command=None;state.view.ballots.clear();state.view.busy=false;
    super::log("Empty or closed Seamless session released at hub; personal progress retained");
}
fn attendant_disabled()->bool {
    static OFF:std::sync::LazyLock<bool>=std::sync::LazyLock::new(||std::env::var("ERDTREE_TOWER_NO_ATTENDANT").as_deref()==Ok("1"));
    *OFF
}
fn stall_victory_enabled()->bool {
    static ON:std::sync::LazyLock<bool>=std::sync::LazyLock::new(||std::env::var("ERDTREE_TOWER_STALL_VICTORY").as_deref()==Ok("1"));
    *ON
}
fn diagnostic_stall_victory(enabled:bool,authority:bool,elapsed:Duration)->bool {enabled&&authority&&elapsed>=STALL_TIMEOUT}
// [fix] Keep the attack-disable guard in sync on every tick, including the early-return error /
// emergency-end paths. Before, those paths skipped ActionGuard::update, so a dialog that had just
// disabled attacks (disabled_secondary_actions) could leave the player unable to attack for good.
fn reconnect_locks_actions(waiting:bool,at_hub:bool)->bool {waiting&&!at_hub}
fn sync_action_guard(state:&mut Runtime,world:&mut WorldChrMan) {
    let at_hub=world.main_player.as_ref().is_some_and(|p|i32::from(p.current_block_id)==super::hub::BLOCK);
    let menu_active=reconnect_locks_actions(state.reconnect.is_some()||state.suspension.prepared.is_some()&&!state.suspension.leave,at_hub)||super::input::capture_actions(state.view.shared_event,super::input::active(),
        state.room.as_ref().is_some_and(|r|r.dialog_active())
        || state.attendant.as_ref().is_some_and(|r|r.dialog_active())
        || state.hub.as_ref().is_some_and(|h|h.dialog_active()));
    if let Some(player)=world.main_player.as_mut() {
        state.action_guard.update(&mut player.chr_ins,menu_active,super::dance_ui::active(),super::input::menu_input_active());
    }
}
fn poll_blocked_hub(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan) {
    sync_action_guard(state,world);
    if game.warp_requested||!world.main_player.as_ref().is_some_and(|p|i32::from(p.current_block_id)==super::hub::BLOCK){return;}
    if let Some(mut hub)=state.hub.take() {
        if hub.active {
            let reason=party_block_reason(state,world);
            if let Err(error)=hub.poll(world,game,Some(&reason),super::hub::PartyChoiceGuard::Solo){super::log_repeating(format!("Blocked hub service: {error}"));}
            state.view.room=Some(hub.view());
        }
        state.hub=Some(hub);
    }
}
fn party_block_reason(state:&Runtime,world:&WorldChrMan)->String {
    let reason=if let Some(error)=state.party_error.as_ref().or_else(||state.party.as_ref().and_then(|p|p.stopped.as_ref())){error.clone()}
    else if let Some(pending)=&state.party_pending{pending.waiting_reason()}
    else if state.hub.as_ref().is_some_and(|h|h.paused){"你保存了一局暂停中的旅程，但现在还在别人的联机房间里。请先退出这个房间，再继续你自己的旅程。".into()}
    else if state.party.as_ref().is_some_and(|p|p.needs_new_run()){"上一局已经结束，还在等所有人完成结算、回到大赐福。请大家都回到大赐福后再开始。".into()}
    else {"正在检查你的游戏文件和联机设置，稍等几秒；检查通过后会和队友确认。".into()};
    player_names(reason,world)
}
/// 提示优先显示 Steam 昵称，未读到的玩家保留尾号供区分。
fn player_names(text:String,world:&WorldChrMan)->String {
    let mut text=text;
    for player in world.player_chr_set.characters() {
        let id=player.player_session_holder.player_network_session.remote_identity;
        let name=super::seamless::player_name(id).unwrap_or_default();
        if id!=0&&!name.trim().is_empty() {text=text.replace(&id.to_string(),&format!("「{}」",name.trim()));}
    }
    // 剩下的 17 位 Steam ID（7656 开头）
    let mut out=String::new();let mut digits=String::new();
    let flush=|digits:&mut String,out:&mut String| {
        if digits.len()==17&&digits.starts_with("7656") {out.push_str(&format!("玩家（尾号 {}）",&digits[13..]));}else{out.push_str(digits);}
        digits.clear();
    };
    for c in text.chars() {if c.is_ascii_digit(){digits.push(c);}else{flush(&mut digits,&mut out);out.push(c);}}
    flush(&mut digits,&mut out);out
}
fn tick_auto_party(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan)->Result<bool,String> {
    if state.suspension.returning&&state.suspension.committed.is_some(){return Ok(false);}
    use eldenring::cs::{CSSessionManager,LobbyState};
    if state.departure.is_some(){return Ok(false);}
    if state.hub.as_ref().is_some_and(|h|h.settling()){return Ok(false);}
    if super::party::manual_config().is_some()||!super::seamless::active(){return Ok(false);}
    let Ok(session)=(unsafe {CSSessionManager::instance()}) else{return Ok(state.party_pending.is_some());};
    if let Some(link)=state.party.as_ref().filter(|p|p.roster().is_some()) {
        if !state.suspension.returning&&!state.suspension.requested&&let (Some(hub),Some(run))=(&state.hub,&state.view.run) {
            if matches!(run.phase,Phase::Dead|Phase::Won){
                super::reconnect::clear(hub.owner)?;
                super::suspension::clear_auto(hub.owner)?;
            } else {
                super::reconnect::save(hub.owner,link,run)?;
                if !run.debug&&erdtree_tower::suspension::Point::of(run).is_some()&&run.retry_node.is_none() {
                    if let Some(player)=world.main_player.as_ref().filter(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()) {
                        let runes=unsafe {player.player_game_data.as_ref()}.rune_count;
                        let flasks=state.flasks.as_ref().map(|f|f.total());
                        if let Ok(mut snap)=erdtree_tower::suspension::prepare(run,runes,flasks) {
                            snap.hp=(i64::from(snap.max_hp)*i64::from(player.chr_ins.modules.data.hp)/i64::from(player.chr_ins.modules.data.max_hp.max(1))).clamp(1,i64::from(snap.max_hp.max(1))) as u32;
                            if let Err(error)=super::suspension::save_auto_if_changed(hub.owner,&snap) {
                                super::log_repeating(format!("PARTY_AUTOSAVE failed: {error}"));
                            }
                        }
                    }
                }
            }
        }
        if !matches!(session.lobby_state,LobbyState::Host|LobbyState::Client) {
            // 房主这边最后一名队员离开后，无缝房间随之关闭：房主按一个人继续这局，不再停队。队员失去房主仍然暂停。
            let alone=link.roster().filter(|r|r.local==r.host).map(|r|erdtree_tower::party_roster::Roster{local:r.local,host:r.host,members:vec![r.host]});
            let peers:Vec<u64>=link.roster().map(|r|r.members.iter().copied().filter(|&id|id!=r.local).collect()).unwrap_or_default();
            for line in link.net_diagnosis(&peers) {super::log(format!("PARTY_NET session closed {line}"));}
            if let Some(next)=alone {
                if link.roster()==Some(&next) {return Ok(false);}
                if let Some(gone)=state.party.as_mut().and_then(|link|link.shrink(&next)) {
                    let names:Vec<String>=gone.iter().map(|id|super::seamless::player_name(*id).unwrap_or_else(||id.to_string())).collect();
                    super::log(format!("Seamless room closed with host alone: {gone:?}; continuing solo"));
                    report(state,format!("{} 已退出联机，旅程继续（剩 1 人）。",names.join("、")));
                    return Ok(false);
                }
            }
            return Err("无缝会话已退出，队伍同步已暂停".into());
        }
        if let Ok(roster)=super::seamless::roster(session) {
            if link.roster()!=Some(&roster){
                let gone:Vec<u64>=link.roster().map(|r|r.members.iter().copied().filter(|id|!roster.members.contains(id)).collect()).unwrap_or_default();
                for line in link.net_diagnosis(&gone) {super::log(format!("PARTY_NET member gone {line}"));}
                // 有队员中途退出（房主还在）：把他从队伍里去掉，其他人的旅程照常继续；房主退出或有人新加入仍暂停同步
                if let Some(gone)=state.party.as_mut().and_then(|link|link.shrink(&roster)) {
                    let names:Vec<String>=gone.iter().map(|id|super::seamless::player_name(*id).unwrap_or_else(||id.to_string())).collect();
                    super::log(format!("Party member left mid-run: {gone:?}; continuing with {} players",roster.members.len()));
                    report(state,format!("{} 已退出联机，旅程继续（剩 {} 人）。",names.join("、"),roster.members.len()));
                    return Ok(false);
                }
                if state.party.as_mut().is_some_and(|p|p.restore_roster(&roster)) {
                    super::log("RECONNECT original member returned; awaiting identity and progress confirmation");return Ok(false);
                }
                return Err("无缝队伍成员已变化，暂停当前旅程同步（仅支持原客机重连）".into());
            }
        }
        return Ok(false);
    }
    if session.lobby_state==LobbyState::None {
        state.party_pending=None;state.reconnect=None;
        return Ok(false);
    }
    let Ok(roster)=super::seamless::roster(session) else{return Ok(state.party_pending.is_some());};
    if roster.members.len()<2 {state.party_pending=None;return Ok(false);}
    if state.reconnect.as_ref().is_some_and(|p|p.roster!=roster){state.reconnect=None;}
    if roster.local!=roster.host&&state.party_pending.is_none() {
        if world.main_player.is_none()||game.save_slot<0{return Ok(true);}
        if state.reconnect.is_none()&&let Some(player)=world.main_player.as_ref() {
            let owner=(unsafe {player.player_game_data.as_ref()}.character_id,game.save_slot);
            if let Some((ticket,run))=super::reconnect::load(owner,&roster)? {
                if state.view.run.is_some(){return Err("请重启游戏、载入原角色后直接加入原房间；重连前不要开始或继续单人旅程".into());}
                let Some(resources)=resource_check() else{return Ok(true);};
                if let Some(file)=resources.first(){return Err(format!("重连资源不匹配：{file}"));}
                let Some(profile)=super::party_compat::profile() else{return Ok(true);};
                state.reconnect=Some(super::reconnect::Pending::new(roster.clone(),ticket,run,profile.clone()?)?);
                super::log(format!("RECONNECT started owner={owner:?} host={}",roster.host));
            }
        }
        if let Some(pending)=state.reconnect.as_mut() {
            match pending.poll()? {
                super::reconnect::Status::Waiting=>return Ok(true),
                super::reconnect::Status::FreshLobby=>{
                    let owner=pending.owner;
                    if !world.main_player.as_ref().is_some_and(|p|(unsafe{p.player_game_data.as_ref()}.character_id,game.save_slot)==owner) {
                        return Err("切换新旅程时角色已改变".into());
                    }
                    super::suspension::backup_restart(owner)?;
                    let progress=super::hub::path(owner,"json");
                    if progress.exists(){std::fs::remove_file(progress).map_err(|e|e.to_string())?;}
                    super::suspension::clear_auto(owner)?;
                    super::reconnect::clear(owner)?;
                    if let Some(hub)=state.hub.as_mut(){hub.finish();}
                    state.reconnect=None;
                    super::log("RECONNECT host confirmed fresh lobby; archived old journey and switching to new handshake");
                    return Ok(true);
                }
                super::reconnect::Status::ResumeLobby=>{
                    let owner=pending.owner;
                    if !world.main_player.as_ref().is_some_and(|p|(unsafe{p.player_game_data.as_ref()}.character_id,game.save_slot)==owner) {
                        return Err("切换恢复旅程时角色已改变".into());
                    }
                    super::reconnect::clear(owner)?;
                    if state.hub.is_none() && let Some(player)=world.main_player.as_ref() {
                        let archetype=unsafe {player.player_game_data.as_ref()}.archetype;
                        state.hub=Some(super::hub::Hub::new(owner,archetype)?);
                    }
                    state.reconnect=None;
                    super::log("RECONNECT host confirmed paused/autosave lobby; switching to coordinated hub resume handshake");
                    return Ok(true);
                }
                super::reconnect::Status::Ready=>{}
            }
            let Some(player)=world.main_player.as_ref() else{return Ok(true);};
            if !pending.landed(i32::from(player.current_block_id),!game.warp_requested&&player.chr_ins.modules.data.hp>0&&player.chr_ins.modules.physics.standing_on_solid_ground){return Ok(true);}
            let owner=pending.owner;
            if owner!=(unsafe {player.player_game_data.as_ref()}.character_id,game.save_slot){return Err("重连过程中角色已改变，已停止恢复".into());}
            if state.hub.is_none(){state.hub=Some(super::hub::Hub::new(owner,unsafe {player.player_game_data.as_ref()}.archetype)?);}
            let (link,run)=state.reconnect.take().unwrap().finish()?;
            let saved_fp=std::fs::read_to_string(super::hub::path(owner,"fp")).ok().and_then(|s|s.parse::<i32>().ok());
            SESSION_PATH.lock().map_err(|_|"存档锁不可用")?.replace(super::hub::path(owner,"json"));
            state.hub.as_mut().unwrap().reconnected(world);
            if let Some(player)=world.main_player.as_mut() {
                let data=&mut player.chr_ins.modules.data;
                data.hp=data.hp.min((i64::from(data.max_hp)*i64::from(run.hp)/i64::from(run.max_hp.max(1))).max(1) as i32);
                if let Some(fp)=saved_fp{data.fp=data.fp.min(fp.max(0));}
            }
            record_checked(&run)?;
            state.view.run=Some(run);state.party=Some(link);state.view.room=None;state.identity=None;state.flasks=None;
            state.command=None;state.view.busy=false;state.view.travelling=false;state.view.ballots.clear();game.save_requested=true;
            report(state,"已重连原队伍；保留个人进度，离线期间的奖励不补发。");
            super::log("RECONNECT guest restored at safe map checkpoint");return Ok(false);
        }
    }
    if state.party_pending.as_ref().is_some_and(|p|p.roster!=roster){state.party_pending=None;}
    if state.party_pending.is_none() {
        let Some(resources)=resource_check() else{return Ok(true);};
        if let Some(file)=resources.first(){return Err(format!("你的爬塔场景文件有缺失或损坏（{file}），和队友对不上。请在启动器里点「校验并修复文件」，然后重启游戏。"));}
        let Some(profile)=super::party_compat::profile() else{return Ok(true);};
        let profile=profile.clone()?;
        if state.view.run.as_ref().is_some_and(|r|r.act!=1||!r.visited.is_empty()||r.phase!=Phase::Map) {
            return Err("你身上还有一局没打完的旅程，不能直接加入别人的队伍。请先在大赐福结束或放弃这一局，再重新加入房间。".into());
        }
        let nonce=super::party::random_entropy()?.max(1);
        let seed=state.view.run.as_ref().or_else(||state.hub.as_ref().and_then(|h|h.suspended_run())).map(|r|r.initial_seed.unwrap_or(r.seed)).unwrap_or(super::party::random_entropy()?);
        state.party_pending=Some(super::steam_messages::Pending::new(roster,nonce,seed,super::options::local_dlc(),profile)?);
        let pending=state.party_pending.as_ref().unwrap();
        super::log(format!("Steam party handshake started: host={} members={}",pending.roster.local==pending.roster.host,pending.roster.members.len()));
    }
    let at_hub_ground=!game.warp_requested&&world.main_player.as_ref().is_some_and(|p|i32::from(p.current_block_id)==super::hub::BLOCK);
    let fresh_lobby=state.view.run.is_none()&&state.hub.as_ref().is_some_and(|h|h.active&&!h.paused)&&at_hub_ground;
    let resume_lobby_seed=(state.view.run.is_none()&&at_hub_ground)
        .then(||state.hub.as_ref().filter(|h|h.active&&h.paused).and_then(|h|h.suspended_run()).map(|r|r.initial_seed.unwrap_or(r.seed)))
        .flatten();
    if let Some((session,seed))=state.party_pending.as_mut().unwrap().poll(fresh_lobby,resume_lobby_seed)? {
        let mut aligned=state.view.run.clone();
        if let Some(run)=&mut aligned {run.align_party_start(seed)?;record_checked(run)?;}
        let link=super::party::Link::from_steam(state.party_pending.take().unwrap(),session,seed)?;
        state.view.run=aligned;state.party=Some(link);state.view.ballots.clear();state.command=None;
        super::log("Steam party handshake committed; shared runtime link enabled");
        return Ok(false);
    }
    Ok(true)
}

fn party_map_combat(fighting:bool,victory:bool,defeated:bool,merchant_active:bool)->bool {
    !defeated&&((fighting&&!victory)||merchant_active)
}
fn party_death_drain(victory:bool,departing:bool,cleanup:Option<(u64,bool)>,epoch:u64)->bool {
    victory&&!departing&&cleanup.is_some_and(|(e,swept)|e==epoch&&!swept)
}

fn tick_party(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan)->Result<(),String> {
    finish_victory_cleanup(state,world);
    let epoch=state.party.as_ref().filter(|p|p.host()&&!p.defeated&&p.stopped.is_none())
        .filter(|_|state.hub.as_ref().is_some_and(|h|!h.active))
        .filter(|_|state.view.run.as_ref().is_some_and(|r|!matches!(r.phase,Phase::Dead|Phase::Won)))
        .map(|p|p.epoch);
    let player=world.main_player.as_ref().map(|p|(i32::from(p.current_block_id),p.chr_ins.modules.data.hp<=0||p.chr_ins.chr_flags1c5.death_flag()));
    if state.host_death.observe(epoch,player,game.warp_requested) {
        super::log(format!("Host death departed encounter: epoch={epoch:?} origin={:?} current={player:?} warp={}; settling party",state.host_death.block,game.warp_requested));
        state.party.as_mut().unwrap().end_run()?;
        stop_map_combat(state,world);
        if let Some(run)=state.view.run.as_mut(){run.end_on_death();record_checked(run)?;}
        state.command=None;
        report(state,"房主死亡后被传离场景，本局已中止，队伍返回大赐福。");
    }
    if let Some(player)=world.main_player.as_ref(){
        let identity=(unsafe {player.player_game_data.as_ref()}.character_id,game.save_slot);
        if super::hub::path(identity,"party-award").exists(){return Err("上次联机领奖中断，未自动重发；请按结束本局快捷键（默认 Home）返回大赐福，旧记录会备份后解除联机阻塞".into());}
    }
    let alive=world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag());
    let present=world.main_player.is_some()&&!game.warp_requested;
    let at_hub=present&&state.view.run.is_none()&&state.hub.as_ref().is_some_and(|h|h.ready_for_party_restart())
        &&world.main_player.as_ref().is_some_and(|p|i32::from(p.current_block_id)==super::hub::BLOCK&&p.chr_ins.modules.physics.standing_on_solid_ground);
    let total=state.flasks.as_ref().map(|f|f.total());
    {let link=state.party.as_mut().unwrap();link.law_flasks.observe(link.epoch,present&&alive&&link.fighting&&!link.victory,total);}
    let orders=state.party.as_mut().unwrap().poll(if present {state.view.run.as_ref()}else{None},alive,at_hub,state.rest_end.requested);
    for order in orders {apply_party_order(state,world,game,order)?;}
    let isolated=state.party.as_mut().unwrap().take_isolated_peers();
    if !isolated.is_empty() {
        let names:Vec<String>=isolated.iter().map(|id|super::seamless::player_name(*id).unwrap_or_else(||id.to_string())).collect();
        let message=player_names(format!("{} 网络超时已暂时移出同步，其余队员继续；对方恢复后可重连加入。",names.join("、")),world);
        report(state,message);
    }
    if let Some(secs)=state.party.as_ref().and_then(|p|p.net_recovering()) && secs>=2 {
        report(state,"联机通道波动，正在自动重建连接并恢复同步……");
    }
    if let Some(waiting)=state.party.as_ref().filter(|p|p.host()&&!p.gate.pause_ready()).map(|p|p.waiting_reconnect_peers()).filter(|w|!w.is_empty()) {
        let names:Vec<String>=waiting.iter().map(|id|super::seamless::player_name(*id).unwrap_or_else(||id.to_string())).collect();
        let message=player_names(format!("{} 正在等待重连，完成当前房间回到路线图后自动合流。",names.join("、")),world);
        report(state,message);
    }
    sync_merchant_reward(state,world)?;
    if state.party.as_ref().is_some_and(|p|p.event.pause.ticket.is_some()){return Ok(());}
    if QUICK_END.load(std::sync::atomic::Ordering::Relaxed){return Ok(());}
    tick_revival(state,world,game)?;
    if state.party.as_ref().unwrap().defeated {return Ok(());}
    tick_shop_departure(state,world,game)?;
    tick_party_scene(state,world,game)?;
    let ballots=state.party.as_ref().unwrap().ballots.clone();
    let prompt=remote_route_prompt(&state.view,&ballots,state.party.as_ref().unwrap().config.player);
    state.view.ballots=ballots;
    if prompt {
        let message="队友已选择路线，请完成契约并选择自己的节点。";
        if state.view.status==message {state.view.prompt=state.view.prompt.wrapping_add(1);}
        else {report(state,message);}
    }
    let link=state.party.as_mut().unwrap();
    if let Some(error)=&link.stopped{return Err(error.clone());}
    // Arm before updating the combat flag: clearing it makes map events restore invincibility mid-death.
    if !link.host()&&link.victory&&alive&&!link.defeated&&state.reward_wait.is_none() {
        if let Some(run)=state.view.run.as_ref() {
            if let Phase::Combat{id}=run.phase {
                if state.victory_cleanup.as_ref().is_none_or(|c|c.0!=link.epoch) {
                    state.victory_cleanup=Some((link.epoch,super::enemies::map_entities(&super::enemies::node_roster(run,id))?,Instant::now(),false));
                    let runes=world.main_player.as_ref().map(|p|unsafe {p.player_game_data.as_ref()}.rune_count);
                    super::log(format!("Party native death drain started epoch={} runes={runes:?}",link.epoch));
                }
            }
        }
    }
    let draining=party_death_drain(link.victory,state.view.travelling||!present,
        state.victory_cleanup.as_ref().map(|c|(c.0,c.3)),link.epoch);
    let merchant_active=matches!(link.event.merchant_phase,2|3)&&state.room.as_ref().and_then(|r|r.hunter.as_ref())
        .is_some_and(|hunter|hunter.map_combat_active());
    super::native_talk::set_flag(super::enemies::MAP_COMBAT_FLAG,party_map_combat(link.fighting,link.victory&&!draining,link.defeated,merchant_active))?;
    if link.victory||link.defeated {super::native_talk::set_flag(45007990,false)?;}
    let Some(run)=state.view.run.as_ref() else{return Ok(());};
    if state.hub.as_ref().is_some_and(|h|h.active){return Ok(());}
    if run.seed!=erdtree_tower::party_game::act_seed(link.config.seed,run.act)?{return Err("队伍种子不匹配，未同步游戏状态".into());}
    if let Phase::Combat{id}=run.phase {
        if !link.host()&&!link.victory&&!link.defeated {
            let arena=super::arenas::for_run(run,true);
            let in_arena=world.main_player.as_ref().is_some_and(|p|i32::from(p.current_block_id)==arena.block)&&!game.warp_requested;
            if in_arena {
                let roster=super::enemies::node_roster(run,id);
                let ids=super::enemies::map_entities(&roster)?;
                // 与房主使用相同普通房波次和精英节奏，未到出场时机的敌人不计入就绪等待。
                let held=member_held(&mut state.member_spawn_gaps,(link.config.session,link.epoch,run.act,id),world,run,id,&roster,&ids);
                let mut ready=vec![false;roster.len()];
                static REQUESTED:std::sync::Mutex<Vec<(u64,u32)>>=std::sync::Mutex::new(Vec::new());
                for (slot,&entity) in ids.iter().enumerate() {
                    if held[slot] {continue;}
                    super::enemies::position_map_entity(entity,slot)?;
                    ready[slot]=super::enemies::request_map_entity(entity,true)?;
                    if let Ok(mut seen)=REQUESTED.lock()&&!seen.contains(&(link.epoch,entity)) {
                        if seen.len()>64 {seen.clear();}
                        seen.push((link.epoch,entity));
                        super::log(format!("TOWER_PARTY member released slot {slot} entity={entity} epoch={} ready={}",link.epoch,ready[slot]));
                    }
                }
                if !link.bound {
                    let mut seen=held.clone();
                    let engaged=link.fighting;
                    for chr in world.chr_sets.iter().flatten().flat_map(|set|set.characters()) {
                        for (slot,spec) in roster.iter().enumerate(){
                            if ready[slot]&&chr.event_entity_id==ids[slot]&&chr.npc_id==spec.model&&chr.npc_param_id==spec.npc
                                &&replica_seen(enemy_loaded(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status),chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag(),chr.debug_flags.character_disabled(),engaged){seen[slot]=true;}
                        }
                    }
                    link.bound=held.iter().any(|h|!h)&&seen.iter().all(|x|*x);
                    if link.bound {super::log(format!("Party replicas ready epoch={} {:.1} s after spawn{}",link.epoch,link.encounter.map_or(0.0,|t|t.elapsed().as_secs_f32()),if engaged{" (fight already engaged)"}else{""}));}
                }
            }
            if !link.bound&&link.encounter.is_some_and(|t|t.elapsed()>Duration::from_secs(90)){
                let roster=super::enemies::node_roster(run,id);
                for (entity,spec) in super::enemies::map_entities(&roster)?.into_iter().zip(roster) {log_map_enemy(world,entity,spec);}
                super::log(format!("Party replica timeout epoch={} node={id} block={:#x}; no complete entity/model/NPC match",link.epoch,super::arenas::for_run(run,true).block));
                return Err("队伍战斗实体未就绪，已停止发奖；请提供双方 tower-preview.log".into());
            }
        }
        if !link.host() {
            super::spirits::set_active((link.bound||run.embers.contains(&erdtree_tower::embers::Ember::SpiritTwin))&&!link.victory&&!link.defeated&&alive&&!run.law.lone_wolf());
            if link.victory&&alive&&state.reward_wait.is_none(){
                // 不马上撤敌人：房主在生命归零时就判定胜利，被处决的敌人要等处决动画播完才真正死亡；
                // 在本机死亡之前撤掉，成员就拿不到这只敌人的卢恩（看起来直接消失、不掉魂）
                state.reward_wait=Some((Instant::now(),run.clone()));state.view.busy=true;
                state.reward_portal=Default::default();
                let healed=heal_victory(world,run);if !healed.is_empty(){super::log(&healed);state.view.status=healed;state.view.prompt=state.view.prompt.wrapping_add(1);}
                // 召唤师「群灵」：与房主、单人相同，胜利时召唤物还活着就补回这次用掉的骨灰
                if let Some(plan)=keep_summoned_ash(world,run) {state.ash_refunds=plan;state.ash_guard=Some(Instant::now());}
                world.summon_buddy_manager.disappear_requested=true;
                give_elite_tear(world,run,id);
                // 与房主、单人相同：Boss 词条的额外卢恩，以及记进本局的战斗记录（结算的回顾、职业熟练度与灵光成就都按它算）
                let bonus=erdtree_tower::boss_affixes::bonus_runes(run,id);
                if let Some(player)=world.main_player.as_mut().filter(|_|bonus>0) {
                    let data=unsafe {player.player_game_data.as_mut()};
                    data.rune_count=data.rune_count.saturating_add(bonus).min(999_999_999);
                    super::log(format!("BOSS_AFFIX bonus runes={bonus}"));
                }
                record_fight(run,Some(id),true);
                super::log(format!("Party victory received epoch={}",link.epoch));
            }
        }
    }
    Ok(())
}
/// 联机成员胜利后撤敌人：死掉的敌人留着尸体（死亡动画、倒地与卢恩照常），等开始传送时一起撤下；
/// 收到胜利 8 秒后仍在本机活着的（与房主不同步），先单独撤下，免得在选奖励时打人
fn finish_victory_cleanup(state:&mut Runtime,world:&mut WorldChrMan) {
    let host=state.party.as_ref().is_some_and(|p|p.host());
    // 换了一场联机（队伍断开重连等）：过期的撤场作废，免得撤掉新一场的敌人
    let epoch=state.party.as_ref().map(|p|p.epoch);
    if state.victory_cleanup.as_ref().is_some_and(|c|Some(c.0)!=epoch) {state.victory_cleanup=None;return;}
    let Some((_,ids,started,swept))=state.victory_cleanup.as_mut() else{return};
    if state.view.travelling {
        let (_,ids,started,_)=state.victory_cleanup.take().unwrap();
        for entity in ids {if let Err(error)=super::enemies::request_map_entity(entity,false){super::log(error);}}
        super::log(format!("Party victory cleanup on departure after {:.1} s",started.elapsed().as_secs_f32()));
        return;
    }
    if *swept||!stray_sweep_due(started.elapsed()) {return;}
    *swept=true;
    let replicas:Vec<_>=world.chr_sets.iter().flatten().flat_map(|set|set.characters()).filter(|chr|ids.contains(&chr.event_entity_id))
        .map(|chr|(chr.event_entity_id,chr.modules.data.hp,chr.chr_flags1c5.death_flag())).collect();
    let runes=world.main_player.as_ref().map(|p|unsafe {p.player_game_data.as_ref()}.rune_count);
    super::log(format!("Party native death drain completed host={host} entities={replicas:?} runes={runes:?}"));
    if host{return;} // Only members sweep desynchronized replicas.
    // 含「0 血却未触发死亡标记」的僵尸副本（Issue #10）：不只清 hp>0，也要清 hp==0 && !death_flag。
    let stray:Vec<u32>=world.chr_sets.iter().flatten().flat_map(|set|set.characters()).filter(|chr|ids.contains(&chr.event_entity_id)
        &&!chr.chr_flags1c5.death_flag()&&!chr.debug_flags.character_disabled()&&enemy_loaded(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status))
        .map(|chr|chr.event_entity_id).collect();
    for &entity in &stray {if let Err(error)=super::enemies::request_map_entity(entity,false){super::log(error);}}
    if !stray.is_empty() {super::log(format!("Party victory: removed {} enemies still active locally {:?}",stray.len(),stray));}
}
fn stray_sweep_due(waited:Duration)->bool {waited>=Duration::from_secs(8)}
/// 本局结束后留在场上的敌人：玩家活着时（复活回到大赐福，或通关）5 秒后撤下，最多留 15 秒
/// 当前是不是单人旅程（每帧由 tick 更新）：界面据此决定显示不显示徘徊赐福
static SOLO:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(true);
pub fn solo()->bool {SOLO.load(std::sync::atomic::Ordering::Relaxed)}
/// 徘徊赐福生效时的提示
fn lingering_message(left:u8)->String {format!("徘徊赐福生效：复活后自动返回原房间重试（剩余 {left} 个）。")}

fn retry_run(run:&Run,destination:Option<usize>,solo:bool,handled:bool)->Option<Run> {
    if !solo||handled||run.retry_node.is_some(){return None;}
    let mut next=run.clone();
    if next.phase==Phase::Map {next.enter(destination?).ok()?;}
    next.retry_floor().then_some(next)
}

fn regional_event_hp(run:&mut Run,hp:i32,max_hp:i32,party:bool) {
    if run.retry_node.is_some() {return;}
    run.hp=normalized_hp(hp,max_hp).max(u32::from(party));
    if hp<=0&&!party&&!run.retry_floor() {run.end_on_death();}
}

fn retry_departure(since:&mut Option<Instant>,now:Instant,ready:bool)->bool {
    if !ready {*since=None;return false;}
    now.saturating_duration_since(*since.get_or_insert(now))>=Duration::from_millis(500)
}

fn begin_death_retry(state:&mut Runtime,world:&mut WorldChrMan,next:Run) {
    if let Some(pending)=state.pending.take() {
        if !pending.completed {
            let fought=if let Phase::Combat{id}=pending.next_run.phase {Some(id)}else{None};
            let names:Vec<_>=pending.foes.iter().map(|f|f.spec.name).collect();
            append_history(&next,fought,false,&names);
        }
        state.lingering=Some((pending,Instant::now()));
    }
    if let Some(room)=state.room.take(){room.retire(world);}
    for flag in [45020970,45020971,45020972,45007990,45007992,10007990,30097990,35007990] {
        if let Err(error)=super::native_talk::set_flag(flag,false){super::log(error);}
    }
    if let Err(error)=clear_chariot(){super::log(error);}
    state.travel=None;state.reward_wait=None;state.reward_portal=Default::default();
    state.flasks=None;state.event_start=None;state.survival_engaged=None;state.descent=None;state.stormveil=None;state.chariot_clock=None;
    super::custom_event_ui::close();super::custom_event_ui::set_survival_hud(None);
    state.boss_fight=Default::default();state.law_fight=Default::default();state.ember_fight=Default::default();state.bout.end(world);
    state.view.boss_affixes=None;state.view.boss_affix_list.clear();state.view.boss_alert=None;state.view.law_clock=None;
    state.command=None;state.view.room=None;state.view.enemy_hp=None;
    state.view.ready=false;state.view.busy=true;state.view.travelling=true;
    state.death_retry=true;state.retry_ready_since=None;
    state.retry_rescue=Default::default();
    let left=next.retries;record(&next);state.view.run=Some(next);
    report(state,lingering_message(left));
}

fn return_to_retry_room(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan)->bool {
    let Some(node)=state.view.run.as_ref().and_then(|r|r.retry_node) else {state.retry_ready_since=None;state.retry_rescue=Default::default();return false;};
    if state.party.is_some()||state.travel.is_some()||state.pending.is_some()||state.room.is_some(){return false;}
    state.command=None;state.view.busy=true;state.view.travelling=true;
    let falling=!game.warp_requested&&world.main_player.as_ref().is_some_and(|p|
        p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()
        &&p.chr_ins.modules.physics.is_falling&&!p.chr_ins.modules.physics.standing_on_solid_ground);
    match state.retry_rescue.tick(falling) {
        super::retry_rescue::Step::Request=>{
            let block=world.main_player.as_ref().map(|p|i32::from(p.current_block_id));
            super::log(format!("TOWER_RETRY_VOID_RESCUE node={node} block={block:?} target=hub entry=11102691"));
            game.move_map_target=BlockId::from(super::hub::BLOCK);
            game.initial_area_entity_id=11102691;game.warp_requested=true;
            state.retry_ready_since=None;
            return true;
        },
        super::retry_rescue::Step::Exhausted=>{
            super::log_repeating("TOWER_RETRY_VOID_RESCUE exhausted; retry state preserved");
            report(state,"复活安全传送未完成，请退出游戏并保留日志；本次重试进度已保留。");
            return true;
        },
        super::retry_rescue::Step::Wait=>{},
    }
    let ready=!game.warp_requested&&world.main_player.as_ref().is_some_and(|p|
        i32::from(p.current_block_id)>=0&&p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()
        &&p.chr_ins.modules.physics.standing_on_solid_ground&&!p.chr_ins.modules.physics.is_falling&&!p.chr_ins.chr_flags1c5.is_invincible());
    if !retry_departure(&mut state.retry_ready_since,Instant::now(),ready){return true;}
    state.retry_ready_since=None;
    let result=(||->Result<(),String>{
        let run=state.view.run.as_ref().unwrap();let player=world.main_player.as_mut().ok_or("角色尚未复活")?;
        let data=unsafe {player.player_game_data.as_mut()};
        let mut flasks=super::flasks::Flasks::bind_bonus(data,run.class,run.flask_bonus,run.total_flask_cut())?;
        flasks.restore(data,run.flasks,run.flask_blue)?;state.flasks=Some(flasks);
        let vitals=&mut player.chr_ins.modules.data;
        vitals.hp=((i64::from(vitals.max_hp)*i64::from(run.hp)/i64::from(run.max_hp)) as i32).max(1);
        state.retry_item.tick(data,Some(run),true);
        request_travel(state,world,game,node)
    })();
    match result {Ok(())=>super::log(format!("TOWER_RETRY automatic return node={node}")),Err(error)=>report(state,format!("正在重试原房间：{error}"))}
    true
}
fn lingering_due(alive:bool,waited:Duration)->bool {alive&&waited>=Duration::from_secs(5)||waited>=Duration::from_secs(15)}
#[cfg(test)]
mod replica_tests {
    #[test]
    fn skip_waits_for_enemies_and_in_a_party_for_the_host_after_engage() {
        use super::skip_fight_due as due;
        assert!(due(Some(3),Some(3),true,false,None));
        assert!(!due(None,Some(3),true,false,None));
        assert!(!due(Some(2),Some(3),true,false,None),"a stale request never skips another node");
        assert!(!due(Some(3),Some(3),false,false,None),"wait for the enemies to appear");
        assert!(!due(Some(3),Some(3),true,true,None),"not while dead");
        assert!(due(Some(3),Some(3),true,false,Some((true,true))));
        assert!(!due(Some(3),Some(3),true,false,Some((true,false))),"host waits for the party to engage");
        assert!(!due(Some(3),Some(3),true,false,Some((false,true))),"members never judge victory");
    }
    #[test]
    fn ailments_are_cleared_once_a_second_only_while_an_overlay_locks_input() {
        use std::time::{Duration,Instant};
        let now=Instant::now();
        assert!(super::overlay_cleanse_due(true,None,now));
        assert!(!super::overlay_cleanse_due(false,None,now),"never while the player can act (combat, rooms, travel)");
        assert!(!super::overlay_cleanse_due(true,Some(now),now+Duration::from_millis(999)));
        assert!(super::overlay_cleanse_due(true,Some(now),now+Duration::from_secs(1)));
    }
    #[test]
    fn slow_members_count_enemies_killed_before_they_loaded() {
        use super::replica_seen as seen;
        // 开战前：只认已加载、启用、活着的
        assert!(seen(true,true,false,false));
        assert!(!seen(true,false,false,false),"a stale corpse never counts before the fight starts");
        assert!(!seen(true,true,true,false),"not enabled yet");
        assert!(!seen(false,true,false,false),"not loaded yet");
        // 开战后：已经被打死的也算，但还没加载、还没启用的活敌人不算
        assert!(seen(true,false,false,true));assert!(seen(true,false,true,true),"a corpse may already be disabled");
        assert!(seen(true,true,false,true));
        assert!(!seen(false,false,false,true));assert!(!seen(true,true,true,true));
    }
}
#[cfg(test)]
mod victory_cleanup_tests {
    #[test]
    fn victory_drain_keeps_native_death_processing_until_cleanup() {
        assert!(super::party_death_drain(true,false,Some((7,false)),7));
        assert!(!super::party_death_drain(true,false,Some((7,true)),7));
        assert!(!super::party_death_drain(true,false,Some((6,false)),7));
        assert!(!super::party_death_drain(true,true,Some((7,false)),7));
        assert!(!super::party_death_drain(false,false,Some((7,false)),7));
        assert!(!super::party_death_drain(true,false,None,7));
    }
    #[test]
    fn victory_packet_does_not_restore_invincibility_before_native_death() {
        let draining=super::party_death_drain(true,false,Some((7,false)),7);
        assert!(super::party_map_combat(true,!draining,false,false));
        assert!(!super::party_map_combat(true,!draining,true,false));
        assert!(!super::party_map_combat(false,!draining,false,false));
        let drained=super::party_death_drain(true,false,Some((7,true)),7);
        assert!(!super::party_map_combat(true,!drained,false,false));
    }
    #[test]
    fn members_keep_corpses_and_only_sweep_desynced_enemies_after_eight_seconds() {
        use std::time::Duration as D;
        assert!(!super::stray_sweep_due(D::from_secs(7)),"a riposted enemy can still be mid-animation");
        assert!(super::stray_sweep_due(D::from_secs(8)));
    }
    #[test]
    fn enemies_stay_while_the_player_is_dead() {
        use std::time::Duration as D;
        assert!(!super::lingering_due(false,D::from_secs(10)),"\"YOU DIED\" keeps the enemies on screen");
        assert!(!super::lingering_due(true,D::from_secs(2)),"a final boss corpse does not vanish at once");
        assert!(super::lingering_due(true,D::from_secs(5)),"retired once the player is back");
        assert!(super::lingering_due(false,D::from_secs(15)),"never kept forever");
    }
}
fn trial_revival_destination(choice:Option<erdtree_tower::regional_events::Trial>)->Option<super::party_death::Position> {
    use erdtree_tower::regional_events::Trial;
    let (block,xyz)=match choice? {
        Trial::Descent=>(0x23000000,[125.644,-408.535,-61.213]),
        Trial::Stormveil=>{let route=parkour_route(false);(route.block,route.goal)},
        Trial::Chariot=>(0x2d010000,[0.0,0.2,0.0]),
        Trial::HereticalRise=>{let route=hidden_road_route(choice?);(route.block,route.goal)},
        Trial::Skip|Trial::Mausoleum=>return None,
    };
    Some(super::party_death::Position{block,xyz})
}
fn tick_revival(state:&mut Runtime,world:&mut WorldChrMan,game:&GameMan)->Result<(),String> {
    if !super::seamless_revival::available(){super::seamless_revival::cancel();return Ok(());}
    use super::party_death::{Action as Recovery,Position};
    let link=state.party.as_ref();
    let epoch=link.filter(|p|!p.defeated&&p.stopped.is_none())
        .filter(|_|state.hub.as_ref().is_some_and(|h|!h.active))
        .filter(|_|state.view.run.as_ref().is_some_and(|r|!matches!(r.phase,Phase::Dead|Phase::Won)))
        .map(|p|p.epoch);
    if state.party_death.epoch()!=epoch {super::seamless_revival::cancel();}
    let wave=epoch.and_then(|epoch|link.and_then(|p|p.event.job.map(|j|(epoch,j.wave))));
    if state.party_death_wave!=wave {
        state.party_death_wave=wave;state.party_death=Default::default();super::seamless_revival::cancel();
    }
    let trial_goal=link.filter(|p|p.event.trial_victory()).and_then(|p|trial_revival_destination(p.event.trial_decision));
    let mausoleum_goal=epoch.and_then(|_|state.room.as_ref().filter(|room|room.mausoleum)).and_then(|room|{
        let survivor=link.and_then(|p|p.roster()).is_some_and(|roster|world.player_chr_set.characters().any(|p|
            i32::from(p.current_block_id)==room.block&&p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()
            &&world.main_player.as_ref().is_none_or(|local|local.chr_ins.field_ins_handle!=p.chr_ins.field_ins_handle)
            &&roster.members.contains(&p.player_session_holder.player_network_session.remote_identity)));
        let shared=link.is_some_and(|p|p.event.mausoleum_open);
        let flow=if shared{Some((true,true))}else{super::mausoleum::flow().ok()};
        super::mausoleum::recovery_target(state.view.run.as_ref(),room.arrival_block_position,flow,survivor||shared)
    });
    let block_goal=mausoleum_goal.or(trial_goal);
    let destination=block_goal.and_then(|goal|world.main_player.as_ref().and_then(|p|{
        let block=i32::from(p.current_block_id);let b=&p.block_position;let h=&p.chr_ins.modules.physics.position;
        goal.in_physics_space(Position{block,xyz:[b.x,b.y,b.z]},Position{block,xyz:[h.0,h.1,h.2]})
    }));
    let victory=mausoleum_goal.is_some()&&destination.is_some()||link.is_some_and(|p|p.victory||p.event.victory()||p.event.merchant_phase==3||p.event.trial_victory());
    if epoch.is_some()&&victory {super::seamless_revival::tick(world,game);}
    let player=world.main_player.as_ref().map(|p|{
        let pos=&p.chr_ins.modules.physics.position;
        (Position{block:i32::from(p.current_block_id),xyz:[pos.0,pos.1,pos.2]},p.chr_ins.modules.data.hp<=0||p.chr_ins.chr_flags1c5.death_flag())
    });
    let recovered=world.main_player.as_ref().is_some_and(|p|super::seamless_revival::finished(super::seamless_revival::owned_address(p)));
    if let Some(destination)=destination {state.party_death.redirect(destination);}
    match state.party_death.observe_confirmed(epoch,player,victory,game.warp_requested,recovered) {
        Recovery::RequestRevival(origin)=>{
            let destination=destination.filter(|p|p.block==origin.block);
            let origin=destination.unwrap_or(origin);
            state.party_death.redirect(origin);
            if let Some(p)=world.main_player.as_ref().filter(|p|i32::from(p.current_block_id)==origin.block) {
                if let Some(goal)=block_goal.filter(|_|destination.is_some()) {super::seamless_revival::request_at_safe_block_goal(p,goal)?;}
                else {super::seamless_revival::request(p,origin)?;}
                super::log(format!("PARTY_REVIVE requested epoch={epoch:?} physics_origin={origin:?} block_goal={block_goal:?} mausoleum={}",mausoleum_goal.is_some()));
            }
        },
        Recovery::FinishRevival=>{
            super::log(format!("PARTY_REVIVE alive at recovery goal epoch={epoch:?}"));
        },
        _=>{},
    }
    Ok(())
}

fn award_path()->Result<std::path::PathBuf,String>{
    SESSION_PATH.lock().map_err(|_|"存档锁不可用")?.as_ref().map(|p|p.with_extension("party-award")).ok_or("角色存档尚未绑定".into())
}
fn write_award_journal(path:&std::path::Path,run:&Run)->Result<(),String>{
    use std::io::Write;
    let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|_|"无法记录领奖开始状态，未发放奖励")?;
    file.write_all(run.encode()?.as_bytes()).and_then(|_|file.sync_all()).map_err(|_|"无法保存领奖记录，未发放奖励".into())
}
fn finish_party_award()->Result<(),String>{std::fs::remove_file(award_path()?).map_err(|_|"奖励已执行，但领取记录未能提交；已停止重发".into())}

/// 本机是否在多人联机房间里（或已组队），每帧由 tick 更新，给界面线程读
pub static IN_PARTY:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
pub fn in_party()->bool {IN_PARTY.load(std::sync::atomic::Ordering::Relaxed)}
// All engine references stay within the ChrIns_PostPhysics callback.
fn solo_session()->bool {
    use eldenring::cs::{CSSessionManager,LobbyState};
    if !super::seamless::active(){return true;}
    unsafe {CSSessionManager::instance()}.ok().is_some_and(|s|s.lobby_state==LobbyState::None
        ||super::seamless::roster(s).is_ok_and(|r|r.local==r.host&&r.members.len()==1))
}
fn suspend_snapshot(state:&mut Runtime,world:&mut WorldChrMan,game:&GameMan)->Result<Run,String> {
    let player=world.main_player.as_ref().ok_or("等待角色加载")?;
    if game.save_slot<0||game.warp_requested||player.chr_ins.modules.data.hp<=0||player.chr_ins.chr_flags1c5.death_flag(){return Err("请等待传送或复活结束后暂退".into());}
    let owner=(unsafe {player.player_game_data.as_ref()}.character_id,game.save_slot);
    if state.identity!=Some(owner)||state.hub.as_ref().is_none_or(|h|h.owner!=owner){return Err("等待当前角色与旅程绑定完成".into());}
    if state.material_award.is_some()||state.command.is_some()||state.reward_wait.is_some()||state.travel.is_some()
        ||state.room.as_ref().is_some_and(|r|r.dialog_active())||state.attendant.as_ref().is_some_and(|r|r.dialog_active())
        ||state.view.travelling||!state.ash_refunds.is_empty()||super::run_screen::active()
        ||super::hub::path(owner,"party-award").exists(){return Err("等待当前交易、领奖、召唤物退场或传送完成".into());}
    if super::spirits::dismiss_local_for_suspend(world)&&super::spirits::has_local_groups(world) {
        return Err("正在遣返召唤物，随后自动暂退".into());
    }
    let player=world.main_player.as_ref().ok_or("等待角色加载")?;
    let run=state.view.run.as_ref().ok_or("当前没有正在进行的旅程")?;
    let mut saved=erdtree_tower::suspension::prepare(run,unsafe {player.player_game_data.as_ref()}.rune_count,state.flasks.as_ref().map(|f|f.total()))?;
    saved.hp=(i64::from(saved.max_hp)*i64::from(player.chr_ins.modules.data.hp)/i64::from(player.chr_ins.modules.data.max_hp.max(1))).clamp(1,i64::from(saved.max_hp.max(1))) as u32;
    SESSION_PATH.lock().map_err(|_|"存档锁不可用")?.replace(super::hub::path(owner,"json"));
    super::suspension::save(owner,&saved)?;record_checked(&saved)?;
    Ok(saved)
}
fn return_suspended_to_hub(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan,saved:Run)->Result<(),String> {
    let player=world.main_player.as_ref().ok_or("等待角色加载")?;
    let owner=(unsafe {player.player_game_data.as_ref()}.character_id,game.save_slot);
    super::reconnect::clear(owner)?;
    if state.hub.is_none(){state.hub=Some(super::hub::Hub::new(owner,unsafe {player.player_game_data.as_ref()}.archetype)?);}
    stop_map_combat(state,world);
    if let Some(room)=state.room.take(){room.retire(world);}
    if let Some(scene)=state.party_scene.take(){scene.retire(world);}
    if let Some(pending)=state.pending.take(){retire_group(&pending,world);}
    state.hub.as_mut().unwrap().pause(saved);
    state.command=None;state.view.run=None;state.view.room=None;state.view.shared_event=false;state.view.busy=false;state.view.travelling=false;
    state.travel=None;state.reward_wait=None;state.flasks=None;state.identity=None;state.party_error=None;state.party_pending=None;
    state.rest_end=Default::default();state.host_death=Default::default();state.party_death=Default::default();
    state.event_start=None;state.survival_engaged=None;state.descent=None;state.stormveil=None;state.chariot_clock=None;let _=clear_chariot();
    super::custom_event_ui::close();super::custom_event_ui::set_survival_hud(None);
    super::seamless_revival::cancel();super::spirits::refund::close();state.ash_tracking=false;state.ash_guard=None;state.ash_refunds.clear();state.ash_revival_refund=false;state.twin_refund=None;
    super::spirits::prepare_warp(world,"suspend_to_hub");
    state.suspension.returning=true;state.suspension.committed=Some(Instant::now());state.suspension.requested=false;
    game.save_requested=true;
    super::suspension::notice("旅程已暂存，正在返回大赐福。请勿强制关闭游戏。");
    super::log("SUSPEND committed; returning to safe hub without ending the run");Ok(())
}
fn tick_suspension(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan)->Result<bool,String> {
    use std::sync::atomic::Ordering;
    if state.view.run.as_ref().is_some_and(|r|matches!(r.phase,Phase::Dead|Phase::Won))&&state.suspension.prepared.is_none() {
        state.suspension=Default::default();if let Some(link)=state.party.as_mut(){link.event.pause.queued=false;}
        super::suspension::notice("");super::suspension::REQUEST.store(false,Ordering::Release);return Ok(false);
    }
    let role=state.party.as_ref().map_or(0,|p|if p.host(){1}else{2});super::suspension::ROLE.store(role,Ordering::Release);
    if super::suspension::REQUEST.swap(false,Ordering::AcqRel)&&!state.suspension.returning {
        if state.suspension.requested&&state.suspension.prepared.is_none()&&state.party.as_ref().is_none_or(|p|p.event.pause.ticket.is_none()) {
            state.suspension.requested=false;if let Some(link)=state.party.as_mut(){link.event.pause.queued=false;}
            super::suspension::notice("已取消等待暂退。");return Ok(false);
        }
        state.suspension.requested=true;
        if let Some(link)=state.party.as_mut().filter(|p|p.host()){link.event.pause.queued=true;}
        super::suspension::notice(if role==0{"正在准备暂存旅程。"}else{"等待当前房间结束并完成领奖，随后保存暂退。"});
    }
    if let Some(link)=state.party.as_mut(){link.event.pause.resume=state.hub.as_ref().and_then(|h|h.suspended_run()).and_then(erdtree_tower::suspension::Point::of);}
    if state.suspension.returning {
        let grounded=!game.warp_requested&&world.main_player.as_ref().is_some_and(|p|i32::from(p.current_block_id)==super::hub::BLOCK&&p.chr_ins.modules.physics.standing_on_solid_ground);
        if grounded&&state.suspension.committed.is_some_and(|t|t.elapsed()>Duration::from_secs(5)) {
            game.save_requested=true;state.suspension.committed=None;state.party=None;state.party_error=None;
            super::suspension::notice("已返回大赐福。请从游戏菜单正常退出；重进后与小红帽交互继续旅程。");
        }
        return Ok(false);
    }
    if state.suspension.leave&&solo_session() {
        if game.warp_requested{return Ok(true);}
        if let (Some(flasks),Some(player))=(state.flasks.as_mut(),world.main_player.as_mut()){flasks.observe(unsafe {player.player_game_data.as_mut()},false)?;}
        let saved=suspend_snapshot(state,world,game)?;state.suspension.prepared=None;
        state.party=None;state.party_error=None;return_suspended_to_hub(state,world,game,saved)?;return Ok(false);
    }
    let ticket=state.party.as_ref().and_then(|p|p.event.pause.ticket);
    if state.suspension.requested&&ticket.is_none()&&state.suspension.prepared.is_none() {
        if role==0&&!solo_session(){return Err("等待联机握手完成，暂不按单人方式退出".into());}
        let Some(run)=state.view.run.as_ref() else{state.suspension.requested=false;return Err("当前没有正在进行的旅程".into());};
        if role!=0&&erdtree_tower::suspension::Point::of(run).is_none(){return Ok(false);}
        if role==0&&matches!(run.phase,Phase::Node{id} if !matches!(run.nodes[id].kind,erdtree_tower::NodeKind::Camp|erdtree_tower::NodeKind::Shop)){
            super::suspension::notice("等待当前房间结束并完成领奖，随后保存暂退。");return Ok(false);
        }
        if state.room.as_ref().is_some_and(|r|r.hunter.is_some()||r.scarabs.active()){
            super::suspension::notice("等待当前房间结束并完成领奖，随后保存暂退。");return Ok(false);
        }
        if role==1 {
            if state.party.as_ref().is_some_and(|p|p.gate.pause_ready()){state.party.as_mut().unwrap().prepare_pause()?;}
        } else {
            let saved=suspend_snapshot(state,world,game)?;
            if role==0 {return_suspended_to_hub(state,world,game,saved)?;return Ok(false);}
            state.suspension.prepared=Some(saved);state.suspension.leave=true;
            state.party.as_mut().unwrap().event.pause.leaving=true;game.save_requested=true;
        }
    }
    let ticket=state.party.as_ref().and_then(|p|p.event.pause.ticket);
    if ticket.is_some()&&state.suspension.prepared.is_none() {
        if state.view.run.as_ref().is_none_or(|r|r.phase!=Phase::Map){return Err("等待本机回到路线图后确认整队暂退".into());}
        state.suspension.prepared=Some(suspend_snapshot(state,world,game)?);
        state.party.as_mut().unwrap().event.pause.saved=true;game.save_requested=true;
    }
    if state.suspension.prepared.is_none(){return Ok(false);}
    state.view.busy=true;state.view.ready=false;state.command=None;
    state.view.room=state.suspension.leave.then(||super::rooms::RoomView{message:"正在保存暂退进度".into(),loading:false,completed:false});
    let capture=!state.suspension.leave&&!super::input::controls_unlocked();
    super::input::CAPTURE.store(capture,Ordering::Relaxed);state.pad_capture.update(capture);
    let alive=world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag());
    let orders=state.party.as_mut().ok_or("暂退队伍通道已断开")?.poll(state.view.run.as_ref(),alive,false,false);
    for order in orders {apply_party_order(state,world,game,order)?;}
    let link=state.party.as_mut().unwrap();
    if state.suspension.leave {
        super::suspension::notice(if link.event.pause.leave_ready{"离队已确认。请使用“分离迷雾”断开无缝房间，随后自动返回大赐福。"}else{"进度已暂存，等待房主确认离队；请勿强退。"});return Ok(true);
    }
    if link.host(){link.commit_pause()?;}
    if link.event.pause.committed {
        let saved=state.suspension.prepared.take().unwrap();return_suspended_to_hub(state,world,game,saved)?;return Ok(false);
    }
    if link.host()&&link.event.pause.started.is_some_and(|t|t.elapsed()>Duration::from_secs(30)){link.abort_pause()?;}
    if link.event.pause.ticket.is_none() {
        if let Some(hub)=&state.hub{super::suspension::clear(hub.owner)?;}
        state.suspension=Default::default();state.view.room=None;state.view.busy=false;
        super::suspension::notice("整队暂退未获全员确认，已取消；旅程仍在进行。");return Ok(false);
    }
    super::suspension::notice("本机进度已暂存，等待全员保存确认。任一人失败都不会结束旅程。");Ok(true)
}
fn ended_progress(run:&Run)->Run {
    let mut ended=run.clone();ended.end_on_death();ended
}
#[test]
fn home_ends_combat_and_rewards_without_a_continuation() {
    let mut run=Run::new(42,erdtree_tower::Class::Samurai);
    let node=run.available()[0];run.enter(node).unwrap();
    for reward in [false,true] {
        let mut source=run.clone();if reward{source.win_combat().unwrap();}
        let ended=ended_progress(&source);
        let mut loaded=Run::decode(&ended.encode().unwrap()).unwrap();
        assert_eq!(loaded.phase,Phase::Dead);
        assert!(loaded.resume_from_hub().is_err());
        assert_eq!(ended_progress(&ended).encode().unwrap(),ended.encode().unwrap());
        assert_ne!(source.phase,Phase::Dead);
    }
}
#[cfg(test)]
fn suspended_progress(run:&Run,gold:u32,flasks:Option<u8>)->Result<Run,String> {
    let mut next=run.clone();next.gold=gold;
    if let Some(flasks)=flasks {next.flasks=flasks;}
    next.validate()?;Ok(next)
}
#[test]
fn suspend_keeps_pending_reward_and_claim_history_across_reload() {
    let mut run=Run::new(42,erdtree_tower::Class::Samurai);
    let node=run.available()[0];run.enter(node).unwrap();run.win_combat().unwrap();
    let saved=suspended_progress(&run,1234,None).unwrap();
    let mut loaded=Run::decode(&saved.encode().unwrap()).unwrap();
    loaded.resume_from_hub().unwrap();assert_eq!(loaded.phase,run.phase);
    loaded.claim_reward(0).unwrap();
    let again=suspended_progress(&loaded,loaded.gold,None).unwrap();
    let mut restored=Run::decode(&again.encode().unwrap()).unwrap();restored.resume_from_hub().unwrap();
    assert_eq!(restored.claimed_items,loaded.claimed_items);assert_eq!(restored.visited,loaded.visited);
    assert!(restored.claim_reward(0).is_err());
}
#[test]
fn suspend_preserves_opened_event_boxes_and_pending_guards() {
    let mut run=Run::new(42,erdtree_tower::Class::Samurai);
    for _ in 0..15 {
        let node=run.available()[0];run.enter(node).unwrap();
        match run.nodes[node].kind {
            erdtree_tower::NodeKind::Event=>break,
            erdtree_tower::NodeKind::Monster|erdtree_tower::NodeKind::Elite|erdtree_tower::NodeKind::Boss=>{run.win_combat().unwrap();run.decline_reward().unwrap();},
            erdtree_tower::NodeKind::Camp=>run.leave_camp().unwrap(),
            erdtree_tower::NodeKind::Shop=>run.leave_shop().unwrap(),
            erdtree_tower::NodeKind::Chest=>run.finish_chest().unwrap(),
        }
    }
    run.regional_event=Default::default();run.regional_event.opened=2;run.regional_event.guards=true;run.regional_event.guard_slot=Some(1);
    let saved=suspended_progress(&run,700,None).unwrap();
    let mut restored=Run::decode(&saved.encode().unwrap()).unwrap();restored.resume_from_hub().unwrap();
    assert_eq!(restored.regional_event,run.regional_event);assert_eq!(restored.phase,run.phase);
}
fn emergency_end(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan)->Result<bool,String> {
    let Some(player)=world.main_player.as_ref() else{return Ok(false);};
    let data=unsafe {player.player_game_data.as_ref()};
    let owner=(data.character_id,game.save_slot);
    if game.save_slot<0{return Ok(false);}
    super::suspension::backup_restart(owner)?;
    if state.hub.is_none(){state.hub=Some(super::hub::Hub::new(owner,data.archetype)?);super::name_tags::load(owner);super::auras::load(owner);}
    super::legacy::set_owner(owner);
    if let Some(run)=state.view.run.as_ref() {
        let run=ended_progress(run);
        if let Ok(mut path)=SESSION_PATH.lock(){*path=Some(super::hub::path(owner,"json"));}
        record_checked(&run)?;
        super::run_screen::summary_with(super::run_log::finish(&run,super::run_log::read(&super::run_log::path(owner)),Some(owner)));
    }else if super::hub::path(owner,"json").exists() {
        let path=super::hub::path(owner,"json");
        let run=ended_progress(&Run::decode(&std::fs::read_to_string(&path).map_err(|e|e.to_string())?)?);
        std::fs::write(path,run.encode()?).map_err(|e|e.to_string())?;
        super::run_screen::summary_with(super::run_log::finish(&run,super::run_log::read(&super::run_log::path(owner)),Some(owner)));
    }
    state.hub.as_mut().unwrap().finish();
    let marker=super::hub::path(owner,"paused");
    if marker.exists(){std::fs::remove_file(marker).map_err(|e|e.to_string())?;}
    super::suspension::clear_auto(owner)?;
    super::reconnect::clear(owner)?;state.reconnect=None;state.suspension=Default::default();
    stop_map_combat(state,world);
    if let Some(room)=state.room.take(){room.retire(world);}
    if let Some(scene)=state.party_scene.take(){scene.retire(world);}
    if let Some(pending)=state.pending.take(){retire_group(&pending,world);}
    if let Some(link)=state.party.take() && link.host() {
        super::log("HOME_SYNC host ending party; retaining transport for five seconds");
        let mut departure=super::party::Departure::new(link);departure.tick();
        state.departure=Some(departure);
    }
    state.party_pending=None;state.party_error=None;state.command=None;state.material_award=None;state.skip_target=None;
    state.rest_end=Default::default();state.host_death=Default::default();
    state.party_death=Default::default();state.party_death_wave=None;super::seamless_revival::cancel();
    state.reward_wait=None;state.travel=None;state.flasks=None;state.event_start=None;state.survival_engaged=None;state.descent=None;state.stormveil=None;state.chariot_clock=None;
    super::custom_event_ui::close();super::custom_event_ui::set_survival_hud(None);
    clear_chariot()?;
    for flag in [45020970,45020971,10007990,30097990,35007990,45007990,super::map_guards::COMBAT] {let _=super::native_talk::set_flag(flag,false);}
    state.view.run=None;state.view.room=None;state.view.shared_event=false;state.view.busy=false;state.view.travelling=false;state.view.ballots.clear();
    state.identity=None;state.pad_capture.update(false);super::input::CAPTURE.store(false,std::sync::atomic::Ordering::Relaxed);
    super::log("HOME_ESCAPE: run ended; continuation cleared; returning to hub");
    report(state,"已强制结束本局，正在返回大赐福。");
    Ok(true)
}
pub fn tick() {
    static INPUT_MODE:std::sync::Once=std::sync::Once::new();
    INPUT_MODE.call_once(||super::log("PLAYER_INPUT unrestricted: movement/action locks and input interception disabled globally"));
    if let Ok(world)=unsafe {WorldChrMan::instance_mut()}{super::frost_shackle::begin_frame(world);}
    super::dance_motion::tick();
    if let Ok(mut state)=STATE.try_lock(){sync_map(&mut state);}
    tick_locked();
    if let Ok(mut state)=STATE.try_lock() {
        if let Ok(world)=unsafe {WorldChrMan::instance_mut()}{
            if let Some(player)=world.main_player.as_mut() {
                state.weapon_traits.sync(player,super::weapon_glow::clock());
            } else {
                state.weapon_traits=Default::default();
            }
            sync_action_guard(&mut state,world);
            super::starsurge::debug_tick(world);
            super::frost_shackle::finish_frame(world,state.view.travelling||state.view.run.is_none());
        }
        sync_map(&mut state);
        super::mausoleum::sync_visibility(state.view.run.as_ref());
        tick_clock(&state);
        let scope=state.view.run.as_ref().map(|run|(run.seed,run.act));
        let ready=state.view.ready&&!state.view.travelling;
        state.ember_notice.tick(scope,ready);
        let mut view=state.view.clone();let owner=state.identity;
        view.latencies=state.party.as_ref().map_or_else(Vec::new,|p|p.latencies());
        drop(state);
        if let Ok(mut ui)=UI.lock(){ui.publish(view,owner);}
    }
}
fn tick_clock(state:&Runtime) {
    let owner=state.identity.or_else(||state.hub.as_ref().map(|h|h.owner));
    let active=state.view.run.as_ref().filter(|r|!matches!(r.phase,Phase::Won|Phase::Dead)
        && !state.hub.as_ref().is_some_and(|h|h.active) && PLAYER_READY.load(std::sync::atomic::Ordering::Relaxed));
    let fight=active.and_then(|run|{
        let node=state.pending.as_ref().filter(|p|!p.completed && p.foes.iter().any(|f|f.requested.is_some()||f.fight.handle().is_some()))
            .and_then(|p|match p.next_run.phase {Phase::Combat{id}=>Some((id,0)),Phase::Node{id}=>Some((id,1+u64::from(run.regional_event.guard_slot.unwrap_or(0)))),_=>None});
        if node.is_some(){return node;}
        if let Some(p)=&state.party {
            if !p.host() && p.spawn && !p.victory && !p.defeated && let Phase::Combat{id}=run.phase {return Some((id,0));}
            if p.event.done.is_none() && let Some(scene)=&state.party_scene && scene.guards.as_ref().is_some_and(|g|g.spawn_requested())
                && let Some(erdtree_tower::party_events::Kind::Guards(slot))=p.event.job.map(|j|j.kind)
                && let Phase::Node{id}=scene.run.phase {return Some((id,1+u64::from(slot)));}
        }
        state.room.as_ref().filter(|r|r.hunter_defeated_at.is_none()&&r.hunter.as_ref().is_some_and(|g|g.spawn_requested())).map(|r|(r.node,4))
    });
    super::run_clock::tick(owner.zip(active),fight);
}
fn tick_locked() {
    super::star_stream::clear();
    super::spirits::begin_frame();
    let Ok(mut state) = STATE.try_lock() else {
        return;
    };
    if state.departure.as_mut().is_some_and(|departure|departure.tick()){state.departure=None;}
    // 无缝入口独自游玩仍可调试；实际组队时关闭。
    let in_party=state.party.is_some()||!solo_session();
    // 界面线程读：选职业时联机提示召唤师的风险
    IN_PARTY.store(in_party,std::sync::atomic::Ordering::Relaxed);
    super::debug_menu::apply_cheats(in_party);
    let lobby=state.view.run.is_none();
    if state.party.as_mut().is_some_and(|link|link.poll_force_end(lobby)) {
        super::log("HOME_SYNC received host force end before room processing");
        QUICK_END.store(true,std::sync::atomic::Ordering::Relaxed);
    }
    let action=UI.lock().ok().and_then(|mut ui|ui.take(&state.view,state.identity,state.command.is_some()));
    if let Some(action)=action {state.command=Some(action);state.view.busy=true;}
    let mut music=super::bgm::Frame::default();
    state.pad_capture.update(super::input::active());
    SOLO.store(state.party.is_none(),std::sync::atomic::Ordering::Relaxed);
    super::options::sync_dlc(state.party.as_ref().map(|link|link.party_dlc()));
    let Ok(game) = (unsafe { GameMan::instance_mut() }) else {
        state.archery.sync(None,None);
        super::boss_bars::sync(Vec::new());
        PLAYER_READY.store(false,std::sync::atomic::Ordering::Relaxed);
        state.view.ready = false;
        return;
    };
    if super::seamless::blocked(game.is_in_online_mode,super::seamless::active()) {
        state.archery.sync(None,None);
        super::boss_bars::sync(Vec::new());
        state.pad_capture.update(false);
        super::input::CAPTURE.store(false, std::sync::atomic::Ordering::Relaxed);
        state.view.ready = false;
        if state.command.take().is_some() {
            state.view.busy = false;
            report(&mut state, "请通过离线启动器进入游戏。");
        }
        return;
    }
    let Ok(world) = (unsafe { WorldChrMan::instance_mut() }) else {
        state.archery.sync(None,None);
        super::boss_bars::sync(Vec::new());
        PLAYER_READY.store(false,std::sync::atomic::Ordering::Relaxed);
        state.view.ready = false;
        return;
    };
    {
        let runtime=&mut *state;
        let run=runtime.view.run.as_ref().filter(|_|game.save_slot>=0 && !game.warp_requested && !runtime.hub.as_ref().is_some_and(|h|h.active)
            && world.main_player.as_ref().is_some_and(|p|runtime.identity==Some((unsafe {p.player_game_data.as_ref()}.character_id,game.save_slot))));
        runtime.archery.sync(world.main_player.as_mut().map(|p|&mut **p),run);
        super::tears::tick(world,runtime.view.run.as_ref().map(|r|r.seed));
    }
    PLAYER_READY.store(world.main_player.is_some()&&game.save_slot>=0,std::sync::atomic::Ordering::Relaxed);
    if let Some(player)=world.main_player.as_ref().filter(|_|game.save_slot>=0) {
        super::smith::trace(unsafe{player.player_game_data.as_ref()},||format!("tick save={} block={:?} warp={} party={} room={:?} run={:?}",
            game.save_slot,player.current_block_id,game.warp_requested,state.party.is_some(),state.room.as_ref().map(|r|(r.kind,r.node,r.view.loading)),state.view.run.as_ref().map(|r|(r.seed,r.act,&r.phase))),false);
    }
    clear_ailments_under_overlay(&mut state,world,game);
    super::debug_menu::tick_enemies(world);
    super::spirits::landing::tick(world,game.warp_requested);
    super::spawn_guard::tick(world,game);
    if state.view.resource_alert.is_none()&&let Some(bad)=resource_check().filter(|bad|!bad.is_empty()) {
        state.view.resource_alert=Some((1,bad.clone()));
        report(&mut state,format!("整合包文件不完整：{} 个场地文件缺失或与当前版本不一致，进入这些战斗会被拒绝。请关闭游戏，在启动器里点「校验并修复文件」。",bad.len()));
    }
    // 内置死亡不掉卢恩（祭品树枝的效果常驻）、清理南瓜头遗留旗帜摆件参数、修正山妖骨灰 AI 参数，并兜底抑制龙王平台原版 c4520 游离苏醒
    super::enemies::disable_lingering_props();
    super::skill_elements::apply_once();
    super::spirits::repair_custom_spirit_ai();
    super::enemies::suppress_stray_dragonlord(world);
    if let Some(player)=world.main_player.as_mut().filter(|_|!game.warp_requested) {super::keep_runes::tick(&mut player.chr_ins);}
    if RECOVER.swap(false,std::sync::atomic::Ordering::Relaxed) {
        let message=match recover_entry(&mut state,world,game) {Ok(())=>"正在重新载入当前入场点，路线与奖励保持不变。".to_string(),Err(error)=>error};
        super::log(format!("ENTRY_RECOVERY {message}"));report(&mut state,message);
    }
    if SKIP.swap(false,std::sync::atomic::Ordering::Relaxed) {
        let message=request_skip(&mut state);super::log(format!("SKIP requested: {message}"));report(&mut state,message);
    }
    if state.skip_target.is_some() {
        match apply_skip(&mut state,world,game) {
            Ok(true)=>{sync_action_guard(&mut state,world);return;},
            Ok(false)=>{},
            Err(error)=>{state.skip_target=None;report(&mut state,format!("跳过未完成：{error}"));},
        }
    }
    if game.save_slot>=0 && let Some(player)=world.main_player.as_ref() {
        let character=unsafe {player.player_game_data.as_ref()}.character_id;
        if let Err(error)=super::card_affixes::select((character,game.save_slot)){super::log_repeating(error);}
    }
    let chase_enabled=!game.warp_requested&&matches!(state.chariot_clock,Some(ChariotClock::Surviving(_)))
        &&state.view.run.as_ref().is_some_and(|r|r.regional_event.chariot&&r.regional_event.stormveil==Some(erdtree_tower::stormveil::Progress::Running));
    let Runtime{chariot_chase,party,..}=&mut *state;
    if let Err(error)=chariot_chase.tick(world,party.as_mut(),chase_enabled){super::log(error);}
    if state.hub.is_none()&&let Some(p)=world.main_player.as_ref().filter(|_|game.save_slot>=0) {
        let data=unsafe {p.player_game_data.as_ref()};let owner=(data.character_id,game.save_slot);
        if super::hub::path(owner,"paused").exists()||super::hub::path(owner,"autosave").exists(){match super::hub::Hub::new(owner,data.archetype){
            Ok(hub)=>{state.hub=Some(hub);if let Ok(mut path)=SESSION_PATH.lock(){*path=Some(super::hub::path(owner,"json"));}},
            Err(e)=>{report(&mut state,e);return;},
        }}
    }
    // 只要本机或任意队友在大赐福持有暂停/自动档，房主就允许通过二次确认按钮同步放弃并重开；不再对混合状态静默强清存档。
    let any_hub_paused=state.hub.as_ref().is_some_and(|h|h.active&&h.paused)
        ||state.party.as_ref().is_some_and(|p|p.gate.any_peer_resume());
    let restart_paused=!game.warp_requested&&state.view.run.is_none()
        &&state.hub.as_ref().is_some_and(|h|h.active)&&any_hub_paused
        &&state.party_error.is_none()&&state.party_pending.is_none()&&state.reconnect.is_none()
        &&world.main_player.as_ref().is_some_and(|p|i32::from(p.current_block_id)==super::hub::BLOCK&&p.chr_ins.modules.physics.standing_on_solid_ground)
        &&state.party.as_ref().is_some_and(|p|p.event.pause.ticket.is_none()&&p.can_restart_paused());
    let restart_context=state.party.as_ref().filter(|_|restart_paused).map_or(0,|p|p.config.session);
    super::suspension::RESTART_CONTEXT.store(restart_context,std::sync::atomic::Ordering::Release);
    let restart_request=super::suspension::RESTART_REQUEST.swap(0,std::sync::atomic::Ordering::AcqRel);
    if restart_request!=0 {
        if restart_paused&&restart_request==restart_context {
            super::log("PARTY_RESTART host confirmed abandoning paused journeys; using synchronized force end");
            QUICK_END.store(true,std::sync::atomic::Ordering::Relaxed);
        }else{super::run_screen::party_notice("队伍状态已变化，未放弃旅程。请等待全员返回大赐福并完成握手后重试。".into());}
    }
    if QUICK_END.load(std::sync::atomic::Ordering::Relaxed) {
        match emergency_end(&mut state,world,game) {
            Ok(true)=>{QUICK_END.store(false,std::sync::atomic::Ordering::Relaxed);},
            Ok(false)=>{sync_action_guard(&mut state,world);return;},
            Err(error)=>{QUICK_END.store(false,std::sync::atomic::Ordering::Relaxed);report(&mut state,format!("结算失败：{error}；可再次按 {} 重试",super::options::label(erdtree_tower::options::Hotkey::Settle)));sync_action_guard(&mut state,world);return;},
        }
    }
    if state.material_award.is_some() {
        if let Err(error)=tick_material_award(&mut state,world,game) {
            state.material_award=None;state.command=None;state.view.busy=false;
            if let Some(link)=state.party.as_mut(){link.stop(error.clone());}
            report(&mut state,error);
        }
        sync_action_guard(&mut state,world);return;
    }
    match tick_suspension(&mut state,world,game) {
        Ok(true)=>{sync_action_guard(&mut state,world);return;},Ok(false)=>{},
        Err(e)=>{
            super::suspension::notice(format!("暂退尚未完成：{e}"));
            let current=state.view.run.clone();
            let alive=world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag());
            if let Some(link)=state.party.as_mut().filter(|p|p.event.pause.ticket.is_some()) {
                let orders=link.poll(current.as_ref(),alive,false,false);
                for order in orders {let _=link.accept(&order);}
                if link.host()&&link.event.pause.started.is_some_and(|t|t.elapsed()>Duration::from_secs(30)){let _=link.abort_pause();}
                if link.event.pause.ticket.is_some(){state.view.busy=true;state.command=None;return;}
                if let Some(hub)=&state.hub{let _=super::suspension::clear(hub.owner);}
                state.suspension=Default::default();state.view.busy=false;state.view.room=None;
            }
        },
    }
    if state.view.run.is_none()&&!game.warp_requested&&state.hub.as_ref().is_some_and(|h|h.ready_for_party_restart()) {
        if let Some(player)=world.main_player.as_ref().filter(|p|i32::from(p.current_block_id)==super::hub::BLOCK) {
            let owner=(unsafe {player.player_game_data.as_ref()}.character_id,game.save_slot);
            if state.hub.as_ref().is_some_and(|h|h.owner==owner) {
                let path=super::hub::path(owner,"party-award");
                match super::material_award::archive_closed(&path) {
                    Ok(Some(backup))=>{
                        state.party=None;state.party_pending=None;state.party_error=None;state.command=None;state.view.busy=false;
                        super::log(format!("PARTY_AWARD archived without replay; rebuilding handshake: {backup:?}"));
                        report(&mut state,"旧局领奖记录已备份，未重复补发物品；正在重新建立联机握手。");
                    },
                    Ok(None)=>{},Err(error)=>{report(&mut state,format!("领奖记录备份失败，保留原记录：{error}"));},
                }
            }
        }
    }
    let now=Instant::now();
    if super::seamless_revival::available()&&state.hub.as_ref().is_some_and(|h|h.settling())&&!game.warp_requested {
        if let Some(p)=world.main_player.as_ref().filter(|p|p.chr_ins.modules.data.hp<=0||p.chr_ins.chr_flags1c5.death_flag()) {
            let pos=&p.chr_ins.modules.physics.position;
            let origin=super::party_death::Position{block:i32::from(p.current_block_id),xyz:[pos.0,pos.1,pos.2]};
            if let Err(error)=super::seamless_revival::request(p,origin){super::log(error);}
        }
        super::seamless_revival::tick(world,game);
    }
    if state.diagnostic_probe.is_none_or(|t|now.duration_since(t)>=Duration::from_millis(250)) {
        state.diagnostic_probe=Some(now);
        let player=world.main_player.as_ref().map(|p|(i32::from(p.current_block_id),p.chr_ins.modules.data.hp<=0||p.chr_ins.chr_flags1c5.death_flag()));
        let party=state.party.as_ref().map(|p|format!("host={} epoch={} arrived={} bound={} spawn={} fighting={} victory={} defeated={} stopped={}",p.host(),p.epoch,p.arrived,p.bound,p.spawn,p.fighting,p.victory,p.defeated,p.stopped.is_some()));
        let key=format!("STATE player={player:?} warp={} run={:?} ready={} busy={} travelling={} pending={} handshake={} party={party:?}",game.warp_requested,state.view.run.as_ref().map(|r|(r.seed,r.act,&r.phase)),state.view.ready,state.view.busy,state.view.travelling,state.pending.is_some(),state.party_pending.is_some());
        let grounded=world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.physics.standing_on_solid_ground);
        let stage=if game.warp_requested {"warp"}else if player.is_none(){"no_player"}else if player.is_some_and(|(block,_)|block<0){"map_unassigned"}
            else if !grounded {"not_grounded"}else if state.party.as_ref().is_some_and(|p|p.fighting&&p.bound&&!p.defeated){"combat_ready"}
            else if state.party.as_ref().is_some_and(|p|p.spawn&&!p.bound){"waiting_enemies"}
            else if state.party.as_ref().is_some_and(|p|p.arrived&&!p.fighting)&&state.view.travelling {"waiting_peers"}
            else if state.view.travelling {"waiting_center"}else if state.pending.is_some(){"waiting_enemies"}
            else if state.room.as_ref().is_some_and(|r|r.view.loading){"room_loading"}else{"room_ready"};
        let menu_bits=unsafe {eldenring::cs::CSMenuManImp::instance()}.ok().map(|menu| {
            let mut created=[0u64;2];let mut visible=[0u64;2];
            for (i,s) in menu.ui_states.iter().enumerate(){if s.created(){created[i/64]|=1u64<<(i%64);}if s.visible(){visible[i/64]|=1u64<<(i%64);}}
            (created,visible)
        });
        let created=menu_bits.map(|b|b.0);let visible=menu_bits.map(|b|b.1);
        let pos=world.main_player.as_ref().map(|p|{let v=&p.chr_ins.modules.physics.position;[v.0,v.1,v.2]});
        let block_pos=world.main_player.as_ref().map(|p|[p.block_position.x,p.block_position.y,p.block_position.z]);
        let physics=world.main_player.as_ref().map(|p|{
            let physics=&p.chr_ins.modules.physics;
            format!("touching_solid={} touching_ground={} falling={} gravity_disabled={} fade_gravity_disabled={} proxy_update_requested={} high_collision_adjust={} character_disabled={}",
                physics.touching_solid_ground,physics.is_touching_ground,physics.is_falling,physics.gravity_disabled,physics.fade_out_gravity_disabled,
                physics.chr_proxy_pos_update_requested,physics.adjust_to_hi_collision,p.chr_ins.debug_flags.character_disabled())
        });
        let arrival_flags=state.travel.as_ref().map(|_|[45007990,45007992,45007993].map(|id|super::native_talk::flag(id).ok()));
        let signature=format!("trace=3 player={player:?} grounded={grounded} physics={physics:?} arrival_flags={arrival_flags:?} target_block={:08x} entry={} ui_created={created:x?} ui_visible={visible:x?} party={party:?}",i32::from(game.move_map_target),game.initial_area_entity_id);
        let trigger=game.warp_requested||state.view.travelling;
        let detail=if state.load_trace.needs_detail(now,trigger,super::diagnostics::verbose()) {
            format!("{signature} position={pos:?} block_position={block_pos:?} loading_screen=unverified seamless_release=unverified")
        } else {
            let player=player.map(|(block,dead)|format!("{block:08x}/{}",if dead{"dead"}else{"alive"})).unwrap_or_else(||"-".into());
            let pos=pos.map(|p|format!("[{:.2},{:.2},{:.2}]",p[0],p[1],p[2])).unwrap_or_else(||"-".into());
            format!("player={player} target_block={:08x} entry={} grounded={grounded} position={pos} party={}",i32::from(game.move_map_target),game.initial_area_entity_id,party.as_deref().unwrap_or("-"))
        };
        if let Some(line)=state.load_trace.observe_state(now,trigger,stage,&signature,&detail){super::log(line);}
        let landing=state.travel.as_ref().filter(|_|state.view.run.as_ref().is_some_and(|r|super::arenas::for_run(r,state.party.is_some()).uses_circle())).map(|(travel,start,_,_)|(travel.node,*start));
        let host=state.party.as_ref().map(|p|p.host());
        state.landing_trace.tick(now,landing,world,game,host);
        if state.diagnostics.due(key.clone(),now) {
            super::log(format!("{key} hp={:?}",world.main_player.as_ref().map(|p|p.chr_ins.modules.data.hp)));
        }
    }
    super::grace_travel::sync(state.view.run.as_ref().map(|r|&r.phase),
        !game.warp_requested&&world.main_player.as_ref().is_some_and(|p|i32::from(p.current_block_id)==super::hub::BLOCK));
    let active=state.view.run.as_ref().is_some_and(|r|!matches!(r.phase,Phase::Dead|Phase::Won));
    let resting=unsafe {eldenring::cs::CSEventManImp::instance()}.ok().is_some_and(|event|{
        let rest=&event.world_area_time;
        rest.bonfire_entity_id!=0&&rest.fade_out_requested&&rest.reset_main_character
    });
    let rested=state.rest_end.observe(active,resting);
    let shortcut=QUICK_END.swap(false,std::sync::atomic::Ordering::Relaxed);
    let quick_end=shortcut&&state.rest_end.request(active);
    if rested||quick_end {
        super::log(format!("RUN_END reason={} party={} host={}; ending journey",if quick_end{"HOME"}else{"REST"},state.party.is_some(),state.party.as_ref().is_none_or(|p|p.host())));
        state.command=None;
        if state.party.is_none() {
            stop_map_combat(&state,world);
            if let Some(run)=state.view.run.as_mut(){run.end_on_death();record(run);}
            report(&mut state,if quick_end{format!("已按 {} 结束本局。",super::options::label(erdtree_tower::options::Hotkey::Settle))}else{"已在赐福休息，本局结束。".into()});
        }
    }
    super::seamless::trace_session();
    recover_party_at_hub(&mut state,world,game);
    let end_requested=super::native_talk::END_REQUESTED.swap(false,std::sync::atomic::Ordering::Relaxed);
    let end_allowed=can_end_run(state.view.run.as_ref().map(|r|&r.phase),state.party.as_ref().is_none_or(|p|p.host()),
        game.warp_requested||state.party_error.is_some()||state.party_pending.is_some()||state.party.as_ref().is_some_and(|p|p.stopped.is_some()));
    super::native_talk::END_ALLOWED.store(end_allowed,std::sync::atomic::Ordering::Relaxed);
    if end_requested&&end_allowed {
        let result=state.party.as_mut().map_or(Ok(()),|p|p.end_run());
        if let Err(error)=result {report(&mut state,error);sync_action_guard(&mut state,world);return;}
        stop_map_combat(&state,world);
        if let Some(run)=state.view.run.as_mut(){run.end_on_death();record(run);}
        state.command=None;
        super::log("Journey voluntarily ended; returning party to hub through normal settlement");
    }
    if state.party.is_none()&&state.reconnect.is_some()&&solo_session(){state.reconnect=None;state.party_error=None;sync_action_guard(&mut state,world);}
    if let Some(error)=state.party_error.clone(){state.view.ready=false;state.command=None;report(&mut state,error);poll_blocked_hub(&mut state,world,game);return;}
    let party_waiting=match tick_auto_party(&mut state,world,game) {
        Ok(waiting)=>waiting,
        Err(error)=>{
            if state.reconnect.take().is_some() {
                super::log(format!("RECONNECT failed; wait released, saved progress retained: {error}"));
            }
            if let Some(link)=state.party.as_mut(){link.stop(error.clone());}
            state.party_error=Some(error.clone());state.view.ready=false;state.command=None;report(&mut state,error);poll_blocked_hub(&mut state,world,game);return;
        },
    };
    if party_waiting {state.view.ready=false;state.command=None;}
    if let Some(status)=state.reconnect.as_ref().map(|p|p.waiting_status()) {
        state.view.ready=false;state.command=None;
        report(&mut state,status);
        sync_action_guard(&mut state,world);
        return;
    }
    if state.party.is_some() {
        if let Err(error)=tick_party(&mut state,world,game){
            if let Some(link)=state.party.as_mut(){link.stop(error.clone());}
            stop_map_combat(&state,world);
            state.view.ready=false;state.view.busy=false;state.command=None;report(&mut state,error);poll_blocked_hub(&mut state,world,game);return;
        }
    }
    super::debug_menu::set_party_role(state.party.as_ref().map(|p|p.host()));
    if let Some(command)=super::debug_menu::take_test() {
        let result=debug_boss_command(&mut state,world,game,command);
        if let Err(error)=result {super::log(format!("BOSS_TEST rejected: {error}"));report(&mut state,error);}
    }
    if QUICK_END.load(std::sync::atomic::Ordering::Relaxed){state.radahn_probe=None;sync_action_guard(&mut state,world);return;}
    tick_radahn_probe(&mut state,world);
    if let Some(player)=world.main_player.as_ref() {
        let current=(unsafe {player.player_game_data.as_ref()}.character_id,game.save_slot);
        if (game.event_world_type==EventWorldType::Local||super::seamless::active())&&game.save_slot>=0&&owner_changed(state.hub.as_ref().map(|h|h.owner),current) {
            super::log(format!("Character changed: clearing prior runtime, binding {current:?}"));
            *state=fresh_runtime();
            if let Ok(mut path)=SESSION_PATH.lock(){*path=None;}
            super::input::CAPTURE.store(false,std::sync::atomic::Ordering::Relaxed);
        }
    }
    let hunter_summon=state.room.as_ref().filter(|room|room.hunter.as_ref().is_some_and(|h|h.ready())&&room.hunter_defeated_at.is_none())
        .zip(state.view.run.as_ref().filter(|r|r.bells.challenge.is_some()))
        .filter(|_|state.party.as_ref().is_none_or(|p|p.event.merchant_phase>=2))
        .map(|(room,run)|(state.party.as_ref().map_or_else(||(u64::from(run.act)<<32)|room.node as u64,|p|p.epoch),room.block));
    let solo_scarab_summon=(state.party.is_none()).then(||state.room.as_ref().filter(|room|room.scarabs.active())
        .zip(state.view.run.as_ref()).map(|(room,run)|((u64::from(run.act)<<32)|room.node as u64,run.regional_event.attempts,room.block))).flatten();
    let party_summon=state.party.as_ref().and_then(|link|{
        if link.event.combat_active()||link.event.scarabs_active() {
            state.party_scene.as_ref().map(|scene|(link.epoch,scene.wave,scene.block))
        }else if !link.host()&&!link.victory&&!link.defeated&&(link.bound||state.view.run.as_ref().is_some_and(|r|r.embers.contains(&erdtree_tower::embers::Ember::SpiritTwin))) {
            state.view.run.as_ref().filter(|run|matches!(run.phase,Phase::Combat{..})).map(|run|(link.epoch,0,super::arenas::for_run(run,true).block))
        }else {None}
    });
    let summon_scope=extra_summon_scope(party_summon,hunter_summon,solo_scarab_summon);
    let summon_available=summon_scope.is_some_and(|(_,_,block)|!game.warp_requested&&world.main_player.as_ref().is_some_and(|p|
        i32::from(p.current_block_id)==block&&p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()));
    let music_active=state.pending.as_ref().is_some_and(|p|!p.completed)
        && world.main_player.as_ref().is_some_and(|p|state.pending.as_ref().is_some_and(|fight|fight.block==i32::from(p.current_block_id))&&p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag())
        && !game.warp_requested;
    let local_combat=music_active||summon_available;
    let solo_music=state.pending.as_ref().is_some_and(|p|!p.completed&&p.foes.iter().any(|f|f.fight.handle().is_some()));
    let party_music=state.party.as_ref().is_some_and(|p|p.event.combat_active()||(p.fighting&&p.bound&&!p.victory&&!p.defeated));
    music.active=super::bgm::playing(world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()),
        game.warp_requested,state.view.travelling,solo_music&&music_active,party_music&&local_combat);
    let music_run=state.party_scene.as_ref().filter(|_|state.party.as_ref().is_some_and(|p|p.event.combat_active())).map(|s|&s.run).or(state.view.run.as_ref());
    let music_wave=state.party.as_ref().and_then(|p|p.event.job.filter(|_|p.event.combat_active())).map_or_else(||music_run.and_then(|r|r.regional_event.guard_slot).unwrap_or(0),|j|j.wave);
    if let Some(player)=world.main_player.as_ref() {
        music.track=super::bgm::selection(music_run,i32::from(player.current_block_id),player.chr_ins.modules.data.hp>0&&!player.chr_ins.chr_flags1c5.death_flag(),game.warp_requested,state.view.travelling,music.active,music_wave);
        // 癫火舞会放舞曲时暂停背景音乐
        if super::dance_ui::dancing() {music.track=None;}
    }
    let run=state.view.run.as_ref();
    let twin=run.is_some_and(|r|r.embers.contains(&erdtree_tower::embers::Ember::SpiritTwin));
    let preparing_twin=twin && state.travel.as_ref().is_some_and(|(t,..)|t.summon_stone_ready())
        && !game.warp_requested && run.is_some_and(|r|world.main_player.as_ref().is_some_and(|p|
            i32::from(p.current_block_id)==super::arenas::for_run(r,state.party.is_some()).block
            &&p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()));
    let lone_wolf=super::law::lone_wolf(run);
    super::spirits::set_active((local_combat||preparing_twin)&&!lone_wolf);
    // 召唤师「群灵」：召唤物全部阵亡后可以再召唤
    super::spirits::set_resummon(local_combat&&state.pending.as_ref().map(|p|&p.next_run).or(state.view.run.as_ref()).is_some_and(|r|r.summoner()&&r.class_rules>0));
    super::spirits::set_twin((local_combat||preparing_twin)&&twin&&!lone_wolf);
    super::spirits::set_party(state.party.is_some());
    super::enemies::set_party(state.party.is_some());
    if let Some(notice)=super::spirits::take_cap_notice() {report(&mut state,notice);}
    // 苦难律法「孤狼」：与房主、单人相同，联机成员与守卫战也不开放召唤点
    if preparing_twin&&!lone_wolf && let Some((travel,..))=state.travel.as_mut() && let Some(player)=world.main_player.as_ref() {
        if let Err(error)=travel.spirits.poll(world,i32::from(player.current_block_id),true){super::log_repeating(format!("Precombat spirit registration: {error}"));}
    }
    if let Some(spirits)=encounter_state(&mut state.party_spirits,summon_scope,summon_available&&!lone_wolf) {
        if let Err(error)=spirits.poll(world,summon_scope.unwrap().2,true){super::log(format!("Party spirit registration: {error}"));}
    }
    let bar_affixes=state.pending.as_ref().map(|p|&p.next_run).or(state.view.run.as_ref())
        .and_then(|run|if let Phase::Combat{id}=run.phase {Some(erdtree_tower::boss_affixes::roll(run,id))}else{None}).unwrap_or_default();
    let mut bars:Vec<_>=if music_active {state.pending.as_ref().map(|pending|pending.foes.iter().filter_map(|foe|{
        let handle=foe.fight.handle()?;let chr=world.chr_ins_by_handle(&handle)?;
        if foe.entity_address!=Some(std::ptr::from_ref(chr) as usize)||chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(){return None;}
        let rank=super::enemies::bar_rank(&foe.spec);let name=super::enemies::bar_text(&foe.spec)?;
        Some((rank,if rank==0 {super::boss_affixes::bar_name(&bar_affixes,name)}else{super::boss_affixes::elite_bar_name(&bar_affixes,name)},handle))
    }).collect()).unwrap_or_default()}else{Vec::new()};
    if state.party.as_ref().is_some_and(|p|!p.host()&&p.bound&&!p.victory)&&!game.warp_requested {
        if let Some(run)=state.view.run.as_ref() {if let Phase::Combat{id}=run.phase {
            let arena=super::arenas::for_run(run,true);
            if world.main_player.as_ref().is_some_and(|p|i32::from(p.current_block_id)==arena.block&&p.chr_ins.modules.data.hp>0) {
                let roster=super::enemies::node_roster(run,id);
                let ids=super::enemies::map_entities(&roster).unwrap_or_default();
                for (slot,spec) in roster.iter().enumerate() {
                    let Some(name)=super::enemies::bar_text(spec) else{continue};
                    if let Some(chr)=world.chr_sets.iter().flatten().flat_map(|set|set.characters())
                        .find(|chr|ids.get(slot)==Some(&chr.event_entity_id)&&chr.npc_id==spec.model&&chr.npc_param_id==spec.npc&&chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag()&&enemy_loaded(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status)) {
                        let rank=super::enemies::bar_rank(spec);
                        bars.push((rank,if rank==0 {super::boss_affixes::bar_name(&bar_affixes,name)}else{super::boss_affixes::elite_bar_name(&bar_affixes,name)},chr.field_ins_handle));
                    }
                }
            }
        }}
    }
    // 商店的铃珠猎人不走战斗流程，单独接上 Boss 血条
    if let Some(hunter)=state.room.as_ref().and_then(|room|room.hunter.as_ref()) {bars.extend(hunter.bars(world));}
    super::boss_bars::sync(super::boss_bars::select(bars));
    // 联机大厅（房主由启动器开房时才有房间文件）：人数与进度每 5 秒同步到 Steam 大厅
    {
        // 旅程出发后（不在大赐福、旅程没结束）房间标为“旅程中”，启动器里不能加入
        let journey=state.view.run.as_ref().filter(|run|!state.hub.as_ref().is_some_and(|h|h.active)&&!matches!(run.phase,Phase::Dead|Phase::Won));
        let progress=journey.map_or_else(||"大赐福".to_string(),|run|format!("第 {} 幕",run.act));
        super::lobby::tick(&progress,journey.is_some());
    }
    // 武器强化基准 = 本局历史最高（Run::weapon_peak，只升不降）：每帧把记录交给 smith；背包里出现更高的武器时（强化、拿到奖励）
    // 每秒检查一次，不在战斗流程里就立刻记进存档，之后卖掉、丢掉或存进仓库都不会让基准掉下来
    {
        let recorded=state.view.run.as_ref().filter(|r|!matches!(r.phase,Phase::Dead|Phase::Won)).map_or(0,erdtree_tower::rewards::shared_upgrade_peak);
        super::smith::set_run_peak(recorded);
        static CHECKED:std::sync::Mutex<Option<Instant>>=std::sync::Mutex::new(None);
        let due=CHECKED.lock().is_ok_and(|mut at|{let due=at.is_none_or(|t|t.elapsed()>=Duration::from_secs(1));if due{*at=Some(Instant::now());}due});
        let steady=state.pending.is_none()&&!state.view.travelling&&state.view.run.as_ref().is_some_and(|r|matches!(r.phase,Phase::Map|Phase::Node{..}));
        if due&&steady&&let Some(player)=world.main_player.as_ref()&&let Ok(params)=unsafe {SoloParamRepository::instance()} {
            let live=super::smith::effective_peak(unsafe {player.player_game_data.as_ref()},params).clamp(0,i32::from(erdtree_tower::contracts::MAX_PEAK)) as u8;
            let spirit=(live/5).min(erdtree_tower::rewards::SPIRIT_MAX);
            if let Some(run)=state.view.run.as_ref().filter(|r|live>r.weapon_peak.unwrap_or(0)||spirit>r.spirit_peak.unwrap_or(0)) {
                let mut next=run.clone();next.weapon_peak=Some(live);next.spirit_peak=Some(next.spirit_peak.unwrap_or(0).max(spirit));
                match record_checked(&next) {
                    Ok(())=>{super::log(format!("Weapon peak recorded: {recorded} -> {live}"));super::smith::set_run_peak(live);state.view.run=Some(next);},
                    Err(error)=>super::log_repeating(format!("Weapon peak not recorded: {error}")),
                }
            }
        }
    }
    if !game.warp_requested {member_combat(&mut state,world);}
    // [fix] Diagnostic switch: ERDTREE_TOWER_NO_ATTENDANT=1 skips the arena-side NPC entirely, to test
    // whether it is what triggers the arena-arrival crash. The fog gate and fight do not need it.
    let attendant_allowed=!attendant_disabled()&&state.room.is_none()&&!game.warp_requested
        &&world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.data.hp>0&&if super::arenas::for_block(i32::from(p.current_block_id)).is_some_and(|a|a.uses_circle()) {
            (p.chr_ins.modules.physics.standing_on_solid_ground||state.attendant.is_some())
                &&(state.travel.is_some()||state.pending.is_some()||state.reward_wait.is_some()||member_live(&state))
        }else{
            (state.travel.is_some()&&state.view.run.as_ref().is_some_and(|r|super::arenas::for_run(r,state.party.is_some()).block==i32::from(p.current_block_id))&&super::native_talk::flag(45007993).unwrap_or(false))
            ||state.pending.as_ref().is_some_and(|fight|fight.block==i32::from(p.current_block_id)&&fight.foes.iter().all(|f|f.entity_address.is_some()))
            // 联机成员没有 Pending：敌人同步后、收到胜利前同样留着
            ||member_live(&state)&&state.view.run.as_ref().is_some_and(|r|super::arenas::for_run(r,true).block==i32::from(p.current_block_id))});
    if attendant_allowed {
        if state.attendant.is_none() {
            match super::rooms::Room::attendant(i32::from(world.main_player.as_ref().unwrap().current_block_id)) {Ok(room)=>state.attendant=Some(room),Err(e)=>super::log(e)}
        }
        let roster=if let Some((travel,_,_,_))=&state.travel {
            state.view.run.as_ref().map(|run|super::enemies::node_roster(run,travel.node)).unwrap_or_default()
        }else if let Some(pending)=state.pending.as_ref() {pending.foes.iter().map(|f|f.spec).collect()
        }else{state.view.run.as_ref().and_then(|run|if let Phase::Combat{id}=run.phase {Some(super::enemies::node_roster(run,id))}else{None}).unwrap_or_default()};
        let texts=super::enemies::preview_texts(&roster);
        if let Some(room)=state.attendant.as_mut() {room.set_enemies(&texts);if let Err(e)=room.poll(world,game){super::log_repeating(format!("Arena attendant: {e}"));}}
    } else if let Some(room)=state.attendant.take() {room.retire(world);}
    // 称号：本机选用的称号随汇报发给房主；原版友方名牌按全队称号显示“称号·名字”（单人时恢复原名）
    if let Some(link)=state.party.as_mut() {link.title=super::name_tags::worn();}
    super::name_tags::sync(world,state.party.as_ref().map_or(&[][..],|l|&l.titles));
    // 篝火营地的测伤假人：营地布置好后生成，离开营地撤下。联机时每人一个、只在本机生成（不同步），只统计自己的伤害；
    // 营地的服务 NPC 用的是地图实体，不占用调试生成接口
    let camp=!game.warp_requested&&state.room.as_ref().is_some_and(|r|r.kind==erdtree_tower::NodeKind::Camp&&!r.view.loading);
    let practice=!game.warp_requested&&!state.view.travelling&&state.pending.is_none()&&!member_fighting(&state)
        &&state.hub.as_ref().is_none_or(|h|!h.active)
        &&state.view.run.as_ref().is_some_and(|r|!matches!(r.phase,Phase::Combat{..}|Phase::Dead|Phase::Won));
    if practice {
        let run=state.view.run.as_ref().unwrap();
        let held=super::embers::practice_held(&super::embers::held(Some(run)));
        let node=if let Phase::Node{id}=run.phase {id}else{usize::MAX};
        let key=(run.seed,run.act,node);
        let keeper=run.ember_stacks.level(erdtree_tower::embers::Ember::SpiritKeeper);
        let foes:Vec<_>=if camp {state.dummy.as_ref().and_then(|d|d.target(world)).map(|h|vec![(h,3,0)]).unwrap_or_default()}else{vec![]};
        let flasks=state.flasks.as_ref().map(|f|f.total());
        // Practice never forwards combat rewards or persistent stack gains.
        let _=state.ember_practice.tick(world,key,&held,&foes,true,false,flasks,keeper,false);
        // 星辉术士：营地的测伤人偶不交给 starsurge；商人房、宝箱房等名单外的战斗靠它扫身边的敌人（联机也算）
        // 联机成员：名单外的敌人没有转发通道，打满只出特效、不在本机扣额外的血（扣了也会被房主同步盖掉）；叠层照常
        let member=state.party.as_ref().is_some_and(|p|!p.host());
        // 房间还在加载（刚开始切图、地图正在卸载）时不扫：这时场上的角色、弹体可能正在销毁
        let settled=state.room.as_ref().is_none_or(|r|!r.view.loading);
        if settled {if member {super::starsurge::fight_unsynced(world);}else{super::starsurge::fight(world,&[],state.party.is_some(),None);}}
        super::frost_shackle::observe(world,key,&foes,held.contains(&erdtree_tower::embers::Ember::FrostShackle));
        super::embers::clear_practice_critical();
    } else {
        if state.ember_practice.active() {super::embers::clear_practice_critical();}
        state.ember_practice=Default::default();
    }
    if camp {
        let clock=super::weapon_glow::clock();
        let act=state.view.run.as_ref().map_or(1,|r|r.act);
        let dummy=state.dummy.get_or_insert_with(Default::default);
        if let Err(error)=dummy.tick(world,clock,act){super::log_repeating(format!("Damage dummy: {error}"));}
        state.view.dummy=Some(dummy.view(clock));
    } else if let Some(mut dummy)=state.dummy.take() {dummy.retire(world);state.view.dummy=None;}
    if practice {state.ember_practice.observe_practice_refill(world);}
    let contract_run=state.view.run.as_ref().filter(|_|state.hub.as_ref().is_some_and(|h|!h.active));
    let spirit_alive=super::embers::living_spirit(world);
    let spirit_link=super::embers::linkable_spirit(world);
    let ember_state=if practice {&state.ember_practice}else{&state.ember_fight};
    if let Some(player)=world.main_player.as_mut(){let greatshield_guard=super::embers::bulwark_guard(player);let light=super::contracts::light_load(unsafe {player.player_game_data.as_ref()});super::contracts::sync(&mut player.chr_ins,contract_run,light);super::elite::sync(&mut player.chr_ins,contract_run);super::passives::sync(&mut player.chr_ins,contract_run);super::auras::sync(&mut player.chr_ins);super::law::sync(&mut player.chr_ins,contract_run,state.law_fight.overtime());super::embers::sync(&mut player.chr_ins,contract_run,ember_state,spirit_alive,spirit_link,greatshield_guard);let data=player.player_game_data.as_ptr();if !game.warp_requested&&i32::from(player.current_block_id)!=-1&&state.hub.as_ref().is_none_or(|h|!h.active){super::card_affixes::observe(unsafe {&*data},super::native_talk::shop_menu_open(),super::native_talk::sell_menu_open());}super::starsurge::sync(&mut player.chr_ins,contract_run);super::weapon_glow::sync(&mut player.chr_ins,unsafe {&*data},super::weapon_glow::clock());super::affix_stack::sync(&mut player.chr_ins,unsafe {&*data});}
    // 处决期间敌人不再无敌，我方照旧无敌（旅程进行中）
    super::throw_guard::sync(world,contract_run.is_some_and(|run|!matches!(run.phase,Phase::Dead|Phase::Won)));
    if let Some(player)=world.main_player.as_mut(){super::auras::countdown(&mut player.chr_ins,false);}
    if !game.warp_requested&&!state.view.travelling {
        if let Some(player)=world.main_player.as_mut(){super::star_stream::tick(&mut player.chr_ins,contract_run);}
    }
    sync_action_guard(&mut state,world);
    if let Some(player)=world.main_player.as_mut() {
        let data=unsafe {player.player_game_data.as_mut()};
        data.unlocked_talisman_slots=4;
        super::loadout::repair_item_icons(data);
        if !game.warp_requested&&!state.view.travelling&&state.view.run.is_some() {
            let _=super::loadout::repair_weapon_slots(data);
        }
    }
    let Some(player) = world.main_player.as_ref() else {
        state.view.ready = false;
        if let Some((travel, started, _, _)) = &mut state.travel {
            travel.unloaded_after_arrival |= travel.saw_arena;
            if !travel.saw_arena && travel.poll(started.elapsed(), false, false) == super::travel::Progress::Failed {
                state.travel = None;
                state.resume_combat=state.view.run.as_ref().and_then(|r|r.preparing_combat);
                state.view.travelling = false;
                state.view.busy = false;
                report(&mut state, "传送超时，未生成敌人，路线没有推进。");
            }
            return;
        }
        if state.command.take().is_some() {
            state.view.busy = state.pending.is_some();
        }
        if state
            .pending
            .as_ref()
            .is_some_and(|pending| !pending.completed)
        {
            state.pending.as_mut().unwrap().completed = true;
            state.view.busy = false;
            report(
                &mut state,
                "角色已卸载，战斗验证停止，不发放奖励。请重新启动游戏。",
            );
        }
        return;
    };
    let player_data = unsafe { &mut *player.player_game_data.as_ptr() };
    state.view.runes = player_data.rune_count;
    if let Err(error)=super::spell_affixes::sync(player_data){super::log_repeating(format!("Spell affix sync: {error}"));}
    if state.build_checked.is_none_or(|t|t.elapsed()>=Duration::from_secs(1)) {
        state.build_checked=Some(Instant::now());state.view.build=equipped_build(player_data);
        state.view.stats=super::stats::collect(&player.chr_ins);
    }
    if let Some(run)=state.view.run.as_mut(){run.gold=player_data.rune_count;}
    let deepen_due=state.deepen_retry.is_none_or(|t|t.elapsed()>=Duration::from_secs(2));
    if deepen_due&&state.view.run.as_ref().is_some_and(|r|r.contract.as_ref().is_some_and(|c|c.deepened&&!c.gear_done)) {state.deepen_retry=Some(Instant::now());}
    if let Some(run)=state.view.run.as_mut() {
        if run.contract.is_none()&&run.phase==Phase::Map {run.enable_contracts();record(run);}
        if let Some(contract)=run.contract.as_mut(){super::contracts::observe_peak(player_data,contract);}
        if deepen_due&&run.contract.as_ref().is_some_and(|c|c.deepened&&!c.gear_done) {
            match super::contracts::deliver_deepen(player_data,run) {Ok(true)=>record(run),Ok(false)=>{},Err(error)=>super::log_repeating(format!("Oath deepen gift: {error}"))}
        }
    }
    let identity = (player_data.character_id, game.save_slot);
    super::transmog::sync(identity);
    if game.event_world_type != EventWorldType::Local && state.party.is_none() && !super::seamless::active() {return;}
    // 联机客机没有本地的战斗记录（敌人由房主生成）：同步上敌人、还没收到胜利的这段时间也算在战斗中，
    // 否则客机拍不到开战时的骨灰快照，召唤师「群灵」胜利时补不回骨灰
    let client_fighting=state.party.as_ref().is_some_and(|p|!p.host()&&!p.victory&&!p.defeated)
        &&state.view.run.as_ref().is_some_and(|r|matches!(r.phase,Phase::Combat{..}));
    // 联机的守卫战（所有人都没有 Pending）也拍快照，胜利时补骨灰要用
    // 召唤师「群灵」也覆盖房间里的挑战：铃珠猎人、粪金龟狩猎、史东薇尔突围与泥头车生存（这些不走普通战斗流程，单独算进来）
    let hunter_active=state.room.as_ref().is_some_and(|room|room.hunter.is_some()&&room.hunter_defeated_at.is_none())
        &&state.view.run.as_ref().is_some_and(|r|r.bells.challenge.is_some());
    let room_challenge=hunter_active||state.room.as_ref().is_some_and(|room|room.scarabs.active())
        ||state.view.run.as_ref().is_some_and(|run|run.regional_event.stormveil==Some(erdtree_tower::stormveil::Progress::Running));
    let ash_fighting=ash_tracking_active(state.travel.is_some(),state.pending.as_ref().is_some_and(|pending|!pending.completed)||client_fighting||state.party.as_ref().is_some_and(|p|p.event.combat_active()||p.event.scarabs_active())||room_challenge);
    let service_preparing=state.room.as_ref().is_some_and(|room|!room.view.loading)
        &&state.view.run.as_ref().is_some_and(|run|!matches!(run.phase,Phase::Dead|Phase::Won));
    let ash_context=ash_fighting||service_preparing;
    let ash_owner_alive=player.chr_ins.modules.data.hp>0&&!player.chr_ins.chr_flags1c5.death_flag();
    let ash_journey=state.view.run.as_ref().is_some_and(|r|!matches!(r.phase,Phase::Dead|Phase::Won));
    if ash_owner_alive&&ash_journey&&state.ash_guard.is_none() {
        let plan=super::spirits::refund::recover(state.view.run.as_ref().is_some_and(|r|r.summoner()));
        if !plan.is_empty(){state.ash_refunds=plan;state.ash_guard=Some(Instant::now());state.ash_revival_refund=true;}
    }
    if ash_tracking_start(ash_context&&ash_owner_alive,state.ash_tracking&&super::spirits::refund::tracking(),
        state.ash_guard.is_some(),world.summon_buddy_manager.disappear_requested) {
        match super::loadout::ash_snapshot(player_data) {
            Ok(before)=>{
                super::spirits::refund::begin(&before);state.ash_before=before;state.ash_refunds.clear();state.ash_tracking=true;super::log(format!("Spirit ash preparation snapshot: {:?}",state.ash_before));
                // 开战时记下背包里骨灰的最高等级：骨灰召唤后会用掉，胜利时背包里可能已经没有它
                let held=super::spirit_tune::inventory_peak(player_data);
                let runtime=&mut *state;
                for run in runtime.view.run.iter_mut().chain(runtime.pending.iter_mut().map(|p|&mut p.next_run)) {
                    if run.spirit_peak.is_some_and(|peak|peak<held) {run.spirit_peak=Some(held);}
                }
                if let Some(run)=state.view.run.as_ref() {record(run);}
            },
            Err(error)=>super::log_repeating(format!("Spirit ash snapshot failed: {error}")),
        }
    } else if !ash_context&&(ash_owner_alive||!ash_journey) {
        if state.ash_tracking {super::spirits::refund::close();}
        state.ash_tracking=false;
    }
    if ash_context||(!ash_owner_alive&&ash_journey&&state.ash_tracking) {super::spirits::refund::observe(world);}
    if ash_journey {
        let losses=super::spirits::refund::take_losses();
        if !losses.is_empty(){
            let runtime=&mut *state;
            for run in runtime.view.run.iter_mut().chain(runtime.pending.iter_mut().map(|p|&mut p.next_run)){
                run.fallen_ashes.extend(losses.iter().copied().filter(|id|erdtree_tower::rewards::spirit_level(*id).is_some()));
            }
            if let Some(run)=runtime.view.run.as_ref(){record(run);}
        }
    }
    // 双生的灵唤铃 / 召唤师再召唤：召唤被原版拒绝时，补回白扣的骨灰（扣除可能稍晚于请求，5 秒内重试）
    if let Some(id)=super::spirits::take_twin_refund() {state.twin_refund=Some((id,Instant::now()));}
    if let Some((id,since))=state.twin_refund {
        match super::spirits::refund::refund_rejected(player_data,&state.ash_before,id) {
            Ok(0) if since.elapsed()<Duration::from_secs(5)=>{},
            Ok(count)=>{super::log(format!("TOWER_TWIN rejected ash {id} returned: {count}"));state.twin_refund=None;},
            Err(error)=>{super::log(format!("TOWER_TWIN ash refund failed: {error}"));state.twin_refund=None;},
        }
    }
    // 已冻结的返还不因快速进入下一场而丢失或阻塞下一场追踪；清空已用骨灰仍在战斗与召唤物存活期间暂缓。
    let refund_ready=frozen_ash_refund_ready(ash_owner_alive,game.warp_requested,state.ash_guard.is_some());
    let clear_ready=ash_owner_alive&&!game.warp_requested&&state.ash_guard.is_none()
        &&ash_refund_ready(ash_fighting.then_some(false),world.summon_buddy_manager.player_has_alive_summon,&mut state.ash_guard);
    if (refund_ready||clear_ready) && state.ashes_checked.is_none_or(|t|t.elapsed()>=Duration::from_millis(500)) {
        state.ashes_checked=Some(Instant::now());
        if state.ash_guard.is_some() {
            match super::spirits::refund::pump(player_data,&mut state.ash_refunds) {
                Ok(true)=>{super::log(format!("ASH_GROUP all refunds confirmed: {:?}",state.ash_refunds));state.ash_refunds.clear();state.ash_guard=None;state.ash_revival_refund=false;},
                Ok(false)=>{},
                Err(error)=>super::log_repeating(format!("Spirit ash refund failed: {error}")),
            }
        } else {
            state.ash_guard=None;
            if let Err(error)=super::loadout::clear_spent_ashes(player_data){super::log_repeating(error);}
        }
    }
    // 每帧按当前角色刷新 legacy owner 与光环/称号：hub 可能已由暂停/自动档/重连/暂退返回路径提前建好，
    // 不能嵌在 hub.is_none() 里，否则旅程档案四个页签会永久提示“载入角色”且光环/称号不会加载。
    if super::legacy::owner()!=Some(identity) {
        super::legacy::set_owner(identity);
        super::name_tags::load(identity);
        super::auras::load(identity);
    }
    if state.hub.is_none() {
        if state.hub_failed.is_some_and(|(owner,at)|owner==identity&&at.elapsed()<Duration::from_secs(5)){return;}
        match super::hub::Hub::new(identity,player_data.archetype) {
            Ok(hub)=>{
                if let Ok(mut path)=SESSION_PATH.lock(){*path=Some(super::hub::path(identity,"json"));}
                state.hub=Some(hub);state.hub_failed=None;
            },
            Err(error)=>{state.hub_failed=Some((identity,Instant::now()));report(&mut state,format!("旅程存档加载失败，未重置角色：{error}"));return;},
        }
    }
    let dead=player.chr_ins.modules.data.hp<=0||player.chr_ins.chr_flags1c5.death_flag();
    if dead||game.warp_requested {state.combat_scene=None;}
    if dead&&state.party.is_none()&&state.hub.as_ref().is_some_and(|h|!h.active) {
        let run=state.reward_wait.as_ref().map(|(_,run)|run)
            .or_else(||state.pending.as_ref().filter(|p|!p.completed).map(|p|&p.next_run)).or(state.view.run.as_ref());
        let retry=run.and_then(|r|retry_run(r,state.travel.as_ref().map(|(t,..)|t.node),true,state.death_retry));
        if let Some(next)=retry {begin_death_retry(&mut state,world,next);return;}
        if state.death_retry||state.view.run.as_ref().is_some_and(|r|r.retry_node.is_some()) {
            state.command=None;state.view.ready=false;state.view.busy=true;state.view.travelling=true;return;
        }
    }
    // 没有可用赐福时，仍按原有死亡结算流程处理。
    let fighting=state.pending.as_ref().is_some_and(|p|!p.completed)||state.reward_wait.is_some()
        ||state.travel.as_ref().is_some_and(|(travel,_,_,_)|travel.is_preparing());
    if state.party.is_none() && !fighting && state.hub.as_ref().is_some_and(|h|!h.active) && dead {
        if let Some(run)=&mut state.view.run {if !matches!(run.phase,Phase::Won|Phase::Dead){
            run.end_on_death();
            record(run);
        }}
    }
    if state.view.run.as_ref().is_some_and(|r|matches!(r.phase,Phase::Dead|Phase::Won))
        && state.hub.as_ref().is_some_and(|h|!h.active) {
        if let Some(run)=&state.view.run {
            let history=state.hub.as_ref().map(|hub|super::run_log::read(&super::run_log::path(hub.owner))).unwrap_or_default();
            super::run_screen::summary_with(super::run_log::finish(run,history,state.hub.as_ref().map(|hub|hub.owner)));
            if let Some(hub)=&state.hub {
                if let Ok(json)=run.encode(){let _=std::fs::write(super::hub::path(hub.owner,"summary.json"),json);}
                if let Err(error)=super::law::record(hub.owner,run){super::log(error);}
            }
        }
        if let Some(room)=state.room.take(){room.retire(world);}
        // 这场战斗的敌人先留着（玩家死亡时像原版一样留在场上），复活或 15 秒后由下面撤下
        if let Some(pending)=state.pending.take() {state.lingering=Some((pending,Instant::now()));}
        let _=super::native_talk::set_flag(45020970,false);
        let _=super::native_talk::set_flag(45020971,false);
        if let Some(player)=world.main_player.as_mut(){super::blessings::Blessings::clear_run(&mut player.chr_ins);}
        state.hub.as_mut().unwrap().finish();
        if let Some(scene)=state.party_scene.take(){scene.retire(world);}
        state.view.shared_event=false;
        state.pending=None;state.reward_wait=None;state.travel=None;state.flasks=None;state.event_start=None;state.survival_engaged=None;state.descent=None;state.stormveil=None;state.chariot_clock=None;
        super::custom_event_ui::close();super::custom_event_ui::set_survival_hud(None);
        let _=clear_chariot();
        let _=super::native_talk::set_flag(10007990,false);
        let _=super::native_talk::set_flag(30097990,false);
        let _=super::native_talk::set_flag(35007990,false);
        let _=super::native_talk::set_flag(45007990,false);
        state.identity=None;state.view.run=None;state.view.travelling=false;
    }
    if let Some((_,since))=&state.lingering {
        let alive=world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag());
        if lingering_due(alive,since.elapsed()) {
            let (pending,since)=state.lingering.take().unwrap();retire_group(&pending,world);
            super::log(format!("Ended run enemies retired after {:.1} s (player alive={alive})",since.elapsed().as_secs_f32()));
        }
    }
    if state.hub.as_ref().is_some_and(|h|h.active) {
        state.command=None;state.view.ready=false;
        state.pad_capture.update(super::input::active());
        let mut hub=state.hub.take().unwrap();
        let in_party=!solo_session()||state.party.is_some();
        let host_hub_wait=!hub.paused&&state.party.as_ref().is_some_and(|p|p.host()&&(p.gate.any_peer_resume()||!p.gate.hub_reports_ready()));
        let paused_party=(hub.paused&&in_party&&!state.party.as_ref().is_some_and(|p|p.event.pause.resume_ready))||host_hub_wait;
        let blocked=(paused_party||party_waiting||state.party.as_ref().is_some_and(|p|p.needs_new_run())).then(||
            if paused_party&&!party_waiting {state.party.as_ref().map_or_else(||party_block_reason(&state,world),|p|p.resume_waiting_reason())}
            else{party_block_reason(&state,world)});
        let guard=if !in_party {super::hub::PartyChoiceGuard::Solo}
            else if hub.paused {super::hub::PartyChoiceGuard::ResumeParty{can_restart:super::suspension::RESTART_CONTEXT.load(std::sync::atomic::Ordering::Acquire)!=0}}
            else {super::hub::PartyChoiceGuard::FreshParty};
        let result=hub.poll(world,game,blocked.as_deref(),guard);
        state.view.room=Some(hub.view());state.hub=Some(hub);
        match result {
            Ok(Some(mut run))=>{
                if run.preparing_combat.is_none()&&run.act==1&&run.visited.is_empty()&&run.completed_nodes==0 {
                    if let Some(hub)=&state.hub {super::run_log::clear(&super::run_log::path(hub.owner));}
                    // 新旅程：上一局留下的强化石作废
                    if let Some(player)=world.main_player.as_mut() {super::smith::clear(unsafe {player.player_game_data.as_mut()});}
                }
                // 房主用固定种子开局：先把队伍种子换成它（广播给队友），下面再照常对齐
                let fixed=state.hub.as_mut().and_then(|h|h.take_fixed_seed());
                if let (Some(seed),Some(link))=(fixed,state.party.as_mut()) && link.host() && let Err(error)=link.reseed(seed) {
                    if let Some(hub)=state.hub.as_mut(){hub.reopen_for_party(world);}
                    super::log(format!("Party reseed refused: {error}; hub reopened"));
                    report(&mut state,format!("{error}。请等队友都回到大赐福后再用种子开局。"));return;
                }
                if let Some(link)=&state.party {
                    let aligned=erdtree_tower::party_game::act_seed(link.config.seed,run.act).and_then(|seed|run.align_party_start(seed));
                    if let Err(error)=aligned {
                        if let Some(hub)=state.hub.as_mut(){hub.reopen_for_party(world);}
                        super::log(format!("Resumed run rejected by party alignment: {error}; hub reopened"));
                        report(&mut state,format!("{error}。保留的旅程与队伍不一致，请在大赐福选择“开始新旅程”。"));return;
                    }
                }
                state.resume_combat=run.preparing_combat;
                state.view.room=None;state.view.run=Some(run);state.identity=None;state.flasks=None;
                state.suspension=Default::default();super::suspension::notice("");
                state.view.busy=false;state.view.prompt=state.view.prompt.wrapping_add(1);
                report(&mut state,"旅程已开启，请选择路线。");
            },
            Ok(None)=>{},
            Err(error)=>{
                if let Some(room)=&mut state.view.room {room.message=error.clone();room.loading=true;}
                report(&mut state,error);
            },
        }
        return;
    }
    if party_waiting {
        state.view.ready=false;state.command=None;
        report(&mut state,"等待队友确认爬塔会话");
        return;
    }
    if !world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.data.hp<=0||p.chr_ins.chr_flags1c5.death_flag()) {state.death_retry=false;}
    if return_to_retry_room(&mut state,world,game){return;}
    if state.identity.is_none() && state.view.run.as_ref().is_some_and(|run|!run.blessings.is_empty()&&run.delivered_talismans.is_empty()) {
        if let Some(player)=world.main_player.as_mut(){super::blessings::Blessings::clear_run(&mut player.chr_ins);}
    }
    let player=world.main_player.as_ref().unwrap();
    let player_data=unsafe {&mut *player.player_game_data.as_ptr()};
    let data = &player.chr_ins.modules.data;
    let hp = data.hp;
    let max_hp = data.max_hp;
    let class = super::class_from_archetype(player_data.archetype);
    if !game.warp_requested&&(state.view.run.is_some()||state.hub.as_ref().is_some_and(|h|h.active)) {
        let counts=state.view.run.as_ref().filter(|r|!matches!(r.phase,Phase::Dead|Phase::Won)).map_or([0;3],|r|r.bells.counts);
        let _=super::merchant::sync(player_data,counts);
    }
    let local_world = game.event_world_type == EventWorldType::Local || state.party.is_some() || super::seamless::active();
    let probe = (
        player_data.character_id,
        game.save_slot,
        data.chara_init_param_id,
        player_data.archetype,
        local_world,
    );
    if state.last_player_probe != Some(probe) {
        state.last_player_probe = Some(probe);
        super::log(format!(
            "Runtime player: character={} slot={} init_param={} archetype={} local_world={} hp={hp}/{max_hp}",
            probe.0, probe.1, probe.2, probe.3, probe.4
        ));
    }
    let block = i32::from(player.current_block_id);
    let player_dead = hp <= 0 || player.chr_ins.chr_flags1c5.death_flag();
    let was_ready = state.view.ready;
    state.view.ready =
        class.is_some() && local_world && !player_dead && max_hp > 0 && max_hp <= 1_000_000 && !state.view.shared_event;
    if !local_world {
        state.command = None;
        state.pad_capture.update(false);
        return;
    }
    if state.identity.is_none() && !state.view.ready {
        let message = if class.is_none() {
            format!(
                "当前职业编号 {} 无效，请等待角色加载完成。",
                player_data.archetype
            )
        } else if !local_world {
            "等待角色进入本地世界；主菜单和创建角色界面不能开始战斗。".into()
        } else {
            "等待角色生命数据就绪，进入能自由移动的角色后再开始。".into()
        };
        state.command = None;
        state.view.busy = false;
        report(&mut state, message);
        return;
    }
    if state.identity.is_some_and(|owner| owner != identity) {
        state.view.ready = false;
        state.command = None;
        state.view.busy = false;
        report(
            &mut state,
            "已切换存档角色，当前验证不会继续写入；请重启启动器。",
        );
        return;
    }
    if state.identity.is_none() && state.view.ready {
        let (bonus,cut)=state.view.run.as_ref().map_or((0,[0,0]),|r|(r.flask_bonus,r.total_flask_cut()));
        match super::flasks::Flasks::bind_bonus(player_data, class.unwrap(),bonus,cut) {
            Ok(mut flasks) => {
                if let Some(run)=&state.view.run {if let Err(error)=flasks.restore(player_data,run.flasks,run.flask_blue){report(&mut state,error);return;}}
                state.flasks = Some(flasks);
                if let Some(run)=&state.view.run {player_data.unlocked_magic_slots=2+run.memory_bonus;}
            },
            Err(error) => {
                state.view.ready = false;
                let (red, blue) = super::flasks::Flasks::missing(player_data);
                if state.flask_grant_requested.is_none() && (red || blue) {
                    match super::items::give_flasks(red, blue) {
                        Ok(()) => {
                            state.flask_grant_requested = Some(Instant::now());
                            report(&mut state, "正在发放开局圣杯瓶，请稍候。");
                        }
                        Err(error) => report(&mut state, error),
                    }
                    return;
                }
                if state
                    .flask_grant_requested
                    .is_some_and(|started| started.elapsed() < Duration::from_secs(5))
                {
                    return;
                }
                report(&mut state, error);
                return;
            }
        }
        state.identity = Some(identity);
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(42, |t| t.as_nanos() as u64);
        if state.view.run.is_none(){
            let entropy=match super::party::random_entropy(){Ok(value)=>value,Err(error)=>{report(&mut state,error);return;}};
            state.view.run=class.map(|class|Run::new_personal(seed,class,entropy));
            if let Some(run)=state.view.run.as_mut(){run.weapon_preferences=super::legacy::load(identity).training.preferences();}
        }
        if let Some(run)=&state.view.run{record(run);}
        if let Some(run)=state.view.run.clone() {
            if let Phase::Node{id}=run.phase {
                let kind=run.nodes[id].kind;
                if run.regional_event.stormveil==Some(erdtree_tower::stormveil::Progress::Running)||run.regional_event.chariot&&run.regional_event.stormveil==Some(erdtree_tower::stormveil::Progress::Complete) {
                    if let Err(error)=start_stormveil(&mut state,world,game,run.clone()){report(&mut state,error);return;}
                } else if run.regional_event.descent&&(run.regional_event.descent_started||run.regional_event.opened!=0||run.regional_event.receipt.is_some()) {
                    if let Err(error)=start_descent(&mut state,world,game,run.clone()){report(&mut state,error);return;}
                } else if super::rooms::destination(kind).is_some() {
                    if kind==erdtree_tower::NodeKind::Shop {
                        match super::native_shop::prepare(&run,id,false,player_data) {
                            Ok(stock)=>if run.shop_stock.as_ref()!=Some(&stock) {if let Some(run)=state.view.run.as_mut(){run.shop_stock=Some(stock);record(run);}},
                            Err(error)=>{report(&mut state,error);return;},
                        }
                    }
                    if kind==erdtree_tower::NodeKind::Chest {if let Err(error)=super::loot::prepare(&run,id,player_data){report(&mut state,error);return;}}
                    match super::rooms::Room::for_run(&run,id,kind,hp,player.chr_ins.modules.data.fp) {
                        Ok(room)=>{game.move_map_target=BlockId::from(room.block);game.initial_area_entity_id=room.entry;game.warp_requested=true;state.view.room=Some(room.view.clone());state.room=Some(room);},
                        Err(error)=>report(&mut state,error),
                    }
                }
            }
        }
        super::log(format!("Tower seed: {seed}"));
        report(&mut state, "角色已绑定，五瓶已就绪。点击金色节点开始旅程。");
    }
    let player = world.main_player.as_ref().unwrap();
    let player_data = unsafe { &mut *player.player_game_data.as_ptr() };

    let larval_due=state.larval_retry.is_none_or(|t|t.elapsed()>=Duration::from_secs(2));
    let mut larval_sent=state.larval_sent;
    let larval=state.view.run.as_mut().filter(|run|larval_due&&run.larval_gift.pending(run.act)&&!matches!(run.phase,Phase::Dead|Phase::Won)).map(|run|{
        let before=run.larval_gift.clone();
        let target=run.larval_gift.begin(super::rebirth::count(player_data));
        if run.larval_gift!=before {record_checked(run)?;}
        let key=(run.initial_seed.unwrap_or(run.seed),run.act);
        if super::rebirth::count(player_data)<target&&larval_sent!=Some(key) {
            super::items::give(&[(0x40000000|super::rebirth::TEAR,1,-1)])?;larval_sent=Some(key);
            super::log(format!("Act {} larval tear requested: held={} target={target}",run.act,super::rebirth::count(player_data)));
        }
        if !run.larval_gift.complete(run.act,super::rebirth::count(player_data)) {return Err("换幕泪滴幼体尚未到账：已发放过一次，若背包已满请清理后重启游戏，会再补发".to_string());}
        record(run);super::log(format!("Act {} larval tear delivered",run.act));Ok(())
    });
    state.larval_sent=larval_sent;
    match larval {Some(Err(error))=>{state.larval_retry=Some(Instant::now());report(&mut state,error);},Some(Ok(()))=>state.larval_retry=None,None=>{}}
    let loading=flask_loading(state.travel.as_ref().map(|(travel,..)|travel),game.warp_requested,
        state.room.as_ref().is_some_and(|room|room.view.loading));
    if let Some(flasks) = &mut state.flasks {
        if let Err(error) = flasks.observe(player_data, loading) {
            state.view.ready = false;
            state.command = None;
            report(&mut state, error);
            return;
        }
        let (count,blue)=(flasks.total(),flasks.remaining[1]);
        if let Some(run) = &mut state.view.run {
            run.flasks = count;run.flask_blue = Some(blue);
            run.physick_empty=super::physick::empty(player_data);
            if !matches!(run.phase,Phase::Reward{..}) {
                // 磨刀石等是重要物品：无缝联机下 items() 读不到，改按背包的全部列表找
                for item in super::loot::bag_items(player_data).into_iter().filter(|i|i.category()==eldenring::cs::ItemCategory::Goods&&erdtree_tower::rewards::progression_good(0x40000000|i.param_id())) {
                    let id=0x40000000|item.param_id();
                    if !run.claimed_items.contains(&id){run.claimed_items.push(id);}
                }
            }
        }
        if let Some(pending) = &mut state.pending {
            pending.next_run.flasks = count;pending.next_run.flask_blue = Some(blue);
        }
    }

    if !loading&&!player_dead {
        let won=state.reward_wait.as_ref().and_then(|(_,run)|match run.phase {Phase::Combat{id}=>Some(id),_=>None})
            .or_else(||state.view.run.as_ref().and_then(|run|match run.phase {Phase::Reward{id,..}=>Some(id),_=>None}));
        if let Some(run)=state.view.run.as_mut() {
            if let Some(id)=won.filter(|id|run.physick_drop(*id,true)) {run.physick_supply.queue(run.act,id);}
            let before=run.physick_supply.clone();
            let held=super::physick::held(player_data);
            run.physick_supply.confirm_load(held);
            if run.physick_supply.credit(){super::log(format!("PHYSICK reserve credited: {}",run.physick_supply.reserve));}
            let load=run.physick_supply.begin_load(held);
            if run.physick_supply!=before {
                match record_checked(run) {
                    Ok(()) if load=>if let Err(error)=super::physick::deliver(player_data,1){super::log(error);},
                    Ok(())=>{},
                    Err(error)=>{run.physick_supply=before;super::log_repeating(error);},
                }
            }
            let supply=run.physick_supply.clone();
            if let Some(pending)=state.pending.as_mut(){pending.next_run.physick_supply=supply.clone();}
            if let Some((_,next))=state.reward_wait.as_mut(){next.physick_supply=supply;}
        }
    }

    if !was_ready
        && state.view.ready
        && !state.view.busy
        && state
            .view
            .run
            .as_ref()
            .is_some_and(|run| matches!(run.phase, Phase::Map))
    {
        report(&mut state, "角色与圣杯瓶已就绪，点击金色节点开始旅程。");
    }

    if state.party.is_none() && player_dead
        && state
            .travel
            .as_ref()
            .is_some_and(|(travel, _, _, _)| travel.is_preparing())
    {
        let (travel, _, _, _) = state.travel.take().unwrap();
        if let Some(run) = &mut state.view.run {
            if run.enter(travel.node).is_ok() {
                let _ = run.observe_hp(0);
                record(run);
            }
        }
        state.view.travelling = false;
        state.view.busy = false;
        report(&mut state, "准备期间角色死亡，本局结束。");
        return;
    }
    let arena=state.view.run.as_ref().map_or_else(||super::arenas::for_act(1),|r|super::arenas::for_run(r,state.party.is_some()));
    if let Some((travel,started,_,_))=state.travel.as_mut() {
        if !travel.unloaded_after_arrival && block==arena.block && !game.warp_requested {
            travel.saw_arena |= started.elapsed()>Duration::from_secs(1) && player.chr_ins.modules.physics.standing_on_solid_ground;
        }
        else if travel.saw_arena && !player_dead {
            if block<0 || game.warp_requested || !player.chr_ins.modules.physics.standing_on_solid_ground {return;}
            let node=travel.node;*travel=super::travel::Travel::new(node);*started=Instant::now();
            game.move_map_target=BlockId::from(arena.block);game.initial_area_entity_id=arena.combat_entry();game.warp_requested=true;
            super::log(format!("PRECOMBAT_RETURN node={node} from={block:08x} target={:08x}",arena.block));return;
        }
    }
    if state.travel.is_some()&&block==arena.block&&!game.warp_requested&&!player_dead&&arena.below_floor(player.block_position.y) {
        let error="门外地面未就绪，已退回安全点，未消耗节点。".to_string();
        if super::native_talk::set_flag(45007994,true).is_ok() {
            let _=super::native_talk::set_flag(45007990,false);
            let _=super::native_talk::set_flag(45007992,false);
            let _=super::native_talk::set_flag(45007993,false);
            state.travel=None;state.view.travelling=false;state.view.busy=false;
            cancel_precombat(&mut state.view);
            if let Some(link)=state.party.as_mut(){link.stop(error.clone());}
            report(&mut state,error);return;
        }
    }
    if let Some((travel, started, _, _)) = &mut state.travel {
        let arrived =
            block == arena.block && !game.warp_requested && started.elapsed() > Duration::from_secs(1);
        let grounded = !player_dead
            && player.chr_ins.modules.physics.standing_on_solid_ground
            && !player.chr_ins.modules.physics.is_falling
            && !player.chr_ins.chr_flags1c5.is_invincible();
        if game.warp_requested||block!=arena.block {
            travel.reused_scene=false;
            let _=super::native_talk::set_flag(super::travel::RESET_REQUEST,false);
        }
        if travel.reused_scene&&!travel.reuse_ready {
            let at_entry=super::arenas::combat_arrival(arena.block).is_some_and(|p|
                (player.block_position.x-p[0]).hypot(player.block_position.z-p[2])<2.0&&(player.block_position.y-p[1]).abs()<2.0);
            if !at_entry||!grounded||!super::native_talk::flag(super::travel::RESET_CLEARED).unwrap_or(false) {
                if started.elapsed()>Duration::from_secs(10) {
                    let _=super::native_talk::set_flag(super::travel::RESET_REQUEST,false);
                    let node=travel.node;*travel=super::travel::Travel::new(node);*started=Instant::now();
                    game.move_map_target=BlockId::from(arena.block);game.initial_area_entity_id=arena.combat_entry();game.warp_requested=true;
                    super::log("TRAVEL_REUSE reset timed out; falling back to map reload");
                    report(&mut state,"场地重置未完成，正在重新加载。");
                }else{report(&mut state,"正在返回出生点并清理上一场传送门……");}
                return;
            }
            travel.reuse_ready=true;
            let _=super::native_talk::set_flag(super::travel::RESET_REQUEST,false);
            let _=super::native_talk::set_flag(45007993,true);
            super::log(format!("TRAVEL_REUSE reset confirmed block={:08x}; player at entry and portal cleared",arena.block));
        }
        if arrived&&grounded&&!travel.reused_scene {let _=super::native_talk::set_flag(45007992,true);}
        // 龙王平台：本机已落地并站进中心圈，就当作已落位、已进圈（不等地图事件，联机队员不再等房主进圈）
        let inside=arrived&&grounded&&arena.inside_circle([player.block_position.x,player.block_position.y,player.block_position.z]);
        let placed=travel.reused_scene||inside||super::native_talk::flag(45007993).unwrap_or(false);
        let crossed=inside||super::native_talk::flag(45007991).unwrap_or(false);
        let progress=travel.poll_entry(started.elapsed(),arrived,grounded,placed,crossed,arena.uses_circle());
        if arrived&&placed&&!travel.resources_restored {
            let (_,_,saved_hp,saved_fp)=state.travel.as_ref().unwrap();
            if let Some(player)=world.main_player.as_mut() {
                if let Err(error)=super::ailments::clear(&mut player.chr_ins){report(&mut state,error);return;}
                super::travel::restore_arrival_hp(&mut player.chr_ins,*saved_hp,"combat arrival");
                let data=&mut player.chr_ins.modules.data;
                data.fp=data.fp.min(*saved_fp);
            }
            state.travel.as_mut().unwrap().0.resources_restored=true;
        }
        match progress {
            super::travel::Progress::Loading => {}
            super::travel::Progress::Fog => {report(&mut state,if arena.uses_circle(){"靠近场地中心，自动倒计时开战。"}else{"穿过雾门开始战斗。"});}
            super::travel::Progress::Countdown(seconds) => {
                let (travel, _, saved_hp, saved_fp) = state.travel.as_mut().unwrap();
                if !travel.resources_restored {
                    if let Some(player) = world.main_player.as_mut() {
                        if let Err(error)=super::ailments::clear(&mut player.chr_ins){report(&mut state,error);return;}
                        super::travel::restore_arrival_hp(&mut player.chr_ins,*saved_hp,"combat preparation");
                        let data = &mut player.chr_ins.modules.data;
                        data.fp = data.fp.min(*saved_fp);
                    }
                    travel.resources_restored = true;
                }
                report(
                    &mut state,
                    format!("已触发 · {} · 敌人将在 {seconds} 秒后出现",arena.name),
                );
            }
            super::travel::Progress::Spawn => {
                if !grounded||!state.view.ready {
                    report(&mut state,"等待角色站稳，随后继续开战。");return;
                }
                if let Some(link)=state.party.as_mut(){link.arrived=true;if !link.spawn {report(&mut state,"已抵达，等待全员准备完成");return;}}
                let (travel, _, _, _) = state.travel.take().unwrap();
                let _=super::native_talk::set_flag(45007992,false);
                state.spawn_used = false;
                state.view.travelling = false;
                if let Err(error) = request_spawn(&mut state, world, travel.node,travel.spirits) {
                    cancel_precombat(&mut state.view);
                    state.view.busy = false;
                    report(&mut state, error);
                }
            }
            super::travel::Progress::Failed => {
                state.travel = None;
                cancel_precombat(&mut state.view);
                state.view.travelling = false;
                state.view.busy = false;
                report(&mut state, "传送超时，未生成敌人，路线没有推进。");
            }
        }
        return;
    }

    if state.party.is_some()&&player_dead&&state.reward_wait.is_some(){return;}
    if let Some((started, mut next)) = state.reward_wait.take() {
        use super::reward_delay::ResultState;
        // 联机时摸传送门只登记“已进入”，等全员（倒下的除外）都进入后由房主发奖励
        let touched=state.reward_portal.armed()&&(state.skip_portal||super::native_talk::flag(45020971).unwrap_or(false));
        // 跳过本关（房主）：不等队友摸传送门，直接通知全队领奖
        if state.skip_portal&&state.reward_portal.armed()&&let Some(link)=state.party.as_mut()&&link.host()&&link.victory&&!link.rewards {
            if let Err(error)=link.force_rewards(){report(&mut state,error);}else{super::log("SKIP party rewards opened by host");}
        }
        let entered=state.party.as_mut().is_some_and(|p|touched&&!p.rewards&&!std::mem::replace(&mut p.portal,true));
        let portal_used=state.party.as_ref().map_or(touched,|p|p.rewards);
        // 最后一幕的 Boss：普通模式没有三选一，传送门直接结束旅程；无尽模式第 3 幕起继续领奖进入下一幕
        let last=matches!(next.phase,Phase::Combat{id} if (next.act>=3&&!next.endless||next.act>=erdtree_tower::MAX_ENDLESS_ACT)&&next.nodes.get(id).is_some_and(|n|n.kind==erdtree_tower::NodeKind::Boss));
        if entered {report(&mut state,if last{"已进入传送门，等待全部队友进入后一起结束旅程。"}else{"已进入传送门，等待全部队友进入后一起领取奖励。"});}
        match state.reward_portal.poll(started.elapsed(), player_dead, portal_used) {
            ResultState::Waiting => {
                state.reward_wait = Some((started, next));
                return;
            }
            ResultState::PlayerDied => {
                state.skip_portal=false;
                let _ = super::native_talk::set_flag(45020970, false);
                let _ = super::native_talk::set_flag(45020971, false);
                let _ = next.observe_hp(0);
            }
            ResultState::PortalReady => {
                let portal = super::native_talk::set_flag(45020971,false)
                    .and_then(|_|super::native_talk::set_flag(45020970,true))
                    .and_then(|_|super::native_talk::set_flag(45020972,true));
                state.reward_wait = Some((started, next));
                report(&mut state, portal.err().unwrap_or_else(|| if last {"传送门已激活，交互后结束旅程。"}else{"传送门已激活，交互后领取奖励。"}.into()));
                return;
            }
            ResultState::Ready => {
                settle_portal_ashes(&mut state,world);
                state.skip_portal=false;
                if let Some(link)=state.party.as_mut(){if let Err(error)=link.open_rewards(){link.stop(error.clone());report(&mut state,error);return;}}
                let _ = super::native_talk::set_flag(45020970, false);
                let _ = super::native_talk::set_flag(45020971, false);
                if let Some(flasks)=state.flasks.as_ref() {flasks.stamp(&mut next);}
                next.casting_tools=Some(super::loot::casting_tools(player_data));
                // 本局历史最高：只升不降（原来直接换成当前背包最高，卖掉或存仓后会掉下来）
                if let Ok(params)=unsafe {SoloParamRepository::instance()} {
                    let live=super::smith::inventory_peak(player_data,params).clamp(0,i32::from(erdtree_tower::contracts::MAX_PEAK)) as u8;
                    next.weapon_peak=Some(next.weapon_peak.unwrap_or(0).max(live));
                }
                next.spirit_peak=Some(spirit_peak(&next,super::spirit_tune::inventory_peak(player_data)));
                let result = next
                    .observe_hp(normalized_hp(hp, max_hp))
                    .and_then(|_| next.win_combat());
                if let Err(error) = result {
                    state.view.busy = false;
                    report(&mut state, error);
                    return;
                }
            }
        }
        record(&next);
        state.view.run = Some(next);
        state.view.busy = false;
        report(
            &mut state,
            if player_dead {
                "角色死亡，本局结束。"
            } else {
                "战斗胜利，选择一项奖励后继续。"
            },
        );
        return;
    }

    // 徘徊赐福实物道具：背包里的数量对齐旅程剩余次数（单人进行中才有，结束或联机时收回）
    if !loading && !player_dead {
        let runtime=&mut *state;
        runtime.retry_item.tick(player_data,runtime.view.run.as_ref(),runtime.party.is_none());
    }
    if !loading
        && !player_dead
        && state
            .last_blessing_sync
            .is_none_or(|at| at.elapsed() >= Duration::from_millis(500))
    {
        let mut desired = state
            .view
            .run
            .as_ref()
            .filter(|r| r.phase != Phase::Dead)
            .cloned();
        if let (Some(player),Some(run)) = (world.main_player.as_mut(),desired.as_mut()) {
            if let Err(error) = super::blessings::Blessings::deliver(unsafe {player.player_game_data.as_mut()},run) {
                state.view.ready = false;
                report(&mut state, error);
                return;
            }
        }
        if let Some(run)=desired {state.view.run=Some(run);}
        state.last_blessing_sync = Some(Instant::now());
    }

    if state.last_save.is_none_or(|t|t.elapsed()>Duration::from_secs(2)) {
        if let Some(run)=&state.view.run{
            if state.last_saved.as_ref()!=Some(run){record(run);state.last_saved=Some(run.clone());}
            else if let Some(player)=world.main_player.as_ref() {record_fp(player.chr_ins.modules.data.fp);}
        }
        state.last_save=Some(Instant::now());
    }
    if state.stormveil.is_some() {
        if let Err(error)=tick_stormveil(&mut state,world,game){report(&mut state,error);}
        return;
    }
    if state.descent.is_some() {
        if let Err(error)=tick_descent(&mut state,world,game){report(&mut state,error);}
        return;
    }
    if state.room.is_some() {
        tick_room(&mut state, world, game, player_dead);
        return;
    }

    if let Some(node)=state.resume_combat {
        if state.view.run.as_ref().and_then(|r|r.preparing_combat)!=Some(node)||state.travel.is_some()||state.pending.is_some() {
            state.resume_combat=None;
        }else if state.view.ready && !game.warp_requested && world.main_player.as_ref().is_some_and(|p|
            i32::from(p.current_block_id)>=0 && p.chr_ins.modules.physics.standing_on_solid_ground && !p.chr_ins.chr_flags1c5.is_invincible()) {
            state.command=Some(Action::Enter(node));state.resume_combat=None;
            super::log(format!("PRECOMBAT_RESUME node={node}"));
        }else{return;}
    }
    if let Some(command)=super::debug_menu::take() {
        let message=match debug_command(&mut state,world,command) {Ok(message)=>message,Err(error)=>format!("调试失败：{error}")};
        super::log(format!("DEBUG_MENU {message}"));report(&mut state,message);
    }
    if let Some(action) = state.command.take() {
        let journaled=state.party.is_some()&&matches!(action,Action::KeepCurrency|Action::Story(_)|Action::Claim(_)|Action::Camp(_));
        let story_message=if let Action::Story(choice)=action {
            state.view.run.as_ref().filter(|r|r.story_index().is_ok()).map(|r|erdtree_tower::events::story_outcome(r,choice))
        }else{None};
        let result = match action {
            Action::Enter(id) => {
                if state.party.is_some() {
                    let run=state.view.run.clone().ok_or("旅程未加载".to_string());
                    run.and_then(|run|state.party.as_mut().unwrap().enter(&run,id))
                }else {request_travel(&mut state, world, game, id)}
            },
            Action::KeepCurrency => transact(&mut state, world, action, journaled, false),
            Action::Contract(_) | Action::Gear(_) | Action::SkipGear | Action::SkipContract => contract_action(&mut state,world,action),
            Action::Story(_) | Action::Claim(_) | Action::Camp(_) => {
                transact(&mut state, world, action, journaled, false)
            }
        };
        if result.is_ok()&&state.material_award.is_some(){state.view.busy=true;return;}
        let result=result.and_then(|_|if journaled&&award_path().is_ok_and(|p|p.exists()) {finish_party_award()}else{Ok(())});
        if let Err(error) = result {
            if journaled&&award_path().is_ok_and(|p|p.exists()) {state.party.as_mut().unwrap().stop(format!("奖励执行中断，已停止重发：{error}"));}
            state.view.busy = false;
            report(&mut state, error);
        } else if matches!(action,Action::Enter(_))&&state.party.is_some() {
            state.view.busy=false;
            report(&mut state,"等待队友选择路线");
        } else if !matches!(action, Action::Enter(_)) {
            state.view.busy = false;
            let message=state.view.run.as_ref().filter(|r|r.act>1&&r.visited.is_empty()&&r.phase==Phase::Map)
                .map(|r|format!("第 {} 幕已开启，生命、专注与圣杯瓶已回满。下一战：{}。",r.act,super::arenas::for_run(r,state.party.is_some()).name))
                .unwrap_or_else(||story_message.unwrap_or_else(||"行动完成，请选择下一层节点。".into()));
            report(&mut state, message);
        }
    }

    let Some(_) = state.pending.as_mut().filter(|pending| !pending.completed) else {
        // 联机成员的律法与 Boss 词条由 member_combat 维护，打完（收到胜利）才清掉
        let member=member_live(&state);
        if state.view.boss_affixes.is_some()&&!member{state.boss_fight=Default::default();state.view.boss_affixes=None;state.view.boss_alert=None;state.view.boss_affix_list.clear();}
        if state.dual_fight.active(){state.dual_fight=Default::default();state.view.boss_alert=None;}
        if state.view.law_clock.is_some()&&!member{state.law_fight=Default::default();state.view.law_clock=None;}
        // 联机成员的余烬状态由 member_embers 维护，不在这里清掉
        if state.ember_fight.active()&&!member_fighting(&state){state.ember_fight=Default::default();}
        if state.bout.active(){state.bout.end(world);}
        return;
    };
    let party_mode=state.party.is_some();
    let stall_authority=state.party.as_ref().is_none_or(|p|p.host());
    let skip_party=state.party.as_ref().map(|p|(p.host(),p.fighting));let skip_requested=state.skip_fight;
    let pending=state.pending.as_mut().unwrap();
    let (outcome, enemy_hp, bound) = poll_group(pending, world, player_dead&&!party_mode, block,stall_authority);
    let node=if let Phase::Combat{id}=pending.next_run.phase {Some(id)}else{None};
    let skipped=skip_fight_due(skip_requested,node,bound,player_dead,skip_party);
    // 跳过本关：撤下还活着的敌人，视为胜利（没生成的不再生成）；联机成员收到胜利后照常清掉本地残留的敌人
    let outcome=if skipped {retire_group(pending,world);Outcome::Victory}else{outcome};
    let pending=state.pending.as_mut().unwrap();
    // 联机「精英成双」：还在等前一只的精英没请求过生成，不算未绑定（否则全队等不到开战）
    let all_bound=pending.foes.iter().all(|foe|foe.fight.handle().is_some()||foe.requested.is_none())&&pending.foes.iter().any(|foe|foe.fight.handle().is_some());
    if let Some(link)=state.party.as_mut(){link.bound=all_bound;}
    let pending=state.pending.as_mut().unwrap();
    if bound && !player_dead && matches!(pending.next_run.phase, Phase::Combat { .. }) {
        let _ = pending.next_run.observe_hp(normalized_hp(hp, max_hp));
    }
    if outcome == Outcome::Waiting {
        if (bound||pending.next_run.embers.contains(&erdtree_tower::embers::Ember::SpiritTwin))&&block==pending.block&&!pending.next_run.law.lone_wolf() {if let Err(error)=pending.spirits.poll(world,block,!player_dead) {super::log(format!("Spirit activation failed: {error}"));}}
        let runtime=&mut *state;
        let mut rise=None;
        if let Some(pending)=runtime.pending.as_ref().filter(|_|bound)&&let Phase::Combat{id}=pending.next_run.phase {
            let run=&pending.next_run;
            runtime.boss_fight.begin((run.seed,run.act,id),erdtree_tower::boss_affixes::roll(run,id),run.law.empowered_affixes());
            let handles:Vec<_>=pending.foes.iter().filter_map(|f|f.fight.handle()).collect();
            let authority=runtime.party.as_ref().is_none_or(|p|p.host());
            let flasks=runtime.flasks.as_ref().map(|f|f.total());
            let team=runtime.party.as_ref().map(|p|p.gate.flask_uses());
            let hits=runtime.party.as_ref().filter(|p|p.host()).map(|p|p.gate.leech_hits(p.config.player)).unwrap_or_default();
            runtime.boss_fight.tick(world,&handles,authority,runtime.party.is_some(),flasks,team.as_deref(),&hits);
            let party_players=runtime.party.as_ref().map_or(0,|p|p.gate.members().len());
            runtime.growth.tick(world,(run.seed,run.act,id),erdtree_tower::scaling::growth_step(run,id),&handles,party_players);
            super::scaling::trace_boss_hp(world,run,id,if runtime.party.is_some() {"host"} else {"solo"},&handles);
            runtime.view.boss_affixes=runtime.boss_fight.title();runtime.view.boss_alert=runtime.boss_fight.alert();runtime.view.boss_affix_list=runtime.boss_fight.affixes().to_vec();
            // Boss 房的 Boss 卡在 1 点生命太久：视为转阶段锁血，直接放行
            if run.nodes.get(id).is_some_and(|n|n.kind==erdtree_tower::NodeKind::Boss) {
                let bosses:Vec<_>=pending.foes.iter().filter(|f|f.spec.bar>0).filter_map(|f|Some((f.fight.handle()?,f.spec.name))).collect();
                runtime.boss_guard.tick(world,(run.seed,run.act,id),&bosses,authority);
            }
            // 第三幕双王共命（含无尽模式抽到的第三幕双王）：超时复生（屏幕上不显示提示）
            if pending.foes.len()==2&&pending.foes.iter().all(|f|super::enemies::dual(f.spec.kind,f.spec.act)&&f.spec.bar>0) {
                // 联机用预置实体，死了不能重新生成：只有单人复生
                let revive=!pending.networked;
                let foes=[0,1].map(|i|(pending.foes[i].spec.name,pending.foes[i].fight.handle()));
                rise=runtime.dual_fight.tick(world,(run.seed,run.act,id),foes,authority,revive);
            }
            if runtime.party.as_ref().is_none_or(|p|p.fighting) {
                let team=runtime.party.as_ref().map(|p|p.gate.flask_uses());
                runtime.law_fight.tick(world,(run.seed,run.act,id),run.law,&handles,authority,flasks,team.as_deref());
        super::stance_floor::tick(world,&handles);
        super::stance_trace::observe(world,&handles);
                runtime.view.law_clock=runtime.law_fight.clock();
            }
            if runtime.bout.active() {
                let named:Vec<_>=pending.foes.iter().filter_map(|f|Some((f.fight.handle()?,f.spec.name))).collect();
                if let Some(message)=runtime.bout.tick(world,&named) {runtime.view.status=message;}
            }
        }
        if bound {host_embers(&mut state,world);}
        // 双王共命：把超时的那只重置为“待生成”，下一帧在尸体的位置重新生成
        if let Some((index,at))=rise && let Some(pending)=state.pending.as_mut() {
            let foe=&mut pending.foes[index];
            foe.fight=Encounter::default();foe.requested=None;foe.entity_address=None;foe.respawn=Some(at);foe.spawn_check=Default::default();
        }
        let runtime=&mut *state;
        if publish_bound_combat(&mut runtime.view,&runtime.pending.as_ref().unwrap().next_run,bound) {
            report(&mut state, "战斗开始：击败竞技场内的敌人。");
        }
        state.view.enemy_hp = enemy_hp;
        return;
    }
    if outcome!=Outcome::Victory {state.skip_fight=None;}
    if outcome==Outcome::Victory {
        let host_entities=if party_mode&&!skipped {
            state.pending.as_ref().unwrap().foes.iter().filter_map(|foe|{
                let chr=world.chr_ins_by_handle(&foe.fight.handle()?)?;
                (foe.entity_address==Some(std::ptr::from_ref(chr) as usize)&&chr.event_entity_id!=0
                    &&(chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag())).then_some(chr.event_entity_id)
            }).collect::<Vec<_>>()
        }else{Vec::new()};
        if let Some(link)=state.party.as_mut(){
            if let Err(error)=link.result(){
                if link.encounter.is_some_and(|t|t.elapsed()>Duration::from_secs(90)){link.stop(error.clone());}
                report(&mut state,error);return;
            }
            if link.host()&&!host_entities.is_empty(){
                let epoch=link.epoch;
                let runes=world.main_player.as_ref().map(|p|unsafe {p.player_game_data.as_ref()}.rune_count);
                super::log(format!("Party native death drain started host=true epoch={epoch} entities={host_entities:?} runes={runes:?}"));
                state.victory_cleanup=Some((epoch,host_entities,Instant::now(),false));
            }
        }
    }
    // 最后一只倒下的那一帧就判定了胜利：余烬再结算这一帧，最后的击倒（往往是 Boss）也算进叠层
    if outcome==Outcome::Victory {host_embers(&mut state,world);}
    state.boss_fight=Default::default();state.view.boss_affixes=None;
    state.law_fight=Default::default();state.view.law_clock=None;state.ember_fight=Default::default();state.bout.end(world);
    let pending = state.pending.as_mut().unwrap();
    pending.completed = true;
    let fought=if let Phase::Combat{id}=pending.next_run.phase {Some(id)} else {None};
    let names:Vec<&'static str>=pending.foes.iter().map(|f|f.spec.name).collect();
    let result = match outcome {
        Outcome::Victory => Ok(()),
        Outcome::PlayerDied if pending.next_run.phase == Phase::Dead => Ok(()),
        Outcome::PlayerDied => pending.next_run.observe_hp(0),
        _ => Ok(()),
    };
    let next = pending.next_run.clone();
    if matches!(outcome,Outcome::Victory|Outcome::PlayerDied) {append_history(&next,fought,outcome==Outcome::Victory,&names);}
    // 玩家死亡时不撤敌人：像原版一样留在场上（“你死了”画面里不会突然消失），回大赐福结算时统一撤下
    if matches!(outcome,Outcome::LostEntity|Outcome::SpawnTimedOut){retire_group(pending,world);}
    state.view.enemy_hp = enemy_hp;
    state.view.busy = false;
    state.view.travelling = false;
    match (outcome, result) {
        (Outcome::Victory, Ok(())) => {
            let _=super::native_talk::set_flag(45007990,false);
            if let Phase::Combat{id}=next.phase {
                give_elite_tear(world,&next,id);
                let bonus=erdtree_tower::boss_affixes::bonus_runes(&next,id);
                if let Some(player)=world.main_player.as_mut().filter(|_|bonus>0) {
                    let data=unsafe {player.player_game_data.as_mut()};
                    data.rune_count=data.rune_count.saturating_add(bonus).min(999_999_999);
                    super::log(format!("BOSS_AFFIX bonus runes={bonus}"));
                }
            }
            state.view.run=Some(next.clone());
            // 敌人全灭后先把战斗里沾上的中毒、腐败清掉：之后是奖励与路线界面，玩家没法自己解
            if let Some(player)=world.main_player.as_mut().filter(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()) {
                if let Err(error)=super::ailments::clear(&mut player.chr_ins){super::log(error);}
            }
            let healed=heal_victory(world,&next);
            if let Some(plan)=keep_summoned_ash(world,&next) {state.ash_refunds=plan;state.ash_guard=Some(Instant::now());}
            world.summon_buddy_manager.disappear_requested=true;
            state.reward_wait = Some((Instant::now(), next));
            state.reward_portal=Default::default();
            if state.skip_fight.take().is_some() {state.skip_portal=true;super::log("SKIP fight judged as victory");}
            state.view.busy = true;
            report(&mut state, format!("敌人已击败，传送门即将出现。{healed}"));
        }
        (Outcome::PlayerDied, Ok(())) => {
            record(&next);
            state.view.run = Some(next);
            report(&mut state, "角色死亡，本局结束。重启启动器可再次验证。");
        }
        (Outcome::SpawnTimedOut, _) => {
            if let Some(link)=state.party.as_mut(){link.stop("房主敌人生成超时，未推进路线".into());}
            report(&mut state,"敌人生成超时，未推进路线。请重启游戏后查看日志。");
        },
        (Outcome::LostEntity, _) => {
            if let Some(link)=state.party.as_mut(){link.stop("房主战斗场景或敌人卸载，同步已停止".into());}
            report(&mut state,"场景或敌人已卸载，停止验证且不发奖励。请重启游戏。");
        },
        (_, Err(error)) => report(&mut state, error),
        _ => {}
    }
}

/// 调试菜单命令：改旅程的命令只在路线图界面、没有战斗与传送时执行；任何命令都会把旅程标记为调试旅程
fn debug_command(state:&mut Runtime,world:&mut WorldChrMan,command:super::debug_menu::Command)->Result<String,String> {
    use super::debug_menu::Command;
    if state.party.is_some()||!solo_session() {return Err("联机时不能使用调试功能".into());}
    let busy=state.pending.as_ref().is_some_and(|p|!p.completed)||state.travel.is_some()||state.reward_wait.is_some()||state.view.travelling;
    let player=world.main_player.as_mut().ok_or("角色尚未就绪")?;
    let data=unsafe {player.player_game_data.as_mut()};
    let mut next=state.view.run.clone().ok_or("旅程尚未开启")?;
    next.debug=true;
    if let Some(pending)=state.pending.as_mut() {pending.next_run.debug=true;}
    let message=match command {
        Command::Heal=>{
            let vitals=&mut player.chr_ins.modules.data;vitals.hp=vitals.max_hp;vitals.fp=vitals.max_fp;
            if let Some(flasks)=state.flasks.as_mut() {flasks.fill(data)?;flasks.stamp(&mut next);}
            if matches!(next.phase,Phase::Combat{..}) {next.hp=100;} else {next.hp=next.max_hp;}
            "已回满生命、专注与圣杯瓶".to_owned()
        },
        // 开关本身由 debug_menu::apply_cheats 每帧写入；这里只把旅程标记为调试旅程并提示
        Command::Cheats(god,one_shot)=>format!("无敌{}，秒杀{}",if god{"开"}else{"关"},if one_shot{"开"}else{"关"}),
        Command::InstantBreak(on)=>format!("攻击立刻爆条{}",if on{"开"}else{"关"}),
        Command::Runes(amount)=>{
            data.rune_count=data.rune_count.saturating_add(amount).min(999_999_999);next.gold=data.rune_count;state.view.runes=data.rune_count;
            format!("卢恩 +{amount}")
        },
        Command::Item(id)=>{
            if id==(0x40000000|super::smith::VOUCHER) {
                super::smith::prepare(data)?;
                "已请求发放强化石 ×1".into()
            } else {
                super::items::give_reward(data,id)?;
                format!("已获得 {}",erdtree_tower::rewards::lookup(id).map_or("物品",|i|i.name))
            }
        },
        Command::SpiritItem(id)=>{
            if !super::debug_menu::valid_spirit_test_item(id){return Err("无效的新骨灰测试物品".into());}
            if state.view.travelling {return Err("传送完成后再领取测试骨灰".into());}
            if matches!(next.phase,Phase::Reward{..})||state.reward_wait.is_some(){return Err("请先完成本场领奖，再领取测试骨灰".into());}
            let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"骨灰参数尚未就绪")?;
            if params.get::<eldenring::cs::EquipParamGoods>(id&0x0fffffff).is_none(){return Err("缺少新版骨灰参数，请更新配套资源".into());}
            super::items::give_stack(data,id&0x0fffffff)?;
            let level=erdtree_tower::rewards::spirit_level(id).unwrap();
            next.spirit_peak=Some(next.spirit_peak.unwrap_or(0).max(level));
            if let Some(pending)=state.pending.as_mut(){pending.next_run.spirit_peak=next.spirit_peak;}
            super::log(format!("CUSTOM_SPIRIT_TEST granted id={id} level={level}"));
            format!("已获得 {} ×1",erdtree_tower::rewards::lookup(id).unwrap().name)
        },
        command=>{
            if busy {return Err("请等战斗、传送或领奖结束后再改旅程".into());}
            let flask_cut=next.total_flask_cut();
            let message=match command {
                Command::Jump(id)=>{next.debug_jump(id)?;format!("已跳到第 {} 层的{}",next.nodes[id].layer+1,next.nodes[id].kind.label())},
                Command::Act(act)=>{next.debug_act(act)?;format!("已切换到第 {act} 幕起点")},
                Command::Oath(kind,level)=>{next.debug_oath(kind,level)?;format!("{} 设为 {}",kind.name(),["移除","I","II","III"][usize::from(level)])},
                Command::Ember(ember,held)=>{next.debug_ember(ember,held)?;format!("{}余烬「{}」",if held{"获得"}else{"移除"},ember.name())},
                Command::Law(law)=>{next.debug_law(law)?;format!("苦难律法改为 {} 分（{}）",law.score(),law.code())},
                _=>unreachable!(),
            };
            if let Some(flasks)=state.flasks.as_mut() {
                if next.total_flask_cut()!=flask_cut {flasks.set_bonus(data,next.class,next.flask_bonus,next.total_flask_cut())?;}
                if next.act!=state.view.run.as_ref().map_or(next.act,|r|r.act) {flasks.fill(data)?;let vitals=&mut player.chr_ins.modules.data;vitals.hp=vitals.max_hp;vitals.fp=vitals.max_fp;}
                flasks.restore(data,next.flasks,next.flask_blue)?;flasks.stamp(&mut next);
            }
            state.view.prompt=state.view.prompt.wrapping_add(1);
            message
        },
    };
    record_checked(&next)?;
    state.view.run=Some(next);
    Ok(message)
}

fn tick_radahn_probe(state:&mut Runtime,world:&mut WorldChrMan) {
    let bound=state.pending.as_ref().filter(|p|!p.completed&&!p.networked)
        .and_then(|p|p.foes.iter().find(|f|f.spec.model==4730))
        .and_then(|f|f.fight.handle()).and_then(|h|world.chr_ins_by_handle(&h));
    super::radahn_entity::sync(world,bound,state.party.is_some());
    if state.radahn_probe.is_none(){return;}
    if state.party.is_some()||!state.view.run.as_ref().is_some_and(|run|run.debug&&run.boss_test.is_some())
        ||state.pending.as_ref().is_some_and(|pending|pending.completed)
        ||world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.data.hp<=0) {
        state.radahn_probe=None;return;
    }
    let handle=state.pending.as_ref().filter(|p|!p.networked).and_then(|p|p.foes.iter().find(|foe|foe.spec.model==4730)).and_then(|foe|foe.fight.handle());
    let player=world.main_player.as_ref().map(|p|{let pos=&p.chr_ins.modules.physics.position;[pos.0,pos.1,pos.2]});
    if let Some(chr)=handle.and_then(|h|world.chr_ins_by_handle_mut(&h)) {state.radahn_probe.as_mut().unwrap().tick(chr,player);}
}

fn prepare_boss_test(state:&Runtime,world:&WorldChrMan,game:&GameMan,test:erdtree_tower::debug::BossTest)->Result<(Run,usize),String> {
    if game.warp_requested||state.travel.is_some()||state.reward_wait.is_some()||state.pending.as_ref().is_some_and(|p|!p.completed){return Err("请结束战斗、传送与领奖后再测试".into());}
    let player=world.main_player.as_ref().ok_or("角色未就绪")?;
    if player.chr_ins.modules.data.hp<=0||!player.chr_ins.modules.physics.standing_on_solid_ground||player.chr_ins.chr_flags1c5.is_invincible(){return Err("请站稳并退出原版菜单后再测试".into());}
    let group=super::enemies::boss_test_roster(test.encounter,state.party.is_some(),erdtree_tower::dlc::enemies_allowed())?;
    if group.first().is_none_or(|e|e.act!=test.act){return Err("Boss 测试幕数不匹配".into());}
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"游戏参数未就绪")?;
    for spec in &group {if params.get::<NpcParam>(spec.npc as u32).is_none()||params.get::<NpcThinkParam>(spec.think).is_none(){return Err(format!("缺少 Boss 参数：{}",spec.name));}}
    let mut next=state.view.run.clone().ok_or("请先在大赐福开始旅程")?;
    let node=next.debug_boss_test(test.encounter,test.act)?;
    super::arenas::require_resources(super::arenas::for_run(&next,state.party.is_some()))?;
    Ok((next,node))
}

fn begin_boss_test(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan,test:erdtree_tower::debug::BossTest)->Result<(),String> {
    let (next,node)=prepare_boss_test(state,world,game,test)?;
    state.radahn_probe=None;
    record_checked(&next)?;
    if let Some(room)=state.room.take(){room.retire(world);}
    if let Some(room)=state.attendant.take(){room.retire(world);}
    if let Some(fight)=state.pending.take(){retire_group(&fight,world);}
    if let Some(scene)=state.party_scene.take(){scene.retire(world);}
    state.view.run=Some(next);state.view.room=None;state.view.shared_event=false;
    state.spawn_used=false;state.command=None;state.reward_portal=Default::default();
    super::debug_menu::clear_roster();
    if let Some(player)=world.main_player.as_mut(){
        let vitals=&mut player.chr_ins.modules.data;vitals.hp=vitals.max_hp;vitals.fp=vitals.max_fp;
        if let Some(flasks)=state.flasks.as_mut(){flasks.fill(unsafe {player.player_game_data.as_mut()})?;}
    }
    super::log(format!("BOSS_TEST begin encounter={} act={} party={} specs={:?}",test.encounter,test.act,state.party.is_some(),super::enemies::node_roster(state.view.run.as_ref().unwrap(),node)));
    request_travel(state,world,game,node)?;super::debug_menu::close();Ok(())
}

fn configure_chain_test(run:&mut Run,level:u32)->Result<(),String> {
    if !matches!(level,0|10)||matches!(run.phase,Phase::Reward{..}|Phase::Dead|Phase::Won){return Err("请在准备或战斗阶段测试处刑之链".into());}
    let ember=erdtree_tower::embers::Ember::Chain;
    run.debug=true;run.ember_rewards=true;
    if !run.embers.contains(&ember){run.embers.push(ember);}
    run.ember_stacks.chain=level;run.validate()
}
fn prepare_event_test(state:&Runtime,world:&WorldChrMan,game:&GameMan,draw:erdtree_tower::events::Draw)->Result<(Run,usize),String> {
    if state.party_pending.is_some()||game.event_world_type==EventWorldType::Remote&&state.party.is_none(){return Err("等待队伍同步建立后再测试事件".into());}
    if game.warp_requested||state.view.busy||state.view.travelling||state.travel.is_some()||state.reward_wait.is_some()||state.pending.as_ref().is_some_and(|p|!p.completed){return Err("请等战斗、传送或领奖结束".into());}
    let player=world.main_player.as_ref().ok_or("角色尚未就绪")?;
    if player.chr_ins.modules.data.hp<=0||!player.chr_ins.modules.physics.standing_on_solid_ground||player.chr_ins.chr_flags1c5.is_invincible(){return Err("请站稳并退出原版菜单后再测试".into());}
    let mut next=state.view.run.clone().ok_or("请先开始旅程")?;
    let node=next.debug_event_test(draw)?;
    Ok((next,node))
}
fn begin_event_test(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan,draw:erdtree_tower::events::Draw)->Result<(),String> {
    let (next,node)=prepare_event_test(state,world,game,draw)?;
    record_checked(&next)?;
    if let Some(room)=state.room.take(){room.retire(world);}
    if let Some(room)=state.attendant.take(){room.retire(world);}
    if let Some(fight)=state.pending.take(){retire_group(&fight,world);}
    if let Some(scene)=state.party_scene.take(){scene.retire(world);}
    state.view.run=Some(next);state.view.room=None;state.view.shared_event=false;
    state.spawn_used=false;state.command=None;state.reward_portal=Default::default();state.event_start=None;state.survival_engaged=None;
    super::custom_event_ui::close();super::custom_event_ui::set_survival_hud(None);
    super::debug_menu::clear_roster();
    request_travel(state,world,game,node)?;
    if let Some(run)=state.view.run.as_ref(){record_checked(run)?;}
    super::debug_menu::close();
    super::log(format!("EVENT_TEST entered node={node} draw={draw:?} party={}",state.party.is_some()));
    report(state,"已进入指定事件测试，请与事件 NPC 或场景物件交互。");Ok(())
}
fn debug_boss_command(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan,command:super::debug_menu::Command)->Result<(),String> {
    if !super::debug_menu::enabled(){return Err("请先开启调试模式".into());}
    if matches!(command,super::debug_menu::Command::MausoleumTest|super::debug_menu::Command::EventTest(_)) {
        let draw=if let super::debug_menu::Command::EventTest(draw)=command {draw}else{erdtree_tower::events::Draw::Mausoleum};
        prepare_event_test(state,world,game,draw)?;
        return if let Some(link)=state.party.as_mut(){
            let order=link.event_test(state.view.run.as_ref().ok_or("旅程未开启")?,draw)?;
            let result=apply_party_order(state,world,game,order);
            if let Err(error)=&result {if let Some(link)=state.party.as_mut(){link.stop(error.clone());}}
            result
        }else{begin_event_test(state,world,game,draw)};
    }
    if let super::debug_menu::Command::ChainTest(level)=command {
        if game.warp_requested||state.travel.is_some()||state.reward_wait.is_some(){return Err("请等待传送或结算结束再开启测试".into());}
        if world.main_player.as_ref().is_none_or(|p|p.chr_ins.modules.data.hp<=0){return Err("角色尚未就绪".into());}
        let mut next=state.view.run.clone().ok_or("请先开始旅程")?;
        configure_chain_test(&mut next,level)?;
        let mut pending=state.pending.as_ref().map(|p|p.next_run.clone());
        if let Some(run)=pending.as_mut(){configure_chain_test(run,level)?;}
        record_checked(&next)?;state.view.run=Some(next);
        if let (Some(p),Some(run))=(state.pending.as_mut(),pending){p.next_run=run;}
        super::embers::begin_chain_test();
        super::log(format!("CHAIN_TEST armed manual_level={level}; real critical counter reset"));
        report(state,format!("处刑之链测试：{level} 层；请在战斗中背刺或处决，再查看调试实测页。"));
        return Ok(());
    }
    if state.party.as_ref().is_some_and(|p|!p.host()){return Err("仅房主可发起或结束 Boss 测试".into());}
    if state.party.is_none()&&(state.party_pending.is_some()||game.event_world_type==EventWorldType::Remote){return Err("请等待队伍同步完成".into());}
    match command {
        super::debug_menu::Command::EndBossTest=>{
            if !state.view.run.as_ref().is_some_and(|r|r.debug&&r.boss_test.is_some()){return Err("当前不是 Boss 测试旅程".into());}
            state.radahn_probe=None;
            super::log("BOSS_TEST exit requested");QUICK_END.store(true,std::sync::atomic::Ordering::Relaxed);super::debug_menu::close();Ok(())
        },
        super::debug_menu::Command::RadahnTest=>{
            if state.party.is_some(){return Err("陨石复现仅限单人".into());}
            let encounter=super::radahn_probe::encounter().ok_or("未找到碎星拉塔恩测试条目")?;
            begin_boss_test(state,world,game,erdtree_tower::debug::BossTest{encounter,act:2})?;
            state.radahn_probe=Some(super::radahn_probe::Probe::new());
            super::log("RADAHN_TEST armed: lower to 40% after spawn; player protected; one-shot disabled");
            Ok(())
        },
        super::debug_menu::Command::BossTest(encounter)=>{
            let group=super::enemies::boss_test_roster(encounter,state.party.is_some(),erdtree_tower::dlc::enemies_allowed())?;
            let test=erdtree_tower::debug::BossTest{encounter,act:group[0].act};
            prepare_boss_test(state,world,game,test)?;
            if let Some(link)=state.party.as_mut(){
                let order=link.boss_test(state.view.run.as_ref().ok_or("旅程未开启")?,test)?;
                apply_party_order(state,world,game,order)
            }else{begin_boss_test(state,world,game,test)}
        },
        _=>Err("无效 Boss 测试命令".into()),
    }
}

/// 六个武器装备栏与身上防具中的词条装备。
fn equipped_build(data:&eldenring::cs::PlayerGameData)->Vec<super::reward_ui::Gear> {
    let equipment=&data.equipment.chr_asm;
    let weapons=super::affix_stack::WEAPON_SLOTS;
    let ready = super::affix_stack::instances_ready();
    weapons.into_iter().filter_map(|slot| {
        if let Some(roll) = ready.then(||super::affix_stack::weapon_instance(data,slot)).flatten() {
            return super::reward_ui::weapon_summary(roll);
        }
        let id = u32::try_from(equipment.equipment_param_ids[slot]).ok()?;
        if ready && erdtree_tower::weapon_rolls::slot_count(id / 10000 * 10000).is_some() {
            return None;
        }
        super::reward_ui::affix_summary(id)
    }).chain((12..=15).filter_map(|slot| {
        let id = u32::try_from(equipment.equipment_param_ids[slot]).ok()?;
        super::reward_ui::affix_summary(0x10000000 | id)
    }))
        .chain(super::affix_stack::equipped_talismans(data).filter_map(|(v, sort)| super::reward_ui::talisman_summary(v, data.character_id, sort)))
        .collect()
}

/// 精英战胜利免费送一个圣杯滴露（与奖励三选一无关）；发放失败只记日志，不影响领奖
fn give_elite_tear(world:&mut WorldChrMan,run:&Run,id:usize) {
    if !run.nodes.get(id).is_some_and(|n|n.kind==erdtree_tower::NodeKind::Elite) {return;}
    let Some(player)=world.main_player.as_mut() else{return};
    match super::items::give_stack(unsafe {player.player_game_data.as_mut()},erdtree_tower::elite::TEAR) {
        Ok(())=>super::log(format!("Elite victory sacred tear granted node={id}")),
        Err(error)=>super::log(format!("Elite victory sacred tear failed node={id}: {error}")),
    }
}

/// 本局战斗记录（.history）：结算回顾、职业熟练度、灵光成就按它算
fn append_history(run:&Run,node:Option<usize>,won:bool,names:&[&str]) {
    if let Some(path)=SESSION_PATH.lock().ok().and_then(|p|p.clone()).map(|p|p.with_extension("history")) {
        super::run_log::append(&path,&super::run_log::entry(run,node,won,names));
    }
}
/// 联机：按这个节点的敌人名单记一场战斗
fn record_fight(run:&Run,node:Option<usize>,won:bool) {
    let roster=node.map(|id|super::enemies::node_roster(run,id)).unwrap_or_default();
    let names:Vec<&'static str>=roster.iter().map(|e|e.name).collect();
    append_history(run,node,won,&names);
}
/// 叠层余烬：击倒与致命一击记进本局。界面上的 run、房主与单人战斗结算用的 next_run、等传送门时暂存的 run 都记
/// （它们之后会互相覆盖，各记一次才不会丢），层数效果立即生效
fn record_ember_gains(state:&mut Runtime,gains:Vec<erdtree_tower::embers::Gain>,key:(u64,u8,usize)) {
    if gains.is_empty() {return;}
    let mut raised=Vec::new();
    for event in gains {
        for run in state.view.run.iter_mut().chain(state.pending.iter_mut().map(|p|&mut p.next_run)).chain(state.reward_wait.iter_mut().map(|(_,run)|run)) {
            for ember in run.ember_stacks.gain_in_room(&run.embers,event,key) {
                let level=run.ember_stacks.level(ember);
                if !raised.iter().any(|(e,l)|*e==ember&&*l>=level) {raised.retain(|(e,_)|*e!=ember);raised.push((ember,level));}
            }
        }
    }
    for (ember,level) in raised {
        super::log(format!("TOWER_EMBER stack {} {level}",ember.name()));
        state.view.status=format!("余烬「{}」第 {level} 层：{}",ember.name(),state.view.run.as_ref().and_then(|r|r.ember_stacks.bonus(ember)).unwrap_or_default());state.view.prompt=state.view.prompt.wrapping_add(1);
    }
}
/// 捕灵壶：捕捉到的敌人给一份它的骨灰，等级按当前骨灰等级（与奖励里刷出的骨灰同一档）。
/// 没有对应骨灰、没有 DLC 时的 DLC 骨灰、律法禁用骨灰时只算击杀
fn grant_captures(state:&mut Runtime,world:&mut WorldChrMan,captures:&[(i32,u8)],thinks:&[u32]) {
    let Some(run)=state.view.run.as_ref() else{return};
    for &(model,slot) in captures {
        let think=thinks.get(usize::from(slot)).copied();
        let reward=erdtree_tower::rewards::spirit_for_enemy(model,think);
        let reason=match reward {
            None=>Some("该敌人暂无对应骨灰"),
            Some(id) if !erdtree_tower::dlc::item_allowed(id)=>Some("当前模式未启用对应 DLC 骨灰"),
            Some(_) if run.law.lone_wolf()=>Some("当前律法禁止获得骨灰"),
            _=>None,
        };
        if let Some(reason)=reason {
            super::log(format!("TOWER_EMBER capture model={model}: no spirit ash granted: {reason}"));
            state.view.status=format!("捕捉成功，但未获得骨灰：{reason}");state.view.prompt=state.view.prompt.wrapping_add(1);continue;
        }
        let Some(base)=reward else{continue;};
        let id=base+u32::from(erdtree_tower::rewards::spirit_level_now(run));
        let name=erdtree_tower::rewards::lookup(id).map_or("骨灰",|i|i.name);
        if world.main_player.is_none() {return;}
        match super::items::give(&[(id as i32,1,-1)]) {
            Ok(())=>{super::log(format!("TOWER_EMBER capture model={model} granted={id}"));state.view.status=format!("捕捉成功：获得「{name}」");}
            Err(error)=>{super::log(format!("TOWER_EMBER capture model={model} grant failed: {error}"));state.view.status=format!("捕捉成功，但骨灰发放失败：{error}");}
        }
        state.view.prompt=state.view.prompt.wrapping_add(1);
    }
}
/// 噬神之蛇的饱食：补回 1 次红圣杯瓶
fn grant_feast(flasks:Option<&mut super::flasks::Flasks>,world:&mut WorldChrMan) {
    if let (Some(flasks),Some(player))=(flasks,world.main_player.as_mut()) {
        match flasks.grant(unsafe {player.player_game_data.as_mut()},[1,0]) {
            Ok(granted)=>super::log(format!("TOWER_EMBER feast granted={granted}")),
            Err(error)=>super::log(format!("TOWER_EMBER feast failed: {error}")),
        }
    }
}
/// 房主与单人的余烬结算一帧；联机时房主还执行成员发来的敌人侧效果
fn host_embers(state:&mut Runtime,world:&mut WorldChrMan) {
    let Some(pending)=state.pending.as_ref() else{return};
    let run=&pending.next_run;
    let Phase::Combat{id}=run.phase else{return};
    let authority=state.party.as_ref().is_none_or(|p|p.host());
    let flasks=state.flasks.as_ref().map(|f|f.total());
    let embers=super::embers::held(Some(run));
    // 名单位置：联机时 Pending 的敌人按名单顺序排（单人时不用）
    let foes:Vec<_>=pending.foes.iter().enumerate().filter_map(|(slot,f)|Some((f.fight.handle()?,super::enemies::bar_rank(&f.spec),slot as u8))).collect();
    let keeper=run.ember_stacks.level(erdtree_tower::embers::Ember::SpiritKeeper);
    let key=(run.seed,run.act,id);
    let thinks:Vec<u32>=pending.foes.iter().map(|f|f.spec.think).collect();
    let events=state.ember_fight.tick(world,key,&embers,&foes,authority,state.party.is_some(),flasks,keeper,false);
    if authority {
        super::starsurge::fight(world,&foes,state.party.is_some(),None);
        if let Some(link)=state.party.as_mut().filter(|p|p.fighting&&!p.victory&&!p.defeated) {
            for (peer,b) in link.gate.star_bursts(run.seed,&run.visited) {
                let seen=link.event.star_seen.entry(peer).or_default();
                if !b.take_new(seen) {continue;}
                if let Some((h,_,_))=foes.iter().find(|f|f.2==b.slot)
                    && let Some(chr)=world.chr_ins_by_handle_mut(h)
                    && chr.modules.data.hp>0 && !chr.chr_flags1c5.death_flag() && !chr.chr_flags1c5.is_invincible() {
                    super::starsurge::burst_fx(chr);
                    chr.modules.data.hp=chr.modules.data.hp.saturating_sub(b.amount.min(chr.modules.data.max_hp.saturating_mul(2))).max(0);
                }
            }
        }
    }
    if events.feast {grant_feast(state.flasks.as_mut(),world);}
    grant_captures(state,world,&events.captures,&thinks);
    record_ember_gains(state,events.gains,key);
    // 房主：先执行成员发来的敌人侧余烬效果，再检测本帧异常发作，确保队友的鲜血君王/永冻/百毒淬羽触发发作时当帧即可被破势与寒霜枷锁捕获
    if let Some(link)=state.party.as_ref().filter(|p|p.host()) {
        for (member,held,requests) in link.gate.strikes() {state.ember_fight.apply_remote(world,&foes,member,&held,&requests);}
    }
    // 鲜血骑士的破势：自己持有，或联机时任一在场队员持有，都由房主（单人即自己）执行
    let breaker=erdtree_tower::embers::Ember::Breaker;
    if authority&&(embers.contains(&breaker)||state.party.as_ref().is_some_and(|p|p.gate.held_embers().contains(&breaker))) {state.ember_fight.break_on_bursts(world,&foes);}
    if authority {
        let frost=erdtree_tower::embers::Ember::FrostShackle;
        let enabled=embers.contains(&frost)||state.party.as_ref().is_some_and(|p|p.gate.held_embers().contains(&frost));
        super::frost_shackle::observe(world,key,&foes,enabled);
        if let Some(link)=state.party.as_mut(){link.frost_shackles(super::frost_shackle::snapshot());}
    }
}
/// 联机成员在战斗里：敌人已同步，直到传送门结算前都算（房主在生命归零时就判定胜利，被处决的敌人在成员这边要等动画播完才倒下，
/// 收到胜利后继续结算，这一下击倒才不会漏掉）
fn member_fighting(state:&Runtime)->bool {
    state.party.as_ref().is_some_and(|p|!p.host()&&p.bound)
        &&state.view.run.as_ref().is_some_and(|r|matches!(r.phase,Phase::Combat{..}))
}
/// 联机成员正在打、还没收到胜利：律法限时与 Boss 词条在这段时间结算
fn member_live(state:&Runtime)->bool {member_fighting(state)&&state.party.as_ref().is_some_and(|p|p.fighting&&!p.victory)}
/// 成员判断这一场的某只敌人在本机“已就绪”：开战前（敌人无敌）要已加载、启用且活着；
/// 收到开战之后（房主宽限后可能先开战），已加载出来但已经死了的也算——慢的成员加载出来时它可能已经被打死，不算就永远凑不齐、整队超时
fn replica_seen(loaded:bool,alive:bool,disabled:bool,engaged:bool)->bool {
    loaded&&(alive&&!disabled||engaged&&!alive)
}
/// 联机成员：这一场已经出怪（收到 Spawn）、还在战斗阶段。本机副本一加载出来就要挂上与房主相同的效果，不等全员就绪
fn member_replicating(state:&Runtime)->bool {
    state.party.as_ref().is_some_and(|p|!p.host()&&p.spawn&&!p.defeated)
        &&state.view.run.as_ref().is_some_and(|r|matches!(r.phase,Phase::Combat{..}))
}
/// 联机成员的战斗结算。成员没有本地的战斗记录，敌人按地图里的预置实体找（与 Boss 血条相同的找法，倒下的也留着，用来判断击倒）。
/// - 苦难律法：限时倒计时与超时失血（敌人的强化由房主挂）
/// - Boss 词条：屏幕上的词条与护盾提示、腐败积累、荆棘反弹（改 Boss 状态的由房主做）
/// - 余烬：作用在自己与召唤物身上的直接生效；作用在敌人身上的（出血、冻伤、死诞之焰等）算出后随汇报交给房主执行
fn member_combat(state:&mut Runtime,world:&mut WorldChrMan) {
    if !member_replicating(state)&&!member_fighting(state) {return;}
    let Some(run)=state.view.run.as_ref() else{return};
    let Phase::Combat{id}=run.phase else{return};
    let key=(run.seed,run.act,id);
    let flasks=state.flasks.as_ref().map(|f|f.total());
    let roster=super::enemies::node_roster(run,id);
    let handles:Vec<_>=super::enemies::map_entities(&roster).map(|ids|roster.iter().zip(ids).filter_map(|(spec,entity)|
        world.chr_sets.iter().flatten().flat_map(|set|set.characters())
            .find(|chr|chr.event_entity_id==entity&&chr.npc_id==spec.model&&chr.npc_param_id==spec.npc&&enemy_loaded(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status))
            .map(|chr|chr.field_ins_handle)).collect()).unwrap_or_default();
    // 层数成长与无缝联机 Boss 多人缩放：房主与成员在本机挂同一个效果，最大生命才一致（见 scaling::Growth）
    let step=erdtree_tower::scaling::growth_step(run,id);
    let party_players=state.party.as_ref().map_or(0,|p|p.gate.members().len());
    state.growth.tick(world,key,step,&handles,party_players);
    super::scaling::trace_boss_hp(world,run,id,"guest",&handles);
    // 诊断与兜底：本机副本生命已到 0 却没有死亡（与房主不同步、卡在原地继续攻击），每只敌人记一次；
    // 非 Boss 副本持续 2.5 秒仍停在 0 血无死亡标记时直接本地隐藏，避免僵尸怪持续攻击客机（Boss 留给胜利清理，不打断二阶段）。
    {
        // (战斗, 实体, 第一次看到 0 生命的时间, 阶段: 0=观察 1=已记日志 2=已隐藏)
        static STUCK:std::sync::Mutex<Vec<((u64,u8,usize,u32),Instant,u8)>>=std::sync::Mutex::new(Vec::new());
        for handle in &handles {
            let Some(chr)=world.chr_ins_by_handle_mut(handle) else{continue};
            let entry=(key.0,key.1,key.2,chr.event_entity_id);
            let Ok(mut seen)=STUCK.lock() else{continue};
            if chr.modules.data.hp>0||chr.chr_flags1c5.death_flag()||chr.debug_flags.character_disabled() {
                seen.retain(|(e,_,_)|*e!=entry);
                continue;
            }
            if seen.len()>256 {seen.clear();}
            let index=match seen.iter().position(|(e,_,_)|*e==entry) {Some(i)=>i,None=>{seen.push((entry,Instant::now(),0));seen.len()-1}};
            let elapsed=seen[index].1.elapsed();
            if seen[index].2==0&&elapsed>=Duration::from_millis(1500) {
                seen[index].2=1;
                let grown=chr.special_effect.entries().any(|e|e.param_id==super::scaling::GROWTH_FIRST+i32::from(step));
                super::log(format!("TOWER_PARTY member copy at 0 hp without death: entity={} max_hp={} growth_step={step} grown={grown}",chr.event_entity_id,chr.modules.data.max_hp));
            }
            if seen[index].2<2&&!super::enemies::boss_npc(chr.npc_param_id)&&elapsed>=Duration::from_millis(2500) {
                seen[index].2=2;
                chr.debug_flags.set_character_disabled(true);
                super::log(format!("TOWER_PARTY disabled undead member copy at 0 hp: entity={}",chr.event_entity_id));
            }
        }
    }
    // 开战后（含宽限后先开战、本机还没全部就绪的那段）就结算律法与 Boss 词条：常驻词条效果要尽早挂到本机副本上
    let engaged=state.party.as_ref().is_some_and(|p|p.fighting&&!p.victory);
    if engaged {
        state.law_fight.tick(world,key,run.law,&handles,false,flasks,None);
        super::stance_floor::tick(world,&handles);
        super::stance_trace::observe(world,&handles);
        state.view.law_clock=state.law_fight.clock();
        state.boss_fight.begin(key,erdtree_tower::boss_affixes::roll(run,id),run.law.empowered_affixes());
        state.boss_fight.tick(world,&handles,false,true,flasks,None,&[]);
        state.view.boss_affixes=state.boss_fight.title();state.view.boss_alert=state.boss_fight.alert();state.view.boss_affix_list=state.boss_fight.affixes().to_vec();
    }
    if member_fighting(state) {member_embers(state,world);}
}
/// 联机成员的余烬（见 member_combat）
fn member_embers(state:&mut Runtime,world:&mut WorldChrMan) {
    let Some(run)=state.view.run.as_ref() else{return};
    let Phase::Combat{id}=run.phase else{return};
    let embers=super::embers::held(Some(run));
    let roster=super::enemies::node_roster(run,id);
    let Ok(ids)=super::enemies::map_entities(&roster) else{return};
    let foes:Vec<_>=roster.iter().enumerate().filter_map(|(slot,spec)|{
        let chr=world.chr_sets.iter().flatten().flat_map(|set|set.characters())
            .find(|chr|ids.get(slot)==Some(&chr.event_entity_id)&&chr.npc_id==spec.model&&chr.npc_param_id==spec.npc&&enemy_loaded(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status))?;
        Some((chr.field_ins_handle,super::enemies::bar_rank(spec),slot as u8))
    }).collect();
    let key=(run.seed,run.act,id);
    if let Some(link)=state.party.as_ref(){super::frost_shackle::receive(world,key,&foes,&link.event.frost_shackles);}
    let keeper=run.ember_stacks.level(erdtree_tower::embers::Ember::SpiritKeeper);
    let flasks=state.flasks.as_ref().map(|f|f.total());
    let thinks:Vec<u32>=roster.iter().map(|spec|spec.think).collect();
    let events=state.ember_fight.tick(world,key,&embers,&foes,false,true,flasks,keeper,false);
    if let Some(link)=state.party.as_mut().filter(|p|p.fighting&&!p.victory&&!p.defeated) {
        let requests=&mut link.event.star_requests;
        super::starsurge::fight(world,&foes,true,Some(&mut |h,amount| {
            if let Some((_,_,slot))=foes.iter().find(|f|f.0==h) {
                let sequence=requests.back().map_or(1,|b|b.sequence.saturating_add(1));
                if requests.len()==erdtree_tower::embers::STRIKE_WINDOW {requests.pop_front();}
                requests.push_back(erdtree_tower::party_game::StarBurst{sequence,slot:*slot,amount});
            }
        }));
    }
    if events.feast {grant_feast(state.flasks.as_mut(),world);}
    grant_captures(state,world,&events.captures,&thinks);
    record_ember_gains(state,events.gains,key);
    // 作用在敌人身上的余烬交给房主执行
    if !events.strikes.is_empty() && let Some(link)=state.party.as_mut() {link.push_strikes(&events.strikes);}
}
/// 联机成员：按与房主相同的苦难律法「精英成双」规则，名单里哪些位置还要等前面的精英（按本机看到的敌人状态）
fn member_gaps(delays:&mut Option<MemberSpawnGaps>,key:(u64,u64,u8,usize),count:usize)->&mut [erdtree_tower::encounter::SpawnGap] {
    let (scope,gaps)=delays.get_or_insert_with(||(key,Vec::new()));
    if *scope!=key {*scope=key;gaps.clear();}
    gaps.resize_with(count,Default::default);gaps
}
fn member_held(delays:&mut Option<MemberSpawnGaps>,key:(u64,u64,u8,usize),world:&WorldChrMan,run:&Run,id:usize,roster:&[super::enemies::Enemy],ids:&[u32])->Vec<bool> {
    use erdtree_tower::encounter::{Outcome as Spawn,Pacing};
    let elite=run.nodes.get(id).is_some_and(|n|n.kind==erdtree_tower::NodeKind::Elite);
    let monster=run.nodes.get(id).is_some_and(|n|n.kind==erdtree_tower::NodeKind::Monster);
    let pacing=if elite {run.law.elite_pacing()}else{Pacing::Together};
    let roles:Vec<_>=roster.iter().map(super::enemies::role).collect();
    let gaps=member_gaps(delays,key,roster.len());
    let now=spawn_clock();
    // 每个位置：(已倒下, 生命, 最大生命)；还没放出来的为 None
    // HansFix：0 血时成员这边的副本会被停用（"disabled undead member copy"）。停用了但血量归零的副本要算「已倒下」，
    // 不能当成「还没放出来」——否则精英战第二波一直等前一波倒下，成员这边永远不放第二波（房主那边已经出来了）。
    // 停用后加载状态也可能变掉、甚至被卸载，所以这一场里只要看到过某个位置倒下，就一直记作已倒下（DOWNED）。
    static DOWNED:std::sync::Mutex<Vec<((u64,u64,u8,usize),usize)>>=std::sync::Mutex::new(Vec::new());
    let mut downed=DOWNED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    downed.retain(|(k,_)|*k==key);
    let state:Vec<Option<(bool,i32,i32)>>=roster.iter().zip(ids).enumerate().map(|(slot,(spec,&entity))|{
        let found=world.chr_sets.iter().flatten().flat_map(|set|set.characters())
            .find(|chr|chr.event_entity_id==entity&&chr.npc_id==spec.model&&chr.npc_param_id==spec.npc
                &&(!chr.debug_flags.character_disabled()||chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag())
                &&enemy_loaded(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status))
            .map(|chr|(chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(),chr.modules.data.hp,chr.modules.data.max_hp));
        if found.is_some_and(|f|f.0)&&!downed.iter().any(|(_,s)|*s==slot) {
            downed.push((key,slot));
            super::log(format!("TOWER_PARTY member saw slot {slot} defeated (entity={entity}); later paced enemies may spawn"));
        }
        if downed.iter().any(|(_,s)|*s==slot) {Some((true,0,found.map_or(0,|f|f.2)))} else {found}
    }).collect();
    drop(downed);
    (0..roster.len()).map(|i|{
        let earlier:Vec<_>=(0..i).filter(|&j|super::enemies::paced_combatant(&roster[j])).map(|j|match state[j] {
            Some((true,_,_))=>(Spawn::Victory,None),
            Some((false,hp,max))=>(Spawn::Waiting,Some((hp,max))),
            None=>(Spawn::Waiting,None),
        }).collect();
        let previous:Vec<_>=state[..i].iter().map(|s|if matches!(s,Some((true,_,_))){Spawn::Victory}else{Spawn::Waiting}).collect();
        let held=super::enemies::paced_combatant(&roster[i])&&pacing.waits(&earlier);
        let eligible=erdtree_tower::encounter::may_spawn_paced(true,monster,held,&roles,i,&previous);
        let allowed=gaps[i].allows(now,eligible);
        pacing_trace("member",key,i,ids[i],&roster[i],world,format!("state={:?} paced_held={held} eligible={eligible} allowed={allowed} role={:?} pacing={pacing:?} monster={monster}",state[i],roles[i]));
        !allowed
    }).collect()
}
/// HansFix 诊断：分波出场的每一步。每个位置在判定结果变化时记一行，另外每 5 秒记一次快照；最多 800 行。
/// raw 列出场上所有这个实体编号的角色（不加任何过滤）：模型/NPC/生命/死亡/停用/加载状态；flags 是激活请求/就绪旗标。
pub(crate) fn pacing_trace(side:&str,key:(u64,u64,u8,usize),slot:usize,entity:u32,spec:&super::enemies::Enemy,world:&WorldChrMan,decision:String) {
    use std::sync::atomic::{AtomicU32,Ordering};
    static LINES:AtomicU32=AtomicU32::new(0);
    static LAST:std::sync::Mutex<Vec<((u64,u64,u8,usize),usize,String,Instant)>>=std::sync::Mutex::new(Vec::new());
    if LINES.load(Ordering::Relaxed)>=800 {return;}
    let Ok(mut last)=LAST.try_lock() else{return};
    last.retain(|(k,_,_,_)|*k==key);
    let now=Instant::now();
    let previous=last.iter().position(|(_,s,_,_)|*s==slot);
    let changed=previous.is_none_or(|i|last[i].2!=decision);
    let stale=previous.is_some_and(|i|now.duration_since(last[i].3)>=Duration::from_secs(5));
    if !changed&&!stale {return;}
    match previous {Some(i)=>last[i]=(key,slot,decision.clone(),now),None=>last.push((key,slot,decision.clone(),now))}
    drop(last);
    let raw:Vec<String>=world.chr_sets.iter().flatten().flat_map(|set|set.characters()).filter(|chr|chr.event_entity_id==entity).map(|chr|format!(
        "{}/{} hp={}/{} dead={} disabled={} load={:?}",chr.npc_id,chr.npc_param_id,chr.modules.data.hp,chr.modules.data.max_hp,chr.chr_flags1c5.death_flag(),
        chr.debug_flags.character_disabled(),unsafe {chr.chr_set_entry.as_ref()}.chr_load_status)).collect();
    let flags=super::enemies::map_controls(entity).and_then(|(request,ready)|{
        use fromsoftware_shared::FromStatic;
        let man=unsafe {eldenring::cs::CSEventFlagMan::instance()}.ok()?;
        Some((man.virtual_memory_flag.get_flag(request),man.virtual_memory_flag.get_flag(ready)))
    });
    if LINES.fetch_add(1,Ordering::Relaxed)<800 {
        super::log(format!("PACING_TRACE {side} node={} epoch={} slot={slot} entity={entity} want={}/{} {} {decision} flags(request,ready)={flags:?} raw={raw:?}{}",
            key.3,key.1,spec.model,spec.npc,spec.name,if changed{""}else{" (snapshot)"}));
    }
}
fn enemy_loaded(status: ChrLoadStatus) -> bool {
    // The pinned 1.17.1 game uses state 4 for the live player and debug-spawned enemies.
    matches!(
        status,
        ChrLoadStatus::Active | ChrLoadStatus::ReadyForActivation
    )
}

fn tick_material_award(state:&mut Runtime,world:&mut WorldChrMan,game:&GameMan)->Result<(),String> {
    let pending=state.material_award.as_ref().ok_or("材料领奖状态丢失")?;
    let Some(player)=world.main_player.as_ref() else{return Ok(());};
    let data=unsafe {player.player_game_data.as_ref()};
    if !pending.matches((data.character_id,game.save_slot),state.view.run.as_ref()) {
        return Err("材料领奖上下文已变化，保留记录且不再发放；请结束旧局后重新组队".into());
    }
    if game.warp_requested{return Ok(());}
    let quantity=super::items::stack_count(data,pending.item);
    match super::material_award::status(pending.before,quantity,pending.started.elapsed()) {
        super::material_award::Status::Waiting=>Ok(()),
        super::material_award::Status::TimedOut=>Err("材料发放后 5 秒仍未确认到账，未重复发放；请检查背包，按结束本局快捷键返回大赐福解除阻塞".into()),
        super::material_award::Status::Confirmed=>{
            if award_path()?!=pending.path{return Err("材料领奖记录不属于当前角色，已停止处理".into());}
            let pending=state.material_award.take().unwrap();
            super::log(format!("MATERIAL_AWARD confirmed item={} before={} current={} elapsed_ms={}",pending.item,pending.before,quantity,pending.started.elapsed().as_millis()));
            transact(state,world,pending.action,false,true)?;
            finish_party_award()?;state.view.busy=false;
            report(state,"材料已到账，奖励已保存，请选择下一层节点。");Ok(())
        },
    }
}

fn transact(state: &mut Runtime, world: &mut WorldChrMan, action: Action, journaled: bool, material_ready:bool) -> Result<(), String> {
    let old = state.view.run.clone().ok_or("角色尚未绑定")?;
    let mut next = old.clone();
    let player = world.main_player.as_mut().ok_or("角色尚未就绪")?;
    let vitals = &player.chr_ins.modules.data;
    if vitals.hp <= 0 {
        return Err("角色已死亡".into());
    }
    next.hp = normalized_hp(vitals.hp, vitals.max_hp);
    next.gold=unsafe {player.player_game_data.as_ref()}.rune_count;
    next.physick_empty=super::physick::empty(unsafe {player.player_game_data.as_ref()});
    let mut effect = erdtree_tower::events::StoryEffect::default();
    let mut item_reward=None;
    let mut reward_ash=-1;
    let mut sacred_tear=false;
    let mut upgrade_stone=false;
    let mut camp_refill=None;
    match action {
        Action::Camp(choice) => {
            if next.tent_available() {next.tent_refill(choice)?;} else {next.camp(choice)?;}
            camp_refill = match choice { CampChoice::Refill => Some([2,1]), CampChoice::RefillBlue => Some([1,2]), CampChoice::RefillRedOnly => Some([3,0]), CampChoice::RefillBlueOnly => Some([0,3]), _ => None };
        }
        Action::Claim(index) => {
            if let Phase::Reward{id:node,ref offers}=old.phase {
                if matches!(offers.get(index),Some(erdtree_tower::Reward::StarStream))&&!super::star_stream::ready(){return Err("星流引导暂不可用，请更新后重试".into());}
                if let Some(erdtree_tower::Reward::Item(id))=offers.get(index){item_reward=Some(*id);reward_ash=super::random_ashes::preview(&old,node,index,*id);}
                sacred_tear=matches!(offers.get(index),Some(erdtree_tower::Reward::SacredTear));
                upgrade_stone=matches!(offers.get(index),Some(erdtree_tower::Reward::UpgradeStone));
            }
            next.claim_reward(index)?;
        },
        Action::Story(choice) => {effect = next.resolve_story(choice)?;item_reward=item_reward.or(effect.item);},
        Action::KeepCurrency => next.decline_reward()?,
        _ => return Err("无效节点行动".into()),
    }
    let (mut next_hp, mut next_fp) =
        resource_result(vitals.hp, vitals.max_hp, vitals.fp, vitals.max_fp, &effect)?;
    super::blessings::Blessings::validate(&next.blessings)?;
    if next.elite_buffs!=old.elite_buffs {super::elite::validate()?;}
    if let Some(id)=item_reward {super::items::check_reward(unsafe {player.player_game_data.as_ref()},id)?;}
    let reforge=if effect.reforge {
        let Phase::Node{id:node}=old.phase else {return Err("当前不在事件节点".into());};
        Some(super::weapons::reforge_target(unsafe {player.player_game_data.as_ref()},&old,node,effect.reforge_upgrade)?)
    }else if effect.furnace {
        let Phase::Node{id:node}=old.phase else {return Err("当前不在事件节点".into());};
        Some(super::weapons::furnace_target(unsafe {player.player_game_data.as_ref()},&old,node,effect.furnace_uses)?)
    }else{None};
    if (sacred_tear||upgrade_stone)&&!material_ready {
        let item=if upgrade_stone{super::smith::VOUCHER}else{erdtree_tower::elite::TEAR};
        let data=unsafe {player.player_game_data.as_ref()};
        let path=award_path()?;
        let owner=state.identity.ok_or("角色存档尚未绑定")?;
        let before=super::items::stack_count(data,item);
        write_award_journal(&path,&old)?;
        super::items::give(&[((0x40000000|item) as i32,1,-1)])?;
        state.material_award=Some(MaterialAward{action,old,owner,item,before,started:Instant::now(),path});
        super::log(format!("MATERIAL_AWARD requested item={item} before={before}; waiting for inventory confirmation"));
        report(state,"正在等待材料到账，请稍候……");return Ok(());
    }
    if journaled&&!material_ready {write_award_journal(&award_path()?,&old)?;}
    if next.act > old.act {
        next_hp=vitals.max_hp;
        next_fp=vitals.max_fp;
        state.flasks.as_mut().ok_or("圣杯瓶未绑定")?.fill(unsafe {player.player_game_data.as_mut()})?;
        state.flasks.as_ref().unwrap().stamp(&mut next);
    }
    if next.weapon_level > old.weapon_level {
        super::weapons::upgrade(unsafe { player.player_game_data.as_mut() })?;
    }
    if let Some(target)=reforge {super::weapons::reforge(unsafe { player.player_game_data.as_mut() },target)?;}
    if let Some(blue) = camp_refill {
        state.flasks.as_mut().ok_or("圣杯瓶未绑定")?
            .camp_refill(unsafe {player.player_game_data.as_mut()}, blue)?;
        state.flasks.as_ref().unwrap().stamp(&mut next);
    } else if effect.refill {
        state
            .flasks
            .as_mut()
            .ok_or("圣杯瓶未绑定")?
            .refill(unsafe { player.player_game_data.as_mut() })?;
        state.flasks.as_ref().unwrap().stamp(&mut next);
    }
    if next.blessings != old.blessings {
        super::blessings::Blessings::deliver(unsafe {player.player_game_data.as_mut()}, &mut next)?;
    }
    if let Some(id)=item_reward {
        let seed=match (&old.phase,&action) {
            (Phase::Reward{id:node,..},Action::Claim(index))=>erdtree_tower::weapon_rolls::card_identity(&old,*node,*index,id),
            _=>None,
        };
        super::card_affixes::grant(unsafe {player.player_game_data.as_mut()},id,seed,|data|super::items::give_reward_with_ash(data,id,reward_ash))?;
    }
    if next.flask_bonus!=old.flask_bonus {
        state.flasks.as_mut().ok_or("圣杯瓶未绑定")?.set_bonus(unsafe {player.player_game_data.as_mut()},next.class,next.flask_bonus,next.total_flask_cut())?;
    }
    if next.memory_bonus!=old.memory_bonus {unsafe {player.player_game_data.as_mut()}.unlocked_magic_slots=2+next.memory_bonus;}
    unsafe {player.player_game_data.as_mut()}.rune_count=next.gold;
    state.view.runes=next.gold;
    let vitals = &mut player.chr_ins.modules.data;
    vitals.hp = next_hp.min(vitals.max_hp);
    vitals.fp = next_fp.min(vitals.max_fp);
    next.hp = normalized_hp(vitals.hp, vitals.max_hp);
    let saved=record_checked(&next);
    state.view.run = Some(next);
    saved.map_err(|error|format!("奖励已发放，但存档写入失败（{error}），稍后自动重试保存"))
}

fn contract_action(state:&mut Runtime,world:&mut WorldChrMan,action:Action)->Result<(),String> {
    super::contracts::validate()?;
    let run=state.view.run.as_mut().ok_or("旅程尚未开启")?;
    if run.phase!=Phase::Map{return Err("当前不能选择契约或适配装备".into());}
    let player=world.main_player.as_mut().ok_or("角色尚未就绪")?;
    if player.chr_ins.modules.data.hp<=0{return Err("角色已死亡".into());}
    let reward_seed=run.reward_seed();
    let contract=run.contract.as_mut().ok_or("契约尚未开启")?;
    match action {
        Action::Contract(index)=>{
            contract.choose(index,reward_seed,run.class)?;
            if run.contract.as_ref().is_some_and(|c|c.deepened) {
                record_checked(run)?;
                super::contracts::deliver_deepen(unsafe {player.player_game_data.as_mut()},run)?;
            }
        },
        Action::SkipGear=>contract.skip_gift()?,
        Action::SkipContract=>contract.skip()?,
        Action::Gear(index)=>{
            contract.begin_gift(index)?;
            let entries=super::contracts::gift_entries(contract,index,unsafe {player.player_game_data.as_ref()})?;
            record_checked(run)?;
            for (slot,(id,qty)) in entries.into_iter().enumerate() {
                if run.contract.as_ref().unwrap().gift.as_ref().unwrap().delivered[slot]{continue;}
                let gem=super::random_ashes::preview(run,usize::MAX,index,id);
                let seed=(slot==0).then(||erdtree_tower::weapon_rolls::card_identity(run,usize::MAX,index,id)).flatten();
                super::card_affixes::grant(unsafe {player.player_game_data.as_mut()},id,seed,|data|super::contracts::give_with_ash(data,id,qty,gem))?;
                run.contract.as_mut().unwrap().gift.as_mut().unwrap().delivered[slot]=true;
                record_checked(run)?;
            }
            run.contract.as_mut().unwrap().finish_gift()?;
        },
        _=>return Err("无效契约操作".into()),
    }
    record_checked(run)?;let light=super::contracts::light_load(unsafe {player.player_game_data.as_ref()});super::contracts::sync(&mut player.chr_ins,Some(run),light);Ok(())
}

fn flask_loading(travel:Option<&super::travel::Travel>,warp:bool,room_loading:bool)->bool {
    warp||room_loading||travel.is_some_and(|travel|!travel.resources_restored)
}
fn prepare_regional_decision(run:&Run,decide:impl FnOnce(&mut Run)->Result<(),String>,close:impl FnOnce()->Result<(),String>)->Result<Run,String> {
    let mut next=run.clone();decide(&mut next)?;
    close()?;
    Ok(next)
}
fn sync_merchant_reward(state:&mut Runtime,world:&mut WorldChrMan)->Result<(),String> {
    let phase=state.party.as_ref().map_or(0,|p|p.event.merchant_phase);
    if phase==0{return Ok(());}
    let Some(run)=state.view.run.as_ref() else{return Ok(());};
    let Phase::Node{id}=run.phase else{return Ok(());};
    if run.nodes[id].kind!=erdtree_tower::NodeKind::Shop{return Err("商人挑战与当前节点不匹配".into());}
    let had_challenge=run.bells.challenge.is_some();
    let mut next=run.clone();
    if !next.bells.down(next.act,id){next.challenge_merchant()?;}
    let reward=if phase==3 {next.defeat_hunter()?}else{None};
    if next!=*run {record_checked(&next)?;state.view.run=Some(next);}
    if phase==3&&(had_challenge||reward.is_some()) {
        if state.ash_guard.is_none()&&let Some(run)=state.view.run.as_ref()&&let Some(plan)=keep_summoned_ash(world,run) {
            state.ash_refunds=plan;state.ash_guard=Some(Instant::now());
        }
        world.summon_buddy_manager.disappear_requested=true;
    }
    if let Some(kind)=reward {report(state,format!("获得{}！本局后续商店对应商品 +5，全队共享。",erdtree_tower::merchant::NAMES[kind]));}
    Ok(())
}
fn tick_merchant(state:&mut Runtime,world:&mut WorldChrMan,room:&mut super::rooms::Room)->Result<bool,String> {
    let Some(run)=state.view.run.as_ref() else{return Ok(false);};
    let host=state.party.as_ref().is_none_or(|p|p.host());
    let newly_down=!room.merchant_down&&!run.bells.down(run.act,room.node)&&room.merchant_dead(world);
    if newly_down&&!erdtree_tower::merchant::hunter_appears(run.seed,run.act,room.node) {
        room.merchant_down=true;room.suspend_dialogues()?;
        room.view.message="商人已倒下，本次未出现铃珠猎人，可通过传送门继续。".into();
        super::log(format!("MERCHANT_HUNT skipped act={} node={} chance={}%",run.act,room.node,erdtree_tower::merchant::HUNTER_CHANCE));
        return Ok(false);
    }
    if newly_down&&host {
        if state.party.as_ref().is_some_and(|p|!p.gate.merchant_present()) {
            room.merchant_down=true;room.suspend_dialogues()?;
            room.view.message="商人已倒下；需全队仍在商店才能触发铃珠猎人。可通过传送门继续。".into();return Ok(false);
        }
        room.suspend_dialogues()?;
        let mut next=run.clone();next.challenge_merchant()?;record_checked(&next)?;
        super::log(format!("MERCHANT_HUNT start act={} node={} party={}",next.act,room.node,state.party.is_some()));state.view.run=Some(next);
    }
    let run=state.view.run.as_ref().unwrap();
    room.merchant_down|=run.bells.down(run.act,room.node);
    if run.bells.challenge.is_none(){
        if room.hunter.is_some()&&room.hunter_defeated_at.get_or_insert_with(Instant::now).elapsed()>=Duration::from_secs(8)
            &&let Some(hunter)=room.hunter.take(){hunter.retire();}
        return Ok(false);
    }
    room.suspend_dialogues()?;
    super::native_talk::set_flag(room.base()+993,false)?;
    super::native_talk::set_flag(room.base()+994,false)?;
    if room.hunter.is_none(){room.hunter=Some(super::map_guards::Guards::merchant()?);}
    let hunter=room.hunter.as_mut().unwrap();hunter.prepare(world)?;
    hunter.scale_merchant(world,run,room.node)?;
    let mut started=hunter.ready();
    if let Some(link)=state.party.as_mut() {
        if host&&link.event.merchant_phase==0 {link.merchant_phase(1)?;}
        link.event.merchant_bound=hunter.ready();
        if host&&link.event.merchant_phase==1&&link.gate.merchant_ready(){link.merchant_phase(2)?;}
        started=link.event.merchant_phase>=2;
    }
    room.view.message=if started {"铃珠猎人已出现：击败后获得随机铃珠。"}else{"商人已倒下，正在等待全队加载铃珠猎人……"}.into();
    if hunter.poll(world,started,host)? {
        // 召唤师「群灵」：击败铃珠猎人时召唤物还活着，补回这次用掉的骨灰
        if let Some(run)=state.view.run.as_ref()&&let Some(plan)=keep_summoned_ash(world,run) {state.ash_refunds=plan;state.ash_guard=Some(Instant::now());}
        world.summon_buddy_manager.disappear_requested=true;
        if let Some(link)=state.party.as_mut(){link.merchant_phase(3)?;sync_merchant_reward(state,world)?;}
        else {
            let mut next=state.view.run.clone().unwrap();let kind=next.defeat_hunter()?;record_checked(&next)?;state.view.run=Some(next);
            if let Some(kind)=kind {report(state,format!("获得{}！本局后续商店对应商品 +5。",erdtree_tower::merchant::NAMES[kind]));}
        }
        room.hunter_defeated_at=Some(Instant::now());
        room.view.message="铃珠猎人已击败，铃珠已收入本局收藏，使用传送门继续。".into();
        super::native_talk::set_flag(room.base()+993,true)?;return Ok(false);
    }
    Ok(true)
}
fn tick_room(state: &mut Runtime, world: &mut WorldChrMan, game: &mut GameMan, dead: bool) {
    if dead&&state.party.as_ref().is_some_and(|p|!p.defeated)&&!state.view.run.as_ref().is_some_and(|r|r.bells.challenge.is_some()) {
        state.command=None;state.view.ready=false;state.view.busy=true;
        if let Some(room)=state.room.as_ref().filter(|room|room.mausoleum) {
            let link=state.party.as_ref().unwrap();
            super::log_repeating(format!("MAUSOLEUM_DEATH host={} epoch={} flow={:?} arrival_block={:?} recovering={}",link.host(),link.epoch,super::mausoleum::flow(),room.arrival_block_position,state.party_death.awaiting_revival()));
        }
        report(state,"等待队友救援或全队结算");return;
    }
    let mut room = state.room.take().unwrap();
    if dead && state.party.is_none() && !room.view.loading && !game.warp_requested {
        room.retire(world);
        if let Some(run) = &mut state.view.run {
            run.end_on_death();
            record(run);
        }
        state.command = None;
        state.view.room = None;
        state.view.busy = false;
        state.survival_engaged = None;
        super::custom_event_ui::close();super::custom_event_ui::set_survival_hud(None);
        report(state, "角色死亡，本局结束。");
        return;
    }
    let mut transferred=false;
    let result = (|| -> Result<bool, String> {
        if state.view.shared_event {return Ok(false);}
        if room.kind==erdtree_tower::NodeKind::Shop {
            room.merchant_down|=state.view.run.as_ref().is_some_and(|r|r.bells.down(r.act,room.node));
        }
        if room.kind==erdtree_tower::NodeKind::Event {
            if let Some(run)=state.view.run.as_ref(){room.event_menu(run,state.party.is_some(),state.party.as_ref().is_some_and(|p|p.event.request.is_some()),state.party.is_none());}
        }
        let storm_order=state.view.run.as_ref().and_then(|r|r.regional_event.stormveil)
            .and_then(|_|state.party.as_ref().and_then(|p|p.event.storm_decision));
        let trial_order=state.view.run.as_ref().filter(|r|r.regional_event.trial)
            .and_then(|_|state.party.as_ref().and_then(|p|p.event.trial_decision));
        room.tent=room.kind==erdtree_tower::NodeKind::Camp&&state.view.run.as_ref().is_some_and(|r|r.tent_available());
        // 单人营地暂退读档回到同一篝火：若本节点已发过免费强化石，不再重复发放。
        if room.kind==erdtree_tower::NodeKind::Camp&&!room.allowance_issued&&!room.allowance_failed
            &&state.view.run.as_ref().is_some_and(|r|!r.camp_stone_available(room.node)) {
            if let Some(player)=world.main_player.as_ref() {
                let held=super::smith::remaining(unsafe {player.player_game_data.as_ref()});
                room.allowance_issued=true;room.allowance_target=held;
                super::log(format!("Camp allowance already granted for node={}; skipping duplicate grant (held={held})",room.node));
            }
        }
        if storm_order.is_none()&&trial_order.is_none(){room.poll(world, game)?;}
        if room.kind==erdtree_tower::NodeKind::Camp&&room.allowance_issued
            &&let Some(run)=state.view.run.as_mut()&&run.camp_stone_available(room.node) {
            run.mark_camp_stone_granted(room.node);record(run);
        }
        if let Some((angry,dead))=room.patches_state()&&let Some(run)=state.view.run.as_ref()
            &&(run.regional_event.patches_angry!=angry||run.regional_event.patches_dead!=dead) {
            let mut next=run.clone();next.regional_event.patches_angry=angry;next.regional_event.patches_dead=dead;
            record_checked(&next)?;state.view.run=Some(next);
            super::log(format!("PATCHES_EVENT angry={angry} dead={dead}"));
        }
        state.command = None;
        if room.view.loading&&storm_order.is_none()&&trial_order.is_none() {
            return Ok(false);
        }
        if room.kind==erdtree_tower::NodeKind::Shop {
            let arrived=!game.warp_requested&&world.main_player.as_ref().is_some_and(|p|
                i32::from(p.current_block_id)==room.block&&p.chr_ins.modules.physics.standing_on_solid_ground);
            if let Some(link)=state.party.as_mut() {
                if link.arrived!=arrived {super::log(format!("MERCHANT_HUNT arrival epoch={} ready={arrived}",link.epoch));}
                link.arrived=arrived;
            }
            if !arrived{return Ok(false);}
            if room.shop_refresh_pending {
                let count=state.view.run.as_ref().and_then(|r|r.shop_stock.as_ref()).filter(|s|s.node==room.node).ok_or("新商店货单未准备完成")?.items.len();
                super::native_shop::reset_arrived_stock(count)?;room.shop_refresh_pending=false;
                // F013 诊断：联机时进店记下限购道具（露滴、钥匙等）在三份列表里各有几个，对照之后买不买得到
                if let (Some(link),Some(player))=(state.party.as_ref(),world.main_player.as_ref()) {
                    let data=unsafe {player.player_game_data.as_ref()};
                    let goods:Vec<_>=state.view.run.as_ref().and_then(|r|r.shop_stock.as_ref()).map(|s|s.items.iter().enumerate()
                        .filter(|(_,i)|i[0]==3&&i[3]==1).map(|(slot,i)|(slot,i[1],super::loot::goods_lists(data,i[1] as u32))).collect()).unwrap_or_default();
                    super::log(format!("SHOP_STOCK party goods role={} (slot,id,[bag,key,multiplay_key])={goods:?}",if link.host(){"host"}else{"member"}));
                }
            }
            // 诊断：售出计数每秒比一次，变了就记下（本机买东西时卢恩会同时减少；卢恩没变而计数变了，说明是别人的旗标同步过来的）
            if (room.shop_counters.as_ref().is_none_or(|(at,_)|at.elapsed()>=Duration::from_secs(1))||super::native_talk::flag(room.base()+994)?)
                &&let (Some(run),Some(player))=(state.view.run.as_mut(),world.main_player.as_mut()) {
                super::native_shop::redeem_ember(run,room.node,unsafe{player.player_game_data.as_mut()})?;
            }
            if room.shop_counters.as_ref().is_none_or(|(at,_)|at.elapsed()>=Duration::from_secs(1)) {
                let count=state.view.run.as_ref().and_then(|r|r.shop_stock.as_ref()).filter(|s|s.node==room.node).map_or(0,|s|s.items.len());
                if let Ok(counts)=super::native_shop::sold_counts(count) {
                    if let Some((_,before))=&room.shop_counters&&*before!=counts {
                        let changed:Vec<_>=before.iter().zip(&counts).enumerate().filter(|(_,(a,b))|a!=b).map(|(i,(a,b))|(i,*a,*b)).collect();
                        let runes=world.main_player.as_ref().map(|p|unsafe {p.player_game_data.as_ref()}.rune_count);
                        super::log(format!("SHOP_STOCK counters changed role={} runes={runes:?} (slot,before,after)={changed:?}",
                            state.party.as_ref().map_or("solo",|p|if p.host(){"host"}else{"member"})));
                        // F013 诊断：联机时买露滴等重要物品，看它进了哪份列表（单机重要物品 / 联机重要物品）
                        let stock=state.view.run.as_ref().and_then(|r|r.shop_stock.as_ref()).map(|s|s.items.clone()).unwrap_or_default();
                        if let Some(player)=world.main_player.as_ref() {
                            for &(slot,_,_) in &changed {
                                let Some(&[kind,id,..])=stock.get(slot) else{continue};
                                if kind==3 {super::log(format!("SHOP_STOCK goods slot={slot} id={id} lists={:?}",super::loot::goods_lists(unsafe {player.player_game_data.as_ref()},id as u32)));}
                            }
                        }
                    }
                    room.shop_counters=Some((Instant::now(),counts));
                }
            }
        }
        if (storm_order.is_some()||trial_order.is_some())&&(game.warp_requested||!world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.physics.standing_on_solid_ground)){return Ok(false);}
        if state.view.run.as_ref().is_some_and(|r|r.regional_event.trial) {
            use erdtree_tower::regional_events::Trial;
            tick_regional_event(state,world,&mut room)?;
            let choice=if let Some(super::native_talk::EventRequest::Trial(choice))=room.take_event_request(){Some(choice)}else{None};
            let decision=if let Some(link)=state.party.as_mut(){
                if let Some(choice)=choice {link.event.trial_choice=Some(choice);super::log(format!("Trial vote: {choice:?}"));}
                if link.event.trial_choice.is_some(){room.view.message="已投票，等待全队选择；票数最多者生效，平票由本局种子决定。可再次交谈改票。".into();}
                link.event.trial_decision
            }else{choice};
            if let Some(choice)=decision {
                let mut next=state.view.run.clone().ok_or("旅程未绑定")?;
                next.gold=unsafe {world.main_player.as_ref().ok_or("角色未加载")?.player_game_data.as_ref()}.rune_count;
                let mut next=prepare_regional_decision(&next,|r|r.choose_trial(choice),||room.suspend_dialogues())?;
                match choice {
                    Trial::Mausoleum=>{
                        let player=world.main_player.as_ref().ok_or("角色未加载")?;
                        let next_room=super::rooms::Room::for_run(&next,room.node,erdtree_tower::NodeKind::Event,player.chr_ins.modules.data.hp,player.chr_ins.modules.data.fp)?;
                        record_checked(&next)?;
                        super::spirits::prepare_warp(world,"trial_mausoleum");
                        game.move_map_target=BlockId::from(next_room.block);game.initial_area_entity_id=next_room.entry;game.warp_requested=true;
                        if let Some(scene)=state.party_scene.as_mut(){scene.run=next.clone();scene.block=next_room.block;}
                        state.view.run=Some(next);state.view.room=Some(next_room.view.clone());state.view.busy=true;
                        state.room=Some(next_room);transferred=true;
                    },
                    Trial::Descent=>{start_descent(state,world,game,next)?;transferred=true;},
                    Trial::Stormveil|Trial::Chariot|Trial::HereticalRise=>{start_stormveil(state,world,game,next)?;transferred=true;},
                    Trial::Skip=>{
                        next.leave_regional_event()?;record_checked(&next)?;
                        let player=world.main_player.as_mut().ok_or("角色未加载")?;
                        unsafe {player.player_game_data.as_mut()}.rune_count=next.gold;
                        state.view.runes=next.gold;state.view.run=Some(next);
                    }
                }
                return Ok(true);
            }
            return Ok(false);
        }
        if state.view.run.as_ref().is_some_and(|r|r.regional_event.stormveil.is_some()) {
            use erdtree_tower::stormveil::Decision;
            tick_regional_event(state,world,&mut room)?;
            let choice=if let Some(super::native_talk::EventRequest::Stormveil(challenge))=room.take_event_request(){Some(challenge)}else{None};
            let decision=if let Some(link)=state.party.as_mut() {
                if let Some(choice)=choice {link.event.storm_choice=Some(choice);}
                if link.event.storm_choice.is_some(){room.view.message="等待全队选择：全员同意后挑战，有人跳过则全队领取 800 卢恩。".into();}
                link.event.storm_decision
            }else{choice.map(|challenge|if challenge{Decision::Challenge}else{Decision::Skip})};
            if let Some(decision)=decision {
                let mut next=prepare_regional_decision(state.view.run.as_ref().ok_or("旅程未绑定")?,|r|r.stormveil_decide(decision),||room.suspend_dialogues())?;
                match decision {
                    Decision::Challenge=>{start_stormveil(state,world,game,next)?;transferred=true;return Ok(true);},
                    Decision::Skip=>{
                        next.leave_regional_event()?;record_checked(&next)?;
                        let player=world.main_player.as_mut().ok_or("角色未加载")?;
                        unsafe {player.player_game_data.as_mut()}.rune_count=next.gold;
                        state.view.run=Some(next);return Ok(true);
                    },
                    Decision::Engage|Decision::Complete=>return Err("跑城尚未开始".into()),
                }
            }
            return Ok(false);
        }
        if state.view.run.as_ref().is_some_and(|r|r.regional_event.descent)
            &&matches!(room.take_event_request(),Some(super::native_talk::EventRequest::Descent)) {
            let mut next=state.view.run.clone().ok_or("旅程未绑定")?;
            next.accept_descent()?;
            start_descent(state,world,game,next)?;
            transferred=true;return Ok(true);
        }
        let flags = unsafe { eldenring::cs::CSEventFlagMan::instance_mut() }
            .map_err(|_| "原生事件系统未加载")?;
        let base = room.base();
        if room.kind==erdtree_tower::NodeKind::Event {
            tick_regional_event(state,world,&mut room)?;
        }
        if room.kind == erdtree_tower::NodeKind::Camp {
            let arrived=!game.warp_requested&&world.main_player.as_ref().is_some_and(|p|
                i32::from(p.current_block_id)==room.block&&p.chr_ins.modules.physics.standing_on_solid_ground);
            if arrived&&let Some(run)=state.view.run.as_mut() {
                if run.physick_drop(room.node,false)&&run.physick_supply.queue(run.act,room.node){record(run);}
            }
            // 战斗法师：铁匠菜单里选了智力或信仰，给右手武器附加 S 补正（免费，记进存档；参数下一帧由 embers::sync 写进内存）
            if let Some(stat)=super::native_talk::take_battlemage() {
                match battlemage_convert(state,world,stat) {
                    Ok(message)=>{room.notice(super::native_talk::BATTLEMAGE_DONE)?;room.view.message=message;},
                    Err(error)=>{super::log(format!("TOWER_EMBER battlemage refused: {error}"));room.notice(super::native_talk::BATTLEMAGE_REFUSED)?;room.view.message=error;},
                }
            }
            let data=unsafe {world.main_player.as_mut().ok_or("角色未加载")?.player_game_data.as_mut()};
            if let Some(run)=state.view.run.as_mut() {
                let held_spirit=super::spirit_tune::inventory_peak(data);
                let next_peak=spirit_peak(run,held_spirit);
                if run.spirit_peak!=Some(next_peak) {
                    run.spirit_peak=Some(next_peak);
                    record(run);
                }
                super::spirit_tune::set_peak(erdtree_tower::rewards::spirit_level_now(run));
            }
            if arrived&&let Some(run)=state.view.run.as_mut(){
                tick_camp_revival(run,&mut room,data)?;
            }
            if super::smith::spent(room.allowance_issued,room.view.completed,super::smith::remaining(data),room.allowance_target) {
                let run=state.view.run.as_mut().ok_or("角色未绑定")?;
                // 调灵也走这里：调灵后的骨灰等级立即记进本局（骨灰召唤后会用掉，不能等下一场胜利再看背包）
                run.spirit_peak=Some(spirit_peak(run,super::spirit_tune::inventory_peak(data)));
                run.leave_camp()?;
                record(run);
                room.complete("已用掉营地送的强化石，这个营地不能再补瓶。升级、换战灰、记忆法术等其他服务照常可用，准备好后从传送门离开。".into());
                super::log("Native camp reinforcement consumed its single allowance");
            }
            let request = room.take_refill().and_then(|choice|match choice {1=>Some(CampChoice::Refill),2=>Some(CampChoice::RefillBlue),3=>Some(CampChoice::RefillRedOnly),4=>Some(CampChoice::RefillBlueOnly),_=>None});
            if let Some(choice) = request {
                // 余烬「帐篷」：先用免费的那一次，不扣营地发的强化石、不结束营地服务
                let tent=state.view.run.as_ref().is_some_and(|r|r.tent_available());
                let result = if room.view.completed&&!tent {
                    Err("篝火服务已用完".to_string())
                } else {
                    transact(state, world, Action::Camp(choice), false, false)
                };
                if let Err(error) = &result {
                    super::log(format!("Camp refill failed: {error}"));
                }
                if result.is_ok()&&tent {
                    super::log(format!("Camp refill done: {choice:?} free=tent party={}",state.party.is_some()));
                    if !room.view.completed {room.view.message="余烬「帐篷」：这次补瓶免费，营地送的强化石还在，仍可强化或再补一次瓶。".into();}
                } else if result.is_ok() {
                    super::log(format!("Camp refill done: {choice:?} party={} stone_issued={}",state.party.is_some(),room.allowance_issued));
                    // 仅当本营地确实发放了免费强化石时才收回一块，避免发放超时失败时倒扣玩家自攒的强化石。
                    if room.allowance_issued {
                        if let Err(error)=super::smith::consume_one(unsafe {world.main_player.as_mut().unwrap().player_game_data.as_mut()}) {super::log(format!("Camp refill stone: {error}"));}
                        room.complete("补瓶完成，营地送的强化石已收回。其他服务照常可用，准备好后从传送门离开。".into());
                    } else {
                        room.complete("补瓶完成。其他服务照常可用，准备好后从传送门离开。".into());
                    }
                }
                room.notice(if result.is_err(){9500012}else if tent{super::native_talk::TENT_REFILL}else{9500010})?;
            }
        }
        if room.kind == erdtree_tower::NodeKind::Chest {tick_chest_award(state,world,&mut room)?;}
        if room.kind == erdtree_tower::NodeKind::Shop {
            if tick_merchant(state,world,&mut room)? {return Ok(false);}
            let player = world.main_player.as_mut().ok_or("角色未加载")?;
            let pending = super::native_shop::pending(unsafe { player.player_game_data.as_ref() });
            if !pending.is_empty() {
                let mut next = state.view.run.clone().ok_or("角色未绑定")?;
                super::native_shop::add_blessings(&mut next, &pending);
                super::blessings::Blessings::deliver(unsafe {player.player_game_data.as_mut()}, &mut next)?;
                super::native_shop::consume(unsafe { player.player_game_data.as_mut() });
                record(&next);
                state.view.run = Some(next);
                super::log(format!("Native shop blessings applied: {pending:?}"));
            }
        }
        if flags.virtual_memory_flag.get_flag(base + 994) {
            flags.virtual_memory_flag.set_flag(base + 994, false);
            if room.scarabs.active()||state.party.as_ref().is_some_and(|p|p.event.pending()) {return Ok(false);}
            let run = state.view.run.as_mut().ok_or("角色未绑定")?;
            if matches!(run.phase, Phase::Node { .. }) {
                match room.kind {
                    erdtree_tower::NodeKind::Camp => run.leave_camp()?,
                    erdtree_tower::NodeKind::Shop => run.leave_shop()?,
                    erdtree_tower::NodeKind::Event => run.leave_regional_event()?,
                    _ => return Err("宝箱尚未领取".into()),
                }
            }
            record(run);
            return Ok(true);
        }
        Ok(false)
    })();
    if transferred {room.retire(world);return;}
    if let Some(next)=state.view.run.as_ref().filter(|run|run.retry_node.is_some()&&!state.death_retry).cloned() {
        state.room=Some(room);
        begin_death_retry(state,world,next);
        return;
    }
    if matches!(result, Ok(true)) {
        settle_portal_ashes(state,world);
        state.event_start=None;state.survival_engaged=None;
        super::custom_event_ui::close();super::custom_event_ui::set_survival_hud(None);
        room.retire(world);
        state.view.room = None;
        state.view.busy = false;
        report(state, "已通过传送门，请选择下一层节点。");
        return;
    }
    if let Err(error) = result {
        if room.view.message != error {
            super::log(&error);
            if room.kind==erdtree_tower::NodeKind::Event {let _=room.notice(9500056);}
        }
        room.view.message = error;
    }
    state.view.busy = room.view.loading;
    state.view.room = Some(room.view.clone());
    state.room = Some(room);
}

fn battlemage_convert(state:&mut Runtime,world:&WorldChrMan,stat:u8)->Result<String,String> {
    let player=world.main_player.as_ref().ok_or("角色未加载")?;
    let run=state.view.run.as_ref().ok_or("旅程未绑定")?;
    let row=super::weapons::battlemage_target(unsafe {player.player_game_data.as_ref()},&run.battlemage)?;
    let mut next=run.clone();next.battlemage_convert(row,stat)?;record_checked(&next)?;state.view.run=Some(next);
    let name=if stat==0{"智力"}else{"信仰"};
    super::log(format!("TOWER_EMBER battlemage weapon={row} stat={name}"));
    Ok(format!("右手武器获得了{name}补正（普通+25／失色+10 达 S），{name}也计入物理伤害。"))
}
fn tick_chest_award(state:&mut Runtime,world:&mut WorldChrMan,room:&mut super::rooms::Room)->Result<(),String> {
    use super::{loot,chest_award::Receipt,native_talk::{flag,set_flag}};
    if room.view.completed {set_flag(45010995,true)?;return Ok(());}
    let run=state.view.run.as_ref().ok_or("个人宝箱进度尚未加载")?;
    if run.phase!= (Phase::Node{id:room.node}) {return Err("个人宝箱进度不匹配，未发放物品".into());}
    let path=award_path()?.with_extension(format!("chest-{:016x}-{}",run.seed,room.node));
    if !flag(45010990)?&&!path.exists(){return Ok(());}
    if room.chest_attempt.is_some_and(|at|at.elapsed()<Duration::from_secs(1)){return Ok(());}
    room.chest_attempt=Some(Instant::now());
    // 新旅程（余烬规则 2 起）：宝箱给一枚随机余烬；已经开始发放的物品（有领取记录）照旧发完，余烬拿完了也照旧给物品
    if !path.exists()&&let Some(ember)=erdtree_tower::embers::chest_ember(run,room.node) {
        let mut next=run.clone();next.claim_chest_ember()?;record_checked(&next)?;
        state.ember_notice.queue(next.seed,next.act,ember);
        state.view.run=Some(next);room.complete(format!("获得余烬「{}」",ember.name()));
        set_flag(45010995,true)?;
        report(state,format!("宝箱里是余烬「{}」！",ember.name()));
        super::log(format!("Personal chest ember: node={} ember={ember:?}",room.node));
        return Ok(());
    }
    let data=unsafe {world.main_player.as_mut().ok_or("角色尚未加载")?.player_game_data.as_mut()};
    let receipt=if let Some(receipt)=Receipt::load(&path)? {receipt}else {
        let tools=loot::casting_tools(data);
        let item=loot::chest_choice_for_run(run,room.node,|id|loot::count(data,id)>0||!erdtree_tower::rewards::castable(id,tools))?;
        Receipt::begin(&path,item.id as u32,loot::count(data,item.id as u32))?
    };
    if receipt.needs_grant(loot::count(data,receipt.item)) {super::items::give_reward(data,receipt.item)?;}
    if receipt.needs_grant(loot::count(data,receipt.item)){return Err("宝箱奖励尚未到账，请清理背包容量".into());}
    let mut next=run.clone();next.finish_chest()?;record_checked(&next)?;
    state.view.run=Some(next);room.complete("宝箱已领取".into());
    set_flag(45010995,true)?;
    if let Err(error)=std::fs::remove_file(path){super::log(format!("Completed chest receipt retained: {error}"));}
    super::log(format!("Personal chest awarded: node={} item={}",room.node,receipt.item));
    Ok(())
}

/// 赌运事件结算后的对话框（事件：0 骰子、1 盲盒、2 重铸；选项 0 / 1）
fn gamble_result(index:usize,choice:usize,wins:bool)->i32 {
    use super::native_talk::{GAMBLE_DICE_LOSE,GAMBLE_DICE_WIN,GAMBLE_MYSTERY,GAMBLE_REFORGE};
    match index {
        erdtree_tower::events::DICE=>(if wins {GAMBLE_DICE_WIN}else{GAMBLE_DICE_LOSE})+choice as i32,
        erdtree_tower::events::MYSTERY=>GAMBLE_MYSTERY,
        erdtree_tower::events::FURNACE=>super::native_talk::FURNACE_RESULT,
        _=>GAMBLE_REFORGE+choice as i32,
    }
}
fn collect_inventory_spirits(data:&eldenring::cs::PlayerGameData)->Vec<super::custom_event_ui::SpiritEntry> {
    let mut seen=std::collections::HashSet::new();
    let mut list=Vec::new();
    for item in super::loot::bag_items(data) {
        if item.category()!=eldenring::cs::ItemCategory::Goods {continue;}
        let id=0x4000_0000|item.param_id();
        let Some(level)=erdtree_tower::rewards::spirit_level(id) else{continue;};
        if !seen.insert(id) {continue;}
        let base=erdtree_tower::rewards::spirit_base(id);
        list.push(super::custom_event_ui::SpiritEntry {id,base,level});
    }
    list.sort_by(|a,b|b.level.cmp(&a.level).then_with(||a.base.cmp(&b.base)));
    list
}
fn grant_survival_rewards(world:&mut WorldChrMan,run:&mut Run,node:usize,kills:u8)->Result<(u32,Vec<String>),String> {
    let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
    let data=unsafe {player.player_game_data.as_mut()};
    let full=erdtree_tower::new_events::survival_full_clear(kills);
    let mut items=Vec::new();
    if let Some(weapon)=erdtree_tower::new_events::survival_equipment(run,node,full) {items.push(weapon);}
    if full {items.push(erdtree_tower::new_events::frenzy_talisman(run,node,0x5017_7a11));}
    run.gold=data.rune_count;
    let reward=run.finish_survival(kills,&items)?;
    let mut names=Vec::new();
    for &item in &items {
        super::items::give_reward(data,item)?;
        let label=erdtree_tower::rewards::lookup(item).map(|i|i.name.to_string()).unwrap_or_else(||format!("装备 #{item}"));
        names.push(label);
    }
    data.rune_count=run.gold;
    Ok((reward,names))
}
fn tick_custom_event(state:&mut Runtime,world:&mut WorldChrMan,room:&mut super::rooms::Room)->Result<bool,String> {
    use erdtree_tower::events::Draw;
    use super::{custom_event_ui as ui,native_talk::{EventRequest,set_flag}};
    let run=state.view.run.as_ref().ok_or("旅程未绑定")?;
    let draw=run.event_draw(room.node);
    if !matches!(draw,Draw::EmberFurnace|Draw::SpiritFurnace|Draw::FrenzyAltar|Draw::Survival) {return Ok(false);}
    set_flag(45010965,false)?;set_flag(45010960,false)?;
    set_flag(45010941,true)?;set_flag(45010940,true)?;
    let fighting=draw==Draw::Survival&&run.regional_event.guards;
    set_flag(45010942,fighting)?;
    set_flag(45010995,!fighting&&!ui::active())?;
    let key=(run.seed,run.act,room.node);
    if let Some(EventRequest::CustomPanel)=room.take_event_request() {
        let player=world.main_player.as_ref().ok_or("角色尚未加载")?;
        let data=unsafe {player.player_game_data.as_ref()};
        match draw {
            Draw::EmberFurnace if run.regional_event.opened==0 => {
                let spirits=!collect_inventory_spirits(data).is_empty()||run.spirit_peak.is_some();
                let tools=super::loot::casting_tools(data);
                ui::open_ember_furnace(ui::EmberFurnaceState {
                    key,runes:data.rune_count,owned:run.embers.clone(),
                    spirits,casting:tools[0]||tools[1],run_snapshot:run.clone(),
                });
            }
            Draw::SpiritFurnace if run.regional_event.opened==0 => {
                ui::open_spirit_furnace(ui::SpiritFurnaceState {
                    key,runes:data.rune_count,spirits:collect_inventory_spirits(data),run_snapshot:run.clone(),
                });
            }
            Draw::FrenzyAltar if run.regional_event.opened==0 => {
                let spirits=!collect_inventory_spirits(data).is_empty()||run.spirit_peak.is_some();
                let tools=super::loot::casting_tools(data);
                let blood_embers=erdtree_tower::new_events::ember_furnace_offers(run,room.node,None,spirits,tools[0]||tools[1]);
                ui::open_frenzy_altar(ui::FrenzyAltarState {
                    key,can_burn_flask:run.can_burn_flask(),blood_embers,
                });
            }
            Draw::Survival if run.regional_event.opened==0&&!run.regional_event.guards => {
                ui::open_survival_offer(ui::SurvivalOfferState {
                    key,
                });
            }
            _=>{}
        }
    }
    if let Some(req)=ui::take_request() {
        let req_key=match &req {
            ui::Request::EmberFurnace{key,..}|ui::Request::SpiritDevour{key,..}|ui::Request::SpiritFuse{key,..}
            |ui::Request::SpiritBoost{key,..}|ui::Request::SpiritFind{key,..}|ui::Request::FrenzyAltar{key,..}
            |ui::Request::SurvivalStart{key}|ui::Request::SurvivalSkip{key}=>*key,
        };
        if req_key==key {
            let mut next=run.clone();
            match req {
                ui::Request::EmberFurnace{burned,picked,price,..}=>{
                    let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
                    let data=unsafe {player.player_game_data.as_mut()};
                    next.gold=data.rune_count;
                    next.resolve_ember_furnace(burned,picked,price)?;
                    data.rune_count=next.gold;state.view.runes=next.gold;
                    record_checked(&next)?;state.view.run=Some(next);
                    let action=if let Some(b)=burned {format!("已投入「{}」并铸得余烬「{}」",b.name(),picked.name())} else {format!("已从灰烬中拾得余烬「{}」",picked.name())};
                    room.view.message=format!("余烬熔炉：{action}。请从传送门离开。");
                    super::log(format!("EMBER_FURNACE burned={burned:?} picked={picked:?} price={price}"));
                    return Ok(true);
                }
                ui::Request::SpiritDevour{main_id,sacrifice_id,target_id,..}=>{
                    let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
                    let data=unsafe {player.player_game_data.as_mut()};
                    let ok=if main_id==sacrifice_id {super::loot::goods_lists(data,main_id&0x0fffffff)[0]>=2}
                        else {super::loot::goods_lists(data,main_id&0x0fffffff)[0]>=1&&super::loot::goods_lists(data,sacrifice_id&0x0fffffff)[0]>=1};
                    if !ok {return Err("背包中缺少选定的骨灰，未消耗材料".into());}
                    next.gold=data.rune_count;
                    next.resolve_spirit_furnace(0,target_id)?;
                    super::loadout::remove_goods(data,main_id,1)?;
                    super::loadout::remove_goods(data,sacrifice_id,1)?;
                    super::items::give_reward(data,target_id)?;
                    super::spirit_tune::set_peak(erdtree_tower::rewards::spirit_level_now(&next));
                    record_checked(&next)?;state.view.run=Some(next);
                    let name=erdtree_tower::rewards::lookup(target_id).map(|i|i.name).unwrap_or("强化骨灰");
                    room.view.message=format!("骨灰融合炉：吞噬强化成功，获得「{name}」。请从传送门离开。");
                    super::log(format!("SPIRIT_FURNACE devour main={main_id} sac={sacrifice_id} target={target_id}"));
                    return Ok(true);
                }
                ui::Request::SpiritFuse{sacrifice_a,sacrifice_b,target_id,price,..}=>{
                    let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
                    let data=unsafe {player.player_game_data.as_mut()};
                    let ok=if sacrifice_a==sacrifice_b {super::loot::goods_lists(data,sacrifice_a&0x0fffffff)[0]>=2}
                        else {super::loot::goods_lists(data,sacrifice_a&0x0fffffff)[0]>=1&&super::loot::goods_lists(data,sacrifice_b&0x0fffffff)[0]>=1};
                    if !ok {return Err("背包中缺少选定的融合骨灰，未消耗材料".into());}
                    next.gold=data.rune_count;
                    next.resolve_spirit_furnace(price,target_id)?;
                    super::loadout::remove_goods(data,sacrifice_a,1)?;
                    super::loadout::remove_goods(data,sacrifice_b,1)?;
                    super::items::give_reward(data,target_id)?;
                    data.rune_count=next.gold;state.view.runes=next.gold;
                    super::spirit_tune::set_peak(erdtree_tower::rewards::spirit_level_now(&next));
                    record_checked(&next)?;state.view.run=Some(next);
                    let name=erdtree_tower::rewards::lookup(target_id).map(|i|i.name).unwrap_or("新骨灰");
                    room.view.message=format!("骨灰融合炉：双灰融魂成功，获得「{name}」。请从传送门离开。");
                    super::log(format!("SPIRIT_FURNACE fuse a={sacrifice_a} b={sacrifice_b} target={target_id} price={price}"));
                    return Ok(true);
                }
                ui::Request::SpiritBoost{main_id,target_id,price,..}=>{
                    let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
                    let data=unsafe {player.player_game_data.as_mut()};
                    if super::loot::goods_lists(data,main_id&0x0fffffff)[0]<1 {return Err("背包中缺少选定的骨灰，未消耗材料".into());}
                    next.gold=data.rune_count;
                    next.resolve_spirit_furnace(price,target_id)?;
                    super::loadout::remove_goods(data,main_id,1)?;
                    super::items::give_reward(data,target_id)?;
                    data.rune_count=next.gold;state.view.runes=next.gold;
                    super::spirit_tune::set_peak(erdtree_tower::rewards::spirit_level_now(&next));
                    record_checked(&next)?;state.view.run=Some(next);
                    let name=erdtree_tower::rewards::lookup(target_id).map(|i|i.name).unwrap_or("强化骨灰");
                    room.view.message=format!("骨灰融合炉：炉火淬炼成功，获得「{name}」。请从传送门离开。");
                    super::log(format!("SPIRIT_FURNACE boost main={main_id} target={target_id} price={price}"));
                    return Ok(true);
                }
                ui::Request::SpiritFind{target_id,price,..}=>{
                    let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
                    let data=unsafe {player.player_game_data.as_mut()};
                    next.gold=data.rune_count;
                    next.resolve_spirit_furnace(price,target_id)?;
                    super::items::give_reward(data,target_id)?;
                    data.rune_count=next.gold;state.view.runes=next.gold;
                    super::spirit_tune::set_peak(erdtree_tower::rewards::spirit_level_now(&next));
                    record_checked(&next)?;state.view.run=Some(next);
                    let name=erdtree_tower::rewards::lookup(target_id).map(|i|i.name).unwrap_or("骨灰");
                    room.view.message=format!("骨灰融合炉：唤灵拾遗获得「{name}」。请从传送门离开。");
                    super::log(format!("SPIRIT_FURNACE find target={target_id} price={price}"));
                    return Ok(true);
                }
                ui::Request::FrenzyAltar{pact,ember_pick,..}=>{
                    use erdtree_tower::new_events::{FrenzyPact,frenzy_legendary_weapon,frenzy_talisman};
                    let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
                    let data=unsafe {player.player_game_data.as_mut()};
                    let mut items=Vec::new();
                    match pact {
                        FrenzyPact::BurnFlask=>{
                            if let Some(w)=frenzy_legendary_weapon(&next,room.node,0x0f01_0001) {items.push(w);}
                            items.push(frenzy_talisman(&next,room.node,0x0f01_0002));
                        }
                        FrenzyPact::BloodMadness=>{}
                        FrenzyPact::FrenzyBrand=>{
                            if let Some(w)=frenzy_legendary_weapon(&next,room.node,0x0f03_0001) {items.push(w);}
                        }
                    }
                    next.gold=data.rune_count;
                    let bonus=next.resolve_frenzy_altar(pact,ember_pick,&items)?;
                    for &item in &items {super::items::give_reward(data,item)?;}
                    if bonus>0 {data.rune_count=next.gold;state.view.runes=next.gold;}
                    if pact==FrenzyPact::BurnFlask && let Some(flasks)=state.flasks.as_mut() {
                        flasks.set_bonus(data,next.class,next.flask_bonus,next.total_flask_cut())?;
                        flasks.restore(data,next.flasks,next.flask_blue)?;
                        flasks.stamp(&mut next);
                        next.flasks=next.flasks.min(next.flask_limit);
                        next.clamp_flask_blue();
                    }
                    if pact==FrenzyPact::BloodMadness {
                        let vitals=&mut player.chr_ins.modules.data;
                        vitals.hp=(vitals.hp/5).max(1);
                        next.hp=normalized_hp(vitals.hp,vitals.max_hp);
                    }
                    record_checked(&next)?;state.view.run=Some(next);
                    room.view.message=match pact {
                        FrenzyPact::BurnFlask=>"癫火祭坛：已燃尽 1 瓶圣杯上限，获赐传说武器与高阶护符！请从传送门离开。".into(),
                        FrenzyPact::BloodMadness=>format!("癫火祭坛：献血成狂！获得余烬{}与 {bonus} 卢恩。请从传送门离开。",ember_pick.map(|e|format!("「{}」",e.name())).unwrap_or_default()),
                        FrenzyPact::FrenzyBrand=>format!("癫火祭坛：已烙下癫火印记（下场战斗易伤 +35%），获赐传说武器与 {bonus} 卢恩！请从传送门离开。"),
                    };
                    super::log(format!("FRENZY_ALTAR pact={pact:?} ember={ember_pick:?} bonus={bonus} items={items:?}"));
                    return Ok(true);
                }
                ui::Request::SurvivalStart{..}=>{
                    next.begin_survival()?;
                    record_checked(&next)?;
                    room.suspend_dialogues()?;
                    state.event_start=Some(Instant::now());
                    state.survival_engaged=None;
                    if let Some(link)=state.party.as_mut() {
                        link.event.request=Some(erdtree_tower::party_events::Kind::Guards(0));
                    }
                    state.view.run=Some(next);
                    set_flag(45010942,true)?;set_flag(45010995,false)?;
                    room.view.message="「坚持 30 秒」大混战已开启，准备迎战！".into();
                    super::log(format!("SURVIVAL_START node={} party={}",room.node,state.party.is_some()));
                    return Ok(true);
                }
                ui::Request::SurvivalSkip{..}=>{
                    next.skip_survival()?;
                    record_checked(&next)?;
                    state.view.run=Some(next);
                    room.view.message="已放弃本次生存挑战，可从传送门离开。".into();
                    return Ok(true);
                }
            }
        }
    }
    if draw==Draw::Survival && run.regional_event.guards {
        if let Some(link)=state.party.as_mut() {
            if link.event.guard_cleared(0) {
                if matches!(link.event.request,Some(erdtree_tower::party_events::Kind::Guards(_))){link.event.request=None;}
                let mut next=run.clone();
                next.regional_event.guards=false;next.regional_event.opened=3;
                record_checked(&next)?;
                set_flag(45010942,false)?;set_flag(45010995,true)?;
                ui::set_survival_hud(None);
                room.view.message="生存大混战已结束，请从传送门离开。".into();
                state.view.run=Some(next);
                return Ok(true);
            }
            link.event.request=Some(erdtree_tower::party_events::Kind::Guards(0));
            set_flag(45010942,true)?;set_flag(45010995,false)?;
            room.view.message="正在同步全队开启「坚持 30 秒」大混战……".into();
            return Ok(true);
        }
        // 单人生存大混战
        if state.pending.is_none() {
            let at=state.event_start.get_or_insert_with(Instant::now);
            if at.elapsed()<super::travel::PREPARATION {
                let prep=super::travel::PREPARATION.saturating_sub(at.elapsed()).as_secs_f64().ceil() as u64;
                ui::set_survival_hud(Some(ui::SurvivalHud {
                    remaining_secs:erdtree_tower::new_events::SURVIVAL_SECONDS as f32,
                    kills:0,total:erdtree_tower::new_events::SURVIVAL_COUNT as u8,preparing_secs:Some(prep),
                }));
                room.view.message=format!("「坚持 30 秒」大混战将在 {prep} 秒后开打！坚持 30 秒或全清 8 只敌人即可领奖。");
                return Ok(true);
            }
            let roster=super::enemies::survival_roster(run.seed,room.node,run.nodes[room.node].layer*11/(run.floors()-1),run.act);
            let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"怪物参数未加载")?;
            if roster.iter().any(|e|params.get::<NpcParam>(e.npc as u32).is_none()||params.get::<NpcThinkParam>(e.think).is_none()) {return Err("生存挑战敌人参数缺失".into());}
            state.pending=Some(Pending {
                entity_base:None,networked:false,all_at_once:true,
                spirits:Default::default(),foes:roster.into_iter().map(|spec|Foe{spec,fight:Encounter::default(),requested:None,entity_address:None,respawn:None,spawn_gap:Default::default(),grace:None,graced:false,spawn_protection:Default::default(),spawn_check:Default::default()}).collect(),
                next_run:run.clone(),block:room.block,completed:false,stall:None,godskin_pace:Default::default(),
            });
            state.survival_engaged=None;
        }
        let pending=state.pending.as_mut().unwrap();
        let (outcome,hp,bound)=poll_group(pending,world,false,room.block,true);
        state.view.enemy_hp=hp;
        if bound&&!run.law.lone_wolf() {pending.spirits.poll(world,room.block,true)?;}
        let total=pending.foes.len().min(255) as u8;
        let kills=pending.foes.iter().filter(|f| {
            f.fight.handle().is_some_and(|h| {
                world.chr_ins_by_handle(&h)
                    .filter(|c|f.entity_address==Some(std::ptr::from_ref(*c) as usize))
                    .is_none_or(|chr|chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag())
            })
        }).count().min(255) as u8;
        if bound {
            let started=state.survival_engaged.get_or_insert_with(Instant::now);
            let rem=(erdtree_tower::new_events::SURVIVAL_SECONDS as f32-started.elapsed().as_secs_f32()).max(0.0);
            ui::set_survival_hud(Some(ui::SurvivalHud {remaining_secs:rem,kills,total,preparing_secs:None}));
        } else {
            ui::set_survival_hud(Some(ui::SurvivalHud {
                remaining_secs:erdtree_tower::new_events::SURVIVAL_SECONDS as f32,
                kills:0,total,preparing_secs:Some(1),
            }));
        }
        let survived=state.survival_engaged.is_some_and(|t|t.elapsed()>=Duration::from_secs(erdtree_tower::new_events::SURVIVAL_SECONDS));
        if matches!(outcome,Outcome::Victory)||survived {
            if !matches!(outcome,Outcome::Victory) {retire_group(pending,world);}
            let healed=heal_victory(world,run);
            if !healed.is_empty(){super::log(&healed);state.view.status=healed;state.view.prompt=state.view.prompt.wrapping_add(1);}
            if let Some(plan)=keep_summoned_ash(world,run) {state.ash_refunds=plan;state.ash_guard=Some(Instant::now());}
            world.summon_buddy_manager.disappear_requested=true;
            state.pending=None;state.event_start=None;state.survival_engaged=None;state.view.enemy_hp=None;
            ui::set_survival_hud(None);
            let mut next=state.view.run.clone().unwrap();
            let (reward,names)=grant_survival_rewards(world,&mut next,room.node,kills)?;
            state.view.runes=next.gold;
            record_checked(&next)?;state.view.run=Some(next);
            set_flag(45010942,false)?;set_flag(45010995,true)?;
            let full=erdtree_tower::new_events::survival_full_clear(kills);
            let loot_desc=if names.is_empty(){String::new()}else{format!("，战利品：{}",names.join("、"))};
            let msg=format!("生存大混战胜利（击杀 {kills}/{total}，{}）：获得 {reward} 卢恩{loot_desc}。请从传送门离开。",if full{"全清额外奖励"}else{"成功生还"});
            super::log(format!("SURVIVAL_SOLO victory kills={kills}/{total} full={full} runes={reward} items={names:?}"));
            room.view.message=msg.clone();
            report(state,msg);
        } else if matches!(outcome,Outcome::SpawnTimedOut|Outcome::LostEntity) {
            ui::set_survival_hud(None);state.survival_engaged=None;
            return Err("生存挑战敌人加载异常，请退出重进。".into());
        } else {
            room.view.message=format!("「坚持 30 秒」大混战进行中：已击杀 {kills}/{total}，倒计时归零自动撤退并领奖！");
        }
        return Ok(true);
    }
    if run.regional_event.opened==0 {
        room.view.message=match draw {
            Draw::EmberFurnace=>"余烬熔炉：与守炉人交谈，免费重铸多余余烬或灰烬拾遗（2000 卢恩）。",
            Draw::SpiritFurnace=>"骨灰融合炉：与引导者交谈，免费吞噬副骨灰（主骨灰 +3）或双灰融魂合成高阶骨灰（1000 卢恩）；单灰/空手也可强化或拾遗。",
            Draw::FrenzyAltar=>"癫火祭坛：与引导者交谈，以圣杯上限、扣除 80% 生命或下战易伤换取传说装备、高阶护符与余烬；也可拒绝离开。",
            Draw::Survival=>if state.party.is_some(){"「坚持 30 秒」大混战：房主与引导者交谈开启全队 30 秒混战（8 只敌人），生还即可领奖，全清额外掉落高阶护符！"}else{"「坚持 30 秒」大混战：与引导者交谈开启 30 秒生存混战（8 只敌人），生还即可领奖，全清额外掉落高阶护符！"},
            _=>"",
        }.into();
    }
    Ok(true)
}
/// 赌运事件没有进行时的对话框：按失败原因选一句，其余归为“暂时无法进行”
fn gamble_refusal(error:&str)->i32 {
    use super::native_talk::{GAMBLE_FAILED,GAMBLE_LEGENDARY,GAMBLE_NO_AFFIX,GAMBLE_NO_RUNES,GAMBLE_NO_SIBLING,GAMBLE_USED};
    if error.contains("卢恩不足") {GAMBLE_NO_RUNES}
    else if error.contains("不能精炼") {GAMBLE_LEGENDARY}
    else if error.contains("没有词条")||error.contains("装备词条武器") {GAMBLE_NO_AFFIX}
    else if error.contains("没有其他词条组合") {GAMBLE_NO_SIBLING}
    else if error.contains("已经用过") {GAMBLE_USED}
    else {GAMBLE_FAILED}
}
fn tick_regional_event(state:&mut Runtime,world:&mut WorldChrMan,room:&mut super::rooms::Room)->Result<(),String> {
    use erdtree_tower::regional_events::Outcome as EventOutcome;
    use super::native_talk::{flag,set_flag};
    let run=state.view.run.as_ref().ok_or("旅程未绑定")?;
    if run.regional_event.settled {
        set_flag(45010965,false)?;set_flag(45010960,false)?;
        set_flag(45010941,true)?;set_flag(45010940,true)?;
        set_flag(45010942,false)?;set_flag(45010995,true)?;
        room.view.message="徘徊赐福：这个事件已经做过选择或领过奖，不能重选、不能再领，请从传送门离开。".into();
        return Ok(());
    }
    if room.mausoleum {
        let mut next=run.clone();
        let copy=next.regional_event.mausoleum.as_mut().ok_or("灵庙进度缺失")?;
        let (lowered,opened)=super::mausoleum::flow()?;
        if copy.observe_flow(lowered,opened) {
            super::log(format!("MAUSOLEUM_FLOW lowered={} opened={}",copy.lowered,copy.opened));
            record_checked(&next)?;state.view.run=Some(next.clone());
        }
        let copy=next.regional_event.mausoleum.as_mut().unwrap();
        if !copy.lowered||!copy.opened{return Ok(());}
        let data=unsafe {world.main_player.as_mut().ok_or("角色未加载")?.player_game_data.as_mut()};
        if let Some((_,choice))=room.hub_choice() {
            if choice==2&&copy.receipt.is_none(){set_flag(room.base()+994,true)?;return Ok(());}
            if choice==1 {
                let weapon=super::mausoleum::equipped(data)?;
                let owned=super::mausoleum::count(&super::mausoleum::inventory(data)?,weapon);
                copy.begin(weapon,owned)?;record_checked(&next)?;
                state.view.run=Some(next.clone());room.copy_requested=false;
            }
        }
        let copy=next.regional_event.mausoleum.as_mut().unwrap();
        if let Some(receipt)=copy.receipt.clone() {
            let owned=super::mausoleum::count(&super::mausoleum::inventory(data)?,receipt.weapon);
            if copy.confirm(owned) {record_checked(&next)?;state.view.run=Some(next.clone());room.notice(9500163)?;}
            else if !room.copy_requested {super::mausoleum::request_copy(data,&receipt)?;room.copy_requested=true;}
        }
        room.view.message="灵庙：在右手装备想复制的武器，与原祭台交互。每人可免费复制一次，原武器保留。".into();
        return Ok(());
    }
    if run.regional_event.trial {
        clear_chariot()?;
        set_flag(45010965,false)?;set_flag(45010960,false)?;
        set_flag(45010941,true)?;set_flag(45010940,true)?;
        set_flag(45010942,false)?;set_flag(45010995,false)?;
        room.view.message=if run.regional_event.mausoleum.is_some(){"灵庙引路人：前往龙王平台的灵庙复制武器，或领取 800 卢恩离开。联机提交全队投票。"}else{"试炼岔路：跳跳乐、跑城、泥头车、隐形道路，或领取 800 卢恩离开。隐形道路前往异端魔法师塔；联机提交全队投票。"}.into();
        return Ok(());
    }
    if run.regional_event.stormveil.is_some() {
        if run.regional_event.chariot&&run.regional_event.stormveil==Some(erdtree_tower::stormveil::Progress::Choosing){clear_chariot()?;}
        set_flag(45010965,false)?;set_flag(45010960,false)?;
        set_flag(45010941,true)?;set_flag(45010940,true)?;
        set_flag(45010942,false)?;set_flag(45010995,false)?;
        room.view.message=if run.regional_event.chariot {
            "泥头车生存：全队进入竞技场，5 秒后出现战车，坚持 30 秒后领取传说宝箱；也可各领 800 卢恩跳过。"
        }else{"史东薇尔突围：抵达葛瑞克门前获得两个随机护符增益，或领取 800 卢恩离开。"}.into();
        return Ok(());
    }
    if run.regional_event.descent {
        set_flag(45010965,false)?;set_flag(45010960,false)?;
        set_flag(45010941,true)?;set_flag(45010940,true)?;
        set_flag(45010942,false)?;set_flag(45010995,true)?;
        room.view.message="是男人就下一百层".into();return Ok(());
    }
    if run.event_draw(room.node)==erdtree_tower::events::Draw::Dance {
        set_flag(45010965,false)?;set_flag(45010960,false)?;
        set_flag(45010941,true)?;set_flag(45010940,true)?;
        let party_exit=state.party.as_ref().is_none_or(|p|!p.event.dance_preparing||p.event.dance_complete);
        set_flag(45010942,false)?;set_flag(45010995,!super::dance_ui::active()&&party_exit)?;
        let key=(run.seed,run.act,room.node);
        if let Some(link)=state.party.as_mut(){
            let report=link.event.dance.get_or_insert_with(Default::default);
            if link.event.dance_preparing{let (fingerprint,failed)=super::dance_ui::prepared();report.fingerprint=fingerprint.filter(|_|world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.physics.standing_on_solid_ground));report.failed=failed;}
            if let Some((song,players))=link.event.dance_prepare.clone(){
                let slot=players.iter().position(|p|*p==link.config.player).ok_or("舞台名单缺少本机玩家")?;
                super::dance_stage::place(world,slot,players.len())?;
                let mut next=run.clone();next.begin_dance(song as usize)?;record_checked(&next)?;
                link.event.dance_prepare=None;link.event.dance=Some(Default::default());room.suspend_dialogues()?;
                state.view.run=Some(next);super::dance_ui::request_party_event(key,song as usize);
                room.view.message="舞会：等待全员加载音乐，随后统一倒计时。".into();return Ok(());
            }
        }
        if let Some((result_key,grade))=super::dance_ui::take_result()&&result_key==key {
            let mut next=run.clone();
            let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
            next.gold=unsafe {player.player_game_data.as_ref()}.rune_count;
            let reward=next.finish_dance(if player.chr_ins.modules.data.hp>0{grade}else{None})?;
            record_checked(&next)?;
            unsafe {player.player_game_data.as_mut()}.rune_count=next.gold;state.view.runes=next.gold;
            state.view.run=Some(next);
            if let Some(link)=state.party.as_mut(){link.event.dance.get_or_insert_with(Default::default).finished=true;}
            room.view.message=format!("舞会结束，获得 {reward} 卢恩。请从传送门离开。");
            super::log(format!("DANCE reward key={key:?} runes={reward}"));
            return Ok(());
        }
        if let Some(super::native_talk::EventRequest::Dance(song))=room.take_event_request() {
            super::log(format!("DANCE_SELECT song={song} party={} host={} key={key:?}",state.party.is_some(),state.party.as_ref().is_some_and(|p|p.host())));
            if let Some(link)=state.party.as_mut(){
                if link.host()&&!link.event.dance_preparing{link.event.dance.get_or_insert_with(Default::default).choice=Some(song as u8);}
                room.view.message=if link.host(){"正在等待全员到达舞台…"}else{"联机舞会由房主选歌，请等待统一开场。"}.into();return Ok(());
            }
            super::dance_stage::place(world,0,1)?;
            let mut next=run.clone();next.begin_dance(song)?;record_checked(&next)?;
            room.suspend_dialogues()?;state.view.run=Some(next);super::dance_ui::request_event(key,song);
            room.view.message="舞会：跟随箭头与节拍完成歌曲。中途退出不发奖励。".into();
        } else if state.party.as_ref().is_some_and(|p|p.event.dance_cancelled) {
            room.view.message="联机舞会已取消：队员离场、音乐或动画未就绪，或准备超时。请保留各自日志；可从传送门离开。".into();
        } else if state.party.as_ref().is_some_and(|p|p.event.dance.is_some_and(|r|r.choice.is_some())&&!p.event.dance_preparing) {
            room.view.message="歌曲已选定，等待全员进入舞会房间并站稳。".into();
        } else if run.regional_event.opened==0 {
            room.view.message=if state.party.is_some(){"联机舞会由房主与引导者交谈选歌，全员自动进入。S / A / B / C：5000 / 3500 / 2000 / 1000 卢恩。"}else{"舞会：与引导者交谈选择歌曲。S / A / B / C：5000 / 3500 / 2000 / 1000 卢恩。也可直接离开。"}.into();
        } else if !super::dance_ui::active(){room.view.message=if party_exit{"本次舞会已参加，请从传送门离开。"}else{"本机已完成，等待队友结束舞会。"}.into();}
        return Ok(());
    }
    if tick_custom_event(state,world,room)? {return Ok(());}
    let run=state.view.run.as_ref().ok_or("旅程未绑定")?;
    if run.regional_event.recovery {
        set_flag(45010965,false)?;set_flag(45010960,false)?;
        set_flag(45010941,true)?;set_flag(45010940,true)?;
        set_flag(45010942,false)?;set_flag(45010995,true)?;
        room.view.message="月光泉：回满生命、回满专注或领取 800 卢恩，三选一。".into();
        if let Some(super::native_talk::EventRequest::Recovery(choice))=room.take_event_request() {
            let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
            if player.chr_ins.modules.data.hp<=0 {return Err("角色已死亡".into());}
            let mut next=run.clone();
            if choice==2 {next.gold=unsafe {player.player_game_data.as_ref()}.rune_count;}
            next.recover_at_spring(choice)?;record_checked(&next)?;
            if choice==2 {unsafe {player.player_game_data.as_mut()}.rune_count=next.gold;state.view.runes=next.gold;}
            let stats=&mut player.chr_ins.modules.data;
            if choice==0 {stats.hp=stats.max_hp;}else if choice==1{stats.fp=stats.max_fp;}
            state.view.run=Some(next);
            room.view.message=match choice {0=>"生命已回满",1=>"专注已回满",_=>"已领取 800 卢恩"}.into();
        }
        return Ok(());
    }
    // 赌运事件（单人）：与 NPC 交谈打开原版对话菜单，选项确认后结算并弹出结果，每处一次；联机时这一格按种子改为三重宝匣或贪欲祭坛
    if let Some(index)=run.gamble(room.node).filter(|_|state.party.is_none()) {
        set_flag(45010965,false)?;set_flag(45010960,false)?;
        set_flag(45010941,true)?;set_flag(45010940,true)?;
        set_flag(45010942,false)?;set_flag(45010995,true)?;
        if index==erdtree_tower::events::DICE&&(run.regional_event.patches_angry||run.regional_event.patches_dead) {
            room.suspend_dialogues()?;let _=room.take_event_request();
            room.view.message=if run.regional_event.patches_dead {"帕琪已死亡，使用传送门继续旅程。"}else{"帕琪被激怒了，不再接受赌博。可以战斗或使用传送门离开。"}.into();
            return Ok(());
        }
        let title=erdtree_tower::events::STORIES[index].title;
        let wins=erdtree_tower::events::dice_wins(run,room.node);
        if index==erdtree_tower::events::FURNACE {
            room.view.message=format!("{title}：与守炉人交谈，重练右手的词条武器。这一次 {} 卢恩（已重练 {} 次，每次翻倍）。",erdtree_tower::events::furnace_price(run.regional_event.reforges),run.regional_event.reforges);
        }else if run.regional_event.opened==0 {room.view.message=format!("{title}：与 NPC 交谈。");}
        if let Some(super::native_talk::EventRequest::Gamble(choice))=room.take_event_request() {
            let message=state.view.run.as_ref().map(|r|erdtree_tower::events::story_outcome(r,choice)).unwrap_or_default();
            match transact(state,world,Action::Story(choice),false,false) {
                Ok(())=>{room.notice(gamble_result(index,choice,wins))?;
                    room.view.message=if index==erdtree_tower::events::FURNACE {
                        let uses=state.view.run.as_ref().map_or(0,|r|r.regional_event.reforges);
                        format!("{message}可以继续重练（下一次 {} 卢恩），或使用传送门继续旅程。",erdtree_tower::events::furnace_price(uses))
                    }else{format!("{message}使用传送门继续旅程。")};},
                Err(error)=>{super::log(format!("Gamble not resolved: {error}"));room.notice(gamble_refusal(&error))?;room.view.message=error;},
            }
        }
        return Ok(());
    }
    let locked=run.regional_event.locked;
    set_flag(45010965,run.regional_event.scarabs)?;
    if run.regional_event.scarabs {return tick_scarabs(state,world,room);}
    let altar=!locked&&run.altar(room.node);
    set_flag(45010960,locked)?;
    set_flag(45010941,altar)?;
    set_flag(45010940,true)?;
    if locked {return tick_locked_chest(state,world,room);}
    if let Some(link)=state.party.as_mut() {
        if run.regional_event.guards && run.regional_event.guard_slot.is_some_and(|s|link.event.guard_cleared(s)) {
            if matches!(link.event.request,Some(erdtree_tower::party_events::Kind::Guards(_))){link.event.request=None;}
            let mut next=run.clone();next.regional_event.guards=false;next.regional_event.guard_slot=None;
            record_checked(&next)?;
            set_flag(45010942,false)?;set_flag(45010995,true)?;
            room.view.message="这个宝箱的守卫已被队伍击败，可以继续探索或离开。".into();
            state.view.run=Some(next);
            return Ok(());
        }
        if run.regional_event.guards {
            let slot=run.regional_event.guard_slot.ok_or("旧事件缺少守卫箱位，请重新开始旅程")?;
            link.event.request=Some(erdtree_tower::party_events::Kind::Guards(slot));
            set_flag(45010942,true)?;set_flag(45010995,false)?;
            return Ok(());
        }
    }
    set_flag(45010942,run.regional_event.guards)?;
    for slot in 0..3 {set_flag(45010950+slot,run.regional_event.opened&(1<<slot)!=0)?;}
    // The room's normal chest-exit condition is reused only when no guards remain.
    set_flag(45010995,!run.regional_event.guards)?;
    if run.regional_event.guards {
        for request in 45010953..=45010958 {set_flag(request,false)?;}
        if state.pending.is_none() {
            let at=state.event_start.get_or_insert_with(Instant::now);
            if at.elapsed()<super::travel::PREPARATION {
                room.view.message=format!("守卫将在 {} 秒后出现，击败后才能离开。",super::travel::PREPARATION.saturating_sub(at.elapsed()).as_secs_f64().ceil() as u64);
                return Ok(());
            }
            let slot=run.regional_event.guard_slot.unwrap_or(0);
            let roster=super::enemies::event_roster(run.seed,room.node,run.nodes[room.node].layer*11/(run.floors()-1),run.act,altar,slot);
            let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"怪物参数未加载")?;
            if roster.iter().any(|e|params.get::<NpcParam>(e.npc as u32).is_none()||params.get::<NpcThinkParam>(e.think).is_none()) {return Err("事件守卫参数缺失".into());}
            state.pending=Some(Pending {
                entity_base:None,networked:false,all_at_once:false,
                spirits:Default::default(),foes:roster.into_iter().map(|spec|Foe{spec,fight:Encounter::default(),requested:None,entity_address:None,respawn:None,spawn_gap:Default::default(),grace:None,graced:false,spawn_protection:Default::default(),spawn_check:Default::default()}).collect(),
                next_run:run.clone(),block:room.block,completed:false,stall:None,godskin_pace:Default::default(),
            });
        }
        let pending=state.pending.as_mut().unwrap();
        let (outcome,hp,bound)=poll_group(pending,world,false,room.block,true);
        state.view.enemy_hp=hp;
        if bound&&!run.law.lone_wolf() {pending.spirits.poll(world,room.block,true)?;}
        match outcome {
            Outcome::Victory => {
                let healed=heal_victory(world,run);if !healed.is_empty(){super::log(&healed);state.view.status=healed;state.view.prompt=state.view.prompt.wrapping_add(1);}
                if let Some(plan)=keep_summoned_ash(world,run) {state.ash_refunds=plan;state.ash_guard=Some(Instant::now());}
                world.summon_buddy_manager.disappear_requested=true;
                super::log("ASH_GROUP event guard victory: native summon dismissal requested");
                state.pending=None;state.event_start=None;state.view.enemy_hp=None;
                let run=state.view.run.as_mut().unwrap();run.regional_event.guards=false;run.regional_event.guard_slot=None;record(run);
                set_flag(45010942,false)?;set_flag(45010995,true)?;
                room.view.message="守卫已击败，可以继续探索或使用传送门离开。".into();
            }
            Outcome::SpawnTimedOut|Outcome::LostEntity => return Err("事件守卫加载异常，请退出重进；已开宝箱和奖励不会重置。".into()),
            _=>room.view.message="击败事件守卫后，传送门才会重新开放。".into(),
        }
        return Ok(());
    }
    if room.view.message.starts_with("靠近") {
        room.view.message=if altar {"贪欲祭坛：献血、挑战守卫或支付 800 卢恩，三选一。"}else{"三重宝匣：三个箱子均可开启，每箱 20% 爆炸、20% 守卫、60% 物品。爆炸损失最大生命的 30%，可能致死；随时可离开，战斗中除外。"}.into();
    }
    let requests:Vec<_>=if altar {
        match room.take_event_request(){Some(super::native_talk::EventRequest::Altar(slot))=>vec![slot],_=>vec![]}
    }else{
        let mut slots=vec![];
        for slot in 0..3 {let request=45010953+slot;if flag(request)? {slots.push(slot as usize);}}
        slots
    };
    for slot in requests {
        if !altar {set_flag(45010953+slot as u32,false)?;}
        let mut next=state.view.run.clone().unwrap();
        let outcome=next.open_regional_event(slot)?;
        let player=world.main_player.as_mut().ok_or("角色未加载")?;
        if altar && slot==2 && unsafe {player.player_game_data.as_ref()}.rune_count<800 {
            room.notice(9500057)?;
            return Ok(());
        }
        if outcome==EventOutcome::Item || altar {
            let loot=super::loot::regional_reward(unsafe {player.player_game_data.as_mut()},&next,next.seed,room.node*4+slot)?;
            room.view.message=format!("获得：{}。",loot.name);
        }
        if altar && slot==2 {
            let data=unsafe {player.player_game_data.as_mut()};
            data.rune_count-=800;
            next.gold=data.rune_count;
            state.view.runes=data.rune_count;
        }
        let in_blast=super::loot::in_chest_blast(slot,[player.block_position.x,player.block_position.y,player.block_position.z]);
        let vitals=&mut player.chr_ins.modules.data;
        if altar && slot==0 {
            vitals.hp=(vitals.hp-(vitals.hp*30+99)/100).max(1);
        } else if outcome==EventOutcome::Explosion {
            // 联机时爆炸不致死（至少留 1 点）：房间里没有战斗胜利，倒下的人等不到队友救援，全队会卡在这个节点
            if in_blast {vitals.hp=(vitals.hp-(vitals.max_hp*30+99)/100).max(i32::from(state.party.is_some()));}
            set_flag(45010943+slot as u32,true)?;
            room.view.message=if in_blast {"箱中火药爆炸！范围 5 米，损失最大生命的 30%。"}else{"箱中火药爆炸！你已离开 5 米爆炸范围。"}.into();
        } else if outcome==EventOutcome::Guards {
            room.view.message="宝箱惊醒了守卫，准备迎战！".into();
        }
        regional_event_hp(&mut next,vitals.hp,vitals.max_hp,state.party.is_some());
        if next.regional_event.guards {state.event_start=Some(Instant::now());}
        set_flag(45010942,next.regional_event.guards)?;set_flag(45010995,!next.regional_event.guards)?;
        record(&next);state.view.run=Some(next);
        break;
    }
    Ok(())
}

fn tick_scarabs(state:&mut Runtime,world:&mut WorldChrMan,room:&mut super::rooms::Room)->Result<(),String> {
    use super::native_talk::set_flag;
    if let Some(link)=state.party.as_mut() {
        let run=state.view.run.as_mut().ok_or("个人事件尚未加载")?;
        let waiting=link.event.pending();
        if matches!(room.take_event_request(),Some(super::native_talk::EventRequest::Scarabs))&&!waiting {
            let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
            let data=unsafe {player.player_game_data.as_mut()};
            if data.rune_count<erdtree_tower::regional_events::SCARAB_PRICE {room.notice(9500084)?;return Ok(());}
            let mut next=run.clone();let remaining=next.pay_scarabs(data.rune_count)?;
            record_checked(&next)?;
            data.rune_count=remaining;state.view.runes=remaining;
            link.event.request=Some(erdtree_tower::party_events::Kind::Scarabs(next.regional_event.attempts));*run=next;
        }
        let busy=waiting||link.event.request.is_some();
        set_flag(45010960,false)?;set_flag(45010941,true)?;set_flag(45010940,true)?;
        set_flag(45010942,busy)?;set_flag(45010995,!busy)?;
        set_flag(45010969,run.regional_event.attempts>=3)?;
        set_flag(45010966,!busy&&run.regional_event.attempts<3)?;
        return Ok(());
    }
    set_flag(45010960,false)?;set_flag(45010941,true)?;set_flag(45010940,true)?;
    set_flag(45010942,room.scarabs.active())?;set_flag(45010995,!room.scarabs.active())?;
    let attempts=state.view.run.as_ref().ok_or("旅程未绑定")?.regional_event.attempts;
    set_flag(45010969,attempts>=3)?;
    if !room.scarabs.active() && attempts<3 {room.scarabs.prepare(world)?;}
    if matches!(room.take_event_request(),Some(super::native_talk::EventRequest::Scarabs)) {
        if room.scarabs.ready() && attempts<3 {
            let player=world.main_player.as_mut().ok_or("角色未加载")?;
            let data=unsafe {player.player_game_data.as_mut()};
            if data.rune_count<erdtree_tower::regional_events::SCARAB_PRICE {room.notice(9500084)?;return Ok(());}
            let mut next=state.view.run.clone().unwrap();
            let remaining=next.pay_scarabs(data.rune_count)?;
            record_checked(&next)?;
            room.scarabs.start()?;
            data.rune_count=remaining;state.view.runes=remaining;state.view.run=Some(next);
            super::log(format!("Scarab hunt paid: attempt={}, runes={remaining}",attempts+1));
        }
    }
    if let Err(error)=room.scarabs.poll(world) {
        room.scarabs.retire(world);room.scarabs=Default::default();
        let run=state.view.run.as_mut().unwrap();
        if let Some(player)=world.main_player.as_mut() {
            let data=unsafe {player.player_game_data.as_mut()};
            data.rune_count=data.rune_count.saturating_add(erdtree_tower::regional_events::SCARAB_PRICE);run.gold=data.rune_count;state.view.runes=data.rune_count;
        }
        run.regional_event.attempts=run.regional_event.attempts.saturating_sub(1);record(run);
        set_flag(45010942,false)?;set_flag(45010995,true)?;set_flag(45010966,false)?;
        return Err(format!("{error}，已退回本次卢恩和次数"));
    }
    for slot in room.scarabs.rewards() {
        let run=state.view.run.as_ref().unwrap();
        let player=world.main_player.as_mut().ok_or("角色未加载")?;
        let item=super::loot::regional_reward(unsafe {player.player_game_data.as_mut()},run,run.seed ^ 0x5ca4ab,room.node*16+usize::from(run.regional_event.attempts)*3+slot)?;
        room.scarabs.rewarded(slot);
        super::log(format!("Scarab hunt reward: slot={slot}, item={}",item.name));
    }
    if room.scarabs.done() {
        // 召唤师「群灵」：狩猎结束时召唤物还活着，补回这次用掉的骨灰
        if let Some(run)=state.view.run.as_ref()&&let Some(plan)=keep_summoned_ash(world,run) {state.ash_refunds=plan;state.ash_guard=Some(Instant::now());}
        world.summon_buddy_manager.disappear_requested=true;
        room.scarabs=Default::default();
    }
    let active=room.scarabs.active();
    set_flag(45010942,active)?;set_flag(45010995,!active)?;
    set_flag(45010966,room.scarabs.ready())?;
    room.view.message=if active {"捕捉粪金龟".into()} else {format!("粪金龟游戏 · 已玩 {}/3 次",state.view.run.as_ref().unwrap().regional_event.attempts)};
    Ok(())
}

fn tick_locked_chest(state:&mut Runtime,world:&mut WorldChrMan,room:&mut super::rooms::Room)->Result<(),String> {
    use super::{loot,native_talk::{flag,set_flag}};
    let run=state.view.run.as_ref().ok_or("旅程未绑定")?;
    set_flag(45010942,false)?;
    set_flag(45010963,run.regional_event.opened!=0)?;
    set_flag(45010995,run.regional_event.receipt.is_none())?;
    let requested=flag(45010961)?;
    set_flag(45010961,false)?;
    if run.regional_event.opened!=0 {return Ok(());}
    let player=world.main_player.as_mut().ok_or("角色未加载")?;
    let data=unsafe {player.player_game_data.as_mut()};
    if run.regional_event.receipt.is_none() {
        if !requested {return Ok(());}
        let keys=loot::count(data,0x40001f40);
        if keys==0 {room.notice(9500072)?;return Ok(());}
        let item=loot::locked_reward(data,run,room.node)?;
        let mut next=run.clone();next.begin_locked(item,keys,loot::count(data,item))?;
        record_checked(&next)?;state.view.run=Some(next);
        set_flag(45010995,false)?;
    }
    let receipt=state.view.run.as_ref().unwrap().regional_event.receipt.clone().unwrap();
    if loot::count(data,0x40001f40)>receipt.key_target {
        room.consume_key()?;
        if loot::count(data,0x40001f40)>receipt.key_target {return Err("钥匙尚未扣除，请重试".into());}
    }
    if loot::count(data,receipt.item)<receipt.reward_target {
        if receipt.item==0x40002724 {super::items::give_stack(data,10020)?;}
        else {super::items::give_reward(data,receipt.item)?;}
        if loot::count(data,receipt.item)<receipt.reward_target {return Err("奖励尚未进入背包，请清理容量后重试".into());}
    }
    let mut next=state.view.run.as_ref().unwrap().clone();
    next.complete_locked()?;record_checked(&next)?;state.view.run=Some(next);
    set_flag(45010962,true)?;set_flag(45010963,true)?;set_flag(45010995,true)?;
    room.view.message="宝箱已开启".into();
    super::log(format!("Locked chest {} granted item {}",room.node,receipt.item));
    Ok(())
}

fn resource_result(
    hp: i32,
    max_hp: i32,
    fp: i32,
    max_fp: i32,
    effect: &erdtree_tower::events::StoryEffect,
) -> Result<(i32, i32), String> {
    let cost = (i64::from(max_hp) * i64::from(effect.hp_cost_percent) + 99) / 100;
    if hp <= 0 || max_hp <= 0 || max_fp < 0 || i64::from(hp) <= cost {
        return Err("实际生命不足，事件不会致死".into());
    }
    if effect.fp_percent > 0 && fp >= max_fp {
        return Err("专注已满，未扣款".into());
    }
    if effect.heal_percent > 0 && hp >= max_hp {
        return Err("生命已满，未扣款".into());
    }
    Ok((
        (i64::from(hp) - cost + i64::from(max_hp) * i64::from(effect.heal_percent) / 100)
            .clamp(1, i64::from(max_hp)) as i32,
        (i64::from(fp) + i64::from(max_fp) * i64::from(effect.fp_percent) / 100)
            .clamp(0, i64::from(max_fp)) as i32,
    ))
}

fn cancel_precombat(view:&mut View) {
    if let Some(run)=view.run.as_mut() && run.preparing_combat.take().is_some(){record(run);}
}
fn request_spawn(state: &mut Runtime, world: &mut WorldChrMan, id: usize, spirits:super::spirits::Spirits) -> Result<(), String> {
    let run = state.view.run.as_ref().ok_or("角色未就绪")?;
    start_allowed(run, id, state.spawn_used)?;
    state.combat_scene=world.main_player.as_ref().map(|p|(run.seed,run.act,i32::from(p.current_block_id),id));
    if let Some(test)=run.boss_test.filter(|t|run.debug&&t.act==run.act&&run.nodes[id].kind==erdtree_tower::NodeKind::Boss) {
        super::enemies::boss_test_roster(test.encounter,state.party.is_some(),erdtree_tower::dlc::enemies_allowed())?;
    }
    if state.party.as_ref().is_some_and(|p|!p.host()) {
        let mut next=run.clone();next.enter(id)?;
        let player=world.main_player.as_mut().ok_or("角色未加载")?;
        let vitals=&mut player.chr_ins.modules.data;vitals.fp=super::travel::combat_fp(run.class,vitals.fp,vitals.max_fp);
        if let Some(link)=state.party.as_ref() {
            state.party_spirits=Some(((link.epoch,0,i32::from(player.current_block_id)),spirits));
        }
        state.view.run=Some(next);state.spawn_used=true;state.view.busy=false;state.view.travelling=false;
        report(state,"等待房主敌人同步");return Ok(());
    }
    let mut roster = super::enemies::node_roster(run,id);
    if state.party.is_none()&&!run.boss_test.is_some_and(|test|test.act==run.act) {if let Some(debug)=super::debug_menu::take_roster() {
        super::log(format!("DEBUG_MENU roster override: {}",debug.iter().map(|e|e.name).collect::<Vec<_>>().join(",")));roster=debug;
        // 斗蛐蛐实验：两只都按 Boss 角色处理，同时出场且保持红方、蓝方的顺序
        if let Some(setup)=super::debug_menu::take_bout() {state.bout.begin(setup);for e in &mut roster {e.kind=erdtree_tower::NodeKind::Boss;}}
    }}
    if !erdtree_tower::dlc::enemies_allowed()&&roster.iter().any(|e|e.dlc){return Err("需要启用 DLC 敌人资源".into());}
    let params = unsafe { SoloParamRepository::instance() }.map_err(|_| "游戏参数未就绪")?;
    for spec in &roster {
        if params.get::<NpcParam>(spec.npc as u32).is_none() || params.get::<NpcThinkParam>(spec.think).is_none() {
            return Err(format!("缺少怪物参数：{}，整组未生成", spec.name));
        }
    }
    let player = world.main_player.as_mut().ok_or("角色未就绪")?;
    if !state.view.ready || player.chr_ins.chr_flags1c5.is_invincible()
        || player.chr_ins.modules.physics.is_falling || !player.chr_ins.modules.physics.standing_on_solid_ground {
        return Err("请等待角色站稳再开始战斗".into());
    }
    if state.party.is_none()&&(world.debug_chr_creator.spawn || std::mem::size_of::<CSDebugChrCreatorInitData>() != 0x100) {
        return Err("角色生成器忙碌或结构不匹配，未生成敌人".into());
    }
    let mut next_run = run.clone();
    next_run.enter(id)?;
    next_run.observe_hp(normalized_hp(player.chr_ins.modules.data.hp, player.chr_ins.modules.data.max_hp))?;
    let vitals = &mut player.chr_ins.modules.data;
    vitals.fp = super::travel::combat_fp(run.class, vitals.fp, vitals.max_fp);
    let description = roster.iter().map(|e| e.name).collect::<Vec<_>>().join("、");
    state.pending = Some(Pending {
        entity_base:None,networked:state.party.is_some(),all_at_once:false,
        spirits,
        foes: roster.into_iter().map(|spec| Foe { spec, fight: Encounter::default(), requested: None, entity_address: None, respawn: None,spawn_gap:Default::default(),grace:None,graced:false,spawn_protection:Default::default(),spawn_check:Default::default() }).collect(),
        next_run, block: i32::from(player.current_block_id), completed: false, stall: None,godskin_pace:Default::default(),
    });
    state.spawn_used = true;
    state.view.travelling = true;
    report(state, format!("正在竞技场中心生成：{description}"));
    Ok(())
}

fn log_map_enemy(world:&WorldChrMan,entity:u32,spec:super::enemies::Enemy) {
    let controls=super::enemies::map_controls(entity).map(|(request,ready)|
        (request,super::native_talk::flag(request),ready,super::native_talk::flag(ready)));
    let mut count=0;
    for chr in world.chr_sets.iter().flatten().flat_map(|set|set.characters()).filter(|c|c.event_entity_id==entity) {
        count+=1;
        super::log(format!("Map enemy diagnostic: entity={entity} handle={} expected={}/{} actual={}/{} hp={}/{} load={:?} dead={} disabled={} controls={controls:?}",
            chr.field_ins_handle,spec.model,spec.npc,chr.npc_id,chr.npc_param_id,chr.modules.data.hp,chr.modules.data.max_hp,
            unsafe {chr.chr_set_entry.as_ref()}.chr_load_status,chr.chr_flags1c5.death_flag(),chr.debug_flags.character_disabled()));
    }
    super::log(format!("Map enemy diagnostic: entity={entity} matching_entities={count} controls={controls:?}"));
}
fn spawn_clock()->Duration {
    static START:std::sync::LazyLock<Instant>=std::sync::LazyLock::new(Instant::now);
    START.elapsed()
}
#[derive(Default)]
struct GodskinPace {
    started:Option<Instant>,
    held:Option<(FieldInsHandle,usize)>,
}
impl GodskinPace {
    fn standby(started:&mut Option<Instant>,both_ready:bool,now:Instant)->Option<usize>{
        if !both_ready{*started=None;return None;}
        Some(1-(now.duration_since(*started.get_or_insert(now)).as_secs()/8%2) as usize)
    }
    fn release(&self,world:&mut WorldChrMan){
        if let Some((handle,address))=self.held&&let Some(chr)=world.chr_ins_by_handle_mut(&handle)
            &&std::ptr::from_ref(&*chr) as usize==address {
            chr.debug_flags.set_disabled_secondary_actions(false);
        }
    }
    fn sync(&mut self,foes:&[Foe],world:&mut WorldChrMan){
        let [a,b]=foes else{return;};
        if [a.spec.npc,b.spec.npc]!=[950000210,950000198]{return;}
        let actors=[a,b].map(|f|{
            let h=f.fight.handle()?;
            let chr=world.chr_ins_by_handle(&h)?;
            let address=std::ptr::from_ref(chr) as usize;
            (f.entity_address==Some(address)&&f.graced&&chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag()).then_some((h,address))
        });
        let target=Self::standby(&mut self.started,actors.iter().all(Option::is_some),Instant::now()).and_then(|i|actors[i]);
        if self.held==target{return;}
        self.release(world);self.held=None;
        if let Some((h,address))=target&&let Some(chr)=world.chr_ins_by_handle_mut(&h)
            &&std::ptr::from_ref(&*chr) as usize==address&&!chr.debug_flags.disabled_secondary_actions(){
            chr.debug_flags.set_disabled_secondary_actions(true);self.held=target;
        }
    }
}
#[test]
fn solo_godskin_pair_trades_attacks_until_one_dies(){
    let now=Instant::now();let mut start=None;
    for (seconds,both,expected) in [(0,false,None),(2,true,Some(1)),(9,true,Some(1)),(10,true,Some(0)),(11,false,None)]{
        assert_eq!(GodskinPace::standby(&mut start,both,now+Duration::from_secs(seconds)),expected);
    }
}
fn poll_group(pending: &mut Pending, world: &mut WorldChrMan, dead: bool, block: i32,authority:bool) -> (Outcome, Option<(i32,i32)>, bool) {
    if dead {
        pending.godskin_pace.release(world);pending.godskin_pace.held=None;
        return (Outcome::PlayerDied,None,false);
    }
    if block != pending.block { return (Outcome::LostEntity, None, false); }
    let mut outcomes = Vec::new();
    let mut hp = 0i32;
    let mut max_hp = 0i32;
    let mut bound = false;
    let count = pending.foes.len();
    // 第三幕双王：两只 Boss 分开站，免得大体型的 Boss 叠在一起
    let dual = count==2&&pending.foes.iter().all(|f|super::enemies::dual(f.spec.kind,f.spec.act));
    let map_ids=if pending.networked&&pending.entity_base.is_none() {
        match super::enemies::map_entities(&pending.foes.iter().map(|f|f.spec).collect::<Vec<_>>()) {
            Ok(ids)=>Some(ids),Err(error)=>{super::log(error);return (Outcome::LostEntity,None,false);}
        }
    }else{None};
    let mut spawning = false;
    let roles:Vec<_>=pending.foes.iter().map(|f|super::enemies::role(&f.spec)).collect();
    let monster_node=matches!(pending.next_run.phase,Phase::Combat{id} if pending.next_run.nodes.get(id).is_some_and(|n|n.kind==erdtree_tower::NodeKind::Monster));
    // 精英和强怪随从共用出场节奏：默认逐只、成双 I 半血、成双 II 同时。
    let elite_node=matches!(pending.next_run.phase,Phase::Combat{id} if pending.next_run.nodes.get(id).is_some_and(|n|n.kind==erdtree_tower::NodeKind::Elite));
    let pacing=if elite_node {pending.next_run.law.elite_pacing()}else{erdtree_tower::encounter::Pacing::Together};
    let mut untouched=Vec::new();
    let now=spawn_clock();
    for i in 0..count {
        let earlier:Vec<_>=(0..i).filter(|&j|super::enemies::paced_combatant(&pending.foes[j].spec)).map(|j|{
            let health=pending.foes[j].fight.handle().and_then(|h|world.chr_ins_by_handle(&h)).map(|c|(c.modules.data.hp,c.modules.data.max_hp));
            (outcomes[j],health)
        }).collect();
        let held=super::enemies::paced_combatant(&pending.foes[i].spec)&&pacing.waits(&earlier);
        let eligible=erdtree_tower::encounter::may_spawn_group(pending.all_at_once,pending.networked,monster_node,held,&roles,i,&outcomes);
        let allowed=pending.foes[i].spawn_gap.allows(now,eligible);
        if pending.networked&&let (Some(ids),Phase::Combat{id})=(&map_ids,&pending.next_run.phase) {
            pacing_trace("host",(pending.next_run.seed,0,pending.next_run.act,*id),i,ids[i],&pending.foes[i].spec,world,
                format!("previous={:?} paced_held={held} eligible={eligible} allowed={allowed} bound={}",&outcomes[..],pending.foes[i].fight.handle().is_some()));
        }
        if !allowed {
            outcomes.push(Outcome::Waiting);
            continue;
        }
        let prior_handles: Vec<_> = pending.foes.iter().filter_map(|f| f.fight.handle()).collect();
        let foe = &mut pending.foes[i];
        if foe.fight.handle().is_none() && let Some(ids)=&map_ids {
            foe.requested.get_or_insert_with(Instant::now);
            if let Err(error)=super::enemies::position_map_entity(ids[i],i){super::log(error);return (Outcome::LostEntity,None,bound);}
            let ready=match super::enemies::request_map_entity(ids[i],true) {
                Ok(ready)=>ready,Err(error)=>{super::log(error);return (Outcome::LostEntity,None,bound);}
            };
            if ready {
                let candidate=world.chr_sets.iter().flatten().flat_map(|set|set.characters()).find(|chr|
                    chr.event_entity_id==ids[i]&&chr.npc_id==foe.spec.model&&chr.npc_param_id==foe.spec.npc
                    &&chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag()&&!chr.debug_flags.character_disabled()
                    &&enemy_loaded(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status))
                    .map(|chr|(chr.field_ins_handle,std::ptr::from_ref(chr) as usize));
                if let Some((handle,address))=candidate && foe.fight.bind(handle,true) {
                    foe.entity_address=Some(address);
                    if authority&&foe.spec.model==4730&&let Some(chr)=world.chr_ins_by_handle_mut(&handle) {chr.apply_speffect(13918,false);}
                    super::log(format!("Map enemy bound: entity={} handle={handle} model={} networked={}",ids[i],foe.spec.model,pending.networked));
                }
            }
        }
        if map_ids.is_none() && foe.fight.handle().is_none() && foe.requested.is_some() {
            if let Some(created) = world.debug_chr_creator.last_created_chr {
                let chr = unsafe { created.as_ref() };
                let handle = chr.field_ins_handle;
                let address = std::ptr::from_ref(chr) as usize;
                if chr.npc_id == foe.spec.model && chr.npc_param_id == foe.spec.npc
                    && chr.modules.data.hp > 0 && !chr.chr_flags1c5.death_flag()
                    && enemy_loaded(unsafe { chr.chr_set_entry.as_ref() }.chr_load_status)
                    && !prior_handles.contains(&handle)
                    && world.chr_ins_by_handle(&handle).is_some_and(|c| std::ptr::from_ref(c) as usize == address)
                    && foe.fight.bind(handle, true)
                {
                    foe.entity_address = Some(address);
                    foe.spawn_check.bound_at = Some(Instant::now());
                    if let Some(chr) = world.chr_ins_by_handle_mut(&handle) {
                        chr.team_type = 6;
                        if authority&&foe.spec.model==4730 {chr.apply_speffect(13918,false);}
                    }
                    world.debug_chr_creator.last_created_chr = None;
                    super::log(format!("Group enemy bound: {}/{} {} handle={handle} networked={}", i+1,count,foe.spec.name,pending.networked));
                }
            }
        }
        if map_ids.is_none() && foe.fight.handle().is_none() && foe.requested.is_none() && !spawning && !world.debug_chr_creator.spawn {
            if let Some(player) = world.main_player.as_ref() {
                let Some(arena)=super::arenas::for_block(block) else {return (Outcome::LostEntity,None,bound);};
                let local = arena.position(if dual {super::enemies::dual_placement(i)}else{super::enemies::placement(i,count)});
                let physics = &player.chr_ins.modules.physics;
                let origin = erdtree_tower::encounter::floating_origin(
                    [physics.position.0,physics.position.1,physics.position.2],
                    [player.block_position.x,player.block_position.y,player.block_position.z],
                );
                let (target_local, pos) = match foe.respawn.take() {
                    Some(at)=>{
                        let world_pos=[at[0],at[1]+0.1,at[2]];
                        let loc=origin.and_then(|o|erdtree_tower::encounter::local_from_world(o,world_pos)).unwrap_or(local);
                        (loc,HavokPosition(world_pos[0],world_pos[1],world_pos[2],0.0))
                    },
                    None=>(local,HavokPosition(physics.position.0-player.block_position.x+local[0],
                        physics.position.1-player.block_position.y+local[1],
                        physics.position.2-player.block_position.z+local[2],0.0)),
                };
                if ![pos.0,pos.1,pos.2].iter().all(|v| v.is_finite()) { return (Outcome::LostEntity,None,bound); }
                let yaw = super::enemies::facing(local,[player.block_position.x,player.block_position.y,player.block_position.z]);
                let init = &mut world.debug_chr_creator.init_data;
                unsafe {
                    let bytes = std::ptr::from_mut(init).cast::<u8>();
                    bytes.add(0x10).cast::<F32Vector4>().write(F32Vector4(0.0,yaw,0.0,0.0));
                    bytes.add(0xc8).write(0);
                }
                world.debug_chr_creator.last_created_chr = None;
                super::spawn_guard::spawn(world,&ChrDebugSpawnRequest {
                    chr_id:foe.spec.model, npc_param_id:foe.spec.npc, npc_think_param_id:foe.spec.think as i32,
                    chara_init_param_id:0, event_entity_id:pending.entity_base.map_or_else(||if pending.networked||foe.spec.bar>0||foe.spec.model==4730 {arena.boss_entity(i)}else{0},|base|base+i as i32), talk_id:-1, pos_x:pos.0,pos_y:pos.1,pos_z:pos.2,
                });
                foe.spawn_check = SpawnCheck { target_local: Some(target_local), bound_at: None, verified: false, fixes: 0 };
                foe.requested = Some(Instant::now());
                super::log(format!("Group spawn {}/{}: {} local={local:?} world={pos:?}",i+1,count,foe.spec.name));
            }
        }
        spawning |= foe.fight.handle().is_none() && foe.requested.is_some();
        // 兜底单人调试刷怪同帧浮点原点切换（如龙王平台 z=328）：若落点偏离预期 >6m 水平或 >4m 垂直，立刻拉回。
        if authority && !foe.spawn_check.verified
            && let Some(expected_local) = foe.spawn_check.target_local
            && let Some(handle) = foe.fight.handle()
        {
            let origin = world.main_player.as_ref().and_then(|p| {
                let h = &p.chr_ins.modules.physics.position;
                let b = &p.block_position;
                erdtree_tower::encounter::floating_origin([h.0,h.1,h.2],[b.x,b.y,b.z])
            });
            if let Some(origin) = origin
                && let Some(chr) = world.chr_ins_by_handle_mut(&handle)
                    .filter(|c| foe.entity_address == Some(std::ptr::from_ref(&**c) as usize))
            {
                let bound_at = *foe.spawn_check.bound_at.get_or_insert_with(Instant::now);
                let alive = chr.modules.data.hp > 0 && !chr.chr_flags1c5.death_flag();
                if !alive || bound_at.elapsed() > Duration::from_secs(2) {
                    foe.spawn_check.verified = true;
                } else {
                    let p = chr.modules.physics.position;
                    let proxy_pending = chr.modules.physics.chr_proxy_pos_update_requested;
                    if let Some((target_world, actual_local, horiz, vert)) =
                        erdtree_tower::encounter::spawn_drift_target([p.0,p.1,p.2], expected_local, origin)
                    {
                        if foe.spawn_check.fixes < 3 {
                            foe.spawn_check.fixes += 1;
                            let moved = super::spirits::landing::move_native(&mut chr.modules.physics, target_world);
                            if moved.is_err() {
                                let physics = &mut chr.modules.physics;
                                physics.position = HavokPosition(target_world[0],target_world[1],target_world[2],0.0);
                                physics.last_update_position = physics.position;
                                physics.chr_proxy_pos_update_requested = true;
                            }
                            super::log(format!(
                                "Group enemy spawn-drift corrected #{}: {}/{} {} handle={handle} actual_local=[{:.2},{:.2},{:.2}] expected_local=[{:.2},{:.2},{:.2}] horiz={horiz:.2}m vert={vert:.2}m native={}",
                                foe.spawn_check.fixes, i+1, count, foe.spec.name,
                                actual_local[0], actual_local[1], actual_local[2],
                                expected_local[0], expected_local[1], expected_local[2],
                                moved.is_ok()
                            ));
                        } else {
                            foe.spawn_check.verified = true;
                        }
                    } else if !proxy_pending {
                        foe.spawn_check.verified = true;
                    }
                }
            }
        }
        // 准备阶段同时禁止出招与受击；结束时恢复原受击状态。
        if authority&&!foe.graced && let Some(chr)=foe.fight.handle().and_then(|h|world.chr_ins_by_handle_mut(&h))
            &&foe.entity_address==Some(std::ptr::from_ref(chr) as usize) {
            let started=*foe.grace.get_or_insert_with(Instant::now);
            let alive=chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag();
            let holding=alive&&foe.spawn_gap.holds_attacks(started.elapsed());
            if let Some(protected)=foe.spawn_protection.update(holding,alive,chr.debug_flags.disabled_hit()) {
                chr.debug_flags.set_disabled_hit(protected);
            }
            chr.debug_flags.set_disabled_secondary_actions(holding);
            if !holding {foe.graced=true;foe.grace=None;}
        }
        let enemy = foe.fight.handle().and_then(|h| world.chr_ins_by_handle(&h));
        let observation = match enemy {
            Some(chr) if chr.npc_id != foe.spec.model || chr.npc_param_id != foe.spec.npc
                || foe.entity_address != Some(std::ptr::from_ref(chr) as usize) => Observation::DifferentEntity,
            Some(chr) if !enemy_loaded(unsafe { chr.chr_set_entry.as_ref() }.chr_load_status) => Observation::Missing,
            Some(chr) if chr.modules.data.hp <= 0 || chr.chr_flags1c5.death_flag() => Observation::Dead,
            Some(_) => Observation::Alive,
            None => Observation::Missing,
        };
        let outcome = foe.fight.poll(false, observation, foe.requested.is_some_and(|t| t.elapsed() > SPAWN_TIMEOUT));
        if matches!(outcome,Outcome::SpawnTimedOut|Outcome::LostEntity)&&let Some(ids)=&map_ids {log_map_enemy(world,ids[i],foe.spec);}
        bound |= foe.fight.handle().is_some();
        if outcome==Outcome::Waiting&&observation==Observation::Alive&&let Some(chr)=enemy&&chr.modules.data.hp>=chr.modules.data.max_hp {untouched.push(i);}
        if let Some(chr) = enemy {
            max_hp = max_hp.saturating_add(chr.modules.data.max_hp);
            if outcome != Outcome::Victory { hp = hp.saturating_add(chr.modules.data.hp.max(0)); }
        }
        outcomes.push(outcome);
    }
    if !pending.networked {pending.godskin_pace.sync(&pending.foes,world);}
    let stalled=outcomes.contains(&Outcome::Victory)&&!untouched.is_empty()
        &&outcomes.iter().enumerate().all(|(i,o)|*o==Outcome::Victory||untouched.contains(&i));
    if !stalled {pending.stall=None;}
    else if diagnostic_stall_victory(stall_victory_enabled(),authority,pending.stall.get_or_insert_with(Instant::now).elapsed()) {
        let origin=world.main_player.as_ref().map(|p|(p.chr_ins.modules.physics.position.0-p.block_position.x,p.chr_ins.modules.physics.position.1-p.block_position.y,p.chr_ins.modules.physics.position.2-p.block_position.z));
        for &i in &untouched {
            let foe=&pending.foes[i];
            if let Some(chr)=foe.fight.handle().and_then(|h|world.chr_ins_by_handle_mut(&h)).filter(|c|foe.entity_address==Some(std::ptr::from_ref(&**c) as usize)) {
                let p=&chr.modules.physics.position;
                let local=origin.map(|o|[p.0-o.0,p.1-o.1,p.2-o.2]);
                super::log(format!("STALL_VICTORY retiring untouched foe {}/{} {} model={} entity={} handle={} hp={}/{} load={:?} disabled={} local={local:?} arena_center={:?}",
                    i+1,count,foe.spec.name,foe.spec.model,chr.event_entity_id,chr.field_ins_handle,chr.modules.data.hp,chr.modules.data.max_hp,
                    unsafe {chr.chr_set_entry.as_ref()}.chr_load_status,chr.debug_flags.character_disabled(),super::arenas::for_block(block).map(|a|a.center)));
                chr.debug_flags.set_character_disabled(true);
            }
        }
        return (Outcome::Victory,bound.then_some((0,max_hp)),bound);
    }
    (super::enemies::group_outcome(&outcomes, false), bound.then_some((hp,max_hp)),bound)
}
fn ash_tracking_active(preparing:bool,fighting:bool)->bool {preparing||fighting}
fn settle_portal_ashes(state:&mut Runtime,world:&mut WorldChrMan) {
    if !world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()){return;}
    if state.ash_guard.is_none() {
        if let Some(run)=state.view.run.as_ref() {
            if let Some(plan)=keep_summoned_ash(world,run) {
                state.ash_refunds=plan;state.ash_guard=Some(Instant::now());
                super::log("ASH_GROUP portal settlement: surviving local summons queued for refund");
            }
        }
    }
    super::spirits::log_remaining(world,"portal_settlement");
    if world.summon_buddy_manager.player_has_alive_summon||super::spirits::has_local_groups(world) {
        world.summon_buddy_manager.disappear_requested=true;
    }
}
fn ash_tracking_start(context:bool,tracking:bool,refund_pending:bool,dismissing:bool)->bool {
    context&&!tracking&&!refund_pending&&!dismissing
}
#[test]
fn event_preparation_starts_tracking_without_resetting_it_at_spawn_or_dismissal() {
    assert!(ash_tracking_start(true,false,false,false));
    assert!(!ash_tracking_start(true,true,false,false));
    assert!(!ash_tracking_start(true,false,true,false));
    assert!(!ash_tracking_start(true,false,false,true));
    assert!(!ash_tracking_start(false,false,false,false));
    assert!(ash_tracking_start(true,false,false,false),"next wave may start after refunds finish");
}
fn extra_summon_scope(party:Option<(u64,u8,i32)>,hunter:Option<(u64,i32)>,solo_scarabs:Option<(u64,u8,i32)>)->Option<(u64,u8,i32)> {
    party.or_else(||hunter.map(|(epoch,block)|(epoch,250,block)))
        .or_else(||solo_scarabs.map(|(epoch,attempt,block)|(epoch,200u8.saturating_add(attempt),block)))
}
fn frozen_ash_refund_ready(owner_alive:bool,warping:bool,guard:bool)->bool {
    owner_alive&&!warping&&guard
}
fn ash_refund_ready(pending_completed:Option<bool>,summon_alive:bool,guard:&mut Option<Instant>)->bool {
    let fighting=pending_completed==Some(false);
    !fighting&&(!summon_alive||guard.is_some())
}
/// 本局的骨灰最高等级：只升不降；旧存档没有记录时从原来的上限（武器换算）接着算
fn tick_camp_revival(run:&mut Run,room:&mut super::rooms::Room,data:&mut eldenring::cs::PlayerGameData)->Result<(),String>{
    use erdtree_tower::rewards::{camp_revival_target,spirit_level};
    let active=!room.view.completed&&matches!(run.phase,Phase::Node{id} if id==room.node);
    let request=room.take_revival();
    if !room.waiting_revival()&&request.is_none()&&run.camp_revival.is_none(){return Ok(());}
    if !active {room.set_revival_offers(&[]);return Ok(());}
    let held=|id:u32|super::loot::held(data,|item|item.into_inner()==id);
    let pending=run.camp_revival.filter(|(act,node,..)|(*act,*node)==(run.act,room.node));
    let mut offers=Vec::new();
    if let Some((_,_,source,target,_))=pending {offers.push((source,target));}
    else{
        for &source in &run.fallen_ashes{
            if let Some(target)=camp_revival_target(run,source)
                &&!offers.iter().any(|&(_,id)|id==target)&&held(target)==0{offers.push((source,target));}
        }
    }
    room.set_revival_offers(&offers);
    if let Some((source,target))=request{
        if !offers.contains(&(source,target)){return room.notice(super::native_talk::REVIVE_NONE);}
        if pending.is_none(){
            super::items::check_reward(data,target)?;
            let mut next=run.clone();
            next.camp_revival=Some((run.act,room.node,source,target,room.allowance_issued.then(||super::smith::remaining(data))));
            record_checked(&next)?;*run=next;
        }
        if super::loot::held(data,|item|item.into_inner()==target)==0 {
            if let Err(err)=super::items::give_reward(data,target){
                super::log(format!("Camp ash revival: {err}"));
                return room.notice(super::native_talk::REVIVE_FAILED);
            }
        }
    }
    let Some((act,node,source,target,stones))=run.camp_revival else{return Ok(());};
    if (act,node)!=(run.act,room.node)||super::loot::held(data,|item|item.into_inner()==target)==0{return Ok(());}
    let Some(pos)=run.fallen_ashes.iter().position(|&id|id==source) else{return Err("找不到战死骨灰记录".into());};
    let mut next=run.clone();
    next.leave_camp()?;
    next.fallen_ashes.remove(pos);
    next.spirit_peak=Some(next.spirit_peak.unwrap_or(0).max(spirit_level(target).unwrap_or(0)));
    next.camp_revival=None;
    if stones.is_some_and(|n|n>0&&super::smith::remaining(data)>=n){super::smith::consume_one(data)?;}
    record_checked(&next)?;*run=next;
    room.complete("骨灰已复活，本次篝火服务已使用。".into());
    room.set_revival_offers(&[]);
    room.notice(super::native_talk::REVIVE_DONE)?;
    super::log(format!("CAMP_ASH_REVIVE source={source} target={target}"));
    Ok(())
}

fn spirit_peak(run:&Run,inventory:u8)->u8 {
    run.spirit_peak.unwrap_or_else(||(erdtree_tower::rewards::spirit_cap(run).saturating_sub(1)) as u8).max(inventory).min(erdtree_tower::rewards::SPIRIT_MAX)
}
/// 每次召唤独立判断存活，冻结本场返还清单；离场后发放并等待背包确认。
fn keep_summoned_ash(world:&WorldChrMan,run:&erdtree_tower::Run)->Option<Vec<super::spirits::refund::Refund>> {
    let plan=super::spirits::refund::settle(world,run.summoner());
    (!plan.is_empty()).then_some(plan)
}
fn retire_group(pending:&Pending,world:&mut WorldChrMan){
    pending.godskin_pace.release(world);
    if !pending.networked {super::radahn_entity::sync(world,None,false);}
    if pending.networked&&pending.entity_base.is_none() {
        if let Ok(ids)=super::enemies::map_entities(&pending.foes.iter().map(|f|f.spec).collect::<Vec<_>>()) {
            for id in ids {if let Err(error)=super::enemies::request_map_entity(id,false){super::log(error);}}
        }
    }
    for foe in &pending.foes {
        if !pending.networked&&foe.fight.handle().is_none()&&foe.requested.is_some()
            &&world.debug_chr_creator.spawn&&world.debug_chr_creator.init_data.npc_param_id==foe.spec.npc {
            world.debug_chr_creator.spawn=false;
            world.debug_chr_creator.last_created_chr=None;
        }
        if let Some(chr)=foe.fight.handle().and_then(|h|world.chr_ins_by_handle_mut(&h)) {
            if foe.entity_address==Some(std::ptr::from_ref(chr) as usize){
                if let Some(previous)=foe.spawn_protection.previous {chr.debug_flags.set_disabled_hit(previous);chr.debug_flags.set_disabled_secondary_actions(false);}
                chr.debug_flags.set_character_disabled(true);
            }
        }
    }
}

struct ParkourRoute {block:i32,entry:u32,flag:u32,name:&'static str,goal:[f32;3]}
fn hidden_road_route(_:erdtree_tower::regional_events::Trial)->ParkourRoute {
    ParkourRoute{block:0x3c343900,entry:1052570988,flag:10007990,name:"异端魔法师塔·隐形桥",goal:[68.088,1683.763,-48.374]}
}
fn run_parkour_route(run:&Run)->ParkourRoute {
    run.regional_event.hidden_road.map(hidden_road_route).unwrap_or_else(||parkour_route(run.regional_event.chariot))
}
fn verify_hidden_road_resources()->Result<(),String> {
    verify_hidden_road_at(&crate::LOG_PATH.with_file_name("scene-assets"))
}
fn verify_hidden_road_at(root:&std::path::Path)->Result<(),String> {
    for line in include_str!("../../scene-assets/hidden-road-resources.txt").lines() {
        let p:Vec<_>=line.split('|').collect();
        let bytes=std::fs::read(root.join(p[0])).map_err(|_|"隐形道路场景资源缺失，请安装完整挑战补丁")?;
        let hash=bytes.iter().fold(0xcbf29ce484222325u64,|h,b|(h^u64::from(*b)).wrapping_mul(0x100000001b3));
        if bytes.len().to_string()!=p[1]||format!("{hash:016x}")!=p[2]{return Err("隐形道路场景与 DLL 不匹配，请一起更新补丁".into());}
    }
    Ok(())
}
#[test]
fn hidden_road_routes_require_matching_scene_and_grounded_finish() {
    use erdtree_tower::regional_events::Trial;
    verify_hidden_road_at(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scene-assets")).unwrap();
    for choice in [Trial::HereticalRise] {
        let route=hidden_road_route(choice);
        assert!(route.reached(route.block,route.goal,true));
        assert!(!route.reached(route.block,route.goal,false));
        assert!(!route.reached(0,route.goal,true));
        assert_eq!(trial_revival_destination(Some(choice)).unwrap().xyz,route.goal);
    }
}
#[test]
fn stormveil_revival_uses_map_coordinates_across_origin_shifts() {
    use super::party_death::{Position,Death,Action};
    let goal=trial_revival_destination(Some(erdtree_tower::regional_events::Trial::Stormveil)).unwrap();
    let block=Position{block:0x0a000000,xyz:[-93.97,21.54,19.71]};
    let physics=Position{xyz:[-5.97,5.54,3.71],..block};
    let target=goal.in_physics_space(block,physics).unwrap();
    for (actual,expected) in target.xyz.into_iter().zip([-114.71,65.864,298.077]) {assert!((actual-expected).abs()<0.001);}
    let mut death=Death::default();let epoch=Some(3);
    assert_eq!(death.observe_confirmed(epoch,Some((physics,true)),false,false,false),Action::Waiting);
    death.redirect(target);
    assert_eq!(death.observe_confirmed(epoch,Some((physics,true)),true,false,false),Action::RequestRevival(target));
    let shifted=Position{xyz:[goal.xyz[0]+160.0,goal.xyz[1]-48.0,goal.xyz[2]-304.0],..goal};
    let current_target=goal.in_physics_space(goal,shifted).unwrap();
    death.redirect(current_target);
    assert_eq!(death.observe_confirmed(epoch,Some((shifted,false)),true,false,false),Action::Waiting);
    assert_eq!(death.observe_confirmed(epoch,Some((shifted,false)),true,false,true),Action::FinishRevival);
}
fn parkour_route(chariot:bool)->ParkourRoute {
    if chariot {ParkourRoute{block:0x2d010000,entry:45012880,flag:45007880,name:"泥头车生存",goal:[0.0,0.0,0.0]}}
    else {ParkourRoute{block:0x0a000000,entry:10000988,flag:10007990,name:"史东薇尔突围",goal:[-202.71,81.864,314.077]}}
}
impl ParkourRoute {
    fn reached(&self,block:i32,pos:[f32;3],grounded:bool)->bool {
        block==self.block&&grounded&&(pos[1]-self.goal[1]).abs()<2.0
            &&(pos[0]-self.goal[0]).powi(2)+(pos[2]-self.goal[2]).powi(2)<16.0
    }
}
fn start_stormveil(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan,run:Run)->Result<(),String> {
    let (hp,fp)={
        let player=world.main_player.as_ref().ok_or("角色尚未加载")?;
        if game.warp_requested||!player.chr_ins.modules.physics.standing_on_solid_ground{return Err("请等待角色站稳".into());}
        (player.chr_ins.modules.data.hp,player.chr_ins.modules.data.fp)
    };
    if run.regional_event.stormveil!=Some(erdtree_tower::stormveil::Progress::Running)&&!(run.regional_event.chariot&&run.regional_event.stormveil==Some(erdtree_tower::stormveil::Progress::Complete)){return Err("跑城挑战尚未确认".into());}
    if run.regional_event.hidden_road.is_some(){verify_hidden_road_resources()?;}
    record_checked(&run)?;
    let route=run_parkour_route(&run);
    state.chariot_clock=None;
    if run.regional_event.chariot {
        clear_chariot()?;
        set_flag(45010942,true)?;set_flag(45010995,true)?;
    }
    super::spirits::prepare_warp(world,"start_parkour");
    super::native_talk::set_flag(route.flag,true)?;
    game.move_map_target=BlockId::from(route.block);game.initial_area_entity_id=route.entry;game.warp_requested=true;
    state.stormveil=Some((Instant::now(),hp,fp,false));
    state.stormveil_barrier.reset();
    state.room=None;state.party_scene=None;state.pending=None;state.travel=None;state.command=None;
    let message=if run.regional_event.chariot{"泥头车生存：正在前往竞技场中心".into()}else{format!("{}：抵达路线终点",route.name)};
    state.view.run=Some(run);state.view.room=Some(super::rooms::RoomView{message,loading:true,completed:false});
    state.view.busy=false;state.view.travelling=false;state.view.enemy_hp=None;
    state.pad_capture.update(false);super::input::CAPTURE.store(false,std::sync::atomic::Ordering::Relaxed);
    if state.view.run.as_ref().is_some_and(|r|r.regional_event.chariot) {
        report(state,"正在前往竞技场中心。全队到齐后倒数 5 秒，战车出现后坚持 30 秒。");
    }else{report(state,format!("正在前往{}。沿路线抵达终点即可完成，无需击败 Boss。",route.name));}Ok(())
}
#[derive(Clone,Copy)]
enum ChariotClock {Warmup(Instant),Loading(Instant),Surviving(Instant)}
impl ChariotClock {
    const SURVIVAL_SECONDS:u64=30;
    fn tick(&mut self,now:Instant,loaded:bool)->(bool,bool,u64) {
        match *self {
            Self::Warmup(start)=>{
                let elapsed=now.duration_since(start).as_secs();
                if elapsed>=5 {*self=Self::Loading(now);(true,false,Self::SURVIVAL_SECONDS)}else{(false,false,5-elapsed)}
            },
            Self::Loading(_)=>{if loaded{*self=Self::Surviving(now);}(true,false,Self::SURVIVAL_SECONDS)},
            Self::Surviving(start)=>{
                if !loaded {*self=Self::Loading(now);return (true,false,Self::SURVIVAL_SECONDS);}
                let elapsed=now.duration_since(start).as_secs();(true,elapsed>=Self::SURVIVAL_SECONDS,Self::SURVIVAL_SECONDS.saturating_sub(elapsed))
            },
        }
    }
}
fn clear_chariot()->Result<(),String> {for id in 45007880..=45007886{set_flag(id,false)?;}Ok(())}
fn tick_chariot(state:&mut Runtime,world:&mut WorldChrMan,game:&GameMan)->Result<(),String> {
    use erdtree_tower::stormveil::{Progress,Decision};
    let player=world.main_player.as_mut().ok_or("角色正在加载")?;
    let (started,hp,fp,restored)=state.stormveil.as_mut().unwrap();
    if game.warp_requested||i32::from(player.current_block_id)!=0x2d010000 {
        if !*restored&&started.elapsed()>Duration::from_secs(60){return Err(format!("泥头车竞技场加载超时，可按 {} 暂离结算",super::options::label(erdtree_tower::options::Hotkey::Settle)));}
        return Ok(());
    }
    let alive=player.chr_ins.modules.data.hp>0&&!player.chr_ins.chr_flags1c5.death_flag();
    if !alive&&state.party.is_none(){clear_chariot()?;if let Some(run)=state.view.run.as_mut(){run.end_on_death();record_checked(run)?;}return Ok(());}
    if !*restored&&alive&&player.chr_ins.modules.physics.standing_on_solid_ground {
        super::ailments::clear(&mut player.chr_ins)?;
        super::travel::restore_arrival_hp(&mut player.chr_ins,*hp,"chariot arrival");
        player.chr_ins.modules.data.fp=player.chr_ins.modules.data.fp.min(*fp);*restored=true;
    }
    let running=state.view.run.as_ref().is_some_and(|r|r.regional_event.stormveil==Some(Progress::Running));
    if running {
        let engage=if let Some(link)=state.party.as_mut(){link.event.chariot_ready=*restored&&alive;matches!(link.event.storm_decision,Some(Decision::Engage|Decision::Complete))}else{*restored};
        if !engage {if let Some(v)=state.view.room.as_mut(){v.message="等待全队进入竞技场……".into();}return Ok(());}
        let loaded=flag(45007882)?&&world.chr_sets.iter().flatten().flat_map(|s|s.characters()).any(|c|c.event_entity_id==45011880&&c.npc_id==8120&&!c.debug_flags.character_disabled());
        let clock=state.chariot_clock.get_or_insert(ChariotClock::Warmup(Instant::now()));
        let (spawn,finished,remaining)=clock.tick(Instant::now(),loaded);
        set_flag(45007881,spawn)?;
        if let ChariotClock::Loading(at)=clock {if at.elapsed()>Duration::from_secs(20){return Err(format!("战车加载超时，尚未开始生存计时；可按 {} 暂离结算",super::options::label(erdtree_tower::options::Hotkey::Settle)));}}
        if let Some(v)=state.view.room.as_mut(){v.loading=false;v.message=match clock {ChariotClock::Warmup(_)=>format!("战车将在 {remaining} 秒后出现"),ChariotClock::Loading(_)=>"正在加载战车……".into(),ChariotClock::Surviving(_)=>format!("泥头车生存：剩余 {remaining} 秒")};}
        let complete=if let Some(link)=state.party.as_mut(){link.event.storm_finish=finished;link.event.storm_decision==Some(Decision::Complete)}else{finished};
        if !complete{return Ok(());}
        let mut next=state.view.run.clone().ok_or("旅程未加载")?;next.stormveil_decide(Decision::Complete)?;record_checked(&next)?;
        // 召唤师「群灵」：撑满时间时召唤物还活着，补回这次用掉的骨灰
        if let Some(plan)=keep_summoned_ash(world,&next) {state.ash_refunds=plan;state.ash_guard=Some(Instant::now());}
        state.view.run=Some(next);
        super::log("Chariot survival complete; personal legendary chest unlocked");
    }
    set_flag(45007881,false)?;
    if flag(45007882)?{return Ok(());}
    set_flag(45007883,true)?;
    if state.party_death.awaiting_revival(){return Ok(());}
    let run=state.view.run.as_ref().ok_or("旅程未加载")?;
    let Phase::Node{id}=run.phase else{return Ok(());};
    let player=world.main_player.as_mut().ok_or("角色正在加载")?;
    let data=unsafe{player.player_game_data.as_mut()};
    if alive&&run.regional_event.opened==0&&run.regional_event.receipt.is_none()&&flag(45007884)? {
        let item=erdtree_tower::regional_events::descent_reward(run.reward_seed(),id);
        let item=erdtree_tower::affixes::roll_min(run,item,id as u64,true,3).ok_or("传说词条奖励缺失")?;
        let mut next=run.clone();next.begin_chariot_reward(item,loot::count(data,item))?;record_checked(&next)?;state.view.run=Some(next);
    }
    if alive {if let Some(receipt)=state.view.run.as_ref().unwrap().regional_event.receipt.clone() {
        if loot::count(data,receipt.item)<receipt.reward_target{super::items::give_reward(data,receipt.item)?;}
        if loot::count(data,receipt.item)<receipt.reward_target{return Err("传说奖励尚未进入背包，请清理容量".into());}
        let mut next=state.view.run.as_ref().unwrap().clone();next.complete_chariot_reward()?;record_checked(&next)?;state.view.run=Some(next);set_flag(45007884,false)?;
    }}
    let claimed=state.view.run.as_ref().unwrap().regional_event.opened!=0;set_flag(45007885,claimed)?;
    if let Some(v)=state.view.room.as_mut(){v.loading=false;v.completed=claimed;v.message=if claimed{"挑战完成，请使用传送门"}else{"挑战胜利！请领取竞技场中心的传说宝箱"}.into();}
    if claimed&&flag(45007886)? {
        let mut next=state.view.run.as_ref().unwrap().clone();next.leave_regional_event()?;record_checked(&next)?;state.view.run=Some(next);
        clear_chariot()?;state.stormveil=None;state.chariot_clock=None;state.view.room=None;state.view.busy=false;report(state,"传说宝箱已领取，请选择下一节点。");
    }
    Ok(())
}
#[cfg(test)]
fn stormveil_finish(block:i32,x:f32,y:f32,z:f32,grounded:bool)->bool {
    parkour_route(false).reached(block,[x,y,z],grounded)
}
fn tick_stormveil(state:&mut Runtime,world:&mut WorldChrMan,game:&GameMan)->Result<(),String> {
    use erdtree_tower::stormveil::{Progress,Decision};
    if state.view.run.as_ref().is_some_and(|r|r.regional_event.chariot){return tick_chariot(state,world,game);}
    let route=run_parkour_route(state.view.run.as_ref().ok_or("旅程未加载")?);
    if state.party.is_some()&&state.stormveil_probe.is_none_or(|at|at.elapsed()>=Duration::from_secs(5)) {
        state.stormveil_probe=Some(Instant::now());
        let local=world.main_player.as_ref().map(|p|{
            let b=&p.block_position;let h=&p.chr_ins.modules.physics;
            (i32::from(p.current_block_id),p.chr_ins.modules.data.hp,[b.x,b.y,b.z],[h.position.0,h.position.1,h.position.2],h.standing_on_solid_ground,h.is_falling)
        });
        let peers:Vec<_>=world.player_chr_set.characters().map(|p|{
            let b=&p.block_position;
            (i32::from(p.current_block_id),p.chr_ins.modules.data.hp,[b.x,b.y,b.z],p.chr_ins.modules.physics.standing_on_solid_ground)
        }).collect();
        super::log(format!("PARKOUR_LOAD host={:?} warp={} local=(block,hp,map,physics,grounded,falling){local:?} peers=(block,hp,map,grounded){peers:?} goal={:?} recovery={} decision={:?}",
            state.party.as_ref().map(|p|p.host()),game.warp_requested,route.goal,state.party_death.awaiting_revival(),state.party.as_ref().and_then(|p|p.event.storm_decision)));
    }
    let player=world.main_player.as_mut().ok_or("角色正在加载")?;
    let (started,hp,fp,restored)=state.stormveil.as_mut().unwrap();
    let block=i32::from(player.current_block_id);
    if game.warp_requested||block!=route.block {
        state.stormveil_barrier.reset();
        if started.elapsed()>Duration::from_secs(60)&&!*restored{return Err(format!("{}加载超时",route.name));}
        return Ok(());
    }
    let alive=player.chr_ins.modules.data.hp>0&&!player.chr_ins.chr_flags1c5.death_flag();
    if alive&&route.block==0x0a000000 {state.stormveil_barrier.apply(player);}else{state.stormveil_barrier.reset();}
    if !alive&&state.party.is_none() {
        if let Some(run)=state.view.run.as_mut(){run.end_on_death();record_checked(run)?;}
        return Ok(());
    }
    let grounded=alive&&player.chr_ins.modules.physics.standing_on_solid_ground&&!player.chr_ins.modules.physics.is_falling;
    if !*restored&&grounded {
        super::ailments::clear(&mut player.chr_ins)?;
        super::travel::restore_arrival_hp(&mut player.chr_ins,*hp,"parkour arrival");
        player.chr_ins.modules.data.fp=player.chr_ins.modules.data.fp.min(*fp);*restored=true;
        if let Some(view)=state.view.room.as_mut(){view.loading=false;}
    }
    let pos=&player.block_position;
    let reached=*restored&&route.reached(block,[pos.x,pos.y,pos.z],grounded);
    let complete=if let Some(link)=state.party.as_mut() {
        link.event.storm_finish=reached;
        link.event.storm_decision==Some(Decision::Complete)
    }else{reached};
    if !complete||!alive||state.party_death.awaiting_revival(){return Ok(());}
    let mut next=state.view.run.clone().ok_or("旅程未加载")?;
    if next.regional_event.stormveil!=Some(Progress::Running){return Err("跑城进度不匹配".into());}
    super::elite::validate()?;
    next.stormveil_decide(Decision::Complete)?;next.leave_regional_event()?;
    record_checked(&next)?;
    // 召唤师「群灵」：抵达葛瑞克门前时召唤物还活着，补回这次用掉的骨灰
    if let Some(plan)=keep_summoned_ash(world,&next) {state.ash_refunds=plan;state.ash_guard=Some(Instant::now());}
    super::run_screen::trial_rewards(route.name,state.view.run.as_ref().unwrap(),&next);
    state.view.run=Some(next);
    super::native_talk::set_flag(route.flag,false)?;
    state.stormveil=None;state.view.room=None;state.view.busy=false;
    report(state,"挑战完成，请确认本次获得的护符赐福。");Ok(())
}

fn start_descent(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan,mut run:Run)->Result<(),String> {
    let (hp,fp)={
        let player=world.main_player.as_ref().ok_or("角色尚未加载")?;
        if game.warp_requested||!player.chr_ins.modules.physics.standing_on_solid_ground{return Err("请等待角色站稳".into());}
        (player.chr_ins.modules.data.hp,player.chr_ins.modules.data.fp)
    };
    if !run.regional_event.descent_started {
        if run.regional_event.opened==0&&run.regional_event.receipt.is_none(){return Err("请先与引导者交谈并参加试炼".into());}
        run.regional_event.descent_started=true;
    }
    record_checked(&run)?;
    for id in 35007990..=35007994 {super::native_talk::set_flag(id,false)?;}
    super::native_talk::set_flag(35007990,true)?;
    super::spirits::prepare_warp(world,"start_descent");
    game.move_map_target=BlockId::from(0x23000000);
    game.initial_area_entity_id=if run.regional_event.opened!=0||run.regional_event.receipt.is_some(){35000989}else{35000988};
    game.warp_requested=true;
    state.descent=Some((Instant::now(),hp,fp,false));
    state.room=None;state.party_scene=None;state.pending=None;state.travel=None;state.command=None;
    state.view.room=Some(super::rooms::RoomView{message:"是男人就下一百层".into(),loading:true,completed:false});
    state.view.run=Some(run);state.view.busy=false;state.view.travelling=false;state.view.enemy_hp=None;
    state.pad_capture.update(false);super::input::CAPTURE.store(false,std::sync::atomic::Ordering::Relaxed);
    report(state,"正在前往「是男人就下一百层」：癫火深坑跳跳乐。");Ok(())
}
fn tick_descent(state:&mut Runtime,world:&mut WorldChrMan,game:&GameMan)->Result<(),String> {
    use super::{native_talk::{flag,set_flag},loot};
    let player=world.main_player.as_mut().ok_or("角色正在加载")?;
    let (started,hp,fp,restored)=state.descent.as_mut().unwrap();
    if game.warp_requested||i32::from(player.current_block_id)!=0x23000000 {
        if started.elapsed()>Duration::from_secs(60)&&!*restored{return Err("是男人就下一百层：地图加载超时".into());}
        return Ok(());
    }
    if player.chr_ins.modules.data.hp<=0||player.chr_ins.chr_flags1c5.death_flag(){
        if state.party.is_none() {
            if let Some(run)=state.view.run.as_mut(){run.end_on_death();record_checked(run)?;}
            state.command=None;state.view.ready=false;state.view.busy=false;
            report(state,"是男人就下一百层：挑战中死亡，本局结束。");
        }
        return Ok(());
    }
    let grounded=player.chr_ins.modules.physics.standing_on_solid_ground&&!player.chr_ins.modules.physics.is_falling;
    if state.party_death.awaiting_revival(){return Ok(());}
    if !*restored {
        if !grounded{return Ok(());}
        super::ailments::clear(&mut player.chr_ins)?;
        super::travel::restore_arrival_hp(&mut player.chr_ins,*hp,"descent arrival");
        player.chr_ins.modules.data.fp=player.chr_ins.modules.data.fp.min(*fp);*restored=true;
        if let Some(view)=state.view.room.as_mut(){view.loading=false;}
    }
    let run=state.view.run.as_ref().ok_or("旅程未加载")?;
    let Phase::Node{id}=run.phase else{return Err("是男人就下一百层：试炼进度不匹配".into());};
    if !run.regional_event.descent{return Err("当前不是「是男人就下一百层」试炼".into());}
    let requested=flag(35007991)?;
    let pos=&player.block_position;
    let at_finish=grounded&&(pos.y+408.535).abs()<2.0&&(pos.x-125.644).powi(2)+(pos.z+61.213).powi(2)<25.0;
    let party_complete=if let Some(link)=state.party.as_mut() {
        link.event.storm_finish|=at_finish||run.regional_event.opened!=0;
        link.event.descent_complete
    }else{false};
    let data=unsafe {player.player_game_data.as_mut()};
    if run.regional_event.opened==0&&run.regional_event.receipt.is_none()&&(requested||party_complete)&&at_finish {
        let item=erdtree_tower::regional_events::descent_reward(run.seed,id);
        let item=erdtree_tower::affixes::roll_min(run,item,id as u64,true,3).ok_or("传说词条奖励缺失")?;
        let mut next=run.clone();next.begin_descent(item,loot::count(data,item))?;
        record_checked(&next)?;state.view.run=Some(next);
    }
    if let Some(receipt)=state.view.run.as_ref().unwrap().regional_event.receipt.clone() {
        if loot::count(data,receipt.item)<receipt.reward_target {super::items::give_reward(data,receipt.item)?;}
        if loot::count(data,receipt.item)<receipt.reward_target{return Err("奖励尚未进入背包，请清理容量".into());}
        let mut next=state.view.run.as_ref().unwrap().clone();next.complete_descent()?;record_checked(&next)?;state.view.run=Some(next);
        set_flag(35007991,false)?;
    }
    let complete=state.view.run.as_ref().unwrap().regional_event.opened!=0;
    set_flag(35007992,complete)?;
    if let Some(view)=state.view.room.as_mut(){view.completed=complete;}
    if complete&&flag(35007993)? {
        let mut next=state.view.run.as_ref().unwrap().clone();next.leave_regional_event()?;record_checked(&next)?;
        state.view.run=Some(next);state.descent=None;state.view.room=None;
        set_flag(35007990,false)?;set_flag(35007993,false)?;
        report(state,"试炼完成。");
    }
    Ok(())
}
fn recover_entry(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan)->Result<(),String> {
    let travel=state.travel.as_ref();
    let run=state.view.run.as_ref();
    let remote=game.event_world_type==EventWorldType::Remote;
    let session_clear=unsafe{eldenring::cs::CSSessionManager::instance()}.is_ok_and(|s|s.players.len()<=1);
    super::recovery::Check {
        solo:state.party.is_none()&&state.party_pending.is_none()&&!remote&&session_clear,
        alive:world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()),
        idle:state.pending.is_none()&&state.reward_wait.is_none()&&state.room.is_none()&&state.command.is_none()&&!game.warp_requested
            &&run.is_some_and(|r|matches!(r.phase,Phase::Map)),
        preparing:travel.is_some_and(|(t,..)|t.is_preparing()),
        kind:travel.and_then(|(t,..)|run?.nodes.get(t.node).map(|n|n.kind)),
        elapsed:travel.map_or(Duration::ZERO,|(_,start,..)|start.elapsed()),
    }.allowed().map_err(str::to_owned)?;
    // Only repeat the already selected solo combat entry. Do not consume/clear rewards or change epochs.
    let arena=super::arenas::for_run(run.unwrap(),false);
    for (id,value) in [(45007991,false),(45007992,false),(45007993,false),(45007994,false),(45007990,true)] {super::native_talk::set_flag(id,value)?;}
    let (travel,started,_,_)=state.travel.as_mut().unwrap();
    let restored=travel.resources_restored;
    *travel=super::travel::Travel::new(travel.node);travel.resources_restored=restored;
    *started=Instant::now();
    super::spirits::prepare_warp(world,"recover_entry");
    game.move_map_target=BlockId::from(arena.block);game.initial_area_entity_id=arena.combat_entry();game.warp_requested=true;
    Ok(())
}

fn scene_players_ready(state:&Runtime,world:&WorldChrMan,arena:super::arenas::Arena)->bool {
    let ready=|p:&eldenring::cs::PlayerIns|i32::from(p.current_block_id)==arena.block
        &&(p.block_position.x-arena.center[0]).hypot(p.block_position.z-arena.center[2])<80.0
        &&(p.block_position.y-arena.center[1]).abs()<15.0
        &&p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag()
        &&p.chr_ins.modules.physics.standing_on_solid_ground&&!p.chr_ins.modules.physics.is_falling;
    let Some(local)=world.main_player.as_ref() else{return false;};
    if !ready(local){return false;}
    let Some(link)=state.party.as_ref() else {
        return unsafe {eldenring::cs::CSSessionManager::instance()}.is_ok_and(|s|s.players.len()<=1);
    };
    let Some(roster)=link.roster() else{return false;};
    link.stopped.is_none()&&!link.defeated&&roster.members.iter().all(|id|*id==roster.local
        ||world.player_chr_set.characters().any(|p|p.player_session_holder.player_network_session.remote_identity==*id&&ready(p)))
}
fn request_travel(
    state: &mut Runtime,
    world: &mut WorldChrMan,
    game: &mut GameMan,
    id: usize,
) -> Result<(), String> {
    request_travel_with(state, world, game, id, false)
}

fn request_travel_with(
    state: &mut Runtime,
    world: &mut WorldChrMan,
    game: &mut GameMan,
    id: usize,
    allow_dynamic_fallback: bool,
) -> Result<(), String> {
    if game.event_world_type==EventWorldType::Remote&&state.party.is_none() {
        return Err("等待队伍同步建立后再选择路线".into());
    }
    let run = state.view.run.as_ref().ok_or("角色未就绪")?;
    start_allowed(
        run,
        id,
        state.travel.is_some() || state.pending.as_ref().is_some_and(|p| !p.completed),
    )?;
    let kind = run.nodes[id].kind;
    if let Some(player)=world.main_player.as_ref() {
        super::smith::trace(unsafe{player.player_game_data.as_ref()},||format!("departure act={} node={id} kind={kind:?} party={}",run.act,state.party.is_some()),true);
    }
    if let Some(test)=run.boss_test.filter(|t|run.debug&&t.act==run.act&&kind==erdtree_tower::NodeKind::Boss) {
        super::enemies::boss_test_roster(test.encounter,state.party.is_some(),erdtree_tower::dlc::enemies_allowed())?;
    }
    if kind.combat()&&let Err(error)=super::arenas::require_resources(super::arenas::for_run(run,state.party.is_some())) {flag_resource_alert(state,&error);return Err(error);}
    if !kind.combat() && super::rooms::destination(kind).is_none() {
        let mut next = run.clone();
        next.enter(id)?;
        state.view.run = Some(next);
        state.view.busy = false;
        report(state, "请选择节点行动。");
        return Ok(());
    }
    let player = world.main_player.as_ref().ok_or("角色未就绪")?;
    let phys = &player.chr_ins.modules.physics;
    let grounded = departure_grounded(phys.standing_on_solid_ground, phys.touching_solid_ground, phys.is_touching_ground, phys.is_falling, allow_dynamic_fallback);
    if game.warp_requested
        || !grounded
        || player.chr_ins.chr_flags1c5.is_invincible()
    {
        return Err("请等待角色站稳并退出原版菜单后再出发。".into());
    }
    if super::rooms::destination(kind).is_some() {
        clear_chariot()?;
        state.event_start=None;
        let shop_stock=if kind == erdtree_tower::NodeKind::Shop {
            Some(super::native_shop::prepare(run,id,true,unsafe {&mut *player.player_game_data.as_ptr()})?)
        }else{super::card_affixes::clear_shop_offers();None};
        if kind == erdtree_tower::NodeKind::Chest {
            super::loot::prepare(run, id,unsafe {player.player_game_data.as_ref()})?;
        }
        let mut room = super::rooms::Room::for_run(
            run,
            id,
            kind,
            player.chr_ins.modules.data.hp,
            player.chr_ins.modules.data.fp,
        )?;
        room.shop_refresh_pending=kind==erdtree_tower::NodeKind::Shop;
        let mut next = run.clone();
        next.enter(id)?;
        if shop_stock.is_some(){next.shop_stock=shop_stock;record(&next);}
        if kind==erdtree_tower::NodeKind::Event {
            let eligible=state.party.as_ref().map_or_else(||loot::count(unsafe {player.player_game_data.as_ref()},0x40001f40)>0,|p|p.event.has_stonesword_key);
            if !eligible {next.regional_event.locked=false;}
        }
        if kind==erdtree_tower::NodeKind::Event&&state.party.is_some() {
            state.party_scene=Some(PartyScene{run:next.clone(),block:room.block,wave:0,started:Instant::now(),guards:None,hunt:None});
            if state.party.as_ref().unwrap().host(){state.party.as_mut().unwrap().gate.begin_events()?;}
        }
        super::spirits::prepare_warp(world,"travel_room");
        game.move_map_target = BlockId::from(room.block);
        game.initial_area_entity_id = room.entry;
        game.warp_requested = true;
        state.view.run = Some(next);
        state.view.room = Some(room.view.clone());
        state.room = Some(room);
        state.pending = None;
        state.view.enemy_hp = None;
        // 死亡重试回服务房间时 begin_death_retry 置了“传送中”；房间不走战斗传送流程，这里不清掉的话
        // 离开房间后路线图一直不显示、不能点（看起来像传送门没反应）
        state.view.travelling = false;
        state.pad_capture.update(false);
        super::input::CAPTURE.store(false, std::sync::atomic::Ordering::Relaxed);
        report(state, "正在前往服务房间。");
        return Ok(());
    }
    let arena=super::arenas::for_run(run,state.party.is_some());
    let previous=state.combat_scene;
    let reuse_check=super::travel::ReuseCheck {
        same_scene:previous.is_some_and(|(seed,act,block,_)|(seed,act,block)==(run.seed,run.act,arena.block)),
        route_ready:run.phase==Phase::Map&&state.spawn_used&&state.reward_wait.is_none()&&state.room.is_none()
            &&state.pending.as_ref().is_none_or(|p|p.completed)&&state.lingering.is_none(),
        players_ready:scene_players_ready(state,world,arena),
        clear:state.ash_guard.is_none()&&world.summon_buddy_manager.active_summon_goods_id<=0
            &&world.summon_buddy_manager.request_summon_speffect_id<0
            &&!world.summon_buddy_manager.player_has_alive_summon
            &&!world.summon_buddy_manager.groups.iter().any(|p|p.second.iter().next().is_some())
            &&unsafe {eldenring::cs::CSBulletManager::instance()}.is_ok_and(|b|b.bullets().next().is_none()),
        recovering:game.warp_requested||state.death_retry||run.retry_node.is_some()||state.reconnect.is_some()
            ||state.party_pending.is_some()||state.party_death.awaiting_revival(),
    };
    let arrival=super::arenas::combat_arrival(arena.block);
    let mut reuse=reuse_check.allowed()&&arrival.is_some();
    let previous_entities=if reuse&&state.party.is_some() {
        match previous.and_then(|(_,_,_,node)|super::enemies::map_entities(&super::enemies::node_roster(run,node)).ok()) {
            Some(ids)=>ids,None=>{reuse=false;vec![]}
        }
    }else{vec![]};
    super::log(format!("TRAVEL_SCENE reuse={reuse} target={:08x} node={id} checks={reuse_check:?}",arena.block));
    if reuse_check.same_scene&&!reuse {
        let buddy=&world.summon_buddy_manager;
        super::log(format!("TRAVEL_SUMMON reload active_goods={} active_effect={} pending={} active_stone={} alive={} dismiss={}",buddy.active_summon_goods_id,buddy.active_summon_speffect_id,buddy.request_summon_speffect_id,buddy.active_summmon_buddy_stone_entity_id,buddy.player_has_alive_summon,buddy.disappear_requested));
    }
    state.combat_scene=None;
    super::native_talk::set_flag(super::travel::RESET_REQUEST,false)?;
    super::native_talk::set_flag(super::travel::RESET_CLEARED,false)?;
    super::native_talk::set_flag(45020970, false)?;
    super::native_talk::set_flag(45020971, false)?;
    super::native_talk::set_flag(45020972, false)?;
    super::native_talk::set_flag(45007991, false)?;
    super::native_talk::set_flag(45007992, false)?;
    super::native_talk::set_flag(45007993, false)?;
    super::native_talk::set_flag(45007994, false)?;
    super::native_talk::set_flag(45007990, true)?;
    world.summon_buddy_manager.disappear_requested=true;
    let data=unsafe {player.player_game_data.as_ref()};
    // 灵唤铃是重要物品：用 loot::count 读（无缝联机下 items() 读不到，会每次出发都再发一个）
    if super::loot::count(data,0x40000000|8158)==0 {
        super::items::give(&[(0x40000000|8158,1,-1)])?;
    }
    let mut selected=run.clone();
    selected.preparing_combat=Some(id);
    record_checked(&selected)?;
    state.view.run=Some(selected);
    state.travel = Some((
        super::travel::Travel::new(id),
        Instant::now(),
        player.chr_ins.modules.data.hp,
        player.chr_ins.modules.data.fp,
    ));
    if reuse {
        super::native_talk::set_flag(super::enemies::MAP_COMBAT_FLAG,false)?;
        super::native_talk::set_flag(45020993,false)?;
        for entity in previous_entities {super::enemies::request_map_entity(entity,false)?;}
        if let Some(pending)=state.pending.take(){retire_group(&pending,world);}
        let player=world.main_player.as_mut().ok_or("角色未就绪")?;
        let entry=arrival.unwrap();let p=&player.chr_ins.modules.physics.position;
        let target=[p.0-player.block_position.x+entry[0],p.1-player.block_position.y+entry[1],p.2-player.block_position.z+entry[2]];
        match super::spirits::landing::move_native(&mut player.chr_ins.modules.physics,target) {
            Ok(())=>{
                super::native_talk::set_flag(super::travel::RESET_REQUEST,true)?;
                state.travel.as_mut().unwrap().0.reused_scene=true;
                super::log(format!("TRAVEL_REUSE move to entry={entry:?} physics={target:?}"));
            }
            Err(error)=>{reuse=false;super::log(format!("TRAVEL_REUSE placement unavailable; reloading: {error}"));}
        }
        state.reward_portal=Default::default();
    }
    state.pending = None;
    state.view.enemy_hp = None;
    state.view.travelling = true;
    state.pad_capture.update(false);
    super::input::CAPTURE.store(false, std::sync::atomic::Ordering::Relaxed);
    if reuse {
        super::log(format!("TRAVEL_REUSE block={:08x} node={id} party={}; keeping loaded scene and normal spawn barrier",arena.block,state.party.is_some()));
        report(state,"正在返回出生点并清理上一场传送门……");
    }else {
        super::spirits::prepare_warp(world,"travel_combat");
        game.move_map_target = BlockId::from(arena.block);
        game.initial_area_entity_id = arena.combat_entry();
        game.warp_requested = true;
        report(state, format!("正在前往{}，{}",arena.name,if arena.uses_circle(){"靠近场地中心自动开始战斗。"}else{"穿过雾门开始战斗。"}));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn lethal_event_damage_uses_grace_and_prevents_duplicate_rewards() {
        let mut original=Run::new(42,erdtree_tower::Class::Samurai);
        let id=original.nodes.iter().position(|n|n.kind==erdtree_tower::NodeKind::Event).unwrap();
        original.debug_jump(id).unwrap();original.enter(id).unwrap();
        original.regional_event=erdtree_tower::regional_events::Progress{opened:1,..Default::default()};
        let mut run=original.clone();
        super::regional_event_hp(&mut run,0,100,false);
        assert_eq!(run.phase,Phase::Map);
        assert_eq!(run.retry_node,Some(id));
        assert_eq!(run.retries,original.retries-1);
        assert!(run.regional_event.settled);
        assert!(super::retry_run(&run,None,true,false).is_none());
        super::regional_event_hp(&mut run,0,100,false);
        assert_eq!(run.retries,original.retries-1);assert!(run.hp>0);
        let mut loaded=Run::decode(&run.encode().unwrap()).unwrap();
        loaded.enter(id).unwrap();
        assert!(loaded.open_regional_event(0).is_err());
        loaded.leave_regional_event().unwrap();
        let mut exhausted=original.clone();exhausted.retries=0;
        super::regional_event_hp(&mut exhausted,0,100,false);
        assert_eq!(exhausted.phase,Phase::Dead);assert_eq!(exhausted.retry_node,None);
        exhausted.validate().unwrap();
        let mut alive=original.clone();
        super::regional_event_hp(&mut alive,20,100,false);
        assert_eq!(alive.phase,original.phase);assert_eq!(alive.retries,original.retries);
        let mut party=original.clone();
        super::regional_event_hp(&mut party,0,100,true);
        assert_eq!(party.phase,original.phase);assert_eq!(party.retries,original.retries);assert_eq!(party.hp,1);
    }
    #[test]
    fn member_wave_delay_survives_frames_and_resets_for_a_new_encounter() {
        use std::time::Duration as D;
        let mut delays=None;let key=(7,1,1,3);
        assert!(!super::member_gaps(&mut delays,key,2)[1].allows(D::ZERO,false));
        assert!(!super::member_gaps(&mut delays,key,2)[1].allows(D::from_secs(1),true));
        assert!(!super::member_gaps(&mut delays,key,2)[1].allows(D::from_millis(3999),true));
        assert!(super::member_gaps(&mut delays,key,2)[1].allows(D::from_secs(4),true));
        let next=super::member_gaps(&mut delays,(7,2,1,3),2);
        assert!(!next[1].allows(D::from_secs(5),false));
        assert!(!next[1].allows(D::from_secs(6),true));
        assert!(!next[1].allows(D::from_millis(8999),true));
        assert!(next[1].allows(D::from_secs(9),true));
        assert!(super::member_gaps(&mut delays,(8,2,1,3),2)[0].allows(D::from_secs(10),true));
    }
    #[test]
    fn retry_returns_to_the_dead_room_and_only_spends_one_charge() {
        use erdtree_tower::{Class,Run,Phase};
        let mut run=Run::new(42,Class::Samurai);let id=run.available()[0];
        run.enter(id).unwrap();
        let next=super::retry_run(&run,None,true,false).unwrap();
        assert_eq!(next.retry_node,Some(id));assert_eq!(next.retries,run.retries-1);
        assert_eq!(next.phase,Phase::Map);
        assert!(!crate::input::capture_route(true,true,&super::View{run:Some(next.clone()),ready:true,..super::initial_view()}));
        assert!(super::retry_run(&next,None,true,false).is_none());
        assert!(super::retry_run(&run,None,true,true).is_none());
        assert!(super::retry_run(&run,None,false,false).is_none());
        run.retries=0;assert!(super::retry_run(&run,None,true,false).is_none());
    }
    #[test]
    fn retry_keeps_destination_before_combat_starts_and_after_victory() {
        use erdtree_tower::{Class,Run};
        let mut run=Run::new(42,Class::Samurai);let id=run.available()[1];
        let travel=super::retry_run(&run,Some(id),true,false).unwrap();
        assert_eq!(travel.retry_node,Some(id));
        assert!(super::retry_run(&run,None,true,false).is_none());
        assert!(super::retry_run(&run,Some(usize::MAX),true,false).is_none());
        run.enter(id).unwrap();
        assert_eq!(super::retry_run(&run,None,true,false).unwrap().retry_node,Some(id));
    }
    #[test]
    fn retry_waits_for_resurrection_and_stable_landing() {
        use std::time::{Instant,Duration};
        let start=Instant::now();let mut since=None;
        for t in [0,1000,20000] {assert!(!super::retry_departure(&mut since,start+Duration::from_millis(t),false));}
        assert!(!super::retry_departure(&mut since,start+Duration::from_millis(21000),true));
        assert!(!super::retry_departure(&mut since,start+Duration::from_millis(21499),true));
        assert!(!super::retry_departure(&mut since,start+Duration::from_millis(21500),false));
        assert!(!super::retry_departure(&mut since,start+Duration::from_millis(22000),true));
        assert!(super::retry_departure(&mut since,start+Duration::from_millis(22500),true));
    }
    #[test]
    fn flask_sync_resumes_on_arrival_before_entering_the_spawn_circle() {
        let mut travel=crate::travel::Travel::new(0);
        assert!(super::flask_loading(Some(&travel),false,false));
        travel.resources_restored=true;
        assert!(!travel.is_preparing());
        assert!(!super::flask_loading(Some(&travel),false,false),"landed outside the circle must already correct travel refills");
        assert!(super::flask_loading(Some(&travel),true,false),"another warp must still protect the ledger");
        assert!(super::flask_loading(None,false,true));
        assert!(!super::flask_loading(None,false,false));
    }
    #[test]
    fn victorious_ash_refund_survives_completed_pending_and_long_settlement() {
        let earned=std::time::Instant::now()-std::time::Duration::from_secs(120);
        let mut guard=Some(earned);
        assert!(super::ash_refund_ready(Some(true),true,&mut guard));
        assert_eq!(guard,Some(earned));
        assert!(super::ash_refund_ready(Some(true),false,&mut guard));
        assert_eq!(guard,Some(earned));
        assert!(super::ash_refund_ready(None,false,&mut guard));
        assert_eq!(guard,Some(earned));
        assert!(!super::ash_refund_ready(Some(false),false,&mut guard));
        assert_eq!(guard,Some(earned));
        guard=None;
        assert!(!super::ash_refund_ready(None,true,&mut guard),"living summons alone do not authorize a refund");
    }
    #[test]
    fn ash_tracking_includes_preparation_and_does_not_restart_at_combat() {
        assert!(super::ash_tracking_active(true,false));
        assert!(super::ash_tracking_active(true,true));
        assert!(super::ash_tracking_active(false,true));
        assert!(!super::ash_tracking_active(false,false));
    }
    #[test]
    fn frozen_ash_refund_keeps_pumping_into_next_fight_unless_dead_or_warping() {
        assert!(super::frozen_ash_refund_ready(true,false,true));
        assert!(!super::frozen_ash_refund_ready(false,false,true));
        assert!(!super::frozen_ash_refund_ready(true,true,true));
        assert!(!super::frozen_ash_refund_ready(true,false,false));
    }
    #[test]
    fn extra_summon_scope_covers_party_challenges_merchant_hunter_and_solo_scarabs() {
        assert_eq!(super::extra_summon_scope(Some((9,2,0x2d010000)),None,None),Some((9,2,0x2d010000)));
        assert_eq!(super::extra_summon_scope(None,Some((4,0x2d020000)),None),Some((4,250,0x2d020000)));
        assert_eq!(super::extra_summon_scope(None,None,Some((5,1,0x2d010000))),Some((5,201,0x2d010000)));
        assert_eq!(super::extra_summon_scope(None,None,Some((5,2,0x2d010000))),Some((5,202,0x2d010000)));
        assert_eq!(super::extra_summon_scope(None,None,None),None);
    }
    #[test]
    fn merchant_ai_stays_enabled_between_party_sync_ticks() {
        for previous_victory in [false,true] {
            assert!(!super::party_map_combat(false,previous_victory,false,false),"loading and countdown must keep AI disabled");
            for _ in 0..120 {
                assert!(super::party_map_combat(false,previous_victory,false,true),"shop challenge must survive every party sync tick");
            }
            assert!(!super::party_map_combat(false,previous_victory,false,false),"reward or departure disables AI");
            assert!(!super::party_map_combat(false,previous_victory,true,true),"party defeat takes precedence");
        }
        assert!(super::party_map_combat(true,false,false,false));
        assert!(!super::party_map_combat(true,true,false,false));
        assert!(!super::party_map_combat(true,false,true,false));
    }
    #[test]
    fn chain_room_limit_is_personal_and_updates_saved_run_copies() {
        use erdtree_tower::embers::{Ember,Gain};
        let mut host=super::fresh_runtime();
        let mut run=Run::new(42,Class::Hero);run.embers.push(Ember::Chain);
        host.view.run=Some(run.clone());host.reward_wait=Some((std::time::Instant::now(),run.clone()));
        let mut member=super::fresh_runtime();member.view.run=Some(run);
        super::record_ember_gains(&mut host,vec![Gain::Critical;9],(42,0,3));
        super::record_ember_gains(&mut member,vec![Gain::Critical;2],(42,0,3));
        assert_eq!(host.view.run.as_ref().unwrap().ember_stacks.chain,3);
        assert_eq!(host.reward_wait.as_ref().unwrap().1.ember_stacks.chain,3);
        assert_eq!(member.view.run.as_ref().unwrap().ember_stacks.chain,2);
        let restored=Run::decode(&host.view.run.as_ref().unwrap().encode().unwrap()).unwrap();
        host.view.run=Some(restored);
        super::record_ember_gains(&mut host,vec![Gain::Critical],(42,0,3));
        assert_eq!(host.view.run.as_ref().unwrap().ember_stacks.chain,3);
        super::record_ember_gains(&mut host,vec![Gain::Critical],(42,0,4));
        assert_eq!(host.view.run.as_ref().unwrap().ember_stacks.chain,4);
    }
    #[test]
    fn chain_test_only_changes_the_selected_ember_and_marks_debug() {
        use erdtree_tower::embers::{Ember,Gain};
        let mut run=Run::new(42,Class::Hero);run.embers.push(Ember::SoulEater);run.ember_stacks.souls=23;
        super::configure_chain_test(&mut run,0).unwrap();
        assert!(run.debug);assert_eq!(run.ember_stacks.chain,0);assert_eq!(run.ember_stacks.souls,23);
        assert_eq!(run.ember_stacks.gain(&run.embers,Gain::Critical),vec![Ember::Chain]);
        assert_eq!(run.ember_stacks.chain,1);
        super::configure_chain_test(&mut run,10).unwrap();
        assert_eq!(run.embers.iter().filter(|&&e|e==Ember::Chain).count(),1);
        assert_eq!(run.ember_stacks.chain,10);assert_eq!(run.ember_stacks.souls,23);
        assert_eq!(Run::decode(&run.encode().unwrap()).unwrap(),run);
        let before=run.clone();assert!(super::configure_chain_test(&mut run,999).is_err());assert_eq!(run,before);
    }
    #[test]
    fn stall_victory_is_opt_in_and_host_only() {
        for seconds in [0,59,60,3600] {
            assert!(!super::diagnostic_stall_victory(false,true,std::time::Duration::from_secs(seconds)));
            assert!(!super::diagnostic_stall_victory(true,false,std::time::Duration::from_secs(seconds)));
            assert_eq!(super::diagnostic_stall_victory(true,true,std::time::Duration::from_secs(seconds)),seconds>=60);
        }
    }
    #[test]
    fn remote_skip_closes_local_dialogue_before_awarding_and_reopening_map() {
        use erdtree_tower::{Class,NodeKind,Phase,Run,regional_events::Trial,stormveil::{Progress,Decision}};
        for trial in [false,true] {for host_first in [false,true] {
            let mut run=Run::new(42,Class::Samurai);let node=run.nodes.iter().position(|n|n.kind==NodeKind::Event).unwrap();
            run.phase=Phase::Node{id:node};run.regional_event=Default::default();
            run.regional_event.trial=trial;if !trial{run.regional_event.stormveil=Some(Progress::Choosing);}
            let decide=|r:&mut Run|if trial{r.choose_trial(Trial::Skip)}else{r.stormveil_decide(Decision::Skip)};
            for player in if host_first {[1,2]}else{[2,1]} {
                let mut dialogue_open=player==1;let mut closed=false;
                let mut next=super::prepare_regional_decision(&run,decide,||{dialogue_open=false;closed=true;Ok(())}).unwrap();
                assert!(closed&&!dialogue_open,"authoritative skip must close local NPC menus");
                next.leave_regional_event().unwrap();assert_eq!(next.phase,Phase::Map);assert_eq!(next.gold,run.gold+800);
                assert!(super::prepare_regional_decision(&next,decide,||Ok(())).is_err());
            }
            let before=run.clone();
            assert!(super::prepare_regional_decision(&run,decide,||Err("close failed".into())).is_err());
            assert_eq!(run.gold,before.gold);assert_eq!(run.phase,before.phase);
        }}
    }
    fn ui_view()->super::View {
        super::View{shared_event:false,ballots:vec![],latencies:vec![],run:Some(erdtree_tower::Run::new(42,erdtree_tower::Class::Samurai)),
            status:String::new(),busy:false,enemy_hp:None,prompt:1,ready:true,travelling:false,room:None,runes:0,boss_affixes:None,boss_alert:None,boss_affix_list:vec![],law_clock:None,build:vec![],dummy:None,stats:Default::default(),resource_alert:None}
    }
    #[test]
    fn queued_selection_is_bounded_and_consumed_once_on_the_game_thread() {
        let view=ui_view();let owner=Some((1,0));
        let mut bridge=super::UiBridge{view:None,owner:None,pending:None};
        bridge.publish(view.clone(),owner);
        assert!(bridge.submit(super::Action::Enter(0)));
        assert!(!bridge.submit(super::Action::Enter(1)));
        assert!(bridge.view.as_ref().unwrap().busy);
        bridge.publish(view.clone(),owner);
        assert!(bridge.view.as_ref().unwrap().busy);
        assert!(matches!(bridge.take(&view,owner,false),Some(super::Action::Enter(0))));
        assert!(bridge.take(&view,owner,false).is_none());
        bridge.publish(view,owner);
        assert!(!bridge.view.as_ref().unwrap().busy);
    }
    #[test]
    fn joining_and_contract_stage_changes_discard_stale_clicks() {
        let view=ui_view();let owner=Some((1,0));
        for changed in 0..6 {
            let mut bridge=super::UiBridge{view:None,owner:None,pending:None};
            bridge.publish(view.clone(),owner);
            assert!(bridge.submit(super::Action::Contract(0)));
            let mut current=view.clone();let mut next_owner=owner;
            match changed {
                0=>current.run.as_mut().unwrap().align_party_start(99).unwrap(),
                1=>next_owner=Some((2,0)),
                2=>current.run.as_mut().unwrap().phase=erdtree_tower::Phase::Dead,
                3=>current.ready=false,
                4=>current.run.as_mut().unwrap().enable_contracts(),
                _=>current.busy=true,
            }
            assert!(bridge.take(&current,next_owner,false).is_none());
            assert!(bridge.pending.is_none());
        }
    }
    #[test]
    fn teammate_vote_reopens_route_once_without_interrupting_native_menus() {
        use erdtree_tower::party_vote::Ballot;
        let mut view=ui_view();
        let remote=vec![Ballot{player:2,node:Some(0)}];
        assert!(super::remote_route_prompt(&view,&remote,1));
        assert!(!super::remote_route_prompt(&view,&remote,2));
        view.ballots=remote.clone();
        assert!(!super::remote_route_prompt(&view,&remote,1));
        let changed=vec![Ballot{player:2,node:Some(1)}];
        assert!(super::remote_route_prompt(&view,&changed,1));
        view.room=Some(crate::rooms::RoomView{message:String::new(),loading:false,completed:false});
        assert!(!super::remote_route_prompt(&view,&changed,1));
        view.room=None;view.run.as_mut().unwrap().phase=erdtree_tower::Phase::Combat{id:0};
        assert!(!super::remote_route_prompt(&view,&changed,1));
    }
    #[test]
    fn ui_reads_and_clicks_return_while_game_runtime_is_locked() {
        let state=super::STATE.lock().unwrap();
        let (sender,receiver)=std::sync::mpsc::channel();
        let worker=std::thread::spawn(move|| {
            let _=super::view();
            super::submit(super::Action::Contract(0));
            let _=super::map_canvas();
            let _=super::submit_map_edit(erdtree_tower::map_drawing::Scope{seed:42,act:1},
                erdtree_tower::map_drawing::Edit::Add(vec![erdtree_tower::map_drawing::Point{x:0,y:0};2]));
            sender.send(()).unwrap();
        });
        let returned=receiver.recv_timeout(std::time::Duration::from_millis(200));
        drop(state);
        worker.join().unwrap();
        assert!(returned.is_ok(),"UI blocked on the game runtime lock");
    }
    #[test]
    fn chariot_uses_arena_and_waits_for_loaded_car_before_thirty_seconds() {
        let route=super::parkour_route(true);
        assert_eq!(route.block,0x2d010000);assert_eq!(route.entry,45012880);
        assert_ne!(route.entry,super::parkour_route(false).entry);
        assert_ne!(route.flag,super::parkour_route(false).flag);
        use std::time::{Instant,Duration};
        let now=Instant::now();let mut clock=super::ChariotClock::Warmup(now);
        assert_eq!(clock.tick(now+Duration::from_secs(4),false),(false,false,1));
        assert_eq!(clock.tick(now+Duration::from_secs(5),false),(true,false,30));
        assert_eq!(clock.tick(now+Duration::from_secs(65),false),(true,false,30));
        assert_eq!(clock.tick(now+Duration::from_secs(66),true),(true,false,30));
        assert_eq!(clock.tick(now+Duration::from_secs(95),true),(true,false,1));
        assert_eq!(clock.tick(now+Duration::from_secs(96),true),(true,true,0));
        assert_eq!(clock.tick(now+Duration::from_secs(97),false),(true,false,30));
    }
    #[test]
    fn dead_host_spectates_until_scene_departure() {
        let mut death=super::HostDeath::default();
        let scope=Some(7);
        assert!(!death.observe(scope,Some((4502,true)),false));
        assert!(!death.observe(scope,Some((4502,true)),false));
        assert!(!death.observe(scope,None,false));
        assert!(death.observe(scope,None,true));
        assert!(death.observe(scope,Some((1110,false)),false));
    }
    #[test]
    fn host_death_recovery_and_epoch_change_clear_departure_guard() {
        let mut death=super::HostDeath::default();
        assert!(!death.observe(Some(7),Some((4502,true)),false));
        assert!(!death.observe(Some(7),Some((4502,false)),false));
        assert!(!death.observe(Some(7),Some((4502,false)),true));
        assert!(!death.observe(Some(7),Some((4502,true)),false));
        assert!(!death.observe(Some(8),Some((1110,false)),false));
        assert!(!death.observe(Some(8),Some((4502,true)),false));
        assert!(!death.observe(None,Some((1110,false)),true));
        assert!(!death.observe(Some(9),None,true));
    }
    #[test]
    fn stormveil_finish_requires_godrick_entrance_floor_and_correct_map() {
        assert!(super::stormveil_finish(0x0a000000,-202.71,81.864,314.077,true));
        assert!(!super::stormveil_finish(0x0a000000,-93.97,21.54,19.71,true));
        assert!(!super::stormveil_finish(0x0a000000,-202.71,71.864,314.077,true));
        assert!(!super::stormveil_finish(0x0a000000,-202.71,81.864,314.077,false));
        assert!(!super::stormveil_finish(0x2d000000,-202.71,81.864,314.077,true));
    }
    #[test]
    fn loaded_battle_replaces_map_even_when_ui_is_not_busy() {
        let mut state=super::fresh_runtime();
        let mut battle=erdtree_tower::Run::new(42,erdtree_tower::Class::Samurai);
        let node=battle.available()[0];
        state.view.run=Some(battle.clone());state.view.travelling=true;state.view.busy=false;
        battle.enter(node).unwrap();
        assert!(!super::publish_bound_combat(&mut state.view,&battle,false));
        assert!(super::publish_bound_combat(&mut state.view,&battle,true));
        assert_eq!(state.view.run.as_ref().unwrap().phase,battle.phase);
        assert!(!state.view.travelling);
        assert!(!super::publish_bound_combat(&mut state.view,&battle,true));
    }
    #[test]
    fn party_summon_state_survives_death_and_loading_but_not_a_new_encounter() {
        let mut cache=None;
        let fight=Some((7,0,0x2d020000));
        *super::encounter_state::<u32>(&mut cache,fight,true).unwrap()=1;
        assert!(super::encounter_state(&mut cache,fight,false).is_none());
        assert_eq!(*super::encounter_state(&mut cache,fight,true).unwrap(),1);
        assert!(super::encounter_state(&mut cache,fight,false).is_none());
        assert_eq!(*super::encounter_state(&mut cache,fight,true).unwrap(),1);
        assert_eq!(*super::encounter_state(&mut cache,Some((7,1,0x2d020000)),true).unwrap(),0);
        assert!(super::encounter_state(&mut cache,None,false).is_none());
        assert!(cache.is_none());
        assert_eq!(*super::encounter_state(&mut cache,Some((8,0,0x2d020000)),true).unwrap(),0);
    }
    #[test]
    fn changed_roster_rebuilds_only_before_start_at_hub() {
        assert!(super::rebuild_lobby(true,true,true,None));
        assert!(!super::rebuild_lobby(false,true,true,None));
        assert!(!super::rebuild_lobby(true,false,true,None));
        assert!(!super::rebuild_lobby(true,true,false,None));
        for phase in [Phase::Map,Phase::Dead,Phase::Won] {
            assert!(!super::rebuild_lobby(true,true,true,Some(&phase)));
        }
    }
    #[test]
    fn same_roster_reconnect_recovers_only_at_idle_hub() {
        let retry=super::lobby_reconnect_error(Some(erdtree_tower::party_handshake::RECONNECTED));
        assert!(retry);assert!(super::rebuild_lobby(retry,true,true,None));
        assert!(!super::rebuild_lobby(retry,false,true,None));
        assert!(!super::rebuild_lobby(retry,true,false,None));
        for phase in [Phase::Map,Phase::Combat{id:0},Phase::Node{id:0},Phase::Dead] {
            assert!(!super::rebuild_lobby(retry,true,true,Some(&phase)));
        }
        assert!(super::lobby_reconnect_error(Some(super::super::party::TEAMMATE_FAULT)));
        for error in [None,Some("队伍协议不匹配"),Some("上次联机领奖中断")] {
            assert!(!super::lobby_reconnect_error(error));
        }
    }
    #[test]
    fn failed_pending_roster_recovers_when_third_player_arrives() {
        use erdtree_tower::party_roster::Roster;
        let old=Roster{local:1,host:1,members:vec![1,2]};
        let current=Roster{members:vec![1,2,3],..old.clone()};
        assert!(super::lobby_roster_changed(None,Some(&old),Some(&current)));
        assert!(super::lobby_roster_changed(Some(&old),None,Some(&current)));
        assert!(!super::lobby_roster_changed(None,Some(&current),Some(&current)));
        assert!(!super::lobby_roster_changed(None,Some(&old),None));
        assert!(!super::lobby_roster_changed(None,None,Some(&current)));
    }
    #[test]
    fn ending_a_run_requires_active_journey_and_unblocked_host() {
        for phase in [Phase::Map,Phase::Node{id:0},Phase::Combat{id:0}] {
            assert!(super::can_end_run(Some(&phase),true,false));
            assert!(!super::can_end_run(Some(&phase),false,false));
            assert!(!super::can_end_run(Some(&phase),true,true));
        }
        for phase in [None,Some(&Phase::Dead),Some(&Phase::Won)] {assert!(!super::can_end_run(phase,true,false));}
    }
    #[test]
    fn closed_party_recovers_only_at_hub_without_active_run() {
        assert!(super::release_closed_party(true,true,true,None));
        assert!(super::release_closed_party(true,true,false,Some(&Phase::Dead)));
        assert!(super::release_closed_party(true,true,false,Some(&Phase::Won)));
        assert!(!super::release_closed_party(false,true,true,None));
        assert!(!super::release_closed_party(true,false,true,None));
        assert!(!super::release_closed_party(true,true,false,None));
        assert!(!super::release_closed_party(true,true,false,Some(&Phase::Map)));
        assert!(!super::release_closed_party(true,true,true,Some(&Phase::Map)));
    }
    #[test]
    fn reconnect_wait_allows_hub_actions_but_guards_unsynced_arena() {
        assert!(!super::reconnect_locks_actions(true,true));
        assert!(super::reconnect_locks_actions(true,false));
        assert!(!super::reconnect_locks_actions(false,false));
        assert!(!super::reconnect_locks_actions(false,true));
    }
    #[test]
    fn material_confirmation_never_applies_to_another_character_or_reward() {
        let mut old=Run::new(42,erdtree_tower::Class::Samurai);
        old.enter(old.available()[0]).unwrap();old.win_combat().unwrap();
        let pending=MaterialAward{action:Action::Claim(0),old:old.clone(),owner:(2,0),item:7500100,before:0,started:Instant::now(),path:std::path::PathBuf::new()};
        assert!(pending.matches((2,0),Some(&old)));
        assert!(!pending.matches((2,1),Some(&old)));
        assert!(!pending.matches((3,0),Some(&old)));
        assert!(!pending.matches((2,0),None));
        let mut claimed=old.clone();claimed.claim_reward(0).unwrap();
        assert!(!pending.matches((2,0),Some(&claimed)));
        let mut other=old.clone();other.seed+=1;assert!(!pending.matches((2,0),Some(&other)));
        other=old.clone();other.act+=1;assert!(!pending.matches((2,0),Some(&other)));
    }
    #[test]
    fn interrupted_award_is_never_overwritten_by_a_retry() {
        let path=std::env::temp_dir().join(format!("tower-award-test-{}-{}.json",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let run=Run::new(42,erdtree_tower::Class::Samurai);
        write_award_journal(&path,&run).unwrap();
        assert!(write_award_journal(&path,&Run::new(43,erdtree_tower::Class::Samurai)).is_err());
        assert_eq!(Run::decode(&std::fs::read_to_string(&path).unwrap()).unwrap(),run);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn switching_save_slots_rebinds_without_carrying_pending_actions() {
        assert!(!super::owner_changed(None,(0,3)));
        assert!(!super::owner_changed(Some((0,3)),(0,3)));
        assert!(super::owner_changed(Some((0,3)),(0,4)));
        assert!(super::owner_changed(Some((0,3)),(1,3)));
        let mut state=super::fresh_runtime();
        state.command=Some(super::Action::Enter(0));state.view.busy=true;state.spawn_used=true;
        assert!(state.command.is_some()&&state.view.busy);
        state=super::fresh_runtime();
        assert!(state.hub.is_none()&&state.pending.is_none()&&state.room.is_none()&&state.attendant.is_none());
        assert!(state.command.is_none()&&state.flasks.is_none()&&state.reward_wait.is_none());
        assert!(!state.view.busy&&!state.spawn_used);
    }
    use super::*;
    #[test]
    fn actual_resource_cost_rounds_up_and_recovery_caps_without_free_full_refills() {
        use erdtree_tower::events::StoryEffect;
        let sacrifice = StoryEffect {
            hp_cost_percent: 15,
            ..Default::default()
        };
        assert!(resource_result(69, 455, 82, 82, &sacrifice).is_err());
        assert_eq!(
            resource_result(70, 455, 82, 82, &sacrifice).unwrap(),
            (1, 82)
        );
        let heal = StoryEffect {
            heal_percent: 40,
            ..Default::default()
        };
        assert_eq!(resource_result(400, 455, 0, 82, &heal).unwrap(), (455, 0));
        assert!(resource_result(455, 455, 0, 82, &heal).is_err());
        let fp = StoryEffect {
            fp_percent: 60,
            ..Default::default()
        };
        assert_eq!(resource_result(455, 455, 50, 82, &fp).unwrap(), (455, 82));
        assert!(resource_result(455, 455, 82, 82, &fp).is_err());
    }

    #[test]
    fn generated_enemy_in_ready_state_can_complete_combat() {
        assert!(enemy_loaded(ChrLoadStatus::ReadyForActivation));
        assert!(enemy_loaded(ChrLoadStatus::Active));
        for status in [
            ChrLoadStatus::Unloaded,
            ChrLoadStatus::Initializing,
            ChrLoadStatus::NetworkInitializing,
            ChrLoadStatus::Unloading,
        ] {
            assert!(!enemy_loaded(status));
        }
        let mut fight = Encounter::<u64>::default();
        assert!(fight.bind(7, enemy_loaded(ChrLoadStatus::ReadyForActivation)));
        assert_eq!(
            fight.poll(false, Observation::Alive, true),
            Outcome::Waiting
        );
        assert_eq!(fight.poll(false, Observation::Dead, true), Outcome::Victory);
    }
    use erdtree_tower::Class;
    #[test]
    fn only_reachable_first_encounter_can_start() {
        let run = Run::new(42, Class::Vagabond);
        assert!(start_allowed(&run, 0, false).is_ok());
        assert!(start_allowed(&run, 1, false).is_ok());
        assert!(start_allowed(&run, 2, false).is_ok());
        assert!(start_allowed(&run, run.start_nodes().len(), false).is_err());
        assert!(start_allowed(&run, 0, true).is_err());
    }
    #[test]
    fn living_player_never_rounds_down_to_dead() {
        assert_eq!(normalized_hp(1, 9999), 1);
        assert_eq!(normalized_hp(0, 9999), 0);
        assert_eq!(normalized_hp(123, 123), 100);
        assert_eq!(normalized_hp(500, 100), 100);
    }
}
