use eldenring::{cs::{BlockId, CSEventFlagMan, FieldInsHandle, TalkScript}, ez_state::EzStateValue};
use fromsoftware_shared::FromStatic;
use std::sync::atomic::{AtomicBool,Ordering};
pub static END_ALLOWED:AtomicBool=AtomicBool::new(false);
pub static END_REQUESTED:AtomicBool=AtomicBool::new(false);

pub struct NativeTalk {
    script: TalkScript,
    service: u32,
    state: u8,
    menu_kind: i32,
    pub choice: Option<i32>,
    flask_upgrade: Option<[(i32,i32,i32);2]>,
    camp: CampInteraction,
    event_menu: EventMenu,
    event_request: Option<EventRequest>,
    event_confirmation: Option<i32>,
    enemies: Vec<i32>,
    talismans: Vec<super::smith::TalismanOffer>,
    talisman: Option<super::smith::TalismanOffer>,
}
#[derive(Clone,Copy,Default)]
pub struct EventMenu {pub trial:bool,pub chariot:bool,pub stormveil:bool,pub descent:bool,pub recovery:bool,pub scarabs:bool,pub locked:bool,pub altar:bool,pub used:bool,pub busy:bool,pub ready:bool,pub attempts:u8}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum EventRequest {Trial(erdtree_tower::regional_events::Trial),Altar(usize),Scarabs,Recovery(usize),Descent,Stormveil(bool)}
impl EventMenu {
    fn confirmation(self,choice:i32)->Option<i32> {
        use erdtree_tower::regional_events::Trial;
        match self.request(choice)? {
            EventRequest::Trial(Trial::Descent)|EventRequest::Descent=>Some(9500118),
            EventRequest::Trial(Trial::Stormveil)=>Some(9500119),
            EventRequest::Trial(Trial::Chariot)=>Some(9500120),
            EventRequest::Trial(Trial::Skip)|EventRequest::Stormveil(false)=>Some(9500121),
            EventRequest::Stormveil(true)=>Some(if self.chariot{9500120}else{9500119}),
            _=>None,
        }
    }
    fn confirmed(self,choice:Option<i32>,answer:i32)->Option<EventRequest> {
        if !dialog_confirmed(answer){return None;}
        let choice=choice?;self.confirmation(choice)?;self.request(choice)
    }
    fn request(self,choice:i32)->Option<EventRequest> {
        if self.trial {
            return (!self.used&&!self.busy).then(||[erdtree_tower::regional_events::Trial::Descent,erdtree_tower::regional_events::Trial::Stormveil,erdtree_tower::regional_events::Trial::Chariot,erdtree_tower::regional_events::Trial::Skip].get(choice.wrapping_sub(1) as usize).copied()).flatten().map(EventRequest::Trial);
        }
        if self.stormveil {return (!self.used&&!self.busy&&(1..=2).contains(&choice)).then_some(EventRequest::Stormveil(choice==1));}
        if self.descent {return (choice==1&&!self.used&&!self.busy).then_some(EventRequest::Descent);}
        if self.recovery {return (!self.used&&!self.busy&&(1..=2).contains(&choice)).then(||EventRequest::Recovery((choice-1) as usize));}
        if self.scarabs {return (choice==1&&self.ready&&!self.busy&&self.attempts<3).then_some(EventRequest::Scarabs);}
        (self.altar&&!self.locked&&!self.used&&!self.busy&&(1..=3).contains(&choice)).then(||EventRequest::Altar((choice-1) as usize))
    }
}
#[derive(Default)]
struct CampInteraction { used: bool, pending: Option<i32> }
impl CampInteraction {
    fn request(&mut self, choice:i32)->bool {
        if self.used || self.pending.is_some() || !matches!(choice,1..=4) {return false;}
        self.pending=Some(choice);true
    }
}
// Owned and invoked only by the serialized game-task callback.
unsafe impl Send for NativeTalk {}
impl NativeTalk {
    pub fn new(block: i32, handle: FieldInsHandle, service: u32) -> Self {
        Self { script: TalkScript::new(BlockId::from(block), 950000000 + service as i32, handle), service, state: 0, menu_kind: 0, choice: None, flask_upgrade: None, camp: CampInteraction::default(),event_menu:Default::default(),event_request:None,event_confirmation:None,enemies:Vec::new(),talismans:Vec::new(),talisman:None }
    }
    fn event(&mut self, id: i32, args: &[i32]) -> Result<(), String> {
        let args: Vec<_> = args.iter().copied().map(EzStateValue::Int32).collect();
        self.script.event((id, args)).map_err(|e| e.to_string())
    }
    fn query(&mut self, id: i32, args: &[i32]) -> Result<i32, String> {
        let args: Vec<_> = args.iter().copied().map(EzStateValue::Int32).collect();
        self.script.env((id, args)).map(i32::from).map_err(|e| e.to_string())
    }
    pub fn dialog(&mut self, text: i32) -> Result<(), String> {
        self.event(17, &[7, text, 1, 0, 1])?;
        self.state = 4;
        Ok(())
    }
    pub fn active(&self) -> bool { self.state != 0 }
    pub fn suspend(&mut self)->Result<(),String> {
        if !self.active(){return Ok(());}
        for event in [67,18,35] {self.event(event,&[])?;}
        self.state=0;self.choice=None;self.event_request=None;self.event_confirmation=None;
        Ok(())
    }
    pub fn set_enemies(&mut self,enemies:&[i32]) {self.enemies=enemies.to_vec();}
    fn information(&mut self,texts:&[i32])->Result<(),String> {
        self.event(110,&[])?;self.event(20,&[])?;
        for (i,&text) in texts.iter().enumerate(){self.event(19,&[i as i32+1,text,-1])?;}
        self.event(19,&[99,9500003,-1])?;self.event(10,&[1])?;self.state=15;Ok(())
    }
    pub fn set_camp_used(&mut self, used:bool) {self.camp.used=used;}
    pub fn take_refill(&mut self)->Option<i32> {self.camp.pending.take()}
    pub fn set_event_menu(&mut self,menu:EventMenu){self.event_menu=menu;}
    pub fn take_event_request(&mut self)->Option<EventRequest>{self.event_request.take()}
    pub fn consume_key(&mut self)->Result<(),String>{self.event(52,&[3,8000,-1])}
    pub fn poll(&mut self) -> Result<(), String> {
        match self.state {
            0 => {
                if self.query(56, &[if self.service == 1 {9500001} else {6000}])? == 0 { return Ok(()); }
                super::log(format!("Native NPC interaction: service={}", self.service));
                if self.service==6 && self.event_menu.trial {
                    if self.event_menu.busy||self.event_menu.used {return Ok(());}
                    self.event(110,&[])?;self.event(20,&[])?;
                    for (i,text) in [9500094,9500115,9500117,9500116].into_iter().enumerate(){self.event(19,&[i as i32+1,text,-1])?;}
                    self.event(19,&[99,9500003,-1])?;self.event(10,&[1])?;self.state=1;return Ok(());
                }
                if self.service==6 && self.event_menu.stormveil {
                    if self.event_menu.busy||self.event_menu.used {return Ok(());}
                    self.event(110,&[])?;self.event(20,&[])?;
                    self.event(19,&[1,if self.event_menu.chariot{9500117}else{9500115},-1])?;self.event(19,&[2,9500116,-1])?;self.event(19,&[99,9500003,-1])?;
                    self.event(10,&[1])?;self.state=1;return Ok(());
                }
                if self.service==6 && self.event_menu.descent {
                    if self.event_menu.busy||self.event_menu.used {return Ok(());}
                    self.event(110,&[])?;self.event(20,&[])?;
                    self.event(19,&[1,9500094,-1])?;self.event(19,&[99,9500003,-1])?;
                    self.event(10,&[1])?;self.state=1;return Ok(());
                }
                if self.service==6 && self.event_menu.recovery {
                    if self.event_menu.used {return self.dialog(9500093);}
                    if self.event_menu.busy {return Ok(());}
                    self.event(110,&[])?;self.event(20,&[])?;
                    self.event(19,&[1,9500091,-1])?;self.event(19,&[2,9500092,-1])?;self.event(19,&[99,9500003,-1])?;
                    self.event(10,&[1])?;self.state=1;return Ok(());
                }
                if self.service==6 && self.event_menu.scarabs {
                    if self.event_menu.attempts>=3 {return self.dialog(9500083);}
                    if !self.event_menu.ready||self.event_menu.busy {return self.dialog(9500082);}
                    self.event(110,&[])?;self.event(20,&[])?;
                    self.event(19,&[1,9500080,-1])?;self.event(19,&[2,9500081,-1])?;self.event(19,&[99,9500003,-1])?;
                    self.event(10,&[1])?;self.state=1;return Ok(());
                }
                if self.service==6 && self.event_menu.locked {return self.dialog(9500073);}
                if self.service==6 && !self.event_menu.altar {return self.dialog(9500053);}
                if self.service==6 && self.event_menu.busy {return self.dialog(9500054);}
                if self.service==6 && self.event_menu.used {return self.dialog(9500055);}
                if self.service == 0 && self.camp.used { return self.dialog(9500011); }
                self.event(110, &[])?;
                self.event(20, &[])?;
                if self.service==6 {
                    self.event(19,&[1,9500050,-1])?;
                    self.event(19,&[2,9500051,-1])?;
                    self.event(19,&[3,9500052,-1])?;
                    self.event(19,&[99,9500003,-1])?;
                } else if self.service == 3 || self.service == 4 {
                    let messages: &[i32] = if self.service == 3 { &[9500020,9500021,9500104] } else { &[9500022,9500023,9500024,9500025,9500060,9500061,9500062,9500063,9500064,9500065] };
                    for (i,&message) in messages.iter().enumerate() { self.event(19,&[i as i32+1,message,-1])?; }
                    self.event(19,&[99,9500003,-1])?;
                } else if self.service==0 {
                    for (choice,message) in [(1,9500000),(2,9500044),(3,9500106),(4,9500107),(99,9500003)] {
                        self.event(19,&[choice,message,-1])?;
                    }
                } else if self.service==1 {
                    for (i,message) in [9500001,9500041,9500042].into_iter().enumerate() {
                        self.event(19,&[i as i32+1,message,-1])?;
                    }
                    self.event(19,&[99,9500003,-1])?;
                } else if self.service==5 {
                    for (i,message) in [9500040,9500042,9500043,9500045,9500046,9500071,9500100,9500101,9500108].into_iter().enumerate() {self.event(19,&[i as i32+1,message,-1])?;}
                    self.event(19,&[99,9500003,-1])?;
                } else if self.service==2 {
                    for (choice,message) in [(1,9500002),(2,9500111),(99,9500003)] {self.event(19,&[choice,message,-1])?;}
                } else {
                    self.event(19, &[1, if self.service==5 {9500040}else{9500000 + self.service as i32}, -1])?;
                    self.event(19, &[2, 9500003, -1])?;
                }
                self.event(10, &[1])?;
                self.state = 1;
            }
            1 => {
                if self.query(59, &[1, 0])? != 0 { return Ok(()); }
                let choice = self.query(23, &[])?;
                if self.service==6 {
                    self.state=0;
                    if self.event_menu.scarabs&&choice==2 {return self.dialog(9500085);}
                    if let Some(text)=self.event_menu.confirmation(choice) {
                        self.event(17,&confirmation_dialog(text))?;
                        self.event_confirmation=Some(choice);self.state=19;return Ok(());
                    }
                    self.event_request=self.event_menu.request(choice);
                    return Ok(());
                }
                if self.service == 3 || self.service == 4 {
                    self.choice=Some(choice); self.state=0; return Ok(());
                }
                if self.service==1 {
                    if smith_uses_allowance(choice)&&self.camp.used {return self.dialog(9500011);}
                    if smith_uses_allowance(choice) {
                        let player=unsafe {eldenring::cs::PlayerIns::local_player()}.map_err(|_|"角色未加载")?;
                        let data=unsafe {player.player_game_data.as_ref()};
                        let quantity=super::smith::remaining(data);
                        super::log(format!("Smith menu: choice={choice}, voucher={quantity}, equipped={:?}",data.equipment.chr_asm.equipment_param_ids));
                        if quantity==0 {return self.dialog(9500012);}
                    }
                    if choice==2 {
                        let player=unsafe {eldenring::cs::PlayerIns::local_player()}.map_err(|_|"角色未加载")?;
                        self.talismans=super::smith::talisman_offers(unsafe {player.player_game_data.as_ref()})?;
                        if self.talismans.is_empty(){return self.dialog(9500112);}
                        self.event(110,&[])?;self.event(20,&[])?;
                        for i in 0..self.talismans.len(){self.event(19,&[i as i32+1,self.talismans[i].text,-1])?;}
                        self.event(19,&[999,9500003,-1])?;self.event(10,&[1])?;self.state=17;return Ok(());
                    }
                    if let Some((event,args,menu))=smith_menu(choice) {
                        if choice==1 {super::smith::configure_menu()?;}
                        self.event(event,&args)?;self.menu_kind=menu;self.state=7;
                    } else {self.state=0;}
                    return Ok(());
                }
                if self.service==5&&choice!=1 {
                    if choice==9 {
                        if !END_ALLOWED.load(Ordering::Relaxed){return self.dialog(9500110);}
                        self.event(17,&confirmation_dialog(9500109))?;self.state=16;return Ok(());
                    }
                    if choice==7 {
                        let player=unsafe {eldenring::cs::PlayerIns::local_player()}.map_err(|_|"角色未加载")?;
                        let data=unsafe {player.player_game_data.as_ref()};
                        return self.information(&[9501000+i32::from(data.max_hp_flask),9501300+i32::from(data.max_fp_flask)]);
                    }
                    if choice==8 {return self.information(&if self.enemies.is_empty(){vec![9500102]}else{self.enemies.clone()});}
                    if choice==5 {
                        if self.query(47,&[3,erdtree_tower::elite::TEAR as i32,2,0,0])?==0{return self.dialog(9500048);}
                        let player=unsafe {eldenring::cs::PlayerIns::local_player()}.map_err(|_|"角色未加载")?;
                        let Ok(plan)=super::flasks::restoration_upgrade(unsafe {player.player_game_data.as_ref()}) else {return self.dialog(9500070);};
                        self.flask_upgrade=Some(plan);
                        self.event(17,&confirmation_dialog(9500047))?;self.state=13;return Ok(());
                    }
                    if choice==4 {
                        if self.query(47,&[3,super::rebirth::TEAR,2,0,0])?==0 {return self.dialog(22001002);}
                        self.event(17,&confirmation_dialog(22001000))?;
                        self.state=9;return Ok(());
                    }
                    if let Some((event,args,menu))=attendant_menu(choice) {self.event(event,&args)?;self.menu_kind=menu;self.state=7;} else {self.state=0;}
                    return Ok(());
                }
                if self.service==2&&choice==2 {
                    self.event(46,&[-1,-1])?;self.menu_kind=6;self.state=7;return Ok(());
                }
                if choice != 1 && !(self.service == 0 && matches!(choice,2..=4)) { self.state = 0; return Ok(()); }
                if self.service == 5 {
                    let player=unsafe {eldenring::cs::PlayerIns::local_player()}.map_err(|_|"角色未加载")?;
                    super::level_cost::configure(unsafe {player.player_game_data.as_ref()}.archetype)?;
                    self.event(31,&[])?;
                    self.state=5;
                } else if self.service == 2 {
                    let player=unsafe {eldenring::cs::PlayerIns::local_player()}.map_err(|_|"角色未加载")?;
                    super::native_shop::refresh_weapon_levels(unsafe {player.player_game_data.as_ref()})?;
                    self.event(22, &[super::native_shop::GOODS as i32, (super::native_shop::GOODS+super::native_shop::STOCK_COUNT-1) as i32])?;
                    self.state = 3;
                } else if self.service == 0 {
                    if !self.camp.request(choice) {return self.dialog(9500011);}
                    self.state = 2;
                }
            }
            19 => if self.query(58,&[0])?==0 {
                let choice=self.event_confirmation.take();
                let answer=self.query(22,&[])?;
                self.event_request=self.event_menu.confirmed(choice,answer);self.state=0;
            },
            2 => {},
            3 => if self.query(59, &[5, 0])? == 0 { self.state = 0; },
            4 => if self.query(58, &[0])? == 0 { self.state = 0; },
            15 => if self.query(59,&[1,0])?==0 {self.state=0;},
            16 => if self.query(58,&[0])?==0 {
                if dialog_confirmed(self.query(22,&[])?)&&END_ALLOWED.load(Ordering::Relaxed){END_REQUESTED.store(true,Ordering::Relaxed);}
                self.state=0;
            },
            17 => if self.query(59,&[1,0])?==0 {
                let choice=self.query(23,&[])?;
                self.talisman=usize::try_from(choice-1).ok().and_then(|i|self.talismans.get(i)).copied();
                if self.talisman.is_some(){self.event(17,&confirmation_dialog(9500113))?;self.state=18;}else{self.state=0;}
            },
            18 => if self.query(58,&[0])?==0 {
                let offer=self.talisman.take();self.state=0;
                if !dialog_confirmed(self.query(22,&[])?) {return Ok(());}
                if self.camp.used {return self.dialog(9500011);}
                let player=unsafe {eldenring::cs::PlayerIns::local_player_mut()}.map_err(|_|"角色未加载")?;
                if player.chr_ins.modules.data.hp<=0{return Ok(());}
                super::smith::upgrade_talisman(unsafe {player.player_game_data.as_mut()},offer.ok_or("护符选择已失效")?)?;
                self.dialog(9500114)?;
            },
            5 => self.state=6,
            6 => if self.query(25,&[23])? == 0 {self.state=0;},
            7 => self.state=8,
            8 => if self.query(59,&[self.menu_kind,0])?==0 && self.query(58,&[0])?==0 {self.state=0;},
            9 => if self.query(58,&[0])?==0 {
                if !dialog_confirmed(self.query(22,&[])?) {self.state=0;return Ok(());}
                if self.query(47,&[3,super::rebirth::TEAR,2,0,0])?==0 {return self.dialog(22001002);}
                self.event(113,&[])?;
                self.event(35,&[])?;
                self.state=10;
            },
            10 => self.state=11,
            11 => if self.query(59,&[19,0])?==0&&self.query(58,&[0])?==0 {
                if self.query(28,&[2])?==1 {self.state=12;}else{return self.dialog(22001001);}
            },
            12 => {
                self.event(52,&[3,super::rebirth::TEAR,-1])?;
                super::log("Rebirth completed: consumed one larval tear");
                self.state=0;
            },
            13 => if self.query(58,&[0])?==0 {
                if !dialog_confirmed(self.query(22,&[])?) {self.flask_upgrade=None;self.state=0;return Ok(());}
                if self.query(47,&[3,erdtree_tower::elite::TEAR as i32,2,0,0])?==0 {self.flask_upgrade=None;return self.dialog(9500048);}
                let player=unsafe {eldenring::cs::PlayerIns::local_player()}.map_err(|_|"角色未加载")?;
                let Ok(plan)=super::flasks::restoration_upgrade(unsafe {player.player_game_data.as_ref()}) else {self.flask_upgrade=None;return self.dialog(9500070);};
                self.flask_upgrade=Some(plan);
                self.state=14;
            },
            14 => {
                let plan=self.flask_upgrade.ok_or("圣杯瓶强化事务丢失")?;
                let player=unsafe {eldenring::cs::PlayerIns::local_player_mut()}.map_err(|_|"角色未加载")?;
                super::flasks::apply_restoration_upgrade(unsafe {player.player_game_data.as_mut()},plan)?;
                self.event(52,&[3,erdtree_tower::elite::TEAR as i32,-1])?;
                self.flask_upgrade=None;self.dialog(9500049)?;
            },
            _ => unreachable!(),
        }
        Ok(())
    }
}
fn smith_uses_allowance(choice:i32)->bool {matches!(choice,1|2)}
fn dialog_confirmed(result:i32)->bool {result==1}
// ER uses GR_Dialogues text IDs for the left/right labels: 1=OK, 2=Cancel.
fn confirmation_dialog(text:i32)->[i32;5] {[8,text,1,2,2]}
fn smith_menu(choice:i32)->Option<(i32,Vec<i32>,i32)> {
    match choice {
        1=>Some((24,vec![0],9)),
        3=>Some((48,vec![],7)),
        _=>None,
    }
}
fn attendant_menu(choice:i32)->Option<(i32,Vec<i32>,i32)> {
    match choice {2=>Some((48,vec![],7)),3=>Some((28,vec![-1,-1],11)),6=>Some((130,vec![],21)),_=>None}
}
pub fn flag(id: u32) -> Result<bool, String> {
    Ok(unsafe { CSEventFlagMan::instance() }.map_err(|_| "原生事件系统未加载")?.virtual_memory_flag.get_flag(id))
}
#[cfg(test)]
mod tests {
    #[test]
    fn trial_confirmations_require_acceptance_and_recheck_availability() {
        let menu=super::EventMenu{trial:true,..Default::default()};
        for choice in 1..=4 {
            assert_eq!(menu.confirmation(choice),Some(9500117+choice));
            assert_eq!(menu.confirmed(Some(choice),1),menu.request(choice));
            for answer in [-1,0,2,3]{assert_eq!(menu.confirmed(Some(choice),answer),None);}
            assert_eq!(super::EventMenu{busy:true,..menu}.confirmed(Some(choice),1),None);
            assert_eq!(super::EventMenu{used:true,..menu}.confirmed(Some(choice),1),None);
        }
        assert_eq!(menu.confirmed(None,1),None);
        assert_eq!(menu.confirmation(99),None);
        assert_eq!(super::EventMenu{descent:true,..Default::default()}.confirmation(1),Some(9500118));
        assert_eq!(super::EventMenu{stormveil:true,chariot:true,..Default::default()}.confirmation(1),Some(9500120));
    }
    #[test]
    fn event_dialog_requests_are_local_and_respect_current_progress() {
        use super::{EventMenu,EventRequest};
        let trial=EventMenu{trial:true,..Default::default()};
        use erdtree_tower::regional_events::Trial;
        for (i,choice) in [Trial::Descent,Trial::Stormveil,Trial::Chariot,Trial::Skip].into_iter().enumerate() {
            assert_eq!(trial.request(i as i32+1),Some(EventRequest::Trial(choice)));
            assert_eq!(EventMenu{busy:true,..trial}.request(i as i32+1),None);
        }
        for i in [-1,0,5,99] {assert_eq!(trial.request(i),None);}
        let altar=EventMenu{altar:true,..Default::default()};
        let storm=EventMenu{stormveil:true,..Default::default()};
        assert_eq!(storm.request(1),Some(EventRequest::Stormveil(true)));
        assert_eq!(storm.request(2),Some(EventRequest::Stormveil(false)));
        assert_eq!(storm.request(99),None);
        assert_eq!(EventMenu{used:true,..storm}.request(1),None);
        assert_eq!(EventMenu{busy:true,..storm}.request(2),None);
        let descent=EventMenu{descent:true,..Default::default()};
        assert_eq!(descent.request(1),Some(EventRequest::Descent));
        for choice in [0,2,99] {assert_eq!(descent.request(choice),None);}
        assert_eq!(EventMenu{busy:true,..descent}.request(1),None);
        assert_eq!(EventMenu{used:true,..descent}.request(1),None);
        let spring=EventMenu{recovery:true,..Default::default()};
        assert_eq!(spring.request(1),Some(EventRequest::Recovery(0)));
        assert_eq!(spring.request(2),Some(EventRequest::Recovery(1)));
        assert_eq!(spring.request(3),None);assert_eq!(spring.request(99),None);
        assert_eq!(EventMenu{used:true,..spring}.request(1),None);
        assert_eq!(EventMenu{busy:true,..spring}.request(2),None);
        assert_eq!(altar.request(1),Some(EventRequest::Altar(0)));
        assert_eq!(altar.request(3),Some(EventRequest::Altar(2)));
        assert_eq!(altar.request(99),None);
        assert_eq!(EventMenu{used:true,..altar}.request(1),None);
        assert_eq!(EventMenu{busy:true,..altar}.request(1),None);
        let scarabs=EventMenu{scarabs:true,ready:true,..Default::default()};
        assert_eq!(scarabs.request(1),Some(EventRequest::Scarabs));
        assert_eq!(EventMenu{attempts:3,..scarabs}.request(1),None);
        assert_eq!(EventMenu{ready:false,..scarabs}.request(1),None);
        assert_eq!(EventMenu{busy:true,..scarabs}.request(1),None);
    }
    #[test]
    fn camp_requests_and_allowances_are_personal() {
        let mut host=super::CampInteraction::default();
        let mut guest=super::CampInteraction::default();
        assert!(host.request(1));
        assert!(!host.request(2));
        assert_eq!(guest.pending,None);
        assert_eq!(host.pending.take(),Some(1));
        host.used=true;
        assert!(!host.request(2));
        assert!(guest.request(2));
        assert_eq!(guest.pending.take(),Some(2));
        assert!(!guest.request(99));
        assert!(guest.request(1));
    }
    #[test]
    fn only_left_confirm_button_accepts_consumable_services() {
        for text in [9500047,22001000,9500113,9500109] {
            assert_eq!(super::confirmation_dialog(text),[8,text,1,2,2]);
        }
        for result in [-1,0,2,3] {assert!(!super::dialog_confirmed(result));}
        assert!(super::dialog_confirmed(1));
    }
    #[test]
    fn smith_services_remain_free_after_upgrade_and_use_verified_native_menus() {
        assert_eq!(super::smith_menu(1),Some((24,vec![0],9)));
        assert_eq!(super::smith_menu(2),None);
        assert_eq!(super::smith_menu(3),Some((48,vec![],7)));
        assert_eq!(super::smith_menu(4),None);
        assert_eq!(super::attendant_menu(2),Some((48,vec![],7)));
        assert_eq!(super::attendant_menu(3),Some((28,vec![-1,-1],11)));
        assert_eq!(super::attendant_menu(6),Some((130,vec![],21)));
        assert!(super::smith_uses_allowance(1));assert!(super::smith_uses_allowance(2));
        assert!(!super::smith_uses_allowance(3));assert!(!super::smith_uses_allowance(4));
    }
}
pub fn set_flag(id: u32, enabled: bool) -> Result<(), String> {
    unsafe { CSEventFlagMan::instance_mut() }.map_err(|_| "原生事件系统未加载")?.virtual_memory_flag.set_flag(id, enabled);
    Ok(())
}
