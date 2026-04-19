#![windows_subsystem = "windows"]

mod audio;
mod inference;

use eframe::egui;
use inference::{ModelSize, WhisperConfig, WhisperModel};
use std::sync::Arc;
use std::sync::Mutex;
fn main() -> Result<(), eframe::Error> {
    println!("🎙️ CoreScribe - Starting...");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        "CoreScribe",
        options,
        Box::new(|_| Box::<MyApp>::default()),
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProcessStage {
    Idle,
    Decoding,
    Resampling,
    Transcribing,
    Done,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tab {
    Transcriber,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Language {
    German,
    English,
    French,
    Spanish,
}

impl Language {
    fn code(&self) -> &'static str {
        match self {
            Language::German => "de",
            Language::English => "en",
            Language::French => "fr",
            Language::Spanish => "es",
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Language::German => "Deutsch",
            Language::English => "English",
            Language::French => "Français",
            Language::Spanish => "Español",
        }
    }
}

struct MyApp {
    active_tab: Tab,
    file_path: String,
    output_text: String,
    status: String,
    stage: ProcessStage,
    is_processing: bool,
    result: Arc<Mutex<Option<String>>>,
    model_size: ModelSize,
    language: Language,
    show_timestamps: bool,
    show_console: bool,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            active_tab: Tab::Transcriber,
            file_path: String::new(),
            output_text: String::new(),
            status: "Ready to transcribe".to_string(),
            stage: ProcessStage::Idle,
            is_processing: false,
            result: Arc::new(Mutex::new(None)),
            model_size: ModelSize::Tiny,
            language: Language::German,
            show_timestamps: true,
            show_console: false,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());
        ctx.set_pixels_per_point(1.2);

        // Check for completed transcription
        if let Ok(mut result_guard) = self.result.try_lock() {
            if let Some(result) = result_guard.take() {
                if result.starts_with("❌") {
                    self.stage = ProcessStage::Error;
                    self.status = result.clone();
                    self.output_text = result;
                } else {
                    self.stage = ProcessStage::Done;
                    self.status = "✅ Done!".to_string();
                    let filtered = if self.show_timestamps {
                        result
                    } else {
                        filter_timestamps(&result)
                    };
                    self.output_text = filtered;
                }
                self.is_processing = false;
                ctx.request_repaint();
            }
        }

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.active_tab == Tab::Transcriber, "🎙️ Transcriber")
                    .clicked()
                {
                    self.active_tab = Tab::Transcriber;
                }
                if ui
                    .selectable_label(self.active_tab == Tab::Settings, "⚙️ Settings")
                    .clicked()
                {
                    self.active_tab = Tab::Settings;
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.active_tab == Tab::Transcriber {
                self.show_transcriber_tab(ui, ctx);
            } else {
                self.show_settings_tab(ui);
            }
        });

        if self.is_processing {
            ctx.request_repaint();
        }
    }
}

impl MyApp {
    fn show_transcriber_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Header section
        ui.horizontal(|ui| {
            ui.heading("🎙️ CoreScribe");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_label("Model")
                    .selected_text(self.model_size.name())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.model_size, ModelSize::Tiny, "Tiny (Fast)");
                        ui.selectable_value(
                            &mut self.model_size,
                            ModelSize::Base,
                            "Base (Balanced)",
                        );
                        ui.selectable_value(
                            &mut self.model_size,
                            ModelSize::Small,
                            "Small (Accurate)",
                        );
                    });
                ui.label("  ");
                egui::ComboBox::from_label("Language")
                    .selected_text(self.language.name())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.language,
                            Language::German,
                            Language::German.name(),
                        );
                        ui.selectable_value(
                            &mut self.language,
                            Language::English,
                            Language::English.name(),
                        );
                        ui.selectable_value(
                            &mut self.language,
                            Language::French,
                            Language::French.name(),
                        );
                        ui.selectable_value(
                            &mut self.language,
                            Language::Spanish,
                            Language::Spanish.name(),
                        );
                    });
            });
        });
        ui.separator();

        // Drop zone - bigger and more prominent
        ui.add_space(20.0);
        let drop_zone_frame = egui::Frame::default()
            .fill(egui::Color32::from_gray(30))
            .stroke(egui::Stroke::new(2.0, egui::Color32::from_gray(80)))
            .rounding(8.0)
            .inner_margin(30.0);
        drop_zone_frame.show(ui, |ui| {
            ui.vertical_centered(|ui| {
                let drop_response = ui.heading("📂 Drop audio file here\nor click to browse");
                if drop_response.clicked() || ui.input(|i| i.key_pressed(egui::Key::Space)) {
                    if let Some(file) = rfd::FileDialog::new()
                        .add_filter("WAV Audio Files", &["wav"])
                        .pick_file()
                    {
                        self.file_path = file.display().to_string();
                        self.status = format!(
                            "Selected: {}",
                            file.file_name().unwrap_or_default().to_string_lossy()
                        );
                    }
                }
                let dropped_files = ui.input(|i| i.raw.dropped_files.clone());
                if !dropped_files.is_empty() {
                    if let Some(path) = &dropped_files[0].path {
                        self.file_path = path.display().to_string();
                        self.status = format!(
                            "Selected: {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        );
                    }
                }
            });
        });
        ui.add_space(18.0);

        // File info and options row
        ui.horizontal(|ui| {
            if !self.file_path.is_empty() {
                let filename = std::path::Path::new(&self.file_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                let file_frame = egui::Frame::default()
                    .fill(egui::Color32::from_rgb(40, 60, 80))
                    .rounding(4.0)
                    .inner_margin(8.0);
                file_frame.show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!("📄 {}", filename))
                            .color(egui::Color32::from_rgb(100, 180, 255)),
                    );
                });
            } else {
                ui.label("📄 No file selected");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ts_color = if self.show_timestamps {
                    egui::Color32::from_rgb(100, 200, 100)
                } else {
                    egui::Color32::GRAY
                };
                ui.colored_label(
                    ts_color,
                    format!(
                        "⏱ Timestamps: {}",
                        if self.show_timestamps { "ON" } else { "OFF" }
                    ),
                );
                ui.checkbox(&mut self.show_timestamps, "");
            });
        });
        ui.add_space(12.0);

        // Transcribe button - larger and centered
        let button_text = if self.is_processing {
            "⏳ Processing..."
        } else {
            "🚀 Transcribe"
        };
        let button_width = 340.0;
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() - button_width) / 2.0);
            if ui
                .add_sized([button_width, 54.0], egui::Button::new(button_text))
                .clicked()
                && !self.is_processing
                && !self.file_path.is_empty()
            {
                self.is_processing = true;
                self.stage = ProcessStage::Decoding;
                self.status = format!("Loading {} model...", self.model_size.name());
                self.output_text.clear();
                let file_path = self.file_path.clone();
                let result = Arc::clone(&self.result);
                let model_size = self.model_size;
                let language = self.language;
                let show_timestamps = self.show_timestamps;
                std::thread::spawn(move || {
                    match process_audio(&file_path, model_size, language, show_timestamps) {
                        Ok(transcription) => {
                            if let Ok(mut res) = result.lock() {
                                *res = Some(transcription);
                            }
                        }
                        Err(e) => {
                            if let Ok(mut res) = result.lock() {
                                *res = Some(format!("❌ Error: {}", e));
                            }
                        }
                    }
                });
            }
        });
        ui.add_space(12.0);

        // Progress stages
        ui.horizontal(|ui| {
            ui.label("Progress:");
            let stages = vec![
                ("●", ProcessStage::Decoding, "Decoding"),
                ("●", ProcessStage::Resampling, "Resampling"),
                ("●", ProcessStage::Transcribing, "Whisper"),
            ];
            for (i, (symbol, stage, label)) in stages.iter().enumerate() {
                let color = if matches!(self.stage, ProcessStage::Error) {
                    egui::Color32::RED
                } else if matches!(self.stage, ProcessStage::Done)
                    || self.stage as u8 >= *stage as u8
                {
                    egui::Color32::GREEN
                } else {
                    egui::Color32::DARK_GRAY
                };
                ui.colored_label(color, format!("{} {}", symbol, label));
                if i < stages.len() - 1 {
                    ui.label("→");
                }
            }
        });
        ui.separator();

        // Result section - take most of available space
        ui.add_space(12.0);
        ui.label("📝 Transcription Result");
        ui.add_space(6.0);

        let available_height = ui.available_height() - 50.0;
        egui::TextEdit::multiline(&mut self.output_text)
            .desired_rows((available_height / 20.0) as usize)
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Body)
            .show(ui);

        // Footer with copy button and status
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if !self.output_text.is_empty() && ui.button("📋 Copy").clicked() {
                ctx.output_mut(|o| o.copied_text = self.output_text.clone());
                self.status = "✅ Copied to clipboard!".to_string();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(&self.status);
            });
        });
    }

    fn show_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("⚙️ Settings");
        ui.separator();

        ui.label("Console Visibility:");
        ui.horizontal(|ui| {
            ui.label("Show console window on startup:");
            ui.checkbox(&mut self.show_console, "");
        });

        if ui.button("Open Console Now").clicked() {
            #[cfg(target_os = "windows")]
            {
                use std::process::Command;
                let _ = Command::new("cmd")
                    .arg("/k")
                    .arg("echo Console opened for debugging. You can close this window anytime.")
                    .spawn();
            }
        }

        ui.separator();
        ui.label("Cache & Data:");
        if ui.button("🗑️ Delete All Downloaded Data").clicked() {
            delete_all_cache();
            std::process::exit(0);
        }
        ui.label("This will delete:");
        ui.label("  • Whisper binary (~2.4MB)");
        ui.label("  • All downloaded models (~500MB)");

        ui.separator();
        ui.label("About");
        ui.label("CoreScribe v0.1.0");
        ui.label("Local speech-to-text with Whisper.cpp");
    }
}

fn process_audio(
    file_path: &str,
    model_size: ModelSize,
    language: Language,
    show_timestamps: bool,
) -> Result<String, String> {
    println!("📥 Decoding audio: {}", file_path);
    let audio_data = audio::decode_audio(file_path)?;
    println!(
        "✅ Decoded: {} samples @ {}Hz, {} channels",
        audio_data.samples.len(),
        audio_data.sample_rate,
        audio_data.channels
    );

    println!("🔄 Resampling to 16kHz mono...");
    let resampled = audio::resample_to_whisper(&audio_data)?;
    println!("✅ Resampled: {} samples", resampled.len());

    println!("🧠 Initializing Whisper model...");
    let config = WhisperConfig { model_size };
    let model = WhisperModel::new(config)?;

    println!("🎯 Running transcription...");
    let result = model.transcribe(&resampled, language.code(), show_timestamps)?;

    println!("✅ Sending result to UI...");
    Ok(result)
}

fn filter_timestamps(text: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with('[') && !line.trim().is_empty())
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn delete_all_cache() {
    use std::fs;
    use std::path::PathBuf;

    let app_data =
        std::env::var("APPDATA").unwrap_or_else(|_| std::env::var("HOME").unwrap_or_default());

    let bin_dir = PathBuf::from(&app_data).join("whisper-cpp-bin");
    let models_dir = PathBuf::from(&app_data).join("whisper-cpp");

    println!("🗑️  Deleting cache directories...");

    if bin_dir.exists() {
        let _ = fs::remove_dir_all(&bin_dir);
        println!("✅ Deleted: {}", bin_dir.display());
    }

    if models_dir.exists() {
        let _ = fs::remove_dir_all(&models_dir);
        println!("✅ Deleted: {}", models_dir.display());
    }

    println!("✅ All cache deleted. Closing app...");
}
