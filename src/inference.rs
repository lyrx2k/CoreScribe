use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[derive(Debug, Clone)]
pub struct WhisperConfig {
    pub model_size: ModelSize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModelSize {
    Tiny,
    Base,
    Small,
}

impl ModelSize {
    pub fn name(&self) -> &'static str {
        match self {
            ModelSize::Tiny => "tiny",
            ModelSize::Base => "base",
            ModelSize::Small => "small",
        }
    }

    pub fn model_filename(&self) -> &'static str {
        match self {
            ModelSize::Tiny => "ggml-tiny.bin",
            ModelSize::Base => "ggml-base.bin",
            ModelSize::Small => "ggml-small.bin",
        }
    }

    pub fn model_url(&self) -> &'static str {
        match self {
            ModelSize::Tiny => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin"
            }
            ModelSize::Base => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
            }
            ModelSize::Small => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
            }
        }
    }
}

pub struct WhisperModel {
    model_path: PathBuf,
    whisper_exe: PathBuf,
}

impl WhisperModel {
    pub fn new(config: WhisperConfig) -> Result<Self, String> {
        println!("📦 Loading Whisper {} model...", config.model_size.name());

        let whisper_exe = Self::get_or_download_whisper_exe()?;
        println!("✅ Whisper binary ready");

        let model_path = Self::get_or_download_model(config.model_size)?;
        println!("✅ Model {} loaded", config.model_size.name());

        Ok(WhisperModel {
            model_path,
            whisper_exe,
        })
    }

    fn get_or_download_whisper_exe() -> Result<PathBuf, String> {
        let app_data =
            std::env::var("APPDATA").unwrap_or_else(|_| std::env::var("HOME").unwrap_or_default());
        let bin_dir = PathBuf::from(app_data).join("whisper-cpp-bin");
        let exe_path = bin_dir.join("main.exe");

        if exe_path.exists() {
            println!("✅ Using cached Whisper binary");
            return Ok(exe_path);
        }

        println!("📥 Downloading whisper.exe...");

        fs::create_dir_all(&bin_dir).map_err(|e| format!("Failed to create bin dir: {}", e))?;

        let url =
            "https://github.com/ggerganov/whisper.cpp/releases/download/v1.5.4/whisper-bin-x64.zip";

        let response = ureq::get(url)
            .call()
            .map_err(|e| format!("Failed to download: {}", e))?;

        let zip_path = bin_dir.join("whisper.zip");
        let mut file =
            fs::File::create(&zip_path).map_err(|e| format!("Failed to create zip: {}", e))?;

        std::io::copy(&mut response.into_reader(), &mut file)
            .map_err(|e| format!("Failed to write zip: {}", e))?;

        println!("   Extracting with zip crate...");

        // Extract ZIP using zip crate
        let file = fs::File::open(&zip_path).map_err(|e| format!("Failed to open zip: {}", e))?;

        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip: {}", e))?;

        println!("   ZIP contains {} files/folders:", archive.len());
        for i in 0..archive.len() {
            if let Ok(f) = archive.by_index(i) {
                println!("     - {}", f.name());
            }
        }

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| format!("Failed to read file {}: {}", i, e))?;

            let outpath = bin_dir.join(file.name());

            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath).map_err(|e| format!("Failed to create dir: {}", e))?;
            } else {
                if let Some(p) = outpath.parent() {
                    fs::create_dir_all(p).map_err(|e| format!("Failed to create parent: {}", e))?;
                }
                let mut outfile = fs::File::create(&outpath)
                    .map_err(|e| format!("Failed to create file {}: {}", outpath.display(), e))?;
                std::io::copy(&mut file, &mut outfile)
                    .map_err(|e| format!("Failed to copy file: {}", e))?;

                if file.name().ends_with(".exe") {
                    println!("   Extracted: {}", file.name());
                }
            }
        }

        fs::remove_file(&zip_path).ok();

        // Check if exe exists
        if exe_path.exists() {
            println!("✅ whisper.exe ready");
            return Ok(exe_path);
        }

        // Search for it recursively
        println!("   Searching for whisper.exe...");
        fn find_exe(dir: &PathBuf) -> Option<PathBuf> {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.file_name().is_some_and(|n| n == "whisper.exe") {
                        return Some(path);
                    }
                    if path.is_dir() && let Some(found) = find_exe(&path) {
                        return Some(found);
                    }
                }
            }
            None
        }

        if let Some(found) = find_exe(&bin_dir) {
            println!("✅ Found whisper.exe at: {}", found.display());
            return Ok(found);
        }

        Err(format!("whisper.exe not found in {}", bin_dir.display()))
    }

    fn get_or_download_model(model_size: ModelSize) -> Result<PathBuf, String> {
        let app_data =
            std::env::var("APPDATA").unwrap_or_else(|_| std::env::var("HOME").unwrap_or_default());
        let models_dir = PathBuf::from(app_data).join("whisper-cpp");

        let model_filename = model_size.model_filename();
        let model_path = models_dir.join(model_filename);

        if model_path.exists() {
            println!("✅ Using cached {} model", model_size.name());
            return Ok(model_path);
        }

        fs::create_dir_all(&models_dir)
            .map_err(|e| format!("Failed to create models dir: {}", e))?;

        println!(
            "📥 Downloading {} model (this may take a moment)...",
            model_size.name()
        );

        let url = model_size.model_url();
        println!("   From: {}", url);

        let response = ureq::get(url).call().map_err(|e| {
            format!(
                "Failed to download {} model: {} (Check internet connection)",
                model_size.name(),
                e
            )
        })?;

        let mut file =
            fs::File::create(&model_path).map_err(|e| format!("Failed to create file: {}", e))?;

        std::io::copy(&mut response.into_reader(), &mut file)
            .map_err(|e| format!("Failed to write file: {}", e))?;

        println!("✅ {} model downloaded", model_size.name());
        Ok(model_path)
    }

    pub fn transcribe(
        &self,
        samples: &[f32],
        language: &str,
        show_timestamps: bool,
        cancel: Arc<AtomicBool>,
    ) -> Result<String, String> {
        if samples.is_empty() {
            return Err("No audio samples provided".to_string());
        }

        println!(
            "🎯 Running Whisper inference (timestamps: {})",
            show_timestamps
        );

        // Save audio to temp WAV file
        let temp_dir = std::env::temp_dir();
        let temp_wav = temp_dir.join("whisper_temp.wav");

        let samples_i16: Vec<i16> = samples
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = hound::WavWriter::create(&temp_wav, spec)
            .map_err(|e| format!("Failed to create WAV: {}", e))?;

        for sample in samples_i16 {
            writer
                .write_sample(sample)
                .map_err(|e| format!("Failed to write sample: {}", e))?;
        }

        writer
            .finalize()
            .map_err(|e| format!("Failed to finalize WAV: {}", e))?;

        // Debug: check WAV file size
        let wav_size = fs::metadata(&temp_wav).map(|m| m.len()).unwrap_or(0);
        println!(
            "   WAV file size: {} bytes ({:.1}s of audio)",
            wav_size,
            wav_size as f32 / (16000.0 * 2.0)
        );

        // Run main.exe (whisper.cpp)
        println!("   Running: {}", self.whisper_exe.display());
        println!("   Model: {}", self.model_path.display());
        println!("   Audio: {}", temp_wav.display());

        // Whisper adds .txt or .srt to the input filename
        let output_file = if show_timestamps {
            temp_wav.with_extension("wav.srt")
        } else {
            temp_wav.with_extension("wav.txt")
        };
        let _ = fs::remove_file(&output_file);

        let mut cmd = Command::new(&self.whisper_exe);

        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        cmd.current_dir(&temp_dir);
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        cmd.arg(temp_wav.file_name().unwrap().to_string_lossy().to_string())
            .arg("-m")
            .arg(self.model_path.to_string_lossy().to_string())
            .arg("-l")
            .arg(language);

        if show_timestamps {
            cmd.arg("-osrt");
        } else {
            cmd.arg("-otxt");
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to run main.exe: {}", e))?;

        // Poll until done or cancelled
        let status = loop {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Cancelled".to_string());
            }
            match child.try_wait().map_err(|e| format!("Process error: {}", e))? {
                Some(status) => break status,
                None => std::thread::sleep(std::time::Duration::from_millis(100)),
            }
        };

        if !status.success() {
            return Err(format!(
                "Whisper failed with code: {}",
                status.code().unwrap_or(-1)
            ));
        }

        // Check if output file exists
        if !output_file.exists() {
            return Err(format!(
                "Output file not created: {}. Whisper might have failed silently.",
                output_file.display()
            ));
        }

        // Read output file
        let result_text = fs::read_to_string(&output_file)
            .map_err(|e| format!("Failed to read output file {}: {}", output_file.display(), e))?;

        println!("✅ Transcription complete: {} chars", result_text.len());
        if result_text.is_empty() {
            println!("   ⚠️  Warning: Output file is empty! Whisper produced no text.");
        }

        // Cleanup
        let _ = fs::remove_file(&temp_wav);
        let _ = fs::remove_file(&output_file);

        Ok(result_text.trim().to_string())
    }
}
