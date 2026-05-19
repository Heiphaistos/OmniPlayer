use anyhow::{Context as _, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{unbounded, Receiver, Sender};
use omni_core::decoder::DecodedAudioFrame;
use ringbuf::{HeapConsumer, HeapProducer, HeapRb};
use std::sync::{
    atomic::{AtomicU32, AtomicU64, AtomicBool, Ordering},
    Arc,
};

use crate::resampler::AudioResampler;

const RING_SECS: usize = 8;  // 8 s de buffer pour absorber les rafales

pub struct AudioEngine {
    _stream:     cpal::Stream,
    sender:      Sender<DecodedAudioFrame>,
    volume:      Arc<AtomicU32>,
    paused:      Arc<AtomicBool>,
    device_rate: u32,
    channels:    usize,
    ring_level:  Arc<AtomicU64>,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        let host   = cpal::default_host();
        let device = host
            .default_output_device()
            .context("aucun périphérique audio")?;
        let config = device
            .default_output_config()
            .context("config audio par défaut")?;

        log::info!("audio device: {:?} — {:?}", device.name(), config);

        let device_rate = config.sample_rate().0;
        let channels    = config.channels() as usize;
        let capacity    = device_rate as usize * channels * RING_SECS;

        let rb: HeapRb<f32> = HeapRb::new(capacity);
        let (producer, consumer) = rb.split();

        let volume     = Arc::new(AtomicU32::new(1.0f32.to_bits()));
        let paused     = Arc::new(AtomicBool::new(false));
        let ring_level = Arc::new(AtomicU64::new(0));

        // Canal non-borné : push_frame ne bloque ni ne droppe jamais de frame
        let (tx, rx) = unbounded::<DecodedAudioFrame>();

        {
            let ring_level2 = ring_level.clone();
            std::thread::Builder::new()
                .name("audio-fill".into())
                .spawn(move || fill_ring(rx, producer, device_rate, channels, ring_level2))?;
        }

        let vol_w      = volume.clone();
        let paused_w   = paused.clone();
        let rl_w       = ring_level.clone();
        let fmt        = config.sample_format();
        let cfg: cpal::StreamConfig = config.into();

        let stream = build_stream(&device, &cfg, fmt, consumer, vol_w, paused_w, rl_w)?;
        stream.play().context("play stream audio")?;

        Ok(Self {
            _stream: stream,
            sender: tx,
            volume,
            paused,
            device_rate,
            channels,
            ring_level,
        })
    }

    pub fn sample_rate(&self) -> u32 { self.device_rate }

    /// Secondes d'audio dans le ring buffer (mis à jour côté CPAL — valeur fraîche).
    pub fn buffered_secs(&self) -> f64 {
        let samples = self.ring_level.load(Ordering::Acquire) as f64;
        samples / (self.device_rate as f64 * self.channels as f64)
    }

    /// Pousse un frame décodé — canal non-borné, jamais de drop.
    pub fn push_frame(&self, frame: DecodedAudioFrame) {
        let _ = self.sender.send(frame);
    }

    pub fn set_volume(&self, v: f32) {
        self.volume.store(v.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
    }

    pub fn set_paused(&self, p: bool) {
        self.paused.store(p, Ordering::Relaxed);
    }
}

// ─── Construction du stream CPAL ────────────────────────────────────────────

fn build_stream(
    device:     &cpal::Device,
    cfg:        &cpal::StreamConfig,
    fmt:        cpal::SampleFormat,
    consumer:   HeapConsumer<f32>,
    volume:     Arc<AtomicU32>,
    paused:     Arc<AtomicBool>,
    ring_level: Arc<AtomicU64>,
) -> Result<cpal::Stream> {
    let err_fn = |e: cpal::StreamError| log::error!("cpal error: {e}");

    // Consumer protégé par Mutex léger — seul CPAL y accède, jamais contention
    let cons = Arc::new(parking_lot::Mutex::new(consumer));

    let stream = match fmt {
        cpal::SampleFormat::F32 => {
            let cons_w = cons.clone();
            let vol_w  = volume.clone();
            let pau_w  = paused.clone();
            let rl_w   = ring_level.clone();
            device.build_output_stream(cfg, move |data: &mut [f32], _| {
                if pau_w.load(Ordering::Relaxed) { data.fill(0.0); return; }
                let mut c = cons_w.lock();
                let n = c.pop_slice(data);
                let remaining = c.len();
                drop(c);
                rl_w.store(remaining as u64, Ordering::Release);
                if n < data.len() { data[n..].fill(0.0); }
                let vol = f32::from_bits(vol_w.load(Ordering::Relaxed));
                if (vol - 1.0).abs() > 0.001 {
                    for s in data.iter_mut() { *s *= vol; }
                }
            }, err_fn, None)?
        }
        cpal::SampleFormat::I16 => {
            let cons_w = cons.clone();
            let vol_w  = volume.clone();
            let pau_w  = paused.clone();
            let rl_w   = ring_level.clone();
            // Pre-allocated scratch: grows to max callback size, never reallocates after warm-up.
            let mut scratch = Vec::<f32>::new();
            device.build_output_stream(cfg, move |data: &mut [i16], _| {
                if pau_w.load(Ordering::Relaxed) { data.fill(0); return; }
                scratch.resize(data.len(), 0.0);
                let mut c = cons_w.lock();
                let n = c.pop_slice(&mut scratch);
                let remaining = c.len();
                drop(c);
                rl_w.store(remaining as u64, Ordering::Release);
                if n < scratch.len() { scratch[n..].fill(0.0); }
                let vol = f32::from_bits(vol_w.load(Ordering::Relaxed));
                for (o, s) in data.iter_mut().zip(scratch.iter()) {
                    *o = ((s * vol).clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                }
            }, err_fn, None)?
        }
        cpal::SampleFormat::U16 => {
            let cons_w = cons.clone();
            let vol_w  = volume.clone();
            let pau_w  = paused.clone();
            let rl_w   = ring_level.clone();
            // Pre-allocated scratch: grows to max callback size, never reallocates after warm-up.
            let mut scratch = Vec::<f32>::new();
            device.build_output_stream(cfg, move |data: &mut [u16], _| {
                if pau_w.load(Ordering::Relaxed) { data.fill(32768); return; }
                scratch.resize(data.len(), 0.0);
                let mut c = cons_w.lock();
                let n = c.pop_slice(&mut scratch);
                let remaining = c.len();
                drop(c);
                rl_w.store(remaining as u64, Ordering::Release);
                if n < scratch.len() { scratch[n..].fill(0.0); }
                let vol = f32::from_bits(vol_w.load(Ordering::Relaxed));
                for (o, s) in data.iter_mut().zip(scratch.iter()) {
                    *o = (((s * vol).clamp(-1.0, 1.0) + 1.0) * 0.5 * u16::MAX as f32) as u16;
                }
            }, err_fn, None)?
        }
        cpal::SampleFormat::I32 => {
            let cons_w = cons.clone();
            let vol_w  = volume.clone();
            let pau_w  = paused.clone();
            let rl_w   = ring_level.clone();
            let mut scratch = Vec::<f32>::new();
            device.build_output_stream(cfg, move |data: &mut [i32], _| {
                if pau_w.load(Ordering::Relaxed) { data.fill(0); return; }
                scratch.resize(data.len(), 0.0);
                let mut c = cons_w.lock();
                let n = c.pop_slice(&mut scratch);
                let remaining = c.len();
                drop(c);
                rl_w.store(remaining as u64, Ordering::Release);
                if n < scratch.len() { scratch[n..].fill(0.0); }
                let vol = f32::from_bits(vol_w.load(Ordering::Relaxed));
                for (o, s) in data.iter_mut().zip(scratch.iter()) {
                    *o = ((*s * vol).clamp(-1.0, 1.0) * i32::MAX as f32) as i32;
                }
            }, err_fn, None)?
        }
        cpal::SampleFormat::F64 => {
            let cons_w = cons.clone();
            let vol_w  = volume.clone();
            let pau_w  = paused.clone();
            let rl_w   = ring_level.clone();
            let mut scratch = Vec::<f32>::new();
            device.build_output_stream(cfg, move |data: &mut [f64], _| {
                if pau_w.load(Ordering::Relaxed) { data.fill(0.0); return; }
                scratch.resize(data.len(), 0.0);
                let mut c = cons_w.lock();
                let n = c.pop_slice(&mut scratch);
                let remaining = c.len();
                drop(c);
                rl_w.store(remaining as u64, Ordering::Release);
                if n < scratch.len() { scratch[n..].fill(0.0); }
                let vol = f32::from_bits(vol_w.load(Ordering::Relaxed));
                for (o, s) in data.iter_mut().zip(scratch.iter()) {
                    *o = ((*s * vol).clamp(-1.0, 1.0)) as f64;
                }
            }, err_fn, None)?
        }
        cpal::SampleFormat::U32 => {
            let cons_w = cons.clone();
            let vol_w  = volume.clone();
            let pau_w  = paused.clone();
            let rl_w   = ring_level.clone();
            let mut scratch = Vec::<f32>::new();
            device.build_output_stream(cfg, move |data: &mut [u32], _| {
                if pau_w.load(Ordering::Relaxed) { data.fill(u32::MAX / 2); return; }
                scratch.resize(data.len(), 0.0);
                let mut c = cons_w.lock();
                let n = c.pop_slice(&mut scratch);
                let remaining = c.len();
                drop(c);
                rl_w.store(remaining as u64, Ordering::Release);
                if n < scratch.len() { scratch[n..].fill(0.0); }
                let vol = f32::from_bits(vol_w.load(Ordering::Relaxed));
                for (o, s) in data.iter_mut().zip(scratch.iter()) {
                    *o = (((*s * vol).clamp(-1.0, 1.0) + 1.0) * 0.5 * u32::MAX as f32) as u32;
                }
            }, err_fn, None)?
        }
        cpal::SampleFormat::I8 => {
            let cons_w = cons.clone();
            let vol_w  = volume.clone();
            let pau_w  = paused.clone();
            let rl_w   = ring_level.clone();
            let mut scratch = Vec::<f32>::new();
            device.build_output_stream(cfg, move |data: &mut [i8], _| {
                if pau_w.load(Ordering::Relaxed) { data.fill(0); return; }
                scratch.resize(data.len(), 0.0);
                let mut c = cons_w.lock();
                let n = c.pop_slice(&mut scratch);
                let remaining = c.len();
                drop(c);
                rl_w.store(remaining as u64, Ordering::Release);
                if n < scratch.len() { scratch[n..].fill(0.0); }
                let vol = f32::from_bits(vol_w.load(Ordering::Relaxed));
                for (o, s) in data.iter_mut().zip(scratch.iter()) {
                    *o = ((*s * vol).clamp(-1.0, 1.0) * i8::MAX as f32) as i8;
                }
            }, err_fn, None)?
        }
        cpal::SampleFormat::U8 => {
            let cons_w = cons.clone();
            let vol_w  = volume.clone();
            let pau_w  = paused.clone();
            let rl_w   = ring_level.clone();
            let mut scratch = Vec::<f32>::new();
            device.build_output_stream(cfg, move |data: &mut [u8], _| {
                if pau_w.load(Ordering::Relaxed) { data.fill(128); return; }
                scratch.resize(data.len(), 0.0);
                let mut c = cons_w.lock();
                let n = c.pop_slice(&mut scratch);
                let remaining = c.len();
                drop(c);
                rl_w.store(remaining as u64, Ordering::Release);
                if n < scratch.len() { scratch[n..].fill(0.0); }
                let vol = f32::from_bits(vol_w.load(Ordering::Relaxed));
                for (o, s) in data.iter_mut().zip(scratch.iter()) {
                    *o = (((*s * vol).clamp(-1.0, 1.0) + 1.0) * 0.5 * u8::MAX as f32) as u8;
                }
            }, err_fn, None)?
        }
        fmt => anyhow::bail!("format CPAL non géré: {fmt:?}"),
    };

    Ok(stream)
}

// ─── Thread fill_ring ────────────────────────────────────────────────────────

fn fill_ring(
    rx:           Receiver<DecodedAudioFrame>,
    mut producer: HeapProducer<f32>,
    device_rate:  u32,
    dev_ch:       usize,
    ring_level:   Arc<AtomicU64>,
) {
    let mut resampler: Option<AudioResampler> = None;

    for frame in rx {
        let in_ch   = frame.channels as usize;
        let in_rate = frame.sample_rate;

        let stereo = if in_ch == dev_ch {
            frame.samples.clone()
        } else {
            downmix_to_stereo(&frame.samples, in_ch)
        };

        let final_samples = if in_rate == device_rate {
            stereo
        } else {
            let out_ch = dev_ch.min(2);
            let needs_new = resampler.as_ref()
                .map(|r| r.in_rate() != in_rate || r.out_rate() != device_rate)
                .unwrap_or(true);
            if needs_new {
                resampler = AudioResampler::new(in_rate, device_rate, out_ch).ok();
            }
            if let Some(r) = &mut resampler {
                r.process_interleaved(&stereo).unwrap_or(stereo)
            } else {
                stereo
            }
        };

        let pushed = producer.push_slice(&final_samples);
        // ring_level mis à jour ici aussi pour le fill ; CPAL met à jour côté consommation
        ring_level.store(producer.len() as u64, Ordering::Release);

        if pushed < final_samples.len() {
            log::warn!("audio ring overflow: dropped {} samples", final_samples.len() - pushed);
        }
    }
}

// ─── Downmix N canaux → stéréo ───────────────────────────────────────────────

fn downmix_to_stereo(samples: &[f32], in_ch: usize) -> Vec<f32> {
    if in_ch == 0 { return Vec::new(); }
    let frames = samples.len() / in_ch;
    let mut out = Vec::with_capacity(frames * 2);

    for f in 0..frames {
        let b = f * in_ch;
        let (l, r) = match in_ch {
            1 => { let m = samples[b]; (m, m) }
            2 => (samples[b], samples[b + 1]),
            6 => {
                let (fl, fr, fc, bl, br) =
                    (samples[b], samples[b+1], samples[b+2], samples[b+4], samples[b+5]);
                ((fl + fc*0.707 + bl*0.707).clamp(-1.0, 1.0),
                 (fr + fc*0.707 + br*0.707).clamp(-1.0, 1.0))
            }
            8 => {
                let (fl, fr, fc, bl, br, sl, sr) =
                    (samples[b], samples[b+1], samples[b+2], samples[b+4],
                     samples[b+5], samples[b+6], samples[b+7]);
                ((fl + fc*0.707 + bl*0.5 + sl*0.707).clamp(-1.0, 1.0),
                 (fr + fc*0.707 + br*0.5 + sr*0.707).clamp(-1.0, 1.0))
            }
            n => {
                let (mut ls, mut rs) = (0f32, 0f32);
                for ch in 0..n {
                    if ch % 2 == 0 { ls += samples[b + ch]; }
                    else           { rs += samples[b + ch]; }
                }
                let h = (n / 2).max(1) as f32;
                ((ls / h).clamp(-1.0, 1.0), (rs / h).clamp(-1.0, 1.0))
            }
        };
        out.push(l);
        out.push(r);
    }
    out
}
