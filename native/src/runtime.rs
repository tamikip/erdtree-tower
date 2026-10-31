use std::{
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use eldenring::{
    cs::{
        BlockId, CSDebugChrCreatorInitData, ChrDebugSpawnRequest, ChrLoadStatus, EventWorldType,
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
pub static PLAYER_READY:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
fn can_end_run(phase:Option<&Phase>,host:bool,blocked:bool)->bool {
    host&&!blocked&&phase.is_some_and(|p|!matches!(p,Phase::Dead|Phase::Won))
}
fn victory_healing_allowed(player:&eldenring::cs::PlayerIns)->bool {
    player.chr_ins.modules.data.hp>0&&!player.chr_ins.chr_flags1c5.death_flag()
        &&!super::seamless_revival::finished(player as *const _ as usize)
}
/// 战斗胜利的回复（狂战士 8% 生命、祷誓者 25% 专注、苦修之誓）：写日志，返回给玩家看的提示（没有回复时为空）
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
}
fn initial_view()->View {
    View{shared_event:false,ballots:vec![],run:None,status:"进入角色后，从金色入口开始旅程。".into(),
        busy:false,enemy_hp:None,prompt:0,ready:false,travelling:false,room:None,runes:0,boss_affixes:None,boss_alert:None,boss_affix_list:vec![],law_clock:None,build:vec![],dummy:None,stats:Default::default()}
}

struct Foe {
    spec: super::enemies::Enemy,
    fight: Encounter<FieldInsHandle>,
    requested: Option<Instant>,
    entity_address: Option<usize>,
    /// 双王共命：在这里（尸体的位置）重新生成
    respawn: Option<[f32;3]>,
}
struct Pending {
    entity_base:Option<i32>,
    networked:bool,
    spirits:super::spirits::Spirits,
    foes: Vec<Foe>,
    next_run: Run,
    block: i32,
    completed: bool,
    // [fix] Since when every remaining live foe has sat untouched at full HP after the others died.
    stall: Option<Instant>,
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
    let Kind::Guards(_)=job.kind else{return Ok(());};
    let Phase::Node{id}=scene.run.phase else{return Err("事件场景进度无效".into());};
    state.view.shared_event=true;state.command=None;
    if let Some(room)=state.room.as_mut(){room.suspend_dialogues()?;}
    state.pad_capture.update(false);super::input::CAPTURE.store(false,std::sync::atomic::Ordering::Relaxed);
    if scene.wave!=job.wave {
        if let Some(guards)=scene.guards.take(){guards.retire();}
        scene.wave=job.wave;scene.started=Instant::now();
    }
    if link.event.done.is_some() {
        let run=state.view.run.as_mut().ok_or("个人事件进度未加载")?;
        if job.player==link.config.player&&(run.phase!=(Phase::Node{id})||!run.regional_event.guards){return Err("个人守卫事件与结算不匹配".into());}
        // [fix] The guard fight is shared: it also settles any guard outcome other players are still
        // waiting on in this room, so their queued waves are never started.
        if run.phase==(Phase::Node{id})&&run.regional_event.guards {
            run.regional_event.guards=false;run.regional_event.guard_slot=None;record_checked(run)?;
            if matches!(link.event.request,Some(Kind::Guards(_))){link.event.request=None;}
        }
        if let Some(guards)=scene.guards.take(){guards.retire();}
        let healed=heal_victory(world,&scene.run);if !healed.is_empty(){super::log(&healed);state.view.status=healed;state.view.prompt=state.view.prompt.wrapping_add(1);}
        // 召唤师「群灵」：与单人的守卫战相同，胜利时召唤物还活着就补回这次用掉的骨灰
        if keep_summoned_ash(world,&scene.run,state.summon_seen,&state.ash_before,state.ash_used) {state.ash_guard=Some(Instant::now());}
        link.event.applied=Some(job.wave);state.view.shared_event=false;return Ok(());
    }
    if game.warp_requested||world.main_player.as_ref().is_none_or(|p|i32::from(p.current_block_id)!=scene.block){return Ok(());}
    let altar=scene.run.altar(id);
    let roster=super::enemies::roster(if altar {erdtree_tower::NodeKind::Elite}else{erdtree_tower::NodeKind::Monster},scene.run.seed,id,scene.run.nodes[id].layer*11/(scene.run.floors()-1),scene.run.act);
    if roster.is_empty()||roster.len()>16{return Err("事件守卫数量无效".into());}
    if scene.guards.is_none(){scene.guards=Some(super::map_guards::Guards::new(roster)?);}
    let guards=scene.guards.as_mut().unwrap();
    guards.prepare(world)?;
    if guards.ready(){link.event.bound=Some(job.wave);}
    if link.host()&&!link.event.hunting&&link.gate.event_bound(job.wave){link.event_hunt(job.wave)?;}
    let defeated=guards.poll(world,link.event.hunting,link.host())?;
    if link.host()&&defeated&&link.gate.event_bound(job.wave){link.event_result(job.wave,0)?;}
    if !link.event.hunting&&scene.started.elapsed()>Duration::from_secs(90){
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
        if job.player==link.config.player {
            let run=state.view.run.as_mut().ok_or("个人事件未加载")?;
            if run.phase!=(Phase::Node{id})||!run.regional_event.scarabs||run.regional_event.attempts!=attempt{return Err("个人粪金龟次数与结算不匹配".into());}
            if link.event.aborted {
                let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
                let data=unsafe {player.player_game_data.as_mut()};
                let refunded=data.rune_count.checked_add(1000).ok_or("退款超过卢恩上限")?;
                write_award_journal(&award_path()?,run)?;
                let mut next=run.clone();next.regional_event.attempts-=1;next.gold=refunded;
                data.rune_count=refunded;state.view.runes=refunded;record_checked(&next)?;*run=next;
                finish_party_award()?;
            }else if killed!=0 {
                write_award_journal(&award_path()?,run)?;
                let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
                for slot in 0..3 {if killed&(1<<slot)!=0 {
                    super::loot::regional_reward(unsafe {player.player_game_data.as_mut()},run,run.seed^0x5ca4ab,id*16+usize::from(attempt)*3+slot)?;
                }}
                record_checked(run)?;finish_party_award()?;
            }
        }
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
        hunt.poll(world)?;
        if link.host(){if let Some(killed)=hunt.result(){link.event_result(job.wave,killed)?;}}
    }
    if scene.started.elapsed()>Duration::from_secs(90)&&link.host()&&link.event.done.is_none(){link.event_abort(job.wave)?;}
    if scene.started.elapsed()>Duration::from_secs(120)&&link.event.done.is_none(){return Err("未收到粪金龟结算，已暂停重发".into());}
    Ok(())
}

struct Runtime {
    load_trace:super::diagnostics::LoadTrace,
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
    /// 最近一次看到召唤物在场的时间（召唤师「群灵」：胜利前一两秒还在也算活着）
    summon_seen: Option<Instant>,
    /// 召唤师胜利后的一小段时间：召唤物离场后才变成“用掉”的骨灰也补回，而不是清理掉
    ash_guard: Option<Instant>,
    ash_tracking:bool,
    ash_before:Vec<(u32,u32)>,
    ash_used:Option<u32>,
    ash_refund_requested:bool,
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
    ember_fight: super::embers::Fight,
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
    action_guard: super::input::ActionGuard,
    last_blessing_sync: Option<Instant>,
    room: Option<super::rooms::Room>,
    event_start: Option<Instant>,
    descent: Option<(Instant,i32,i32,bool)>,
    stormveil: Option<(Instant,i32,i32,bool)>,
    stormveil_barrier:super::parkour_barrier::Barrier,
    chariot_clock:Option<ChariotClock>,
    chariot_chase:super::chariot::Chase,
}

fn fresh_runtime()->Runtime {
    super::seamless_revival::cancel();
    let (party,party_error)=match super::party::Link::load(){Ok(p)=>(p,None),Err(e)=>(None,Some(e))};
    Runtime {
        drawing:Default::default(),drawing_owner:None,
        rest_end:Default::default(),
        diagnostics:Default::default(),
        diagnostic_probe:None,
        host_death:Default::default(),
        party_death:Default::default(),
        party_death_wave:None,
        party_spirits:None,
        party_scene:None,
        party,party_error,party_pending:None,departure:None,
        attendant:None,
        hub: None,
        last_save: None,
        last_saved: None,
        view: initial_view(),
        command: None,
        identity: None,
        hub_failed: None,
        ashes_checked: None,
        summon_seen: None,
        ash_guard: None,
        ash_tracking:false,
        ash_before:vec![],
        ash_used:None,
        ash_refund_requested:false,
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
        ember_fight: Default::default(),
        bout: Default::default(),
        pending: None,
        spawn_used: false,
        dummy: None,
        last_player_probe: None,
        load_trace:Default::default(),
        pad_capture: Default::default(),
        travel: None,
        flasks: None,
        flask_grant_requested: None,
        reward_wait: None,
        reward_portal:Default::default(),
        victory_cleanup:None,
        lingering:None,
        action_guard: Default::default(),
        last_blessing_sync: None,
        room: None,
        event_start: None,
        descent: None,
        stormveil: None,
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
fn record_checked(run: &Run)->Result<(),String> {
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
fn apply_party_order(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan,order:erdtree_tower::party_game::Order)->Result<(),String> {
    use erdtree_tower::party_game::Order;
    if matches!(order,Order::Enter{epoch,..}|Order::BossTest{epoch,..} if epoch==state.party.as_ref().unwrap().epoch){return Ok(());}
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
        Order::BossTest{test,..}=>begin_boss_test(state,world,game,test),
        Order::Law{seed,law,..}=>{
            let run=state.view.run.as_ref().ok_or("律法同步时旅程尚未开启")?;
            if run.seed!=seed{return Err("律法同步路线不一致".into());}
            if run.law==law{return Ok(());}
            let mut next=run.clone();next.set_law(law)?;
            if let Some(flasks)=state.flasks.as_mut() {
                let data=unsafe {world.main_player.as_mut().ok_or("律法同步时角色未加载")?.player_game_data.as_mut()};
                flasks.set_bonus(data,next.class,next.flask_bonus,law.flask_cut())?;
                flasks.restore(data,next.flasks)?;
                next.flasks=flasks.total().min(next.flask_limit);
            }
            record_checked(&next)?;state.view.run=Some(next);
            report(state,format!("已同步房主苦难律法：{} 分，律法码 {}",law.score(),law.code()));Ok(())
        },
        Order::ForceEnd=>{
            super::log("HOME_SYNC received host force end; scheduling terminal settlement");
            QUICK_END.store(true,std::sync::atomic::Ordering::Relaxed);Ok(())
        },
        Order::NewRun{..}=>{
            if state.view.run.is_some()||!state.hub.as_ref().is_some_and(|h|h.active) {return Err("新局同步时角色尚未返回大赐福".into());}
            stop_map_combat(state,world);
            state.pending=None;state.party_scene=None;state.party_spirits=None;state.room=None;state.attendant=None;
            state.reward_wait=None;state.reward_portal=Default::default();state.travel=None;state.event_start=None;
            state.spawn_used=false;state.command=None;state.view.ballots.clear();state.view.shared_event=false;
            state.view.busy=false;state.view.travelling=false;state.view.enemy_hp=None;
            report(state,"队伍已重置，可以开始新的旅程。");Ok(())
        },
        Order::Enter{node,..}=>{
            super::native_talk::set_flag(super::enemies::MAP_COMBAT_FLAG,false)?;
            if let Some(scene)=state.party_scene.take(){scene.retire(world);}
            state.view.shared_event=false;
            request_travel(state,world,game,node)
        },
        Order::Defeat{..}=>{
            stop_map_combat(state,world);
            // 全队倒下：房主与成员都记下这场战斗（单人在战斗结算时记）
            if let Some(run)=state.view.run.as_ref()&&let Phase::Combat{id}=run.phase {record_fight(run,Some(id),false);}
            if let Some(run)=state.view.run.as_mut(){run.end_on_death();record_checked(run)?;}
            Ok(())
        },
        Order::Stop(error)=>{stop_map_combat(state,world);Err(error)},
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
    error.is_some_and(|error|error=="队员已重新连接，需要重新建立会话"||error.starts_with("队友报告游戏执行失败"))
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
    let reconnect=lobby_reconnect_error(state.party_error.as_deref().or_else(||state.party.as_ref().and_then(|p|p.stopped.as_deref())));
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
fn sync_action_guard(state:&mut Runtime,world:&mut WorldChrMan) {
    let menu_active=super::input::capture_actions(state.view.shared_event,super::input::active(),
        state.room.as_ref().is_some_and(|r|r.dialog_active())
        || state.attendant.as_ref().is_some_and(|r|r.dialog_active())
        || state.hub.as_ref().is_some_and(|h|h.dialog_active()));
    if let Some(player)=world.main_player.as_mut() {state.action_guard.update(&mut player.chr_ins,menu_active);}
}
fn poll_blocked_hub(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan) {
    sync_action_guard(state,world);
    if game.warp_requested||!world.main_player.as_ref().is_some_and(|p|i32::from(p.current_block_id)==super::hub::BLOCK){return;}
    if let Some(mut hub)=state.hub.take() {
        if hub.active {
            let reason=party_block_reason(state);
            if let Err(error)=hub.poll(world,game,Some(&reason)){super::log_repeating(format!("Blocked hub service: {error}"));}
            state.view.room=Some(hub.view());
        }
        state.hub=Some(hub);
    }
}
fn party_block_reason(state:&Runtime)->String {
    if let Some(error)=state.party_error.as_ref().or_else(||state.party.as_ref().and_then(|p|p.stopped.as_ref())){return error.clone();}
    if let Some(pending)=&state.party_pending{return pending.waiting_reason();}
    if state.hub.as_ref().is_some_and(|h|h.paused){return "本机保留了暂停进度，但仍在无缝房间中。请先退出旧房间，再继续旅程。".into();}
    if state.party.as_ref().is_some_and(|p|p.needs_new_run()){return "上一局已经结束，尚未收到全员存活并返回大赐福的确认。请所有玩家完成结算并回到大赐福。".into();}
    "尚未收到队伍就绪确认，当前无法确定具体原因。请保留 tower-preview.log 进行排查。".into()
}
fn tick_auto_party(state:&mut Runtime)->Result<bool,String> {
    use eldenring::cs::{CSSessionManager,LobbyState};
    if state.departure.is_some(){return Ok(false);}
    if state.hub.as_ref().is_some_and(|h|h.paused||h.settling()){return Ok(false);}
    if super::party::manual_config().is_some()||!super::seamless::active(){return Ok(false);}
    let Ok(session)=(unsafe {CSSessionManager::instance()}) else{return Ok(state.party_pending.is_some());};
    if let Some(link)=state.party.as_ref().filter(|p|p.roster().is_some()) {
        if !matches!(session.lobby_state,LobbyState::Host|LobbyState::Client) {return Err("无缝会话已退出，队伍同步已暂停".into());}
        if let Ok(roster)=super::seamless::roster(session) {
            if link.roster()!=Some(&roster){return Err("无缝队伍成员已变化，暂停当前旅程同步".into());}
        }
        return Ok(false);
    }
    if session.lobby_state==LobbyState::None {
        state.party_pending=None;
        return Ok(false);
    }
    let Ok(roster)=super::seamless::roster(session) else{return Ok(state.party_pending.is_some());};
    if roster.members.len()<2 {state.party_pending=None;return Ok(false);}
    if state.party_pending.as_ref().is_some_and(|p|p.roster!=roster){state.party_pending=None;}
    if state.party_pending.is_none() {
        if state.view.run.as_ref().is_some_and(|r|r.act!=1||!r.visited.is_empty()||r.phase!=Phase::Map) {
            return Err("请在大赐福组队后开始共同旅程；中途加入尚未完成进度衔接".into());
        }
        let nonce=super::party::random_entropy()?.max(1);
        let seed=state.view.run.as_ref().map(|r|r.seed).unwrap_or(super::party::random_entropy()?);
        state.party_pending=Some(super::steam_messages::Pending::new(roster,nonce,seed)?);
        let pending=state.party_pending.as_ref().unwrap();
        super::log(format!("Steam party handshake started: host={} members={}",pending.roster.local==pending.roster.host,pending.roster.members.len()));
    }
    if let Some((session,seed))=state.party_pending.as_mut().unwrap().poll()? {
        let mut aligned=state.view.run.clone();
        if let Some(run)=&mut aligned {run.align_party_start(seed)?;record_checked(run)?;}
        let link=super::party::Link::from_steam(state.party_pending.take().unwrap(),session,seed)?;
        state.view.run=aligned;state.party=Some(link);state.view.ballots.clear();state.command=None;
        super::log("Steam party handshake committed; shared runtime link enabled");
        return Ok(false);
    }
    Ok(true)
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
        if super::hub::path(identity,"party-award").exists(){return Err("上次联机领奖中断，已停止自动重发；请保留日志和 party-award 记录核对背包".into());}
    }
    let alive=world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.data.hp>0&&!p.chr_ins.chr_flags1c5.death_flag());
    let present=world.main_player.is_some()&&!game.warp_requested;
    let at_hub=present&&state.view.run.is_none()&&state.hub.as_ref().is_some_and(|h|h.ready_for_party_restart())
        &&world.main_player.as_ref().is_some_and(|p|i32::from(p.current_block_id)==super::hub::BLOCK&&p.chr_ins.modules.physics.standing_on_solid_ground);
    let total=state.flasks.as_ref().map(|f|f.total());
    {let link=state.party.as_mut().unwrap();link.law_flasks.observe(link.epoch,present&&alive&&link.fighting&&!link.victory,total);}
    let orders=state.party.as_mut().unwrap().poll(if present {state.view.run.as_ref()}else{None},alive,at_hub,state.rest_end.requested);
    for order in orders {apply_party_order(state,world,game,order)?;}
    sync_merchant_reward(state)?;
    if QUICK_END.load(std::sync::atomic::Ordering::Relaxed){return Ok(());}
    tick_revival(state,world,game)?;
    if state.party.as_ref().unwrap().defeated {return Ok(());}
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
    super::native_talk::set_flag(super::enemies::MAP_COMBAT_FLAG,link.fighting&&!link.victory&&!link.defeated)?;
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
                // 精英成双：与房主同一规则，前面的精英还没倒下（或没到半血）时，后面的精英先不放出来，也不等它
                let held=member_held(world,run,id,&roster,&ids);
                let mut ready=vec![false;roster.len()];
                for (slot,&entity) in ids.iter().enumerate() {
                    if held[slot] {continue;}
                    super::enemies::position_map_entity(entity,slot)?;
                    ready[slot]=super::enemies::request_map_entity(entity,true)?;
                }
                if !link.bound {
                    let mut seen=held.clone();
                    for chr in world.chr_sets.iter().flatten().flat_map(|set|set.characters()) {
                        for (slot,spec) in roster.iter().enumerate(){
                            if ready[slot]&&chr.event_entity_id==ids[slot]&&chr.npc_id==spec.model&&chr.npc_param_id==spec.npc
                                &&chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag()&&!chr.debug_flags.character_disabled()
                                &&enemy_loaded(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status){seen[slot]=true;}
                        }
                    }
                    link.bound=held.iter().any(|h|!h)&&seen.iter().all(|x|*x);
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
            super::spirits::set_active(link.bound&&!link.victory&&alive&&!run.law.lone_wolf());
            if link.victory&&alive&&state.reward_wait.is_none(){
                // 不马上撤敌人：房主在生命归零时就判定胜利，被处决的敌人要等处决动画播完才真正死亡；
                // 在本机死亡之前撤掉，成员就拿不到这只敌人的卢恩（看起来直接消失、不掉魂）
                state.victory_cleanup=Some((link.epoch,super::enemies::map_entities(&super::enemies::node_roster(run,id))?,Instant::now(),false));
                state.reward_wait=Some((Instant::now(),run.clone()));state.view.busy=true;
                state.reward_portal=Default::default();
                let healed=heal_victory(world,run);if !healed.is_empty(){super::log(&healed);state.view.status=healed;state.view.prompt=state.view.prompt.wrapping_add(1);}
                // 召唤师「群灵」：与房主、单人相同，胜利时召唤物还活着就补回这次用掉的骨灰
                if keep_summoned_ash(world,run,state.summon_seen,&state.ash_before,state.ash_used) {state.ash_guard=Some(Instant::now());}
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
    let stray:Vec<u32>=world.chr_sets.iter().flatten().flat_map(|set|set.characters()).filter(|chr|ids.contains(&chr.event_entity_id)
        &&chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag()&&!chr.debug_flags.character_disabled()&&enemy_loaded(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status))
        .map(|chr|chr.event_entity_id).collect();
    for &entity in &stray {if let Err(error)=super::enemies::request_map_entity(entity,false){super::log(error);}}
    if !stray.is_empty() {super::log(format!("Party victory: removed {} enemies still alive locally {:?}",stray.len(),stray));}
}
fn stray_sweep_due(waited:Duration)->bool {waited>=Duration::from_secs(8)}
/// 本局结束后留在场上的敌人：玩家活着时（复活回到大赐福，或通关）5 秒后撤下，最多留 15 秒
fn lingering_due(alive:bool,waited:Duration)->bool {alive&&waited>=Duration::from_secs(5)||waited>=Duration::from_secs(15)}
#[cfg(test)]
mod victory_cleanup_tests {
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
    let victory=link.is_some_and(|p|p.victory||p.event.victory()||p.event.merchant_phase==3);
    if epoch.is_some()&&victory {super::seamless_revival::tick(world,game);}
    let player=world.main_player.as_ref().map(|p|{
        let pos=&p.chr_ins.modules.physics.position;
        (Position{block:i32::from(p.current_block_id),xyz:[pos.0,pos.1,pos.2]},p.chr_ins.modules.data.hp<=0||p.chr_ins.chr_flags1c5.death_flag())
    });
    let recovered=world.main_player.as_ref().is_some_and(|p|super::seamless_revival::finished(super::seamless_revival::owned_address(p)));
    match state.party_death.observe_confirmed(epoch,player,victory,game.warp_requested,recovered) {
        Recovery::RequestRevival(origin)=>{
            if let Some(p)=world.main_player.as_ref().filter(|p|i32::from(p.current_block_id)==origin.block) {
                super::seamless_revival::request(p,origin)?;
                super::log(format!("PARTY_REVIVE requested epoch={epoch:?} origin={origin:?}"));
            }
        },
        Recovery::FinishRevival=>{
            super::log(format!("PARTY_REVIVE alive at death position epoch={epoch:?}"));
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

// All engine references stay within the ChrIns_PostPhysics callback.
fn solo_session()->bool {
    use eldenring::cs::{CSSessionManager,LobbyState};
    if !super::seamless::active(){return true;}
    unsafe {CSSessionManager::instance()}.ok().is_some_and(|s|s.lobby_state==LobbyState::None
        ||super::seamless::roster(s).is_ok_and(|r|r.local==r.host&&r.members.len()==1))
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
    stop_map_combat(state,world);
    if let Some(room)=state.room.take(){room.retire(world);}
    if let Some(scene)=state.party_scene.take(){scene.retire(world);}
    if let Some(pending)=state.pending.take(){retire_group(&pending,world);}
    if let Some(link)=state.party.take() && link.host() {
        super::log("HOME_SYNC host ending party; retaining transport for five seconds");
        let mut departure=super::party::Departure::new(link);departure.tick();
        state.departure=Some(departure);
    }
    state.party_pending=None;state.party_error=None;state.command=None;
    state.rest_end=Default::default();state.host_death=Default::default();
    state.party_death=Default::default();state.party_death_wave=None;super::seamless_revival::cancel();
    state.reward_wait=None;state.travel=None;state.flasks=None;state.event_start=None;state.descent=None;state.stormveil=None;state.chariot_clock=None;
    clear_chariot()?;
    for flag in [45020970,45020971,10007990,30097990,35007990,45007990,super::map_guards::COMBAT] {let _=super::native_talk::set_flag(flag,false);}
    state.view.run=None;state.view.room=None;state.view.shared_event=false;state.view.busy=false;state.view.travelling=false;state.view.ballots.clear();
    state.identity=None;state.pad_capture.update(false);super::input::CAPTURE.store(false,std::sync::atomic::Ordering::Relaxed);
    super::log("HOME_ESCAPE: run ended; continuation cleared; returning to hub");
    report(state,"已强制结束本局，正在返回大赐福。");
    Ok(true)
}
pub fn tick() {
    if let Ok(mut state)=STATE.try_lock(){sync_map(&mut state);}
    tick_locked();
    if let Ok(mut state)=STATE.try_lock() {
        if let Ok(world)=unsafe {WorldChrMan::instance_mut()}{sync_action_guard(&mut state,world);}
        sync_map(&mut state);
        let view=state.view.clone();let owner=state.identity;
        drop(state);
        if let Ok(mut ui)=UI.lock(){ui.publish(view,owner);}
    }
}
fn tick_locked() {
    super::star_stream::clear();
    super::spirits::begin_frame();
    let Ok(mut state) = STATE.try_lock() else {
        return;
    };
    if state.departure.as_mut().is_some_and(|departure|departure.tick()){state.departure=None;}
    // 调试的无敌、秒杀：联机（含无缝联机）时一律关掉
    super::debug_menu::apply_cheats(state.party.is_some()||super::seamless::active());
    let lobby=state.view.run.is_none();
    if state.party.as_mut().is_some_and(|link|link.poll_force_end(lobby)) {
        super::log("HOME_SYNC received host force end before room processing");
        QUICK_END.store(true,std::sync::atomic::Ordering::Relaxed);
    }
    let action=UI.lock().ok().and_then(|mut ui|ui.take(&state.view,state.identity,state.command.is_some()));
    if let Some(action)=action {state.command=Some(action);state.view.busy=true;}
    let mut music=super::bgm::Frame::default();
    state.pad_capture.update(super::input::active());
    let Ok(game) = (unsafe { GameMan::instance_mut() }) else {
        super::boss_bars::sync(Vec::new());
        PLAYER_READY.store(false,std::sync::atomic::Ordering::Relaxed);
        state.view.ready = false;
        return;
    };
    if super::seamless::blocked(game.is_in_online_mode,super::seamless::active()) {
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
        super::boss_bars::sync(Vec::new());
        PLAYER_READY.store(false,std::sync::atomic::Ordering::Relaxed);
        state.view.ready = false;
        return;
    };
    PLAYER_READY.store(world.main_player.is_some()&&game.save_slot>=0,std::sync::atomic::Ordering::Relaxed);
    let chase_enabled=!game.warp_requested&&matches!(state.chariot_clock,Some(ChariotClock::Surviving(_)))
        &&state.view.run.as_ref().is_some_and(|r|r.regional_event.chariot&&r.regional_event.stormveil==Some(erdtree_tower::stormveil::Progress::Running));
    let Runtime{chariot_chase,party,..}=&mut *state;
    if let Err(error)=chariot_chase.tick(world,party.as_mut(),chase_enabled){super::log(error);}
    if state.hub.is_none()&&world.main_player.as_ref().is_some_and(|p|{
        let owner=(unsafe {p.player_game_data.as_ref()}.character_id,game.save_slot);
        game.save_slot>=0&&super::hub::path(owner,"paused").exists()
    }) {QUICK_END.store(true,std::sync::atomic::Ordering::Relaxed);}
    if QUICK_END.load(std::sync::atomic::Ordering::Relaxed) {
        match emergency_end(&mut state,world,game) {
            Ok(true)=>{QUICK_END.store(false,std::sync::atomic::Ordering::Relaxed);},
            Ok(false)=>{sync_action_guard(&mut state,world);return;},
            Err(error)=>{QUICK_END.store(false,std::sync::atomic::Ordering::Relaxed);report(&mut state,format!("结算失败：{error}；可再次按 {} 重试",super::options::label(erdtree_tower::options::Hotkey::Settle)));sync_action_guard(&mut state,world);return;},
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
        let visible=unsafe {eldenring::cs::CSMenuManImp::instance()}.ok().map(|menu| {
            let mut bits=[0u64;2];for (i,s) in menu.ui_states.iter().enumerate(){if s.visible(){bits[i/64]|=1u64<<(i%64);}}bits
        });
        let pos=world.main_player.as_ref().map(|p|{let v=&p.chr_ins.modules.physics.position;[v.0,v.1,v.2]});
        let detail=format!("player={player:?} grounded={grounded} position={pos:?} ui_visible={visible:x?} party={party:?}");
        let trigger=game.warp_requested||state.view.travelling;
        if let Some(line)=state.load_trace.observe(now,trigger,stage,&detail){super::log(line);}
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
    if let Some(error)=state.party_error.clone(){state.view.ready=false;state.command=None;report(&mut state,error);poll_blocked_hub(&mut state,world,game);return;}
    let party_waiting=match tick_auto_party(&mut state) {
        Ok(waiting)=>waiting,
        Err(error)=>{
            if let Some(link)=state.party.as_mut(){link.stop(error.clone());}
            state.party_error=Some(error.clone());state.view.ready=false;state.command=None;report(&mut state,error);poll_blocked_hub(&mut state,world,game);return;
        },
    };
    if party_waiting {state.view.ready=false;state.command=None;}
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
    if QUICK_END.load(std::sync::atomic::Ordering::Relaxed){sync_action_guard(&mut state,world);return;}
    if let Some(player)=world.main_player.as_ref() {
        let current=(unsafe {player.player_game_data.as_ref()}.character_id,game.save_slot);
        if (game.event_world_type==EventWorldType::Local||super::seamless::active())&&game.save_slot>=0&&owner_changed(state.hub.as_ref().map(|h|h.owner),current) {
            super::log(format!("Character changed: clearing prior runtime, binding {current:?}"));
            *state=fresh_runtime();
            if let Ok(mut path)=SESSION_PATH.lock(){*path=None;}
            super::input::CAPTURE.store(false,std::sync::atomic::Ordering::Relaxed);
        }
    }
    let summon_scope=state.party.as_ref().and_then(|link|{
        if link.event.combat_active() {
            state.party_scene.as_ref().map(|scene|(link.epoch,scene.wave,scene.block))
        }else if !link.host()&&link.bound&&!link.victory {
            state.view.run.as_ref().filter(|run|matches!(run.phase,Phase::Combat{..})).map(|run|(link.epoch,0,super::arenas::for_run(run,true).block))
        }else {None}
    });
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
    }
    super::spirits::set_active(local_combat&&!super::law::lone_wolf(state.view.run.as_ref()));
    super::spirits::set_twin(local_combat&&state.pending.as_ref().map(|p|&p.next_run).or(state.view.run.as_ref()).is_some_and(|r|r.embers.contains(&erdtree_tower::embers::Ember::SpiritTwin)));
    super::enemies::set_party(state.party.is_some());
    // 苦难律法「孤狼」：与房主、单人相同，联机成员与守卫战也不开放召唤点
    let lone_wolf=super::law::lone_wolf(state.view.run.as_ref());
    if let Some(spirits)=encounter_state(&mut state.party_spirits,summon_scope,summon_available&&!lone_wolf) {
        if let Err(error)=spirits.poll(world,summon_scope.unwrap().2,true){super::log(format!("Party spirit registration: {error}"));}
    }
    let bar_affixes=state.pending.as_ref().map(|p|&p.next_run).or(state.view.run.as_ref())
        .and_then(|run|if let Phase::Combat{id}=run.phase {Some(erdtree_tower::boss_affixes::roll(run,id))}else{None}).unwrap_or_default();
    let mut bars:Vec<_>=if music_active {state.pending.as_ref().map(|pending|pending.foes.iter().filter_map(|foe|{
        let handle=foe.fight.handle()?;let chr=world.chr_ins_by_handle(&handle)?;
        if foe.entity_address!=Some(std::ptr::from_ref(chr) as usize)||chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(){return None;}
        let rank=super::enemies::bar_rank(&foe.spec);let name=super::enemies::bar_text(&foe.spec)?;
        Some((rank,if rank==0 {super::boss_affixes::bar_name(&bar_affixes,name)}else{name},handle))
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
                        bars.push((rank,if rank==0 {super::boss_affixes::bar_name(&bar_affixes,name)}else{name},chr.field_ins_handle));
                    }
                }
            }
        }}
    }
    super::boss_bars::sync(super::boss_bars::select(bars));
    if !game.warp_requested {member_combat(&mut state,world);}
    // [fix] Diagnostic switch: ERDTREE_TOWER_NO_ATTENDANT=1 skips the arena-side NPC entirely, to test
    // whether it is what triggers the arena-arrival crash. The fog gate and fight do not need it.
    let attendant_allowed=!attendant_disabled()&&state.room.is_none()&&!game.warp_requested
        &&world.main_player.as_ref().is_some_and(|p|p.chr_ins.modules.data.hp>0&&if super::arenas::for_block(i32::from(p.current_block_id)).is_some_and(|a|a.uses_circle()) {
            state.reward_wait.is_some()&&super::native_talk::flag(45020970).unwrap_or(false)
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
    if camp {
        let clock=super::weapon_glow::clock();
        let act=state.view.run.as_ref().map_or(1,|r|r.act);
        let dummy=state.dummy.get_or_insert_with(Default::default);
        if let Err(error)=dummy.tick(world,clock,act){super::log_repeating(format!("Damage dummy: {error}"));}
        state.view.dummy=Some(dummy.view(clock));
    } else if let Some(dummy)=state.dummy.take() {dummy.retire(world);state.view.dummy=None;}
    let contract_run=state.view.run.as_ref().filter(|_|state.hub.as_ref().is_some_and(|h|!h.active));
    if let Some(player)=world.main_player.as_mut(){let light=super::contracts::light_load(unsafe {player.player_game_data.as_ref()});super::contracts::sync(&mut player.chr_ins,contract_run,light);super::elite::sync(&mut player.chr_ins,contract_run);super::passives::sync(&mut player.chr_ins,contract_run);super::auras::sync(&mut player.chr_ins);super::law::sync(&mut player.chr_ins,contract_run,state.law_fight.overtime());super::embers::sync(&mut player.chr_ins,contract_run,&state.ember_fight);let data=player.player_game_data.as_ptr();super::weapon_glow::sync(&mut player.chr_ins,unsafe {&*data},super::weapon_glow::clock());super::affix_stack::sync(&mut player.chr_ins,unsafe {&*data});}
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
            if travel.poll(started.elapsed(), false, false) == super::travel::Progress::Failed {
                state.travel = None;
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
    let ash_fighting=ash_tracking_active(state.travel.is_some(),state.pending.as_ref().is_some_and(|pending|!pending.completed)||client_fighting||state.party.as_ref().is_some_and(|p|p.event.combat_active()));
    if ash_fighting&&!state.ash_tracking {
        match super::loadout::ash_snapshot(player_data) {
            Ok(before)=>{
                state.ash_before=before;state.ash_used=None;state.summon_seen=None;state.ash_tracking=true;state.ash_refund_requested=false;super::log(format!("Spirit ash preparation snapshot: {:?}",state.ash_before));
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
    } else if !ash_fighting {state.ash_tracking=false;}
    if world.summon_buddy_manager.player_has_alive_summon {
        state.summon_seen=Some(Instant::now());
        if ash_fighting {
            let candidates=[world.summon_buddy_manager.active_summon_goods_id,world.summon_buddy_manager.requested_summon_goods_id];
            if let Some(id)=first_used_ash(state.ash_used,&state.ash_before,candidates) {state.ash_used=Some(id);super::log(format!("Spirit ash used: goods={id}"));}
        }
    }
    // 双生的灵唤铃：第二只的召唤被原版拒绝时，补回白扣的骨灰（扣除可能稍晚于请求，5 秒内重试）
    if let Some(id)=super::spirits::take_twin_refund() {state.twin_refund=Some((id,Instant::now()));}
    if let Some((id,since))=state.twin_refund {
        match super::loadout::refund_spent_ashes(player_data,&state.ash_before,Some(id)) {
            Ok(0) if since.elapsed()<Duration::from_secs(5)=>{},
            Ok(count)=>{super::log(format!("TOWER_TWIN rejected ash {id} returned: {count}"));state.twin_refund=None;},
            Err(error)=>{super::log(format!("TOWER_TWIN ash refund failed: {error}"));state.twin_refund=None;},
        }
    }
    // 新的一场战斗开始，上一场的保护就不再适用
    let ash_ready=ash_refund_ready(ash_fighting.then_some(false),world.summon_buddy_manager.player_has_alive_summon,&mut state.ash_guard);
    if ash_ready && state.ashes_checked.is_none_or(|t|t.elapsed()>=Duration::from_millis(500)) {
        state.ashes_checked=Some(Instant::now());
        if state.ash_guard.is_some() {
            if state.ash_refund_requested {
                match super::loadout::ash_snapshot(player_data) {
                    Ok(current) if super::loadout::ash_refund_confirmed(&state.ash_before,&current,state.ash_used)=>{
                        super::log(format!("Spirit ash refund confirmed: goods={:?}",state.ash_used));state.ash_guard=None;
                    },
                    Ok(_)=>{},
                    Err(error)=>super::log_repeating(format!("Spirit ash refund confirmation failed: {error}")),
                }
            } else {
                // Only issue one grant; ItemGive is asynchronous. Keep the receipt until inventory confirms it.
                match super::loadout::refund_spent_ashes(player_data,&state.ash_before,state.ash_used) {
                    Ok(0)=>{},
                    Ok(_)=>{state.ash_refund_requested=true;},
                    Err(error)=>super::log_repeating(format!("Spirit ash refund failed: {error}")),
                }
            }
        } else {
            state.ash_guard=None;
            if let Err(error)=super::loadout::clear_spent_ashes(player_data){super::log_repeating(error);}
        }
    }
    if state.hub.is_none() {
        if state.hub_failed.is_some_and(|(owner,at)|owner==identity&&at.elapsed()<Duration::from_secs(5)){return;}
        super::legacy::set_owner(identity);
        match super::hub::Hub::new(identity,player_data.archetype) {
            Ok(hub)=>{
                if let Ok(mut path)=SESSION_PATH.lock(){*path=Some(super::hub::path(identity,"json"));}
                state.hub=Some(hub);state.hub_failed=None;super::name_tags::load(identity);super::auras::load(identity);
            },
            Err(error)=>{state.hub_failed=Some((identity,Instant::now()));report(&mut state,format!("旅程存档加载失败，未重置角色：{error}"));return;},
        }
    }
    if state.party.is_none() && state.hub.as_ref().is_some_and(|h|!h.active) && player.chr_ins.chr_flags1c5.death_flag() {
        if let Some(run)=&mut state.view.run {if !matches!(run.phase,Phase::Won|Phase::Dead){run.end_on_death();record(run);}}
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
        state.pending=None;state.reward_wait=None;state.travel=None;state.flasks=None;state.event_start=None;state.descent=None;state.stormveil=None;state.chariot_clock=None;
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
        state.command=None;state.view.ready=false;state.pad_capture.update(false);
        super::input::CAPTURE.store(false,std::sync::atomic::Ordering::Relaxed);
        let mut hub=state.hub.take().unwrap();
        let paused_party=hub.paused&&!solo_session();
        let blocked=(paused_party||party_waiting||state.party.as_ref().is_some_and(|p|p.needs_new_run())).then(||if paused_party {"本机仍在旧联机房间，请退出房间后继续保留的旅程。".into()}else{party_block_reason(&state)});
        let result=hub.poll(world,game,blocked.as_deref());
        state.view.room=Some(hub.view());state.hub=Some(hub);
        match result {
            Ok(Some(mut run))=>{
                if run.act==1&&run.visited.is_empty()&&run.completed_nodes==0 {
                    if let Some(hub)=&state.hub {super::run_log::clear(&super::run_log::path(hub.owner));}
                    // 新旅程：上一局留下的强化石作废
                    if let Some(player)=world.main_player.as_mut() {super::smith::clear(unsafe {player.player_game_data.as_mut()});}
                }
                if let Some(link)=&state.party {
                    let aligned=erdtree_tower::party_game::act_seed(link.config.seed,run.act).and_then(|seed|run.align_party_start(seed));
                    if let Err(error)=aligned {
                        if let Some(hub)=state.hub.as_mut(){hub.reopen_for_party(world);}
                        super::log(format!("Resumed run rejected by party alignment: {error}; hub reopened"));
                        report(&mut state,format!("{error}。保留的旅程与队伍不一致，请在大赐福选择“开始新旅程”。"));return;
                    }
                }
                state.view.room=None;state.view.run=Some(run);state.identity=None;state.flasks=None;
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
        let (bonus,cut)=state.view.run.as_ref().map_or((0,[0,0]),|r|(r.flask_bonus,r.law.flask_cut()));
        match super::flasks::Flasks::bind_bonus(player_data, class.unwrap(),bonus,cut) {
            Ok(mut flasks) => {
                if let Some(run)=&state.view.run {if let Err(error)=flasks.restore(player_data,run.flasks){report(&mut state,error);return;}}
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
                    match super::rooms::Room::new(id,kind,hp,player.chr_ins.modules.data.fp) {
                        Ok(room)=>{game.move_map_target=BlockId::from(room.block);game.initial_area_entity_id=room.entry;game.warp_requested=true;state.view.room=Some(room.view.clone());state.room=Some(room);},
                        Err(error)=>report(&mut state,error),
                    }
                }
            }
        }
        super::log(format!("Tower seed: {seed}"));
        report(&mut state, "角色已绑定，五瓶已就绪。点击金色节点开始旅程。");
    }

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
        let count = flasks.total();
        if let Some(run) = &mut state.view.run {
            run.flasks = count;
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
            pending.next_run.flasks = count;
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
    if state.travel.is_some()&&block==arena.block&&!game.warp_requested&&!player_dead&&arena.below_floor(player.block_position.y) {
        let error="门外地面未就绪，已退回安全点，未消耗节点。".to_string();
        if super::native_talk::set_flag(45007994,true).is_ok() {
            let _=super::native_talk::set_flag(45007990,false);
            let _=super::native_talk::set_flag(45007992,false);
            let _=super::native_talk::set_flag(45007993,false);
            state.travel=None;state.view.travelling=false;state.view.busy=false;
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
        if arrived&&grounded {let _=super::native_talk::set_flag(45007992,true);}
        let placed=super::native_talk::flag(45007993).unwrap_or(false);
        let crossed=super::native_talk::flag(45007991).unwrap_or(false);
        let progress = if arena.uses_circle(){travel.poll_circle(started.elapsed(),arrived&&placed,grounded,crossed)}else{travel.poll_fog(started.elapsed(), arrived&&placed, grounded,crossed)};
        if arrived&&placed&&!travel.resources_restored {
            let (_,_,saved_hp,saved_fp)=state.travel.as_ref().unwrap();
            if let Some(player)=world.main_player.as_mut() {
                if let Err(error)=super::ailments::clear(&mut player.chr_ins){report(&mut state,error);return;}
                let data=&mut player.chr_ins.modules.data;
                data.hp=data.hp.min(*saved_hp);data.fp=data.fp.min(*saved_fp);
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
                        let data = &mut player.chr_ins.modules.data;
                        data.hp = data.hp.min(*saved_hp);
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
                if let Some(link)=state.party.as_mut(){link.arrived=true;if !link.spawn {report(&mut state,"已抵达，等待全员准备完成");return;}}
                let (travel, _, _, _) = state.travel.take().unwrap();
                let _=super::native_talk::set_flag(45007992,false);
                state.spawn_used = false;
                state.view.travelling = false;
                if let Err(error) = request_spawn(&mut state, world, travel.node) {
                    state.view.busy = false;
                    report(&mut state, error);
                }
            }
            super::travel::Progress::Failed => {
                state.travel = None;
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
        let touched=state.reward_portal.armed()&&super::native_talk::flag(45020971).unwrap_or(false);
        let entered=state.party.as_mut().is_some_and(|p|touched&&!p.rewards&&!std::mem::replace(&mut p.portal,true));
        let portal_used=state.party.as_ref().map_or(touched,|p|p.rewards);
        // 最后一幕的 Boss：没有三选一，传送门直接结束旅程
        let last=matches!(next.phase,Phase::Combat{id} if next.act>=3&&next.nodes.get(id).is_some_and(|n|n.kind==erdtree_tower::NodeKind::Boss));
        if entered {report(&mut state,if last{"已进入传送门，等待全部队友进入后一起结束旅程。"}else{"已进入传送门，等待全部队友进入后一起领取奖励。"});}
        match state.reward_portal.poll(started.elapsed(), player_dead, portal_used) {
            ResultState::Waiting => {
                state.reward_wait = Some((started, next));
                return;
            }
            ResultState::PlayerDied => {
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
                if let Some(link)=state.party.as_mut(){if let Err(error)=link.open_rewards(){link.stop(error.clone());report(&mut state,error);return;}}
                let _ = super::native_talk::set_flag(45020970, false);
                let _ = super::native_talk::set_flag(45020971, false);
                next.flasks = state.flasks.as_ref().map_or(next.flasks, |f| f.total());
                next.casting_tools=Some(super::loot::casting_tools(player_data));
                if let Ok(params)=unsafe {SoloParamRepository::instance()} {next.weapon_peak=Some(super::smith::inventory_peak(player_data,params).clamp(0,50) as u8);}
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
            Action::KeepCurrency => transact(&mut state, world, action, journaled),
            Action::Contract(_) | Action::Gear(_) | Action::SkipGear | Action::SkipContract => contract_action(&mut state,world,action),
            Action::Story(_) | Action::Claim(_) | Action::Camp(_) => {
                transact(&mut state, world, action, journaled)
            }
        };
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
    let pending=state.pending.as_mut().unwrap();
    let (outcome, enemy_hp, bound) = poll_group(pending, world, player_dead&&!party_mode, block,stall_authority);
    // 联机「精英成双」：还在等前一只的精英没请求过生成，不算未绑定（否则全队等不到开战）
    let all_bound=pending.foes.iter().all(|foe|foe.fight.handle().is_some()||foe.requested.is_none())&&pending.foes.iter().any(|foe|foe.fight.handle().is_some());
    if let Some(link)=state.party.as_mut(){link.bound=all_bound;}
    let pending=state.pending.as_mut().unwrap();
    if bound && !player_dead && matches!(pending.next_run.phase, Phase::Combat { .. }) {
        let _ = pending.next_run.observe_hp(normalized_hp(hp, max_hp));
    }
    if outcome == Outcome::Waiting {
        if bound&&!pending.next_run.law.lone_wolf() {if let Err(error)=pending.spirits.poll(world,block,!player_dead) {super::log(format!("Spirit activation failed: {error}"));}}
        let runtime=&mut *state;
        let mut rise=None;
        if let Some(pending)=runtime.pending.as_ref().filter(|_|bound)&&let Phase::Combat{id}=pending.next_run.phase {
            let run=&pending.next_run;
            runtime.boss_fight.begin((run.seed,run.act,id),erdtree_tower::boss_affixes::roll(run,id),run.law.empowered_affixes());
            let handles:Vec<_>=pending.foes.iter().filter_map(|f|f.fight.handle()).collect();
            let authority=runtime.party.as_ref().is_none_or(|p|p.host());
            let flasks=runtime.flasks.as_ref().map(|f|f.total());
            let team=runtime.party.as_ref().map(|p|p.gate.flask_uses());
            runtime.boss_fight.tick(world,&handles,authority,runtime.party.is_some(),flasks,team.as_deref());
            runtime.growth.tick(world,(run.seed,run.act,id),erdtree_tower::scaling::growth_step(run,id),&handles,authority);
            runtime.view.boss_affixes=runtime.boss_fight.title();runtime.view.boss_alert=runtime.boss_fight.alert();runtime.view.boss_affix_list=runtime.boss_fight.affixes().to_vec();
            // Boss 房的 Boss 卡在 1 点生命太久：视为转阶段锁血，直接放行
            if run.nodes.get(id).is_some_and(|n|n.kind==erdtree_tower::NodeKind::Boss) {
                let bosses:Vec<_>=pending.foes.iter().filter(|f|f.spec.bar>0).filter_map(|f|Some((f.fight.handle()?,f.spec.name))).collect();
                runtime.boss_guard.tick(world,(run.seed,run.act,id),&bosses,authority);
            }
            // 第三幕双王共命：超时复生（屏幕上不显示提示）
            if run.nodes.get(id).is_some_and(|n|super::enemies::dual(n.kind,run.act))&&pending.foes.len()==2&&pending.foes.iter().all(|f|f.spec.bar>0) {
                // 联机用预置实体，死了不能重新生成：只有单人复生
                let revive=!pending.networked;
                let foes=[0,1].map(|i|(pending.foes[i].spec.name,pending.foes[i].fight.handle()));
                rise=runtime.dual_fight.tick(world,(run.seed,run.act,id),foes,authority,revive);
            }
            if runtime.party.as_ref().is_none_or(|p|p.fighting) {
                let team=runtime.party.as_ref().map(|p|p.gate.flask_uses());
                runtime.law_fight.tick(world,(run.seed,run.act,id),run.law,&handles,authority,flasks,team.as_deref());
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
            foe.fight=Encounter::default();foe.requested=None;foe.entity_address=None;foe.respawn=Some(at);
        }
        let runtime=&mut *state;
        if publish_bound_combat(&mut runtime.view,&runtime.pending.as_ref().unwrap().next_run,bound) {
            report(&mut state, "战斗开始：击败竞技场内的敌人。");
        }
        state.view.enemy_hp = enemy_hp;
        return;
    }
    if outcome==Outcome::Victory {
        if let Some(link)=state.party.as_mut(){
            if let Err(error)=link.result(){
                if link.encounter.is_some_and(|t|t.elapsed()>Duration::from_secs(90)){link.stop(error.clone());}
                report(&mut state,error);return;
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
            let healed=heal_victory(world,&next);
            if keep_summoned_ash(world,&next,state.summon_seen,&state.ash_before,state.ash_used) {state.ash_guard=Some(Instant::now());}
            world.summon_buddy_manager.disappear_requested=true;
            state.reward_wait = Some((Instant::now(), next));
            state.reward_portal=Default::default();
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
    if state.party.is_some() {return Err("联机时不能使用调试功能".into());}
    let busy=state.pending.as_ref().is_some_and(|p|!p.completed)||state.travel.is_some()||state.reward_wait.is_some()||state.view.travelling;
    let player=world.main_player.as_mut().ok_or("角色尚未就绪")?;
    let data=unsafe {player.player_game_data.as_mut()};
    let mut next=state.view.run.clone().ok_or("旅程尚未开启")?;
    next.debug=true;
    if let Some(pending)=state.pending.as_mut() {pending.next_run.debug=true;}
    let message=match command {
        Command::Heal=>{
            let vitals=&mut player.chr_ins.modules.data;vitals.hp=vitals.max_hp;vitals.fp=vitals.max_fp;
            if let Some(flasks)=state.flasks.as_mut() {flasks.fill(data)?;next.flasks=flasks.total();}
            if matches!(next.phase,Phase::Combat{..}) {next.hp=100;} else {next.hp=next.max_hp;}
            "已回满生命、专注与圣杯瓶".to_owned()
        },
        // 开关本身由 debug_menu::apply_cheats 每帧写入；这里只把旅程标记为调试旅程并提示
        Command::Cheats(god,one_shot)=>format!("无敌{}，秒杀{}",if god{"开"}else{"关"},if one_shot{"开"}else{"关"}),
        Command::Runes(amount)=>{
            data.rune_count=data.rune_count.saturating_add(amount).min(999_999_999);next.gold=data.rune_count;state.view.runes=data.rune_count;
            format!("卢恩 +{amount}")
        },
        Command::Item(id)=>{
            super::items::give_reward(data,id)?;
            format!("已获得 {}",erdtree_tower::rewards::lookup(id).map_or("物品",|i|i.name))
        },
        command=>{
            if busy {return Err("请等战斗、传送或领奖结束后再改旅程".into());}
            let flask_cut=next.law.flask_cut();
            let message=match command {
                Command::Jump(id)=>{next.debug_jump(id)?;format!("已跳到第 {} 层的{}",next.nodes[id].layer+1,next.nodes[id].kind.label())},
                Command::Act(act)=>{next.debug_act(act)?;format!("已切换到第 {act} 幕起点")},
                Command::Oath(kind,level)=>{next.debug_oath(kind,level)?;format!("{} 设为 {}",kind.name(),["移除","I","II","III"][usize::from(level)])},
                Command::Ember(ember,held)=>{next.debug_ember(ember,held)?;format!("{}余烬「{}」",if held{"获得"}else{"移除"},ember.name())},
                Command::Law(law)=>{next.debug_law(law)?;format!("苦难律法改为 {} 分（{}）",law.score(),law.code())},
                _=>unreachable!(),
            };
            if let Some(flasks)=state.flasks.as_mut() {
                if next.law.flask_cut()!=flask_cut {flasks.set_bonus(data,next.class,next.flask_bonus,next.law.flask_cut())?;}
                if next.act!=state.view.run.as_ref().map_or(next.act,|r|r.act) {flasks.fill(data)?;let vitals=&mut player.chr_ins.modules.data;vitals.hp=vitals.max_hp;vitals.fp=vitals.max_fp;}
                flasks.restore(data,next.flasks)?;next.flasks=flasks.total();
            }
            state.view.prompt=state.view.prompt.wrapping_add(1);
            message
        },
    };
    record_checked(&next)?;
    state.view.run=Some(next);
    Ok(message)
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
fn debug_boss_command(state:&mut Runtime,world:&mut WorldChrMan,game:&mut GameMan,command:super::debug_menu::Command)->Result<(),String> {
    if !super::debug_menu::enabled(){return Err("请先开启调试模式".into());}
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
            super::log("BOSS_TEST exit requested");QUICK_END.store(true,std::sync::atomic::Ordering::Relaxed);super::debug_menu::close();Ok(())
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

/// 当前握在手里的武器与身上防具中的词条装备
fn equipped_build(data:&eldenring::cs::PlayerGameData)->Vec<super::reward_ui::Gear> {
    let equipment=&data.equipment.chr_asm;
    let weapons=[equipment.equipment.active_right_weapon_slot() as usize,equipment.equipment.active_left_weapon_slot() as usize];
    weapons.into_iter().filter_map(|slot|equipment.equipment_param_ids.get(slot).copied().filter(|id|*id>0).map(|id|id as u32/100*100))
        .chain((12..=15).filter_map(|slot|equipment.equipment_param_ids.get(slot).copied().filter(|id|*id>0).map(|id|0x10000000|id as u32)))
        .filter_map(super::reward_ui::affix_summary).collect()
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
fn record_ember_gains(state:&mut Runtime,gains:Vec<erdtree_tower::embers::Gain>) {
    if gains.is_empty() {return;}
    let mut raised=Vec::new();
    for event in gains {
        for run in state.view.run.iter_mut().chain(state.pending.iter_mut().map(|p|&mut p.next_run)).chain(state.reward_wait.iter_mut().map(|(_,run)|run)) {
            for ember in run.ember_stacks.gain(&run.embers.clone(),event) {
                let level=run.ember_stacks.level(ember);
                if !raised.iter().any(|(e,l)|*e==ember&&*l>=level) {raised.retain(|(e,_)|*e!=ember);raised.push((ember,level));}
            }
        }
    }
    for (ember,level) in raised {
        super::log(format!("TOWER_EMBER stack {} {level}",ember.name()));
        state.view.status=format!("余烬「{}」第 {level} 层：{}",ember.name(),ember.stack_bonus(level).unwrap_or_default());state.view.prompt=state.view.prompt.wrapping_add(1);
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
    let events=state.ember_fight.tick(world,(run.seed,run.act,id),&embers,&foes,authority,state.party.is_some(),flasks,keeper);
    if events.feast {grant_feast(state.flasks.as_mut(),world);}
    record_ember_gains(state,events.gains);
    // 房主：执行成员发来的敌人侧余烬效果
    if let Some(link)=state.party.as_ref().filter(|p|p.host()) {
        for (member,held,requests) in link.gate.strikes() {state.ember_fight.apply_remote(world,&foes,member,&held,&requests);}
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
/// 联机成员的战斗结算。成员没有本地的战斗记录，敌人按地图里的预置实体找（与 Boss 血条相同的找法，倒下的也留着，用来判断击倒）。
/// - 苦难律法：限时倒计时与超时失血（敌人的强化由房主挂）
/// - Boss 词条：屏幕上的词条与护盾提示、腐败积累、荆棘反弹（改 Boss 状态的由房主做）
/// - 余烬：作用在自己与召唤物身上的直接生效；作用在敌人身上的（出血、冻伤、死诞之焰等）算出后随汇报交给房主执行
fn member_combat(state:&mut Runtime,world:&mut WorldChrMan) {
    if !member_fighting(state) {return;}
    let Some(run)=state.view.run.as_ref() else{return};
    let Phase::Combat{id}=run.phase else{return};
    let key=(run.seed,run.act,id);
    let flasks=state.flasks.as_ref().map(|f|f.total());
    let roster=super::enemies::node_roster(run,id);
    let handles:Vec<_>=super::enemies::map_entities(&roster).map(|ids|roster.iter().zip(ids).filter_map(|(spec,entity)|
        world.chr_sets.iter().flatten().flat_map(|set|set.characters())
            .find(|chr|chr.event_entity_id==entity&&chr.npc_id==spec.model&&chr.npc_param_id==spec.npc&&enemy_loaded(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status))
            .map(|chr|chr.field_ins_handle)).collect()).unwrap_or_default();
    if member_live(state) {
        state.law_fight.tick(world,key,run.law,&handles,false,flasks,None);
        state.view.law_clock=state.law_fight.clock();
        state.boss_fight.begin(key,erdtree_tower::boss_affixes::roll(run,id),run.law.empowered_affixes());
        state.boss_fight.tick(world,&handles,false,true,flasks,None);
        state.view.boss_affixes=state.boss_fight.title();state.view.boss_alert=state.boss_fight.alert();state.view.boss_affix_list=state.boss_fight.affixes().to_vec();
    }
    member_embers(state,world);
}
/// 联机成员的余烬（见 member_combat）
fn member_embers(state:&mut Runtime,world:&mut WorldChrMan) {
    let Some(run)=state.view.run.as_ref() else{return};
    let Phase::Combat{id}=run.phase else{return};
    let embers=super::embers::held(Some(run));
    if embers.is_empty() {return;}
    let roster=super::enemies::node_roster(run,id);
    let Ok(ids)=super::enemies::map_entities(&roster) else{return};
    let foes:Vec<_>=roster.iter().enumerate().filter_map(|(slot,spec)|{
        let chr=world.chr_sets.iter().flatten().flat_map(|set|set.characters())
            .find(|chr|ids.get(slot)==Some(&chr.event_entity_id)&&chr.npc_id==spec.model&&chr.npc_param_id==spec.npc&&enemy_loaded(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status))?;
        Some((chr.field_ins_handle,super::enemies::bar_rank(spec),slot as u8))
    }).collect();
    let key=(run.seed,run.act,id);
    let keeper=run.ember_stacks.level(erdtree_tower::embers::Ember::SpiritKeeper);
    let flasks=state.flasks.as_ref().map(|f|f.total());
    let events=state.ember_fight.tick(world,key,&embers,&foes,false,true,flasks,keeper);
    if events.feast {grant_feast(state.flasks.as_mut(),world);}
    record_ember_gains(state,events.gains);
    // 作用在敌人身上的余烬交给房主执行
    if !events.strikes.is_empty() && let Some(link)=state.party.as_mut() {link.push_strikes(&events.strikes);}
}
/// 联机成员：按与房主相同的苦难律法「精英成双」规则，名单里哪些位置还要等前面的精英（按本机看到的敌人状态）
fn member_held(world:&WorldChrMan,run:&Run,id:usize,roster:&[super::enemies::Enemy],ids:&[u32])->Vec<bool> {
    use erdtree_tower::encounter::{Outcome as Spawn,Pacing,Role};
    let elite=run.nodes.get(id).is_some_and(|n|n.kind==erdtree_tower::NodeKind::Elite);
    let pacing=if elite {run.law.elite_pacing()}else{Pacing::Together};
    let roles:Vec<_>=roster.iter().map(super::enemies::role).collect();
    // 每个位置：(已倒下, 生命, 最大生命)；还没放出来的为 None
    let state:Vec<Option<(bool,i32,i32)>>=roster.iter().zip(ids).map(|(spec,&entity)|{
        world.chr_sets.iter().flatten().flat_map(|set|set.characters())
            .find(|chr|chr.event_entity_id==entity&&chr.npc_id==spec.model&&chr.npc_param_id==spec.npc&&!chr.debug_flags.character_disabled()
                &&enemy_loaded(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status))
            .map(|chr|(chr.modules.data.hp<=0||chr.chr_flags1c5.death_flag(),chr.modules.data.hp,chr.modules.data.max_hp))
    }).collect();
    (0..roster.len()).map(|i|{
        let earlier:Vec<_>=(0..i).filter(|&j|roles[j]==Role::Strong).map(|j|match state[j] {
            Some((true,_,_))=>(Spawn::Victory,None),
            Some((false,hp,max))=>(Spawn::Waiting,Some((hp,max))),
            None=>(Spawn::Waiting,None),
        }).collect();
        !erdtree_tower::encounter::may_spawn_paced(true,false,pacing.waits(&earlier),&roles,i,&[])
    }).collect()
}
fn enemy_loaded(status: ChrLoadStatus) -> bool {
    // The pinned 1.17.1 game uses state 4 for the live player and debug-spawned enemies.
    matches!(
        status,
        ChrLoadStatus::Active | ChrLoadStatus::ReadyForActivation
    )
}

fn transact(state: &mut Runtime, world: &mut WorldChrMan, action: Action, journaled: bool) -> Result<(), String> {
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
            next.camp(choice)?;
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
    }else{None};
    if journaled {write_award_journal(&award_path()?,&old)?;}
    if next.act > old.act {
        next_hp=vitals.max_hp;
        next_fp=vitals.max_fp;
        state.flasks.as_mut().ok_or("圣杯瓶未绑定")?.fill(unsafe {player.player_game_data.as_mut()})?;
        next.flasks=state.flasks.as_ref().unwrap().total();
    }
    if next.weapon_level > old.weapon_level {
        super::weapons::upgrade(unsafe { player.player_game_data.as_mut() })?;
    }
    if let Some(target)=reforge {super::weapons::reforge(unsafe { player.player_game_data.as_mut() },target)?;}
    if let Some(blue) = camp_refill {
        state.flasks.as_mut().ok_or("圣杯瓶未绑定")?
            .camp_refill(unsafe {player.player_game_data.as_mut()}, blue)?;
        next.flasks = state.flasks.as_ref().unwrap().total();
    } else if effect.refill {
        state
            .flasks
            .as_mut()
            .ok_or("圣杯瓶未绑定")?
            .refill(unsafe { player.player_game_data.as_mut() })?;
        next.flasks = state.flasks.as_ref().unwrap().total();
    }
    if next.blessings != old.blessings {
        super::blessings::Blessings::deliver(unsafe {player.player_game_data.as_mut()}, &mut next)?;
    }
    if let Some(id)=item_reward {super::items::give_reward_with_ash(unsafe {player.player_game_data.as_mut()},id,reward_ash)?;}
    if sacred_tear {super::items::give_stack(unsafe {player.player_game_data.as_mut()},erdtree_tower::elite::TEAR)?;}
    if upgrade_stone {super::items::give_stack(unsafe {player.player_game_data.as_mut()},super::smith::VOUCHER)?;}
    if next.flask_bonus!=old.flask_bonus {
        state.flasks.as_mut().ok_or("圣杯瓶未绑定")?.set_bonus(unsafe {player.player_game_data.as_mut()},next.class,next.flask_bonus,next.law.flask_cut())?;
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
                super::contracts::give_with_ash(unsafe {player.player_game_data.as_mut()},id,qty,gem)?;
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
fn sync_merchant_reward(state:&mut Runtime)->Result<(),String> {
    let phase=state.party.as_ref().map_or(0,|p|p.event.merchant_phase);
    if phase==0{return Ok(());}
    let Some(run)=state.view.run.as_ref() else{return Ok(());};
    let Phase::Node{id}=run.phase else{return Ok(());};
    if run.nodes[id].kind!=erdtree_tower::NodeKind::Shop{return Err("商人挑战与当前节点不匹配".into());}
    let mut next=run.clone();
    if !next.bells.down(next.act,id){next.challenge_merchant()?;}
    let reward=if phase==3 {next.defeat_hunter()?}else{None};
    if next!=*run {record_checked(&next)?;state.view.run=Some(next);}
    if let Some(kind)=reward {report(state,format!("获得{}！本局后续商店对应商品 +5，全队共享。",erdtree_tower::merchant::NAMES[kind]));}
    Ok(())
}
fn tick_merchant(state:&mut Runtime,world:&WorldChrMan,room:&mut super::rooms::Room)->Result<bool,String> {
    let Some(run)=state.view.run.as_ref() else{return Ok(false);};
    let host=state.party.as_ref().is_none_or(|p|p.host());
    if !run.bells.down(run.act,room.node)&&room.merchant_dead(world)&&host {
        if state.party.as_ref().is_some_and(|p|!p.gate.merchant_present()) {
            room.merchant_down=true;room.suspend_dialogues()?;
            room.view.message="商人已倒下；需全队仍在商店才能触发铃珠猎人。可通过传送门继续。".into();return Ok(false);
        }
        room.suspend_dialogues()?;
        let mut next=run.clone();next.challenge_merchant()?;record_checked(&next)?;
        super::log(format!("MERCHANT_HUNT start act={} node={} party={}",next.act,room.node,state.party.is_some()));state.view.run=Some(next);
    }
    let run=state.view.run.as_ref().unwrap();
    room.merchant_down=run.bells.down(run.act,room.node);
    if run.bells.challenge.is_none(){if let Some(hunter)=room.hunter.take(){hunter.retire();}return Ok(false);}
    room.suspend_dialogues()?;
    super::native_talk::set_flag(room.base()+993,false)?;
    super::native_talk::set_flag(room.base()+994,false)?;
    if room.hunter.is_none(){room.hunter=Some(super::map_guards::Guards::merchant()?);}
    let hunter=room.hunter.as_mut().unwrap();hunter.prepare(world)?;
    let mut started=hunter.ready();
    if let Some(link)=state.party.as_mut() {
        if host&&link.event.merchant_phase==0 {link.merchant_phase(1)?;}
        link.event.merchant_bound=hunter.ready();
        if host&&link.event.merchant_phase==1&&link.gate.merchant_ready(){link.merchant_phase(2)?;}
        started=link.event.merchant_phase>=2;
    }
    room.view.message=if started {"铃珠猎人已出现：击败后获得随机铃珠。"}else{"商人已倒下，正在等待全队加载铃珠猎人……"}.into();
    if hunter.poll(world,started,host)? {
        if let Some(link)=state.party.as_mut(){link.merchant_phase(3)?;sync_merchant_reward(state)?;}
        else {
            let mut next=state.view.run.clone().unwrap();let kind=next.defeat_hunter()?;record_checked(&next)?;state.view.run=Some(next);
            if let Some(kind)=kind {report(state,format!("获得{}！本局后续商店对应商品 +5。",erdtree_tower::merchant::NAMES[kind]));}
        }
        hunter.retire();room.hunter=None;
        room.view.message="铃珠猎人已击败，铃珠已收入本局收藏，使用传送门继续。".into();
        super::native_talk::set_flag(room.base()+993,true)?;return Ok(false);
    }
    Ok(true)
}
fn tick_room(state: &mut Runtime, world: &mut WorldChrMan, game: &mut GameMan, dead: bool) {
    if dead&&state.party.as_ref().is_some_and(|p|!p.defeated)&&!state.view.run.as_ref().is_some_and(|r|r.bells.challenge.is_some()) {
        state.command=None;state.view.ready=false;state.view.busy=true;
        report(state,"等待队友救援或全队结算");return;
    }
    let mut room = state.room.take().unwrap();
    if dead && state.party.is_none() && !room.view.loading && !game.warp_requested {
        room.retire(world);
        if let Some(run) = &mut state.view.run {
            run.hp = 0;
            run.phase = Phase::Dead;
            record(run);
        }
        state.command = None;
        state.view.room = None;
        state.view.busy = false;
        report(state, "角色死亡，本局结束。");
        return;
    }
    let mut transferred=false;
    let result = (|| -> Result<bool, String> {
        if state.view.shared_event {return Ok(false);}
        if room.kind==erdtree_tower::NodeKind::Shop {
            room.merchant_down=state.view.run.as_ref().is_some_and(|r|r.bells.down(r.act,room.node));
        }
        if room.kind==erdtree_tower::NodeKind::Event {
            if let Some(run)=state.view.run.as_ref(){room.event_menu(run,state.party.is_some(),state.party.as_ref().is_some_and(|p|p.event.request.is_some()),state.party.is_none());}
        }
        let storm_order=state.view.run.as_ref().and_then(|r|r.regional_event.stormveil)
            .and_then(|_|state.party.as_ref().and_then(|p|p.event.storm_decision));
        let trial_order=state.view.run.as_ref().filter(|r|r.regional_event.trial)
            .and_then(|_|state.party.as_ref().and_then(|p|p.event.trial_decision));
        if storm_order.is_none()&&trial_order.is_none(){room.poll(world, game)?;}
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
                    Trial::Descent=>{start_descent(state,world,game,next)?;transferred=true;},
                    Trial::Stormveil|Trial::Chariot=>{start_stormveil(state,world,game,next)?;transferred=true;},
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
            let data=unsafe {world.main_player.as_mut().ok_or("角色未加载")?.player_game_data.as_mut()};
            if let Some(run)=state.view.run.as_ref() {super::spirit_tune::set_peak(erdtree_tower::rewards::spirit_level_now(run));}
            if super::smith::spent(room.allowance_issued,room.view.completed,super::smith::remaining(data),room.allowance_target) {
                let run=state.view.run.as_mut().ok_or("角色未绑定")?;
                // 调灵也走这里：调灵后的骨灰等级立即记进本局（骨灰召唤后会用掉，不能等下一场胜利再看背包）
                run.spirit_peak=Some(spirit_peak(run,super::spirit_tune::inventory_peak(data)));
                run.leave_camp()?;
                record(run);
                room.complete("强化完成。换战灰、记忆魔法和升级仍可免费使用服务；加点消耗卢恩。".into());
                super::log("Native camp reinforcement consumed its single allowance");
            }
            let request = room.take_refill().and_then(|choice|match choice {1=>Some(CampChoice::Refill),2=>Some(CampChoice::RefillBlue),3=>Some(CampChoice::RefillRedOnly),4=>Some(CampChoice::RefillBlueOnly),_=>None});
            if let Some(choice) = request {
                let result = if room.view.completed {
                    Err("篝火服务已用完".to_string())
                } else {
                    transact(state, world, Action::Camp(choice), false)
                };
                if let Err(error) = &result {
                    super::log(error);
                }
                if result.is_ok() {
                    // 补瓶代替强化：用掉营地发的那块强化石，身上其他强化石保留
                    if let Err(error)=super::smith::consume_one(unsafe {world.main_player.as_mut().unwrap().player_game_data.as_mut()}) {super::log(format!("Camp refill stone: {error}"));}
                    room.complete("服务已完成，使用传送门继续".into());
                }
                room.notice(if result.is_err(){9500012}else{9500010})?;
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
    if matches!(result, Ok(true)) {
        state.event_start=None;
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

fn tick_chest_award(state:&mut Runtime,world:&mut WorldChrMan,room:&mut super::rooms::Room)->Result<(),String> {
    use super::{loot,chest_award::Receipt,native_talk::{flag,set_flag}};
    if room.view.completed {set_flag(45010995,true)?;return Ok(());}
    let run=state.view.run.as_ref().ok_or("个人宝箱进度尚未加载")?;
    if run.phase!= (Phase::Node{id:room.node}) {return Err("个人宝箱进度不匹配，未发放物品".into());}
    let path=award_path()?.with_extension(format!("chest-{:016x}-{}",run.seed,room.node));
    if !flag(45010990)?&&!path.exists(){return Ok(());}
    if room.chest_attempt.is_some_and(|at|at.elapsed()<Duration::from_secs(1)){return Ok(());}
    room.chest_attempt=Some(Instant::now());
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
        _=>GAMBLE_REFORGE+choice as i32,
    }
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
    if run.regional_event.trial {
        clear_chariot()?;
        set_flag(45010965,false)?;set_flag(45010960,false)?;
        set_flag(45010941,true)?;set_flag(45010940,true)?;
        set_flag(45010942,false)?;set_flag(45010995,false)?;
        room.view.message="试炼岔路：是男人就下一百层、男生女生向前冲、泥头车，或领取 800 卢恩离开。选择后查看说明并确认，联机提交投票。".into();
        return Ok(());
    }
    if run.regional_event.stormveil.is_some() {
        if run.regional_event.chariot&&run.regional_event.stormveil==Some(erdtree_tower::stormveil::Progress::Choosing){clear_chariot()?;}
        set_flag(45010965,false)?;set_flag(45010960,false)?;
        set_flag(45010941,true)?;set_flag(45010940,true)?;
        set_flag(45010942,false)?;set_flag(45010995,false)?;
        room.view.message=if run.regional_event.chariot {
            "泥头车生存：全队进入竞技场，5 秒后出现战车，坚持 60 秒后领取传说宝箱；也可各领 800 卢恩跳过。"
        }else{"史东薇尔突围：抵达葛瑞克门前获得两个随机护符增益，或领取 800 卢恩离开。"}.into();
        return Ok(());
    }
    if run.regional_event.descent {
        set_flag(45010965,false)?;set_flag(45010960,false)?;
        set_flag(45010941,true)?;set_flag(45010940,true)?;
        set_flag(45010942,false)?;set_flag(45010995,true)?;
        room.view.message="是男人就下一百层".into();return Ok(());
    }
    if run.regional_event.recovery {
        set_flag(45010965,false)?;set_flag(45010960,false)?;
        set_flag(45010941,true)?;set_flag(45010940,true)?;
        set_flag(45010942,false)?;set_flag(45010995,true)?;
        room.view.message="月光泉：免费回满生命或专注。".into();
        if let Some(super::native_talk::EventRequest::Recovery(choice))=room.take_event_request() {
            let player=world.main_player.as_mut().ok_or("角色尚未加载")?;
            if player.chr_ins.modules.data.hp<=0 {return Err("角色已死亡".into());}
            let mut next=run.clone();next.recover_at_spring(choice)?;record_checked(&next)?;
            let stats=&mut player.chr_ins.modules.data;
            if choice==0 {stats.hp=stats.max_hp;}else{stats.fp=stats.max_fp;}
            state.view.run=Some(next);
            room.view.message=if choice==0{"生命已回满"}else{"专注已回满"}.into();
        }
        return Ok(());
    }
    // 赌运事件（单人）：与 NPC 交谈打开原版对话菜单，选项确认后结算并弹出结果，每处一次；联机时这一格按种子改为三重宝匣或贪欲祭坛
    if let Some(index)=run.gamble(room.node).filter(|_|state.party.is_none()) {
        set_flag(45010965,false)?;set_flag(45010960,false)?;
        set_flag(45010941,true)?;set_flag(45010940,true)?;
        set_flag(45010942,false)?;set_flag(45010995,true)?;
        let title=erdtree_tower::events::STORIES[index].title;
        let wins=erdtree_tower::events::dice_wins(run,room.node);
        if run.regional_event.opened==0 {room.view.message=format!("{title}：与 NPC 交谈。");}
        if let Some(super::native_talk::EventRequest::Gamble(choice))=room.take_event_request() {
            let message=state.view.run.as_ref().map(|r|erdtree_tower::events::story_outcome(r,choice)).unwrap_or_default();
            match transact(state,world,Action::Story(choice),false) {
                Ok(())=>{room.notice(gamble_result(index,choice,wins))?;room.view.message=format!("{message}使用传送门继续旅程。");},
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
        if run.regional_event.guards&&link.event.guards_cleared {
            // [fix] This room's guards were already defeated by the party. A second wave would try to
            // reload the same pre-placed guard entities, which never unload after death, and lock the
            // room forever. Resolve the outcome locally instead.
            if matches!(link.event.request,Some(erdtree_tower::party_events::Kind::Guards(_))){link.event.request=None;}
            let mut next=run.clone();next.regional_event.guards=false;next.regional_event.guard_slot=None;
            record_checked(&next)?;
            set_flag(45010942,false)?;set_flag(45010995,true)?;
            room.view.message="本房间的守卫已被队伍击败，可以继续探索或使用传送门离开。".into();
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
            let roster=super::enemies::roster(if altar {erdtree_tower::NodeKind::Elite}else{erdtree_tower::NodeKind::Monster},run.seed,room.node,run.nodes[room.node].layer*11/(run.floors()-1),run.act);
            let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"怪物参数未加载")?;
            if roster.iter().any(|e|params.get::<NpcParam>(e.npc as u32).is_none()||params.get::<NpcThinkParam>(e.think).is_none()) {return Err("事件守卫参数缺失".into());}
            state.pending=Some(Pending {
                entity_base:None,networked:false,
                spirits:Default::default(),foes:roster.into_iter().map(|spec|Foe{spec,fight:Encounter::default(),requested:None,entity_address:None,respawn:None}).collect(),
                next_run:run.clone(),block:room.block,completed:false,stall:None,
            });
        }
        let pending=state.pending.as_mut().unwrap();
        let (outcome,hp,bound)=poll_group(pending,world,false,room.block,true);
        state.view.enemy_hp=hp;
        if bound&&!run.law.lone_wolf() {pending.spirits.poll(world,room.block,true)?;}
        match outcome {
            Outcome::Victory => {
                let healed=heal_victory(world,run);if !healed.is_empty(){super::log(&healed);state.view.status=healed;state.view.prompt=state.view.prompt.wrapping_add(1);}
                if keep_summoned_ash(world,run,state.summon_seen,&state.ash_before,state.ash_used) {state.ash_guard=Some(Instant::now());}
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
        next.hp=normalized_hp(vitals.hp,vitals.max_hp).max(u32::from(state.party.is_some()));
        if vitals.hp<=0&&state.party.is_none() {next.phase=Phase::Dead;}
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
            if data.rune_count<1000 {room.notice(9500084)?;return Ok(());}
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
            if data.rune_count<1000 {room.notice(9500084)?;return Ok(());}
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
            data.rune_count=data.rune_count.saturating_add(1000);run.gold=data.rune_count;state.view.runes=data.rune_count;
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
    if room.scarabs.done() {room.scarabs=Default::default();}
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

fn request_spawn(state: &mut Runtime, world: &mut WorldChrMan, id: usize) -> Result<(), String> {
    let run = state.view.run.as_ref().ok_or("角色未就绪")?;
    start_allowed(run, id, state.spawn_used)?;
    if let Some(test)=run.boss_test.filter(|t|run.debug&&t.act==run.act&&run.nodes[id].kind==erdtree_tower::NodeKind::Boss) {
        super::enemies::boss_test_roster(test.encounter,state.party.is_some(),erdtree_tower::dlc::enemies_allowed())?;
    }
    if state.party.as_ref().is_some_and(|p|!p.host()) {
        let mut next=run.clone();next.enter(id)?;
        let player=world.main_player.as_mut().ok_or("角色未加载")?;
        let vitals=&mut player.chr_ins.modules.data;vitals.fp=super::travel::combat_fp(run.class,vitals.fp,vitals.max_fp);
        state.view.run=Some(next);state.spawn_used=true;state.view.busy=false;state.view.travelling=false;
        report(state,"等待房主敌人同步");return Ok(());
    }
    let mut roster = super::enemies::node_roster(run,id);
    if state.party.is_none()&&!run.boss_test.is_some_and(|test|test.act==run.act) {if let Some(debug)=super::debug_menu::take_roster() {
        super::log(format!("DEBUG_MENU roster override: {}",debug.iter().map(|e|e.name).collect::<Vec<_>>().join(",")));roster=debug;
        // 斗蛐蛐实验：两只都按 Boss 角色处理，同时出场且保持红方、蓝方的顺序
        if let Some(setup)=super::debug_menu::take_bout() {state.bout.begin(setup);for e in &mut roster {e.kind=erdtree_tower::NodeKind::Boss;}}
    }}
    if state.party.is_none()&&!state.bout.active()&&run.nodes[id].kind==erdtree_tower::NodeKind::Monster {roster.sort_by_key(|e|super::enemies::role(e)!=erdtree_tower::encounter::Role::Weak);}
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
        entity_base:None,networked:state.party.is_some(),
        spirits:Default::default(),
        foes: roster.into_iter().map(|spec| Foe { spec, fight: Encounter::default(), requested: None, entity_address: None, respawn: None }).collect(),
        next_run, block: i32::from(player.current_block_id), completed: false, stall: None,
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
fn poll_group(pending: &mut Pending, world: &mut WorldChrMan, dead: bool, block: i32,authority:bool) -> (Outcome, Option<(i32,i32)>, bool) {
    if dead { return (Outcome::PlayerDied, None, false); }
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
    // 精英战里精英怪池的精英：平时一只一只上；苦难律法「精英成双 I」前一只半血时下一只上，「精英成双 II」一起上
    let elite_node=matches!(pending.next_run.phase,Phase::Combat{id} if pending.next_run.nodes.get(id).is_some_and(|n|n.kind==erdtree_tower::NodeKind::Elite));
    let pacing=if elite_node {pending.next_run.law.elite_pacing()}else{erdtree_tower::encounter::Pacing::Together};
    let mut untouched=Vec::new();
    for i in 0..count {
        let earlier:Vec<_>=(0..i).filter(|&j|roles[j]==erdtree_tower::encounter::Role::Strong).map(|j|{
            let health=pending.foes[j].fight.handle().and_then(|h|world.chr_ins_by_handle(&h)).map(|c|(c.modules.data.hp,c.modules.data.max_hp));
            (outcomes[j],health)
        }).collect();
        let held=pacing.waits(&earlier);
        if !erdtree_tower::encounter::may_spawn_paced(pending.networked,monster_node,held,&roles,i,&outcomes) {
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
                    super::log(format!("Party map enemy bound: entity={} handle={handle} model={}",ids[i],foe.spec.model));
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
                    if let Some(chr) = world.chr_ins_by_handle_mut(&handle) {
                        chr.team_type = 6;
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
                let pos = match foe.respawn.take() {
                    Some(at)=>HavokPosition(at[0],at[1]+0.1,at[2],0.0),
                    None=>HavokPosition(physics.position.0-player.block_position.x+local[0],
                        physics.position.1-player.block_position.y+local[1],
                        physics.position.2-player.block_position.z+local[2],0.0),
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
                world.spawn_debug_character(&ChrDebugSpawnRequest {
                    chr_id:foe.spec.model, npc_param_id:foe.spec.npc, npc_think_param_id:foe.spec.think as i32,
                    chara_init_param_id:0, event_entity_id:pending.entity_base.map_or_else(||if pending.networked||foe.spec.bar>0 {arena.boss_entity(i)}else{0},|base|base+i as i32), talk_id:-1, pos_x:pos.0,pos_y:pos.1,pos_z:pos.2,
                });
                foe.requested = Some(Instant::now());
                super::log(format!("Group spawn {}/{}: {} local={local:?} world={pos:?}",i+1,count,foe.spec.name));
            }
        }
        spawning |= foe.fight.handle().is_none() && foe.requested.is_some();
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
fn ash_refund_ready(pending_completed:Option<bool>,summon_alive:bool,guard:&mut Option<Instant>)->bool {
    let fighting=pending_completed==Some(false);
    if fighting {*guard=None;}
    !fighting&&!summon_alive
}
/// 本局的骨灰最高等级：只升不降；旧存档没有记录时从原来的上限（武器换算）接着算
fn spirit_peak(run:&Run,inventory:u8)->u8 {
    run.spirit_peak.unwrap_or_else(||(erdtree_tower::rewards::spirit_cap(run).saturating_sub(1)) as u8).max(inventory).min(10)
}
/// 召唤师「群灵」：战斗胜利时召唤物还活着，这次召唤不消耗骨灰。
/// 胜利前两秒内仍存活也计入；离场后再请求返还，保留资格直到背包确认或下一场战斗。
/// 这场战斗第一次召唤用的骨灰（召唤师「群灵」胜利时补回的就是它）：已经记下就不再改。
/// 双生的灵唤铃召唤第二只时 requested / active 会换成第二个骨灰，不能覆盖第一个——第二个骨灰按余烬说明照常消耗
fn first_used_ash(recorded:Option<u32>,before:&[(u32,u32)],candidates:[i32;2])->Option<u32> {
    if recorded.is_some() {return None;}
    candidates.into_iter().filter(|raw|*raw>0).map(|raw|raw as u32&0x0fffffff).find(|id|before.iter().any(|(item,_)|item==id))
}
fn keep_summoned_ash(world:&mut WorldChrMan,run:&erdtree_tower::Run,summon_seen:Option<Instant>,before:&[(u32,u32)],used:Option<u32>)->bool {
    if !run.summoner() {return false;}
    let alive=world.summon_buddy_manager.player_has_alive_summon||summon_seen.is_some_and(|t|t.elapsed()<Duration::from_secs(2));
    if !alive {super::log("Spirit ash not kept: no living summon at victory");return false;}
    super::log(format!("Spirit ash: summon alive at victory, refund pending; used={used:?} before={before:?}"));
    true
}
fn retire_group(pending:&Pending,world:&mut WorldChrMan){
    if pending.networked&&pending.entity_base.is_none() {
        if let Ok(ids)=super::enemies::map_entities(&pending.foes.iter().map(|f|f.spec).collect::<Vec<_>>()) {
            for id in ids {if let Err(error)=super::enemies::request_map_entity(id,false){super::log(error);}}
        }
    }
    for foe in &pending.foes {
        if let Some(chr)=foe.fight.handle().and_then(|h|world.chr_ins_by_handle_mut(&h)) {
            if foe.entity_address==Some(std::ptr::from_ref(chr) as usize){chr.debug_flags.set_character_disabled(true);}
        }
    }
}

struct ParkourRoute {block:i32,entry:u32,flag:u32,name:&'static str,goal:[f32;3]}
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
fn start_stormveil(state:&mut Runtime,world:&WorldChrMan,game:&mut GameMan,run:Run)->Result<(),String> {
    let player=world.main_player.as_ref().ok_or("角色尚未加载")?;
    if game.warp_requested||!player.chr_ins.modules.physics.standing_on_solid_ground{return Err("请等待角色站稳".into());}
    if run.regional_event.stormveil!=Some(erdtree_tower::stormveil::Progress::Running)&&!(run.regional_event.chariot&&run.regional_event.stormveil==Some(erdtree_tower::stormveil::Progress::Complete)){return Err("跑城挑战尚未确认".into());}
    record_checked(&run)?;
    let route=parkour_route(run.regional_event.chariot);
    state.chariot_clock=None;
    if run.regional_event.chariot {
        clear_chariot()?;
        set_flag(45010942,true)?;set_flag(45010995,true)?;
    }
    super::native_talk::set_flag(route.flag,true)?;
    game.move_map_target=BlockId::from(route.block);game.initial_area_entity_id=route.entry;game.warp_requested=true;
    state.stormveil=Some((Instant::now(),player.chr_ins.modules.data.hp,player.chr_ins.modules.data.fp,false));
    state.stormveil_barrier.reset();
    state.room=None;state.party_scene=None;state.pending=None;state.travel=None;state.command=None;
    let message=if run.regional_event.chariot{"泥头车生存：正在前往竞技场中心".into()}else{format!("{}：抵达 Boss 雾门前",route.name)};
    state.view.run=Some(run);state.view.room=Some(super::rooms::RoomView{message,loading:true,completed:false});
    state.view.busy=false;state.view.travelling=false;state.view.enemy_hp=None;
    state.pad_capture.update(false);super::input::CAPTURE.store(false,std::sync::atomic::Ordering::Relaxed);
    if state.view.run.as_ref().is_some_and(|r|r.regional_event.chariot) {
        report(state,"正在前往竞技场中心。全队到齐后倒数 5 秒，战车出现后坚持 60 秒。");
    }else{report(state,format!("正在前往{}。抵达 Boss 雾门前即可完成，无需击败 Boss。",route.name));}Ok(())
}
#[derive(Clone,Copy)]
enum ChariotClock {Warmup(Instant),Loading(Instant),Surviving(Instant)}
impl ChariotClock {
    fn tick(&mut self,now:Instant,loaded:bool)->(bool,bool,u64) {
        match *self {
            Self::Warmup(start)=>{
                let elapsed=now.duration_since(start).as_secs();
                if elapsed>=5 {*self=Self::Loading(now);(true,false,60)}else{(false,false,5-elapsed)}
            },
            Self::Loading(_)=>{if loaded{*self=Self::Surviving(now);}(true,false,60)},
            Self::Surviving(start)=>{
                if !loaded {*self=Self::Loading(now);return (true,false,60);}
                let elapsed=now.duration_since(start).as_secs();(true,elapsed>=60,60u64.saturating_sub(elapsed))
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
        player.chr_ins.modules.data.hp=player.chr_ins.modules.data.hp.min(*hp);
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
        let mut next=state.view.run.clone().ok_or("旅程未加载")?;next.stormveil_decide(Decision::Complete)?;record_checked(&next)?;state.view.run=Some(next);
        super::log("Chariot survival complete; personal legendary chest unlocked");
    }
    set_flag(45007881,false)?;
    if flag(45007882)?{return Ok(());}
    set_flag(45007883,true)?;
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
    let route=parkour_route(state.view.run.as_ref().ok_or("旅程未加载")?.regional_event.chariot);
    let player=world.main_player.as_mut().ok_or("角色正在加载")?;
    let (started,hp,fp,restored)=state.stormveil.as_mut().unwrap();
    let block=i32::from(player.current_block_id);
    if game.warp_requested||block!=route.block {
        state.stormveil_barrier.reset();
        if started.elapsed()>Duration::from_secs(60)&&!*restored{return Err(format!("{}加载超时",route.name));}
        return Ok(());
    }
    let alive=player.chr_ins.modules.data.hp>0&&!player.chr_ins.chr_flags1c5.death_flag();
    if alive {state.stormveil_barrier.apply(player);}else{state.stormveil_barrier.reset();}
    if !alive&&state.party.is_none() {
        if let Some(run)=state.view.run.as_mut(){run.end_on_death();record_checked(run)?;}
        return Ok(());
    }
    let grounded=alive&&player.chr_ins.modules.physics.standing_on_solid_ground&&!player.chr_ins.modules.physics.is_falling;
    if !*restored&&grounded {
        super::ailments::clear(&mut player.chr_ins)?;
        player.chr_ins.modules.data.hp=player.chr_ins.modules.data.hp.min(*hp);
        player.chr_ins.modules.data.fp=player.chr_ins.modules.data.fp.min(*fp);*restored=true;
        if let Some(view)=state.view.room.as_mut(){view.loading=false;}
    }
    let pos=&player.block_position;
    let reached=*restored&&route.reached(block,[pos.x,pos.y,pos.z],grounded);
    let complete=if let Some(link)=state.party.as_mut() {
        link.event.storm_finish=reached;
        link.event.storm_decision==Some(Decision::Complete)
    }else{reached};
    if !complete{return Ok(());}
    let mut next=state.view.run.clone().ok_or("旅程未加载")?;
    if next.regional_event.stormveil!=Some(Progress::Running){return Err("跑城进度不匹配".into());}
    super::elite::validate()?;
    next.stormveil_decide(Decision::Complete)?;next.leave_regional_event()?;
    record_checked(&next)?;
    super::run_screen::trial_rewards(state.view.run.as_ref().unwrap(),&next);
    state.view.run=Some(next);
    super::native_talk::set_flag(route.flag,false)?;
    state.stormveil=None;state.view.room=None;state.view.busy=false;
    report(state,"挑战完成，请确认本次获得的护符赐福。");Ok(())
}

fn start_descent(state:&mut Runtime,world:&WorldChrMan,game:&mut GameMan,mut run:Run)->Result<(),String> {
    let player=world.main_player.as_ref().ok_or("角色尚未加载")?;
    if game.warp_requested||!player.chr_ins.modules.physics.standing_on_solid_ground{return Err("请等待角色站稳".into());}
    if !run.regional_event.descent_started {
        if run.regional_event.opened==0&&run.regional_event.receipt.is_none(){return Err("请先与引导者交谈并参加试炼".into());}
        run.regional_event.descent_started=true;
    }
    record_checked(&run)?;
    for id in 35007990..=35007994 {super::native_talk::set_flag(id,false)?;}
    super::native_talk::set_flag(35007990,true)?;
    game.move_map_target=BlockId::from(0x23000000);
    game.initial_area_entity_id=if run.regional_event.opened!=0||run.regional_event.receipt.is_some(){35000989}else{35000988};
    game.warp_requested=true;
    state.descent=Some((Instant::now(),player.chr_ins.modules.data.hp,player.chr_ins.modules.data.fp,false));
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
    if !*restored {
        if !grounded{return Ok(());}
        super::ailments::clear(&mut player.chr_ins)?;
        player.chr_ins.modules.data.hp=player.chr_ins.modules.data.hp.min(*hp);
        player.chr_ins.modules.data.fp=player.chr_ins.modules.data.fp.min(*fp);*restored=true;
        if let Some(view)=state.view.room.as_mut(){view.loading=false;}
    }
    let run=state.view.run.as_ref().ok_or("旅程未加载")?;
    let Phase::Node{id}=run.phase else{return Err("是男人就下一百层：试炼进度不匹配".into());};
    if !run.regional_event.descent{return Err("当前不是「是男人就下一百层」试炼".into());}
    let requested=flag(35007991)?;
    let pos=&player.block_position;
    let at_finish=grounded&&(pos.y+408.535).abs()<2.0&&(pos.x-125.644).powi(2)+(pos.z+61.213).powi(2)<25.0;
    let data=unsafe {player.player_game_data.as_mut()};
    if run.regional_event.opened==0&&run.regional_event.receipt.is_none()&&requested&&at_finish {
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
fn request_travel(
    state: &mut Runtime,
    world: &mut WorldChrMan,
    game: &mut GameMan,
    id: usize,
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
    if let Some(test)=run.boss_test.filter(|t|run.debug&&t.act==run.act&&kind==erdtree_tower::NodeKind::Boss) {
        super::enemies::boss_test_roster(test.encounter,state.party.is_some(),erdtree_tower::dlc::enemies_allowed())?;
    }
    if kind.combat(){super::arenas::require_resources(super::arenas::for_run(run,state.party.is_some()))?;}
    if !kind.combat() && super::rooms::destination(kind).is_none() {
        let mut next = run.clone();
        next.enter(id)?;
        state.view.run = Some(next);
        state.view.busy = false;
        report(state, "请选择节点行动。");
        return Ok(());
    }
    let player = world.main_player.as_ref().ok_or("角色未就绪")?;
    if game.warp_requested
        || !player.chr_ins.modules.physics.standing_on_solid_ground
        || player.chr_ins.chr_flags1c5.is_invincible()
    {
        return Err("请等待角色站稳并退出原版菜单后再出发。".into());
    }
    if super::rooms::destination(kind).is_some() {
        clear_chariot()?;
        state.event_start=None;
        let shop_stock=if kind == erdtree_tower::NodeKind::Shop {
            Some(super::native_shop::prepare(run,id,true,unsafe {&mut *player.player_game_data.as_ptr()})?)
        }else{None};
        if kind == erdtree_tower::NodeKind::Chest {
            super::loot::prepare(run, id,unsafe {player.player_game_data.as_ref()})?;
        }
        let mut room = super::rooms::Room::new(
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
        game.move_map_target = BlockId::from(room.block);
        game.initial_area_entity_id = room.entry;
        game.warp_requested = true;
        state.view.run = Some(next);
        state.view.room = Some(room.view.clone());
        state.room = Some(room);
        state.pending = None;
        state.view.enemy_hp = None;
        state.pad_capture.update(false);
        super::input::CAPTURE.store(false, std::sync::atomic::Ordering::Relaxed);
        report(state, "正在前往服务房间。");
        return Ok(());
    }
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
    state.travel = Some((
        super::travel::Travel::new(id),
        Instant::now(),
        player.chr_ins.modules.data.hp,
        player.chr_ins.modules.data.fp,
    ));
    state.pending = None;
    state.view.enemy_hp = None;
    state.view.travelling = true;
    state.pad_capture.update(false);
    super::input::CAPTURE.store(false, std::sync::atomic::Ordering::Relaxed);
    let arena=super::arenas::for_run(run,state.party.is_some());
    game.move_map_target = BlockId::from(arena.block);
    game.initial_area_entity_id = arena.combat_entry();
    game.warp_requested = true;
    report(state, format!("正在前往{}，{}",arena.name,if arena.uses_circle(){"靠近场地中心自动开始战斗。"}else{"穿过雾门开始战斗。"}));
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn summoner_refund_keeps_the_first_ash_when_a_twin_spirit_follows() {
        use super::first_used_ash;
        let before=[(212000,1),(232000,1)];
        // 第一只召唤：记下第一个骨灰
        assert_eq!(first_used_ash(None,&before,[212000,0]),Some(212000));
        // 双生的灵唤铃召唤第二只：requested / active 换成第二个骨灰，已经记下的第一个不变
        assert_eq!(first_used_ash(Some(212000),&before,[212000,232000]),None);
        assert_eq!(first_used_ash(Some(212000),&before,[232000,232000]),None);
        // 请求里带分类高位、或不在开战快照里的骨灰都按原规则处理
        assert_eq!(first_used_ash(None,&before,[0x4000_0000|232000,0]),Some(232000));
        assert_eq!(first_used_ash(None,&before,[999000,0]),None);
        assert_eq!(first_used_ash(None,&before,[0,212000]),Some(212000));
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
        assert!(!super::ash_refund_ready(Some(true),true,&mut guard));
        assert_eq!(guard,Some(earned));
        assert!(super::ash_refund_ready(Some(true),false,&mut guard));
        assert_eq!(guard,Some(earned));
        assert!(super::ash_refund_ready(None,false,&mut guard));
        assert_eq!(guard,Some(earned));
        assert!(!super::ash_refund_ready(Some(false),false,&mut guard));
        assert!(guard.is_none());
    }
    #[test]
    fn ash_tracking_includes_preparation_and_does_not_restart_at_combat() {
        assert!(super::ash_tracking_active(true,false));
        assert!(super::ash_tracking_active(true,true));
        assert!(super::ash_tracking_active(false,true));
        assert!(!super::ash_tracking_active(false,false));
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
        super::View{shared_event:false,ballots:vec![],run:Some(erdtree_tower::Run::new(42,erdtree_tower::Class::Samurai)),
            status:String::new(),busy:false,enemy_hp:None,prompt:1,ready:true,travelling:false,room:None,runes:0,boss_affixes:None,boss_alert:None,boss_affix_list:vec![],law_clock:None,build:vec![],dummy:None,stats:Default::default()}
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
    fn chariot_uses_arena_and_waits_for_loaded_car_before_sixty_seconds() {
        let route=super::parkour_route(true);
        assert_eq!(route.block,0x2d010000);assert_eq!(route.entry,45012880);
        assert_ne!(route.entry,super::parkour_route(false).entry);
        assert_ne!(route.flag,super::parkour_route(false).flag);
        use std::time::{Instant,Duration};
        let now=Instant::now();let mut clock=super::ChariotClock::Warmup(now);
        assert_eq!(clock.tick(now+Duration::from_secs(4),false),(false,false,1));
        assert_eq!(clock.tick(now+Duration::from_secs(5),false),(true,false,60));
        assert_eq!(clock.tick(now+Duration::from_secs(65),false),(true,false,60));
        assert_eq!(clock.tick(now+Duration::from_secs(66),true),(true,false,60));
        assert_eq!(clock.tick(now+Duration::from_secs(125),true),(true,false,1));
        assert_eq!(clock.tick(now+Duration::from_secs(126),true),(true,true,0));
        assert_eq!(clock.tick(now+Duration::from_secs(127),false),(true,false,60));
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
        let retry=super::lobby_reconnect_error(Some("队员已重新连接，需要重新建立会话"));
        assert!(retry);assert!(super::rebuild_lobby(retry,true,true,None));
        assert!(!super::rebuild_lobby(retry,false,true,None));
        assert!(!super::rebuild_lobby(retry,true,false,None));
        for phase in [Phase::Map,Phase::Combat{id:0},Phase::Node{id:0},Phase::Dead] {
            assert!(!super::rebuild_lobby(retry,true,true,Some(&phase)));
        }
        assert!(super::lobby_reconnect_error(Some("队友报告游戏执行失败，请查看队友日志")));
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
