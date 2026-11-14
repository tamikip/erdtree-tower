//! Optional D3D11/DirectComposition host; never hooks the game's Present.
use std::{sync::{Mutex,atomic::{AtomicBool,AtomicIsize,Ordering}},time::{Duration,Instant}};
use hudhook::{ImguiRenderLoop,RenderContext,imgui::{Context,DrawData,MouseButton}};
use windows::{core::{Interface,Result,Error,w},Win32::{Foundation::*,Graphics::{
    Direct3D::D3D_DRIVER_TYPE_HARDWARE,Direct3D11::*,DirectComposition::*,
    Dxgi::{IDXGIDevice,IDXGISurface,Common::*},Gdi::{ClientToScreen,ScreenToClient}},
    UI::{Input::KeyboardAndMouse::GetAsyncKeyState,HiDpi::*,WindowsAndMessaging::*}}};
use crate::gpu_engine::D3D11RenderEngine;

pub static ACTIVE:AtomicBool=AtomicBool::new(false);
static ORIGINAL:AtomicIsize=AtomicIsize::new(0);
static EVENTS:Mutex<Vec<(u32,usize)>>=Mutex::new(Vec::new());

fn visible(foreground:bool,minimized:bool,width:i32,height:i32)->bool {
    foreground&&!minimized&&width>0&&height>0
}
unsafe extern "system" fn window_proc(hwnd:HWND,msg:u32,w:WPARAM,l:LPARAM)->LRESULT {
    match msg {WM_NCHITTEST=>LRESULT(HTTRANSPARENT as isize),WM_MOUSEACTIVATE=>LRESULT(MA_NOACTIVATE as isize),
        _=>unsafe{DefWindowProcW(hwnd,msg,w,l)}}
}
unsafe extern "system" fn game_proc(hwnd:HWND,msg:u32,w:WPARAM,l:LPARAM)->LRESULT {
    if ACTIVE.load(Ordering::Acquire)&&crate::input::foreground() {
        if msg==WM_SETCURSOR&&crate::input::active() {unsafe{SetCursor(None);}return LRESULT(1);}
        if matches!(msg,WM_CHAR|WM_MOUSEWHEEL|WM_MOUSEHWHEEL) {
            if let Ok(mut events)=EVENTS.lock(){if events.len()<1024 {events.push((msg,w.0));}}
        }
        if crate::input::active()&&matches!(msg,WM_MOUSEMOVE|WM_LBUTTONDOWN|WM_LBUTTONUP|WM_RBUTTONDOWN|WM_RBUTTONUP|WM_MBUTTONDOWN|WM_MBUTTONUP|WM_MOUSEWHEEL|WM_MOUSEHWHEEL|WM_CHAR) {return LRESULT(0);}
    }
    let original=ORIGINAL.load(Ordering::Acquire);
    if original==0 {unsafe{DefWindowProcW(hwnd,msg,w,l)}} else {unsafe{CallWindowProcW(std::mem::transmute::<isize,WNDPROC>(original),hwnd,msg,w,l)}}
}

struct Window {overlay:HWND,game:HWND,dpi:DPI_AWARENESS_CONTEXT}
impl Window {
    fn new(game:HWND)->Result<Self> {unsafe {
        let dpi=SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        RegisterClassW(&WNDCLASSW{lpfnWndProc:Some(window_proc),lpszClassName:w!("ErdtreeTowerComposition"),..Default::default()});
        let overlay=match CreateWindowExW(WS_EX_NOREDIRECTIONBITMAP|WS_EX_LAYERED|WS_EX_TRANSPARENT|WS_EX_TOPMOST|WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE,
            w!("ErdtreeTowerComposition"),w!("Erdtree Tower UI"),WS_POPUP,0,0,1,1,None,None,None,None) {
            Ok(hwnd)=>hwnd,Err(e)=>{SetThreadDpiAwarenessContext(dpi);return Err(e);}
        };
        let window=Self{overlay,game,dpi};
        SetLayeredWindowAttributes(overlay,COLORREF(0),255,LWA_ALPHA)?;
        // Preserve the existing hook chain, including ReShade's procedure.
        let old=GetWindowLongPtrW(game,GWLP_WNDPROC);
        if old==0 {return Err(Error::from_thread());}
        ORIGINAL.store(old,Ordering::Release);
        SetLastError(WIN32_ERROR(0));
        if SetWindowLongPtrW(game,GWLP_WNDPROC,game_proc as *const () as isize)==0&&GetLastError().0!=0 {return Err(Error::from_thread());}
        crate::input::WINDOW.store(game.0 as isize,Ordering::Release);
        Ok(window)
    }}
}
impl Drop for Window {fn drop(&mut self){unsafe {
    ACTIVE.store(false,Ordering::Release);
    crate::input::CAPTURE.store(false,Ordering::Release);
    // A later subclass may still call ours: leave its chain intact and forward.
    if GetWindowLongPtrW(self.game,GWLP_WNDPROC)==game_proc as *const () as isize {
        SetWindowLongPtrW(self.game,GWLP_WNDPROC,ORIGINAL.load(Ordering::Acquire));
    }
    let _=DestroyWindow(self.overlay);
    SetThreadDpiAwarenessContext(self.dpi);
}}}

struct Surface {
    device:ID3D11Device,context:ID3D11DeviceContext,dcomp:IDCompositionDevice,
    visual:IDCompositionVisual,_target:IDCompositionTarget,surface:IDCompositionSurface,
    texture:ID3D11Texture2D,view:ID3D11RenderTargetView,size:[u32;2],
}
impl Surface {
    fn buffers(device:&ID3D11Device,dcomp:&IDCompositionDevice,size:[u32;2])->Result<(IDCompositionSurface,ID3D11Texture2D,ID3D11RenderTargetView)> {unsafe {
        let surface=dcomp.CreateSurface(size[0],size[1],DXGI_FORMAT_B8G8R8A8_UNORM,DXGI_ALPHA_MODE_PREMULTIPLIED)?;
        let texture=hudhook::util::try_out_ptr(|out|device.CreateTexture2D(&D3D11_TEXTURE2D_DESC{
            Width:size[0],Height:size[1],MipLevels:1,ArraySize:1,Format:DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc:DXGI_SAMPLE_DESC{Count:1,Quality:0},Usage:D3D11_USAGE_DEFAULT,BindFlags:D3D11_BIND_RENDER_TARGET.0 as u32,..Default::default()
        },None,Some(out)))?;
        let view=hudhook::util::try_out_ptr(|out|device.CreateRenderTargetView(&texture,None,Some(out)))?;
        Ok((surface,texture,view))
    }}
    fn new(hwnd:HWND,size:[u32;2])->Result<Self> {unsafe {
        let(mut device,mut context)=(None,None);
        D3D11CreateDevice(None,D3D_DRIVER_TYPE_HARDWARE,Default::default(),D3D11_CREATE_DEVICE_BGRA_SUPPORT,None,D3D11_SDK_VERSION,Some(&mut device),None,Some(&mut context))?;
        let device=device.ok_or_else(Error::from_thread)?;
        let context=context.ok_or_else(Error::from_thread)?;
        let dxgi:IDXGIDevice=device.cast()?;
        let dcomp:IDCompositionDevice=DCompositionCreateDevice(&dxgi)?;
        let target=dcomp.CreateTargetForHwnd(hwnd,true)?;
        let visual=dcomp.CreateVisual()?;
        let(surface,texture,view)=Self::buffers(&device,&dcomp,size)?;
        visual.SetContent(&surface)?;target.SetRoot(&visual)?;dcomp.Commit()?;
        Ok(Self{device,context,dcomp,visual,_target:target,surface,texture,view,size})
    }}
    fn resize(&mut self,size:[u32;2])->Result<()> {if self.size==size{return Ok(());} unsafe {
        let(surface,texture,view)=Self::buffers(&self.device,&self.dcomp,size)?;
        self.visual.SetContent(&surface)?;self.dcomp.Commit()?;
        self.surface=surface;self.texture=texture;self.view=view;self.size=size;Ok(())
    }}
    fn draw(&self,engine:&mut D3D11RenderEngine,data:&DrawData)->Result<()> {unsafe {
        self.context.ClearRenderTargetView(&self.view,&[0.0;4]);
        if data.total_vtx_count>0 {engine.render(data,self.texture.clone())?;}
        let mut offset=POINT::default();
        let dxgi:IDXGISurface=self.surface.BeginDraw(None,&mut offset)?;
        // Composition may return an atlas texture and a non-zero update origin.
        let copied=(||->Result<()>{
            let target:ID3D11Texture2D=dxgi.cast()?;
            let mut desc=D3D11_TEXTURE2D_DESC::default();target.GetDesc(&mut desc);
            if !copy_fits([offset.x,offset.y],self.size,[desc.Width,desc.Height]) {return Err(Error::new(E_INVALIDARG,"Invalid composition update bounds"));}
            self.context.CopySubresourceRegion(&target,0,offset.x as u32,offset.y as u32,0,&self.texture,0,None);
            Ok(())
        })();
        let ended=self.surface.EndDraw();
        copied?;ended?;self.dcomp.Commit()?;self.device.GetDeviceRemovedReason()
    }}
}
fn copy_fits(offset:[i32;2],size:[u32;2],target:[u32;2])->bool {
    (0..2).all(|i|offset[i]>=0&&(offset[i] as u32).checked_add(size[i]).is_some_and(|end|end<=target[i]))
}

fn game_window()->Option<HWND> {
    unsafe extern "system" fn visit(hwnd:HWND,param:LPARAM)->windows::core::BOOL {
        let mut pid=0;unsafe{GetWindowThreadProcessId(hwnd,Some(&mut pid));}
        if pid==std::process::id()&&unsafe{IsWindowVisible(hwnd)}.as_bool() {
            let mut rect=RECT::default();let best=unsafe{&mut *(param.0 as *mut (i64,HWND))};
            if unsafe{GetClientRect(hwnd,&mut rect)}.is_ok() {
                let area=i64::from(rect.right)*i64::from(rect.bottom);
                if area>best.0 { *best=(area,hwnd); }
            }
        }
        true.into()
    }
    let mut best=(0i64,HWND::default());
    unsafe{let _=EnumWindows(Some(visit),LPARAM(&mut best as *mut _ as isize));}
    (best.0>0).then_some(best.1)
}

#[derive(Default)]
struct Keys {blocked:[bool;5],keyboard:Vec<bool>,focused:bool}
fn gate_down(blocked:&mut bool,regained:bool,down:bool)->bool {
    if regained {*blocked=down;}
    if !down {*blocked=false;}
    down&&!*blocked
}
impl Keys {
    fn feed(&mut self,ctx:&mut Context,game:HWND,focused:bool) {
        let io=ctx.io_mut();
        io.app_focus_lost=!focused;
        if self.keyboard.is_empty(){self.keyboard.resize(crate::overlay_keys::KEYS.len(),false);}
        for (i,&(key,vk)) in crate::overlay_keys::KEYS.iter().enumerate() {
            // XInput owns gamepad events; do not overwrite them with virtual keys.
            if (0xc3..=0xda).contains(&vk.0)||matches!(vk.0,1|2|4|5|6){continue;}
            let down=unsafe{GetAsyncKeyState(vk.0 as i32)}<0;
            let accepted=gate_down(&mut self.keyboard[i],focused&&!self.focused,down);
            io.add_key_event(key,focused&&accepted);
        }
        for (i,(button,vk)) in [(MouseButton::Left,1),(MouseButton::Right,2),(MouseButton::Middle,4),(MouseButton::Extra1,5),(MouseButton::Extra2,6)].into_iter().enumerate() {
            let down=unsafe{GetAsyncKeyState(vk)}<0;
            let accepted=gate_down(&mut self.blocked[i],focused&&!self.focused,down);
            io.add_mouse_button_event(button,focused&&accepted);
        }
        for (key,vk) in [(hudhook::imgui::Key::ModCtrl,0x11),(hudhook::imgui::Key::ModShift,0x10),(hudhook::imgui::Key::ModAlt,0x12)] {
            io.add_key_event(key,focused&&unsafe{GetAsyncKeyState(vk)}<0);
        }
        if focused {
            let mut point=POINT::default();unsafe{let _=GetCursorPos(&mut point);let _=ScreenToClient(game,&mut point);}
            io.add_mouse_pos_event([point.x as f32,point.y as f32]);
        }
        if let Ok(mut events)=EVENTS.lock() {
            let mut text=Vec::new();
            for (msg,value) in events.drain(..) {if focused&&self.focused {
                if msg==WM_CHAR {text.push(value as u16);} else {
                    let delta=((value>>16) as u16 as i16) as f32/120.0;
                    io.add_mouse_wheel_event(if msg==WM_MOUSEWHEEL{[0.0,delta]}else{[delta,0.0]});
                }
            }}
            for c in char::decode_utf16(text).flatten(){io.add_input_character(c);}
        }
        self.focused=focused;
    }
}

pub fn install(address:usize) {
    std::thread::spawn(move||{
        let result=std::panic::catch_unwind(run);
        ACTIVE.store(false,Ordering::Release);
        crate::input::CAPTURE.store(false,Ordering::Release);
        match result {Ok(Ok(()))=>{},other=>{
            crate::log(format!("OVERLAY stopped ({other:?}); falling back to game renderer"));
            crate::install_dx12(address);
        }}
    });
}
fn run()->Result<()> {
    let start=Instant::now();
    let game=loop {
        if let Some(hwnd)=game_window(){break hwnd;}
        if start.elapsed()>Duration::from_secs(60){return Err(Error::new(E_FAIL,"Game window unavailable"));}
        std::thread::sleep(Duration::from_millis(100));
    };
    let window=Window::new(game)?;
    let mut surface=Surface::new(window.overlay,[1,1])?;
    let mut ctx=Context::create();
    ctx.io_mut().display_size=[1.0,1.0];
    let mut engine=D3D11RenderEngine::new(&surface.device,&mut ctx)?;
    let mut tower=crate::TowerPreview::new();tower.initialize(&mut ctx,&mut engine);
    let fonts=ctx.fonts();let atlas=fonts.build_rgba32_texture();
    fonts.tex_id=engine.load_texture(atlas.data,atlas.width,atlas.height)?;
    ACTIVE.store(true,Ordering::Release);
    crate::log("UI_INIT stage=overlay_gpu; independent composition surface ready");
    let(mut keys,mut last,mut shown)=(Keys::default(),Instant::now(),false);
    loop {unsafe {
        let mut msg=MSG::default();
        while PeekMessageW(&mut msg,None,0,0,PM_REMOVE).as_bool() {
            if msg.message==WM_QUIT{return Ok(());}
            let _=TranslateMessage(&msg);DispatchMessageW(&msg);
        }
        if !IsWindow(Some(game)).as_bool(){return Ok(());}
        let mut rect=RECT::default();GetClientRect(game,&mut rect)?;
        let(width,height)=(rect.right-rect.left,rect.bottom-rect.top);
        let foreground=GetForegroundWindow()==game&&IsWindowVisible(game).as_bool();
        let show=visible(foreground,IsIconic(game).as_bool(),width,height);
        keys.feed(&mut ctx,game,show);
        if !show {
            if shown {let _=ShowWindow(window.overlay,SW_HIDE);shown=false;}
            crate::input::CAPTURE.store(false,Ordering::Release);
            crate::native_settings::begin_frame();
            crate::reward_ui::poll_pad(ctx.io_mut(),false,false);
            // Drain release/focus events without running any UI actions in the background.
            ctx.io_mut().delta_time=0.05;ctx.frame();ctx.render();
            last=Instant::now();std::thread::sleep(Duration::from_millis(50));continue;
        }
        surface.resize([width as u32,height as u32])?;
        let mut origin=POINT::default();ClientToScreen(game,&mut origin).ok()?;
        SetWindowPos(window.overlay,Some(HWND_TOPMOST),origin.x,origin.y,width,height,SWP_NOACTIVATE)?;
        ctx.io_mut().display_size=[width as f32,height as f32];
        ctx.io_mut().display_framebuffer_scale=[1.0,1.0];
        let now=Instant::now();ctx.io_mut().update_delta_time(now.duration_since(last).min(Duration::from_millis(100)));last=now;
        tower.before_render(&mut ctx,&mut engine);
        if crate::input::active(){let _=ClipCursor(None);}
        tower.render(ctx.frame());
        surface.draw(&mut engine,ctx.render())?;
        if !shown {let _=ShowWindow(window.overlay,SW_SHOWNOACTIVATE);shown=true;}
        std::thread::sleep(Duration::from_millis(16));
    }}
}

#[cfg(test)]
mod tests {
    #[test] fn focus_return_requires_release_before_click_or_hotkey(){
        let mut blocked=false;
        assert!(!super::gate_down(&mut blocked,true,true));
        assert!(!super::gate_down(&mut blocked,false,true));
        assert!(!super::gate_down(&mut blocked,false,false));
        assert!(super::gate_down(&mut blocked,false,true));
        assert!(super::gate_down(&mut blocked,false,true),"Held mouse must continue dragging");
    }
    #[test]
    #[ignore = "Requires a Windows desktop and D3D11 hardware; run explicitly"]
    fn hardware_composition_draw_resize_and_cleanup() {
        use super::*;
        unsafe {
            RegisterClassW(&WNDCLASSW{lpfnWndProc:Some(window_proc),lpszClassName:w!("TowerCompositionTest"),..Default::default()});
            let game=CreateWindowExW(Default::default(),w!("TowerCompositionTest"),w!(""),WS_POPUP,0,0,320,200,None,None,None,None).unwrap();
            let original=GetWindowLongPtrW(game,GWLP_WNDPROC);
            {
                let window=Window::new(game).unwrap();
                let mut surface=Surface::new(window.overlay,[64,64]).unwrap();
                let mut ctx=Context::create();
                let mut engine=D3D11RenderEngine::new(&surface.device,&mut ctx).unwrap();
                let fonts=ctx.fonts();let atlas=fonts.build_rgba32_texture();
                fonts.tex_id=engine.load_texture(atlas.data,atlas.width,atlas.height).unwrap();
                let mut keys=Keys::default();
                for size in [[64,64],[320,200],[120,80]] {
                    surface.resize(size).unwrap();ctx.io_mut().display_size=[size[0] as f32,size[1] as f32];
                    keys.feed(&mut ctx,game,true);
                    ctx.frame().get_foreground_draw_list().add_rect([10.0,10.0],[40.0,40.0],hudhook::imgui::ImColor32::from_rgba(255,0,0,128)).filled(true).build();
                    surface.draw(&mut engine,ctx.render()).unwrap();
                    let mut desc=D3D11_TEXTURE2D_DESC::default();surface.texture.GetDesc(&mut desc);
                    desc.Usage=D3D11_USAGE_STAGING;desc.BindFlags=0;desc.CPUAccessFlags=D3D11_CPU_ACCESS_READ.0 as u32;
                    let staging:ID3D11Texture2D=hudhook::util::try_out_ptr(|out|surface.device.CreateTexture2D(&desc,None,Some(out))).unwrap();
                    surface.context.CopyResource(&staging,&surface.texture);
                    let mut mapped=D3D11_MAPPED_SUBRESOURCE::default();surface.context.Map(&staging,0,D3D11_MAP_READ,0,Some(&mut mapped)).unwrap();
                    let pixel=std::slice::from_raw_parts((mapped.pData as *const u8).add(mapped.RowPitch as usize*20+20*4),4).to_vec();
                    surface.context.Unmap(&staging,0);
                    assert!(pixel[0]<3&&pixel[1]<3&&(126..=130).contains(&pixel[2])&&(126..=130).contains(&pixel[3]),"Premultiplied BGRA: {pixel:?}");
                    keys.feed(&mut ctx,game,false);ctx.frame();ctx.render();
                }
            }
            assert_eq!(GetWindowLongPtrW(game,GWLP_WNDPROC),original);
            DestroyWindow(game).unwrap();
        }
    }
    #[test] fn background_and_minimized_windows_are_never_visible(){
        assert!(super::visible(true,false,1920,1080));
        for (focus,min,w,h) in [(false,false,1920,1080),(true,true,1920,1080),(true,false,0,1080),(true,false,1920,0)]{assert!(!super::visible(focus,min,w,h));}
    }
    #[test] fn composition_copy_respects_atlas_offset_and_bounds(){
        assert!(super::copy_fits([16,32],[1920,1080],[2048,2048]));
        for offset in [[-1,0],[200,0],[i32::MAX,0]]{assert!(!super::copy_fits(offset,[1920,1080],[2048,2048]));}
    }
}
