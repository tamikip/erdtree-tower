use eldenring::cs::{
    CSEventFlagMan, ChrDebugSpawnRequest, ChrLoadStatus, FieldInsHandle, GameMan, NpcParam, NpcThinkParam,
    SoloParamRepository, WorldChrMan,
};
use erdtree_tower::NodeKind;
use fromsoftware_shared::{F32Vector4, FromStatic};
use std::time::{Duration, Instant};

fn first_stale<T>(bindings:&[(T,usize)],mut lookup:impl FnMut(&T)->Option<usize>)->Option<usize> {
    bindings.iter().position(|(handle,address)|lookup(handle)!=Some(*address))
}
fn service(hub:bool,kind:NodeKind,index:usize)->u32 {
    if kind==NodeKind::Event {return 6;}
    if hub {if index==2 {5}else{3+index as u32}} else if kind==NodeKind::Shop {if index==0 {2}else{5}} else if kind!=NodeKind::Camp||index==2 {5} else {index as u32}
}
fn npc_data(model:i32,role:u32)->(i32,i32,i32) {
    match model {2180=>(21801034,21800000,-1),3451=>(34510179,34519000,-1),0 if role==4||role==5=>(523180079,523180000,23180),0=>(523200079,523200000,23200),_=>(model*10000,model*10000,-1)}
}
fn loaded(status:ChrLoadStatus)->bool {matches!(status,ChrLoadStatus::Active|ChrLoadStatus::ReadyForActivation)}
fn map_service_matches(slot:usize,entity:u32,model:i32,param:i32,status:ChrLoadStatus,hp:i32)->bool {
    let expected=match slot {0=>(0,523200079),1=>(0,523180079),_=>return false};
    entity==45001800+slot as u32&&(model,param)==expected&&loaded(status)&&hp>0
}
fn player_model(model:i32)->u8 {u8::from(model==0)}
fn conditional_services(block:i32,kind:NodeKind)->Vec<(u32,i32,i32,u32)> {
    if block==0x2d000000 && kind==NodeKind::Camp {
        return vec![(45001800,0,523200079,0),(45001801,3451,34510179,1),(45001802,2180,21801034,5)];
    }
    include_str!("../../scene-assets/party-services.txt").lines().filter_map(|line| {
        let p:Vec<_>=line.split('|').collect();
        let map:Vec<u8>=p[0][1..].split('_').map(|v|v.parse().unwrap()).collect();
        let matches=match p[1]{"Shop"=>kind==NodeKind::Shop,"Chest"=>kind==NodeKind::Chest,"Event"=>kind==NodeKind::Event,"Monster"=>kind==NodeKind::Monster,_=>false};
        (matches&&i32::from_be_bytes([map[0],map[1],map[2],map[3]])==block).then(|| {
            let role=p[7].parse().unwrap();
            (p[2].parse().unwrap(),if role==5{2180}else{p[3].parse().unwrap()},if role==5{21801034}else{p[4].parse().unwrap()},role)
        })
    }).collect()
}
fn camp_service_matches(slot:usize,entity:u32,model:i32,param:i32,status:ChrLoadStatus,hp:i32)->bool {
    let expected=match slot {0=>(0,523200079),1=>(3451,34510179),2=>(2180,21801034),_=>return false};
    entity==45001800+slot as u32&&(model,param)==expected&&loaded(status)&&hp>0
}

#[derive(Clone)]
pub struct RoomView {
    pub message: String,
    pub loading: bool,
    pub completed: bool,
}

pub struct Room {
    hub: bool,
    pub node: usize,
    pub kind: NodeKind,
    pub block: i32,
    pub entry: u32,
    started: Instant,
    pending_since: Option<Instant>,
    handles: Vec<(FieldInsHandle, usize)>,
    talks: Vec<super::native_talk::NativeTalk>,
    saved_hp: i32,
    saved_fp: i32,
    restored: bool,
    pub allowance_issued: bool,
    allowance:super::smith::Grant,
    pub view: RoomView,
    pub scarabs:super::scarabs::Hunt,
    pub chest_attempt:Option<Instant>,
}
impl Room {
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
    pub fn event_menu(&mut self,run:&erdtree_tower::Run,party_ready:bool,party_busy:bool) {
        let progress=&run.regional_event;
        let menu=super::native_talk::EventMenu{trial:progress.trial,chariot:progress.chariot,stormveil:progress.stormveil.is_some(),descent:progress.descent,recovery:progress.recovery,scarabs:progress.scarabs,locked:progress.locked,
            altar:erdtree_tower::regional_events::kind(run.seed,self.node)==1,used:progress.opened!=0||progress.stormveil.is_some_and(|s|s!=erdtree_tower::stormveil::Progress::Choosing),
            busy:progress.guards||self.scarabs.active()||party_busy,ready:self.scarabs.ready()||party_ready,attempts:progress.attempts};
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
    pub fn attendant(block:i32)->Result<Self,String> {
        let arena=super::arenas::for_block(block).ok_or("未知竞技场")?;
        Self::create(0,NodeKind::Monster,0,0,Some(arena),false)
    }
    fn create(node:usize,kind:NodeKind,hp:i32,fp:i32,attendant:Option<super::arenas::Arena>,hub:bool)->Result<Self,String> {
        let (block, entry, title) = if hub {(super::hub::BLOCK,11102691,"大赐福")} else if let Some(arena)=attendant {(arena.block,arena.entry,"场边引导者")} else {destination(kind).ok_or("此节点没有房间")?};
        let params = unsafe { SoloParamRepository::instance() }.map_err(|_| "NPC 参数未就绪")?;
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
            scarabs:Default::default(),
            chest_attempt:None,
            hub,
            node,
            kind,
            block,
            entry,
            started: Instant::now(),
            pending_since: None,
            handles: Vec::new(),
            talks: Vec::new(),
            saved_hp: hp,
            saved_fp: fp,
            restored: kind==NodeKind::Monster,
            allowance_issued: false,
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
        if !arrived || (!self.restored && !grounded) {
            self.view.loading = true;
            if self.started.elapsed() > Duration::from_secs(60) {
                return Err("服务房间传送超时，未消耗节点".into());
            }
            return Ok(());
        }
        let flags = unsafe { CSEventFlagMan::instance() }.map_err(|_| "场景事件系统未就绪")?;
        if !self.hub && self.kind!=NodeKind::Monster && !flags.virtual_memory_flag.get_flag(ready_flag(self.kind)) {
            self.view.message = "正在等待房间地图和交互物件就绪……".into();
            if self.started.elapsed() > Duration::from_secs(60) {
                return Err("服务房间事件未加载，请使用包含 scene-assets 的新启动包".into());
            }
            return Ok(());
        }
        if !self.restored {
            let player = world.main_player.as_mut().unwrap();
            super::ailments::clear(&mut player.chr_ins)?;
            if self.hub {super::smith::clear(unsafe {player.player_game_data.as_mut()});}
            player.chr_ins.modules.data.hp = player.chr_ins.modules.data.hp.min(self.saved_hp);
            player.chr_ins.modules.data.fp = player.chr_ins.modules.data.fp.min(self.saved_fp);
            self.restored = true;
            super::log(format!(
                "Native room {} arrived; event handshake received",
                self.node
            ));
        }
        let specs = if self.hub {vec![(0,[-329.5,-21.9,-309.0]),(0,[-329.5,-21.9,-302.0])]} else {npcs(self.kind)};
        if let Some(index)=first_stale(&self.handles,|handle|world.chr_ins_by_handle(handle)
            .filter(|c|!c.debug_flags.character_disabled()&&c.modules.data.hp>0&&!c.chr_flags1c5.death_flag())
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
        let conditional=conditional_services(self.block,self.kind);
        for &(id,_,_,_) in &conditional {
            super::native_talk::set_flag(id+70,true)?;
            if !super::native_talk::flag(id+70)? {return Err("服务 NPC 显示标记不可用".into());}
        }
        if self.map_backed() && self.handles.len()<specs.len() {
            let slot=self.handles.len();
            let matches=|slot,entity,model,param,status,hp| {
                if self.hub {map_service_matches(slot,entity,model,param,status,hp)}
                else if self.kind==NodeKind::Camp {camp_service_matches(slot,entity,model,param,status,hp)}
                else {conditional.get(slot).is_some_and(|&(id,m,p,_)|(entity,model,param)==(id,m,p))&&loaded(status)&&hp>0}
            };
            let found=world.chr_sets.iter().flatten().flat_map(|set|set.characters()).find(|chr|
                matches(slot,chr.event_entity_id,chr.npc_id,chr.npc_param_id,
                    unsafe {chr.chr_set_entry.as_ref()}.chr_load_status,chr.modules.data.hp)
                &&!chr.debug_flags.character_disabled()&&!chr.chr_flags1c5.death_flag())
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
        for &(handle, address) in &self.handles {
            let chr = world
                .chr_ins_by_handle_mut(&handle)
                .filter(|c| std::ptr::from_ref(*c) as usize == address)
                .ok_or("服务 NPC 已卸载")?;
            chr.team_type = 0;
            chr.chr_flags1c5.set_is_invincible(true);
            chr.debug_flags.set_disabled_hit(true);
            chr.debug_flags.set_disabled_movement(true);
            chr.debug_flags.set_disabled_secondary_actions(true);
            chr.chr_ctrl.flags.set_disable_player_collision(true);
            chr.chr_ctrl.flags.set_disable_character_capsule_collision(true);
        }
        self.view.loading = self.handles.len() < specs.len();
        if self.view.loading {
            return Ok(());
        }
        if !self.hub && self.kind==NodeKind::Camp && !self.allowance_issued {
            let data=unsafe {world.main_player.as_mut().unwrap().player_game_data.as_mut()};
            match self.allowance.poll(self.started.elapsed(),super::smith::remaining(data)) {
                super::smith::GrantStep::Request=>{
                    self.view.loading=true;
                    self.view.message="正在确认本次营地的强化机会……".into();
                    super::smith::prepare(data)?;
                    return Ok(());
                },
                super::smith::GrantStep::Waiting=>{self.view.loading=true;return Ok(());},
                super::smith::GrantStep::Ready=>{
                    self.allowance_issued=true;
                    super::log("Camp allowance confirmed; automatic issuance disabled for this room");
                },
                super::smith::GrantStep::Failed=>{
                    self.view.message="强化凭证尚未确认到账，已停止自动发放；补瓶、加点和免费菜单仍可使用。".into();
                },
            }
        }
        for talk in &mut self.talks {talk.set_camp_used(self.view.completed);}
        if let Some(index) = self.talks.iter().position(|talk| talk.active()) {
            self.talks[index].poll()?;
        } else {
            for talk in &mut self.talks {
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
        self.scarabs.retire(world);
        if self.kind!=NodeKind::Monster {if let Some(player)=world.main_player.as_mut(){super::smith::clear(unsafe {player.player_game_data.as_mut()});}}
        if !self.hub && self.kind!=NodeKind::Monster && let Ok(flags) = unsafe { CSEventFlagMan::instance_mut() } {
            flags.virtual_memory_flag.set_flag(self.base() + 993, false);
            if self.kind==NodeKind::Chest { flags.virtual_memory_flag.set_flag(45010939,false); }
            if self.kind==NodeKind::Event { flags.virtual_memory_flag.set_flag(45010940,false); }
        }
        for (id,_,_,_) in conditional_services(self.block,self.kind) {let _=super::native_talk::set_flag(id+70,false);}
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
        NodeKind::Event => vec![(2170,[-7.0,0.1,2.0])],
        _ => vec![],
    }
}
#[cfg(test)]
mod tests {
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
                assert_eq!(role,service(false,kind,slot));assert_eq!(param,npc_data(model,role).0);
                assert!([45001,45011,45021].contains(&((id+70)/1000)));
            }
        }
        assert!(conditional_services(0x0b0a0000,NodeKind::Shop).is_empty());
        let camp=conditional_services(0x2d000000,NodeKind::Camp);
        let battle=conditional_services(0x2d000000,NodeKind::Monster);
        assert!(camp.iter().all(|c|battle.iter().all(|b|c.0!=b.0)));
    }
    #[test]
    fn camp_bindings_require_matching_role_entity_and_live_character() {
        for (slot,model,param) in [(0,0,523200079),(1,3451,34510179),(2,2180,21801034)] {
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
                if service(false,kind,i)==5 {assert_eq!(npc_data(model,5),(21801034,21800000,-1));}
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
