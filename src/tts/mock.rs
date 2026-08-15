use super::TTSEngine;

pub struct MockTTSEngine {
    sample_rate: u32,
}

impl MockTTSEngine {
    pub fn new() -> Self {
        Self { sample_rate: 22050 }
    }
}

impl TTSEngine for MockTTSEngine {
    fn synthesize(&mut self, text: &str, _speed: f32) -> Result<(u32, Vec<f32>), String> {
        // Generate a 440 Hz sinusoidal beep with duration proportional to word count
        let words_count = text.split_whitespace().count().max(1);
        let duration_secs = (words_count as f32 * 0.25).clamp(0.4, 4.0);
        let num_samples = (self.sample_rate as f32 * duration_secs) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        let freq = 440.0;
        for i in 0..num_samples {
            let t = i as f32 / self.sample_rate as f32;
            let envelope = (1.0 - (t / duration_secs)).max(0.0).min(1.0);
            let sample = (t * freq * 2.0 * std::f32::consts::PI).sin() * 0.3 * envelope;
            samples.push(sample);
        }

        Ok((self.sample_rate, samples))
    }

    fn reload(&mut self, _model_path: &str, _config_path: &str, _speaker_id: i64) -> Result<(), String> {
        Ok(())
    }
}
