// 律法路线的专属图标（原创生成，不用原版图标）：scene-assets/law-art/icons.rgba，5 × 4 格、每格 160 像素，
// 第 N 格对应 src/law.rs 的 Node::art（scene-builder/law_icons.py 生成）。界面上都画 icons-line.rgba（同样排布的简约白色线条版），
// 原来的插画图标只在线条版缺失时用。
use std::sync::atomic::{AtomicUsize,Ordering};
use hudhook::{RenderContext,imgui::{DrawListMut,ImColor32,TextureId}};
static ATLAS:AtomicUsize=AtomicUsize::new(usize::MAX);
static LINE_ATLAS:AtomicUsize=AtomicUsize::new(usize::MAX);
const CELL:u32=160;const COLUMNS:u32=5;const ROWS:u32=4;
pub fn initialize(renderer:&mut dyn RenderContext) {
    let (width,height)=(CELL*COLUMNS,CELL*ROWS);
    for (slot,name) in [(&ATLAS,"icons.rgba"),(&LINE_ATLAS,"icons-line.rgba")] {
        slot.store(usize::MAX,Ordering::Relaxed);
        match std::fs::read(crate::LOG_PATH.with_file_name("scene-assets").join("law-art").join(name)) {
            Ok(data) if data.len()==(width*height*4) as usize=>match renderer.load_texture(&data,width,height) {
                Ok(id)=>slot.store(id.id(),Ordering::Relaxed),
                Err(e)=>crate::log(format!("Law icons upload failed ({name}): {e:?}")),
            },
            _=>crate::log(format!("Law icons missing: scene-assets/law-art/{name}")),
        }
    }
}
/// 第 index 格在图标集里的 UV
fn uv(index:u8)->([f32;2],[f32;2]) {
    let (x,y)=(f32::from(index)%COLUMNS as f32,(u32::from(index)/COLUMNS) as f32);
    ([x/COLUMNS as f32,y/ROWS as f32],[(x+1.0)/COLUMNS as f32,(y+1.0)/ROWS as f32])
}
/// 画律法图标（简约线条版；线条版没加载时退回原来的插画图标）；都没加载时返回 false（调用方不画图标）
pub fn draw_line(draw:&DrawListMut<'_>,index:u8,min:[f32;2],max:[f32;2],light:u8)->bool {
    if index==19 {
        let w=max[0]-min[0];let h=max[1]-min[1];let color=ImColor32::from_rgba(light,light,light,255);
        for x in [0.12,0.48] {draw.add_polyline(vec![[min[0]+w*x,min[1]+h*0.2],[min[0]+w*(x+0.3),min[1]+h*0.5],[min[0]+w*x,min[1]+h*0.8]],color).thickness((w*0.08).max(1.0)).build();}
        return true;
    }
    // 精英横行复用精英徽记，并加上升箭头，不增加纹理依赖。
    let base=if index==18 {2}else{index};
    let shown=draw_from(&LINE_ATLAS,draw,base,min,max,light)||draw_from(&ATLAS,draw,base,min,max,light);
    if shown&&index==18 {
        let w=max[0]-min[0];let h=max[1]-min[1];let x=min[0]+w*0.85;let y=min[1]+h*0.12;
        let color=ImColor32::from_rgba(light,light,light,255);let stroke=(w*0.06).max(1.0);
        for (a,b) in [([x,y+h*0.45],[x,y]),([x-w*0.12,y+h*0.15],[x,y]),([x,y],[x+w*0.12,y+h*0.15])] {
            draw.add_line(a,b,color).thickness(stroke).build();
        }
    }
    shown
}
fn draw_from(atlas:&AtomicUsize,draw:&DrawListMut<'_>,index:u8,min:[f32;2],max:[f32;2],light:u8)->bool {
    let Some(id)=Some(atlas.load(Ordering::Relaxed)).filter(|&id|id!=usize::MAX).map(TextureId::new) else{return false;};
    let (a,b)=uv(index);
    draw.add_image(id,min,max).uv_min(a).uv_max(b).col(ImColor32::from_rgba(light,light,light,255)).build();true
}
#[cfg(test)]
mod tests {
    #[test]
    fn cells_cover_the_grid_without_overlap() {
        assert_eq!(super::uv(0),([0.0,0.0],[0.2,0.25]));
        assert_eq!(super::uv(16),([0.2,0.75],[0.4,1.0]));
        assert!(erdtree_tower::law::NODES.iter().all(|n|u32::from(n.art)<super::COLUMNS*super::ROWS));
    }
}
