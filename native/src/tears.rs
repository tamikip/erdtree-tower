//! HansFix 惜别泪滴（Tears of Denial）：祷告，动作复用黄金树立誓。
//! 施放后身上挂特效 1699000（regulation 里新增，见 tools/hansfix/tears_regulation.py）：
//! 下一次会致死的伤害改为留 1 点生命，然后特效消失。持续到触发，或换房间时被游戏清掉为止。
//! 物品：祷告 6990（复制自黄金树立誓 6600，消耗 50 专注、100 精力）。名字与说明由这里在文本查询时补上。
use std::sync::atomic::{AtomicBool,AtomicU64,AtomicUsize,Ordering};
use eldenring::cs::ChrInsExt;
use fromsoftware_shared::FromStatic;
use std::time::Instant;

pub const SPELL:i32=6990;
pub const EFFECT:i32=1699000;
pub const ITEM:u32=0x40000000|SPELL as u32;
pub const HUD_ICON:u32=990950;
/// 秩序符文贴图（256×256）：惜别存在期间画在带着惜别的玩家胸前，破碎时闪烁后放大淡出
pub const RUNE_ICON:u32=990951;

/// 本机玩家角色的地址（每帧更新）与是否带着惜别
static PLAYER:AtomicUsize=AtomicUsize::new(0);
static ACTIVE:AtomicBool=AtomicBool::new(false);
/// 触发后要在游戏线程里摘掉特效
static CONSUMED:AtomicBool=AtomicBool::new(false);
/// 触发的时刻（毫秒，进程启动起）：0.4 秒内同一波连段的致死伤害也一并挡住
static TRIGGERED_AT:AtomicU64=AtomicU64::new(0);
static EPOCH:std::sync::LazyLock<Instant>=std::sync::LazyLock::new(Instant::now);
fn now_ms()->u64 {EPOCH.elapsed().as_millis() as u64+1}
const GRACE_MS:u64=400;
/// 破碎动画开始的时刻（毫秒）：图标放大、变红、淡出
static BROKEN_AT:AtomicU64=AtomicU64::new(0);
const BREAK_MS:u64=500;

/// 每帧（游戏任务线程）
/// 场上每个玩家：句柄、胸口位置、是否带着惜别、是否还活着
fn world_marks(world:&eldenring::cs::WorldChrMan)->Vec<(u64,[f32;3],bool,bool)> {
    let mut out:Vec<(u64,[f32;3],bool,bool)>=Vec::new();
    let mut add=|chr:&eldenring::cs::ChrIns| {
        let handle=unsafe {std::ptr::read_unaligned(std::ptr::from_ref(&chr.field_ins_handle).cast::<u64>())};
        if out.iter().any(|(h,..)|*h==handle) {return;}
        let p=&chr.modules.physics.position;
        let has=chr.special_effect.entries().any(|e|e.param_id==EFFECT);
        out.push((handle,[p.0,p.1+LAYOUT[4],p.2],has,chr.modules.data.hp>0));
    };
    if let Some(p)=world.main_player.as_ref() {add(&p.chr_ins);}
    for p in world.player_chr_set.characters() {add(&p.chr_ins);}
    out
}

#[derive(Clone,Copy)]
struct Mark {handle:u64,pos:[f32;3],active:bool,broken_at:u64}
static MARKS:std::sync::Mutex<Vec<Mark>>=std::sync::Mutex::new(Vec::new());
/// 相机：右、上、前、位置（游戏线程每帧拷一份给画面线程用）+ 纵向视角、宽高比
static CAMERA:std::sync::Mutex<Option<([[f32;3];4],f32,f32)>>=std::sync::Mutex::new(None);

fn track(now_marks:Vec<(u64,[f32;3],bool,bool)>) {
    let now=now_ms();
    let Ok(mut marks)=MARKS.lock() else{return};
    for (handle,pos,has,alive) in &now_marks {
        match marks.iter_mut().find(|m|m.handle==*handle) {
            Some(m)=>{
                m.pos=*pos;
                // 带着惜别 → 消失且人还活着：破碎（切图 / 死亡时整个消失，不放破碎动画）
                if m.active&&!has {m.active=false;if *alive {m.broken_at=now;}}
                if *has {m.active=true;m.broken_at=0;}
            }
            None if *has=>marks.push(Mark{handle:*handle,pos:*pos,active:true,broken_at:0}),
            None=>{}
        }
    }
    marks.retain(|m|(m.active&&now_marks.iter().any(|(h,..)|*h==m.handle))||(m.broken_at!=0&&now.saturating_sub(m.broken_at)<BREAK_MS));
    drop(marks);
    if let Ok(camera)=unsafe {eldenring::cs::CSCamera::instance()} {
        let c:&eldenring::cs::CSCam=&camera.pers_cam_1;
        let m=&c.matrix;
        let v=|r:&fromsoftware_shared::F32Vector4|[r.0,r.1,r.2];
        if let Ok(mut cam)=CAMERA.lock() {*cam=Some(([v(&m.0),v(&m.1),v(&m.2),v(&m.3)],c.fov,c.aspect_ratio));}
    }
}

fn project(pos:[f32;3],cam:&([[f32;3];4],f32,f32),screen:[f32;2])->Option<([f32;2],f32)> {
    let ([right,up,forward,eye],fov,_)=*cam;
    let d=[pos[0]-eye[0],pos[1]-eye[1],pos[2]-eye[2]];
    let dot=|a:[f32;3]|a[0]*d[0]+a[1]*d[1]+a[2]*d[2];
    let (x,y,z)=(dot(right),dot(up),dot(forward));
    if z<0.3 {return None;}
    let t=(fov*0.5).tan();
    let aspect=screen[0]/screen[1];
    let sx=if LAYOUT[6]!=0.0 {-x} else {x};
    // 每米在屏幕上多少像素（按纵向视角）
    let ppm=screen[1]*0.5/(t*z);
    Some(([screen[0]*0.5+sx/(t*aspect*z)*screen[0]*0.5,screen[1]*0.5-y/(t*z)*screen[1]*0.5],ppm))
}

/// 画世界里的符文（所有带着惜别的玩家，包括队友）
pub fn draw_world(ui:&hudhook::imgui::Ui,icon:&mut dyn FnMut(u32)->Option<hudhook::imgui::TextureId>,shown:bool) {
    if !shown {return;}
    let marks=MARKS.lock().map(|m|m.clone()).unwrap_or_default();
    if marks.is_empty() {return;}
    let Some(cam)=CAMERA.lock().ok().and_then(|c|*c) else{return};
    let Some(texture)=icon(RUNE_ICON) else{return};
    let screen=ui.io().display_size;
    let now=now_ms();
    let draw=ui.get_background_draw_list();
    for m in marks {
        let Some((center,ppm))=project(m.pos,&cam,screen) else{continue};
        let (scale,alpha,white)=if m.active {
            // 缓慢呼吸
            (1.0,LAYOUT[5]*(0.82+0.18*((ui.time() as f32)*1.6).sin()),0.0)
        } else {
            let t=now.saturating_sub(m.broken_at) as f32;
            if t<150.0 {
                // 闪烁：两下高亮
                let flash=if (t/37.5) as i32%2==0 {1.0} else {0.35};
                (1.0+0.08*flash,flash,flash)
            } else {
                let p=((t-150.0)/(BREAK_MS as f32-150.0)).clamp(0.0,1.0);
                (1.0+1.2*(1.0-(1.0-p)*(1.0-p)),LAYOUT[5]*(1.0-p).powf(1.3),0.0)
            }
        };
        let half=LAYOUT[3]*ppm*scale*0.5;
        if half<2.0||half>screen[1]*2.0 {continue;}
        // 贴图本身就是金色，用白色不压暗；按亮度倍数叠画几层（默认两层 ≈ 亮一倍），闪烁时额外偏白
        let _=white;
        let tint=hudhook::imgui::ImColor32::from_rgba(255,255,255,(alpha.clamp(0.0,1.0)*255.0) as u8);
        let layers=LAYOUT[7].round().max(1.0) as i32;
        for _ in 0..layers {draw.add_image(texture,[center[0]-half,center[1]-half],[center[0]+half,center[1]+half]).col(tint).build();}
    }
}

/// 换关卡（切图）时游戏会清掉身上的特效：记住「本局带着惜别」，新地图里角色出现后重新挂上。
/// 触发、死亡、开新局时忘掉。
static REMEMBERED:AtomicBool=AtomicBool::new(false);
static REMEMBERED_RUN:AtomicU64=AtomicU64::new(0);
/// 触发后 1 秒内：特效即使还在列表里也不再「记住」，并再摘一次
static SUPPRESS_UNTIL:AtomicU64=AtomicU64::new(0);
/// 胸口闪光点：和惜别同时存在的第二个状态（regulation 里 1699001，持续特效）
const GLYPH:i32=EFFECT+1;
static ACTIVE_SINCE:AtomicU64=AtomicU64::new(0);
static REFRESHED:std::sync::atomic::AtomicU8=std::sync::atomic::AtomicU8::new(0);
static REFRESH_GUARD:AtomicU64=AtomicU64::new(0);
static LAST_RESTORE:AtomicU64=AtomicU64::new(0);

pub fn tick(world:&mut eldenring::cs::WorldChrMan,run_seed:Option<u64>) {
    if let Some(seed)=run_seed&&REMEMBERED_RUN.swap(seed,Ordering::Relaxed)!=seed&&REMEMBERED.swap(false,Ordering::Relaxed) {
        crate::log("TEARS forgotten: new run");
    }
    let Some(player)=world.main_player.as_mut() else{PLAYER.store(0,Ordering::Relaxed);ACTIVE.store(false,Ordering::Relaxed);return};
    let chr=&mut player.chr_ins;
    PLAYER.store(std::ptr::from_ref::<eldenring::cs::ChrIns>(chr) as usize,Ordering::Relaxed);
    let consumed_now=CONSUMED.swap(false,Ordering::AcqRel);
    if consumed_now {
        // 摘掉特效：身上的黄金光纹（特效自带的 vfx）由游戏自己淡出
        chr.remove_speffect(EFFECT);
        REMEMBERED.store(false,Ordering::Relaxed);
        SUPPRESS_UNTIL.store(now_ms()+1000,Ordering::Relaxed);
        BROKEN_AT.store(now_ms(),Ordering::Relaxed);
        crate::sfx::play("shatter");
        chr.remove_speffect(GLYPH);
        crate::log("TEARS consumed: survived a lethal hit with 1 HP");
    }
    // 刚触发的这一帧：特效可能还没从列表里消失，按已失去处理，免得又被「记住」
    let suppressed=consumed_now||now_ms()<SUPPRESS_UNTIL.load(Ordering::Relaxed);
    let mut has=chr.special_effect.entries().any(|e|e.param_id==EFFECT);
    if suppressed {
        if has&&!consumed_now {chr.remove_speffect(EFFECT);}
        has=false;
    }
    let alive=chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag();
    if suppressed {
    } else if !alive {
        if REMEMBERED.swap(false,Ordering::Relaxed) {crate::log("TEARS forgotten: player died");}
    } else if has {
        REMEMBERED.store(true,Ordering::Relaxed);
    } else if REMEMBERED.load(Ordering::Relaxed)&&now_ms().saturating_sub(LAST_RESTORE.load(Ordering::Relaxed))>=500 {
        LAST_RESTORE.store(now_ms(),Ordering::Relaxed);
        chr.apply_speffect(EFFECT,false);
        crate::log("TEARS restored after map change");
    }
    // 本机特效补建：刚挂上（施放、切图重挂）时特效系统可能还没就绪，状态挂上了但本机没建出特效实例，之后也不会再建，
    // 结果自己看不见、队友看得见。挂上后 2 秒、6 秒各重挂一次；重挂造成的短暂消失不算新的一次挂上。
    let now=now_ms();
    if has&&!ACTIVE.load(Ordering::Relaxed)&&now>=REFRESH_GUARD.load(Ordering::Relaxed) {
        ACTIVE_SINCE.store(now,Ordering::Relaxed);REFRESHED.store(0,Ordering::Relaxed);
    }
    if has&&alive {
        let age=now.saturating_sub(ACTIVE_SINCE.load(Ordering::Relaxed));
        let stage=REFRESHED.load(Ordering::Relaxed);
        if (stage==0&&age>=2000)||(stage==1&&age>=6000) {
            REFRESHED.store(stage+1,Ordering::Relaxed);
            REFRESH_GUARD.store(now+1500,Ordering::Relaxed);
            chr.remove_speffect(EFFECT);
            chr.apply_speffect(EFFECT,false);
            chr.remove_speffect(GLYPH);
            chr.apply_speffect(GLYPH,false);
            crate::log(format!("TEARS aura refreshed at {}s",age/1000));
        }
    }
    // 胸口闪光点（GLYPH，持续特效）跟着惜别走：惜别在就挂，没了就摘（补建造成的短暂消失不摘）
    let glyph=chr.special_effect.entries().any(|e|e.param_id==GLYPH);
    if has&&alive&&!glyph {chr.apply_speffect(GLYPH,false);}
    if !(has&&alive)&&glyph&&now>=REFRESH_GUARD.load(Ordering::Relaxed) {chr.remove_speffect(GLYPH);}
    track(world_marks(world));

    if has&&!ACTIVE.load(Ordering::Relaxed)&&now>=REFRESH_GUARD.load(Ordering::Relaxed) {crate::log("TEARS active");}
    ACTIVE.store(has,Ordering::Relaxed);
}

/// 扣血前（游戏线程，damage.rs）：本机玩家带着惜别、这一下会致死时，返回改后的伤害（留 1 点生命）
pub fn lethal(target:usize,hp:i32,damage:i32)->Option<i32> {
    if target==0||target!=PLAYER.load(Ordering::Relaxed)||hp<=0||damage<hp {return None;}
    let now=now_ms();
    let recent=now.saturating_sub(TRIGGERED_AT.load(Ordering::Relaxed))<GRACE_MS;
    if ACTIVE.swap(false,Ordering::AcqRel) {
        TRIGGERED_AT.store(now,Ordering::Relaxed);
        CONSUMED.store(true,Ordering::Release);
    } else if !recent {return None;}
    Some((hp-1).max(0))
}

/// 状态图标：带着惜别时画在星辉叠层图标下面（位置可在模组根目录 tears-hud.ini 里改 x= y= size=）
/// [x, y, size, 符文世界尺寸米 rune_m, 符文高度米 rune_h, 符文不透明度 rune_alpha, 水平翻转 flip_x, 亮度倍数 rune_bright]
static LAYOUT:std::sync::LazyLock<[f32;8]>=std::sync::LazyLock::new(|| {
    let mut layout=[150.0f32,200.0,34.0,0.55,1.0,1.0,0.0,2.0];
    if let Ok(text)=std::fs::read_to_string(crate::LOG_PATH.with_file_name("tears-hud.ini")) {
        for line in text.lines() {
            let Some((k,v))=line.split_once('=') else{continue};
            let Ok(v)=v.trim().parse::<f32>() else{continue};
            match k.trim() {"x"=>layout[0]=v,"y"=>layout[1]=v,"size"=>layout[2]=v.max(8.0),"rune_m"=>layout[3]=v.clamp(0.2,5.0),"rune_h"=>layout[4]=v.clamp(-1.0,3.0),"rune_alpha"=>layout[5]=v.clamp(0.05,1.0),"flip_x"=>layout[6]=v,"rune_bright"=>layout[7]=v.clamp(1.0,4.0),_=>{}}
        }
    }
    layout
});
pub fn draw_hud(ui:&hudhook::imgui::Ui,icon:&mut dyn FnMut(u32)->Option<hudhook::imgui::TextureId>,shown:bool) {
    let broken=BROKEN_AT.load(Ordering::Relaxed);
    let since=now_ms().saturating_sub(broken);
    let breaking=broken!=0&&since<BREAK_MS;
    let refreshing=now_ms()<REFRESH_GUARD.load(Ordering::Relaxed);
    if !shown||!(ACTIVE.load(Ordering::Relaxed)||breaking||refreshing) {return;}
    if unsafe {eldenring::cs::CSFeManImp::instance()}.ok().is_some_and(|fe|fe.hud_state==eldenring::cs::CSFeManHudState::HideAll) {return;}
    let s=(ui.io().display_size[1]/1080.0).max(0.5);
    let [x,y,size]=[LAYOUT[0]*s,LAYOUT[1]*s,LAYOUT[2]*s];
    let Some(texture)=icon(HUD_ICON) else{return};
    let (scale,color)=if breaking {
        // 破碎：0.5 秒内放大到 1.7 倍、染红、淡出
        let p=since as f32/BREAK_MS as f32;
        let ease=1.0-(1.0-p)*(1.0-p);
        let fade=(1.0-p).powf(1.4);
        let gb=(255.0*(1.0-ease)).max(40.0) as u8;
        (1.0+0.7*ease,hudhook::imgui::ImColor32::from_rgba(255,gb,gb,(fade*255.0) as u8))
    } else {
        // 轻微呼吸，提示「还有一次机会」
        let pulse=0.85+0.15*((ui.time() as f32)*2.2).sin();
        (1.0,hudhook::imgui::ImColor32::from_rgba(255,255,255,(pulse*255.0) as u8))
    };
    let (cx,cy,half)=(x+size/2.0,y+size/2.0,size*scale/2.0);
    ui.get_foreground_draw_list().add_image(texture,[cx-half,cy-half],[cx+half,cy+half]).col(color).build();
}

/// 文本：祷告 6990 在原版文本表里没有条目。查询返回空时按表的种类补上名字 / 简介 / 说明。
/// 用黄金树立誓（6600）在同一张表里的文本判断这张表是哪一种：名字很短、简介一行、说明有空行分段。
pub fn text(table:usize,lookup:&dyn Fn(usize,i32)->*const u16)->Option<*const u16> {
    use std::collections::HashMap;use std::sync::Mutex;
    static CACHE:std::sync::LazyLock<Mutex<HashMap<usize,Option<Box<[u16]>>>>>=std::sync::LazyLock::new(Default::default);
    let mut cache=CACHE.lock().ok()?;
    if let Some(v)=cache.get(&table) {return v.as_ref().map(|t|t.as_ptr());}
    let read=|id:i32|->Option<String> {
        let raw=lookup(table,id);
        if raw.is_null() {return None;}
        let len=(0..4096).position(|i|unsafe {*raw.add(i)}==0)?;
        Some(String::from_utf16_lossy(unsafe {std::slice::from_raw_parts(raw,len)}))
    };
    // 物品表：同时有黄金树立誓（6600）和红色圣杯瓶（1000）
    let value=match (read(6600),read(1000)) {
        (Some(vow),Some(_))=>{
            let lang=if crate::i18n::t("祷告")=="祷告" {0} else if crate::i18n::t("祷告")=="禱告" {1} else {2};
            let kind=if vow.contains("\n\n")||vow.chars().count()>60 {2} else if vow.chars().count()>14 {1} else {0};
            let text=match (kind,lang) {
                (0,0)=>"惜别泪滴",(0,1)=>"惜別淚滴",(0,_)=>"Tears of Denial",
                (1,0)=>"生命值归零时，获得一次撑住的机会",(1,1)=>"生命值歸零時，獲得一次撐住的機會",
                (1,_)=>"Grants one chance to endure when HP reaches 0.",
                (_,0)=>"自遥远国度流传而来的祷告。\n\n施放后，下一次受到致命伤害时，\n会以 1 点生命撑住，效果随即消失。\n效果持续到触发为止，换关卡也会保留；
死亡或开始新的旅程时消失。\n\n消耗 50 点专注、100 点精力。",
                (_,1)=>"自遙遠國度流傳而來的禱告。\n\n施放後，下一次受到致命傷害時，\n會以 1 點生命撐住，效果隨即消失。\n效果持續到觸發為止，換關卡也會保留；
死亡或開始新的旅程時消失。\n\n消耗 50 點專注、100 點精力。",
                _=>"An incantation handed down from a distant land.\n\nGrants one chance to endure when HP reaches 0:\nthe next lethal blow leaves you at 1 HP, then the effect fades.\nLasts until triggered, even across stages;
lost on death or when a new journey begins.\n\nCosts 50 FP and 100 stamina.",
            };
            Some(text.encode_utf16().chain([0]).collect::<Box<[u16]>>())
        }
        _=>None,
    };
    let ptr=value.as_ref().map(|t|t.as_ptr());
    cache.insert(table,value);
    ptr
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_first_lethal_hit_and_its_combo_are_survived() {
        PLAYER.store(0x1000,Ordering::Relaxed);ACTIVE.store(true,Ordering::Relaxed);
        assert_eq!(lethal(0x2000,100,500),None,"other characters are untouched");
        assert_eq!(lethal(0x1000,100,50),None,"non-lethal hits are untouched");
        assert_eq!(lethal(0x1000,100,500),Some(99));
        assert!(!ACTIVE.load(Ordering::Relaxed)&&CONSUMED.load(Ordering::Relaxed));
        assert_eq!(lethal(0x1000,1,30),Some(0),"a follow-up hit in the same combo keeps you at 1 HP");
        std::thread::sleep(std::time::Duration::from_millis(GRACE_MS+50));
        assert_eq!(lethal(0x1000,1,30),None,"after the grace window you can die normally");
    }
}
