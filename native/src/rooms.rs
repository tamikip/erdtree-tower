use eldenring::cs::{
    CSEventFlagMan, ChrDebugSpawnRequest, ChrLoadStatus, FieldInsHandle, GameMan, NpcParam, NpcThinkParam,
    SoloParamRepository, WorldChrMan,
};
use erdtree_tower::NodeKind;
use fromsoftware_shared::{F32Vector4, FromStatic};
use std::time::{Duration, Instant};

fn merchant_collision(flags:&mut eldenring::cs::ChrCtrlFlags,down:bool) {
    flags.set_disable_hit(down);
    flags.set_disable_player_collision(down);
    flags.set_disable_character_capsule_collision(down);
}
fn patches_reaction(angry:bool,dead:bool,before:Option<i32>,hp:i32)->(bool,bool) {
    (angry||hp<=0||before.is_some_and(|old|hp<old),dead||hp<=0)
}
#[test]
fn patches_remains_hostile_after_healing_and_dead_after_reload() {
    assert_eq!(patches_reaction(false,false,None,1551),(false,false));
    assert_eq!(patches_reaction(false,false,Some(1551),1500),(true,false));
    assert_eq!(patches_reaction(true,false,Some(1500),1551),(true,false));
    assert_eq!(patches_reaction(false,false,Some(1551),0),(true,true));
    assert_eq!(patches_reaction(true,true,None,1551),(true,true));
}

fn first_stale<T>(bindings:&[(T,usize)],mut lookup:impl FnMut(&T)->Option<usize>)->Option<usize> {
    bindings.iter().position(|(handle,address)|lookup(handle)!=Some(*address))
}
const SPAWN_SETTLE:Duration=Duration::from_secs(3);
fn spawn_settled(since:&mut Option<Instant>,now:Instant,ready:bool)->bool {
    if !ready {*since=None;return false;}
    now.saturating_duration_since(*since.get_or_insert(now))>=SPAWN_SETTLE
}
fn service(hub:bool,kind:NodeKind,index:usize)->u32 {
    if kind==NodeKind::Event {return 6;}
    if hub {if index==2 {5}else{3+index as u32}} else if kind==NodeKind::Shop {if index==0 {2}else{5}} else if kind!=NodeKind::Camp||index==2 {5} else {index as u32}
}
fn npc_data(model:i32,role:u32)->(i32,i32,i32) {
    match model {2180=>(21800000,21800000,-1),3451=>(34510179,34519000,-1),0 if role==4||role==5=>(523180079,523180000,23180),0=>(523200079,523200000,23200),_=>(model*10000,model*10000,-1)}
}
fn loaded(status:ChrLoadStatus)->bool {matches!(status,ChrLoadStatus::Active|ChrLoadStatus::ReadyForActivation)}
fn map_service_matches(slot:usize,entity:u32,model:i32,param:i32,status:ChrLoadStatus,hp:i32)->bool {
    let expected=match slot {0=>(0,523200079),1=>(0,523180079),_=>return false};
    entity==45001800+slot as u32&&(model,param)==expected&&loaded(status)&&hp>0
}
fn player_model(model:i32)->u8 {u8::from(model==0)}
fn conditional_services(block:i32,kind:NodeKind)->Vec<(u32,i32,i32,u32)> {
    if block==0x2d010000&&kind==NodeKind::Event{return super::event_hosts::service(0);}
    if block==0x2d000000 && kind==NodeKind::Camp {
        return vec![(45001800,0,523200079,0),(45001801,3451,34510179,1),(45001802,2180,21800000,5)];
    }
    static SERVICES:std::sync::LazyLock<Vec<(i32,NodeKind,(u32,i32,i32,u32))>>=std::sync::LazyLock::new(||include_str!("../../scene-assets/party-services.txt").lines().filter_map(|line| {
        let p:Vec<_>=line.split('|').collect();
        let map:Vec<u8>=p[0][1..].split('_').map(|v|v.parse().unwrap()).collect();
        let kind=match p[1]{"Shop"=>NodeKind::Shop,"Chest"=>NodeKind::Chest,"Event"=>NodeKind::Event,"Monster"=>NodeKind::Monster,_=>return None};
        let role=p[7].parse().unwrap();
        Some((i32::from_be_bytes([map[0],map[1],map[2],map[3]]),kind,(p[2].parse().unwrap(),if role==5{2180}else{p[3].parse().unwrap()},if role==5{21800000}else{p[4].parse().unwrap()},role)))
    }).collect());
    let block=if super::arenas::is_built(block) {0x2d020000}else{block};
    SERVICES.iter().filter(|(b,k,_)|*b==block&&*k==kind).map(|(_,_,service)|*service).collect()
}
fn camp_service_matches(slot:usize,entity:u32,model:i32,param:i32,status:ChrLoadStatus,hp:i32)->bool {
    let expected=match slot {0=>(0,523200079),1=>(3451,34510179),2=>(2180,21800000),_=>return false};
    entity==45001800+slot as u32&&(model,param)==expected&&loaded(status)&&hp>0
}

#[derive(Clone)]
pub struct RoomView {
    pub message: String,
    pub loading: bool,
    pub completed: bool,
}

pub struct Room {
    pub mausoleum:bool,
    pub arrival_block_position:Option<super::party_death::Position>,
    pub copy_requested:bool,
    merchant_hit_logged:bool,
    pub merchant_down:bool,
    pub shop_refresh_pending:bool,
    /// 上次读到的商品售出计数（诊断：联机客机商品无故变空）
    pub shop_counters:Option<(std::time::Instant,Vec<u8>)>,
    pub hunter:Option<super::map_guards::Guards>,
    hub: bool,
    event_host:usize,
    /// 三重宝匣不放引导 NPC：直接开箱，房间里只有三个箱子
    event_hostless:bool,
    patches_angry:bool,
    patches_dead:bool,
    patches_hp:Option<i32>,
    pub node: usize,
    pub kind: NodeKind,
    pub block: i32,
    pub entry: u32,
    started: Instant,
    pending_since: Option<Instant>,
    // [fix] When the player first stood still in this room; debug NPC spawns wait for it.
    settled: Option<Instant>,
    handles: Vec<(FieldInsHandle, usize)>,
    talks: Vec<super::native_talk::NativeTalk>,
    saved_hp: i32,
    saved_fp: i32,
    restored: bool,
    pub allowance_issued: bool,
    /// 营地发放强化石到账时身上的强化石数量：少于它就说明营地的一次服务用掉了
    pub allowance_target: u32,
    allowance:super::smith::Grant,
    pub view: RoomView,
    pub scarabs:super::scarabs::Hunt,
    pub chest_attempt:Option<Instant>,
}
impl Room {
    fn patches_host(&self)->bool {self.kind==NodeKind::Event&&!self.event_hostless&&super::event_hosts::HOSTS[self.event_host].draw==erdtree_tower::events::Draw::Dice}
    pub fn patches_state(&self)->Option<(bool,bool)> {self.patches_host().then_some((self.patches_angry,self.patches_dead))}
    pub fn merchant_dead(&self,world:&WorldChrMan)->bool {
        self.kind==NodeKind::Shop&&self.handles.first().is_some_and(|(handle,address)|world.chr_ins_by_handle(handle)
            .is_some_and(|c|std::ptr::from_ref(c) as usize==*address&&!c.debug_flags.character_disabled()
                &&loaded(unsafe {c.chr_set_entry.as_ref()}.chr_load_status)&&(c.modules.data.hp<=0||c.chr_flags1c5.death_flag())))
    }
    fn map_backed(&self)->bool {self.hub||self.kind==NodeKind::Camp||!conditional_services(self.block,self.kind).is_empty()}
    pub fn dialog_active(&self)->bool {self.talks.iter().any(|t|t.active())}
    pub fn suspend_dialogues(&mut self)->Result<(),String> {
        for talk in &mut self.talks {talk.suspend()?;}
        Ok(())
    }
    pub fn set_enemies(&mut self,enemies:&[i32]) {for talk in &mut self.talks {talk.set_enemies(enemies);}}
    pub fn hub(hp: i32, fp: i32) -> Result<Self, String> {
        Self::create(0,NodeKind::Camp,hp,fp,None,true)
    }
    pub fn hub_choice(&mut self) -> Option<(usize,i32)> {
        self.talks.iter_mut().enumerate().find_map(|(i,t)|t.choice.take().map(|c|(i,c)))
    }
    pub fn notice(&mut self,text:i32)->Result<(),String>{
        if let Some(talk)=self.talks.first_mut(){talk.dialog(text)?;} Ok(())
    }
    pub fn take_refill(&mut self)->Option<i32> {
        self.talks.iter_mut().find_map(|talk|talk.take_refill())
    }
    pub fn event_menu(&mut self,run:&erdtree_tower::Run,party_ready:bool,party_busy:bool,solo:bool) {
        if self.mausoleum {
            let copy=run.regional_event.mausoleum.as_ref();
            let menu=super::native_talk::EventMenu{used:copy.is_some_and(|c|c.completed),busy:copy.is_some_and(|c|c.receipt.is_some()),..Default::default()};
            for talk in &mut self.talks {talk.set_event_menu(menu);}return;
        }
        self.event_host=super::event_hosts::index(if run.regional_event.mausoleum.is_some(){erdtree_tower::events::Draw::Altar}else{run.event_draw(self.node)});
        let progress=&run.regional_event;
        self.patches_angry|=progress.patches_angry;self.patches_dead|=progress.patches_dead;
        self.event_hostless=progress.mausoleum.is_none()&&!progress.trial&&progress.stormveil.is_none()&&!progress.descent&&!progress.recovery
            &&!progress.scarabs&&!progress.locked&&run.gamble(self.node).filter(|_|solo).is_none()&&!run.altar(self.node);
        // 死亡重试回到已结束的事件：引导者不再提供任何选项
        if progress.settled {
            let menu=super::native_talk::EventMenu{trial:true,used:true,..Default::default()};
            for talk in &mut self.talks {talk.set_event_menu(menu);}
            return;
        }
        let menu=super::native_talk::EventMenu{mausoleum_offer:progress.mausoleum.is_some(),gamble:run.gamble(self.node).filter(|_|solo).map(|g|g as u8),trial:progress.trial,chariot:progress.chariot,stormveil:progress.stormveil.is_some(),descent:progress.descent,recovery:progress.recovery,scarabs:progress.scarabs,locked:progress.locked,
            altar:run.altar(self.node),used:progress.opened!=0||progress.stormveil.is_some_and(|s|s!=erdtree_tower::stormveil::Progress::Choosing),
            busy:progress.guards||self.scarabs.active()||party_busy||self.patches_angry||self.patches_dead,ready:self.scarabs.ready()||party_ready,attempts:progress.attempts};
        for talk in &mut self.talks {talk.set_event_menu(menu);}
    }
    pub fn take_event_request(&mut self)->Option<super::native_talk::EventRequest> {
        self.talks.iter_mut().find_map(|talk|talk.take_event_request())
    }
    pub fn consume_key(&mut self)->Result<(),String>{
        self.talks.first_mut().ok_or("等待事件交互加载")?.consume_key()
    }
    pub fn new(node: usize, kind: NodeKind, hp: i32, fp: i32) -> Result<Self, String> {
        Self::create(node,kind,hp,fp,None,false)
    }
    pub fn for_run(run:&erdtree_tower::Run,node:usize,kind:NodeKind,hp:i32,fp:i32)->Result<Self,String> {
        let mut room=Self::new(node,kind,hp,fp)?;
        if kind==NodeKind::Event&&run.regional_event.mausoleum.as_ref().is_some_and(|copy|copy.accepted) {
            super::mausoleum::restore_flow(run.regional_event.mausoleum.as_ref().unwrap())?;
            let arena=super::arenas::for_block(0x0d000000).ok_or("龙王平台资源未接入")?;
            room.mausoleum=true;
            room.block=arena.block;room.entry=arena.entry;
            room.view.message="正在前往灵庙……".into();
            super::log(format!("MAUSOLEUM room node={node} block={:08x} entry={}",room.block,room.entry));
        }
        super::native_talk::set_flag(45011859,room.mausoleum)?;
        Ok(room)
    }
    pub fn attendant(block:i32)->Result<Self,String> {
        let arena=super::arenas::for_block(block).ok_or("未知竞技场")?;
        Self::create(0,NodeKind::Monster,0,0,Some(arena),false)
    }
    fn create(node:usize,kind:NodeKind,hp:i32,fp:i32,attendant:Option<super::arenas::Arena>,hub:bool)->Result<Self,String> {
        let (block, entry, title) = if hub {(super::hub::BLOCK,11102691,"大赐福")} else if let Some(arena)=attendant {(arena.block,arena.entry,"场边引导者")} else {destination(kind).ok_or("此节点没有房间")?};
        let params = unsafe { SoloParamRepository::instance() }.map_err(|_| "NPC 参数未就绪")?;
        if kind==NodeKind::Event&&super::event_hosts::HOSTS.iter().any(|h|!params.get::<NpcParam>(h.npc as u32).is_some_and(|p|p.name_id()==h.name_id)) {
            return Err("事件 NPC 名称参数缺失，请更新配套 regulation.bin".into());
        }
        let models=if hub {vec![0,0]}else{npcs(kind).into_iter().map(|(model,_)|model).collect()};
        for (index,model) in models.into_iter().enumerate() {
            let (npc,think,_)=npc_data(model,service(hub,kind,index));
            if params.get::<NpcParam>(npc as u32).is_none()
                || params
                    .get::<NpcThinkParam>(think as u32)
                    .is_none()
            {
                return Err(format!("服务 NPC {model} 参数不存在"));
            }
        }
        if !hub {
        let flags = unsafe { CSEventFlagMan::instance_mut() }.map_err(|_| "场景事件系统未就绪")?;
        flags.virtual_memory_flag.set_flag(45010939,kind==NodeKind::Chest);
        if matches!(kind,NodeKind::Event|NodeKind::Chest) {
            for offset in 940..=969 { flags.virtual_memory_flag.set_flag(45010000+offset,false); }
        }
        if kind!=NodeKind::Monster { for offset in 993..=999 {
            flags
                .virtual_memory_flag
                .set_flag(ready_flag(kind) - 991 + offset, false);
        }
        flags.virtual_memory_flag.set_flag(ready_flag(kind), false);
        flags
            .virtual_memory_flag
            .set_flag(ready_flag(kind) + 1, true);
        if !flags.virtual_memory_flag.get_flag(ready_flag(kind) + 1) {
            return Err("房间事件标记不可用，未传送".into());
        }
        }
        }
        super::log(format!("Native room {node}: {title}, block={block:x}"));
        Ok(Self {
            mausoleum:false,arrival_block_position:None,copy_requested:false,
            merchant_down:false,shop_refresh_pending:false,shop_counters:None,hunter:None,merchant_hit_logged:false,
            scarabs:Default::default(),
            chest_attempt:None,
            hub,event_host:0,event_hostless:false,patches_angry:false,patches_dead:false,patches_hp:None,
            node,
            kind,
            block,
            entry,
            started: Instant::now(),
            pending_since: None,
            settled: None,
            handles: Vec::new(),
            talks: Vec::new(),
            saved_hp: hp,
            saved_fp: fp,
            restored: kind==NodeKind::Monster,
            allowance_issued: false,
            allowance_target: 0,
            allowance:super::smith::Grant::default(),
            view: RoomView {
                message: "正在前往服务房间……".into(),
                loading: true,
                completed: false,
            },
        })
    }
    pub fn poll(&mut self, world: &mut WorldChrMan, game: &GameMan) -> Result<(), String> {
        let player = world.main_player.as_ref().ok_or("等待角色加载")?;
        let arrived = i32::from(player.current_block_id) == self.block
            && !game.warp_requested
            && self.started.elapsed() > Duration::from_secs(1);
        let grounded = player.chr_ins.modules.physics.standing_on_solid_ground
            && !player.chr_ins.modules.physics.is_falling
            && player.chr_ins.modules.data.hp > 0
            && !player.chr_ins.chr_flags1c5.is_invincible();
        let spawn_ready=spawn_settled(&mut self.settled,Instant::now(),arrived&&grounded);
        if !arrived || (!self.restored && !grounded) {
            self.view.loading = true;
            if self.started.elapsed() > Duration::from_secs(60) {
                return Err("服务房间传送超时，未消耗节点".into());
            }
            return Ok(());
        }
        let flags = unsafe { CSEventFlagMan::instance() }.map_err(|_| "场景事件系统未就绪")?;
        if !self.hub && !self.mausoleum && self.kind!=NodeKind::Monster && !flags.virtual_memory_flag.get_flag(ready_flag(self.kind)) {
            self.view.message = "正在等待房间地图和交互物件就绪……".into();
            if self.started.elapsed() > Duration::from_secs(60) {
                return Err("服务房间事件未加载，请使用包含 scene-assets 的新启动包".into());
            }
            return Ok(());
        }
        if !self.restored {
            let player = world.main_player.as_mut().unwrap();
            if self.mausoleum {
                let p=&player.block_position;
                self.arrival_block_position=Some(super::party_death::Position{block:self.block,xyz:[p.x,p.y,p.z]});
                super::log(format!("MAUSOLEUM arrival recorded node={} block_position={:?}",self.node,self.arrival_block_position));
            }
            super::ailments::clear(&mut player.chr_ins)?;
            super::travel::restore_arrival_hp(&mut player.chr_ins,self.saved_hp,"service room arrival");
            player.chr_ins.modules.data.fp = player.chr_ins.modules.data.fp.min(self.saved_fp);
            self.restored = true;
            super::log(format!(
                "Native room {} arrived; event handshake received",
                self.node
            ));
        }
        if self.mausoleum {
            let (lowered,opened)=super::mausoleum::flow()?;
            super::mausoleum::trace(world,self.node);
            self.view.loading=false;
            if !lowered||!opened {
                self.view.message=if lowered{"灵庙已经跪下：靠近大门，使用原生交互开门。".into()}else{format!("骷髅清除：{}/{}（共 44 块）。达标后等待灵庙跪下，再开门进入。",super::mausoleum::broken_skulls()?,super::mausoleum::REQUIRED_SKULLS)};
                return Ok(());
            }
            if self.talks.is_empty() {
                if let Some(chr)=world.chr_sets.iter().flatten().flat_map(|set|set.characters()).find(|c|c.event_entity_id==45011860&&c.npc_id==4450&&!c.debug_flags.character_disabled()&&loaded(unsafe {c.chr_set_entry.as_ref()}.chr_load_status)) {
                    self.talks.push(super::native_talk::NativeTalk::new(self.block,chr.field_ins_handle,7));
                }else{return Err("等待灵庙祭台加载".into());}
            }
            self.view.loading=false;
            self.talks[0].poll()?;return Ok(());
        }
        if self.patches_host() {
            if let Some((handle,address))=self.handles.first()&&let Some(chr)=world.chr_ins_by_handle(handle).filter(|c|std::ptr::from_ref(*c) as usize==*address) {
                (self.patches_angry,self.patches_dead)=patches_reaction(self.patches_angry,self.patches_dead,self.patches_hp,chr.modules.data.hp);
                self.patches_hp=Some(chr.modules.data.hp);
            }
            if self.patches_angry||self.patches_dead {self.suspend_dialogues()?;for talk in &mut self.talks {let _=talk.take_event_request();}}
            if self.patches_dead {
                super::native_talk::set_flag(super::event_hosts::HOSTS[self.event_host].flag,false)?;
                super::native_talk::set_flag(self.base()+993,true)?;
                self.view.loading=false;return Ok(());
            }
        }
        let specs = if self.hub {vec![(0,[-329.5,-21.9,-309.0]),(0,[-329.5,-21.9,-302.0])]} else if self.kind==NodeKind::Event&&self.event_hostless {vec![]} else {npcs(self.kind)};
        if let Some(index)=first_stale(&self.handles,|handle|world.chr_ins_by_handle(handle)
            .filter(|c|(self.kind==NodeKind::Shop&&self.handles.first().is_some_and(|(h,_)|h==handle))||(!c.debug_flags.character_disabled()&&c.modules.data.hp>0&&!c.chr_flags1c5.death_flag()))
            .map(|c|std::ptr::from_ref(c) as usize)) {
            // Complete an outstanding spawn before rebuilding the ordered service slots.
            if self.pending_since.is_none() {
                for &(handle,address) in self.handles[index..].iter().filter(|_|!self.map_backed()) {
                    if let Some(chr)=world.chr_ins_by_handle_mut(&handle)
                        && std::ptr::from_ref(chr) as usize==address {
                        chr.debug_flags.set_character_disabled(true);
                    }
                }
                self.handles.truncate(index);
                self.talks.truncate(index);
                if self.map_backed() {self.started=Instant::now();}
                self.view.loading=true;
                self.view.message="正在恢复房间 NPC……".into();
                super::log(format!("Native NPC unloaded; rebuilding service slots from {index}"));
            }
        }
        if self.handles.len()<specs.len() && !grounded {return Ok(());}
        let conditional=if self.kind==NodeKind::Event&&self.event_hostless {Vec::new()} else if self.kind==NodeKind::Event {super::event_hosts::service(self.event_host)}else{conditional_services(self.block,self.kind)};
        if self.kind==NodeKind::Event {
            super::native_talk::set_flag(45011871,false)?;
            for host in super::event_hosts::HOSTS.iter().filter(|h|conditional.first().is_none_or(|c|h.entity!=c.0)) {super::native_talk::set_flag(host.flag,false)?;}
        }
        for &(id,_,_,_) in &conditional {
            let flag=super::event_hosts::visibility_flag(id);
            super::native_talk::set_flag(flag,true)?;
            if !super::native_talk::flag(flag)? {return Err(format!("服务 NPC {id} 显示标记 {flag} 不可用"));}
        }
        if self.map_backed() && self.handles.len()<specs.len() {
            let slot=self.handles.len();
            let matches=|slot,entity,model,param,status,hp| {
                if self.hub {map_service_matches(slot,entity,model,param,status,hp)}
                else if self.kind==NodeKind::Camp {camp_service_matches(slot,entity,model,param,status,hp)}
                else {conditional.get(slot).is_some_and(|&(id,m,p,_)|(entity,model,param)==(id,m,p))&&loaded(status)&&(hp>0||self.kind==NodeKind::Shop&&slot==0&&self.merchant_down)}
            };
            let found=world.chr_sets.iter().flatten().flat_map(|set|set.characters()).find(|chr|
                matches(slot,chr.event_entity_id,chr.npc_id,chr.npc_param_id,
                    unsafe {chr.chr_set_entry.as_ref()}.chr_load_status,chr.modules.data.hp)
                &&((self.kind==NodeKind::Shop&&slot==0&&self.merchant_down)||(!chr.debug_flags.character_disabled()&&!chr.chr_flags1c5.death_flag())))
                .map(|chr|(chr.field_ins_handle,std::ptr::from_ref(chr) as usize));
            if let Some((handle,address))=found {
                self.handles.push((handle,address));
                self.talks.push(super::native_talk::NativeTalk::new(self.block,handle,service(self.hub,self.kind,slot)));
                super::log(format!("Map-native service NPC bound: block={:x}, slot={slot}, handle={handle}",self.block));
            } else if self.started.elapsed()>Duration::from_secs(60) {
                return Err(format!("服务 NPC {} 未加载，请检查配套地图补丁",slot+1));
            }
            self.view.message="正在等待服务 NPC……".into();
        } else if !self.hub && self.handles.len() < specs.len() {
            let (model, pos) = specs[self.handles.len()];
            let (npc,think,init_id)=npc_data(model,service(self.hub,self.kind,self.handles.len()));
            if let Some(at) = self.pending_since {
                if let Some(created) = world.debug_chr_creator.last_created_chr {
                    let chr = unsafe { created.as_ref() };
                    if chr.npc_id == model
                        && chr.npc_param_id == npc
                        && chr.modules.data.hp > 0
                        && loaded(unsafe {chr.chr_set_entry.as_ref()}.chr_load_status)
                        && !self.handles.iter().any(|(handle,_)|*handle==chr.field_ins_handle)
                    {
                        let handle = chr.field_ins_handle;
                        let address = std::ptr::from_ref(chr) as usize;
                        if world
                            .chr_ins_by_handle(&handle)
                            .is_some_and(|c| std::ptr::from_ref(c) as usize == address)
                        {
                            self.handles.push((handle, address));
                            let service = service(self.hub,self.kind,self.talks.len());
                            self.talks.push(super::native_talk::NativeTalk::new(self.block, handle, service));
                            super::log(format!("Native NPC bound: model={model}, handle={handle}, service={service}"));
                            self.pending_since = None;
                            world.debug_chr_creator.last_created_chr = None;
                        }
                    }
                }
                if self.pending_since.is_some() && at.elapsed() > Duration::from_secs(15) {
                    return Err(format!("服务 NPC {model} 生成超时，未消耗节点"));
                }
            } else if !spawn_ready {
                // [fix] Arena-arrival crashes (eldenring+0x2360a9 via ersc.dll) all happened in the
                // first seconds after this debug spawn was issued on a freshly loaded map. Wait until
                // the player has stood on solid ground for a moment before creating the NPC.
                self.view.message = "正在等待地图稳定……".into();
            } else if !world.debug_chr_creator.spawn {
                let player = world.main_player.as_ref().unwrap();
                let physics = &player.chr_ins.modules.physics;
                let arena=if self.kind==NodeKind::Monster {super::arenas::for_block(self.block)}else{None};
                let pos=arena.map_or(pos,|a|a.attendant_position());
                let xyz = [
                    physics.position.0 - player.block_position.x + pos[0],
                    physics.position.1 - player.block_position.y + pos[1],
                    physics.position.2 - player.block_position.z + pos[2],
                ];
                let init = &mut world.debug_chr_creator.init_data;
                let yaw=if self.hub {-1.57}else{super::enemies::facing(pos,arena.map_or([0.0,0.15,10.0],|a|a.center))};
                unsafe {
                    let bytes = std::ptr::from_mut(init).cast::<u8>();
                    bytes
                        .add(0x10)
                        .cast::<F32Vector4>()
                        .write(F32Vector4(0.0, yaw, 0.0, 0.0));
                    bytes.add(0xc8).write(player_model(model));
                }
                world.debug_chr_creator.last_created_chr = None;
                world.spawn_debug_character(&ChrDebugSpawnRequest {
                    chr_id: model,
                    npc_param_id: npc,
                    npc_think_param_id: think,
                    chara_init_param_id: init_id,
                    event_entity_id: ready_flag(self.kind) as i32 - 991
                        + 1800
                        + self.handles.len() as i32,
                    talk_id: -1,
                    pos_x: xyz[0],
                    pos_y: xyz[1],
                    pos_z: xyz[2],
                });
                self.pending_since = Some(Instant::now());
            }
            self.view.message = "正在布置 NPC……".into();
        }
        for (index,&(handle, address)) in self.handles.iter().enumerate() {
            let chr = world
                .chr_ins_by_handle_mut(&handle)
                .filter(|c| std::ptr::from_ref(*c) as usize == address)
                .ok_or("服务 NPC 已卸载")?;
            let merchant=self.kind==NodeKind::Shop&&index==0;
            let patches=self.patches_host()&&index==0;
            let vulnerable=merchant||patches;
            chr.team_type = if patches&&!self.patches_angry {26}else if vulnerable {6}else{0};
            chr.chr_flags1c5.set_is_invincible(!vulnerable);
            chr.debug_flags.set_disabled_hit(!vulnerable||self.merchant_down);
            if merchant&&self.merchant_down {chr.debug_flags.set_character_disabled(true);}
            chr.debug_flags.set_disabled_movement(!(patches&&self.patches_angry));
            chr.debug_flags.set_disabled_secondary_actions(!(patches&&self.patches_angry));
            if patches {merchant_collision(&mut chr.chr_ctrl.flags,false);}
            if merchant {
                let before=chr.chr_ctrl.flags;
                merchant_collision(&mut chr.chr_ctrl.flags,self.merchant_down);
                if !self.merchant_hit_logged {
                    super::log(format!("MERCHANT_HIT entity={} hp={} team={} invincible={} debug_hit_disabled={} ctrl_before={before:?} ctrl_after={:?}",chr.event_entity_id,chr.modules.data.hp,chr.team_type,chr.chr_flags1c5.is_invincible(),chr.debug_flags.disabled_hit(),chr.chr_ctrl.flags));
                    self.merchant_hit_logged=true;
                }
            }else if !patches{
                chr.chr_ctrl.flags.set_disable_player_collision(true);
                chr.chr_ctrl.flags.set_disable_character_capsule_collision(true);
            }
        }
        self.view.loading = self.handles.len() < specs.len();
        if self.view.loading {
            return Ok(());
        }
        if !self.hub && self.kind==NodeKind::Camp && !self.allowance_issued {
            let data=unsafe {world.main_player.as_mut().unwrap().player_game_data.as_mut()};
            let held=super::smith::remaining(data);
            match self.allowance.poll(self.started.elapsed(),held) {
                super::smith::GrantStep::Request=>{
                    self.view.loading=true;
                    self.view.message="正在发放本次营地的强化石……".into();
                    super::smith::prepare(data)?;
                    return Ok(());
                },
                super::smith::GrantStep::Waiting=>{self.view.loading=true;return Ok(());},
                super::smith::GrantStep::Ready=>{
                    self.allowance_issued=true;self.allowance_target=held;
                    super::log("Camp allowance confirmed; automatic issuance disabled for this room");
                },
                super::smith::GrantStep::Failed=>{
                    self.view.message="强化石尚未确认到账，已停止自动发放；补瓶、加点和免费菜单仍可使用。".into();
                },
            }
        }
        for talk in &mut self.talks {talk.set_camp_used(self.view.completed);}
        let skip=usize::from(self.merchant_down||self.patches_host()&&self.patches_angry);
        if self.merchant_down&&let Some(talk)=self.talks.first_mut(){talk.suspend()?;}
        if let Some((index,_)) = self.talks.iter().enumerate().skip(skip).find(|(_,talk)|talk.active()) {
            self.talks[index].poll()?;
        } else {
            for talk in self.talks.iter_mut().skip(skip) {
                talk.poll()?;
                if talk.active() { break; }
            }
        }
        if !self.hub && self.kind!=NodeKind::Monster { unsafe { CSEventFlagMan::instance_mut() }
            .map_err(|_| "原生交互事件未加载")?
            .virtual_memory_flag
            .set_flag(self.base() + 993, true); }
        if self.view.message == "正在布置 NPC……" || self.view.message.starts_with("正在")
        {
            self.view.message = "靠近交互对象按 E；通过传送门返回路线图。".into();
        }
        Ok(())
    }
    pub fn base(&self) -> u32 {
        ready_flag(self.kind) - 991
    }
    pub fn retire(&self, world: &mut WorldChrMan) {
        if self.mausoleum {let _=super::native_talk::set_flag(45011859,false);}
        if let Some(hunter)=&self.hunter {hunter.retire();}
        self.scarabs.retire(world);
        if !self.hub && self.kind!=NodeKind::Monster && let Ok(flags) = unsafe { CSEventFlagMan::instance_mut() } {
            flags.virtual_memory_flag.set_flag(self.base() + 993, false);
            if self.kind==NodeKind::Chest { flags.virtual_memory_flag.set_flag(45010939,false); }
            if self.kind==NodeKind::Event { flags.virtual_memory_flag.set_flag(45010940,false); }
        }
        for (id,_,_,_) in conditional_services(self.block,self.kind) {let _=super::native_talk::set_flag(super::event_hosts::visibility_flag(id),false);}
        if self.kind==NodeKind::Event {for host in super::event_hosts::HOSTS.iter(){let _=super::native_talk::set_flag(host.flag,false);}}
        for &(handle, address) in self.handles.iter().filter(|_|!self.map_backed()) {
            if let Some(chr) = world.chr_ins_by_handle_mut(&handle)
                && std::ptr::from_ref(chr) as usize == address
            {
                chr.debug_flags.set_character_disabled(true);
            }
        }
    }
    pub fn complete(&mut self, message: String) {
        self.view.completed = true;
        self.view.message = message;
    }
}
pub fn destination(kind: NodeKind) -> Option<(i32, u32, &'static str)> {
    match kind {
        NodeKind::Camp => Some((0x2d000000, 45002600, "篝火营地")),
        NodeKind::Chest => Some((0x2d010000, 45012600, "宝箱密室")),
        NodeKind::Event => Some((0x2d010000, 45012600, "命运岔室")),
        NodeKind::Shop => Some((0x2d020000, 45022710, "行商驿站")),
        _ => None,
    }
}
fn ready_flag(kind: NodeKind) -> u32 {
    match kind {
        NodeKind::Camp => 45000991,
        NodeKind::Chest | NodeKind::Event => 45010991,
        _ => 45020991,
    }
}
fn npcs(kind: NodeKind) -> Vec<(i32, [f32; 3])> {
    match kind {
        NodeKind::Camp => vec![(0, [-5.0, 0.15, 5.0]), (3451, [5.0, 0.05, 1.0]), (2180,[5.0,0.15,6.0])],
        NodeKind::Shop => vec![(3200, [0.0, 0.1, 0.0]),(2180,[-12.0,0.1,8.0])],
        NodeKind::Chest|NodeKind::Monster => vec![(2180,[-16.0,0.1,10.0])],
        NodeKind::Event => vec![(0,[0.0,0.1,0.0])],
        _ => vec![],
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn living_merchant_restores_hit_and_body_collisions_without_touching_map_collision() {
        let mut flags=eldenring::cs::ChrCtrlFlags(0);
        flags.set_disable_hit(true);flags.set_disable_player_collision(true);flags.set_disable_character_capsule_collision(true);flags.set_disable_map_collision(true);
        super::merchant_collision(&mut flags,false);
        assert!(!flags.disable_hit()&&!flags.disable_player_collision()&&!flags.disable_character_capsule_collision());
        assert!(flags.disable_map_collision(),"unrelated movement flags are preserved");
        super::merchant_collision(&mut flags,true);
        assert!(flags.disable_hit()&&flags.disable_player_collision()&&flags.disable_character_capsule_collision());
    }
    #[test]
    fn npc_spawning_requires_three_continuous_grounded_seconds() {
        let start=std::time::Instant::now();let mut since=None;
        assert!(!super::spawn_settled(&mut since,start,true));
        assert!(!super::spawn_settled(&mut since,start+std::time::Duration::from_secs(2),true));
        assert!(super::spawn_settled(&mut since,start+std::time::Duration::from_secs(3),true));
        assert!(!super::spawn_settled(&mut since,start+std::time::Duration::from_secs(4),false));
        assert!(!super::spawn_settled(&mut since,start+std::time::Duration::from_secs(5),true));
        assert!(super::spawn_settled(&mut since,start+std::time::Duration::from_secs(8),true));
    }
    use super::*;
    #[test]
    fn conditional_service_catalog_matches_native_menu_roles_and_models() {
        for (block,kind) in [(0x2d020000,NodeKind::Shop),(0x2d010000,NodeKind::Chest),(0x2d010000,NodeKind::Event),
            (0x2d020000,NodeKind::Monster),(0x2d010000,NodeKind::Monster),(0x2d000000,NodeKind::Monster),(0x2d000000,NodeKind::Camp),
            (0x0c010000,NodeKind::Monster),(0x0b000000,NodeKind::Monster),(0x0a000000,NodeKind::Monster)] {
            let entries=conditional_services(block,kind);let specs=npcs(kind);
            assert_eq!(entries.len(),specs.len());
            let mut ids=std::collections::HashSet::new();
            for (slot,&(id,model,param,role)) in entries.iter().enumerate() {
                assert!(ids.insert(id));assert_eq!(model,specs[slot].0);
                assert_eq!(role,service(false,kind,slot));
                assert_eq!(param,if kind==NodeKind::Event {super::super::event_hosts::HOSTS[0].npc}else{npc_data(model,role).0});
                assert!([45001,45011,45021].contains(&(super::super::event_hosts::visibility_flag(id)/1000)));
            }
        }
        assert!(conditional_services(0x0b0a0000,NodeKind::Shop).is_empty());
        let camp=conditional_services(0x2d000000,NodeKind::Camp);
        let battle=conditional_services(0x2d000000,NodeKind::Monster);
        assert!(camp.iter().all(|c|battle.iter().all(|b|c.0!=b.0)));
    }
    #[test]
    fn camp_bindings_require_matching_role_entity_and_live_character() {
        for (slot,model,param) in [(0,0,523200079),(1,3451,34510179),(2,2180,21800000)] {
            let id=45001800+slot as u32;
            assert!(camp_service_matches(slot,id,model,param,ChrLoadStatus::Active,1));
            assert!(!camp_service_matches(slot,id+1,model,param,ChrLoadStatus::Active,1));
            assert!(!camp_service_matches(slot,id,model,param+1,ChrLoadStatus::Active,1));
            assert!(!camp_service_matches(slot,id,model,param,ChrLoadStatus::Unloading,1));
            assert!(!camp_service_matches(slot,id,model,param,ChrLoadStatus::Active,0));
        }
        assert!(!camp_service_matches(3,45001803,0,523180079,ChrLoadStatus::Active,1));
    }
    #[test]
    fn map_service_binding_rejects_original_npcs_and_unloaded_or_wrong_roles() {
        assert!(map_service_matches(0,45001800,0,523200079,ChrLoadStatus::ReadyForActivation,1));
        assert!(!map_service_matches(0,45001800,2170,21700000,ChrLoadStatus::Active,1));
        assert!(map_service_matches(1,45001801,0,523180079,ChrLoadStatus::Active,1));
        assert!(!map_service_matches(0,11100705,2170,21700000,ChrLoadStatus::Active,1));
        assert!(!map_service_matches(0,45001801,2170,21700000,ChrLoadStatus::Active,1));
        assert!(!map_service_matches(1,45001801,0,523200079,ChrLoadStatus::Active,1));
        assert!(!map_service_matches(1,45001801,2170,523180079,ChrLoadStatus::Active,1));
        assert!(!map_service_matches(1,45001801,0,523180079,ChrLoadStatus::Unloading,1));
        assert!(!map_service_matches(1,45001801,0,523180079,ChrLoadStatus::Active,0));
        assert!(!map_service_matches(2,45001802,0,523180079,ChrLoadStatus::Active,1));
    }
    #[test]
    fn human_services_use_player_creation_without_affecting_other_models() {
        assert_eq!(player_model(0),1);
        for model in [2170,3200,3451] {assert_eq!(player_model(model),0);}
    }
    #[test]
    fn camp_level_up_has_its_own_unlimited_service() {
        assert_eq!(npcs(NodeKind::Camp).len(),3);
        assert_eq!((0..3).map(|i|service(false,NodeKind::Camp,i)).collect::<Vec<_>>(),vec![0,1,5]);
        assert_eq!(service(false,NodeKind::Shop,0),2);
        assert_eq!(service(true,NodeKind::Camp,1),4);
        assert_eq!(service(true,NodeKind::Camp,2),5);
        assert_eq!(service(false,NodeKind::Shop,1),5);
        assert_eq!(service(false,NodeKind::Chest,0),5);
        assert_eq!(service(false,NodeKind::Monster,0),5);
        assert_eq!(npc_data(3451,1),(34510179,34519000,-1));
        assert_eq!(npc_data(0,0),(523200079,523200000,23200));
        assert_eq!(npc_data(0,5),npc_data(0,4));
        assert!(!loaded(ChrLoadStatus::Initializing));
        assert!(!loaded(ChrLoadStatus::Unloading));
        assert!(loaded(ChrLoadStatus::ReadyForActivation));
        for kind in [NodeKind::Camp,NodeKind::Shop,NodeKind::Chest,NodeKind::Monster] {
            for (i,(model,_)) in npcs(kind).into_iter().enumerate() {
                if service(false,kind,i)==5 {assert_eq!(npc_data(model,5),(21800000,21800000,-1));}
            }
        }
        assert!(npcs(NodeKind::Monster)[0].1[0].abs()>=15.0);
    }
    #[test]
    fn resting_invalidates_cached_npcs_even_when_slot_count_is_unchanged() {
        let bindings=[(10u32,100usize),(11,200)];
        assert_eq!(first_stale(&bindings,|h|Some(if *h==10 {100}else{200})),None);
        assert_eq!(first_stale(&bindings,|_|None),Some(0));
        assert_eq!(first_stale(&bindings,|h|if *h==10 {Some(100)}else{None}),Some(1));
        assert_eq!(first_stale(&bindings,|h|Some(if *h==10 {999}else{200})),Some(0));
    }
    #[test]
    fn native_rooms_use_distinct_maps_and_flag_groups() {
        for kind in [NodeKind::Monster,NodeKind::Elite,NodeKind::Boss] {assert!(destination(kind).is_none(),"Combat must never enter the service-room branch");}
        assert_ne!(
            destination(NodeKind::Camp).unwrap().0,
            destination(NodeKind::Chest).unwrap().0
        );
        assert_ne!(ready_flag(NodeKind::Shop), ready_flag(NodeKind::Chest));
        assert_eq!(ready_flag(NodeKind::Camp), 45000991);
    }
}
