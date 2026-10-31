use std::{collections::HashMap,sync::LazyLock};
use erdtree_tower::{Reward,Run,Phase};
use hudhook::{RenderContext,imgui::{Ui,Io,Key,Condition,StyleColor,StyleVar,ImColor32,TextureId}};
use crate::runtime::{self,Action,View};

#[derive(Clone)]
struct Detail {icon:u32,name:String,info:String,caption:String,stats:String,skill:String}
static RARITIES:LazyLock<HashMap<u32,u8>>=LazyLock::new(||include_str!("../../scene-assets/reward-rarities.tsv").lines().chain(include_str!("../../scene-assets/affix-rarities.tsv").lines()).chain(include_str!("../../scene-assets/affix-v2-rarities.tsv").lines()).chain(include_str!("../../scene-assets/talisman-rarities.tsv").lines()).map(|line| {
    let (id,rarity)=line.split_once('\t').unwrap();(id.parse().unwrap(),rarity.parse().unwrap())
}).collect());
pub fn item_quality(id:u32)->u8 {RARITIES.get(&erdtree_tower::rewards::spirit_base(id)).copied().unwrap_or(0).min(3)}
pub(crate) fn rarity_color(tier:usize)->[f32;4] {
    static COLORS:LazyLock<[[f32;4];4]>=LazyLock::new(|| {
        let rows:Vec<_>=include_str!("../../scene-assets/equipment-colors.txt").lines().enumerate().map(|(index,line)| {
            let (id,hex)=line.split_once('|').unwrap();assert_eq!(id.parse::<usize>().unwrap(),index);
            let rgb=u32::from_str_radix(hex,16).unwrap();[(rgb>>16) as f32/255.0,((rgb>>8)&255) as f32/255.0,(rgb&255) as f32/255.0,1.0]
        }).collect();rows.try_into().unwrap()
    });
    COLORS[tier.min(3)]
}
#[test]
fn affixed_equipment_details_and_icons_cover_every_variant() {
    for item in erdtree_tower::affixes::CATALOG.iter() {
        let detail=&DETAILS[&item.id];
        assert_eq!(detail.name,item.name);assert!(!detail.info.is_empty());
        assert_eq!(item_quality(item.id),erdtree_tower::affixes::tier(item.id).unwrap());
        let base=erdtree_tower::affixes::base(item.id);
        assert_eq!(detail.icon,DETAILS[&base].icon);
    }
}
fn quality(run:&Run,reward:&Reward)->usize {
    if matches!(reward,Reward::MemorySlot){return 1;}
    if let Reward::Ember(ember)=reward {return usize::from(ember.quality());}
    if matches!(reward,Reward::StarStream|Reward::UpgradeStone){return 2;}
    item_id(run,reward).map_or(0,item_quality) as usize
}
/// 三选一离场动画的长度（秒），播完才提交领取 / 放弃
const REWARD_OUT:f64=0.45;
/// 0→1 的缓出（三次方）
fn ease(elapsed:f64,start:f64,len:f64)->f32 {let t=((elapsed-start)/len).clamp(0.0,1.0) as f32;1.0-(1.0-t).powi(3)}
/// 带回弹的 0→1（越过一点再回来）
fn back_out(elapsed:f64,start:f64,len:f64)->f32 {
    let t=((elapsed-start)/len).clamp(0.0,1.0) as f32;let c=1.70158;let u=t-1.0;
    if t<=0.0 {0.0}else{1.0+(c+1.0)*u*u*u+c*u*u}
}
fn card_light(tier:usize,selected:bool,hover:bool,time:f64)->u8 {
    let base=if selected{240.0}else if hover{195.0}else{150.0};
    (base+if tier==3 {15.0*(time*std::f64::consts::TAU/3.6).sin()}else{0.0}) as u8
}
static DETAILS:LazyLock<HashMap<u32,Detail>>=LazyLock::new(|| {
    let mut result:HashMap<u32,Detail>=include_str!("../../scene-assets/reward-details.tsv").lines().chain(include_str!("../../scene-assets/affix-details.tsv").lines()).chain(include_str!("../../scene-assets/affix-v2-details.tsv").lines()).chain(include_str!("../../scene-assets/talisman-details.tsv").lines()).map(|line| {
        let p:Vec<_>=line.split('\t').collect();
        (p[0].parse().unwrap(),Detail{icon:p[1].parse().unwrap(),name:p[2].into(),info:p[3].replace("\\n","\n"),caption:p[4].replace("\\n","\n"),stats:p[5].replace("\\n","\n"),skill:p[6].replace("\\n","\n")})
    }).collect();
    for (id,detail) in &mut result {
        // 原版没有战技的盾牌，默认战灰是“无技能”，中文名是占位的“DLC dummy”
        if detail.skill.starts_with("DLC dummy") {detail.skill="无\n这件装备原本没有战技，可在铁匠处安装战灰。".into();}
        if super::random_ashes::eligible(*id){detail.skill="获得时独立随机战灰\n包含本体与 DLC，已放开武器类型限制；获得后固定，可在铁匠处更换。".into();}
    }
    for item in erdtree_tower::rewards::CATALOG.iter().filter(|i|i.kind==erdtree_tower::rewards::ItemKind::Spirit) {
        if let Some(base)=result.get(&item.id).cloned() {
            for level in 1..=10 {
                let mut detail=base.clone();detail.name=erdtree_tower::rewards::lookup(item.id+level).unwrap().name.into();
                result.insert(item.id+level,detail);
            }
        }
    }
    result
});
/// 路线图的构筑面板：词条装备的名称与词条行（只保留带数值的行）；id 为奖励编号（防具带 0x10000000 标记）
/// 构筑面板里的一件词条装备：图标、品质、名字与带数值的词条
#[derive(Clone,Debug,Default,PartialEq)]
pub struct Gear {pub icon:u32,pub quality:u8,pub name:String,pub lines:Vec<String>}
pub fn affix_summary(id:u32)->Option<Gear> {
    let quality=erdtree_tower::affixes::tier(id)?;
    let detail=DETAILS.get(&id)?;
    let lines=detail.info.lines().filter(|l|l.chars().any(|c|c.is_ascii_digit())&&(l.contains('+')||l.contains('-'))).map(str::to_owned).collect();
    Some(Gear{icon:detail.icon,quality,name:detail.name.clone(),lines})
}
/// 图鉴：所有词条装备上出现过的词条名（去掉数值，按装备编号的先后）
pub fn affix_names()->&'static [String] {
    static NAMES:LazyLock<Vec<String>>=LazyLock::new(|| {
        let mut ids:Vec<_>=DETAILS.keys().copied().filter(|id|erdtree_tower::affixes::tier(*id).is_some()).collect();ids.sort_unstable();
        let mut names:Vec<String>=Vec::new();
        for gear in ids.into_iter().filter_map(affix_summary) {
            for name in gear.lines.iter().filter_map(|line|erdtree_tower::journal::affix_name(line)) {if !names.iter().any(|n|n==name){names.push(name.to_owned());}}
        }
        names
    });
    &NAMES
}
/// 护符赐福的护符图标
pub fn talisman_icon(id:u32)->Option<u32> {DETAILS.get(&(0x20000000|id)).map(|detail|detail.icon)}
pub fn weapon_type_icon(kind:u8)->Option<u32> {
    erdtree_tower::rewards::CATALOG.iter().filter(|i|i.kind==erdtree_tower::rewards::ItemKind::Weapon&&i.id/1_000_000==u32::from(kind))
        .find_map(|i|DETAILS.get(&i.id).map(|d|d.icon))
}
/// 调试菜单：按名称或说明搜索可以直接发放的奖励物品（含词条装备）
pub fn search_items(query:&str,limit:usize)->Vec<(u32,String)> {
    let query=query.trim();if query.is_empty() {return vec![];}
    let mut hits:Vec<_>=DETAILS.iter().filter(|(id,d)|erdtree_tower::rewards::lookup(**id).is_some()&&(d.name.contains(query)||d.info.contains(query)))
        .map(|(id,d)|(*id,format!("{}　{}",d.name,d.info.replace("\\n","　").replace('\n',"　")))).collect();
    hits.sort_by_key(|(id,_)|*id);hits.truncate(limit);hits
}
pub fn talisman_description(id:u32)->(String,String) {
    DETAILS.get(&(0x20000000|id)).map(|d|(d.name.clone(),d.info.clone()))
        .unwrap_or_else(||(erdtree_tower::rewards::lookup(0x20000000|id).map_or("护符赐福",|i|i.name).into(),String::new()))
}
fn item_id(run:&Run,reward:&Reward)->Option<u32> {
    match reward {
        Reward::Item(id)=>Some(*id),
        Reward::TalismanBuff(id)=>Some(0x20000000|id),
        Reward::SacredTear=>Some(0x40000000|erdtree_tower::elite::TEAR),
        Reward::Blessing(kind)=>Some(0x20000000|crate::blessings::talisman(*kind,run.blessings.iter().find(|b|b.kind==*kind).map_or(1,|b|(b.level+1).min(3))).0),
        _=>None,
    }
}
/// 余烬徽记（scene-builder/ember_art.py 生成的 990000 起 12 张）；缺任何一张就整套借用原版护符图标
const EMBER_EMBLEMS:u32=990000;
static EMBLEMS_READY:LazyLock<bool>=LazyLock::new(|| {
    let root=crate::LOG_PATH.with_file_name("scene-assets").join("reward-icons");
    (0..erdtree_tower::embers::Ember::ALL.len() as u32).all(|i|std::fs::metadata(root.join(format!("{}.rgba",EMBER_EMBLEMS+i))).is_ok_and(|m|m.len()==160*160*4))
});
pub fn ember_icon(ember:erdtree_tower::embers::Ember)->Option<u32> {
    if *EMBLEMS_READY {return Some(EMBER_EMBLEMS+ember.index() as u32);}
    DETAILS.get(&(0x20000000|erdtree_tower::embers::icon_talisman(ember))).map(|detail|detail.icon)
}
/// 特殊奖励的卡片图标（scene-builder/extra_reward_icons.py 生成的 990100 起 5 张）：武器强化、圣杯扩容、记忆扩容、卢恩、强化石；
/// 文件缺失时照旧显示两个字
const EXTRA_ICONS:u32=990100;
fn card_icon(run:&Run,reward:&Reward)->Option<u32> {
    if let Reward::Ember(ember)=reward {return ember_icon(*ember);}
    let extra=match reward {Reward::UpgradeWeapon=>Some(0),Reward::FlaskCapacity=>Some(1),Reward::MemorySlot=>Some(2),Reward::Gold(_)=>Some(3),Reward::UpgradeStone=>Some(4),_=>None};
    if let Some(index)=extra {return Some(EXTRA_ICONS+index);}
    let id=match reward {Reward::StarStream=>Some(0x40000000|4200),_=>item_id(run,reward)};
    id.and_then(|id|DETAILS.get(&id)).map(|detail|detail.icon)
}
fn card_summary(detail:&Detail)->String {
    if !detail.info.trim().is_empty(){return if detail.skill.is_empty(){detail.info.clone()}else{format!("战技：{}\n{}",detail.skill.lines().next().unwrap_or(""),detail.info)};}
    let weight=detail.stats.lines().find(|line|line.starts_with("重量：")).unwrap_or("");
    let skill=detail.skill.lines().next().unwrap_or("");
    if !skill.is_empty() {
        let purpose=match detail.stats.lines().find_map(|line|line.strip_prefix("施法能力：")) {
            Some("魔法与祷告")=>"施展魔法与祷告",Some("魔法")=>"施展魔法",Some("祷告")=>"施展祷告",_=>"",
        };
        if !purpose.is_empty() {
            let effect=detail.caption.split("\n\n").find(|part|part.contains("能强化")||part.contains("能提升")||part.contains("能缩短"));
            return format!("战技：{skill}\n{}\n{}",effect.map_or(purpose.to_owned(),|s|s.replace('\n',"")),weight);
        }
        return format!("战技：{skill}\n{weight}");
    }
    detail.caption.split("\n\n").next().unwrap_or("").replace('\n',"")
}
fn offer_detail(run:&Run,node:usize,slot:usize,reward:&Reward)->Option<Detail> {
    let id=item_id(run,reward)?;let mut detail=DETAILS.get(&id)?.clone();
    let gem=super::random_ashes::preview(run,node,slot,id);
    if let Some((name,text))=super::random_ashes::description(gem){detail.skill=format!("{name}\n{}",text.replace("\\n","\n"));}
    Some(detail)
}
#[derive(Default)]
struct Selection {key:String,selected:usize,details:bool,skip:bool,armed_at:f64,last_submit:f64,
    /// 这组奖励出现的时间（入场动画从这里算）
    opened:f64,
    /// 离场动画：开始时间与动画播完后要提交的操作
    closing:Option<(f64,Pending)>,
    /// 操作已提交、等游戏切走这一页的时间（这期间不画；2 秒还没切走就重新显示）
    closed:Option<f64>}
/// 离场动画播完后提交的操作
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Pending {Claim(usize),Gear(usize),Skip}
impl Selection {
    fn close_details(&mut self,now:f64) {self.details=false;self.armed_at=now+0.25;}
    fn sync(&mut self,key:String,now:f64) {
        if self.key!=key {*self=Self{key,armed_at:now+0.35,last_submit:f64::NEG_INFINITY,opened:now,..Default::default()};}
    }
    fn step(&mut self,delta:isize,count:usize) {if count>0 {self.selected=(self.selected as isize+delta).rem_euclid(count as isize) as usize;}}
    fn claim(&mut self,now:f64,ready:bool)->Option<usize> {
        if !ready||self.details||self.skip||now<self.armed_at||now-self.last_submit<0.5 {return None;}
        self.last_submit=now;Some(self.selected)
    }
}
#[derive(Default)]
pub struct Panel {selection:Selection,textures:HashMap<u32,Option<TextureId>>,wanted:Vec<u32>,card:Option<TextureId>,rarity_cards:Option<TextureId>,rare_blue:Option<TextureId>}
impl Panel {
    pub fn icon(&mut self,icon:u32)->Option<TextureId> {
        let texture=self.textures.get(&icon).copied();if texture.is_none()&&!self.wanted.contains(&icon){self.wanted.push(icon);}texture.flatten()
    }
    pub fn load_icons(&mut self,renderer:&mut dyn RenderContext) {
        let root=crate::LOG_PATH.with_file_name("scene-assets").join("reward-icons");
        for icon in self.wanted.drain(..self.wanted.len().min(6)).collect::<Vec<_>>() {
            let texture=match std::fs::read(root.join(format!("{icon}.rgba"))) {
                Ok(data) if data.len()==160*160*4=>renderer.load_texture(&data,160,160).map_err(|e|crate::log(format!("Reward texture upload failed: {icon} {e:?}"))).ok(),
                _=>{crate::log(format!("Reward icon missing: {icon}"));None},
            };
            self.textures.insert(icon,texture);
        }
    }
    pub fn draw_contract(&mut self,ui:&Ui,run:&Run,view:&View) {
        let Some(contract)=run.contract.as_ref().filter(|c|c.selection_pending())else{return;};
        let now=ui.time();
        self.selection.sync(format!("contract:{}:{}:{:?}",run.seed,run.act,contract.cards()),now);
        let ready=view.ready&&!view.busy&&now>=self.selection.armed_at;
        let key=|keys:&[Key]|keys.iter().any(|k|ui.is_key_pressed_no_repeat(*k));
        let count=contract.options();
        if ready {
            if key(&[Key::A,Key::LeftArrow,Key::GamepadDpadLeft]){self.selection.step(-1,count);}
            if key(&[Key::D,Key::RightArrow,Key::GamepadDpadRight]){self.selection.step(1,count);}
        }
        const LEVELS:[&str;4]=["","I","II","III"];
        let fresh=contract.rules==1;
        use erdtree_tower::contracts::Card;
        // (贴图, 标题, 角标说明, 增益, 代价, 描边颜色)
        let cards:Vec<(usize,String,Option<String>,String,String,Option<(u8,u8,u8)>)>=contract.cards().into_iter().map(|card|match card {
            Card::Oath(kind) if fresh=>(kind.index(),format!("{} I",kind.name()),None,kind.boon_at(1),kind.bane_at().to_owned(),None),
            Card::Oath(kind)=>(kind.index(),kind.name().to_owned(),None,kind.boon().to_owned(),kind.bane().to_owned(),None),
            Card::Deepen(kind)=>{
                let level=(contract.level(kind).unwrap_or(1)+1).min(3);
                (kind.index(),format!("{} {}",kind.name(),LEVELS[usize::from(level)]),Some(format!("深化 · {} 升至 {}",LEVELS[usize::from(level-1)],LEVELS[usize::from(level)])),kind.boon_at(level),"代价不变 · 获得圣杯滴露 ×1".to_owned(),Some((236,196,104)))
            },
        }).collect();
        let size=ui.io().display_size;let s=(size[0]/1600.0).min(size[1]/900.0);
        let oy=(size[1]-900.0*s)*0.5;let mid=size[0]*0.5;
        let gold=[0.91,0.79,0.53,1.0];
        let _bg=ui.push_style_color(StyleColor::WindowBg,[0.02,0.02,0.02,1.0]);
        let _border=ui.push_style_color(StyleColor::Border,[0.0,0.0,0.0,0.0]);
        ui.window("幕间契约").position([0.0,0.0],Condition::Always).size(size,Condition::Always)
            .title_bar(false).resizable(false).movable(false).collapsible(false).scroll_bar(false).build(|| {
            crate::oath_art::background(&ui.get_window_draw_list(),size);
            ui.set_window_font_scale(s*0.95);center(ui,mid,oy+32.0*s,&format!("第 {} 幕",run.act),gold);
            ui.set_window_font_scale(s*1.85);center(ui,mid,oy+73.0*s,"立 下 誓 约",gold);
            let subtitle=if !fresh {"二选一 · 本幕生效"}else if count==3 {"三选一 · 誓约整局保留，可以深化"}else{"二选一 · 誓约整局保留"};
            ui.set_window_font_scale(s*0.9);center(ui,mid,oy+127.0*s,subtitle,[0.63,0.58,0.46,1.0]);
            if fresh&&!contract.held.is_empty() {
                let held=contract.held.iter().map(|o|format!("{} {}",o.kind.name(),LEVELS[usize::from(o.level.min(3))])).collect::<Vec<_>>().join(" · ");
                ui.set_window_font_scale(s*0.8);center(ui,mid,oy+155.0*s,&format!("已立誓约：{held}"),[0.80,0.70,0.50,1.0]);
            }
            let width=366.0*s;let gap=72.0*s;let top=oy+187.0*s;let height=549.0*s;
            for (i,(art,title,label,boon,bane,accent)) in cards.iter().enumerate() {
                let x=mid-(width*count as f32+gap*(count as f32-1.0))*0.5+i as f32*(width+gap);
                ui.set_cursor_pos([x,top-7.0*s]);
                {let _disabled=ui.begin_disabled(!ready);if ui.invisible_button(format!("contract-{i}"),[width,height+7.0*s]){self.selection.selected=i;}}
                let hovered=ui.is_item_hovered();let selected=self.selection.selected==i;
                let y=top-if selected{7.0*s}else if hovered{3.0*s}else{0.0};
                crate::oath_art::card(&ui.get_window_draw_list(),*art,[x,y],[x+width,y+height],if selected{255}else if hovered{225}else{165});
                if let Some((r,g,b))=accent {
                    ui.get_window_draw_list().add_rect([x-4.0*s,y-4.0*s],[x+width+4.0*s,y+height+4.0*s],ImColor32::from_rgba(*r,*g,*b,if selected{230}else{150})).rounding(6.0*s).thickness(2.5*s).build();
                }
                if let Some(label)=label {
                    let (r,g,b)=accent.unwrap_or((236,196,104));
                    ui.set_window_font_scale(s*0.85);
                    let half=ui.calc_text_size(label)[0]*0.5+14.0*s;let (cx,cy)=(x+width*0.5,y+300.0*s);
                    let draw=ui.get_window_draw_list();
                    draw.add_rect([cx-half,cy-3.0*s],[cx+half,cy+ui.text_line_height()+3.0*s],ImColor32::from_rgba(14,10,7,225)).filled(true).rounding(11.0*s).build();
                    draw.add_rect([cx-half,cy-3.0*s],[cx+half,cy+ui.text_line_height()+3.0*s],ImColor32::from_rgba(r,g,b,220)).rounding(11.0*s).thickness(1.4*s).build();
                    drop(draw);
                    center(ui,cx,cy,label,[f32::from(r)/255.0*0.6+0.4,f32::from(g)/255.0*0.6+0.4,f32::from(b)/255.0*0.6+0.4,1.0]);
                }
                ui.set_window_font_scale(s*1.35);center(ui,x+width*0.5,y+333.0*s,title,gold);
                ui.set_window_font_scale(s*0.95);
                let bane_y=(401.0+boon.lines().count() as f32*24.0+14.0).max(462.0);
                for (offset,text,color) in [(401.0,boon.as_str(),[0.64,0.83,0.58,1.0]),(bane_y,bane.as_str(),[0.91,0.49,0.40,1.0])] {
                    center(ui,x+width*0.5,y+offset*s,text,color);
                }
            }
            let min=[mid-210.0*s,oy+786.0*s];let max=[mid+210.0*s,oy+852.0*s];
            ui.set_cursor_pos(min);
            let confirm={let _disabled=ui.begin_disabled(!ready);ui.invisible_button("confirm-oath",[420.0*s,66.0*s])};
            let hovered=ui.is_item_hovered();
            crate::oath_art::button(&ui.get_window_draw_list(),min,max,if !ready{130}else if hovered{255}else{220});
            crate::pad::label_in(ui,min,max,Some(crate::pad::CONFIRM),"确认誓约",gold,s,crate::pad::Tone::Light);
            if confirm||key(&[Key::Enter,Key::GamepadFaceDown]) {if let Some(index)=self.selection.claim(now,ready){runtime::submit(Action::Contract(index));}}
            // 跳过：和确认同款的纹理按钮，小一号
            let skip={let _disabled=ui.begin_disabled(!ready);ui.set_window_font_scale(s*0.9);textured_button(ui,"跳过誓约",Some(crate::pad::BACK),[32.0*s,oy+834.0*s],[250.0*s,46.0*s],gold)};
            if (skip||key(&[Key::Escape,Key::GamepadFaceRight]))&&self.selection.claim(now,ready).is_some() {runtime::submit(Action::SkipContract);}
            if !crate::oath_art::ready(){ui.set_window_font_scale(s*0.8);center(ui,mid,oy+866.0*s,"誓约贴图缺失，请重启更新后的启动器",[0.91,0.49,0.40,1.0]);}
        });
    }
    pub fn initialize(&mut self,renderer:&mut dyn RenderContext) {
        self.textures.clear();self.wanted.clear();self.rare_blue=None;self.rarity_cards=None;self.card=None;
        let path=crate::LOG_PATH.with_file_name("scene-assets").join("reward-art/rare-blue.rgba");
        match std::fs::read(path) {
            Ok(bytes) if bytes.len()==887*1774*4=>self.rare_blue=renderer.load_texture(&bytes,887,1774).ok(),
            _=>crate::log("Blue reward texture missing or invalid"),
        }
        let path=crate::LOG_PATH.with_file_name("scene-assets").join("reward-art/rarity-cards.rgba");
        match std::fs::read(path) {
            Ok(bytes) if bytes.len()==1536*1024*4=>self.rarity_cards=renderer.load_texture(&bytes,1536,1024).ok(),
            _=>crate::log("Reward rarity texture missing or invalid"),
        }
        let card_path=crate::LOG_PATH.with_file_name("scene-assets").join("reward-art/card.rgba");
        match std::fs::read(card_path) {
            Ok(bytes) if bytes.len()==1024*1536*4=>self.card=renderer.load_texture(&bytes,1024,1536).ok(),
            _=>crate::log("Reward card texture missing or invalid"),
        }
        std::thread::spawn(||{LazyLock::force(&DETAILS);LazyLock::force(&RARITIES);});
    }
    pub fn draw(&mut self,ui:&Ui,run:&Run,view:&View) {
        let gift=run.contract.as_ref().filter(|c|c.chosen.is_some()&&!c.gear_done&&!c.deepened);
        let gift_offers:Vec<_>=gift.map(|c|c.gear.iter().copied().map(Reward::Item).collect()).unwrap_or_default();
        let (id,offers)=if gift.is_some(){(usize::MAX,&gift_offers)}else if let Phase::Reward{id,offers}=&run.phase{(*id,offers)}else{return;};
        if offers.is_empty(){return;}
        let now=ui.time();
        self.selection.sync(format!("{}:{id}:{offers:?}",run.seed),now);
        if let Some(receipt)=gift.and_then(|c|c.gift.as_ref()){self.selection.selected=receipt.index;}
        // 已提交、等游戏切走这一页：先不画；2 秒还没切走（提交没生效）就重新显示
        if let Some(at)=self.selection.closed {
            if now-at<2.0 {return;}
            self.selection.closed=None;self.selection.opened=now;self.selection.armed_at=now+0.35;
        }
        // 离场动画播完：提交操作
        if let Some((at,pending))=self.selection.closing.filter(|(at,_)|now-at>=REWARD_OUT) {
            let _=at;
            runtime::submit(match pending {Pending::Claim(i)=>Action::Claim(i),Pending::Gear(i)=>Action::Gear(i),Pending::Skip=>if gift.is_some(){Action::SkipGear}else{Action::KeepCurrency}});
            self.selection.closing=None;self.selection.closed=Some(now);return;
        }
        let closing=self.selection.closing.map(|(at,pending)|(((now-at)/REWARD_OUT).clamp(0.0,1.0) as f32,pending));
        let opened=now-self.selection.opened;
        let ready=view.ready&&!view.busy&&now>=self.selection.armed_at&&closing.is_none();
        let key=|keys:&[Key]|keys.iter().any(|k|ui.is_key_pressed_no_repeat(*k));
        let was_details=self.selection.details;
        if ready&&!self.selection.details&&!self.selection.skip {
            if key(&[Key::A,Key::LeftArrow,Key::GamepadDpadLeft]){self.selection.step(-1,offers.len());}
            if key(&[Key::D,Key::RightArrow,Key::GamepadDpadRight]){self.selection.step(1,offers.len());}
            if key(&[Key::Tab,Key::GamepadFaceUp]){self.selection.details=true;}
        }
        if closing.is_none()&&key(&[Key::Escape,Key::GamepadFaceRight]){if self.selection.details {self.selection.close_details(now);}self.selection.skip=false;}
        if ready&&!self.selection.details&&!self.selection.skip&&key(&[Key::GamepadFaceLeft]) {
            self.selection.skip=true;self.selection.armed_at=now+0.25;
        }
        let size=ui.io().display_size;let scale=(size[0]/1600.0).min(size[1]/900.0);
        let gold=[0.91,0.79,0.53,1.0];let ink=[0.88,0.85,0.76,1.0];
        // 不铺背景图：透出游戏场景，只压一层暗色（详情页文字多，压得更暗）
        let _bg=ui.push_style_color(StyleColor::WindowBg,[0.0,0.0,0.0,0.0]);
        let _border=ui.push_style_color(StyleColor::Border,[0.0,0.0,0.0,0.0]);
        let _text=ui.push_style_color(StyleColor::Text,gold);
        let _button=ui.push_style_color(StyleColor::Button,[0.16,0.14,0.10,1.0]);
        let _hover=ui.push_style_color(StyleColor::ButtonHovered,[0.32,0.26,0.15,1.0]);
        ui.window("战利品##reward").position([0.0,0.0],Condition::Always).size(size,Condition::Always)
            .title_bar(false).resizable(false).movable(false).collapsible(false).scroll_bar(false).build(|| {
            ui.set_window_font_scale(scale);
            {
                let draw=ui.get_window_draw_list();let shade=|a:f32|ImColor32::from_rgba(0,0,0,(a*255.0) as u8);
                // 入场第一帧也留一点暗色（完全透明的图形 ImGui 不画，这一帧就什么都没有了）
                let fade=ease(opened,0.0,0.3).max(0.05)*closing.map_or(1.0,|(k,_)|1.0-k);
                let shade=|a:f32|shade(a*fade);
                let base=if self.selection.details {0.75}else{0.45};
                draw.add_rect([0.0,0.0],size,shade(base)).filled(true).build();
                // 上下两端再暗一些（底部是按钮），四周压暗角，卡片所在的中间最亮
                draw.add_rect_filled_multicolor([0.0,0.0],[size[0],size[1]*0.22],shade(0.35),shade(0.35),shade(0.0),shade(0.0));
                draw.add_rect_filled_multicolor([0.0,size[1]*0.7],size,shade(0.0),shade(0.0),shade(0.45),shade(0.45));
                draw.add_rect_filled_multicolor([0.0,0.0],[size[0]*0.18,size[1]],shade(0.3),shade(0.0),shade(0.0),shade(0.3));
                draw.add_rect_filled_multicolor([size[0]*0.82,0.0],size,shade(0.0),shade(0.3),shade(0.3),shade(0.0));
            }
            if self.selection.details {
                let reward=&offers[self.selection.selected];
                let preview=offer_detail(run,id,self.selection.selected,reward);let d=preview.as_ref();
                let left=(size[0]-1360.0*scale)*0.5;
                let top=(size[1]-900.0*scale)*0.5;
                ui.set_window_font_scale(scale*1.45);
                ui.set_cursor_pos([left,top+48.0*scale]);
                let tier=quality(run,reward);let text_color=rarity_color(tier);
                {
                    // 详情页标题：与奖励卡相同的品质特效
                    let font=crate::fancy_text::title_font().map(|f|ui.push_font(f));
                    if font.is_some() {ui.set_window_font_scale(scale*0.94);}
                    let origin=ui.window_pos();let size=ui.current_font_size();
                    let name=d.map_or("奖励详情",|d|d.name.as_str());
                    crate::fancy_text::quality_name(crate::fancy_text::Layer::Window,origin[0]+left,origin[1]+top+48.0*scale,size,name,text_color,tier,0.0,ui.time() as f32);
                    if let Some(font)=font {font.pop();}
                }
                ui.set_window_font_scale(scale);
                let _child=ui.push_style_color(StyleColor::ChildBg,[0.035,0.04,0.035,0.92]);
                ui.set_cursor_pos([left,top+115.0*scale]);
                ui.child_window("物品属性").size([540.0*scale,640.0*scale]).build(|| {
                    ui.set_window_font_scale(scale);
                    if let Some(d)=d {
                        if let Some(texture)=self.icon(d.icon) {
                            ui.set_cursor_pos([150.0*scale,18.0*scale]);
                            hudhook::imgui::Image::new(texture,[240.0*scale,240.0*scale]).build(ui);
                        }
                        ui.set_cursor_pos([24.0*scale,285.0*scale]);
                        let _indent=ui.push_style_var(hudhook::imgui::StyleVar::IndentSpacing(24.0*scale));ui.indent();
                        let _wrap=ui.push_text_wrap_pos_with_pos(516.0*scale);let _cjk=crate::fancy_text::wrap_at(516.0*scale);
                        ui.text_colored(gold,"属性");ui.spacing();
                        let _text=ui.push_style_color(StyleColor::Text,ink);
                        crate::fancy_text::text_wrapped(ui,if d.stats.trim().is_empty(){"—"}else{&d.stats});
                        ui.unindent();
                    } else {crate::fancy_text::text_wrapped(ui,crate::map_ui::reward_description(run,reward));}
                });
                ui.set_cursor_pos([left+580.0*scale,top+115.0*scale]);
                ui.child_window("物品说明").size([780.0*scale,640.0*scale]).build(|| {
                    ui.set_window_font_scale(scale);
                    if key(&[Key::UpArrow,Key::GamepadDpadUp]){ui.set_scroll_y((ui.scroll_y()-80.0*scale).max(0.0));}
                    if key(&[Key::DownArrow,Key::GamepadDpadDown]){ui.set_scroll_y(ui.scroll_y()+80.0*scale);}
                    ui.set_cursor_pos([24.0*scale,20.0*scale]);
                    let _indent=ui.push_style_var(hudhook::imgui::StyleVar::IndentSpacing(24.0*scale));
                    ui.indent();
                    let _wrap=ui.push_text_wrap_pos_with_pos(750.0*scale);let _cjk=crate::fancy_text::wrap_at(750.0*scale);
                    let _text=ui.push_style_color(StyleColor::Text,ink);
                    if let Some(d)=d {
                        if !d.info.trim().is_empty(){ui.text_colored(gold,"效果");ui.spacing();{let _color=ui.push_style_color(StyleColor::Text,text_color);crate::fancy_text::text_wrapped(ui,&d.info);}ui.spacing();ui.separator();ui.spacing();}
                        if matches!(reward,Reward::TalismanBuff(_)){crate::fancy_text::text_wrapped(ui,"本局生效，不占护符栏位");ui.spacing();}
                        ui.text_colored(gold,"物品说明");ui.spacing();
                        if item_id(run,reward).is_some_and(|id|erdtree_tower::affixes::tier(id).is_some()) {
                            if let Some((lore,affix))=d.caption.rsplit_once("\n\n") {
                                crate::fancy_text::text_wrapped(ui,lore);ui.spacing();let _color=ui.push_style_color(StyleColor::Text,text_color);crate::fancy_text::text_wrapped(ui,affix);
                            } else {let _color=ui.push_style_color(StyleColor::Text,text_color);crate::fancy_text::text_wrapped(ui,&d.caption);}
                        } else {crate::fancy_text::text_wrapped(ui,&d.caption);}
                        if !d.skill.trim().is_empty(){ui.spacing();ui.separator();ui.spacing();ui.text_colored(gold,"战技");ui.spacing();crate::fancy_text::text_wrapped(ui,&d.skill);}
                    } else {crate::fancy_text::text_wrapped(ui,crate::map_ui::reward_description(run,reward));}
                    ui.unindent();
                });
                if textured_button(ui,"返回",Some(crate::pad::BACK),[left,top+795.0*scale],[300.0*scale,52.0*scale],gold){self.selection.close_details(now);}
                let origin=ui.window_pos();
                crate::pad::hints(ui,[origin[0]+left+580.0*scale,origin[1]+top+821.0*scale],0.0,&[(None,"滚轮"),(Some(crate::pad::MOVE_Y),"滚动说明")],24.0*scale,17.0*scale,gold,crate::pad::Tone::Light);
                return;
            }
            let width=330.0*scale;let gap=36.0*scale;let height=490.0*scale;
            let left=(size[0]-(width+gap)*offers.len() as f32+gap)*0.5;let top=178.0*scale;
            for (i,reward) in offers.iter().enumerate() {
                let x=left+i as f32*(width+gap);
                let selected=self.selection.selected==i;
                let hover=ready&&!self.selection.details&&!self.selection.skip&&ui.is_mouse_hovering_rect([x,top],[x+width,top+height]);
                // 入场：从下方依次升起（略微越过再落回）并淡入；离场：选中的卡上浮淡出，其余下沉淡出
                let start=0.05+i as f64*0.08;
                let (rise,appear)=(back_out(opened,start,0.5),ease(opened,start,0.3));
                let chosen=closing.is_some_and(|(_,p)|p==Pending::Claim(i)||p==Pending::Gear(i));
                let (out_y,out_a)=match closing {
                    Some((k,_)) if chosen=>(-30.0*k,1.0-((k-0.35)/0.65).clamp(0.0,1.0)),
                    Some((k,_))=>(90.0*k*k,1.0-(k*1.7).min(1.0)),
                    None=>(0.0,1.0),
                };
                let alpha=appear*out_a;
                if alpha<=0.0 {continue;}
                let _alpha=ui.push_style_var(StyleVar::Alpha(alpha));
                let a8=|a:f32|(a*alpha*255.0) as u8;
                let y=top-if selected{7.0*scale}else if hover{3.0*scale}else{0.0}+((1.0-rise)*70.0+out_y)*scale;
                let preview=offer_detail(run,id,i,reward);let detail=preview.as_ref();
                let (category,title,summary)=match reward {
                    Reward::Item(id)=>{
                        let item=erdtree_tower::rewards::lookup(*id);let base=erdtree_tower::affixes::base(*id);let legendary=erdtree_tower::elite::LEGENDARY.contains(&base);
                        let category=item.map_or("物品",|v|if legendary {match v.kind {erdtree_tower::rewards::ItemKind::Sorcery=>"传说魔法",erdtree_tower::rewards::ItemKind::Incantation=>"传说祷告",_=>"传说武器"}}else if erdtree_tower::elite::EPIC.contains(&base){"史诗武器"}else{v.kind.label()});
                        (category,detail.map_or("未知物品".into(),|v|v.name.clone()),detail.map_or(String::new(),card_summary))
                    },
                    Reward::Blessing(_)=>("护符",detail.map_or("护符".into(),|v|v.name.clone()),detail.map_or(String::new(),|v|v.info.clone())),
                    Reward::UpgradeWeapon=>("强化","武器强化".into(),"背包最高等效等级\n再普通 +3 / 失色 +1".into()),
                    Reward::Gold(n)=>("卢恩",format!("{n} 卢恩"),"获得卢恩".into()),
                    Reward::TalismanBuff(_)=>("护符赐福",detail.map_or("护符效果".into(),|v|v.name.clone()),format!("{}\n本局生效，不占护符栏位",detail.map_or("",|v|v.info.as_str()))),
                    Reward::SacredTear=>("强化材料","圣杯滴露 ×1".into(),"圣杯瓶恢复量 +1\n交给服务 NPC 使用，最高 +12".into()),
                    Reward::UpgradeStone=>("强化材料","强化石 ×1".into(),"在营地铁匠或商店\n强化一次武器（追上最高等效等级）\n也可升阶护符、调灵骨灰；可保留".into()),
                    Reward::FlaskCapacity=>("增益","圣杯扩容".into(),"红、蓝瓶上限各 +1\n本局生效，不补瓶".into()),
                    Reward::MemorySlot=>("增益","记忆扩容".into(),"记忆栏位 +1\n魔法与祷告共用，本局生效".into()),
                    Reward::StarStream=>("增益","星流引导".into(),"彗星亚兹勒随视角左右瞄准\n光束保持水平".into()),
                    Reward::Ember(ember)=>("余烬",ember.name().into(),format!("{}\n本局生效",ember.effect())),
                };
                let tier=quality(run,reward);
                // 余烬：卡面与名字按余烬的品质着色，类别写成“余烬 · 品质”，外加一圈余火色的外框，和普通奖励区分开
                let ember=matches!(reward,Reward::Ember(_));
                let quality_color=rarity_color(tier);
                let ember_category=format!("余烬 · {}",["普通","稀有","史诗","传说"][tier.min(3)]);
                let category=if ember {ember_category.as_str()}else{category};
                if let Some(texture)=if tier==1 {self.rare_blue}else{None} {
                    let light=card_light(tier,selected,hover,now);
                    ui.get_window_draw_list().add_image(texture,[x,y],[x+width,y+height])
                        .col(ImColor32::from_rgba(light,light,light,a8(1.0))).build();
                } else if let Some(texture)=self.rarity_cards {
                    let cell=tier.saturating_sub(1) as f32;
                    let light=card_light(tier,selected,hover,now+i as f64*0.4);
                    ui.get_window_draw_list().add_image(texture,[x,y],[x+width,y+height])
                        .uv_min([(cell+0.002)/3.0,0.0]).uv_max([(cell+0.998)/3.0,1.0])
                        .col(ImColor32::from_rgba(light,light,light,a8(1.0))).build();
                } else if let Some(texture)=self.card {
                    let light=if selected{255}else if hover{225}else{172};
                    ui.get_window_draw_list().add_image(texture,[x,y],[x+width,y+height])
                        .col(ImColor32::from_rgba(light,light,light,a8(1.0))).build();
                }
                // 领取时选中的卡外扩一圈金光
                if let Some((k,_))=closing.filter(|_|chosen) {
                    let d=(4.0+36.0*k)*scale;
                    ui.get_window_draw_list().add_rect([x-d,y-d],[x+width+d,y+height+d],ImColor32::from_rgba(255,226,160,((1.0-k)*230.0) as u8)).rounding(4.0*scale).thickness((3.0-2.0*k)*scale).build();
                    ui.get_window_draw_list().add_rect([x,y],[x+width,y+height],ImColor32::from_rgba(255,240,205,((1.0-k)*(1.0-k)*120.0) as u8)).filled(true).build();
                }
                if selected {
                    let draw=ui.get_window_draw_list();
                    for (offset,alpha,thickness) in [(3.0,35,9.0),(2.0,80,4.0),(1.0,220,1.5)] {
                        draw.add_rect([x-offset*scale,y-offset*scale],[x+width+offset*scale,y+height+offset*scale],ImColor32::from_rgba(225,204,143,a8(alpha as f32/255.0))).rounding(2.0*scale).thickness(thickness*scale).build();
                    }
                }
                if ember {
                    let draw=ui.get_window_draw_list();
                    for (offset,alpha,thickness) in [(6.0,40,10.0),(4.0,90,4.0),(2.0,230,2.0)] {
                        draw.add_rect([x-offset*scale,y-offset*scale],[x+width+offset*scale,y+height+offset*scale],ImColor32::from_rgba(236,120,58,a8(alpha as f32/255.0))).rounding(4.0*scale).thickness(thickness*scale).build();
                    }
                }
                center(ui,x+width*0.5,y+61.0*scale,category,quality_color);
                if let Some(texture)=card_icon(run,reward).and_then(|icon|self.icon(icon)) {
                    let side=175.0*scale;ui.get_window_draw_list().add_image(texture,[x+(width-side)*0.5,y+91.0*scale],[x+(width+side)*0.5,y+266.0*scale]).col(ImColor32::from_rgba(255,255,255,a8(1.0))).build();
                } else {
                    center(ui,x+width*0.5,y+145.0*scale,match reward {Reward::UpgradeWeapon=>"强化",Reward::FlaskCapacity=>"圣杯",Reward::MemorySlot=>"记忆",Reward::StarStream=>"星流",_=>"卢恩"},gold);
                }
                // 物品名与余烬名用标题字体：传说整名金色流光，其他品质的前缀带特效；其他奖励保持原样
                if !ember&&!matches!(reward,Reward::Item(_)) {
                    ui.set_window_font_scale(scale*1.2);
                    let title_width=ui.calc_text_size(&title)[0];
                    if title_width>width-40.0*scale {ui.set_window_font_scale(scale*1.2*(width-40.0*scale)/title_width);}
                    center(ui,x+width*0.5,y+278.0*scale,&title,quality_color);
                } else {
                    let font=crate::fancy_text::title_font().map(|f|ui.push_font(f));
                    ui.set_window_font_scale(scale*if font.is_some(){0.78}else{1.2});
                    let mut size=ui.current_font_size();
                    let title_width=crate::fancy_text::measure(&title,size);
                    if title_width>width-40.0*scale {size*=(width-40.0*scale)/title_width;}
                    let origin=ui.window_pos();
                    crate::fancy_text::quality_name(crate::fancy_text::Layer::Window,origin[0]+x+width*0.5,origin[1]+y+276.0*scale,size,&title,[quality_color[0],quality_color[1],quality_color[2],quality_color[3]*alpha],tier,0.5,now as f32);
                    if let Some(font)=font {font.pop();}
                }
                ui.set_window_font_scale(scale*0.90);
                let summary=if summary.chars().count()>48 {format!("{}…",summary.chars().take(48).collect::<String>())}else{summary};
                ui.set_cursor_pos([x+40.0*scale,y+329.0*scale]);
                {let _color=ui.push_style_color(StyleColor::Text,quality_color);let _wrap=ui.push_text_wrap_pos_with_pos(x+width-40.0*scale);let _cjk=crate::fancy_text::wrap_at(x+width-40.0*scale);crate::fancy_text::text_wrapped(ui,summary);}
                ui.set_window_font_scale(scale);
                if selected {center(ui,x+width*0.5,y+height-76.0*scale,"已选",gold);}
                ui.set_cursor_pos([x,y]);let _disabled=ui.begin_disabled(!ready||self.selection.details||self.selection.skip);
                if ui.invisible_button(format!("reward-card-{i}"),[width,height]){self.selection.selected=i;}
            }
            let buttons_y=top+height+25.0*scale;
            let controls=ease(opened,0.35,0.3)*closing.map_or(1.0,|(k,_)|1.0-(k*2.0).min(1.0));
            let _controls=ui.push_style_var(StyleVar::Alpha(controls.max(0.001)));
            ui.set_cursor_pos([size[0]*0.5-255.0*scale,buttons_y]);
            let mut confirm=false;
            {let _disabled=ui.begin_disabled(!ready||self.selection.details||self.selection.skip);
                confirm|=textured_button(ui,"领取",Some(crate::pad::CONFIRM),[size[0]*0.5-255.0*scale,buttons_y],[245.0*scale,48.0*scale],gold);
                if textured_button(ui,"详情",Some(crate::pad::Input::new(crate::pad::Pad::Y,"Tab")),[size[0]*0.5+10.0*scale,buttons_y],[245.0*scale,48.0*scale],gold){self.selection.details=true;}
            }
            if !was_details&&!self.selection.details&&!self.selection.skip {confirm|=key(&[Key::Enter,Key::GamepadFaceDown]);}
            if confirm {if let Some(index)=self.selection.claim(now,ready){self.selection.closing=Some((now,if gift.is_some(){Pending::Gear(index)}else{Pending::Claim(index)}));}}
            {let origin=ui.window_pos();crate::pad::hints(ui,[origin[0]+size[0]*0.5,origin[1]+buttons_y+76.0*scale],0.5,&[(Some(crate::pad::MOVE_X),"切换")],24.0*scale,17.0*scale,[gold[0],gold[1],gold[2],controls],crate::pad::Tone::Light);}
            {let _disabled=ui.begin_disabled(!ready||self.selection.details||self.selection.skip);
                ui.set_window_font_scale(scale*0.85);
                if textured_button(ui,"跳过奖励",Some(crate::pad::Input::new(crate::pad::Pad::X,"")),[24.0*scale,size[1]-66.0*scale],[220.0*scale,44.0*scale],gold){self.selection.skip=true;self.selection.armed_at=now+0.25;}
                ui.set_window_font_scale(scale);
            }
            if self.selection.skip {
                ui.set_cursor_pos([size[0]*0.5-220.0*scale,size[1]*0.4]);
                let _child=ui.push_style_color(StyleColor::ChildBg,[0.09,0.095,0.08,1.0]);
                ui.child_window("确认放弃").size([440.0*scale,150.0*scale]).border(true).build(||{
                    crate::fancy_text::text_wrapped(ui,"放弃本次三选一奖励并继续旅程？");
                    if (crate::pad::button(ui,"skip-confirm","确认放弃",Some(crate::pad::Input::new(crate::pad::Pad::A,"")),[0.0,0.0])||key(&[Key::GamepadFaceDown]))&&ready&&now>=self.selection.armed_at {self.selection.closing=Some((now,Pending::Skip));self.selection.skip=false;}
                    ui.same_line();if crate::pad::button(ui,"skip-cancel","返回选择",Some(crate::pad::BACK),[0.0,0.0]){self.selection.skip=false;}
                });
            }
        });
    }
}
/// 纹理按钮：pos 为窗口内坐标；文字左边加按键标签（手柄 / 键盘随最后用的设备切换），文字用当前的字号
fn textured_button(ui:&Ui,label:&str,input:Option<crate::pad::Input>,pos:[f32;2],size:[f32;2],color:[f32;4])->bool {
    ui.set_cursor_pos(pos);
    let clicked=ui.invisible_button(label,size);
    let origin=ui.window_pos();let (min,max)=([origin[0]+pos[0],origin[1]+pos[1]],[origin[0]+pos[0]+size[0],origin[1]+pos[1]+size[1]]);
    crate::oath_art::button_alpha(&ui.get_window_draw_list(),min,max,if ui.is_item_hovered(){255}else{205},(ui.clone_style().alpha*255.0) as u8);
    let scale=ui.current_font_size()/{ui.set_window_font_scale(1.0);ui.current_font_size()};
    crate::pad::label_in(ui,min,max,input,label,color,scale,crate::pad::Tone::Light);
    clicked
}
fn center(ui:&Ui,x:f32,y:f32,text:&str,color:[f32;4]) {ui.set_cursor_pos([x-ui.calc_text_size(text)[0]*0.5,y]);ui.text_colored(color,text);}

#[cfg(test)]
fn write_frame(path:&std::path::Path,data:&hudhook::imgui::DrawData,width:i32,height:i32) {
    use std::io::Write;
    let mut file=std::fs::File::create(path).unwrap();writeln!(file,"{width} {height}").unwrap();
    for list in data.draw_lists(){for cmd in list.commands(){if let hudhook::imgui::DrawCmd::Elements{count,cmd_params:p}=cmd {
        writeln!(file,"C {} {} {} {} {}",p.texture_id.id(),p.clip_rect[0],p.clip_rect[1],p.clip_rect[2],p.clip_rect[3]).unwrap();
        for triangle in list.idx_buffer()[p.idx_offset..p.idx_offset+count].chunks_exact(3) {
            write!(file,"T").unwrap();for index in triangle {let v=list.vtx_buffer()[*index as usize+p.vtx_offset];write!(file," {} {} {} {} {} {} {} {}",v.pos[0],v.pos[1],v.uv[0],v.uv[1],v.col[0],v.col[1],v.col[2],v.col[3]).unwrap();}writeln!(file).unwrap();
        }
    }}}
}

#[cfg(test)]
pub fn test_frames(context:&mut hudhook::imgui::Context) {
    crate::oath_art::preview();
    use std::io::Write;
    let output=std::env::var("ERDTREE_UI_PREVIEW").ok().map(std::path::PathBuf::from);
    if let Some(path)=&output {
        std::fs::create_dir_all(path).unwrap();let atlas=context.fonts().build_rgba32_texture();
        std::fs::write(path.join("font.rgba"),atlas.data).unwrap();
        std::fs::write(path.join("font.size"),format!("{} {}",atlas.width,atlas.height)).unwrap();
    }
    let mut run=Run::new(42,erdtree_tower::Class::Samurai);
    run.phase=Phase::Reward{id:0,offers:vec![Reward::Blessing(erdtree_tower::BlessingKind::Vigor),Reward::Item(9000000),Reward::UpgradeWeapon]};
    let mut view=runtime::view().unwrap();view.ready=true;view.busy=false;view.status.clear();
    let mut panel=Panel::default();
    panel.card=Some(TextureId::new(200003));
    panel.rarity_cards=Some(TextureId::new(200004));
    panel.rare_blue=Some(TextureId::new(200005));
    for (width,height) in [(1280,720),(1920,1080),(3440,1440)] {
        context.io_mut().display_size=[width as f32,height as f32];
    }
    for detail in DETAILS.values(){panel.textures.insert(detail.icon,Some(TextureId::new(detail.icon as usize+1)));}
    if let Some(path)=&output {
        let mut talismans=run.clone();
        talismans.phase=Phase::Reward{id:0,offers:[("生命护符",2),("汲灵护符",3),("雷龙爆发护符",3)].into_iter().map(|(name,tier)|Reward::Item(erdtree_tower::talismans::CATALOG.iter().find(|i|i.name.ends_with(name)&&erdtree_tower::talismans::tier(i.id)==Some(tier)).unwrap().id)).collect()};
        context.io_mut().display_size=[1920.0,1080.0];
        for details in [false,true] {
            for _ in 0..4 {panel.draw(context.frame(),&talismans,&view);context.render();panel.selection.selected=1;panel.selection.details=details;}
            panel.draw(context.frame(),&talismans,&view);let data=context.render();
            write_frame(&path.join(if details{"talisman-details-1920.draw"}else{"talismans-1920.draw"}),data,1920,1080);
        }
        let mut colored=run.clone();
        colored.phase=Phase::Reward{id:0,offers:(1..=3).map(|tier|Reward::Item(erdtree_tower::affixes::CATALOG.iter().find(|i|erdtree_tower::affixes::base(i.id)==1000000&&erdtree_tower::affixes::tier(i.id)==Some(tier)&&erdtree_tower::affixes::profile(i.id)==Some(0)).unwrap().id)).collect()};
        context.io_mut().display_size=[1920.0,1080.0];
        for details in [false,true] {
            for _ in 0..4 {panel.draw(context.frame(),&colored,&view);context.render();panel.selection.selected=1;panel.selection.details=details;}
            panel.draw(context.frame(),&colored,&view);let data=context.render();
            write_frame(&path.join(if details{"rarity-details-1920.draw"}else{"rarity-1920.draw"}),data,1920,1080);
        }
    }
    for (width,height) in [(1280,720),(1920,1080),(3440,1440)] {
        context.io_mut().display_size=[width as f32,height as f32];
        let mut elite=Run::new(42,erdtree_tower::Class::Astrologer);
        elite.phase=Phase::Reward{id:0,offers:erdtree_tower::elite::offers(&elite,0)};
        if let Phase::Reward{ref offers,..}=elite.phase {for reward in offers {assert!(item_id(&elite,reward).is_some_and(|id|DETAILS.contains_key(&id)));}}
        for _ in 0..3 {panel.draw(context.frame(),&elite,&view);context.render();}
        panel.draw(context.frame(),&elite,&view);let data=context.render();assert!(data.total_vtx_count>0);
        if width==1920 {if let Some(path)=&output {write_frame(&path.join("elite-1920.draw"),data,width,height);}}
    }
    for (width,height) in [(1280,720),(1920,1080),(3440,1440)] {
        context.io_mut().display_size=[width as f32,height as f32];
        for kind in erdtree_tower::contracts::Kind::ALL {
            let mut contract=Run::new(42,erdtree_tower::Class::Samurai);contract.enable_contracts();
            contract.contract.as_mut().unwrap().offers=[kind,erdtree_tower::contracts::Kind::ALL[(kind.index()+1)%erdtree_tower::contracts::Kind::ALL.len()]];
            for _ in 0..3 {panel.draw_contract(context.frame(),&contract,&view);context.render();}
            panel.draw_contract(context.frame(),&contract,&view);let data=context.render();assert!(data.total_vtx_count>0);
            if let Some(path)=&output {write_frame(&path.join(format!("oath-{}-{width}.draw",kind.index())),data,width,height);}
            contract.contract.as_mut().unwrap().choose(0,contract.seed,contract.class).unwrap();
            contract.contract.as_mut().unwrap().peak=24;
            for _ in 0..3 {panel.draw(context.frame(),&contract,&view);context.render();}
            panel.draw(context.frame(),&contract,&view);let data=context.render();assert!(data.total_vtx_count>0);
            if width==1920&&kind==erdtree_tower::contracts::Kind::Mystic {if let Some(path)=&output {write_frame(&path.join("contract-gift-1920.draw"),data,width,height);}}
        }
        use erdtree_tower::contracts::{Kind,Oath,State};
        let mut deepen=Run::new(42,erdtree_tower::Class::Samurai);
        deepen.contract=Some(State::start(deepen.reward_seed(),deepen.class,2,vec![Oath{kind:Kind::Steel,level:1},Oath{kind:Kind::Blood,level:2}],0));
        assert_eq!(deepen.contract.as_ref().unwrap().options(),3);
        for _ in 0..3 {panel.draw_contract(context.frame(),&deepen,&view);context.render();}
        panel.draw_contract(context.frame(),&deepen,&view);let data=context.render();assert!(data.total_vtx_count>0);
        if let Some(path)=&output {write_frame(&path.join(format!("oath-deepen-{width}.draw")),data,width,height);}
        let seed=deepen.reward_seed();deepen.contract.as_mut().unwrap().choose(2,seed,deepen.class).unwrap();
        panel.draw(context.frame(),&deepen,&view);context.render();
    }
    run.phase=Phase::Reward{id:0,offers:vec![Reward::Item(1000000),Reward::Item(erdtree_tower::elite::EPIC[0]),Reward::Item(2140000)]};
    for (width,height,details) in [(1280,720,false),(1280,720,true),(1920,1080,false),(1920,1080,true),(3440,1440,false),(3440,1440,true)] {
        context.io_mut().display_size=[width as f32,height as f32];
        for _ in 0..24 {
            panel.draw(context.frame(),&run,&view);context.render();
            panel.selection.selected=1;panel.selection.details=details;
        }
        panel.draw(context.frame(),&run,&view);
        let data=context.render();assert!(data.total_vtx_count>0);
        if let Some(path)=&output {
            let mut f=std::fs::File::create(path.join(format!("{width}-{height}-{details}.draw"))).unwrap();
            writeln!(f,"{width} {height}").unwrap();
            for list in data.draw_lists(){for command in list.commands(){if let hudhook::imgui::DrawCmd::Elements{count,cmd_params:p}=command {
                writeln!(f,"C {} {} {} {} {}",p.texture_id.id(),p.clip_rect[0],p.clip_rect[1],p.clip_rect[2],p.clip_rect[3]).unwrap();
                for triangle in list.idx_buffer()[p.idx_offset..p.idx_offset+count].chunks_exact(3) {
                    write!(f,"T").unwrap();for index in triangle {let v=list.vtx_buffer()[*index as usize+p.vtx_offset];write!(f," {} {} {} {} {} {} {} {}",v.pos[0],v.pos[1],v.uv[0],v.uv[1],v.col[0],v.col[1],v.col[2],v.col[3]).unwrap();}writeln!(f).unwrap();
                }
            }}}
        }
    }
}

#[repr(C)]
#[derive(Default)]
struct Gamepad {buttons:u16,left_trigger:u8,right_trigger:u8,lx:i16,ly:i16,rx:i16,ry:i16}
#[repr(C)]
#[derive(Default)]
struct PadState {packet:u32,pad:Gamepad}
#[link(name="xinput1_4",kind="raw-dylib")]
unsafe extern "system" {fn XInputGetState(index:u32,state:*mut PadState)->u32;}
fn stick(value:i16)->f32 {
    let value=value as f32;
    if value.abs()<=8000.0 {0.0}else{value.signum()*((value.abs()-8000.0)/24767.0).min(1.0)}
}
fn pad_direction(pad:&Gamepad,mask:u16,map:bool)->bool {
    pad.buttons&mask!=0 || !map&&match mask {1=>stick(pad.ly)>0.5,2=>stick(pad.ly)< -0.5,4=>stick(pad.lx)< -0.5,8=>stick(pad.lx)>0.5,_=>false}
}
#[derive(Default)]
struct MapPointer {position:Option<[f32;2]>,physical:Option<[i32;2]>}
impl MapPointer {
    fn step(&mut self,active:bool,physical:Option<[i32;2]>,mouse:[f32;2],size:[f32;2],motion:[f32;2],dt:f32)->Option<[f32;2]> {
        if !active {*self=Self::default();return None;}
        if physical.zip(self.physical).is_some_and(|(now,old)|now!=old) {self.position=None;}
        self.physical=physical;
        let moving=motion.iter().any(|&value|value!=0.0);
        if !moving&&self.position.is_none(){return None;}
        let old=self.position.unwrap_or(mouse);
        let point=std::array::from_fn(|axis| {
            let end=(size[axis]-1.0).max(0.0);
            let start=if old[axis].is_finite()&&old[axis]>=0.0 {old[axis]}else{end*0.5};
            (start+motion[axis].clamp(-1.0,1.0)*700.0*dt.clamp(0.0,0.05)).clamp(0.0,end)
        });
        self.position=Some(point);Some(point)
    }
}
thread_local! {static MAP_POINTER:std::cell::RefCell<MapPointer>=std::cell::RefCell::new(MapPointer::default());}
#[derive(Default)]
struct MapConfirm {active:bool,down:bool,pressed:bool}
impl MapConfirm {
    fn update(&mut self,active:bool,down:bool) {
        self.pressed=active&&self.active&&down&&!self.down;
        self.active=active;self.down=down;
    }
}
thread_local! {static MAP_CONFIRM:std::cell::RefCell<MapConfirm>=std::cell::RefCell::new(MapConfirm::default());}
pub fn map_command()->Option<(Option<[f32;2]>,bool)> {
    MAP_CONFIRM.with(|state|{let state=state.borrow();state.active.then(||(MAP_POINTER.with(|p|p.borrow().position),state.pressed))})
}
thread_local! {static PAD_RETRY:std::cell::Cell<[Option<std::time::Instant>;4]>=const {std::cell::Cell::new([None;4])};}
pub fn poll_pad(io:&mut Io,active:bool,map:bool) {
    let mut state=PadState::default();let mut found=false;
    if active {PAD_RETRY.with(|retry| {
        let mut slots=retry.get();let now=std::time::Instant::now();
        for i in 0..4 {
            if slots[i].is_some_and(|at|now<at){continue;}
            match unsafe{XInputGetState(i as u32,&mut state)} {0=>{slots[i]=None;found=true;break;},1167=>slots[i]=Some(now+std::time::Duration::from_secs(2)),_=>slots[i]=None}
        }
        retry.set(slots);
    });}
    feed_pad(io,&state.pad,found,map);
}
fn feed_pad(io:&mut Io,pad:&Gamepad,found:bool,map:bool) {
    crate::pad::note_pad(found,pad.buttons!=0||[pad.lx,pad.ly,pad.rx,pad.ry].iter().any(|v|stick(*v)!=0.0));
    MAP_CONFIRM.with(|state|state.borrow_mut().update(found&&map,found&&pad.buttons&0x1000!=0));
    io.backend_flags.set(hudhook::imgui::BackendFlags::HAS_GAMEPAD,found);
    for (key,mask) in [(Key::GamepadDpadUp,1),(Key::GamepadDpadDown,2),(Key::GamepadDpadLeft,4),(Key::GamepadDpadRight,8),(Key::GamepadFaceDown,0x1000),(Key::GamepadFaceRight,0x2000),(Key::GamepadFaceLeft,0x4000),(Key::GamepadFaceUp,0x8000),(Key::GamepadBack,0x20),(Key::GamepadL1,0x100),(Key::GamepadR1,0x200)] {
        io.add_key_event(key,found&&pad_direction(pad,mask,map));
    }
    if !found||!map {MAP_POINTER.with(|state|*state.borrow_mut()=MapPointer::default());}
    if found&&map {
        let button=|mask|if pad.buttons&mask!=0 {1.0}else{0.0};
        let x=stick(pad.lx)+button(8)-button(4);
        let y= -stick(pad.ly)+button(2)-button(1);
        let point=MAP_POINTER.with(|state|state.borrow_mut().step(true,crate::input::desktop_pointer(),io.mouse_pos,io.display_size,[x,y],io.delta_time));
        if let Some(point)=point {io.add_mouse_pos_event(point);}
        let scroll=stick(pad.ry)+button(0x100)-button(0x200);
        if scroll!=0.0 {io.add_mouse_wheel_event([0.0,scroll.clamp(-1.0,1.0)*8.0*io.delta_time.clamp(0.0,0.05)]);}
    }
}

#[cfg(test)]
pub fn test_map_pad(io:&mut Io,point:[f32;2],down:bool) {
    MAP_POINTER.with(|state|state.borrow_mut().position=Some(point));
    feed_pad(io,&Gamepad{buttons:if down {0x1000}else{0},..Default::default()},true,true);
}
#[cfg(test)]
pub fn test_pad_frames(context:&mut hudhook::imgui::Context) {
    for key in [Key::GamepadDpadRight,Key::GamepadFaceDown,Key::GamepadFaceLeft,Key::GamepadFaceUp,Key::GamepadFaceRight] {
        let mask=match key {Key::GamepadDpadRight=>8,Key::GamepadFaceDown=>0x1000,Key::GamepadFaceLeft=>0x4000,Key::GamepadFaceUp=>0x8000,_=>0x2000};
        feed_pad(context.io_mut(),&Gamepad::default(),false,false);
        context.frame();context.render();
        feed_pad(context.io_mut(),&Gamepad{buttons:mask,..Default::default()},true,false);
        assert!(context.frame().is_key_pressed_no_repeat(key),"Controller key lost during ImGui NewFrame: {key:?}");
        context.render();
        feed_pad(context.io_mut(),&Gamepad{buttons:mask,..Default::default()},true,false);
        assert!(!context.frame().is_key_pressed_no_repeat(key),"Held button repeated unexpectedly");context.render();
        feed_pad(context.io_mut(),&Gamepad::default(),false,false);
        assert!(!context.frame().is_key_down(key),"Disconnected controller left a stuck button");context.render();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn special_rewards_have_their_own_card_icons() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scene-assets/reward-icons");
        for index in 0..5 {assert_eq!(std::fs::metadata(root.join(format!("{}.rgba",super::EXTRA_ICONS+index))).map(|m|m.len()).unwrap_or(0),160*160*4);}
    }
    #[test]
    fn talisman_variants_have_chinese_details_icons_and_matching_rarity() {
        for item in erdtree_tower::talismans::CATALOG.iter() {
            let d=&DETAILS[&item.id];assert_eq!(d.name,item.name);assert!(!d.info.is_empty()&&!d.caption.is_empty());
            assert_eq!(item_quality(item.id),erdtree_tower::talismans::tier(item.id).unwrap());
            assert_eq!(std::fs::metadata(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scene-assets/reward-icons").join(format!("{}.rgba",d.icon))).unwrap().len(),102400);
        }
    }
    #[test]
    fn no_detail_shows_the_dlc_dummy_placeholder() {
        assert!(DETAILS.values().all(|d|!d.skill.contains("DLC dummy")&&!d.info.contains("DLC dummy")));
    }
    #[test]
    fn fixed_skill_crossbows_keep_their_real_description() {
        let run=Run::new(42,erdtree_tower::Class::Samurai);
        for id in [43000000,44000000] {
            assert!(!crate::random_ashes::eligible(id));
            let detail=offer_detail(&run,3,0,&Reward::Item(id)).unwrap();
            assert!(!detail.skill.is_empty());assert!(!detail.skill.contains("获得时独立随机"));
            assert_eq!(detail.skill,DETAILS[&id].skill);
        }
    }
    #[test]
    fn card_and_details_show_the_same_gem_used_by_delivery() {
        let run=Run::new(42,erdtree_tower::Class::Samurai);
        for (node,slot,id) in [(3,0,1000000),(3,1,1000000),(usize::MAX,2,33000000)] {
            let d=offer_detail(&run,node,slot,&Reward::Item(id)).unwrap();
            let gem=crate::random_ashes::preview(&run,node,slot,id);
            let (name,description)=crate::random_ashes::description(gem).unwrap();
            assert_eq!(d.skill,format!("{name}\n{}",description.replace("\\n","\n")));
            assert!(card_summary(&d).contains(name));
        }
    }
    #[test]
    fn equipment_palette_is_distinct_opaque_and_matches_native_hex_colors() {
        let colors:Vec<_>=(0..4).map(super::rarity_color).collect();
        assert!(colors.iter().all(|c|c[3]==1.0&&c.iter().all(|v|(0.0..=1.0).contains(v))));
        for i in 0..4 {for j in i+1..4 {assert_ne!(colors[i],colors[j]);}}
        assert_eq!(super::rarity_color(99),colors[3]);
        assert!(colors[1][2]>colors[1][0]);
        assert!(colors[2][0]>colors[2][1]&&colors[2][2]>colors[2][1]);
        assert!(colors[3][0]>colors[3][2]);
    }
    use super::*;
    #[test]
    fn map_confirm_requires_new_press_and_does_not_carry_from_rewards() {
        let mut state=MapConfirm::default();
        state.update(false,true);state.update(true,true);assert!(!state.pressed);
        state.update(true,false);state.update(true,true);assert!(state.pressed);
        state.update(true,true);assert!(!state.pressed);
        state.update(false,false);assert!(!state.pressed);
    }
    #[test]
    fn map_pointer_accumulates_and_stays_put_until_real_mouse_moves() {
        let mut cursor=MapPointer::default();
        let physical=Some([100,100]);let mouse=[100.0,100.0];let size=[1920.0,1080.0];
        let first=cursor.step(true,physical,mouse,size,[1.0,0.0],0.02).unwrap();
        let second=cursor.step(true,physical,mouse,size,[1.0,0.0],0.02).unwrap();
        assert!(second[0]>first[0]);
        for _ in 0..60 {assert_eq!(cursor.step(true,physical,mouse,size,[0.0,0.0],0.02),Some(second));}
        assert_eq!(cursor.step(true,Some([101,100]),mouse,size,[0.0,0.0],0.02),None);
        assert_eq!(cursor.step(true,physical,[1918.0,1079.0],size,[1.0,1.0],0.02),Some([1919.0,1079.0]));
        assert_eq!(cursor.step(false,physical,mouse,size,[0.0,0.0],0.02),None);
        assert!(cursor.position.is_none());
    }
    #[test]
    fn controller_deadzone_and_card_navigation_do_not_conflict_with_map_cursor() {
        for value in [-8000,0,8000] {assert_eq!(stick(value),0.0);}
        assert_eq!(stick(i16::MIN),-1.0);assert_eq!(stick(i16::MAX),1.0);
        let pad=Gamepad{lx:i16::MAX,ly:i16::MIN,..Default::default()};
        assert!(pad_direction(&pad,8,false));assert!(pad_direction(&pad,2,false));
        assert!(!pad_direction(&pad,8,true));assert!(!pad_direction(&pad,0x1000,false));
        let pad=Gamepad{buttons:0x4004,..Default::default()};
        assert!(pad_direction(&pad,4,true));assert!(pad_direction(&pad,0x4000,false));
    }
    #[test]
    fn reward_quality_covers_all_details_and_legendary_animation_is_bounded() {
        assert!(DETAILS.keys().all(|id|RARITIES.contains_key(&erdtree_tower::rewards::spirit_base(*id))));
        let run=Run::new(42,erdtree_tower::Class::Samurai);
        assert_eq!(quality(&run,&Reward::Gold(100)),0);
        assert_eq!(quality(&run,&Reward::StarStream),2);
        let icon=card_icon(&run,&Reward::StarStream).unwrap();
        assert_eq!(icon,DETAILS[&(0x40000000|4200)].icon);
        assert_eq!(item_id(&run,&Reward::StarStream),None);
        assert_eq!(std::fs::metadata(format!("../scene-assets/reward-icons/{icon}.rgba")).unwrap().len(),160*160*4);
        assert_eq!(quality(&run,&Reward::Item(1000000)),1);
        for id in erdtree_tower::elite::EPIC.iter(){assert_eq!(quality(&run,&Reward::Item(*id)),2);}
        for id in erdtree_tower::elite::LEGENDARY {assert_eq!(quality(&run,&Reward::Item(*id)),3);}
        for selected in [false,true] {for tier in 0..4 {
            let lights:Vec<_>=(0..100).map(|i|card_light(tier,selected,false,i as f64/20.0)).collect();
            assert!(lights.iter().all(|v|*v>=135));
            assert_eq!(lights.iter().min()!=lights.iter().max(),tier==3);
        }}
        assert_eq!(std::fs::metadata("../scene-assets/reward-art/rarity-cards.rgba").unwrap().len(),1536*1024*4);
        assert_eq!(std::fs::metadata("../scene-assets/reward-art/rare-blue.rgba").unwrap().len(),887*1774*4);
    }
    #[test]
    fn returning_from_details_preserves_selection_and_debounces_claim() {
        let mut s=Selection::default();s.sync("reward".into(),1.0);s.selected=2;s.details=true;
        s.close_details(2.0);
        assert_eq!(s.selected,2);assert!(!s.details);
        assert_eq!(s.claim(2.0,true),None);assert_eq!(s.claim(2.2,true),None);
        assert_eq!(s.claim(2.26,true),Some(2));
    }
    #[test]
    fn generated_reward_card_has_complete_rgba_pixels() {
        assert_eq!(std::fs::metadata("../scene-assets/reward-art/card.rgba").unwrap().len(),1024*1536*4);
    }
    #[test]
    fn selection_requires_confirmation_and_blocks_entry_click_and_detail_confirmation() {
        let mut s=Selection::default();s.sync("a".into(),1.0);
        assert_eq!(s.claim(1.1,true),None);s.step(-1,3);assert_eq!(s.selected,2);
        s.details=true;assert_eq!(s.claim(2.0,true),None);s.details=false;
        assert_eq!(s.claim(2.0,false),None);assert_eq!(s.claim(2.0,true),Some(2));
        assert_eq!(s.claim(2.1,true),None);s.sync("b".into(),3.0);assert_eq!(s.selected,0);
    }
    #[test]
    fn every_offered_item_and_talisman_has_native_description() {
        for item in erdtree_tower::rewards::CATALOG.iter(){assert!(!DETAILS[&item.id].caption.is_empty());}
        for kind in erdtree_tower::BlessingKind::ALL {for level in 1..=3 {assert!(!DETAILS[&(0x20000000|crate::blessings::talisman(kind,level).0)].caption.is_empty());}}
        let katana=&DETAILS[&9000000];assert_eq!(katana.name,"打刀");assert!(katana.stats.contains("属性要求：无"));assert!(!katana.skill.trim().is_empty());
        for item in erdtree_tower::rewards::CATALOG.iter().filter(|i|i.kind==erdtree_tower::rewards::ItemKind::Spirit) {assert!(DETAILS[&item.id].stats.contains("0 专注值、0 生命值"));}
    }
    #[test]
    fn staff_in_name_does_not_make_a_melee_weapon_a_catalyst() {
        for id in [2210000,11110000,12200000,23010000,23070000,23140000] {
            let summary=card_summary(&DETAILS[&id]);
            assert!(summary.starts_with("战技："),"{}: {summary}",DETAILS[&id].name);
        }
        assert!(card_summary(&DETAILS[&33000000]).contains("施展魔法"));
        assert!(card_summary(&DETAILS[&34000000]).contains("施展祷告"));
    }
}
