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
    let url = format!(
        "https://github.com/{}/releases/download/{}/corescribe.exe",
        REPO, tag
    );
    println!("Downloading from: {}", url);

    let response = ureq::get(&url)
        .call()
        .map_err(|e| format!("Failed to download: {}", e))?;

    let temp_dir = std::env::temp_dir();
    let update_exe = temp_dir.join("corescribe_update.exe");

    let mut file =
        fs::File::create(&update_exe).map_err(|e| format!("Failed to create temp file: {}", e))?;

    std::io::copy(&mut response.into_reader(), &mut file)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    drop(file);

    let current_exe =
        std::env::current_exe().map_err(|e| format!("Failed to get current exe path: {}", e))?;

    let current_dir = current_exe
        .parent()
        .ok_or("Failed to get exe directory".to_string())?;

    #[cfg(target_os = "windows")]
    {
        let cmd = format!(
            "cmd /c \"ping -n 3 127.0.0.1 > nul & move /y \\\"{}\\\" \\\"{}\\\" & start \\\"\\\" \\\"{}\\\"\"",
            update_exe.display(),
            current_exe.display(),
            current_exe.display()
        );
        std::process::Command::new("cmd")
            .args(&["/c", &cmd])
            .current_dir(current_dir)
            .spawn()
            .map_err(|e| format!("Failed to spawn update process: {}", e))?;
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
