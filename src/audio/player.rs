use super::devices::AudioDeviceManager;
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tracing::{error, info};

pub struct AudioPlayer {
    _stream: Mutex<Option<OutputStream>>,
    stream_handle: Mutex<Option<OutputStreamHandle>>,
    sink: Mutex<Option<Sink>>,
    device_name: Mutex<String>,
    volume: Mutex<f32>,
    is_muted: AtomicBool,
    is_speaking: AtomicBool,
}

impl AudioPlayer {
    pub fn new(device_name: &str, initial_volume: f32) -> Self {
        let (stream, stream_handle) = Self::init_device(device_name);

        let sink = if let Some(ref handle) = stream_handle {
            Sink::try_new(handle).ok()
        } else {
            None
        };

        Self {
            _stream: Mutex::new(stream),
            stream_handle: Mutex::new(stream_handle),
            sink: Mutex::new(sink),
            device_name: Mutex::new(device_name.to_string()),
            volume: Mutex::new(initial_volume),
            is_muted: AtomicBool::new(false),
            is_speaking: AtomicBool::new(false),
        }
    }

    fn init_device(device_name: &str) -> (Option<OutputStream>, Option<OutputStreamHandle>) {
        if let Some(device) = AudioDeviceManager::get_device(device_name) {
            match OutputStream::try_from_device(&device) {
                Ok((stream, handle)) => (Some(stream), Some(handle)),
                Err(err) => {
                    error!("Failed to open audio stream from device '{}': {}", device_name, err);
                    // Fallback to default
                    match OutputStream::try_default() {
                        Ok((stream, handle)) => (Some(stream), Some(handle)),
                        Err(e) => {
                            error!("Failed to open default audio output stream: {}", e);
                            (None, None)
                        }
                    }
                }
            }
        } else {
            match OutputStream::try_default() {
                Ok((stream, handle)) => (Some(stream), Some(handle)),
                Err(e) => {
                    error!("Failed to open default audio output stream: {}", e);
                    (None, None)
                }
            }
        }
    }

    pub fn set_device(&self, device_name: &str) {
        let mut current_name = self.device_name.lock().unwrap();
        if *current_name == device_name {
            return;
        }
        *current_name = device_name.to_string();
        drop(current_name);

        self.stop();
        let (stream, handle) = Self::init_device(device_name);

        let mut stream_lock = self._stream.lock().unwrap();
        let mut handle_lock = self.stream_handle.lock().unwrap();
        *stream_lock = stream;
        *handle_lock = handle.clone();
        drop(stream_lock);
        drop(handle_lock);

        let mut sink_lock = self.sink.lock().unwrap();
        if let Some(ref h) = handle {
            *sink_lock = Sink::try_new(h).ok();
        } else {
            *sink_lock = None;
        }
    }

    pub fn play_samples(&self, sample_rate: u32, samples: &[f32], padding_sec: f32) -> Result<(), String> {
        if self.is_muted.load(Ordering::SeqCst) {
            info!("TTS is muted. Skipping playback.");
            return Ok(());
        }

        if samples.is_empty() {
            return Ok(());
        }

        // Add padding silence samples to end
        let padding_samples_count = (sample_rate as f32 * padding_sec.max(0.0)) as usize;
        let mut full_samples = Vec::with_capacity(samples.len() + padding_samples_count);
        full_samples.extend_from_slice(samples);
        full_samples.resize(samples.len() + padding_samples_count, 0.0f32);

        let vol = *self.volume.lock().unwrap();

        // Create fresh sink if needed or reuse
        let stream_handle_opt = self.stream_handle.lock().unwrap().clone();
        let sink = if let Some(ref handle) = stream_handle_opt {
            match Sink::try_new(handle) {
                Ok(s) => s,
                Err(e) => return Err(format!("Failed to create Rodio Sink: {}", e)),
            }
        } else {
            return Err("No active audio output stream handle available".to_string());
        };

        sink.set_volume(vol);
        let buffer = SamplesBuffer::new(1, sample_rate, full_samples);
        sink.append(buffer);

        {
            let mut sink_lock = self.sink.lock().unwrap();
            if let Some(old_sink) = sink_lock.take() {
                old_sink.stop();
            }
            *sink_lock = Some(sink);
        }

        self.is_speaking.store(true, Ordering::SeqCst);

        // Block or monitor until finished or stopped
        loop {
            std::thread::sleep(Duration::from_millis(50));
            let sink_lock = self.sink.lock().unwrap();
            if let Some(ref current_sink) = *sink_lock {
                if current_sink.empty() {
                    break;
                }
            } else {
                break;
            }
        }

        self.is_speaking.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub fn stop(&self) {
        let mut sink_lock = self.sink.lock().unwrap();
        if let Some(sink) = sink_lock.take() {
            sink.stop();
            info!("Playback stopped (Skipped).");
        }
        self.is_speaking.store(false, Ordering::SeqCst);
    }

    pub fn set_volume(&self, volume: f32) {
        let mut vol_lock = self.volume.lock().unwrap();
        *vol_lock = volume.clamp(0.0, 1.5);
        let vol = *vol_lock;
        drop(vol_lock);

        let sink_lock = self.sink.lock().unwrap();
        if let Some(ref sink) = *sink_lock {
            sink.set_volume(vol);
        }
    }

    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, Ordering::SeqCst);
        if muted {
            self.stop();
        }
    }

    pub fn is_muted(&self) -> bool {
        self.is_muted.load(Ordering::SeqCst)
    }

    pub fn is_speaking(&self) -> bool {
        self.is_speaking.load(Ordering::SeqCst)
    }
}

unsafe impl Send for AudioPlayer {}
unsafe impl Sync for AudioPlayer {}
