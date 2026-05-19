use anyhow::{Context as _, Result};
use ffmpeg_next as ffmpeg;
use ffmpeg::software::scaling::{context::Context as SwsContext, flag::Flags};

use super::PixelFormat;

/// Frame vidéo décodée, prête à envoyer au renderer.
#[derive(Clone)]
pub struct DecodedVideoFrame {
    /// Timestamp de présentation en secondes.
    pub pts_secs:  f64,
    pub width:     u32,
    pub height:    u32,
    pub format:    PixelFormat,
    /// Plans vidéo: [Y, U, V] ou [Y+UV pour NV12] ou [RGBA unique].
    pub planes:    Vec<Vec<u8>>,
    /// Strides (bytes par ligne) par plan.
    pub strides:   Vec<usize>,
}

/// Décodeur vidéo avec gestion du scaling/conversion de format.
pub struct VideoDecoder {
    decoder:     ffmpeg::codec::decoder::Video,
    scaler:      Option<SwsContext>,
    target_fmt:  ffmpeg::format::Pixel,
    time_base:   f64,
    // Tracks source properties to detect mid-stream changes requiring scaler rebuild.
    scaler_src_w:   u32,
    scaler_src_h:   u32,
    scaler_src_fmt: Option<ffmpeg::format::Pixel>,
}

impl VideoDecoder {
    pub fn new(decoder: ffmpeg::codec::decoder::Video, time_base: f64) -> Result<Self> {
        Ok(Self {
            decoder,
            scaler: None,
            target_fmt: ffmpeg::format::Pixel::YUV420P,
            time_base,
            scaler_src_w:   0,
            scaler_src_h:   0,
            scaler_src_fmt: None,
        })
    }

    /// Envoie un paquet compressé au décodeur.
    pub fn send_packet(&mut self, packet: &ffmpeg::Packet) -> Result<()> {
        self.decoder
            .send_packet(packet)
            .context("send_packet vidéo")
    }

    /// Envoie le signal de fin de flux.
    pub fn send_eof(&mut self) -> Result<()> {
        self.decoder.send_eof().context("send_eof vidéo")
    }

    /// Reçoit une frame décodée si disponible.
    pub fn receive_frame(&mut self) -> Result<Option<DecodedVideoFrame>> {
        let mut raw = ffmpeg::util::frame::video::Video::empty();
        match self.decoder.receive_frame(&mut raw) {
            Ok(()) => {}
            Err(ffmpeg::Error::Other { errno: ffmpeg::error::EAGAIN }) => return Ok(None),
            Err(e) => return Err(e).context("receive_frame vidéo"),
        }

        let pts_secs = raw
            .pts()
            .map(|p| p as f64 * self.time_base)
            .unwrap_or(0.0);

        // Conversion de format si nécessaire (ex: yuv420p10le → yuv420p)
        let frame = if raw.format() != self.target_fmt {
            // Rebuild scaler if source dimensions or pixel format changed mid-stream.
            let needs_rebuild = self.scaler.is_none()
                || self.scaler_src_w   != raw.width()
                || self.scaler_src_h   != raw.height()
                || self.scaler_src_fmt != Some(raw.format());

            if needs_rebuild {
                self.scaler = Some(
                    SwsContext::get(
                        raw.format(),
                        raw.width(),
                        raw.height(),
                        self.target_fmt,
                        raw.width(),
                        raw.height(),
                        Flags::BILINEAR,
                    )
                    .context("création SwsContext — format/dimensions incompatibles")?,
                );
                self.scaler_src_w   = raw.width();
                self.scaler_src_h   = raw.height();
                self.scaler_src_fmt = Some(raw.format());
            }

            let scaler = self.scaler.as_mut().expect("scaler vient d'être initialisé");
            let mut converted = ffmpeg::util::frame::video::Video::empty();
            scaler.run(&raw, &mut converted)?;
            converted
        } else {
            raw
        };

        let (planes, strides, format) = extract_planes(&frame);

        Ok(Some(DecodedVideoFrame {
            pts_secs,
            width:   frame.width(),
            height:  frame.height(),
            format,
            planes,
            strides,
        }))
    }

    pub fn width(&self)  -> u32 { self.decoder.width() }
    pub fn height(&self) -> u32 { self.decoder.height() }
}

fn extract_planes(frame: &ffmpeg::util::frame::video::Video) -> (Vec<Vec<u8>>, Vec<usize>, PixelFormat) {
    match frame.format() {
        ffmpeg::format::Pixel::YUV420P => {
            let (_w, h) = (frame.width() as usize, frame.height() as usize);
            let y_stride  = frame.stride(0);
            let uv_stride = frame.stride(1);

            let y = frame.data(0)[..y_stride * h].to_vec();
            let u = frame.data(1)[..uv_stride * (h / 2)].to_vec();
            let v = frame.data(2)[..uv_stride * (h / 2)].to_vec();

            (vec![y, u, v], vec![y_stride, uv_stride, uv_stride], PixelFormat::Yuv420p)
        }
        ffmpeg::format::Pixel::NV12 => {
            let (_w, h) = (frame.width() as usize, frame.height() as usize);
            let y_stride  = frame.stride(0);
            let uv_stride = frame.stride(1);
            let y  = frame.data(0)[..y_stride * h].to_vec();
            let uv = frame.data(1)[..uv_stride * (h / 2)].to_vec();
            (vec![y, uv], vec![y_stride, uv_stride], PixelFormat::Nv12)
        }
        _ => {
            // Fallback: data du plan 0 en RGBA (après conversion SwsContext)
            let stride = frame.stride(0);
            let data   = frame.data(0)[..stride * frame.height() as usize].to_vec();
            (vec![data], vec![stride], PixelFormat::Rgba)
        }
    }
}
