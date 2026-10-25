use hudhook::windows::{
    Win32::{
        Foundation::CloseHandle,
        Graphics::{
            Direct3D::D3D_FEATURE_LEVEL_11_0,
            Direct3D12::*,
            Dxgi::{Common::*, *},
        },
        System::Threading::{CreateEventW, WaitForSingleObject},
    },
    core::{Interface, Result},
};
use hudhook::{
    Hudhook, ImguiRenderLoop,
    hooks::{DummyHwnd, dx12::ImguiDx12Hooks},
    imgui::Ui,
    util,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static FRAMES: AtomicUsize = AtomicUsize::new(0);
struct Overlay;
impl ImguiRenderLoop for Overlay {
    fn render(&mut self, ui: &mut Ui) {
        ui.window("queue regression")
            .build(|| ui.text("late attachment works"));
        FRAMES.fetch_add(1, Ordering::Relaxed);
    }
}

fn main() -> Result<()> {
    unsafe { run() }
}

unsafe fn run() -> Result<()> {
    unsafe {
        let window = DummyHwnd::new();
        let factory: IDXGIFactory4 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
        let adapter: IDXGIAdapter = if std::env::args().any(|arg| arg == "--warp") {
            factory.EnumWarpAdapter()?
        } else {
            factory.EnumAdapters(0)?
        };
        let device: ID3D12Device =
            util::try_out_ptr(|out| D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, out))?;
        let queue: ID3D12CommandQueue =
            device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC::default())?;
        let unrelated: ID3D12CommandQueue =
            device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC::default())?;
        let allocator: ID3D12CommandAllocator =
            device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let list: ID3D12GraphicsCommandList =
            device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)?;
        list.Close()?;
        let early = std::env::args().any(|arg| arg == "--early");
        if early {
            Hudhook::builder()
                .with::<ImguiDx12Hooks>(Overlay)
                .build()
                .apply()
                .unwrap();
        }
        let swap: IDXGISwapChain3 = factory
            .CreateSwapChainForHwnd(
                &queue,
                window.hwnd(),
                &DXGI_SWAP_CHAIN_DESC1 {
                    Width: 64,
                    Height: 64,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: 2,
                    SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                    ..Default::default()
                },
                None,
                None,
            )?
            .cast()?;
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        let event = CreateEventW(None, false, false, None)?;
        let mut tick = 0;

        if !early {
            // The swap chain already exists when hooks are installed.
            Hudhook::builder()
                .with::<ImguiDx12Hooks>(Overlay)
                .build()
                .apply()
                .unwrap();
        }
        if !early {
            swap.Present(0, DXGI_PRESENT(0)).ok()?;
            assert_eq!(FRAMES.load(Ordering::Relaxed), 0);

            allocator.Reset()?;
            list.Reset(&allocator, None)?;
            list.Close()?;
            unrelated.ExecuteCommandLists(&[Some(list.cast()?)]);
            wait(&unrelated, &fence, event, &mut tick)?;
            swap.Present(0, DXGI_PRESENT(0)).ok()?;
            assert_eq!(
                FRAMES.load(Ordering::Relaxed),
                0,
                "unrelated queue initialized overlay"
            );

            // Abandon a recorded back-buffer transition and submit an empty recording.
            allocator.Reset()?;
            list.Reset(&allocator, None)?;
            record(&list, &swap)?;
            list.Close()?;
            list.Reset(&allocator, None)?;
            list.Close()?;
            unrelated.ExecuteCommandLists(&[Some(list.cast()?)]);
            wait(&unrelated, &fence, event, &mut tick)?;
            swap.Present(0, DXGI_PRESENT(0)).ok()?;
            assert_eq!(
                FRAMES.load(Ordering::Relaxed),
                0,
                "abandoned recording selected queue"
            );
        }

        let deadline = Instant::now() + Duration::from_secs(10);
        while FRAMES.load(Ordering::Relaxed) < 3 && Instant::now() < deadline {
            draw(&queue, &allocator, &list, &swap, &fence, event, &mut tick)?;
        }
        assert!(
            FRAMES.load(Ordering::Relaxed) >= 3,
            "late attachment never rendered"
        );
        wait(&queue, &fence, event, &mut tick)?;
        let before_resize = FRAMES.load(Ordering::Relaxed);
        swap.ResizeBuffers(
            2,
            96,
            96,
            DXGI_FORMAT_R8G8B8A8_UNORM,
            DXGI_SWAP_CHAIN_FLAG(0),
        )?;
        for _ in 0..3 {
            draw(&queue, &allocator, &list, &swap, &fence, event, &mut tick)?;
        }
        assert!(
            FRAMES.load(Ordering::Relaxed) > before_resize,
            "rendering stopped after resize"
        );
        wait(&queue, &fence, event, &mut tick)?;
        CloseHandle(event)?;
        println!(
            "PASS: {} attachment, queue association, resize; {} overlay frames",
            if early {
                "early"
            } else {
                "late (unrelated/reset rejected)"
            },
            FRAMES.load(Ordering::Relaxed)
        );
        Ok(())
    }
}

unsafe fn record(list: &ID3D12GraphicsCommandList, swap: &IDXGISwapChain3) -> Result<()> {
    unsafe {
        let buffer: ID3D12Resource = swap.GetBuffer(swap.GetCurrentBackBufferIndex())?;
        let barriers = [
            util::create_barrier(
                &buffer,
                D3D12_RESOURCE_STATE_PRESENT,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            ),
            util::create_barrier(
                &buffer,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_PRESENT,
            ),
        ];
        list.ResourceBarrier(&barriers);
        barriers.into_iter().for_each(util::drop_barrier);
        Ok(())
    }
}

unsafe fn draw(
    queue: &ID3D12CommandQueue,
    allocator: &ID3D12CommandAllocator,
    list: &ID3D12GraphicsCommandList,
    swap: &IDXGISwapChain3,
    fence: &ID3D12Fence,
    event: hudhook::windows::Win32::Foundation::HANDLE,
    tick: &mut u64,
) -> Result<()> {
    unsafe {
        allocator.Reset()?;
        list.Reset(allocator, None)?;
        record(list, swap)?;
        list.Close()?;
        queue.ExecuteCommandLists(&[Some(list.cast()?)]);
        wait(queue, fence, event, tick)?;
        swap.Present(0, DXGI_PRESENT(0)).ok()?;
        wait(queue, fence, event, tick)?;
        Ok(())
    }
}

unsafe fn wait(
    queue: &ID3D12CommandQueue,
    fence: &ID3D12Fence,
    event: hudhook::windows::Win32::Foundation::HANDLE,
    tick: &mut u64,
) -> Result<()> {
    unsafe {
        *tick += 1;
        queue.Signal(fence, *tick)?;
        if fence.GetCompletedValue() < *tick {
            fence.SetEventOnCompletion(*tick, event)?;
            assert_eq!(WaitForSingleObject(event, 5000).0, 0, "GPU fence timeout");
        }
        Ok(())
    }
}
