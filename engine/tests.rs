#[cfg(test)]
use std::sync::{Arc, RwLock};

#[cfg(test)]
use wgpu::{Adapter, CommandEncoder, CommandEncoderDescriptor, Instance};

use crate::gpu::allocator::gpu_allocator::{AllocEntry, GpuAllocator};
#[cfg(test)]
use crate::gpu::tools::GpuTools;

#[cfg(test)]
fn needs() -> (Instance, Adapter, Arc<RwLock<CommandEncoder>>, Arc<GpuTools>) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let frame_encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
    let gpu_tools = GpuTools::new(device, queue);
    (instance, adapter, Arc::new(RwLock::new(frame_encoder)), Arc::new(gpu_tools))
}

#[test]
fn allocator() {
    let (_instance, _adapter, frame_encoder, gpu_tools) = needs();
    let mut alloc = GpuAllocator::new(gpu_tools, frame_encoder);

    // ADD

    assert_eq!(alloc.add(&[0, 2, 4, 8]), Ok(0));
    assert_eq!(alloc.add(&[3, 6, 9, 12]), Ok(1));

    // FREE

    assert_eq!(alloc.free(0), Ok(()));

    // IN-GAP ADD

    assert_eq!(alloc.add(&[0, 2, 4, 8]), Ok(0));
    assert_eq!(
        alloc.get_mesh_entry(0),
        Some(&AllocEntry {
            id: 0,
            position: 0,
            length: 4
        })
    );

    // DEFRAG

    assert_eq!(alloc.free(0), Ok(()));
    alloc.reallocate_defragment(4);
    assert_eq!(
        alloc.data,
        vec![AllocEntry {
            id: 1,
            position: 0,
            length: 4
        }]
    );
}
