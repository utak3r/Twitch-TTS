use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

pub enum HotkeyAction {
    ToggleMute,
    SkipCurrent,
}

pub struct HotkeysManager {
    manager: Option<GlobalHotKeyManager>,
    mute_hotkey: Option<HotKey>,
    skip_hotkey: Option<HotKey>,
    running: Arc<AtomicBool>,
}

impl HotkeysManager {
    pub fn new() -> Self {
        Self {
            manager: None,
            mute_hotkey: None,
            skip_hotkey: None,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(
        &mut self,
        mute_key_str: &str,
        skip_key_str: &str,
        action_tx: mpsc::UnboundedSender<HotkeyAction>,
    ) -> Result<(), String> {
        self.stop();

        let manager = GlobalHotKeyManager::new()
            .map_err(|e| format!("Failed to create GlobalHotKeyManager: {:?}", e))?;

        let mute_hk = Self::parse_hotkey(mute_key_str)
            .ok_or_else(|| format!("Invalid mute hotkey: {}", mute_key_str))?;
        let skip_hk = Self::parse_hotkey(skip_key_str)
            .ok_or_else(|| format!("Invalid skip hotkey: {}", skip_key_str))?;

        manager
            .register(mute_hk)
            .map_err(|e| format!("Failed to register mute hotkey: {:?}", e))?;
        manager
            .register(skip_hk)
            .map_err(|e| format!("Failed to register skip hotkey: {:?}", e))?;

        let mute_id = mute_hk.id();
        let skip_id = skip_hk.id();

        self.manager = Some(manager);
        self.mute_hotkey = Some(mute_hk);
        self.skip_hotkey = Some(skip_hk);

        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let receiver = GlobalHotKeyEvent::receiver();

        std::thread::spawn(move || {
            info!("Global hotkeys listener thread started.");
            while running.load(Ordering::SeqCst) {
                if let Ok(event) = receiver.recv_timeout(std::time::Duration::from_millis(100)) {
                    if event.state == HotKeyState::Pressed {
                        if event.id == mute_id {
                            info!("Mute hotkey triggered!");
                            let _ = action_tx.send(HotkeyAction::ToggleMute);
                        } else if event.id == skip_id {
                            info!("Skip hotkey triggered!");
                            let _ = action_tx.send(HotkeyAction::SkipCurrent);
                        }
                    }
                }
            }
            info!("Global hotkeys listener thread ended.");
        });

        Ok(())
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(ref manager) = self.manager {
            if let Some(hk) = self.mute_hotkey.take() {
                let _ = manager.unregister(hk);
            }
            if let Some(hk) = self.skip_hotkey.take() {
                let _ = manager.unregister(hk);
            }
        }
        self.manager = None;
    }

    fn parse_hotkey(s: &str) -> Option<HotKey> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        let mut modifiers = Modifiers::empty();
        let mut code = None;

        for part in parts {
            match part.to_uppercase().as_str() {
                "CTRL" | "CONTROL" => modifiers |= Modifiers::CONTROL,
                "ALT" => modifiers |= Modifiers::ALT,
                "SHIFT" => modifiers |= Modifiers::SHIFT,
                "SUPER" | "WIN" | "META" => modifiers |= Modifiers::SUPER,
                "F1" => code = Some(Code::F1),
                "F2" => code = Some(Code::F2),
                "F3" => code = Some(Code::F3),
                "F4" => code = Some(Code::F4),
                "F5" => code = Some(Code::F5),
                "F6" => code = Some(Code::F6),
                "F7" => code = Some(Code::F7),
                "F8" => code = Some(Code::F8),
                "F9" => code = Some(Code::F9),
                "F10" => code = Some(Code::F10),
                "F11" => code = Some(Code::F11),
                "F12" => code = Some(Code::F12),
                "A" => code = Some(Code::KeyA),
                "B" => code = Some(Code::KeyB),
                "C" => code = Some(Code::KeyC),
                "D" => code = Some(Code::KeyD),
                "E" => code = Some(Code::KeyE),
                "F" => code = Some(Code::KeyF),
                "G" => code = Some(Code::KeyG),
                "H" => code = Some(Code::KeyH),
                "I" => code = Some(Code::KeyI),
                "J" => code = Some(Code::KeyJ),
                "K" => code = Some(Code::KeyK),
                "L" => code = Some(Code::KeyL),
                "M" => code = Some(Code::KeyM),
                "N" => code = Some(Code::KeyN),
                "O" => code = Some(Code::KeyO),
                "P" => code = Some(Code::KeyP),
                "Q" => code = Some(Code::KeyQ),
                "R" => code = Some(Code::KeyR),
                "S" => code = Some(Code::KeyS),
                "T" => code = Some(Code::KeyT),
                "U" => code = Some(Code::KeyU),
                "V" => code = Some(Code::KeyV),
                "W" => code = Some(Code::KeyW),
                "X" => code = Some(Code::KeyX),
                "Y" => code = Some(Code::KeyY),
                "Z" => code = Some(Code::KeyZ),
                _ => {}
            }
        }

        let key_code = code?;
        let mods = if modifiers.is_empty() {
            None
        } else {
            Some(modifiers)
        };

        Some(HotKey::new(mods, key_code))
    }
}

unsafe impl Send for HotkeysManager {}
unsafe impl Sync for HotkeysManager {}
