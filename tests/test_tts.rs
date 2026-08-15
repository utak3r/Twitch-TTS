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
fn test_piper_tts_synthesis() {
    use twitch_tts::tts::piper::PiperEngine;

    let model_path = "models/voice.onnx";
    let config_path = "models/voice.onnx.json";

    if std::path::Path::new(model_path).exists() && std::path::Path::new(config_path).exists() {
        let mut engine = PiperEngine::new(model_path, config_path, 0);
        let res = engine.synthesize("Cześć! To jest test polskiej syntezy mowy.", 1.0);
        assert!(res.is_ok());

        let (sample_rate, samples) = res.unwrap();
        assert_eq!(sample_rate, 22050);
        assert!(!samples.is_empty());
    }
}
