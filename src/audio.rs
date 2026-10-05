use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

fn err_fn(errored: Arc<AtomicBool>) -> impl FnMut(cpal::Error) + Send + 'static {
    move |e| {
        if !errored.swap(true, Ordering::SeqCst) {
            eprintln!("audio stream error: {e}");
            eprintln!("audio disabled; continuing without sound");
        }
    }
}

fn pop_sample(buffer: &Arc<Mutex<VecDeque<f32>>>) -> f32 {
    buffer
        .lock()
        .ok()
        .and_then(|mut b| b.pop_front())
        .unwrap_or(0.0)
}

pub struct Audio {
    pub sample_rate: u32,
    buffer: Arc<Mutex<VecDeque<f32>>>,
    errored: Arc<AtomicBool>,
    _stream: Option<cpal::Stream>,
}

const MAX_BUFFERED: usize = 8192;

impl Audio {
    pub fn new() -> Self {
        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => return Audio::silent(),
        };
        let supported = match device.default_output_config() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("no default audio config: {e}");
                return Audio::silent();
            }
        };

        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let sample_rate = config.sample_rate;
        let channels = config.channels as usize;
        let buffer: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));
        let errored = Arc::new(AtomicBool::new(false));

        let stream = match sample_format {
            SampleFormat::F32 => {
                let b = buffer.clone();
                device.build_output_stream(
                    config,
                    move |data: &mut [f32], _| {
                        for frame in data.chunks_mut(channels) {
                            let s = pop_sample(&b);
                            for out in frame.iter_mut() {
                                *out = s;
                            }
                        }
                    },
                    err_fn(errored.clone()),
                    None,
                )
            }
            SampleFormat::I16 => {
                let b = buffer.clone();
                device.build_output_stream(
                    config,
                    move |data: &mut [i16], _| {
                        for frame in data.chunks_mut(channels) {
                            let s = pop_sample(&b).clamp(-1.0, 1.0);
                            let v = (s * i16::MAX as f32) as i16;
                            for out in frame.iter_mut() {
                                *out = v;
                            }
                        }
                    },
                    err_fn(errored.clone()),
                    None,
                )
            }
            SampleFormat::U16 => {
                let b = buffer.clone();
                device.build_output_stream(
                    config,
                    move |data: &mut [u16], _| {
                        for frame in data.chunks_mut(channels) {
                            let s = pop_sample(&b).clamp(-1.0, 1.0);
                            let v = ((s * 0.5 + 0.5) * u16::MAX as f32) as u16;
                            for out in frame.iter_mut() {
                                *out = v;
                            }
                        }
                    },
                    err_fn(errored.clone()),
                    None,
                )
            }
            _ => {
                eprintln!("unsupported audio sample format {sample_format:?}, audio disabled");
                return Audio::silent();
            }
        };

        match stream {
            Ok(stream) => {
                if let Err(e) = stream.play() {
                    eprintln!("failed to start audio stream: {e}");
                }
                Audio {
                    sample_rate,
                    buffer,
                    errored,
                    _stream: Some(stream),
                }
            }
            Err(e) => {
                eprintln!("failed to build audio stream: {e}");
                Audio::silent_with_rate(sample_rate)
            }
        }
    }

    fn silent() -> Self {
        Self::silent_with_rate(44100)
    }

    fn silent_with_rate(rate: u32) -> Self {
        Audio {
            sample_rate: rate,
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            errored: Arc::new(AtomicBool::new(false)),
            _stream: None,
        }
    }

    pub fn push(&self, samples: &[f32]) {
        if let Ok(mut b) = self.buffer.lock() {
            b.extend(samples.iter().copied());
            while b.len() > MAX_BUFFERED {
                b.pop_front();
            }
        }
    }

    pub fn has_output(&self) -> bool {
        self._stream.is_some() && !self.errored.load(Ordering::SeqCst)
    }

    pub fn errored(&self) -> bool {
        self.errored.load(Ordering::SeqCst)
    }

    pub fn stop(&mut self) {
        self._stream = None;
    }

    pub fn buffered(&self) -> usize {
        self.buffer.lock().map(|b| b.len()).unwrap_or(0)
    }
}
