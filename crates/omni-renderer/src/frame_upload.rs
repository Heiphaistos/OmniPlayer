use wgpu::{Device, Queue, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages};
use omni_core::decoder::{DecodedVideoFrame, PixelFormat};

/// Textures GPU pour un frame YUV420p (8-bit) ou YUV420P10LE (HDR 10-bit).
pub struct YuvTextures {
    pub y:  Texture,
    pub u:  Texture,
    pub v:  Texture,
    pub width:  u32,
    pub height: u32,
    texel_format: TextureFormat,
}

impl YuvTextures {
    /// Alloue ou ré-alloue les textures si la résolution ou la profondeur change.
    pub fn ensure(
        current: Option<Self>,
        device:  &Device,
        w: u32,
        h: u32,
        hdr10: bool,
    ) -> Self {
        let texel_format = if hdr10 { TextureFormat::R16Unorm } else { TextureFormat::R8Unorm };
        if let Some(t) = current {
            if t.width == w && t.height == h && t.texel_format == texel_format { return t; }
        }
        let make = |lw: u32, lh: u32| {
            device.create_texture(&TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: lw, height: lh, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count:    1,
                dimension:       TextureDimension::D2,
                format:          texel_format,
                usage:           TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats:    &[],
            })
        };
        Self {
            y: make(w, h), u: make(w / 2, h / 2), v: make(w / 2, h / 2),
            width: w, height: h, texel_format,
        }
    }

    /// Upload un frame décodé vers les textures GPU.
    pub fn upload(&self, queue: &Queue, frame: &DecodedVideoFrame) {
        let upload = |tex: &Texture, data: &[u8], stride: usize, w: u32, h: u32| {
            queue.write_texture(
                tex.as_image_copy(),
                data,
                wgpu::TexelCopyBufferLayout {
                    offset:         0,
                    bytes_per_row:  Some(stride as u32),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
        };

        match frame.format {
            PixelFormat::Yuv420p => {
                upload(&self.y, &frame.planes[0], frame.strides[0], self.width, self.height);
                upload(&self.u, &frame.planes[1], frame.strides[1], self.width / 2, self.height / 2);
                upload(&self.v, &frame.planes[2], frame.strides[2], self.width / 2, self.height / 2);
            }
            PixelFormat::Yuv420p10le if self.texel_format == TextureFormat::R16Unorm => {
                upload(&self.y, &frame.planes[0], frame.strides[0], self.width, self.height);
                upload(&self.u, &frame.planes[1], frame.strides[1], self.width / 2, self.height / 2);
                upload(&self.v, &frame.planes[2], frame.strides[2], self.width / 2, self.height / 2);
            }
            PixelFormat::Yuv420p10le => {
                // GPU sans TEXTURE_FORMAT_16BIT_NORM : les textures sont retombées
                // en R8Unorm (cf. VideoRenderer::supports_16bit) mais les plans
                // décodés restent en 16-bit décalé (extract_planes, indépendant
                // du GPU) — on retombe en 8-bit ici en gardant l'octet haut de
                // chaque échantillon (équivalent à une division par 256), en
                // respectant le stride d'origine (peut inclure du padding).
                let (cw, ch) = (self.width as usize / 2, self.height as usize / 2);
                let y8 = downsample_16_to_8(&frame.planes[0], frame.strides[0], self.width as usize, self.height as usize);
                let u8_ = downsample_16_to_8(&frame.planes[1], frame.strides[1], cw, ch);
                let v8 = downsample_16_to_8(&frame.planes[2], frame.strides[2], cw, ch);
                upload(&self.y, &y8, self.width as usize, self.width, self.height);
                upload(&self.u, &u8_, cw, cw as u32, ch as u32);
                upload(&self.v, &v8, cw, cw as u32, ch as u32);
            }
            _ => {
                log::warn!("format {:?} non géré dans upload YUV", frame.format);
            }
        }
    }
}

/// Downsample 16-bit décalé (P010) → 8-bit tassé, en respectant le stride
/// d'origine (peut contenir du padding non pertinent en fin de ligne).
fn downsample_16_to_8(data: &[u8], src_stride: usize, w: usize, h: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h);
    for row in 0..h {
        let row_start = row * src_stride;
        for col in 0..w {
            let idx = row_start + col * 2 + 1; // octet haut de l'échantillon 16-bit LE
            out.push(data.get(idx).copied().unwrap_or(0));
        }
    }
    out
}
