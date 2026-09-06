use super::mock::MockTTSEngine;
use super::TTSEngine;
use crate::config::TTSConfig;
use ndarray::{s, Array1, Array2, Array3, Array4};
use ort::session::Session;
use ort::value::{DynValue, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use tokenizers::Tokenizer;
use tracing::{info, warn};
use unicode_normalization::UnicodeNormalization;

const S3GEN_SR: u32 = 24000;
const START_SPEECH_TOKEN: i64 = 6561;
const STOP_SPEECH_TOKEN: i64 = 6562;
const NUM_HIDDEN_LAYERS: usize = 30;
const NUM_KEY_VALUE_HEADS: usize = 16;
const HEAD_DIM: usize = 64;
const REPETITION_PENALTY: f32 = 1.2;
const MAX_NEW_TOKENS: usize = 256;

struct CachedVoice {
    voice_path: String,
    cond_emb: Array3<f32>,
    prompt_token: Array2<i64>,
    ref_x_vector: Array2<f32>,
    prompt_feat: Array3<f32>,
}

pub struct ChatterboxEngine {
    mock_fallback: MockTTSEngine,
    models_dir: PathBuf,
    language: String,
    voice_sample_path: String,
    exaggeration: f32,

    speech_encoder: Option<Session>,
    embed_tokens: Option<Session>,
    language_model: Option<Session>,
    conditional_decoder: Option<Session>,
    tokenizer: Option<Tokenizer>,

    cached_voice: Option<CachedVoice>,
}

impl ChatterboxEngine {
    pub fn new_uninitialized(config: &TTSConfig) -> Self {
        Self {
            mock_fallback: MockTTSEngine::new(),
            models_dir: PathBuf::from("models"),
            language: config.language.clone(),
            voice_sample_path: config.voice_sample.clone(),
            exaggeration: config.exaggeration,
            speech_encoder: None,
            embed_tokens: None,
            language_model: None,
            conditional_decoder: None,
            tokenizer: None,
            cached_voice: None,
        }
    }

    pub fn new(config: &TTSConfig) -> Self {
        let mut engine = Self::new_uninitialized(config);
        if let Err(e) = engine.reload(config) {
            warn!("Failed to load Chatterbox models: {}. Falling back to mock engine.", e);
        }
        engine
    }

    pub fn is_ready(&self) -> bool {
        self.speech_encoder.is_some()
            && self.embed_tokens.is_some()
            && self.language_model.is_some()
            && self.conditional_decoder.is_some()
            && self.tokenizer.is_some()
            && self.cached_voice.is_some()
    }

    pub fn init_sessions(&mut self) -> Result<(), String> {
        self.init_sessions_with_progress(|_| {})
    }

    pub fn init_sessions_with_progress<F: FnMut(&str)>(&mut self, mut progress: F) -> Result<(), String> {
        if self.speech_encoder.is_some()
            && self.embed_tokens.is_some()
            && self.language_model.is_some()
            && self.conditional_decoder.is_some()
            && self.tokenizer.is_some()
        {
            return Ok(());
        }

        let se_path = self.models_dir.join("speech_encoder.onnx");
        let et_path = self.models_dir.join("embed_tokens.onnx");
        let lm_path = self.models_dir.join("language_model.onnx");
        let cd_path = self.models_dir.join("conditional_decoder.onnx");
        let tok_path = self.models_dir.join("tokenizer.json");

        if !se_path.exists() || !et_path.exists() || !lm_path.exists() || !cd_path.exists() || !tok_path.exists() {
            return Err(format!(
                "Required Chatterbox model files missing in '{}'. Ensure speech_encoder, embed_tokens, language_model, conditional_decoder .onnx and tokenizer.json exist.",
                self.models_dir.display()
            ));
        }

        let total_start = std::time::Instant::now();
        info!("[TTS] Loading Chatterbox ONNX inference sessions from '{}'...", self.models_dir.display());

        // Step 1: speech_encoder.onnx
        progress("Loading speech encoder (1/5)...");
        info!("[TTS] [1/5] Loading speech encoder (speech_encoder.onnx)...");
        let t0 = std::time::Instant::now();
        let se = Session::builder()
            .map_err(|e| format!("Failed to create SessionBuilder: {}", e))?
            .commit_from_file(&se_path)
            .map_err(|e| format!("Failed to load speech_encoder.onnx: {}", e))?;
        info!("[TTS] [1/5] Speech encoder loaded successfully in {:.2}s", t0.elapsed().as_secs_f32());

        // Step 2: embed_tokens.onnx
        progress("Loading token embeddings (2/5)...");
        info!("[TTS] [2/5] Loading token embeddings (embed_tokens.onnx)...");
        let t0 = std::time::Instant::now();
        let et = Session::builder()
            .map_err(|e| format!("Failed to create SessionBuilder: {}", e))?
            .commit_from_file(&et_path)
            .map_err(|e| format!("Failed to load embed_tokens.onnx: {}", e))?;
        info!("[TTS] [2/5] Token embeddings loaded successfully in {:.2}s", t0.elapsed().as_secs_f32());

        // Step 3: language_model.onnx
        progress("Loading language model (3/5)...");
        info!("[TTS] [3/5] Loading language model (language_model.onnx)...");
        let t0 = std::time::Instant::now();
        let lm = Session::builder()
            .map_err(|e| format!("Failed to create SessionBuilder: {}", e))?
            .commit_from_file(&lm_path)
            .map_err(|e| format!("Failed to load language_model.onnx: {}", e))?;
        info!("[TTS] [3/5] Language model loaded successfully in {:.2}s", t0.elapsed().as_secs_f32());

        // Step 4: conditional_decoder.onnx
        progress("Loading conditional decoder (4/5)...");
        info!("[TTS] [4/5] Loading conditional decoder (conditional_decoder.onnx)...");
        let t0 = std::time::Instant::now();
        let cd = Session::builder()
            .map_err(|e| format!("Failed to create SessionBuilder: {}", e))?
            .commit_from_file(&cd_path)
            .map_err(|e| format!("Failed to load conditional_decoder.onnx: {}", e))?;
        info!("[TTS] [4/5] Conditional decoder loaded successfully in {:.2}s", t0.elapsed().as_secs_f32());

        // Step 5: tokenizer.json
        progress("Loading tokenizer (5/5)...");
        info!("[TTS] [5/5] Loading tokenizer (tokenizer.json)...");
        let t0 = std::time::Instant::now();
        let tok = Tokenizer::from_file(&tok_path)
            .map_err(|e| format!("Failed to load tokenizer.json: {}", e))?;
        info!("[TTS] [5/5] Tokenizer loaded successfully in {:.2}s", t0.elapsed().as_secs_f32());

        self.speech_encoder = Some(se);
        self.embed_tokens = Some(et);
        self.language_model = Some(lm);
        self.conditional_decoder = Some(cd);
        self.tokenizer = Some(tok);

        info!("[TTS] All Chatterbox models and tokenizer loaded successfully in {:.2}s!", total_start.elapsed().as_secs_f32());
        Ok(())
    }

    fn load_voice_audio(path: &str) -> Result<Vec<f32>, String> {
        let mut reader = hound::WavReader::open(path)
            .map_err(|e| format!("Failed to open voice sample '{}': {}", path, e))?;
        let spec = reader.spec();

        if spec.channels == 0 {
            return Err("Voice WAV file has 0 channels".to_string());
        }

        let raw_samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
            hound::SampleFormat::Int => {
                let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i32>()
                    .filter_map(Result::ok)
                    .map(|s| (s as f32) / max_val)
                    .collect()
            }
        };

        if raw_samples.is_empty() {
            return Err("Voice sample WAV is empty".to_string());
        }

        // Downmix to mono if stereo
        let mono_samples: Vec<f32> = if spec.channels > 1 {
            let ch = spec.channels as usize;
            raw_samples
                .chunks(ch)
                .map(|chunk| chunk.iter().sum::<f32>() / ch as f32)
                .collect()
        } else {
            raw_samples
        };

        // Note: voice samples in ./voices (utak3r.wav, default.wav) are already 24000 Hz.
        Ok(mono_samples)
    }

    pub fn encode_reference_voice(&mut self) -> Result<(), String> {
        self.encode_reference_voice_with_progress(|_| {})
    }

    pub fn encode_reference_voice_with_progress<F: FnMut(&str)>(&mut self, mut progress: F) -> Result<(), String> {
        if let Some(ref cached) = self.cached_voice {
            if cached.voice_path == self.voice_sample_path {
                return Ok(());
            }
        }

        progress(&format!("Encoding voice reference ({})", self.voice_sample_path));
        info!("[TTS] Encoding reference voice '{}'...", self.voice_sample_path);
        let t0 = std::time::Instant::now();

        let se = self.speech_encoder.as_mut().ok_or("Speech encoder not initialized")?;

        let audio = Self::load_voice_audio(&self.voice_sample_path)?;
        let audio_len = audio.len();
        let audio_array = Array2::from_shape_vec((1, audio_len), audio)
            .map_err(|e| format!("Failed to create audio tensor: {}", e))?;

        let audio_val = Value::from_array(audio_array)
            .map_err(|e| format!("Failed to create audio Value: {}", e))?;

        let outputs = se
            .run(ort::inputs![ "audio_values" => audio_val ])
            .map_err(|e| format!("Failed to run speech_encoder: {}", e))?;

        // Extract outputs: cond_emb, prompt_token, ref_x_vector, prompt_feat
        let (cond_shape, cond_data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract cond_emb: {}", e))?;
        let cond_emb = Array3::from_shape_vec(
            (cond_shape[0] as usize, cond_shape[1] as usize, cond_shape[2] as usize),
            cond_data.to_vec(),
        ).map_err(|e| format!("cond_emb shape mismatch: {}", e))?;

        let (prompt_shape, prompt_data) = outputs[1]
            .try_extract_tensor::<i64>()
            .map_err(|e| format!("Failed to extract prompt_token: {}", e))?;
        let prompt_token = Array2::from_shape_vec(
            (prompt_shape[0] as usize, prompt_shape[1] as usize),
            prompt_data.to_vec(),
        ).map_err(|e| format!("prompt_token shape mismatch: {}", e))?;

        let (ref_shape, ref_data) = outputs[2]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract ref_x_vector: {}", e))?;
        let ref_x_vector = Array2::from_shape_vec(
            (ref_shape[0] as usize, ref_shape[1] as usize),
            ref_data.to_vec(),
        ).map_err(|e| format!("ref_x_vector shape mismatch: {}", e))?;

        let (feat_shape, feat_data) = outputs[3]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract prompt_feat: {}", e))?;
        let prompt_feat = Array3::from_shape_vec(
            (feat_shape[0] as usize, feat_shape[1] as usize, feat_shape[2] as usize),
            feat_data.to_vec(),
        ).map_err(|e| format!("prompt_feat shape mismatch: {}", e))?;

        self.cached_voice = Some(CachedVoice {
            voice_path: self.voice_sample_path.clone(),
            cond_emb,
            prompt_token,
            ref_x_vector,
            prompt_feat,
        });

        info!("[TTS] Reference voice '{}' encoded and cached successfully in {:.2}s!", self.voice_sample_path, t0.elapsed().as_secs_f32());
        Ok(())
    }

    pub fn reload_with_progress<F: FnMut(&str)>(&mut self, config: &TTSConfig, mut progress: F) -> Result<(), String> {
        self.language = config.language.clone();
        self.exaggeration = config.exaggeration;

        let voice_changed = self.voice_sample_path != config.voice_sample;
        self.voice_sample_path = config.voice_sample.clone();

        self.init_sessions_with_progress(&mut progress)?;

        if voice_changed || self.cached_voice.is_none() {
            self.encode_reference_voice_with_progress(&mut progress)?;
        }

        progress("Ready to synthesize");
        info!("[TTS] Chatterbox engine is READY to synthesize.");
        Ok(())
    }
}

impl TTSEngine for ChatterboxEngine {
    fn synthesize(&mut self, text: &str, _speed: f32) -> Result<(u32, Vec<f32>), String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok((S3GEN_SR, Vec::new()));
        }

        if self.speech_encoder.is_none() || self.embed_tokens.is_none() || self.language_model.is_none() || self.conditional_decoder.is_none() || self.tokenizer.is_none() {
            warn!("Chatterbox models not loaded. Using mock fallback.");
            return self.mock_fallback.synthesize(text, _speed);
        }

        if self.cached_voice.is_none() {
            if let Err(e) = self.encode_reference_voice() {
                warn!("Failed to encode reference voice: {}. Using mock fallback.", e);
                return self.mock_fallback.synthesize(text, _speed);
            }
        }

        let cached = self.cached_voice.as_ref().unwrap();

        // 1. Text normalization & preparation
        let normalized = trimmed.to_lowercase().nfkd().collect::<String>();
        let text_processed = if !self.language.is_empty() {
            format!("[{}]{}", self.language.to_lowercase(), normalized)
        } else {
            normalized
        };

        // 2. Tokenize
        let tokenizer = self.tokenizer.as_ref().unwrap();
        let encoding = tokenizer
            .encode(text_processed.as_str(), true)
            .map_err(|e| format!("Tokenization failed: {}", e))?;
        let token_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        tracing::debug!(
            "[TTS] text_processed: '{}', token_ids len: {}",
            text_processed,
            token_ids.len()
        );

        if token_ids.is_empty() {
            return Ok((S3GEN_SR, Vec::new()));
        }

        let seq_len = token_ids.len();
        let mut position_ids_vec = Vec::with_capacity(seq_len);
        for (idx, &tok) in token_ids.iter().enumerate() {
            if tok >= START_SPEECH_TOKEN {
                position_ids_vec.push(0i64);
            } else {
                position_ids_vec.push(idx as i64 - 1);
            }
        }

        let mut input_ids = Array2::from_shape_vec((1, seq_len), token_ids)
            .map_err(|e| format!("Failed to create input_ids array: {}", e))?;
        let mut position_ids = Array2::from_shape_vec((1, seq_len), position_ids_vec)
            .map_err(|e| format!("Failed to create position_ids array: {}", e))?;

        let mut generate_tokens = vec![START_SPEECH_TOKEN];

        let mut past_key_values: HashMap<String, Array4<f32>> = HashMap::new();
        for layer in 0..NUM_HIDDEN_LAYERS {
            for kv in &["key", "value"] {
                let k = format!("past_key_values.{}.{}", layer, kv);
                past_key_values.insert(k, Array4::zeros((1, NUM_KEY_VALUE_HEADS, 0, HEAD_DIM)));
            }
        }

        let mut attention_mask: Option<Array2<i64>> = None;

        let embed_tokens = self.embed_tokens.as_mut().unwrap();
        let language_model = self.language_model.as_mut().unwrap();

        // 3. Autoregressive Loop
        for i in 0..MAX_NEW_TOKENS {
            // Run embed_tokens
            let in_ids_val = Value::from_array(input_ids.clone())
                .map_err(|e| format!("Failed to build in_ids Value: {}", e))?;
            let pos_ids_val = Value::from_array(position_ids.clone())
                .map_err(|e| format!("Failed to build pos_ids Value: {}", e))?;
            let exag_val = Value::from_array(Array1::from_vec(vec![self.exaggeration]))
                .map_err(|e| format!("Failed to build exag Value: {}", e))?;

            let et_outputs = embed_tokens
                .run(ort::inputs![
                    "input_ids" => in_ids_val,
                    "position_ids" => pos_ids_val,
                    "exaggeration" => exag_val
                ])
                .map_err(|e| format!("Failed to run embed_tokens: {}", e))?;

            let (embed_shape, embed_data) = et_outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Failed to extract embeds: {}", e))?;
            let mut inputs_embeds = Array3::from_shape_vec(
                (embed_shape[0] as usize, embed_shape[1] as usize, embed_shape[2] as usize),
                embed_data.to_vec(),
            ).map_err(|e| format!("inputs_embeds shape mismatch: {}", e))?;

            if i == 0 {
                // Prepend cond_emb to inputs_embeds along sequence axis
                let cond = &cached.cond_emb;
                let cond_seq = cond.shape()[1];
                let cur_seq = inputs_embeds.shape()[1];
                let hidden_dim = cond.shape()[2];
                let total_seq = cond_seq + cur_seq;

                let mut combined = Array3::zeros((1, total_seq, hidden_dim));
                combined.slice_mut(s![0, 0..cond_seq, ..]).assign(&cond.slice(s![0, .., ..]));
                combined.slice_mut(s![0, cond_seq..total_seq, ..]).assign(&inputs_embeds.slice(s![0, .., ..]));
                inputs_embeds = combined;

                attention_mask = Some(Array2::ones((1, total_seq)));
            }

            let mask = attention_mask.as_ref().unwrap();

            // Prepare LLM inputs
            let mut lm_inputs: Vec<(String, DynValue)> = Vec::with_capacity(62);
            lm_inputs.push(("inputs_embeds".to_string(), Value::from_array(inputs_embeds).unwrap().upcast().into()));
            lm_inputs.push(("attention_mask".to_string(), Value::from_array(mask.clone()).unwrap().upcast().into()));

            for layer in 0..NUM_HIDDEN_LAYERS {
                for kv in &["key", "value"] {
                    let k = format!("past_key_values.{}.{}", layer, kv);
                    let val = past_key_values.get(&k).unwrap().clone();
                    lm_inputs.push((k, Value::from_array(val).unwrap().upcast().into()));
                }
            }

            // Run language_model
            let lm_outputs = language_model
                .run(lm_inputs)
                .map_err(|e| format!("Failed to run language_model at step {}: {}", i, e))?;

            // Output 0 is logits: [1, seq_len, vocab_size]
            let (logits_shape, logits_data) = lm_outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Failed to extract logits: {}", e))?;

            let cur_seq_len = logits_shape[1] as usize;
            let vocab_size = logits_shape[2] as usize;
            let last_token_start = (cur_seq_len - 1) * vocab_size;
            let mut last_logits = logits_data[last_token_start..last_token_start + vocab_size].to_vec();

            // Apply repetition penalty to tokens generated so far
            let mut penalized = std::collections::HashSet::new();
            for &prev_tok in &generate_tokens {
                if penalized.insert(prev_tok) && (prev_tok as usize) < last_logits.len() {
                    let val = last_logits[prev_tok as usize];
                    if val < 0.0 {
                        last_logits[prev_tok as usize] = val * REPETITION_PENALTY;
                    } else {
                        last_logits[prev_tok as usize] = val / REPETITION_PENALTY;
                    }
                }
            }

            // Argmax
            let mut best_idx = 0;
            let mut best_score = f32::NEG_INFINITY;
            for (idx, &score) in last_logits.iter().enumerate() {
                if score > best_score {
                    best_score = score;
                    best_idx = idx;
                }
            }

            let next_token = best_idx as i64;
            generate_tokens.push(next_token);

            if next_token == STOP_SPEECH_TOKEN {
                break;
            }

            // Update inputs for next iteration
            input_ids = Array2::from_shape_vec((1, 1), vec![next_token]).unwrap();
            position_ids = Array2::from_shape_vec((1, 1), vec![(i + 1) as i64]).unwrap();

            let cur_mask = attention_mask.take().unwrap();
            let new_mask_len = cur_mask.shape()[1] + 1;
            let new_mask = Array2::ones((1, new_mask_len));
            attention_mask = Some(new_mask);

            // Update past_key_values from present_key_values (outputs 1..61)
            let mut out_idx = 1;
            for layer in 0..NUM_HIDDEN_LAYERS {
                for kv in &["key", "value"] {
                    let k = format!("past_key_values.{}.{}", layer, kv);
                    let (kv_shape, kv_data) = lm_outputs[out_idx]
                        .try_extract_tensor::<f32>()
                        .map_err(|e| format!("Failed to extract present kv: {}", e))?;
                    let kv_arr = Array4::from_shape_vec(
                        (kv_shape[0] as usize, kv_shape[1] as usize, kv_shape[2] as usize, kv_shape[3] as usize),
                        kv_data.to_vec(),
                    ).map_err(|e| format!("present kv shape mismatch: {}", e))?;
                    past_key_values.insert(k, kv_arr);
                    out_idx += 1;
                }
            }
        }

        tracing::debug!("[TTS] Total generate_tokens len: {}", generate_tokens.len());

        // 4. Conditional Decoder
        let speech_tokens_slice = if generate_tokens.len() > 2 {
            &generate_tokens[1..generate_tokens.len() - 1]
        } else {
            &[]
        };

        let prompt_tok = &cached.prompt_token;
        let prompt_len = prompt_tok.shape()[1];
        let total_speech_len = prompt_len + speech_tokens_slice.len();
        tracing::debug!("[TTS] prompt_len={}, speech_tokens len={}, total_speech_len={}", prompt_len, speech_tokens_slice.len(), total_speech_len);

        let mut full_speech_tokens = Array2::zeros((1, total_speech_len));
        full_speech_tokens.slice_mut(s![0, 0..prompt_len]).assign(&prompt_tok.slice(s![0, ..]));
        for (j, &tok) in speech_tokens_slice.iter().enumerate() {
            full_speech_tokens[[0, prompt_len + j]] = tok;
        }

        let cond_decoder = self.conditional_decoder.as_mut().unwrap();
        let cd_inputs = ort::inputs![
            "speech_tokens" => Value::from_array(full_speech_tokens).unwrap(),
            "speaker_embeddings" => Value::from_array(cached.ref_x_vector.clone()).unwrap(),
            "speaker_features" => Value::from_array(cached.prompt_feat.clone()).unwrap()
        ];

        let cd_outputs = cond_decoder
            .run(cd_inputs)
            .map_err(|e| format!("Failed to run conditional_decoder: {}", e))?;

        let (wav_shape, wav_data) = cd_outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract wav: {}", e))?;

        tracing::debug!("[TTS] wav_shape={:?}, total wav_data len={}", wav_shape, wav_data.len());
        let samples = wav_data.to_vec();
        info!("Chatterbox generated {} audio samples at {} Hz", samples.len(), S3GEN_SR);


        Ok((S3GEN_SR, samples))
    }

    fn reload(&mut self, config: &TTSConfig) -> Result<(), String> {
        self.reload_with_progress(config, |_| {})
    }

    fn reload_with_progress(
        &mut self,
        config: &TTSConfig,
        progress: Box<dyn FnMut(&str) + Send>,
    ) -> Result<(), String> {
        ChatterboxEngine::reload_with_progress(self, config, progress)
    }

    fn is_ready(&self) -> bool {
        ChatterboxEngine::is_ready(self)
    }
}

