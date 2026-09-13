#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub id: u32,
    pub name: String,
    pub dedicated_vram_bytes: usize,
    pub is_discrete: bool,
    pub is_nvidia: bool,
}

impl GpuInfo {
    pub fn vram_mb(&self) -> usize {
        self.dedicated_vram_bytes / (1024 * 1024)
    }
}

/// Ensures CUDA bin/x64 directories are in the process PATH on Windows so ONNX Runtime CUDA provider
/// can find cublas64_*.dll and cublasLt64_*.dll.
pub fn setup_cuda_env() {
    #[cfg(windows)]
    {
        let mut cuda_dirs = Vec::new();

        // 1. Always include the executable directory so DirectML.dll and onnxruntime_providers_*.dll are found
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                cuda_dirs.push(exe_dir.to_path_buf());
            }
        }

        if let Ok(cuda_path) = std::env::var("CUDA_PATH") {
            let p = std::path::PathBuf::from(cuda_path);
            cuda_dirs.push(p.join("bin").join("x64"));
            cuda_dirs.push(p.join("bin"));
        }

        let toolkit_base = std::path::Path::new(r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA");
        if toolkit_base.is_dir() {
            if let Ok(entries) = std::fs::read_dir(toolkit_base) {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.is_dir() {
                        cuda_dirs.push(path.join("bin").join("x64"));
                        cuda_dirs.push(path.join("bin"));
                    }
                }
            }
        }

        if let Ok(current_path) = std::env::var("PATH") {
            let mut prepends = Vec::new();
            for dir in cuda_dirs {
                if dir.is_dir() {
                    let s = dir.to_string_lossy().to_string();
                    if !current_path.contains(&s) {
                        prepends.push(s);
                    }
                }
            }
            if !prepends.is_empty() {
                let new_path = format!("{};{}", prepends.join(";"), current_path);
                unsafe { std::env::set_var("PATH", new_path) };
            }
        }
    }
}

#[cfg(windows)]
pub fn detect_gpus() -> Vec<GpuInfo> {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE,
    };

    let mut gpus = Vec::new();

    let factory: IDXGIFactory1 = match unsafe { CreateDXGIFactory1() } {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("[TTS] Failed to create DXGI factory for GPU detection: {}", e);
            return gpus;
        }
    };

    let mut i = 0;
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(i) } {
        if let Ok(desc) = unsafe { adapter.GetDesc1() } {
            // Skip software rasterizers (WARP, Microsoft Basic Render Driver, etc.)
            let is_software = (desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32)) != 0;
            if !is_software {
                let len = desc
                    .Description
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(desc.Description.len());
                let name = String::from_utf16_lossy(&desc.Description[..len]).trim().to_string();

                let name_lower = name.to_lowercase();
                if !name_lower.contains("basic render") && !name_lower.contains("software") {
                    let vram = desc.DedicatedVideoMemory;

                    // Dedicated VRAM > 512 MB or NVIDIA vendor ID (0x10DE) indicates discrete GPU
                    let is_nvidia = desc.VendorId == 0x10DE || name_lower.contains("nvidia") || name_lower.contains("geforce");
                    let has_large_vram = vram > 512 * 1024 * 1024;
                    let is_discrete = is_nvidia || has_large_vram;

                    gpus.push(GpuInfo {
                        id: i,
                        name,
                        dedicated_vram_bytes: vram,
                        is_discrete,
                        is_nvidia,
                    });
                }
            }
        }
        i += 1;
    }

    gpus
}

#[cfg(not(windows))]
pub fn detect_gpus() -> Vec<GpuInfo> {
    Vec::new()
}

/// Selects the best GPU according to the given preference:
/// - "cpu": returns None
/// - "auto" or empty: picks discrete GPU with largest VRAM, or first GPU if no discrete
/// - a number (e.g. "0", "1"): picks the GPU with that ID
/// - a name substring (e.g. "nvidia", "rtx"): picks the first GPU matching that substring (case-insensitive)
pub fn select_best_gpu(gpus: &[GpuInfo], preference: &str) -> Option<GpuInfo> {
    let pref = preference.trim();
    if pref.eq_ignore_ascii_case("cpu") {
        return None;
    }

    if gpus.is_empty() {
        return None;
    }

    // Try matching specific ID
    if let Ok(target_id) = pref.parse::<u32>() {
        if let Some(gpu) = gpus.iter().find(|g| g.id == target_id) {
            return Some(gpu.clone());
        }
    }

    // Try matching name substring (if preference is not "auto" or empty)
    if !pref.is_empty() && !pref.eq_ignore_ascii_case("auto") {
        let pref_lower = pref.to_lowercase();
        if let Some(gpu) = gpus.iter().find(|g| g.name.to_lowercase().contains(&pref_lower)) {
            return Some(gpu.clone());
        }
    }

    // Default "auto": Prioritize discrete GPUs with highest VRAM
    let mut discrete_gpus: Vec<&GpuInfo> = gpus.iter().filter(|g| g.is_discrete).collect();
    if !discrete_gpus.is_empty() {
        discrete_gpus.sort_by_key(|g| std::cmp::Reverse(g.dedicated_vram_bytes));
        return Some((*discrete_gpus[0]).clone());
    }

    // Otherwise pick the GPU with the highest VRAM or first GPU
    let mut all_sorted: Vec<&GpuInfo> = gpus.iter().collect();
    all_sorted.sort_by_key(|g| std::cmp::Reverse(g.dedicated_vram_bytes));
    all_sorted.first().copied().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_and_select_gpu() {
        let gpus = detect_gpus();
        println!("Detected {} GPUs:", gpus.len());
        for g in &gpus {
            println!("  [{}] {} ({} MB VRAM, discrete: {})", g.id, g.name, g.vram_mb(), g.is_discrete);
        }
        let best = select_best_gpu(&gpus, "auto");
        println!("Best GPU: {:?}", best);

        if !gpus.is_empty() {
            assert!(best.is_some());
            let best_gpu = best.unwrap();
            if gpus.iter().any(|g| g.is_discrete) {
                assert!(best_gpu.is_discrete);
            }
        }
    }
}

