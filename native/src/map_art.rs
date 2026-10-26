use std::sync::atomic::{AtomicUsize,Ordering};
use hudhook::{RenderContext,imgui::{TextureId,DrawListMut,ImColor32}};
use erdtree_tower::NodeKind;
static PAPER:AtomicUsize=AtomicUsize::new(usize::MAX);
static ICONS:AtomicUsize=AtomicUsize::new(usize::MAX);
fn texture(slot:&AtomicUsize)->Option<TextureId> {Some(slot.load(Ordering::Relaxed)).filter(|&id|id!=usize::MAX).map(TextureId::new)}
pub fn initialize(renderer:&mut dyn RenderContext) {
    for (name,width,height,target) in [("paper",1024,1536,&PAPER),("icons",1254,1254,&ICONS)] {
        target.store(usize::MAX,Ordering::Relaxed);
        let path=crate::LOG_PATH.with_file_name("scene-assets").join("map-art").join(format!("{name}.rgba"));
        match std::fs::read(path) {
            Ok(data) if data.len()==width as usize*height as usize*4=>match renderer.load_texture(&data,width,height) {
                Ok(id)=>target.store(id.id(),Ordering::Relaxed),
                Err(e)=>crate::log(format!("Map art upload failed: {name}: {e:?}")),
            },
            _=>crate::log(format!("Map art missing: {name}")),
        }
    }
}
pub fn paper(draw:&DrawListMut<'_>,min:[f32;2],max:[f32;2])->bool {
    let Some(id)=texture(&PAPER) else{return false;};
    draw.add_image(id,min,max).build();true
}
pub fn icon(draw:&DrawListMut<'_>,p:[f32;2],kind:NodeKind,alpha:f32)->bool {
    icon_scaled(draw,p,kind,alpha,1.0)
}
pub fn icon_scaled(draw:&DrawListMut<'_>,p:[f32;2],kind:NodeKind,alpha:f32,zoom:f32)->bool {
    let Some(id)=texture(&ICONS) else{return false;};
    let index=match kind {NodeKind::Monster=>0,NodeKind::Elite=>1,NodeKind::Camp=>2,NodeKind::Shop=>3,NodeKind::Chest=>4,NodeKind::Event=>5,NodeKind::Boss=>6};
    let boxes=[[33.0,308.0,313.0,599.0],[347.0,279.0,611.0,618.0],[674.0,286.0,905.0,624.0],[984.0,324.0,1215.0,606.0],[28.0,740.0,316.0,1001.0],[397.0,713.0,572.0,998.0],[630.0,679.0,934.0,1017.0]];
    let b=boxes[index];let size=if kind==NodeKind::Boss{78.0}else{49.0}*zoom;
    let ratio:f32=(b[2]-b[0])/(b[3]-b[1]);let width=size*ratio.min(1.0);let height=size/ratio.max(1.0);
    draw.add_image(id,[p[0]-width*0.5,p[1]-height*0.5],[p[0]+width*0.5,p[1]+height*0.5])
        .uv_min([b[0]/1254.0,b[1]/1254.0]).uv_max([b[2]/1254.0,b[3]/1254.0]).col(ImColor32::from_rgba(255,255,255,(alpha*255.0) as u8)).build();true
}
#[cfg(test)]
pub fn preview(){PAPER.store(100000,Ordering::Relaxed);ICONS.store(100001,Ordering::Relaxed);}
