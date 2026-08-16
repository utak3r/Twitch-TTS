use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3u64);
    }
    hash
}

fn make_wix_id(prefix: &str, rel_path: &Path) -> String {
    let path_str = rel_path.to_string_lossy().replace('/', "\\");
    let hash = fnv1a_64(path_str.to_lowercase().as_bytes());
    let sanitized: String = path_str
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let truncated = if sanitized.len() > 30 {
        &sanitized[sanitized.len() - 30..]
    } else {
        &sanitized
    };
    format!("{}_{}_{:016x}", prefix, truncated, hash)
}

struct WixGenerator {
    component_ids: Vec<String>,
}

impl WixGenerator {
    fn new() -> Self {
        Self {
            component_ids: Vec::new(),
        }
    }

    fn generate_dir_tree<W: Write>(
        &mut self,
        writer: &mut W,
        dir_path: &Path,
        indent: usize,
    ) -> io::Result<()> {
        let pad = "    ".repeat(indent);
        let mut entries: Vec<_> = fs::read_dir(dir_path)?.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.path());

        // 1. Output files in current directory
        for entry in &entries {
            let path = entry.path();
            if path.is_file() {
                let rel_path = path.to_string_lossy().replace('/', "\\");
                let comp_id = make_wix_id("cmp", &path);
                let file_id = make_wix_id("fil", &path);

                writeln!(writer, "{pad}<Component Id=\"{comp_id}\" Guid=\"*\">")?;
                writeln!(
                    writer,
                    "{pad}    <File Id=\"{file_id}\" Source=\"{rel_path}\" KeyPath=\"yes\" />"
                )?;
                writeln!(writer, "{pad}</Component>")?;

                self.component_ids.push(comp_id);
            }
        }

        // 2. Output subdirectories recursively
        for entry in &entries {
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name().unwrap().to_string_lossy();
                let dir_id = make_wix_id("dir", &path);

                writeln!(writer, "{pad}<Directory Id=\"{dir_id}\" Name=\"{dir_name}\">")?;
                self.generate_dir_tree(writer, &path, indent + 1)?;
                writeln!(writer, "{pad}</Directory>")?;
            }
        }

        Ok(())
    }

    pub fn generate_fragment(
        target_file: &Path,
        root_folder_name: &str,
        component_group_id: &str,
        parent_dir_ref: &str,
    ) -> io::Result<()> {
        let mut generator = WixGenerator::new();
        let mut output = Vec::new();

        writeln!(output, "<?xml version=\"1.0\" encoding=\"utf-8\"?>")?;
        writeln!(output, "<Wix xmlns=\"http://schemas.microsoft.com/wix/2006/wi\">")?;
        writeln!(output, "    <Fragment>")?;
        writeln!(output, "        <DirectoryRef Id=\"{parent_dir_ref}\">")?;

        let root_path = PathBuf::from(root_folder_name);
        let root_dir_id = make_wix_id("dir", &root_path);

        writeln!(output, "            <Directory Id=\"{root_dir_id}\" Name=\"{root_folder_name}\">")?;
        generator.generate_dir_tree(&mut output, &root_path, 4)?;
        writeln!(output, "            </Directory>")?;

        writeln!(output, "        </DirectoryRef>")?;
        writeln!(output, "    </Fragment>")?;

        writeln!(output, "    <Fragment>")?;
        writeln!(output, "        <ComponentGroup Id=\"{component_group_id}\">")?;
        for comp_id in &generator.component_ids {
            writeln!(output, "            <ComponentRef Id=\"{comp_id}\" />")?;
        }
        writeln!(output, "        </ComponentGroup>")?;
        writeln!(output, "    </Fragment>")?;
        writeln!(output, "</Wix>")?;

        if let Some(parent) = target_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target_file, output)?;
        Ok(())
    }
}

fn main() {
    println!("cargo:rerun-if-changed=ui/main.slint");
    println!("cargo:rerun-if-changed=app_icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=piper");
    println!("cargo:rerun-if-changed=models");

    // 1. Generate WiX fragments for data folders if they exist
    if Path::new("piper").exists() {
        if let Err(e) = WixGenerator::generate_fragment(
            Path::new("wix/piper_files.wxs"),
            "piper",
            "PiperFiles",
            "APPLICATIONFOLDER",
        ) {
            eprintln!("Warning: Failed to generate piper_files.wxs: {}", e);
        }
    }

    if Path::new("models").exists() {
        if let Err(e) = WixGenerator::generate_fragment(
            Path::new("wix/models_files.wxs"),
            "models",
            "ModelsFiles",
            "APPLICATIONFOLDER",
        ) {
            eprintln!("Warning: Failed to generate models_files.wxs: {}", e);
        }
    }

    // 2. Compile Slint UI templates
    slint_build::compile("ui/main.slint").expect("Slint build failed");

    // 3. Embed Windows resources (.exe icon & version metadata)
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("app_icon.ico");
        res.set("FileDescription", "Twitch TTS Streamer App");
        res.set("ProductName", "Twitch TTS");
        res.set("OriginalFilename", "twitch-tts.exe");
        res.set("LegalCopyright", "Copyright © 2026 Piotr <utak3r> Borys");
        if let Err(e) = res.compile() {
            eprintln!("Failed to compile Windows resources: {}", e);
        }
    }
}

