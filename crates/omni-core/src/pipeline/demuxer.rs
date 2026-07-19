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

    // Décodeurs sous-titres : TOUTES les pistes texte dès le départ. Les paquets ne
    // sont lus qu'une seule fois (en avance sur la lecture) — une activation tardive
    // de la piste doit retrouver les cues déjà passés. Le player filtre par piste.
    // (stream_idx, ordinal piste, décodeur, time_base)
    let mut sub_decs: Vec<(usize, usize, ffmpeg::codec::decoder::Subtitle, f64)> = Vec::new();
    for (ord, &si) in all_sub_idx.iter().enumerate() {
        if let Some(st) = ctx.format_ctx.stream(si) {
            let tb = st.time_base().numerator() as f64
                / st.time_base().denominator().max(1) as f64;
            match ffmpeg::codec::context::Context::from_parameters(st.parameters())
                .ok()
                .and_then(|cc| cc.decoder().subtitle().ok())
            {
                Some(dec) => sub_decs.push((si, ord, dec, tb)),
                None => log::warn!("piste sous-titre {si}: codec non supporté (bitmap PGS/VOBSUB ?)"),
            }
        }
    }

    let mut paused = false;
    // Après un seek en pause : décoder une frame vidéo pour rafraîchir l'affichage
    let mut preview_after_seek = false;
    // Après seek : av_seek_frame se cale sur la keyframe ≤ cible. On décode depuis
    // la keyframe mais on jette les frames jusqu'au PTS cible (précision à la frame).
    let mut v_skip_until: Option<f64> = None;
    let mut a_skip_until: Option<f64> = None;

    'main: loop {
        // Traite toutes les commandes en attente
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                PipelineCommand::Stop   => break 'main,
                PipelineCommand::Pause  => paused = true,
                PipelineCommand::Resume => paused = false,
                PipelineCommand::Seek(pos) => {
                    // Un seek raté (flux réseau, format exotique) ne doit pas tuer la
                    // lecture : on log et on continue à la position courante.
                    if let Err(e) = ctx.seek(pos) {
                        log::warn!("seek ignoré: {e:#}");
                        continue;
                    }
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
                    preview_after_seek = paused;
                    v_skip_until = Some(pos);
                    a_skip_until = Some(pos);
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
                    // Toutes les pistes texte sont décodées en continu — la sélection
                    // est purement un filtre côté player. Log informatif seulement.
                    log::info!("subtitle track selection: {track_opt:?}");
                }
                _ => {}
            }
        }

        if paused {
            if preview_after_seek {
                // Seek pendant la pause : décode UNE frame vidéo à la nouvelle
                // position pour que l'affichage se rafraîchisse (les paquets
                // audio/sous-titres sont ignorés).
                preview_after_seek = false;
                if let (Some(vi), Some(dec)) = (v_idx, video_dec.as_mut()) {
                    'preview: for _ in 0..400 {
                        let mut pkt = ffmpeg::Packet::empty();
                        if pkt.read(&mut ctx.format_ctx).is_err() { break; }
                        if pkt.stream() == vi {
                            let _ = dec.send_packet(&pkt);
                            let mut sent = false;
                            while let Ok(Some(frame)) = dec.receive_frame() {
                                // Même logique post-seek : atteindre le PTS cible
                                if let Some(su) = v_skip_until {
                                    if frame.pts_secs < su - 0.05 { continue; }
                                    v_skip_until = None;
                                }
                                let _ = event_tx.try_send(
                                    PipelineEvent::PositionChanged(frame.pts_secs));
                                let _ = video_tx.try_send(frame);
                                sent = true;
                            }
                            if sent { break 'preview; }
                        }
                    }
                }
            } else {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            continue;
        }

        // Régulation du débit : queues aval pleines = on attend au lieu de décoder tout
        // le fichier en avance (sinon drops massifs de frames vidéo et overflow du ring
        // audio). Pas de deadlock : pump_audio draine toujours, pump_video draine aussi
        // en Loading, et PositionChanged est émis dès qu'une frame passe.
        if video_tx.is_full() || audio_tx.is_full() {
            std::thread::sleep(std::time::Duration::from_millis(4));
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
                    // Post-seek : jeter les frames entre la keyframe et la cible
                    if let Some(su) = v_skip_until {
                        if frame.pts_secs < su - 0.05 { continue; }
                        v_skip_until = None;
                    }
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
                    // Post-seek : jeter l'audio entre la keyframe et la cible
                    if let Some(su) = a_skip_until {
                        if frame.pts_secs < su - 0.05 { continue; }
                        a_skip_until = None;
                    }
                    let pos = frame.pts_secs;
                    let _ = event_tx.try_send(PipelineEvent::PositionChanged(pos));
                    // Audio : on attend si nécessaire — ne jamais dropper de frame audio
                    let _ = audio_tx.send_timeout(
                        frame,
                        std::time::Duration::from_millis(200),
                    );
                }
            }
        } else if let Some((_, ord, dec, tb)) = sub_decs.iter_mut()
            .find(|(si, _, _, _)| *si == stream_idx)
        {
            // Décode les paquets sous-titres de toutes les pistes texte
            let pts_start = packet.pts().unwrap_or(0).max(0) as f64 * *tb;
            let duration_secs = packet.duration() as f64 * *tb;
            let pts_end = pts_start + duration_secs.max(1.0);

            let mut subtitle = ffmpeg::Subtitle::new();
            if dec.decode(&packet, &mut subtitle) == Ok(true) {
                let text = collect_subtitle_text(&subtitle);
                if !text.is_empty() {
                    let _ = event_tx.try_send(
                        PipelineEvent::SubtitleLine(*ord, text, pts_start, pts_end)
                    );
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
                // Le decoder subrip/ass de FFmpeg produit soit une ligne complète
                // "Dialogue: Layer,Start,End,Style,Name,..,Text", soit directement les
                // champs après le timing selon le codec. On extrait le texte après le
                // 8e champ « Effect » quand le préfixe Dialogue est présent, sinon on
                // prend tout. Robuste aux deux formes.
                let line = a.get();
                let body = line.strip_prefix("Dialogue:").unwrap_or(line);
                // Une ligne Dialogue a 9 champs avant le texte ; les événements bruts
                // du décodeur subrip n'ont que Layer + texte. On compte les virgules.
                let comma_count = body.matches(',').count();
                let text = if comma_count >= 9 {
                    body.splitn(10, ',').nth(9).unwrap_or("")
                } else {
                    // Format "ReadOrder,Layer,Style,Name,MarginL,MarginR,MarginV,Effect,Text"
                    // (subrip → ass : 8 champs avant le texte)
                    body.splitn(9, ',').last().unwrap_or(body)
                };
                let clean = strip_ass_overrides(text.trim());
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
