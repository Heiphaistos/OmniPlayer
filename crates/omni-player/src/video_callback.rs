use std::sync::Arc;
use parking_lot::Mutex;
use omni_core::decoder::DecodedVideoFrame;
use omni_renderer::VideoRenderer;

use eframe::{egui_wgpu, wgpu};

pub type SharedFrame = Arc<Mutex<Option<DecodedVideoFrame>>>;

/// Callback egui_wgpu : upload la dernière frame YUV vers le GPU et encode le rendu.
pub struct VideoPaintCallback {
    pub frame:       SharedFrame,
    pub color_space: u32,  // 0=BT601, 1=BT709, 2=BT2020
}

impl egui_wgpu::CallbackTrait for VideoPaintCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue:  &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _enc:   &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = resources.get_mut::<VideoRenderer>() {
            renderer.set_color_space(queue, self.color_space);
            if let Some(frame) = self.frame.lock().take() {
                renderer.upload_frame(device, queue, &frame);
            }
        }
        vec![]
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        rp:   &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = resources.get::<VideoRenderer>() {
            renderer.render(rp);
        }
    }
}
