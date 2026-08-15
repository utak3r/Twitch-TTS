fn main() {
    println!("cargo:rerun-if-changed=ui/main.slint");
    println!("cargo:rerun-if-changed=app_icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    // 1. Compile Slint UI templates
    slint_build::compile("ui/main.slint").expect("Slint build failed");

    // 2. Embed Windows resources (.exe icon & version metadata)
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
