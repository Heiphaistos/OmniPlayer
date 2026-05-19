use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use ffmpeg_next as ffmpeg;

use crate::decoder::{
    audio::AudioDecoder, video::VideoDecoder, DecodedAudioFrame, DecodedVideoFrame,
};
use crate::decoder::context::DecodeContext;
use crate::pipeline::{PipelineCommand, PipelineEvent};
use crate::probe;

pub fn run_demuxer(
    path:     &str,
    cmd_rx:   Receiver<PipelineCommand>,
    event_tx: Sender<PipelineEvent>,
    video_tx: Sender<DecodedVideoFrame>,
    audio_tx: Sender<DecodedAudioFrame>,
) -> Result<()> {
    let info = probe::probe_file(std::path::Path::new(path))
        .unwrap_or_else(|_| probe::MediaInfo {
            path: path.to_string(),
            duration_secs: 0.0,
            video: None,
            audio: vec![],
            subtitles: vec![],
            chapters: vec![],
            format_name: "unknown".into(),
            bit_rate: 0,
        });

    let duration = info.duration_secs;
    let _ = event_tx.send(PipelineEvent::MetadataReady(Box::new(info)));
    let _ = event_tx.send(PipelineEvent::DurationKnown(duration));

    // Prefer d3d11va (modern API) on Windows; fall back to dxva2, then software.
    #[cfg(windows)]
    let preferred_hw = {
        use crate::hw_accel::windows::win::is_d3d11va_available;
        if is_d3d11va_available() { Some("d3d11va") } else { Some("dxva2") }
    };
    #[cfg(not(windows))]
    let preferred_hw: Option<&str> = None;

    let mut ctx = DecodeContext::open(path, preferred_hw)?;

    let v_idx = ctx.video_stream_idx;
    let a_idx = ctx.audio_stream_idx;

    let v_tb = v_idx.and_then(|i| {
        ctx.format_ctx.stream(i).map(|s| {
            s.time_base().numerator() as f64 / s.time_base().denominator().max(1) as f64
        })
    }).unwrap_or(0.0);
    let a_tb = a_idx.and_then(|i| {
        ctx.format_ctx.stream(i).map(|s| {
            s.time_base().numerator() as f64 / s.time_base().denominator().max(1) as f64
        })
    }).unwrap_or(0.0);

    let mut video_dec = v_idx
        .map(|_| ctx.build_video_decoder().map(|d| VideoDecoder::new(d, v_tb)))
        .transpose()?
        .and_then(|r| r.ok());

    let mut audio_dec = a_idx
        .map(|_| ctx.build_audio_decoder().map(|d| AudioDecoder::new(d, a_tb)))
        .transpose()?
        .and_then(|r| r.ok());

    let mut paused = false;

    'main: loop {
        // Traite toutes les commandes en attente
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                PipelineCommand::Stop   => break 'main,
                PipelineCommand::Pause  => paused = true,
                PipelineCommand::Resume => paused = false,
                PipelineCommand::Seek(pos) => {
                    ctx.seek(pos)?;
                    // Vide les buffers internes des décodeurs pour éviter les artefacts post-seek
                    if let Some(dec) = &mut video_dec {
                        let _ = dec.send_eof();
                        while dec.receive_frame().ok().flatten().is_some() {}
                    }
                    if let Some(dec) = &mut audio_dec {
                        let _ = dec.send_eof();
                        while dec.receive_frame().ok().flatten().is_some() {}
                    }
                    // Reconstruit les décodeurs pour repartir d'un état propre
                    video_dec = v_idx
                        .and_then(|_| ctx.build_video_decoder().ok())
                        .and_then(|d| VideoDecoder::new(d, v_tb).ok());
                    audio_dec = a_idx
                        .and_then(|_| ctx.build_audio_decoder().ok())
                        .and_then(|d| AudioDecoder::new(d, a_tb).ok());
                }
                _ => {}
            }
        }

        if paused {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }

        // Backpressure vidéo uniquement — l'audio doit TOUJOURS avancer
        if video_tx.is_full() {
            std::thread::sleep(std::time::Duration::from_millis(2));
            continue;
        }

        let mut packet = ffmpeg::Packet::empty();
        match packet.read(&mut ctx.format_ctx) {
            Ok(_) => {}
            Err(ffmpeg::Error::Eof) => {
                flush_decoders(&mut video_dec, &mut audio_dec, &video_tx, &audio_tx);
                let _ = event_tx.send(PipelineEvent::EndOfStream);
                break 'main;
            }
            Err(e) => {
                let _ = event_tx.send(PipelineEvent::Error(e.to_string()));
                break 'main;
            }
        }

        let stream_idx = packet.stream();

        if Some(stream_idx) == v_idx {
            if let Some(dec) = &mut video_dec {
                let _ = dec.send_packet(&packet);
                while let Ok(Some(frame)) = dec.receive_frame() {
                    // Vidéo : on peut sauter des frames si la queue est pleine
                    let _ = video_tx.try_send(frame);
                }
            }
        } else if Some(stream_idx) == a_idx {
            if let Some(dec) = &mut audio_dec {
                let _ = dec.send_packet(&packet);
                while let Ok(Some(frame)) = dec.receive_frame() {
                    let pos = frame.pts_secs;
                    let _ = event_tx.try_send(PipelineEvent::PositionChanged(pos));
                    // Audio : on attend si nécessaire — ne jamais dropper de frame audio
                    let _ = audio_tx.send_timeout(
                        frame,
                        std::time::Duration::from_millis(200),
                    );
                }
            }
        }
    }

    Ok(())
}

fn flush_decoders(
    video_dec: &mut Option<VideoDecoder>,
    audio_dec: &mut Option<AudioDecoder>,
    video_tx:  &Sender<DecodedVideoFrame>,
    audio_tx:  &Sender<DecodedAudioFrame>,
) {
    if let Some(dec) = video_dec {
        let _ = dec.send_eof();
        while let Ok(Some(f)) = dec.receive_frame() {
            let _ = video_tx.try_send(f);
        }
    }
    if let Some(dec) = audio_dec {
        let _ = dec.send_eof();
        while let Ok(Some(f)) = dec.receive_frame() {
            let _ = audio_tx.send_timeout(f, std::time::Duration::from_millis(200));
        }
    }
}
