pub mod mock;
pub mod piper;

use std::path::Path;

pub trait TTSEngine: Send + Sync {
    /// Synthesizes text to PCM f32 audio samples and returns (sample_rate, samples)
    fn synthesize(&mut self, text: &str, speed: f32) -> Result<(u32, Vec<f32>), String>;
    fn reload(&mut self, model_path: &str, config_path: &str, speaker_id: i64) -> Result<(), String>;
}

/// Helper function to export PCM samples to a standard 16-bit PCM WAV file
pub fn export_wav_file<P: AsRef<Path>>(path: P, sample_rate: u32, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| format!("Failed to create WAV writer: {}", e))?;

    for &sample in samples {
        // Clamp and scale f32 to i16 range [-32767, 32767]
        let clamped = sample.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0) as i16;
        writer
            .write_sample(sample_i16)
            .map_err(|e| format!("Failed to write WAV sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV file: {}", e))?;

    Ok(())
}
