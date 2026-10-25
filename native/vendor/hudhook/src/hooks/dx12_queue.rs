use std::collections::HashMap;

use windows::core::{IUnknown, Interface, Result};
use windows::Win32::Graphics::Direct3D12::{
    ID3D12CommandList, ID3D12CommandQueue, ID3D12Device, ID3D12Resource,
    D3D12_COMMAND_LIST_TYPE_DIRECT,
};
use windows::Win32::Graphics::Dxgi::IDXGISwapChain3;

pub(super) struct BackBufferEvidence<T> {
    buffers: Vec<usize>,
    recordings: HashMap<usize, T>,
}

impl<T> Default for BackBufferEvidence<T> {
    fn default() -> Self {
        Self {
            buffers: Vec::new(),
            recordings: HashMap::new(),
        }
    }
}

impl<T> BackBufferEvidence<T> {
    pub(super) fn set_buffers(&mut self, buffers: Vec<usize>) {
        if self.buffers != buffers {
            self.recordings.clear();
            self.buffers = buffers;
        }
    }

    pub(super) fn record(&mut self, list: usize, resource: usize, retention: T) {
        if self.buffers.contains(&resource) && self.recordings.len() < 64 {
            self.recordings.insert(list, retention);
        }
    }

    pub(super) fn reset(&mut self, list: usize) {
        self.recordings.remove(&list);
    }

    pub(super) fn submitted(&mut self, lists: &[usize]) -> bool {
        let mut matched = false;
        for list in lists {
            matched |= self.recordings.remove(list).is_some();
        }
        matched
    }

    pub(super) fn clear(&mut self) {
        self.buffers.clear();
        self.recordings.clear();
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.recordings.len()
    }
}

#[derive(Default)]
pub(super) struct QueueDiscovery {
    swap_chain: Option<IDXGISwapChain3>,
    device_identity: usize,
    evidence: BackBufferEvidence<ID3D12CommandList>,
}

fn identity<T: Interface>(object: &T) -> Result<usize> {
    Ok(object.cast::<IUnknown>()?.as_raw() as usize)
}

impl QueueDiscovery {
    pub(super) unsafe fn observe(&mut self, swap_chain: &IDXGISwapChain3) -> Result<()> {
        let device: ID3D12Device = swap_chain.GetDevice()?;
        let desc = swap_chain.GetDesc()?;
        let mut buffers = Vec::with_capacity(desc.BufferCount as usize);
        for index in 0..desc.BufferCount {
            let resource: ID3D12Resource = swap_chain.GetBuffer(index)?;
            buffers.push(identity(&resource)?);
        }
        if self.swap_chain.as_ref().map(identity).transpose()? != Some(identity(swap_chain)?) {
            self.evidence.clear();
        }
        self.swap_chain = Some(swap_chain.clone());
        self.device_identity = identity(&device)?;
        // Keep identities only: retaining a back buffer would prevent ResizeBuffers.
        self.evidence.set_buffers(buffers);
        Ok(())
    }

    pub(super) fn record(&mut self, list: &ID3D12CommandList, resource: &ID3D12Resource) {
        let Ok(resource_identity) = identity(resource) else {
            return;
        };
        if !self.evidence.buffers.contains(&resource_identity) {
            return;
        }
        let Ok(list_identity) = list.cast::<IUnknown>() else {
            return;
        };
        // Retain the list until submission/reset so its address cannot be reused.
        self.evidence.record(
            list_identity.as_raw() as usize,
            resource_identity,
            list.clone(),
        );
    }

    pub(super) fn reset(&mut self, list: &ID3D12CommandList) {
        if let Ok(identity) = identity(list) {
            self.evidence.reset(identity);
        }
    }

    pub(super) unsafe fn submitted(
        &mut self,
        queue: &ID3D12CommandQueue,
        lists: &[ID3D12CommandList],
    ) -> Option<IDXGISwapChain3> {
        if queue.GetDesc().Type != D3D12_COMMAND_LIST_TYPE_DIRECT {
            return None;
        }
        let device: ID3D12Device = crate::util::try_out_ptr(|out| queue.GetDevice(out)).ok()?;
        if identity(&device).ok()? != self.device_identity {
            return None;
        }
        let identities: Vec<_> = lists
            .iter()
            .filter_map(|list| identity(list).ok())
            .collect();
        self.evidence
            .submitted(&identities)
            .then(|| self.swap_chain.clone())
            .flatten()
    }

    pub(super) fn clear(&mut self) {
        self.swap_chain = None;
        self.device_identity = 0;
        self.evidence.clear();
    }
}
