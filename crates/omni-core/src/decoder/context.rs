use anyhow::{Context as _, Result};
use ffmpeg_next as ffmpeg;

use crate::hw_accel::HwAccelContext;

/// Contexte de décodage principal — ouvre le fichier et initialise les codecs.
pub struct DecodeContext {
    pub format_ctx: ffmpeg::format::context::Input,
    pub video_stream_idx: Option<usize>,
    pub audio_stream_idx: Option<usize>,
    pub subtitle_stream_idx: Option<usize>,
    pub hw_accel: Option<HwAccelContext>,
}

impl DecodeContext {
    /// Ouvre un fichier local ou une URL réseau (HTTP/RTSP/RTMP/HLS).
    pub fn open(path: &str, preferred_hw: Option<&str>) -> Result<Self> {
        ffmpeg::init().context("ffmpeg::init")?;

        let format_ctx = ffmpeg::format::input(&path)
            .with_context(|| format!("impossible d'ouvrir '{path}'"))?;

        let video_stream_idx = format_ctx
            .streams()
            .best(ffmpeg::media::Type::Video)
            .map(|s| s.index());

        let audio_stream_idx = format_ctx
            .streams()
            .best(ffmpeg::media::Type::Audio)
            .map(|s| s.index());

        let subtitle_stream_idx = format_ctx
            .streams()
            .best(ffmpeg::media::Type::Subtitle)
            .map(|s| s.index());

        let hw_accel = preferred_hw
            .and_then(|name| HwAccelContext::try_init(name).ok());

        Ok(Self {
            format_ctx,
            video_stream_idx,
            audio_stream_idx,
            subtitle_stream_idx,
            hw_accel,
        })
    }

    /// Construit un décodeur vidéo pour le flux sélectionné.
    pub fn build_video_decoder(&self) -> Result<ffmpeg::codec::decoder::Video> {
        let stream_idx = self
            .video_stream_idx
            .context("aucun flux vidéo trouvé")?;
        let stream = self
            .format_ctx
            .stream(stream_idx)
            .context("stream vidéo introuvable")?;

        let mut codec_ctx =
            ffmpeg::codec::context::Context::from_parameters(stream.parameters())
                .context("création du contexte codec vidéo")?;

        // Active l'accélération matérielle si disponible
        if let Some(hw) = &self.hw_accel {
            hw.apply_to_codec(&mut codec_ctx);
        }

        codec_ctx
            .decoder()
            .video()
            .context("ouverture décodeur vidéo")
    }

    /// Construit un décodeur audio pour le flux sélectionné.
    pub fn build_audio_decoder(&self) -> Result<ffmpeg::codec::decoder::Audio> {
        let stream_idx = self
            .audio_stream_idx
            .context("aucun flux audio trouvé")?;
        self.build_audio_decoder_for(stream_idx)
    }

    /// Construit un décodeur audio pour un flux arbitraire (changement de piste).
    pub fn build_audio_decoder_for(&self, stream_idx: usize) -> Result<ffmpeg::codec::decoder::Audio> {
        let stream = self
            .format_ctx
            .stream(stream_idx)
            .context("stream audio introuvable")?;

        ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .context("création du contexte codec audio")?
            .decoder()
            .audio()
            .context("ouverture décodeur audio")
    }

    /// Retourne la durée totale en secondes.
    pub fn duration_secs(&self) -> f64 {
        self.format_ctx.duration() as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE)
    }

    /// Seek vers une position en secondes.
    /// av_seek_frame + BACKWARD : se cale sur la keyframe ≤ cible (standard lecteurs).
    /// L'ancien `format_ctx.seek(ts, ts..)` (min_ts = cible) échouait en EPERM sur MP4.
    pub fn seek(&mut self, position_secs: f64) -> Result<()> {
        let ts = (position_secs * f64::from(ffmpeg::ffi::AV_TIME_BASE)) as i64;
        unsafe {
            let ret = ffmpeg::ffi::av_seek_frame(
                self.format_ctx.as_mut_ptr(),
                -1,
                ts,
                ffmpeg::ffi::AVSEEK_FLAG_BACKWARD,
            );
            if ret >= 0 { return Ok(()); }
            // Fallback pour les formats sans av_seek_frame fonctionnel
            let ret2 = ffmpeg::ffi::avformat_seek_file(
                self.format_ctx.as_mut_ptr(),
                -1,
                i64::MIN,
                ts,
                i64::MAX,
                ffmpeg::ffi::AVSEEK_FLAG_BACKWARD,
            );
            if ret2 >= 0 { return Ok(()); }
            anyhow::bail!("seek échoué (av_seek_frame={ret}, avformat_seek_file={ret2})");
        }
    }
}
