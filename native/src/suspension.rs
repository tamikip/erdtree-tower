use std::{path::Path,time::Instant,sync::{Mutex,atomic::{AtomicBool,AtomicU8,Ordering}}};
use erdtree_tower::{Run,suspension::{Point,Snapshot}};
pub static REQUEST:AtomicBool=AtomicBool::new(false);
pub static ROLE:AtomicU8=AtomicU8::new(0);
static NOTICE:Mutex<String>=Mutex::new(String::new());
pub fn notice(text:impl Into<String>){if let Ok(mut s)=NOTICE.lock(){*s=text.into();}}
#[derive(Default)]
pub struct LinkState {
    pub queued:bool,pub ticket:Option<u64>,pub saved:bool,pub committed:bool,pub started:Option<Instant>,
    pub leaving:bool,pub leave_ready:bool,pub departed:Vec<u64>,
    pub resume:Option<Point>,pub resume_ready:bool,
}
#[derive(Default)]
pub struct Flow {pub requested:bool,pub prepared:Option<Run>,pub returning:bool,pub leave:bool,pub committed:Option<Instant>}
fn atomic_write(path:&Path,text:&str)->Result<(),String> {
    use std::io::Write;
    let tmp=path.with_extension("pause-tmp");
    let mut file=std::fs::File::create(&tmp).map_err(|e|e.to_string())?;
    file.write_all(text.as_bytes()).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);
    std::fs::rename(tmp,path).map_err(|e|e.to_string())
}
pub fn save(owner:(u32,i32),run:&Run)->Result<(),String> {
    let path=super::hub::path(owner,"paused");
    save_at(&path,owner,run)
}
fn save_at(path:&Path,owner:(u32,i32),run:&Run)->Result<(),String> {
    let text=Snapshot::encode(owner,run)?;
    if path.exists(){
        let old=std::fs::read_to_string(path).map_err(|e|e.to_string())?;
        if !old.trim().is_empty(){Snapshot::decode(&old,owner)?;std::fs::copy(path,path.with_extension("paused-backup")).map_err(|e|e.to_string())?;}
    }
    atomic_write(&path,&text)?;
    Snapshot::decode(&std::fs::read_to_string(path).map_err(|e|e.to_string())?,owner)?;Ok(())
}
pub fn load(owner:(u32,i32))->Result<Option<Run>,String> {
    let path=super::hub::path(owner,"paused");
    if !path.exists(){return Ok(None);}
    let text=std::fs::read_to_string(&path).map_err(|e|e.to_string())?;
    if text.trim().is_empty(){return Ok(None);}
    Snapshot::decode(&text,owner).map(Some)
}
pub fn clear(owner:(u32,i32))->Result<(),String> {
    let path=super::hub::path(owner,"paused");
    if path.exists(){
        if std::fs::read_to_string(&path).ok().is_some_and(|text|Snapshot::decode(&text,owner).is_ok()) {
            std::fs::copy(&path,path.with_extension("paused-backup")).map_err(|e|e.to_string())?;
        }
        std::fs::remove_file(path).map_err(|e|e.to_string())?;
    }Ok(())
}
pub fn draw(ui:&hudhook::imgui::Ui,view:Option<&super::runtime::View>) {
    use hudhook::imgui::{Condition,Key};
    let eligible=view.and_then(|v|v.run.as_ref()).is_some_and(|r|!matches!(r.phase,erdtree_tower::Phase::Dead|erdtree_tower::Phase::Won));
    if eligible&&ui.is_key_pressed_no_repeat(Key::F6){REQUEST.store(true,Ordering::Release);}
    let text=NOTICE.lock().map(|s|s.clone()).unwrap_or_default();
    if !eligible&&text.is_empty(){return;}
    ui.window("Journey suspension##tower-suspension").title_bar(false).resizable(false).movable(false)
        .position([(ui.io().display_size[0]-375.0).max(0.0),(ui.io().display_size[1]-145.0).max(0.0)],Condition::Always)
        .size([360.0,130.0],Condition::Always).build(||{
            if eligible {
                let label=match ROLE.load(Ordering::Acquire){1=>"整队保存并暂退 [F6]",2=>"保存并安全离队 [F6]",_=>"保存并暂退 [F6]"};
                if ui.button(super::i18n::t(label)){REQUEST.store(true,Ordering::Release);}
            }
            if !text.is_empty(){ui.text_wrapped(super::i18n::t(&text));}
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suspension_disk_roundtrip_preserves_backup_and_rejects_corruption() {
        let dir=std::env::temp_dir().join(format!("tower-pause-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&dir).unwrap();let path=dir.join("session.paused");let owner=(7,0);
        let mut run=Run::new(42,erdtree_tower::Class::Samurai);run.gold=123;
        save_at(&path,owner,&run).unwrap();run.gold=456;save_at(&path,owner,&run).unwrap();
        assert_eq!(Snapshot::decode(&std::fs::read_to_string(&path).unwrap(),owner).unwrap().gold,456);
        let backup=path.with_extension("paused-backup");assert_eq!(Snapshot::decode(&std::fs::read_to_string(&backup).unwrap(),owner).unwrap().gold,123);
        std::fs::write(&path,"broken").unwrap();assert!(save_at(&path,owner,&run).is_err());
        assert_eq!(Snapshot::decode(&std::fs::read_to_string(&backup).unwrap(),owner).unwrap().gold,123);
        std::fs::remove_file(path).unwrap();std::fs::remove_file(backup).unwrap();std::fs::remove_dir(dir).unwrap();
    }
}
