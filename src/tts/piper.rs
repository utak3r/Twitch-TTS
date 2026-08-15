use super::mock::MockTTSEngine;
use super::TTSEngine;
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tracing::{error, info, warn};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[derive(Debug, Deserialize, Clone)]
struct ModelConfig {
    audio: AudioConfig,
    #[serde(default)]
    inference: InferenceConfig,
}

#[derive(Debug, Deserialize, Clone)]
struct AudioConfig {
    sample_rate: u32,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct InferenceConfig {
    #[serde(default = "default_length_scale")]
    length_scale: f32,
}

fn default_length_scale() -> f32 {
    1.0
}

pub struct PiperEngine {
    config: Option<ModelConfig>,
    mock_fallback: MockTTSEngine,
    current_model: String,
    current_config: String,
    speaker_id: i64,
    piper_exe_path: Option<PathBuf>,
    espeak_data_path: Option<PathBuf>,
}

impl PiperEngine {
    pub fn new(model_path: &str, config_path: &str, speaker_id: i64) -> Self {
        let mut engine = Self {
            config: None,
            mock_fallback: MockTTSEngine::new(),
            current_model: model_path.to_string(),
            current_config: config_path.to_string(),
            speaker_id,
            piper_exe_path: None,
            espeak_data_path: None,
        };

        let _ = engine.reload(model_path, config_path, speaker_id);
        engine
    }

    fn find_piper_executable() -> Option<PathBuf> {
        // 1. Check local portable directory
        let local_paths = [
            PathBuf::from("piper").join("piper.exe"),
            PathBuf::from("piper_bin").join("piper").join("piper.exe"),
            PathBuf::from("piper_bin").join("piper.exe"),
            PathBuf::from("piper.exe"),
        ];

        for path in &local_paths {
            if path.exists() {
                return Some(path.clone());
            }
        }

        // 2. Check standard system locations
        let system_paths = [
            PathBuf::from(r"C:\Program Files\Piper\piper.exe"),
            PathBuf::from(r"C:\Program Files (x86)\Piper\piper.exe"),
        ];

        for path in &system_paths {
            if path.exists() {
                return Some(path.clone());
            }
        }

        // 3. Fallback to PATH
        Some(PathBuf::from("piper.exe"))
    }

    fn find_espeak_data() -> Option<PathBuf> {
        let local_paths = [
            PathBuf::from("piper").join("espeak-ng-data"),
            PathBuf::from("piper_bin").join("piper").join("espeak-ng-data"),
            PathBuf::from("espeak-ng-data"),
            PathBuf::from(r"C:\Program Files\eSpeak NG\espeak-ng-data"),
        ];

        for path in &local_paths {
            if path.exists() {
                return Some(path.clone());
            }
        }

        None
    }
}

impl TTSEngine for PiperEngine {
    fn synthesize(&mut self, text: &str, speed: f32) -> Result<(u32, Vec<f32>), String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            let sample_rate = self.config.as_ref().map(|c| c.audio.sample_rate).unwrap_or(22050);
            return Ok((sample_rate, Vec::new()));
        }

        if !Path::new(&self.current_model).exists() {
            warn!("Piper model not found at '{}'. Using mock fallback.", self.current_model);
            return self.mock_fallback.synthesize(text, speed);
        }

        let piper_exe = match self.piper_exe_path {
            Some(ref p) => p,
            None => {
                warn!("Piper executable not found. Using mock fallback.");
                return self.mock_fallback.synthesize(text, speed);
            }
        };

        let sample_rate = self.config.as_ref().map(|c| c.audio.sample_rate).unwrap_or(22050);
        let base_length_scale = self.config.as_ref().map(|c| c.inference.length_scale).unwrap_or(1.0);
        let length_scale = if speed > 0.0 {
            base_length_scale / speed
        } else {
            base_length_scale
        };

        // Construct Piper command
        let mut cmd = Command::new(piper_exe);
        cmd.arg("-m")
            .arg(&self.current_model)
            .arg("-c")
            .arg(&self.current_config)
            .arg("--output_raw")
            .arg("-s")
            .arg(self.speaker_id.to_string())
            .arg("--length_scale")
            .arg(format!("{:.3}", length_scale))
            .arg("-q");

        if let Some(ref espeak_data) = self.espeak_data_path {
            cmd.arg("--espeak_data").arg(espeak_data);
        }

        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to spawn piper process ({}): {}. Using mock fallback.", piper_exe.display(), e);
                return self.mock_fallback.synthesize(text, speed);
            }
        };

        // Write input text to stdin
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(trimmed.as_bytes());
            let _ = stdin.write_all(b"\n");
        }

        let output = match child.wait_with_output() {
            Ok(out) => out,
            Err(e) => {
                error!("Failed to read output from piper process: {}. Using mock fallback.", e);
                return self.mock_fallback.synthesize(text, speed);
            }
        };

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            error!("Piper exited with error status: {}. Using mock fallback.", err_msg);
            return self.mock_fallback.synthesize(text, speed);
        }

        // Convert raw 16-bit mono PCM bytes to f32 normalized samples [-1.0, 1.0]
        let raw_bytes = output.stdout;
        if raw_bytes.len() < 2 {
            warn!("Piper produced empty audio output. Using mock fallback.");
            return self.mock_fallback.synthesize(text, speed);
        }

        let mut samples = Vec::with_capacity(raw_bytes.len() / 2);
        for chunk in raw_bytes.chunks_exact(2) {
            let sample_i16 = i16::from_le_bytes([chunk[0], chunk[1]]);
            let sample_f32 = sample_i16 as f32 / 32768.0;
            samples.push(sample_f32);
        }

        Ok((sample_rate, samples))
    }

    fn reload(&mut self, model_path: &str, config_path: &str, speaker_id: i64) -> Result<(), String> {
        self.current_model = model_path.to_string();
        self.current_config = config_path.to_string();
        self.speaker_id = speaker_id;

        self.piper_exe_path = Self::find_piper_executable();
        self.espeak_data_path = Self::find_espeak_data();

        if let Some(ref exe) = self.piper_exe_path {
            info!("Found Piper binary at: {}", exe.display());
        } else {
            warn!("Piper executable not found in system or local folder.");
        }

        if let Some(ref data) = self.espeak_data_path {
            info!("Found eSpeak-NG data at: {}", data.display());
        }

        if !Path::new(model_path).exists() || !Path::new(config_path).exists() {
            warn!(
                "Piper model files not found at '{}' and '{}'. Using mock fallback.",
                model_path, config_path
            );
            self.config = None;
            return Ok(());
        }

        // Load Config JSON
        let config_str = fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read {}: {}", config_path, e))?;
        let model_cfg: ModelConfig = serde_json::from_str(&config_str)
            .map_err(|e| format!("Failed to parse {}: {}", config_path, e))?;

        info!(
            "Successfully initialized Piper TTS! Target sample rate: {} Hz",
            model_cfg.audio.sample_rate
        );

        self.config = Some(model_cfg);
        Ok(())
    }
}
