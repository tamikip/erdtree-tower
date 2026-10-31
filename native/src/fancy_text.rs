// 动态文字特效：把文字拆成逐个字形来画，每个字单独计算颜色与位置，做出流光、渐变、呼吸发光、波浪、抖动、残影等“着色器风格”的效果。
// ImGui 的渲染管线是固定的，不能插入自定义着色器；这里只用绘制列表叠加多层字形实现，开销是每个字多画十来次。
use hudhook::imgui::sys;

/// 界面字表（scene-builder/title_font.py 生成的 scene-assets/fonts/ui-glyphs.txt）转成 ImGui 的字形范围：
/// 正文黑体与标题宋体都只加载这些字，字体贴图比加载全部汉字小得多。缺文件时返回 None，由调用方退回原来的范围
pub fn ui_glyph_ranges()->Option<&'static [u32]> {
    static RANGES:std::sync::LazyLock<Option<Vec<u32>>>=std::sync::LazyLock::new(|| {
        let text=std::fs::read_to_string(crate::LOG_PATH.with_file_name("scene-assets").join("fonts").join("ui-glyphs.txt")).ok()?;
        Some(ranges(&text))
    });
    RANGES.as_deref()
}
/// 字符集合并成连续区间 [起, 止, 起, 止, …, 0]；总是包含可见 ASCII
fn ranges(text:&str)->Vec<u32> {
    let mut codes:Vec<u32>=text.chars().map(u32::from).filter(|c|*c>=0x80).chain(0x20..0x7f).collect();codes.sort_unstable();codes.dedup();
    let mut out=Vec::new();
    for code in codes {
        match out.last_mut() {Some(end) if *end+1==code=>*end=code,_=>{out.push(code);out.push(code);}}
    }
    out.push(0);out
}
/// 标题字体文件（思源宋体粗体子集）
pub fn title_font_path()->std::path::PathBuf {crate::LOG_PATH.with_file_name("scene-assets").join("fonts").join("TowerTitleSerif-Bold.otf")}
/// 正文字体文件（思源宋体常规体子集）
pub fn body_font_path()->std::path::PathBuf {crate::LOG_PATH.with_file_name("scene-assets").join("fonts").join("TowerSerif-Regular.otf")}
/// 标题字体的 ImGui 句柄：启动时加载后登记在这里，各界面画标题时推入。
/// 它指向字体图集里的字体，图集在整个渲染期间不会释放，且只在渲染线程上使用
#[derive(Clone,Copy)]
pub struct TitleFont(pub hudhook::imgui::FontId);
unsafe impl Send for TitleFont {}
unsafe impl Sync for TitleFont {}
static TITLE_FONT:std::sync::Mutex<Option<TitleFont>>=std::sync::Mutex::new(None);
pub fn set_title_font(font:TitleFont) {if let Ok(mut slot)=TITLE_FONT.lock(){*slot=Some(font);}}
pub fn title_font()->Option<hudhook::imgui::FontId> {TITLE_FONT.lock().ok().and_then(|f|*f).map(|f|f.0)}
/// 结算页黑体：思源黑体粗体的子集（scene-builder/button_font.py），结算页的文字、按钮与大号数字用
pub fn button_font_path()->std::path::PathBuf {crate::LOG_PATH.with_file_name("scene-assets").join("fonts").join("TowerSans-Bold.otf")}
/// 正文字号的黑体（26 像素）
pub const SANS_PIXELS:f32=26.0;
static BUTTON_FONT:std::sync::Mutex<Option<TitleFont>>=std::sync::Mutex::new(None);
pub fn set_button_font(font:TitleFont) {if let Ok(mut slot)=BUTTON_FONT.lock(){*slot=Some(font);}}
pub fn button_font()->Option<hudhook::imgui::FontId> {BUTTON_FONT.lock().ok().and_then(|f|*f).map(|f|f.0)}
/// 大号数字与评级字母（同一份黑体按 220 像素只加载这几个字）
pub const DISPLAY_CHARS:[u32;15]=[0x30,0x39,0x41,0x44,0x53,0x53,0x2B,0x2B,0x2E,0x2E,0xD7,0xD7,0x20,0x20,0];
static DISPLAY_FONT:std::sync::Mutex<Option<TitleFont>>=std::sync::Mutex::new(None);
pub fn set_display_font(font:TitleFont) {if let Ok(mut slot)=DISPLAY_FONT.lock(){*slot=Some(font);}}
pub fn display_font()->Option<hudhook::imgui::FontId> {DISPLAY_FONT.lock().ok().and_then(|f|*f).map(|f|f.0)}
/// 大标题黑体：同一份思源黑体子集按 48 像素只加载 ASCII 与下面这些字（苦难律法页的标题、分类名、按钮），大字不放大位图
pub const HEADING_PIXELS:f32=48.0;
pub const HEADING_CHARS:&str="苦难律法已选项敌人我方经济规则开始旅程";
static HEADING_FONT:std::sync::Mutex<Option<TitleFont>>=std::sync::Mutex::new(None);
pub fn set_heading_font(font:TitleFont) {if let Ok(mut slot)=HEADING_FONT.lock(){*slot=Some(font);}}
pub fn heading_font()->Option<hudhook::imgui::FontId> {HEADING_FONT.lock().ok().and_then(|f|*f).map(|f|f.0)}
/// 大标题黑体的字形范围（ASCII 加 HEADING_CHARS）
pub fn heading_ranges()->&'static [u32] {
    static RANGES:std::sync::LazyLock<Vec<u32>>=std::sync::LazyLock::new(||ranges(HEADING_CHARS));
    &RANGES
}
/// 大标题黑体已加载，且这段文字的每个字都在里面
pub fn heading_covers(text:&str)->bool {heading_font().is_some()&&text.chars().all(|c|(' '..='~').contains(&c)||HEADING_CHARS.contains(c))}
/// 黑体子集里有的字（ui-sans-glyphs.txt；ASCII 都有）
static SANS_GLYPHS:std::sync::LazyLock<std::collections::HashSet<char>>=std::sync::LazyLock::new(||
    std::fs::read_to_string(button_font_path().with_file_name("ui-sans-glyphs.txt")).unwrap_or_default().chars().collect());
/// 黑体已加载，且这段文字的每个字黑体里都有（否则用宋体画，免得出现问号）
pub fn sans_covers(text:&str)->bool {button_font().is_some()&&text.chars().all(|c|c.is_ascii()||c=='\u{3000}'||SANS_GLYPHS.contains(&c))}
#[derive(Clone,Copy,Debug,PartialEq)]
pub enum Effect {
    /// 不加动态效果，只有描边（与特效文字并排时保持一致的外观）
    Plain,
    /// 一道高光从左到右扫过
    Shimmer,
    /// 颜色在主色与辅色之间沿文字流动
    Gradient,
    /// 整体亮度呼吸起伏
    Pulse,
    /// 每个字上下波动
    Wave,
    /// 每个字轻微不规则抖动
    Jitter,
    /// 身后拖着两道残影
    Afterimage,
    /// 每个字明暗不定地闪烁
    Flicker,
}
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Style {pub color:[u8;3],pub accent:[u8;3],pub effect:Effect,pub glow:bool}
/// 画到哪一层：前景（全屏 HUD）或当前窗口
#[derive(Clone,Copy)]
pub enum Layer {Foreground,Window}
fn list(layer:Layer)->*mut sys::ImDrawList {unsafe {match layer {Layer::Foreground=>sys::igGetForegroundDrawList(),Layer::Window=>sys::igGetWindowDrawList()}}}
fn rgba(c:[f32;3],a:f32)->u32 {
    let ch=|v:f32|(v.clamp(0.0,1.0)*255.0) as u32;
    ch(c[0])|ch(c[1])<<8|ch(c[2])<<16|ch(a)<<24
}
fn mix(a:[u8;3],b:[u8;3],t:f32)->[f32;3] {
    let t=t.clamp(0.0,1.0);[0,1,2].map(|i|(f32::from(a[i])*(1.0-t)+f32::from(b[i])*t)/255.0)
}
fn scale(c:[f32;3],k:f32)->[f32;3] {c.map(|v|v*k)}
/// 0..1 的确定性随机数
fn noise(i:u32,k:u32)->f32 {
    let mut x=i.wrapping_mul(0x9E37_79B1)^k.wrapping_mul(0x85EB_CA77);x^=x>>15;x=x.wrapping_mul(0x2C1B_3C6D);x^=x>>12;(x&0xffff) as f32/65535.0
}
/// 按当前字体、指定字号量文字宽度
pub fn measure(text:&str,size:f32)->f32 {
    if text.is_empty() {return 0.0;}
    let mut out=sys::ImVec2{x:0.0,y:0.0};
    unsafe {
        let font=sys::igGetFont();let range=text.as_bytes().as_ptr_range();
        sys::ImFont_CalcTextSizeA(&mut out,font,size,f32::MAX,0.0,range.start.cast(),range.end.cast(),std::ptr::null_mut());
    }
    out.x
}
fn glyph(list:*mut sys::ImDrawList,size:f32,pos:[f32;2],color:u32,text:&str) {
    let range=text.as_bytes().as_ptr_range();
    unsafe {sys::ImDrawList_AddText_FontPtr(list,sys::igGetFont(),size,sys::ImVec2{x:pos[0],y:pos[1]},color,range.start.cast(),range.end.cast(),0.0,std::ptr::null());}
}
/// 从 pos（左上角）起画一段带特效的文字，返回画完后的宽度
pub fn draw(layer:Layer,pos:[f32;2],size:f32,text:&str,style:&Style,time:f32,alpha:f32)->f32 {
    let list=list(layer);
    let total=measure(text,size).max(1.0);
    let mut x=pos[0];
    let mut buffer=[0u8;4];
    for (i,ch) in text.chars().enumerate() {
        let s:&str=ch.encode_utf8(&mut buffer);
        let width=measure(s,size);
        let (i_f,u)=(i as f32,(x-pos[0]+width*0.5)/total);
        // 颜色
        let mut color=mix(style.color,style.color,0.0);let mut light=1.0f32;
        match style.effect {
            Effect::Gradient=>color=mix(style.color,style.accent,0.5+0.5*(time*1.6-u*5.0).sin()),
            Effect::Shimmer=>{let band=(time*0.7).rem_euclid(1.6)-0.3;let hit=(1.0-(u-band).abs()*4.0).max(0.0);color=mix(style.color,style.accent,hit);light=1.0+0.35*hit;},
            Effect::Pulse=>light=0.78+0.22*(time*2.6).sin(),
            Effect::Flicker=>{light=0.62+0.38*((time*11.0+i_f*1.7).sin()*(time*6.3+i_f*2.9).sin()).abs();color=mix(style.color,style.accent,1.0-light);},
            _=>{},
        }
        // 位置
        let (mut dx,mut dy)=(0.0,0.0);
        match style.effect {
            Effect::Wave=>dy=(time*3.2+i_f*0.7).sin()*size*0.1,
            Effect::Jitter=>{let step=(time*18.0) as u32;dx=(noise(step,i as u32*2)-0.5)*size*0.09;dy=(noise(step,i as u32*2+1)-0.5)*size*0.09;},
            _=>{},
        }
        let at=[x+dx,pos[1]+dy];
        // 外发光：辅色在四周叠一圈半透明字形，亮度随呼吸起伏
        if style.glow {
            let breath=0.55+0.45*(time*2.2+i_f*0.3).sin();
            // 两圈：外圈大而淡，内圈小而亮
            for (r,a) in [(size*0.13,0.14),(size*0.07,0.26)] {
                for k in 0..8 {
                    let angle=k as f32*std::f32::consts::FRAC_PI_4;
                    glyph(list,size,[at[0]+angle.cos()*r,at[1]+angle.sin()*r],rgba(mix(style.accent,style.accent,0.0),a*breath*alpha),s);
                }
            }
        }
        // 残影：身后两道淡色拖影，随时间伸缩
        if style.effect==Effect::Afterimage {
            let stretch=0.6+0.4*(time*5.0).sin().abs();
            for (k,a) in [(3.0,0.14),(2.0,0.24),(1.0,0.4)] {glyph(list,size,[at[0]-size*0.12*k*stretch,at[1]],rgba(mix(style.accent,style.accent,0.0),a*alpha),s);}
        }
        // 深色描边，保证在任何背景上都清楚
        let outline=rgba([0.02,0.02,0.02],0.85*alpha);
        for (ox,oy) in [(-1.5,0.0),(1.5,0.0),(0.0,-1.5),(0.0,1.5),(1.5,1.5)] {glyph(list,size,[at[0]+ox,at[1]+oy],outline,s);}
        glyph(list,size,at,rgba(scale(color,light),alpha),s);
        x+=width;
    }
    x-pos[0]
}
/// 中文换行：汉字之间都可以断行；连续的数字、英文与符号（如“-25%”）作为整体不拆开；
/// “，。；）%”等标点不放在行首（放不下时挂在行尾），“（《”等不留在行尾；遇到换行符强制换行
pub fn wrap_lines(text:&str,width:f32,measure:impl Fn(&str)->f32)->Vec<String> {
    const NO_START:&str="，。、；：！？）》」』】%…·”’,.;:!?)";
    const NO_END:&str="（《「『【“‘(";
    let mut lines=Vec::new();
    for paragraph in text.split('\n') {
        // 切成记号：连续的 ASCII 可见字符一组，其余每个字一组
        let mut tokens:Vec<String>=Vec::new();
        for ch in paragraph.chars() {
            let ascii=ch.is_ascii_graphic();
            match tokens.last_mut() {Some(last) if ascii&&last.chars().all(|c|c.is_ascii_graphic())=>last.push(ch),_=>tokens.push(ch.to_string())}
        }
        let mut line=String::new();
        for token in tokens {
            if line.is_empty()&&token==" " {continue;}
            let candidate=format!("{line}{token}");
            if line.is_empty()||measure(&candidate)<=width||token.chars().all(|c|NO_START.contains(c)) {line=candidate;continue;}
            // 换行：上一行末尾的开括号一起带到下一行
            let mut next=String::new();
            while let Some(last)=line.chars().last().filter(|c|NO_END.contains(*c)) {line.pop();next.insert(0,last);}
            lines.push(line.trim_end().to_owned());
            line=next+token.trim_start();
        }
        lines.push(line.trim_end().to_owned());
    }
    lines
}
thread_local! {static WRAP_AT:std::cell::Cell<Option<f32>>=const {std::cell::Cell::new(None)};}
/// 与 ImGui 的 push_text_wrap_pos 配对：记下换行位置（窗口内坐标）给 text_wrapped 用，离开作用域后恢复
pub struct WrapAt(Option<f32>);
pub fn wrap_at(right:f32)->WrapAt {WrapAt(WRAP_AT.with(|w|w.replace(Some(right))))}
impl Drop for WrapAt {fn drop(&mut self) {WRAP_AT.with(|w|w.set(self.0));}}
/// 代替 ImGui 的 text_wrapped：按中文规则换行，换行位置取 wrap_at 记下的位置，没有时取窗口内容区右边界；每行都从起始的横坐标开始
pub fn text_wrapped(ui:&hudhook::imgui::Ui,text:impl AsRef<str>) {
    let x=ui.cursor_pos()[0];
    let right=WRAP_AT.with(|w|w.get()).unwrap_or_else(||ui.window_content_region_max()[0]);
    for line in wrap_lines(text.as_ref(),(right-x).max(1.0),|t|ui.calc_text_size(t)[0]) {
        ui.set_cursor_pos([x,ui.cursor_pos()[1]]);ui.text(line);
    }
}
/// 传说品质名的描边金色（灾厄燃金的描边色）与字身的暗红紫（在四个相近色之间缓慢流动）
const RIM:[u8;3]=[255,207,90];
const CORE:[[u8;3];4]=[[77,0,33],[94,11,40],[69,0,29],[102,20,44]];
fn mixf(a:[f32;3],b:[f32;3],t:f32)->[f32;3] {let t=t.clamp(0.0,1.0);[0,1,2].map(|i|a[i]*(1.0-t)+b[i]*t)}
fn unit(c:[u8;3])->[f32;3] {c.map(|v|f32::from(v)/255.0)}
fn v2(x:f32,y:f32)->sys::ImVec2 {sys::ImVec2{x,y}}
/// 十字星芒：两条细长菱形交叉
fn flare(list:*mut sys::ImDrawList,x:f32,y:f32,len:f32,spin:f32,color:[f32;3],alpha:f32) {
    if alpha<=0.01||len<=0.3 {return;}
    let thin=len*0.13;
    for arm in 0..2 {
        let (sin,cos)=(spin+arm as f32*std::f32::consts::FRAC_PI_2).sin_cos();
        unsafe {sys::ImDrawList_AddQuadFilled(list,v2(x+cos*len,y+sin*len),v2(x-sin*thin,y+cos*thin),v2(x-cos*len,y-sin*len),v2(x+sin*thin,y-cos*thin),rgba(color,alpha));}
    }
}
/// 闪光：约每 2 秒有六成机会亮一下（平均约 3.3 秒一次），0.5 秒内渐亮渐暗，返回 0..1 的亮度
fn gleam(time:f32)->f32 {
    let at=time.rem_euclid(2.0);
    if noise((time/2.0).floor() as u32,7)<0.6&&at<0.5 {(at/0.5*std::f32::consts::PI).sin()}else{0.0}
}
const VIOLET:[[u8;3];4]=[[42,10,70],[58,18,98],[36,8,60],[68,24,110]];
/// 字后的横向光带（传说金、史诗紫）：多层很淡的圆角矩形由外向内叠加，边缘不显形
fn backdrop(list:*mut sys::ImDrawList,origin:[f32;2],total:f32,size:f32,tier:usize,time:f32,alpha:f32) {
    let (color,strength)=match tier {3=>([1.0,0.6,0.18],0.014),2=>([0.62,0.38,1.0],0.009),_=>return};
    let breath=0.5+0.5*(time*2.3).sin();
    for layer in 0..8 {
        let pad=size*(1.0-layer as f32*0.12);
        let (top,bottom)=(origin[1]-pad*0.45,origin[1]+size+pad*0.45);
        unsafe {sys::ImDrawList_AddRectFilled(list,v2(origin[0]-pad,top),v2(origin[0]+total+pad,bottom),rgba(color,strength*(0.7+0.6*breath)*alpha),(bottom-top)*0.5,0);}
    }
}
/// 星光（灾厄 CosmicPurple 的做法）：位置固定，轮流亮起、上升、旋转；传说密、史诗稀
fn sparkles(list:*mut sys::ImDrawList,origin:[f32;2],total:f32,size:f32,tier:usize,time:f32,alpha:f32) {
    use std::f32::consts::{PI,TAU};
    let (spacing,minimum,scale,outer,inner)=match tier {
        3=>(0.65,4,1.0,[1.0,0.71,0.24],[1.0,0.97,0.88]),
        2=>(1.45,3,0.8,[0.66,0.44,1.0],[0.95,0.9,1.0]),
        _=>return,
    };
    let stars=((total/(size*spacing)).round() as u32).max(minimum);
    for star in 0..stars {
        let life=(time*3.0+noise(star,90)*TAU).rem_euclid(TAU);
        if life>PI {continue;}
        let glint=life.sin();
        let (sx,sy)=(origin[0]+noise(star,0)*total,origin[1]+noise(star,50)*size*0.6-life*size*0.17);
        let spin=time*(0.8+0.7*noise(star,130));
        let reach=size*(0.06+0.14*glint)*scale;
        flare(list,sx,sy,reach*1.6,spin,outer,glint*0.3*alpha);
        flare(list,sx,sy,reach,spin,inner,glint*0.85*alpha);
    }
}
/// 按品质画一段（文字, 主色）：先画光晕与描边，再画字身；品质越高效果越多（tier：0 普通、1 稀有、2 史诗、3 传说）。
/// 传说：只往上飘的火焰副本 + 主色 8 方向描边 + 暗红紫字身；史诗：8 份绕圈的紫色副本 + 主色描边 + 暗紫字身；
/// 稀有：一圈随呼吸明暗的柔光 + 深色描边 + 主色与浅色之间流动的字身；普通：深色描边 + 主色字身。
/// 扫光 ((sin(kx − vt)+1)/2)^n 只落在字上：普通、稀有窄而淡，史诗、传说宽而柔；传说另有渐亮渐暗的闪光（flash 为 0..1）。返回整段宽度
fn glyph_pass(list:*mut sys::ImDrawList,origin:[f32;2],size:f32,parts:&[(&str,[u8;3])],time:f32,alpha:f32,tier:usize,flash:f32)->f32 {
    use std::f32::consts::FRAC_PI_4;
    let breath=0.5+0.5*(time*2.3).sin();
    let mut glyphs=Vec::new();
    let mut x=origin[0];
    for (text,color) in parts {
        for (at,ch) in text.char_indices() {
            let s=&text[at..at+ch.len_utf8()];let width=measure(s,size);
            glyphs.push((x,width,s,*color));x+=width;
        }
    }
    // 光晕
    match tier {
        3=>{
            let pulse=1.0+0.1*(time*20.0).sin();
            for &(gx,_,s,_) in &glyphs {
                for k in 0..8 {
                    let angle=k as f32*FRAC_PI_4+time*2.0;
                    let distort=((origin[1]+k as f32*100.0+time*20.0)*0.05).sin()*size*0.05;
                    glyph(list,size,[gx+angle.cos()*size*0.11*pulse+distort,origin[1]-angle.sin().abs()*size*0.14],rgba([1.0,0.55,0.16],0.12*alpha),s);
                }
            }
        },
        2=>{
            let radius=size*0.08*(1.0+0.4*(time*5.0).sin());
            for &(gx,_,s,_) in &glyphs {
                for k in 0..8 {
                    let (sin,cos)=(k as f32*FRAC_PI_4+time*2.0).sin_cos();
                    glyph(list,size,[gx+cos*radius,origin[1]+sin*radius],rgba([0.69,0.44,1.0],0.13*alpha),s);
                }
            }
        },
        1=>{
            for &(gx,_,s,color) in &glyphs {
                for k in 0..8 {
                    let (sin,cos)=(k as f32*FRAC_PI_4).sin_cos();
                    glyph(list,size,[gx+cos*size*0.08,origin[1]+sin*size*0.08],rgba(unit(lighter(color)),0.09*(0.5+0.5*breath)*alpha),s);
                }
            }
        },
        _=>{},
    }
    // 描边：传说、史诗用主色 8 方向描边（传说闪光时变白金）；稀有、普通用深色描边保证清楚
    if tier>=2 {
        let radius=(size*if tier==3{0.06}else{0.05}).max(1.5);
        for &(gx,_,s,color) in &glyphs {
            let rim=mixf(unit(color),unit([255,244,200]),0.8*flash);
            for k in 0..8 {
                let (sin,cos)=(k as f32*FRAC_PI_4).sin_cos();
                glyph(list,size,[gx+cos*radius,origin[1]+sin*radius],rgba(rim,alpha),s);
            }
        }
    } else {
        let outline=rgba([0.02,0.02,0.02],0.85*alpha);
        for &(gx,_,s,_) in &glyphs {for (ox,oy) in [(-1.5,0.0),(1.5,0.0),(0.0,-1.5),(0.0,1.5),(1.5,1.5)] {glyph(list,size,[gx+ox,origin[1]+oy],outline,s);}}
    }
    // 字身
    // 普通、稀有字身本来就亮，用窄峰；史诗、传说字身暗，用宽峰（约两个字宽、边缘渐变）并只提亮到四成多，避免单个字突然跳亮
    let (speed,strength,sharpness,shine_color)=[(0.7,0.5,120,[255,255,255]),(1.1,0.85,120,[244,248,255]),(1.1,0.4,24,[234,212,255]),(1.2,0.45,24,[254,231,117])][tier.min(3)];
    for (i,&(gx,width,s,color)) in glyphs.iter().enumerate() {
        let body=match tier {
            3|2=>{
                let palette=if tier==3 {&CORE}else{&VIOLET};
                let phase=(time*0.2-i as f32*0.07).rem_euclid(1.0)*palette.len() as f32;
                let slot=phase as usize%palette.len();
                mix(palette[slot],palette[(slot+1)%palette.len()],phase.fract())
            },
            1=>mix(color,lighter(color),0.5+0.5*(time*1.6-i as f32*0.5).sin()),
            _=>unit(color),
        };
        let shine=((((gx+width*0.5-origin[0])*0.33/size-speed*time).sin()+1.0)*0.5).powi(sharpness)*strength;
        let body=mixf(mixf(body,unit(shine_color),shine),unit([255,246,208]),0.25*flash);
        glyph(list,size,[gx,origin[1]],rgba(body,alpha),s);
    }
    x-origin[0]
}
/// 传说品质名，照灾厄“燃金”稀有度的画法：字后金光、火焰副本与描边、暗红紫字身、柔和的扫光、约每 3.3 秒一次渐亮渐暗的闪光（主要在描边上，不抖动）、十字星光。
/// parts 为（文字, 描边色）：补正属性等仍用各自的颜色描边。返回整段宽度
fn auric(layer:Layer,pos:[f32;2],size:f32,parts:&[(&str,[u8;3])],time:f32,alpha:f32)->f32 {
    let list=list(layer);
    let (flash,origin)=(gleam(time),pos);
    let total:f32=parts.iter().map(|(t,_)|measure(t,size)).sum();
    backdrop(list,origin,total,size,3,time,alpha);
    glyph_pass(list,origin,size,parts,time,alpha,3,flash);
    sparkles(list,origin,total,size,3,time,alpha);
    total
}
/// 传说品质名拆成（文字, 描边色）：前缀与装备名用金色描边，补正属性各用自己的颜色（全属性五色轮转），防具类型用类型色，斜杠灰色
fn legendary_parts(text:&str,time:f32)->Vec<(&str,[u8;3])> {
    let Some(rest)=text.strip_prefix("传说·") else {return vec![(text,RIM)]};
    let mut parts=vec![(&text[..text.len()-rest.len()],RIM)];
    if let Some((word,name))=rest.split_once('·') {
        let dot=&rest[word.len()..word.len()+'·'.len_utf8()];
        let mut middle=Vec::new();
        if let Some(style)=armor(word) {middle.push((word,style.color));}
        else if word=="全属性" {
            let colors=["力气","灵巧","智力","信仰","感应"].map(|w|attribute(w).unwrap());
            for (i,(at,ch)) in word.char_indices().enumerate() {
                let phase=time*1.2+i as f32*0.9;let k=phase.floor() as usize;
                let (a,b)=(colors[k%5],colors[(k+1)%5]);let t=phase.fract();
                middle.push((&word[at..at+ch.len_utf8()],[0,1,2].map(|c|(f32::from(a[c])*(1.0-t)+f32::from(b[c])*t) as u8)));
            }
        } else if word.split('/').all(|w|attribute(w).is_some()) {
            let mut start=0;
            for (i,w) in word.split('/').enumerate() {
                if i>0 {middle.push((&word[start-1..start],[150,142,126]));}
                middle.push((&word[start..start+w.len()],attribute(w).unwrap()));start+=w.len()+1;
            }
        }
        if !middle.is_empty() {parts.extend(middle);parts.push((dot,RIM));parts.push((name,RIM));return parts;}
    }
    parts.push((rest,RIM));
    parts
}
/// 补正属性的颜色：力气赤红、灵巧翠绿、智力蔚蓝、信仰圣金、感应绯紫
fn attribute(word:&str)->Option<[u8;3]> {
    match word {"力气"=>Some([228,108,70]),"灵巧"=>Some([104,204,146]),"智力"=>Some([98,152,242]),"信仰"=>Some([238,206,108]),"感应"=>Some([204,88,176]),_=>None}
}
/// 防具类型的样式：物理钢灰流光、魔法蓝色呼吸、锈蚀锈红波动、艾尔登金色流光发光
fn armor(word:&str)->Option<Style> {
    Some(match word {
        "物理"=>Style{color:[170,178,190],accent:[236,240,246],effect:Effect::Shimmer,glow:false},
        "魔法"=>Style{color:[112,152,238],accent:[196,216,255],effect:Effect::Pulse,glow:false},
        "锈蚀"=>Style{color:[198,120,72],accent:[238,170,110],effect:Effect::Wave,glow:false},
        "艾尔登"=>Style{color:[232,196,108],accent:[255,244,200],effect:Effect::Shimmer,glow:true},
        _=>return None,
    })
}
fn lighter(c:[u8;3])->[u8;3] {c.map(|v|(u16::from(v)+(255-u16::from(v))*3/5) as u8)}
/// 名字中间的补正组合或防具类型；认不出来时返回 false，由调用方按普通文字画
fn middle(layer:Layer,x:&mut f32,y:f32,size:f32,word:&str,time:f32,alpha:f32)->bool {
    if let Some(style)=armor(word) {*x+=draw(layer,[*x,y],size,word,&style,time,alpha);return true;}
    if word=="全属性" {
        // 五色轮转：每个字的颜色随时间在五种属性色之间流动
        let colors=["力气","灵巧","智力","信仰","感应"].map(|w|attribute(w).unwrap());
        let mut buffer=[0u8;4];
        for (i,ch) in word.chars().enumerate() {
            let phase=time*1.2+i as f32*0.9;let k=phase.floor() as usize;
            let (a,b)=(colors[k%5],colors[(k+1)%5]);
            let t=phase.fract();let mix=[0,1,2].map(|c|(f32::from(a[c])*(1.0-t)+f32::from(b[c])*t) as u8);
            *x+=draw(layer,[*x,y],size,ch.encode_utf8(&mut buffer),&Style{color:mix,accent:lighter(mix),effect:Effect::Plain,glow:true},time,alpha);
        }
        return true;
    }
    let words:Vec<&str>=word.split('/').collect();
    if words.iter().any(|w|attribute(w).is_none()) {return false;}
    let slash=Style{color:[150,142,126],accent:[150,142,126],effect:Effect::Plain,glow:false};
    for (i,w) in words.into_iter().enumerate() {
        if i>0 {*x+=draw(layer,[*x,y],size,"/",&slash,time,alpha);}
        let color=attribute(w).unwrap();
        *x+=draw(layer,[*x,y],size,w,&Style{color,accent:lighter(color),effect:Effect::Gradient,glow:false},time,alpha);
    }
    true
}
/// 物品名，如“传说·灵巧/信仰·亵渎圣剑”“史诗·物理·恶兆之子铠甲”，品质越高效果越多（见 glyph_pass）：
/// 传说整行按灾厄“燃金”画（auric，补正属性与防具类型体现在描边颜色上）；
/// 史诗加字后紫光与稀疏星光；稀有、普通只有柔光或描边与扫光。史诗及以下的补正组合与防具类型保留各自的颜色与动态效果（见 middle）。
/// x 是按 align 对齐的位置（0 左对齐、0.5 居中），返回整段宽度
pub fn quality_name(layer:Layer,x:f32,y:f32,size:f32,text:&str,color:[f32;4],tier:usize,align:f32,time:f32)->f32 {
    let rgb=[0,1,2].map(|i|(color[i].clamp(0.0,1.0)*255.0) as u8);let alpha=color[3];
    let total=measure(text,size);let mut x=x-total*align;
    if tier>=3 {auric(layer,[x,y],size,&legendary_parts(text,time),time,alpha);return total;}
    let list=list(layer);let origin=[x,y];
    backdrop(list,origin,total,size,tier,time,alpha);
    let prefix=["普通·","稀有·","史诗·","传说·"].into_iter().find(|p|text.starts_with(p));
    let rest=match prefix {
        Some(prefix)=>{x+=glyph_pass(list,[x,y],size,&[(prefix,rgb)],time,alpha,tier,0.0);&text[prefix.len()..]},
        None=>text,
    };
    let mut drawn=false;
    if prefix.is_some()&&let Some((word,name))=rest.split_once('·') {
        let start=x;
        if middle(layer,&mut x,y,size,word,time,alpha) {
            x+=glyph_pass(list,[x,y],size,&[("·",rgb)],time,alpha,tier,0.0);
            glyph_pass(list,[x,y],size,&[(name,rgb)],time,alpha,tier,0.0);
            drawn=true;
        } else {x=start;}
    }
    if !drawn {glyph_pass(list,[x,y],size,&[(rest,rgb)],time,alpha,tier,0.0);}
    sparkles(list,origin,total,size,tier,time,alpha);
    total
}
/// 一行由多段组成的特效文字（段之间插分隔符），整行居中画在 center_x
pub fn draw_line(layer:Layer,center_x:f32,y:f32,size:f32,parts:&[(&str,Style)],separator:(&str,[u8;3]),time:f32) {
    if parts.is_empty() {return;}
    let sep_width=measure(separator.0,size);
    let total:f32=parts.iter().map(|(t,s)|measure(t,size)+if s.effect==Effect::Afterimage{size*0.4}else{0.0}).sum::<f32>()+sep_width*(parts.len()-1) as f32;
    let mut x=center_x-total*0.5;
    let plain=Style{color:separator.1,accent:separator.1,effect:Effect::Pulse,glow:false};
    for (index,(text,style)) in parts.iter().enumerate() {
        if index>0 {x+=draw(layer,[x,y],size,separator.0,&Style{effect:Effect::Shimmer,..plain},0.0,1.0);}
        // 残影拖在字的左边，给它留出空位，不压到前面的分隔符
        if style.effect==Effect::Afterimage {x+=size*0.4;}
        x+=draw(layer,[x,y],size,text,style,time,1.0);
    }
}

/// 屏幕上方词条行的颜色与动态效果：与 Boss 身上光效的颜色一致
pub fn affix_style(affix:erdtree_tower::boss_affixes::Affix)->Style {
    use erdtree_tower::boss_affixes::Affix;
    let (color,accent,effect,glow)=match affix {
        Affix::Stalwart=>([196,182,150],[244,226,178],Effect::Pulse,false),
        Affix::Bulwark=>([140,172,204],[228,240,255],Effect::Shimmer,false),
        Affix::Frenzied=>([232,72,52],[255,170,90],Effect::Jitter,true),
        Affix::Regenerating=>([112,204,124],[204,255,196],Effect::Pulse,true),
        Affix::Rot=>([206,82,62],[244,146,96],Effect::Wave,false),
        Affix::Envious=>([160,96,214],[60,24,90],Effect::Flicker,true),
        Affix::Proud=>([244,238,224],[255,214,140],Effect::Shimmer,true),
        Affix::Golden=>([242,200,92],[255,248,210],Effect::Shimmer,true),
        Affix::Reborn=>([244,150,62],[255,224,128],Effect::Gradient,true),
        Affix::Leeching=>([204,32,54],[255,96,116],Effect::Pulse,true),
        Affix::Thorned=>([172,42,42],[232,96,84],Effect::Jitter,false),
        Affix::Swift=>([92,222,204],[204,255,246],Effect::Afterimage,false),
    };
    Style{color,accent,effect,glow}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn colors_pack_as_imgui_abgr_and_noise_stays_in_range() {
        assert_eq!(rgba([1.0,0.0,0.0],1.0),0xff0000ff);
        assert_eq!(rgba([0.0,0.0,1.0],0.5)>>24,127);
        assert!((0..1000).all(|i|(0.0..=1.0).contains(&noise(i,7))));
        assert_eq!(mix([0,0,0],[255,255,255],0.5),[0.5019608,0.5019608,0.5019608]);
        // 每个字宽 1：一行最多 6 个字
        let width=|t:&str|t.chars().count() as f32;
        assert_eq!(wrap_lines("举盾防御时受到的所有伤害 -25%（含物理）；防御时",6.0,width),vec!["举盾防御时受","到的所有伤害","-25%（含","物理）；防御","时"]);
        assert_eq!(wrap_lines("一二三四五六，七",6.0,width),vec!["一二三四五六，","七"],"punctuation hangs at the line end");
        assert_eq!(wrap_lines("一二三四五（六）",6.0,width),vec!["一二三四五","（六）"],"opening brackets move to the next line");
        assert_eq!(wrap_lines("甲\n乙",6.0,width),vec!["甲","乙"]);
        assert_eq!(ranges("文乙丁丙"),vec![0x20,0x7e,0x4e01,0x4e01,0x4e19,0x4e19,0x4e59,0x4e59,0x6587,0x6587,0]);
    }
}
