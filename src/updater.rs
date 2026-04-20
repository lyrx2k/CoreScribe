use std::fs;

const REPO: &str = "lyrx2k/CoreScribe";

pub fn check_for_update() -> Result<Option<String>, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
    let response = ureq::get(&url)
        .set("User-Agent", "CoreScribe-Updater")
        .call()
        .map_err(|e| format!("Failed to check updates: {}", e))?;

    let body = response
        .into_string()
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let latest_tag = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or("No tag_name in response".to_string())?;

    let current = format!("v{}", env!("CARGO_PKG_VERSION"));
    if is_newer(&current, latest_tag) {
        Ok(Some(latest_tag.to_string()))
    } else {
        Ok(None)
    }
}

pub fn download_and_install(tag: &str) -> Result<(), String> {
    let temp_dir = std::env::temp_dir();
    let log_path = temp_dir.join("corescribe_update.log");
    let mut log = fs::File::create(&log_path).unwrap();

    macro_rules! log {
        ($($arg:tt)*) => {{
            let line = format!($($arg)*);
            use std::io::Write;
            let _ = writeln!(log, "{}", line);
        }};
    }

    let url = format!(
        "https://github.com/{}/releases/download/{}/corescribe.exe",
        REPO, tag
    );
    log!("Downloading from: {}", url);

    let response = ureq::get(&url)
        .call()
        .map_err(|e| { log!("Download failed: {}", e); format!("Failed to download: {}", e) })?;

    log!("Download response OK, status: {}", response.status());

    let update_exe = temp_dir.join("corescribe_update.exe");
    log!("Saving to: {}", update_exe.display());

    let mut file = fs::File::create(&update_exe)
        .map_err(|e| { log!("Create file failed: {}", e); format!("Failed to create temp file: {}", e) })?;

    let bytes = std::io::copy(&mut response.into_reader(), &mut file)
        .map_err(|e| { log!("Write failed: {}", e); format!("Failed to write file: {}", e) })?;

    log!("Downloaded {} bytes", bytes);
    drop(file);

    let file_size = fs::metadata(&update_exe).map(|m| m.len()).unwrap_or(0);
    log!("File size on disk: {} bytes", file_size);

    if file_size < 100_000 {
        let content = fs::read_to_string(&update_exe).unwrap_or_default();
        log!("File too small, content: {}", &content[..content.len().min(500)]);
        return Err(format!("Downloaded file is too small ({} bytes) - likely an error page", file_size));
    }

    let current_exe = std::env::current_exe()
        .map_err(|e| { log!("current_exe failed: {}", e); format!("Failed to get current exe: {}", e) })?;

    log!("Current exe: {}", current_exe.display());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let bat = format!(
            "@echo off\r\nping -n 3 127.0.0.1 > nul\r\nmove /y \"{}\" \"{}\"\r\nstart \"\" \"{}\"\r\ndel \"%~f0\"\r\n",
            update_exe.display(),
            current_exe.display(),
            current_exe.display()
        );

        let bat_path = temp_dir.join("corescribe_update.bat");
        log!("Writing bat to: {}", bat_path.display());
        log!("Bat content:\n{}", bat);

        fs::write(&bat_path, bat.as_bytes())
            .map_err(|e| { log!("Write bat failed: {}", e); format!("Failed to write update script: {}", e) })?;

        log!("Spawning bat script...");
        std::process::Command::new("cmd")
            .args(&["/c", &bat_path.to_string_lossy().to_string()])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| { log!("Spawn failed: {}", e); format!("Failed to spawn updater: {}", e) })?;

        log!("Bat spawned, exiting app...");
    }

    std::process::exit(0);
}

fn is_newer(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let v = v.trim_start_matches('v');
        let parts: Vec<u32> = v.split('.').filter_map(|p| p.parse().ok()).collect();
        (
            parts.get(0).copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        )
    };
    parse(latest) > parse(current)
}
