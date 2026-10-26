use std::{collections::HashMap,sync::LazyLock};
use erdtree_tower::{Reward,Run,Phase};
use hudhook::{RenderContext,imgui::{Ui,Io,Key,Condition,StyleColor,ImColor32,TextureId}};
use crate::runtime::{self,Action,View};

#[derive(Clone)]
struct Detail {icon:u32,name:String,info:String,caption:String,stats:String,skill:String}
static RARITIES:LazyLock<HashMap<u32,u8>>=LazyLock::new(||include_str!("../../scene-assets/reward-rarities.tsv").lines().chain(include_str!("../../scene-assets/affix-rarities.tsv").lines()).chain(include_str!("../../scene-assets/affix-v2-rarities.tsv").lines()).map(|line| {
    let (id,rarity)=line.split_once('\t').unwrap();(id.parse().unwrap(),rarity.parse().unwrap())
}).collect());
pub fn item_quality(id:u32)->u8 {RARITIES.get(&erdtree_tower::rewards::spirit_base(id)).copied().unwrap_or(0).min(3)}
fn rarity_color(tier:usize)->[f32;4] {
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
    if matches!(reward,Reward::StarStream){return 2;}
    item_id(run,reward).map_or(0,item_quality) as usize
}
fn card_light(tier:usize,selected:bool,hover:bool,time:f64)->u8 {
    let base=if selected{240.0}else if hover{195.0}else{150.0};
    (base+if tier==3 {15.0*(time*std::f64::consts::TAU/3.6).sin()}else{0.0}) as u8
}
static DETAILS:LazyLock<HashMap<u32,Detail>>=LazyLock::new(|| {
    let mut result:HashMap<u32,Detail>=include_str!("../../scene-assets/reward-details.tsv").lines().chain(include_str!("../../scene-assets/affix-details.tsv").lines()).chain(include_str!("../../scene-assets/affix-v2-details.tsv").lines()).map(|line| {
        let p:Vec<_>=line.split('\t').collect();
        (p[0].parse().unwrap(),Detail{icon:p[1].parse().unwrap(),name:p[2].into(),info:p[3].replace("\\n","\n"),caption:p[4].replace("\\n","\n"),stats:p[5].replace("\\n","\n"),skill:p[6].replace("\\n","\n")})
    }).collect();
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
fn card_icon(run:&Run,reward:&Reward)->Option<u32> {
    let id=if matches!(reward,Reward::StarStream){Some(0x40000000|4200)}else{item_id(run,reward)};
    id.and_then(|id|DETAILS.get(&id)).map(|detail|detail.icon)
}
fn card_summary(detail:&Detail)->String {
    if !detail.info.trim().is_empty(){return detail.info.clone();}
    let weight=detail.stats.lines().find(|line|line.starts_with("重量：")).unwrap_or("");
    let skill=detail.skill.lines().next().unwrap_or("");
    if !skill.is_empty() {
        let purpose=match detail.stats.lines().find_map(|line|line.strip_prefix("施法能力：")) {
            Some("魔法与祷告")=>"施展魔法与祷告",Some("魔法")=>"施展魔法",Some("祷告")=>"施展祷告",_=>"",
        };
        if !purpose.is_empty() {
            let effect=detail.caption.split("\n\n").find(|part|part.contains("能强化")||part.contains("能提升")||part.contains("能缩短"));
            return format!("{}\n{}",effect.map_or(purpose.to_owned(),|s|s.replace('\n',"")),weight);
        }
        return format!("战技：{skill}\n{weight}");
    }
    detail.caption.split("\n\n").next().unwrap_or("").replace('\n',"")
}
#[derive(Default)]
struct Selection {key:String,selected:usize,details:bool,skip:bool,armed_at:f64,last_submit:f64}
impl Selection {
    fn close_details(&mut self,now:f64) {self.details=false;self.armed_at=now+0.25;}
    fn sync(&mut self,key:String,now:f64) {
        if self.key!=key {*self=Self{key,armed_at:now+0.35,last_submit:f64::NEG_INFINITY,..Default::default()};}
    }
    fn step(&mut self,delta:isize,count:usize) {if count>0 {self.selected=(self.selected as isize+delta).rem_euclid(count as isize) as usize;}}
    fn claim(&mut self,now:f64,ready:bool)->Option<usize> {
        if !ready||self.details||self.skip||now<self.armed_at||now-self.last_submit<0.5 {return None;}
        self.last_submit=now;Some(self.selected)
    }
}
#[derive(Default)]
pub struct Panel {selection:Selection,textures:HashMap<u32,Option<TextureId>>,wanted:Vec<u32>,card:Option<TextureId>,rarity_cards:Option<TextureId>,rare_blue:Option<TextureId>,story:Option<(Run,u32,[Option<String>;3])>}
impl Panel {
    fn icon(&mut self,icon:u32)->Option<TextureId> {
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
        self.selection.sync(format!("contract:{}:{}:{:?}",run.seed,run.act,contract.offers),now);
        let ready=view.ready&&!view.busy&&now>=self.selection.armed_at;
        let key=|keys:&[Key]|keys.iter().any(|k|ui.is_key_pressed_no_repeat(*k));
        if ready {
            if key(&[Key::A,Key::LeftArrow,Key::GamepadDpadLeft]){self.selection.step(-1,2);}
            if key(&[Key::D,Key::RightArrow,Key::GamepadDpadRight]){self.selection.step(1,2);}
        }
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
            ui.set_window_font_scale(s*0.9);center(ui,mid,oy+127.0*s,"二选一 · 本幕生效",[0.63,0.58,0.46,1.0]);
            let width=366.0*s;let gap=72.0*s;let top=oy+187.0*s;let height=549.0*s;
            for (i,kind) in contract.offers.iter().enumerate() {
                let x=mid-width-gap*0.5+i as f32*(width+gap);
                ui.set_cursor_pos([x,top-7.0*s]);
                {let _disabled=ui.begin_disabled(!ready);if ui.invisible_button(format!("contract-{i}"),[width,height+7.0*s]){self.selection.selected=i;}}
                let hovered=ui.is_item_hovered();let selected=self.selection.selected==i;
                let y=top-if selected{7.0*s}else if hovered{3.0*s}else{0.0};
                crate::oath_art::card(&ui.get_window_draw_list(),kind.index(),[x,y],[x+width,y+height],if selected{255}else if hovered{225}else{165});
                ui.set_window_font_scale(s*1.35);center(ui,x+width*0.5,y+333.0*s,kind.name(),gold);
                ui.set_window_font_scale(s*0.95);
                for (offset,text,color) in [(401.0,kind.boon(),[0.64,0.83,0.58,1.0]),(462.0,kind.bane(),[0.91,0.49,0.40,1.0])] {
                    center(ui,x+width*0.5,y+offset*s,text,color);
                }
            }
            let min=[mid-210.0*s,oy+786.0*s];let max=[mid+210.0*s,oy+852.0*s];
            ui.set_cursor_pos(min);
            let confirm={let _disabled=ui.begin_disabled(!ready);ui.invisible_button("confirm-oath",[420.0*s,66.0*s])};
            let hovered=ui.is_item_hovered();
            crate::oath_art::button(&ui.get_window_draw_list(),min,max,if !ready{130}else if hovered{255}else{220});
            ui.set_window_font_scale(s);center(ui,mid,oy+808.0*s,"确认誓约   Enter / A",gold);
            if confirm||key(&[Key::Enter,Key::GamepadFaceDown]) {if let Some(index)=self.selection.claim(now,ready){runtime::submit(Action::Contract(index));}}
            ui.set_cursor_pos([32.0*s,oy+844.0*s]);
            let skip={let _disabled=ui.begin_disabled(!ready);ui.button("跳过誓约   Esc / B")};
            if (skip||key(&[Key::Escape,Key::GamepadFaceRight]))&&self.selection.claim(now,ready).is_some() {runtime::submit(Action::SkipContract);}
            if !crate::oath_art::ready(){ui.set_window_font_scale(s*0.8);center(ui,mid,oy+866.0*s,"誓约贴图缺失，请重启更新后的启动器",[0.91,0.49,0.40,1.0]);}
        });
    }
    pub fn draw_story(&mut self,ui:&Ui,run:&Run,view:&View) {
        let Ok(index)=run.story_index() else{return;};
        let story=&erdtree_tower::events::STORIES[index];
        let now=ui.time();
        self.selection.sync(format!("event:{}:{}:{:?}",run.seed,run.act,run.phase),now);
        let ready=view.ready&&!view.busy&&now>=self.selection.armed_at;
        let key=|keys:&[Key]|keys.iter().any(|k|ui.is_key_pressed_no_repeat(*k));
        if ready {
            if key(&[Key::A,Key::LeftArrow,Key::GamepadDpadLeft]){self.selection.step(-1,3);}
            if key(&[Key::D,Key::RightArrow,Key::GamepadDpadRight]){self.selection.step(1,3);}
        }
        if !self.story.as_ref().is_some_and(|(cached,runes,_)|*runes==view.runes&&cached==run) {
            let errors=std::array::from_fn(|i|{let mut preview=run.clone();preview.gold=view.runes;preview.resolve_story(i).err()});
            self.story=Some((run.clone(),view.runes,errors));
        }
        let errors=&self.story.as_ref().unwrap().2;
        let size=ui.io().display_size;
        let s=(size[0]/1600.0).min(size[1]/900.0).min(1.7);
        let gold=[0.86,0.72,0.43,1.0];let ink=[0.18,0.15,0.10,1.0];
        let _bg=ui.push_style_color(StyleColor::WindowBg,[0.025,0.03,0.025,0.97]);
        let _text=ui.push_style_color(StyleColor::Text,gold);
        let _button=ui.push_style_color(StyleColor::Button,[0.25,0.20,0.12,1.0]);
        let _hover=ui.push_style_color(StyleColor::ButtonHovered,[0.40,0.30,0.16,1.0]);
        ui.window("旅途奇遇##story").position([0.0,0.0],Condition::Always).size(size,Condition::Always)
            .title_bar(false).resizable(false).movable(false).collapsible(false).scroll_bar(false).build(|| {
            let mid=size[0]*0.5;
            ui.set_window_font_scale(s);
            center(ui,mid,32.0*s,"黄 金 树 爬 塔  ·  旅 途 奇 遇",gold);
            ui.set_window_font_scale(s*1.8);center(ui,mid,77.0*s,story.title,gold);
            ui.set_window_font_scale(s);
            ui.set_cursor_pos([mid-490.0*s,142.0*s]);
            {let _wrap=ui.push_text_wrap_pos_with_pos(mid+490.0*s);ui.text_wrapped(story.text);}
            center(ui,mid,224.0*s,&format!("持有卢恩  {}　·　每个事件只能选择一项",view.runes),gold);
            let w=330.0*s;let h=370.0*s;let gap=34.0*s;let top=278.0*s;
            let labels=match index {
                0=>["献出鲜血","献出力量","离开祭坛"],
                1=>["祈求生命","祈求耐力","告别旅人"],
                2=>["捧起红泉","捧起蓝泉","离开泉水"],
                3=>["收起钱袋","带走补给","留给后来者"],
                4=>["研读魔法","追随火焰","合上禁书"],
                _=>["帮助铁匠","委托锻造","继续前行"],
            };
            for i in 0..3 {
                let x=mid-(3.0*w+2.0*gap)*0.5+i as f32*(w+gap);
                ui.set_cursor_pos([x,top]);
                {let _disabled=ui.begin_disabled(!ready);if ui.invisible_button(format!("story-card-{i}"),[w,h]){self.selection.selected=i;}}
                let hovered=ui.is_item_hovered();let selected=self.selection.selected==i;
                let draw=ui.get_window_draw_list();
                draw.add_rect([x,top],[x+w,top+h],ImColor32::from_rgb(218,204,170)).filled(true).build();
                for inset in [0.0,7.0*s] {draw.add_rect([x+inset,top+inset],[x+w-inset,top+h-inset],if selected||hovered{ImColor32::from_rgb(239,191,90)}else{ImColor32::from_rgb(117,97,60)}).thickness(if selected{3.0*s}else{1.0*s}).build();}
                draw.add_line([x+26.0*s,top+110.0*s],[x+w-26.0*s,top+110.0*s],ImColor32::from_rgb(155,127,78)).build();
                drop(draw);
                center(ui,x+w*0.5,top+24.0*s,&format!("0{}",i+1),ink);
                ui.set_window_font_scale(s*1.3);center(ui,x+w*0.5,top+63.0*s,labels[i],ink);ui.set_window_font_scale(s);
                let mut description=if i==2{"不支付卢恩，不损失生命。\n放弃本次机遇，继续旅程。".to_string()}else{story.choices[i].to_string()};
                if let Some(kind)=match (index,i) {
                    (0,0)=>Some(erdtree_tower::BlessingKind::Bleed),(0,1)=>Some(erdtree_tower::BlessingKind::Charge),
                    (1,0)=>Some(erdtree_tower::BlessingKind::Vigor),(1,1)=>Some(erdtree_tower::BlessingKind::Endurance),
                    (4,0)=>Some(erdtree_tower::BlessingKind::Sorcery),(4,1)=>Some(erdtree_tower::BlessingKind::Flame),_=>None,
                } {
                    let level=run.blessings.iter().find(|b|b.kind==kind).map_or(1,|b|(b.level+1).min(3));
                    description.push_str(&format!("\n\n实际获得：{}\n自行装备后生效",crate::blessings::talisman(kind,level).1));
                }
                ui.set_cursor_pos([x+26.0*s,top+137.0*s]);
                {let _color=ui.push_style_color(StyleColor::Text,ink);let _wrap=ui.push_text_wrap_pos_with_pos(x+w-26.0*s);ui.text_wrapped(description);}
                center(ui,x+w*0.5,top+h-43.0*s,if errors[i].is_some(){"条件不足"}else if selected{"已选中 · 等待确认"}else{"点击选择"},ink);
            }
            ui.set_cursor_pos([mid-190.0*s,688.0*s]);
            let available=ready&&errors[self.selection.selected].is_none();
            let confirm={let _disabled=ui.begin_disabled(!available);ui.button_with_size("确认选择  [Enter / A]",[380.0*s,48.0*s])};
            if confirm||key(&[Key::Enter,Key::GamepadFaceDown]) {if let Some(choice)=self.selection.claim(now,available){runtime::submit(Action::Story(choice));}}
            center(ui,mid,759.0*s,"A / D 或方向键切换　·　选中后确认　·　离开不消耗资源",gold);
            let status=errors[self.selection.selected].as_deref().unwrap_or(&view.status);
            ui.set_cursor_pos([mid-500.0*s,812.0*s]);let _wrap=ui.push_text_wrap_pos_with_pos(mid+500.0*s);ui.text_wrapped(status);
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
        let gift=run.contract.as_ref().filter(|c|c.chosen.is_some()&&!c.gear_done);
        let gift_offers:Vec<_>=gift.map(|c|c.gear.iter().copied().map(Reward::Item).collect()).unwrap_or_default();
        let (id,offers)=if gift.is_some(){(usize::MAX,&gift_offers)}else if let Phase::Reward{id,offers}=&run.phase{(*id,offers)}else{return;};
        if offers.is_empty(){return;}
        let now=ui.time();
        self.selection.sync(format!("{}:{id}:{offers:?}",run.seed),now);
        if let Some(receipt)=gift.and_then(|c|c.gift.as_ref()){self.selection.selected=receipt.index;}
        let ready=view.ready&&!view.busy&&now>=self.selection.armed_at;
        let key=|keys:&[Key]|keys.iter().any(|k|ui.is_key_pressed_no_repeat(*k));
        let was_details=self.selection.details;
        if ready&&!self.selection.details&&!self.selection.skip {
            if key(&[Key::A,Key::LeftArrow,Key::GamepadDpadLeft]){self.selection.step(-1,offers.len());}
            if key(&[Key::D,Key::RightArrow,Key::GamepadDpadRight]){self.selection.step(1,offers.len());}
            if key(&[Key::Tab,Key::GamepadFaceUp]){self.selection.details=true;}
        }
        if key(&[Key::Escape,Key::GamepadFaceRight]){if self.selection.details {self.selection.close_details(now);}self.selection.skip=false;}
        if ready&&!self.selection.details&&!self.selection.skip&&key(&[Key::GamepadFaceLeft]) {
            self.selection.skip=true;self.selection.armed_at=now+0.25;
        }
        let size=ui.io().display_size;let scale=(size[0]/1600.0).min(size[1]/900.0);
        let gold=[0.91,0.79,0.53,1.0];let ink=[0.88,0.85,0.76,1.0];
        let _bg=ui.push_style_color(StyleColor::WindowBg,[0.025,0.03,0.025,0.88]);
        let _text=ui.push_style_color(StyleColor::Text,gold);
        let _button=ui.push_style_color(StyleColor::Button,[0.16,0.14,0.10,1.0]);
        let _hover=ui.push_style_color(StyleColor::ButtonHovered,[0.32,0.26,0.15,1.0]);
        ui.window("战利品##reward").position([0.0,0.0],Condition::Always).size(size,Condition::Always)
            .title_bar(false).resizable(false).movable(false).collapsible(false).scroll_bar(false).build(|| {
            ui.set_window_font_scale(scale);
            crate::oath_art::background(&ui.get_window_draw_list(),size);
            if self.selection.details {
                let reward=&offers[self.selection.selected];
                let d=item_id(run,reward).and_then(|id|DETAILS.get(&id));
                let left=(size[0]-1360.0*scale)*0.5;
                let top=(size[1]-900.0*scale)*0.5;
                ui.set_window_font_scale(scale*1.45);
                ui.set_cursor_pos([left,top+48.0*scale]);
                let text_color=rarity_color(quality(run,reward));
                ui.text_colored(text_color,d.map_or("奖励详情",|d|d.name.as_str()));
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
                        let _wrap=ui.push_text_wrap_pos_with_pos(516.0*scale);
                        ui.text_colored(gold,"属性");ui.spacing();
                        let _text=ui.push_style_color(StyleColor::Text,ink);
                        ui.text_wrapped(if d.stats.trim().is_empty(){"—"}else{&d.stats});
                        ui.unindent();
                    } else {ui.text_wrapped(crate::map_ui::reward_description(run,reward));}
                });
                ui.set_cursor_pos([left+580.0*scale,top+115.0*scale]);
                ui.child_window("物品说明").size([780.0*scale,640.0*scale]).build(|| {
                    ui.set_window_font_scale(scale);
                    if key(&[Key::UpArrow,Key::GamepadDpadUp]){ui.set_scroll_y((ui.scroll_y()-80.0*scale).max(0.0));}
                    if key(&[Key::DownArrow,Key::GamepadDpadDown]){ui.set_scroll_y(ui.scroll_y()+80.0*scale);}
                    ui.set_cursor_pos([24.0*scale,20.0*scale]);
                    let _indent=ui.push_style_var(hudhook::imgui::StyleVar::IndentSpacing(24.0*scale));
                    ui.indent();
                    let _wrap=ui.push_text_wrap_pos_with_pos(750.0*scale);
                    let _text=ui.push_style_color(StyleColor::Text,ink);
                    if let Some(d)=d {
                        if !d.info.trim().is_empty(){ui.text_colored(gold,"效果");ui.spacing();{let _color=ui.push_style_color(StyleColor::Text,text_color);ui.text_wrapped(&d.info);}ui.spacing();ui.separator();ui.spacing();}
                        if matches!(reward,Reward::TalismanBuff(_)){ui.text_wrapped("本局生效，不占护符栏位");ui.spacing();}
                        ui.text_colored(gold,"物品说明");ui.spacing();
                        if item_id(run,reward).is_some_and(|id|erdtree_tower::affixes::tier(id).is_some()) {
                            if let Some((lore,affix))=d.caption.rsplit_once("\n\n") {
                                ui.text_wrapped(lore);ui.spacing();let _color=ui.push_style_color(StyleColor::Text,text_color);ui.text_wrapped(affix);
                            } else {let _color=ui.push_style_color(StyleColor::Text,text_color);ui.text_wrapped(&d.caption);}
                        } else {ui.text_wrapped(&d.caption);}
                        if !d.skill.trim().is_empty(){ui.spacing();ui.separator();ui.spacing();ui.text_colored(gold,"战技");ui.spacing();ui.text_wrapped(&d.skill);}
                    } else {ui.text_wrapped(crate::map_ui::reward_description(run,reward));}
                    ui.unindent();
                });
                if textured_button(ui,"返回  [Esc / B]",[left,top+795.0*scale],[300.0*scale,52.0*scale],gold){self.selection.close_details(now);}
                ui.set_cursor_pos([left+580.0*scale,top+811.0*scale]);ui.text_colored(gold,"滚轮 / 上下键  滚动说明");
                return;
            }
            let width=330.0*scale;let gap=36.0*scale;let height=490.0*scale;
            let left=(size[0]-(width+gap)*offers.len() as f32+gap)*0.5;let top=178.0*scale;
            for (i,reward) in offers.iter().enumerate() {
                let x=left+i as f32*(width+gap);
                let selected=self.selection.selected==i;
                let hover=ready&&!self.selection.details&&!self.selection.skip&&ui.is_mouse_hovering_rect([x,top],[x+width,top+height]);
                let y=top-if selected{7.0*scale}else if hover{3.0*scale}else{0.0};
                let detail=item_id(run,reward).and_then(|id|DETAILS.get(&id));
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
                    Reward::FlaskCapacity=>("增益","圣杯扩容".into(),"红、蓝瓶上限各 +1\n本局生效，不补瓶".into()),
                    Reward::MemorySlot=>("增益","记忆扩容".into(),"记忆栏位 +1\n魔法与祷告共用，本局生效".into()),
                    Reward::StarStream=>("增益","星流引导".into(),"彗星亚兹勒随视角左右瞄准\n光束保持水平".into()),
                };
                let tier=quality(run,reward);
                let quality_color=rarity_color(tier);
                if let Some(texture)=if tier==1 {self.rare_blue}else{None} {
                    let light=card_light(tier,selected,hover,now);
                    ui.get_window_draw_list().add_image(texture,[x,y],[x+width,y+height])
                        .col(ImColor32::from_rgba(light,light,light,255)).build();
                } else if let Some(texture)=self.rarity_cards {
                    let cell=tier.saturating_sub(1) as f32;
                    let light=card_light(tier,selected,hover,now+i as f64*0.4);
                    ui.get_window_draw_list().add_image(texture,[x,y],[x+width,y+height])
                        .uv_min([(cell+0.002)/3.0,0.0]).uv_max([(cell+0.998)/3.0,1.0])
                        .col(ImColor32::from_rgba(light,light,light,255)).build();
                } else if let Some(texture)=self.card {
                    let light=if selected{255}else if hover{225}else{172};
                    ui.get_window_draw_list().add_image(texture,[x,y],[x+width,y+height])
                        .col(ImColor32::from_rgba(light,light,light,255)).build();
                }
                if selected {
                    let draw=ui.get_window_draw_list();
                    for (offset,alpha,thickness) in [(3.0,35,9.0),(2.0,80,4.0),(1.0,220,1.5)] {
                        draw.add_rect([x-offset*scale,y-offset*scale],[x+width+offset*scale,y+height+offset*scale],ImColor32::from_rgba(225,204,143,alpha)).rounding(2.0*scale).thickness(thickness*scale).build();
                    }
                }
                center(ui,x+width*0.5,y+61.0*scale,category,quality_color);
                if let Some(texture)=card_icon(run,reward).and_then(|icon|self.icon(icon)) {
                    let side=175.0*scale;ui.get_window_draw_list().add_image(texture,[x+(width-side)*0.5,y+91.0*scale],[x+(width+side)*0.5,y+266.0*scale]).build();
                } else {
                    center(ui,x+width*0.5,y+145.0*scale,match reward {Reward::UpgradeWeapon=>"强化",Reward::FlaskCapacity=>"圣杯",Reward::MemorySlot=>"记忆",Reward::StarStream=>"星流",_=>"卢恩"},gold);
                }
                ui.set_window_font_scale(scale*1.2);
                let title_width=ui.calc_text_size(&title)[0];
                if title_width>width-40.0*scale {ui.set_window_font_scale(scale*1.2*(width-40.0*scale)/title_width);}
                center(ui,x+width*0.5,y+278.0*scale,&title,quality_color);
                ui.set_window_font_scale(scale*0.90);
                let summary=if summary.chars().count()>48 {format!("{}…",summary.chars().take(48).collect::<String>())}else{summary};
                ui.set_cursor_pos([x+40.0*scale,y+329.0*scale]);
                {let _color=ui.push_style_color(StyleColor::Text,quality_color);let _wrap=ui.push_text_wrap_pos_with_pos(x+width-40.0*scale);ui.text_wrapped(summary);}
                ui.set_window_font_scale(scale);
                if selected {center(ui,x+width*0.5,y+height-76.0*scale,"已选",gold);}
                ui.set_cursor_pos([x,y]);let _disabled=ui.begin_disabled(!ready||self.selection.details||self.selection.skip);
                if ui.invisible_button(format!("reward-card-{i}"),[width,height]){self.selection.selected=i;}
            }
            let buttons_y=top+height+25.0*scale;
            ui.set_cursor_pos([size[0]*0.5-255.0*scale,buttons_y]);
            let mut confirm=false;
            {let _disabled=ui.begin_disabled(!ready||self.selection.details||self.selection.skip);
                confirm|=textured_button(ui,"领取  [Enter / A]",[size[0]*0.5-255.0*scale,buttons_y],[245.0*scale,48.0*scale],gold);
                if textured_button(ui,"详情  [Tab / Y]",[size[0]*0.5+10.0*scale,buttons_y],[245.0*scale,48.0*scale],gold){self.selection.details=true;}
            }
            if !was_details&&!self.selection.details&&!self.selection.skip {confirm|=key(&[Key::Enter,Key::GamepadFaceDown]);}
            if confirm {if let Some(index)=self.selection.claim(now,ready){runtime::submit(if gift.is_some(){Action::Gear(index)}else{Action::Claim(index)});}}
            center(ui,size[0]*0.5,buttons_y+65.0*scale,"左 / 右  切换",gold);
            ui.set_cursor_pos([24.0*scale,size[1]-45.0*scale]);
            {let _disabled=ui.begin_disabled(!ready||self.selection.details||self.selection.skip);
                if ui.small_button("跳过奖励 [X]"){self.selection.skip=true;self.selection.armed_at=now+0.25;}
            }
            if self.selection.skip {
                ui.set_cursor_pos([size[0]*0.5-220.0*scale,size[1]*0.4]);
                let _child=ui.push_style_color(StyleColor::ChildBg,[0.09,0.095,0.08,1.0]);
                ui.child_window("确认放弃").size([440.0*scale,150.0*scale]).border(true).build(||{
                    ui.text_wrapped("放弃本次三选一奖励并继续旅程？");
                    if (ui.button("确认放弃 [A]")||key(&[Key::GamepadFaceDown]))&&ready&&now>=self.selection.armed_at {runtime::submit(if gift.is_some(){Action::SkipGear}else{Action::KeepCurrency});self.selection.skip=false;}
                    ui.same_line();if ui.button("返回选择 [B]"){self.selection.skip=false;}
                });
            }
        });
    }
}
fn textured_button(ui:&Ui,label:&str,pos:[f32;2],size:[f32;2],color:[f32;4])->bool {
    ui.set_cursor_pos(pos);
    let clicked=ui.invisible_button(label,size);
    crate::oath_art::button(&ui.get_window_draw_list(),pos,[pos[0]+size[0],pos[1]+size[1]],if ui.is_item_hovered(){255}else{205});
    center(ui,pos[0]+size[0]*0.5,pos[1]+(size[1]-ui.text_line_height())*0.5,label,color);
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
        for seed in 0..24 {
            let mut event=Run::new(seed,erdtree_tower::Class::Samurai);
            event.nodes[0].kind=erdtree_tower::NodeKind::Event;event.phase=Phase::Node{id:0};
            panel.draw_story(context.frame(),&event,&view);
            assert!(context.render().total_vtx_count>0);
        }
    }
    for detail in DETAILS.values(){panel.textures.insert(detail.icon,Some(TextureId::new(detail.icon as usize+1)));}
    if let Some(path)=&output {
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
    MAP_CONFIRM.with(|state|state.borrow_mut().update(found&&map,found&&pad.buttons&0x1000!=0));
    io.backend_flags.set(hudhook::imgui::BackendFlags::HAS_GAMEPAD,found);
    for (key,mask) in [(Key::GamepadDpadUp,1),(Key::GamepadDpadDown,2),(Key::GamepadDpadLeft,4),(Key::GamepadDpadRight,8),(Key::GamepadFaceDown,0x1000),(Key::GamepadFaceRight,0x2000),(Key::GamepadFaceLeft,0x4000),(Key::GamepadFaceUp,0x8000)] {
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
