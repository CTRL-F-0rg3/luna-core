use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;
use std::time::Duration;

pub const TARGET_RATE: u32 = 16_000;

#[derive(Clone)]
pub struct AudioConfig {
    pub silence_ms: u64,
    pub min_utterance_ms: u64,
    pub max_utterance_ms: u64,
    pub rms_threshold: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            silence_ms: 800,
            min_utterance_ms: 400,
            max_utterance_ms: 30_000,
            rms_threshold: 0.008,
        }
    }
}

pub struct AudioCapture {
    _stream: cpal::Stream,
    stop: Arc<AtomicBool>,
}

impl AudioCapture {
    pub fn start<F>(cfg: AudioConfig, mut on_utterance: F) -> Result<Self, String>
    where
        F: FnMut(Vec<f32>) + Send + 'static,
    {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "brak domyślnego mikrofonu".to_string())?;
        let supported = device
            .default_input_config()
            .map_err(|e| format!("default_input_config: {e}"))?;
        let in_rate = supported.sample_rate().0;
        let in_channels = supported.channels() as usize;
        let fmt = supported.sample_format();
        let stream_config: StreamConfig = supported.into();

        eprintln!(
            "[audio] device='{}' rate={} ch={} fmt={:?}",
            device.name().unwrap_or_default(),
            in_rate,
            in_channels,
            fmt
        );

        let (tx_raw, rx_raw) = sync_channel::<Vec<f32>>(64);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_cb = stop.clone();

        let stream = match fmt {
            SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, tx_raw, stop_cb),
            SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, tx_raw, stop_cb),
            SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, tx_raw, stop_cb),
            other => return Err(format!("nieobsługiwany format: {other:?}")),
        }?;

        stream.play().map_err(|e| format!("play: {e}"))?;

        let stop_worker = stop.clone();
        std::thread::spawn(move || {
            worker_loop(rx_raw, in_rate, in_channels, cfg, stop_worker, move |utt| {
                on_utterance(utt);
            });
        });

        Ok(Self { _stream: stream, stop })
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    tx: SyncSender<Vec<f32>>,
    stop: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + Send + 'static,   // <-- ZMIANA: Sample → cpal::SizedSample
    f32: cpal::FromSample<T>,
{
    let err_fn = |e| eprintln!("[audio] stream error: {e}");
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if stop.load(Ordering::Relaxed) { return; }
                let v: Vec<f32> = data.iter().map(|s| (*s).to_sample::<f32>()).collect();
                let _ = tx.try_send(v);
            },
            err_fn,
            None,
        )
        .map_err(|e| format!("build_input_stream: {e}"))
}

fn worker_loop<F>(
    rx: Receiver<Vec<f32>>,
    in_rate: u32,
    in_channels: usize,
    cfg: AudioConfig,
    stop: Arc<AtomicBool>,
    mut emit: F,
) where
    F: FnMut(Vec<f32>),
{
    let step = in_rate as f64 / TARGET_RATE as f64;
    let silence_samples = ((cfg.silence_ms * TARGET_RATE as u64) / 1000) as usize;
    let min_samples = ((cfg.min_utterance_ms * TARGET_RATE as u64) / 1000) as usize;
    let max_samples = ((cfg.max_utterance_ms * TARGET_RATE as u64) / 1000) as usize;
    let preroll = TARGET_RATE as usize / 2;

    let mut buf: Vec<f32> = Vec::with_capacity(max_samples + silence_samples);
    let mut speaking = false;
    let mut silence: usize = 0;
    let mut accum: f64 = 0.0;

    while !stop.load(Ordering::Relaxed) {
        let raw = match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let mono: Vec<f32> = if in_channels == 1 {
            raw
        } else {
            raw.chunks(in_channels)
                .map(|c| c.iter().sum::<f32>() / in_channels as f32)
                .collect()
        };

        let mut resampled: Vec<f32> =
            Vec::with_capacity((mono.len() as f64 / step) as usize + 2);
        for &s in &mono {
            accum += 1.0;
            if accum >= step {
                accum -= step;
                resampled.push(s);
            }
        }
        if resampled.is_empty() { continue; }

        let rms =
            (resampled.iter().map(|x| x * x).sum::<f32>() / resampled.len() as f32).sqrt();
        let is_speech = rms > cfg.rms_threshold;

        buf.extend_from_slice(&resampled);
        if buf.len() > max_samples + silence_samples {
            let drop = buf.len() - (max_samples + silence_samples);
            buf.drain(0..drop);
        }

        if is_speech {
            silence = 0;
            if !speaking {
                speaking = true;
                eprintln!("[audio] ▶ speech (rms={:.4})", rms);
            }
        } else if speaking {
            silence += resampled.len();
            if silence >= silence_samples {
                if buf.len() >= min_samples {
                    let trim = silence.min(buf.len());
                    let end = buf.len() - trim;
                    let utt = buf[..end].to_vec();
                    if utt.len() >= min_samples {
                        eprintln!(
                            "[audio] ■ utterance {:.2}s",
                            utt.len() as f32 / TARGET_RATE as f32
                        );
                        emit(utt);
                    }
                }
                buf.clear();
                speaking = false;
                silence = 0;
            }
        } else if buf.len() > preroll {
            let drop = buf.len() - preroll;
            buf.drain(0..drop);
        }
    }
}
