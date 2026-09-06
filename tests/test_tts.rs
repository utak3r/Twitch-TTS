use twitch_tts::tts::mock::MockTTSEngine;
use twitch_tts::tts::{export_wav_file, TTSEngine};

#[test]
fn test_mock_tts_and_wav_export() {
    let mut engine = MockTTSEngine::new();
    let (sample_rate, samples) = engine
        .synthesize("Witaj na streamie!", 1.0)
        .expect("Synthesis failed");

    assert_eq!(sample_rate, 22050);
    assert!(!samples.is_empty());

    let temp_wav = "target/test_tts_output.wav";
    let _ = std::fs::create_dir_all("target");

    let export_res = export_wav_file(temp_wav, sample_rate, &samples);
    assert!(export_res.is_ok());
    assert!(std::path::Path::new(temp_wav).exists());

    let _ = std::fs::remove_file(temp_wav);
}

#[test]
#[cfg(feature = "piper")]
fn test_piper_tts_synthesis() {
    use twitch_tts::config::TTSConfig;
    use twitch_tts::tts::piper::PiperEngine;

    let model_path = "models/pl_zenski_1.onnx";
    let config_path = "models/pl_zenski_1.onnx.json";

    if std::path::Path::new(model_path).exists() && std::path::Path::new(config_path).exists() {
        let mut cfg = TTSConfig::default();
        cfg.model_path = model_path.to_string();
        cfg.config_path = config_path.to_string();
        let mut engine = PiperEngine::new(&cfg);
        let res = engine.synthesize("Cześć! To jest test polskiej syntezy mowy.", 1.0);
        assert!(res.is_ok());

        let (sample_rate, samples) = res.unwrap();
        assert_eq!(sample_rate, 22050);
        assert!(!samples.is_empty());
    }
}

#[test]
#[cfg(feature = "chatterbox")]
fn test_chatterbox_tts_synthesis() {
    use twitch_tts::config::TTSConfig;
    use twitch_tts::tts::chatterbox::ChatterboxEngine;

    let models_dir = std::path::Path::new("models");
    let voice_sample = "voices/utak3r.wav";

    if models_dir.join("speech_encoder.onnx").exists()
        && models_dir.join("tokenizer.json").exists()
        && std::path::Path::new(voice_sample).exists()
    {
        let mut cfg = TTSConfig::default();
        cfg.language = "pl".to_string();
        cfg.voice_sample = voice_sample.to_string();
        cfg.exaggeration = 0.5;

        use twitch_tts::config::FiltersConfig;
        use twitch_tts::filter::TextFilter;
        let filters_cfg = FiltersConfig::default();
        let filter = TextFilter::new(filters_cfg);
        let filter_res = filter.process("Tester", "Cześć utak3r! Jak tam dzisiejszy stream?", false);
        let spoken_text = match filter_res {
            twitch_tts::domain::models::FilterResult::Ready(item) => item.spoken_text,
            _ => "Cześć utak3r! Jak tam dzisiejszy stream?".to_string(),
        };
        println!("Filtered spoken_text: '{}'", spoken_text);

        let mut engine = ChatterboxEngine::new(&cfg);
        let res = engine.synthesize(&spoken_text, 1.0);
        assert!(res.is_ok());

        let (sample_rate, samples) = res.unwrap();
        assert_eq!(sample_rate, 24000);
        assert!(!samples.is_empty());
        assert!(samples.len() > 10000, "Audio sample count was too low: {}", samples.len());
        let max_val = samples.iter().copied().fold(0.0f32, |acc, x| acc.max(x.abs()));
        assert!(max_val > 0.05, "Audio was silent, max_val={}", max_val);
    }
}


