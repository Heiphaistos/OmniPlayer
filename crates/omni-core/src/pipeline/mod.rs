pub mod clock;
pub mod demuxer;

use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::thread;

use crate::decoder::{DecodedAudioFrame, DecodedVideoFrame};

/// Commandes envoyées au thread de pipeline.
#[derive(Debug)]
pub enum PipelineCommand {
    Pause,
    Resume,
    Seek(f64),          // position en secondes
    SetVolume(f32),     // 0.0–1.0
    SelectAudioTrack(usize),
    SelectSubtitleTrack(Option<usize>),
    Stop,
}

/// Événements émis par le pipeline vers l'UI.
#[derive(Debug)]
pub enum PipelineEvent {
    PositionChanged(f64),    // secondes
    DurationKnown(f64),
    BufferingProgress(u8),   // 0–100
    EndOfStream,
    Error(String),
    MetadataReady(Box<crate::probe::MediaInfo>),
}

/// Capacité max des queues de frames (frames buffered).
const VIDEO_QUEUE_DEPTH: usize = 16;
const AUDIO_QUEUE_DEPTH: usize = 512;

pub struct MediaPipeline {
    cmd_tx:   Sender<PipelineCommand>,
    event_rx: Receiver<PipelineEvent>,
    video_rx: Receiver<DecodedVideoFrame>,
    audio_rx: Receiver<DecodedAudioFrame>,
}

impl MediaPipeline {
    /// Lance le pipeline de décodage dans des threads dédiés.
    pub fn launch(path: String) -> Result<Self> {
        let (cmd_tx, cmd_rx)         = bounded::<PipelineCommand>(16);
        let (event_tx, event_rx)     = bounded::<PipelineEvent>(64);
        let (video_tx, video_rx)     = bounded::<DecodedVideoFrame>(VIDEO_QUEUE_DEPTH);
        let (audio_tx, audio_rx)     = bounded::<DecodedAudioFrame>(AUDIO_QUEUE_DEPTH);

        let path_clone = path.clone();
        thread::Builder::new()
            .name("omni-demuxer".into())
            .spawn(move || {
                if let Err(e) = demuxer::run_demuxer(
                    &path_clone, cmd_rx, event_tx, video_tx, audio_tx,
                ) {
                    log::error!("demuxer: {e:#}");
                }
            })?;

        Ok(Self { cmd_tx, event_rx, video_rx, audio_rx })
    }

    pub fn send_command(&self, cmd: PipelineCommand) {
        let _ = self.cmd_tx.try_send(cmd);
    }

    pub fn try_recv_event(&self) -> Option<PipelineEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn try_recv_video_frame(&self) -> Option<DecodedVideoFrame> {
        self.video_rx.try_recv().ok()
    }

    pub fn try_recv_audio_frame(&self) -> Option<DecodedAudioFrame> {
        self.audio_rx.try_recv().ok()
    }
}
