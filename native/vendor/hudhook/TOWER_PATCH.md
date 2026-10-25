Based on hudhook 72e1cc062c35df847b703e5507b82eb21d80d82c (MIT).

Late DX12 attachment discovers a direct command queue by observing a PRESENT
transition of an actual swap-chain back buffer and submission of that command
list on the swap chain's device. It no longer probes private swap-chain memory.
Factory and ResizeBuffers1 capture remain the preferred initialization paths.
Evidence is bounded, consumed on submission, and invalidated on command-list
reset, buffer resize, pipeline reset, and unhook. Back buffers are not retained.
