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

    // Indexe tous les flux audio disponibles pour le changement de piste
    let all_audio_idx: Vec<usize> = ctx.format_ctx
        .streams()
        .filter(|s| s.parameters().medium() == ffmpeg::media::Type::Audio)
        .map(|s| s.index())
        .collect();

    // Indexe tous les flux subtitle disponibles pour le changement de piste
    let all_sub_idx: Vec<usize> = ctx.format_ctx
        .streams()
        .filter(|s| s.parameters().medium() == ffmpeg::media::Type::Subtitle)
        .map(|s| s.index())
        .collect();

    let v_idx = ctx.video_stream_idx;
    let mut a_idx = ctx.audio_stream_idx;
    // Index du flux subtitle actif (None = sous-titres intégrés désactivés)
    let mut s_idx: Option<usize> = None;

    let v_tb = v_idx.and_then(|i| {
        ctx.format_ctx.stream(i).map(|s| {
            s.time_base().numerator() as f64 / s.time_base().denominator().max(1) as f64
        })
    }).unwrap_or(0.0);
    let mut a_tb = a_idx.and_then(|i| {
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

    // Décodeur subtitle intégré — construit à la demande lors de SelectSubtitleTrack
    let mut subtitle_dec: Option<ffmpeg::codec::decoder::Subtitle> = None;
    let mut s_tb = 0.0f64;

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
                PipelineCommand::SelectAudioTrack(track) => {
                    if let Some(&new_idx) = all_audio_idx.get(track) {
                        // Flush et reconstruit le décodeur audio pour la nouvelle piste
                        if let Some(dec) = &mut audio_dec {
                            let _ = dec.send_eof();
                            while dec.receive_frame().ok().flatten().is_some() {}
                        }
                        a_tb = ctx.format_ctx.stream(new_idx)
                            .map(|s| s.time_base().numerator() as f64 / s.time_base().denominator().max(1) as f64)
                            .unwrap_or(0.0);
                        a_idx = Some(new_idx);
                        ctx.audio_stream_idx = a_idx;
                        audio_dec = ctx.build_audio_decoder_for(new_idx)
                            .ok()
                            .and_then(|d| AudioDecoder::new(d, a_tb).ok());
                        log::info!("audio track switched → stream {new_idx}");
                    }
                }
                PipelineCommand::SelectSubtitleTrack(track_opt) => {
                    match track_opt {
                        None => {
                            // Désactive les sous-titres intégrés
                            s_idx = None;
                            subtitle_dec = None;
                            log::info!("embedded subtitles disabled");
                        }
                        Some(track) => {
                            if let Some(&new_idx) = all_sub_idx.get(track) {
                                s_idx = Some(new_idx);
                                // Construit le décodeur subtitle pour la nouvelle piste
                                subtitle_dec = None;
                                if let Some(st) = ctx.format_ctx.stream(new_idx) {
                                    s_tb = st.time_base().numerator() as f64
                                        / st.time_base().denominator().max(1) as f64;
                                    if let Ok(codec_ctx) = ffmpeg::codec::context::Context::from_parameters(
                                        st.parameters()
                                    ) {
                                        subtitle_dec = codec_ctx.decoder().subtitle().ok();
                                    }
                                }
                                if subtitle_dec.is_some() {
                                    log::info!("subtitle track switched → stream {new_idx}");
                                } else {
                                    log::warn!("subtitle track {new_idx}: codec init failed (bitmap/PGS not supported)");
                                }
                            } else {
                                log::warn!("subtitle track {track} out of range (max {})", all_sub_idx.len());
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if paused {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }

        // NOTE : pas de check video_tx.is_full() ici — le faire bloquerait l'audio et
        // empêcherait le premier PositionChanged d'arriver, laissant le player en Loading
        // indéfiniment (deadlock). Le try_send ci-dessous gère l'overflow silencieusement.

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
                    // PositionChanged depuis la vidéo aussi : garantit Loading→Playing
                    // même si l'audio n'a pas encore produit de frame (ex: début I-frame lourd)
                    let _ = event_tx.try_send(PipelineEvent::PositionChanged(frame.pts_secs));
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
        } else if s_idx.is_some() && Some(stream_idx) == s_idx {
            // Décode les paquets de sous-titres intégrés via le décodeur initialisé
            if let Some(dec) = &mut subtitle_dec {
                let pts_start = packet.pts().unwrap_or(0).max(0) as f64 * s_tb;
                let duration_secs = packet.duration() as f64 * s_tb;
                let pts_end = pts_start + duration_secs.max(1.0);

                let mut subtitle = ffmpeg::Subtitle::new();
                if dec.decode(&packet, &mut subtitle) == Ok(true) {
                    let text = collect_subtitle_text(&subtitle);
                    if !text.is_empty() {
                        let _ = event_tx.try_send(
                            PipelineEvent::SubtitleLine(text, pts_start, pts_end)
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

/// Extrait le texte brut d'un paquet subtitle ffmpeg (SRT/ASS/WebVTT/HDMV-PGS partiellement).
fn collect_subtitle_text(subtitle: &ffmpeg::Subtitle) -> String {
    use ffmpeg::subtitle::Rect;
    let mut parts: Vec<String> = Vec::new();
    for rect in subtitle.rects() {
        match rect {
            Rect::Text(t) => {
                let raw = t.get();
                // Supprime les balises HTML basiques (<i>, <b>, etc.) souvent présentes en SRT
                let clean = strip_basic_tags(raw);
                if !clean.trim().is_empty() {
                    parts.push(clean.trim().to_string());
                }
            }
            Rect::Ass(a) => {
                // Ligne ASS : format "Layer,Start,End,Style,Name,MLeft,MRight,MVert,Effect,Text"
                let line = a.get();
                let text = line.splitn(10, ',').nth(9).unwrap_or("").trim().to_string();
                // Supprime les overrides ASS {…}
                let clean = strip_ass_overrides(&text);
                if !clean.trim().is_empty() {
                    parts.push(clean.trim().to_string());
                }
            }
            _ => {} // Rect::Bitmap (PGS/VOBSUB) — pas de texte extractible
        }
    }
    parts.join("\n")
}

fn strip_basic_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out
}

fn strip_ass_overrides(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    // Remplace les sauts de ligne ASS \N et \n
    out.replace("\\N", "\n").replace("\\n", "\n")
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
