use cpal::traits::{DeviceTrait, HostTrait};
use tracing::warn;

pub struct AudioDeviceManager;

impl AudioDeviceManager {
    pub fn list_output_devices() -> Vec<String> {
        let mut devices = vec!["Default".to_string()];
        let host = cpal::default_host();

        if let Ok(output_devices) = host.output_devices() {
            for device in output_devices {
                if let Ok(name) = device.name() {
                    if !name.trim().is_empty() && !devices.contains(&name) {
                        devices.push(name);
                    }
                }
            }
        }

        devices
    }

    pub fn get_device(name: &str) -> Option<cpal::Device> {
        let host = cpal::default_host();
        if name == "Default" || name.is_empty() {
            return host.default_output_device();
        }

        if let Ok(devices) = host.output_devices() {
            for device in devices {
                if let Ok(dev_name) = device.name() {
                    if dev_name == name {
                        return Some(device);
                    }
                }
            }
        }

        warn!("Audio device '{}' not found. Falling back to default output device.", name);
        host.default_output_device()
    }
}
