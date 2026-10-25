use super::dx12_queue::BackBufferEvidence;

#[test]
fn actual_dx12_objects_reject_compute_and_foreign_device_queues() {
    use super::{dx12_queue::QueueDiscovery, DummyHwnd};
    use windows::{
        core::Interface,
        Win32::Graphics::{
            Direct3D::D3D_FEATURE_LEVEL_11_0,
            Direct3D12::*,
            Dxgi::{Common::*, *},
        },
    };
    unsafe {
        let window = DummyHwnd::new();
        let factory: IDXGIFactory4 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)).unwrap();
        let adapter = factory.EnumAdapters(0).unwrap();
        let warp: IDXGIAdapter = factory.EnumWarpAdapter().unwrap();
        let device: ID3D12Device = crate::util::try_out_ptr(|out| {
            D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, out)
        })
        .unwrap();
        let foreign_device: ID3D12Device =
            crate::util::try_out_ptr(|out| D3D12CreateDevice(&warp, D3D_FEATURE_LEVEL_11_0, out))
                .unwrap();
        let queue: ID3D12CommandQueue = device
            .CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC::default())
            .unwrap();
        let foreign_queue: ID3D12CommandQueue = foreign_device
            .CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC::default())
            .unwrap();
        let compute: ID3D12CommandQueue = device
            .CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_COMPUTE,
                ..Default::default()
            })
            .unwrap();
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
            )
            .unwrap()
            .cast()
            .unwrap();
        let allocator: ID3D12CommandAllocator = device
            .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
            .unwrap();
        let list: ID3D12GraphicsCommandList = device
            .CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)
            .unwrap();
        list.Close().unwrap();
        let list: ID3D12CommandList = list.cast().unwrap();
        let buffer: ID3D12Resource = swap.GetBuffer(0).unwrap();
        let mut discovery = QueueDiscovery::default();
        discovery.observe(&swap).unwrap();
        discovery.record(&list, &buffer);
        let submitted = [list];
        assert!(discovery.submitted(&compute, &submitted).is_none());
        assert!(discovery.submitted(&foreign_queue, &submitted).is_none());
        assert!(discovery.submitted(&queue, &submitted).is_some());
        assert!(discovery.submitted(&queue, &submitted).is_none());
        drop(buffer);
        swap.ResizeBuffers(
            2,
            96,
            96,
            DXGI_FORMAT_R8G8B8A8_UNORM,
            DXGI_SWAP_CHAIN_FLAG(0),
        )
        .unwrap();
    }
}

#[test]
fn unrelated_resource_and_command_list_do_not_select_a_queue() {
    let mut evidence = BackBufferEvidence::<()>::default();
    evidence.set_buffers(vec![10, 11]);
    evidence.record(20, 99, ());
    assert!(!evidence.submitted(&[20]));
    evidence.record(21, 10, ());
    assert!(!evidence.submitted(&[22]));
    assert!(evidence.submitted(&[21]));
    assert!(!evidence.submitted(&[21]));
}

#[test]
fn reset_and_buffer_replacement_discard_stale_evidence() {
    let mut evidence = BackBufferEvidence::<()>::default();
    evidence.set_buffers(vec![10]);
    evidence.record(20, 10, ());
    evidence.reset(20);
    assert!(!evidence.submitted(&[20]));
    evidence.record(20, 10, ());
    evidence.set_buffers(vec![11]);
    assert!(!evidence.submitted(&[20]));
    evidence.record(20, 11, ());
    evidence.set_buffers(vec![11]);
    assert!(evidence.submitted(&[20]));
}

#[test]
fn evidence_is_bounded_and_all_submitted_lists_are_consumed() {
    let mut evidence = BackBufferEvidence::<()>::default();
    evidence.set_buffers(vec![10]);
    for list in 1..=128 {
        evidence.record(list, 10, ());
    }
    assert_eq!(evidence.len(), 64);
    assert!(evidence.submitted(&[1, 2]));
    assert!(!evidence.submitted(&[2]));
    assert!(!evidence.submitted(&[128]));
    evidence.clear();
    assert!(!evidence.submitted(&[3]));
}
