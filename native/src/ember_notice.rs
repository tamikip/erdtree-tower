use erdtree_tower::embers::Ember;
static PREVIEW:std::sync::Mutex<Option<Ember>>=std::sync::Mutex::new(None);
pub fn preview(ember:Ember) {if let Ok(mut request)=PREVIEW.lock(){*request=Some(ember);}}

#[derive(Default)]
pub struct Notice { pending:Option<((u64,u8),Ember)>, last_error:Option<String> }

impl Notice {
    pub fn queue(&mut self,seed:u64,act:u8,ember:Ember) {self.pending=Some(((seed,act),ember));}
    pub fn tick(&mut self,scope:Option<(u64,u8)>,ready:bool) {
        if ready&&self.pending.is_none()&&let Some((seed,act))=scope&&let Some(ember)=PREVIEW.lock().ok().and_then(|mut r|r.take()) {self.queue(seed,act,ember);}
        let mut error=None;
        self.deliver(scope,ready,|ember|match popup(ember){Ok(shown)=>shown,Err(e)=>{error=Some(e);false}});
        if error!=self.last_error {if let Some(e)=&error {super::log(format!("CHEST_EMBER native pickup waiting: {e}"));}self.last_error=error;}
    }
    fn deliver(&mut self,scope:Option<(u64,u8)>,ready:bool,mut show:impl FnMut(Ember)->bool) {
        let Some((owner,ember))=self.pending else{return;};
        if scope!=Some(owner) {self.pending=None;return;}
        if ready&&show(ember) {self.pending=None;}
    }
}

pub const GOODS_BASE:u32=9900000;
const ICON_BASE:u16=65000;

#[repr(C)]
struct Acquisition {item:u32,quantity:i32,gem:i32,storage:u8,new_item:u8,silent:u8,padding:u8}
impl Acquisition {
    fn ember(ember:Ember)->Self {Self {item:0x40000000|GOODS_BASE+ember.index() as u32,quantity:1,gem:-1,storage:0,new_item:1,silent:0,padding:0}}
}

fn popup(ember:Ember)->Result<bool,String> {
    use eldenring::cs::{MapItemMan,SoloParamRepository,EquipParamGoods,CSMenuManImp};
    use fromsoftware_shared::FromStatic;
    // The verified game's acquisition queue is separate from inventory insertion.
    let append=super::items::checked_function(0x5653f0,&[0x40,0x57,0x48,0x83,0xec,0x30,0x48,0xc7,0x44,0x24,0x20,0xfe,0xff,0xff,0xff])?;
    super::items::checked_function(0x564b4e,&[0x49,0x8d,0xbd,0xa0,0,0,0,0x4c,0x8b,0xf7])?;
    unsafe {CSMenuManImp::instance()}.map_err(|_|"原生菜单尚未就绪")?;
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"物品参数尚未就绪")?;
    let row=params.get::<EquipParamGoods>(GOODS_BASE+ember.index() as u32).ok_or("缺少余烬原生显示资源")?;
    if row.icon_id()!=ICON_BASE+ember.icon_index() as u16 {return Err("余烬原生图标参数不匹配".into());}
    let manager=unsafe {MapItemMan::instance_mut()}.map_err(|_|"物品显示队列尚未就绪")? as *mut MapItemMan as *mut u8;
    let count=unsafe {manager.add(0x148).cast::<u64>().read()};
    if count>10 {return Err("物品显示队列布局不匹配".into());}
    if count==10 {return Ok(false);}
    let record=Acquisition::ember(ember);
    let append:unsafe extern "C" fn(*mut u8,*const Acquisition)=unsafe {std::mem::transmute(append)};
    unsafe {append(manager.add(0xa0),&record);}
    super::log(format!("CHEST_EMBER native acquisition queued: {ember:?} item={} inventory_grant=false",record.item));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn records_match_verified_native_layout_and_use_distinct_display_goods() {
        assert_eq!(std::mem::size_of::<Acquisition>(),16);
        assert_eq!(std::mem::offset_of!(Acquisition,new_item),13);
        let mut ids=std::collections::HashSet::new();
        for ember in Ember::ALL {let r=Acquisition::ember(ember);assert!(ids.insert(r.item));assert_eq!(r.item>>28,4);assert_eq!(r.quantity,1);assert_eq!(r.gem,-1);assert_eq!((r.storage,r.new_item,r.silent),(0,1,0));}
    }
    #[test]
    fn waits_for_menu_and_delivers_once_without_granting_again() {
        let mut notice=Notice::default();notice.queue(7,1,Ember::Chain);
        let scope=Some((7,1));
        notice.deliver(scope,false,|_|panic!("loading must not open a popup"));
        notice.deliver(scope,true,|_|false);
        assert!(notice.pending.is_some());
        let mut calls=0;
        for _ in 0..3 {notice.deliver(scope,true,|ember|{assert_eq!(ember,Ember::Chain);calls+=1;true});}
        assert_eq!(calls,1);
    }
    #[test]
    fn stale_reward_does_not_show_in_another_run() {
        for scope in [None,Some((8,1)),Some((7,2))] {
            let mut notice=Notice::default();notice.queue(7,1,Ember::Chain);
            notice.deliver(scope,true,|_|panic!("stale notice"));
            assert!(notice.pending.is_none());
        }
    }
}
