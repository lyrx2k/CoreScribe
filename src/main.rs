#![windows_subsystem = "windows"]

mod audio;
mod inference;
mod updater;

use eframe::egui;
use inference::{ModelSize, WhisperConfig, WhisperModel};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
fn main() -> Result<(), eframe::Error> {
    println!("🎙️ CoreScribe - Starting...");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native("CoreScribe", options, Box::new(|_| Box::<MyApp>::default()))
}

#[derive(Debug, Clone, PartialEq)]
enum PendingAction {
    None,
    DeleteCache,
    UpdateApp(String),
}

struct AppDialog {
    title: String,
    message: String,
    is_warning: bool,
    action: PendingAction,
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

#[derive(Debug, Clone, PartialEq)]
enum UpdateStatus {
    Idle,
    UpToDate,
    Available(String),
    Error(String),
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
    auto_save: bool,
    save_pending: bool,
    cancel_flag: Arc<AtomicBool>,
    pending_dialog: Option<AppDialog>,
    update_status: UpdateStatus,
    auto_checked: bool,
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
            auto_save: false,
            save_pending: false,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            pending_dialog: None,
            update_status: UpdateStatus::Idle,
            auto_checked: false,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());
        ctx.set_pixels_per_point(1.2);

        // Auto-check for updates on first run
        if !self.auto_checked {
            self.auto_checked = true;
            match updater::check_for_update() {
                Ok(Some(latest)) => {
                    self.pending_dialog = Some(AppDialog {
                        title: "Update Available".to_string(),
                        message: format!(
                            "CoreScribe {} is available.\n\nDownload and install?",
                            latest
                        ),
                        is_warning: false,
                        action: PendingAction::UpdateApp(latest.clone()),
                    });
                    self.update_status = UpdateStatus::Available(latest);
                }
                Ok(None) => {
                    self.update_status = UpdateStatus::UpToDate;
                }
                Err(_) => {
                    self.update_status = UpdateStatus::Idle;
                }
            }
        }

        // Check for completed transcription
        if let Ok(mut result_guard) = self.result.try_lock()
            && let Some(result) = result_guard.take()
        {
            if result == "CANCELLED" || result == "ERROR: Cancelled" {
                self.stage = ProcessStage::Idle;
                self.status = "Cancelled".to_string();
            } else if result.starts_with("ERROR: ") {
                self.stage = ProcessStage::Error;
                let error_msg = result.trim_start_matches("ERROR: ").to_string();
                self.status = format!("Error: {}", error_msg);
                self.output_text = String::new();
                let (title, is_warning) = classify_error(&error_msg);
                self.pending_dialog = Some(AppDialog {
                    title: title.to_string(),
                    message: error_msg.clone(),
                    is_warning,
                    action: PendingAction::None,
                });
            } else {
                self.stage = ProcessStage::Done;
                self.status = if self.auto_save { "Done! Saving...".to_string() } else { "Done!".to_string() };
                let filtered = if self.show_timestamps {
                    result
                } else {
                    filter_timestamps(&result)
                };
                self.output_text = filtered;
                if self.auto_save {
                    self.save_pending = true;
                }
            }
            self.is_processing = false;
            ctx.request_repaint();
        }

        // Auto-save dialog after transcription
        if self.save_pending && !self.output_text.is_empty() {
            self.save_pending = false;
            let stem = std::path::Path::new(&self.file_path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let default_name = format!("{}_transcript.txt", stem);
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name(&default_name)
                .add_filter("Text Files", &["txt"])
                .save_file()
            {
                match std::fs::write(&path, &self.output_text) {
                    Ok(_) => {
                        self.status = format!(
                            "Saved: {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        )
                    }
                    Err(e) => self.status = format!("Save failed: {}", e),
                }
            } else {
                self.status = "Done!".to_string();
            }
        }

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.active_tab == Tab::Transcriber, "Transcriber")
                    .clicked()
                {
                    self.active_tab = Tab::Transcriber;
                }
                if ui
                    .selectable_label(self.active_tab == Tab::Settings, "Settings")
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

        // Egui-native modal dialog
        if self.pending_dialog.is_some() {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::PanelResizeLine,
                egui::Id::new("modal_bg"),
            ));
            painter.rect_filled(ctx.screen_rect(), 0.0, egui::Color32::from_black_alpha(180));

            let dialog_open = self.pending_dialog.is_some();
            if dialog_open {
                let mut close = false;
                let mut confirmed = false;
                egui::Window::new(
                    self.pending_dialog
                        .as_ref()
                        .map(|d| d.title.as_str())
                        .unwrap_or(""),
                )
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    if let Some(dialog) = &self.pending_dialog {
                        let icon = if dialog.is_warning { "(!)" } else { "(X)" };
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(icon).size(24.0));
                            ui.add_space(8.0);
                            ui.label(&dialog.message);
                        });
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);
                        if dialog.action == PendingAction::None {
                            ui.vertical_centered(|ui| {
                                if ui.button("  OK  ").clicked() {
                                    close = true;
                                }
                            });
                        } else {
                            ui.horizontal(|ui| {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("  No  ").clicked() {
                                            close = true;
                                        }
                                        let yes_btn = egui::Button::new("  Yes  ")
                                            .fill(egui::Color32::from_rgb(160, 40, 40));
                                        if ui.add(yes_btn).clicked() {
                                            confirmed = true;
                                            close = true;
                                        }
                                    },
                                );
                            });
                        }
                        ui.add_space(4.0);
                    }
                });
                if close {
                    if confirmed {
                        if let Some(dialog) = self.pending_dialog.take() {
                            match dialog.action {
                                PendingAction::DeleteCache => {
                                    delete_all_cache();
                                    std::process::exit(0);
                                }
                                PendingAction::UpdateApp(tag) => {
                                    if let Err(e) = updater::download_and_install(&tag) {
                                        self.pending_dialog = Some(AppDialog {
                                            title: "Update Failed".to_string(),
                                            message: format!("Failed to update: {}", e),
                                            is_warning: true,
                                            action: PendingAction::None,
                                        });
                                    }
                                }
                                PendingAction::None => {}
                            }
                        }
                    } else {
                        self.pending_dialog = None;
                    }
                }
            }
        }

        if self.is_processing {
            ctx.request_repaint();
        }
    }
}

impl MyApp {
    fn show_transcriber_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Header section
        ui.horizontal(|ui| {
            ui.heading("CoreScribe");
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
                let drop_response = ui.heading("Drop audio file here\nor click to browse");
                if (drop_response.clicked() || ui.input(|i| i.key_pressed(egui::Key::Space)))
                    && let Some(file) = rfd::FileDialog::new()
                        .add_filter("WAV Audio Files", &["wav"])
                        .pick_file()
                {
                    self.file_path = file.display().to_string();
                    self.status = format!(
                        "Selected: {}",
                        file.file_name().unwrap_or_default().to_string_lossy()
                    );
                }
                let dropped_files = ui.input(|i| i.raw.dropped_files.clone());
                if !dropped_files.is_empty()
                    && let Some(path) = &dropped_files[0].path
                {
                    self.file_path = path.display().to_string();
                    self.status = format!(
                        "Selected: {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );
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
                        egui::RichText::new(filename.to_string())
                            .color(egui::Color32::from_rgb(100, 180, 255)),
                    );
                });
            } else {
                ui.label("No file selected");
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
                        "Timestamps: {}",
                        if self.show_timestamps { "ON" } else { "OFF" }
                    ),
                );
                ui.checkbox(&mut self.show_timestamps, "");
                ui.separator();
                let save_color = if self.auto_save {
                    egui::Color32::from_rgb(100, 200, 100)
                } else {
                    egui::Color32::GRAY
                };
                ui.colored_label(
                    save_color,
                    format!("Auto-Save: {}", if self.auto_save { "ON" } else { "OFF" }),
                );
                ui.checkbox(&mut self.auto_save, "");
            });
        });
        ui.add_space(12.0);

        // Transcribe + Cancel buttons
        let button_width = if self.is_processing { 260.0 } else { 340.0 };
        ui.horizontal(|ui| {
            ui.add_space(
                (ui.available_width()
                    - button_width
                    - if self.is_processing { 110.0 } else { 0.0 })
                    / 2.0,
            );
            if ui
                .add_sized(
                    [button_width, 54.0],
                    egui::Button::new(if self.is_processing {
                        "Processing..."
                    } else {
                        "Transcribe"
                    }),
                )
                .clicked()
                && !self.is_processing
                && !self.file_path.is_empty()
            {
                self.is_processing = true;
                self.stage = ProcessStage::Decoding;
                self.status = format!("Loading {} model...", self.model_size.name());
                self.output_text.clear();
                self.cancel_flag.store(false, Ordering::Relaxed);
                let file_path = self.file_path.clone();
                let result = Arc::clone(&self.result);
                let model_size = self.model_size;
                let language = self.language;
                let show_timestamps = self.show_timestamps;
                let cancel = Arc::clone(&self.cancel_flag);
                std::thread::spawn(move || {
                    if cancel.load(Ordering::Relaxed) {
                        if let Ok(mut res) = result.lock() {
                            *res = Some("CANCELLED".to_string());
                        }
                        return;
                    }
                    match process_audio(
                        &file_path,
                        model_size,
                        language,
                        show_timestamps,
                        Arc::clone(&cancel),
                    ) {
                        Ok(transcription) => {
                            if let Ok(mut res) = result.lock() {
                                *res = Some(transcription);
                            }
                        }
                        Err(e) => {
                            if let Ok(mut res) = result.lock() {
                                *res = Some(format!("ERROR: {}", e));
                            }
                        }
                    }
                });
            }
            if self.is_processing {
                ui.add_space(8.0);
                let cancel_btn =
                    egui::Button::new("Cancel").fill(egui::Color32::from_rgb(160, 40, 40));
                if ui.add_sized([100.0, 54.0], cancel_btn).clicked() {
                    self.cancel_flag.store(true, Ordering::Relaxed);
                    self.is_processing = false;
                    self.stage = ProcessStage::Idle;
                    self.status = "Cancelled".to_string();
                    ctx.request_repaint();
                }
            }
        });
        ui.add_space(12.0);

        // Progress stages
        ui.horizontal(|ui| {
            ui.label("Progress:");
            let stages = [
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
        ui.label("Transcription Result");
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
            if !self.output_text.is_empty() && ui.button("Copy").clicked() {
                ctx.output_mut(|o| o.copied_text = self.output_text.clone());
                self.status = "Copied to clipboard!".to_string();
            }
            if !self.output_text.is_empty() && ui.button("Save As").clicked() {
                let stem = std::path::Path::new(&self.file_path)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let default_name = format!("{}_transcript.txt", stem);
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name(&default_name)
                    .add_filter("Text Files", &["txt"])
                    .save_file()
                {
                    match std::fs::write(&path, &self.output_text) {
                        Ok(_) => {
                            self.status = format!(
                                "Saved: {}",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            )
                        }
                        Err(e) => self.status = format!("Save failed: {}", e),
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let status_color = if self.status.starts_with("Copied")
                    || self.status.starts_with("Saved")
                    || self.status.starts_with("Done")
                {
                    egui::Color32::from_rgb(100, 200, 100)
                } else if self.status.starts_with("Cancelled") || self.status.starts_with("Error") {
                    egui::Color32::from_rgb(220, 80, 80)
                } else {
                    egui::Color32::GRAY
                };
                ui.colored_label(status_color, &self.status);
            });
        });
    }

    fn show_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
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
        if ui
            .add(
                egui::Button::new("Delete All Downloaded Data")
                    .fill(egui::Color32::from_rgb(160, 40, 40)),
            )
            .clicked()
        {
            self.pending_dialog = Some(AppDialog {
                title: "Delete All Downloaded Data".to_string(),
                message: "This will delete all downloaded models and the Whisper binary.\nThe app will close afterwards.\n\nAre you sure?".to_string(),
                is_warning: true,
                action: PendingAction::DeleteCache,
            });
        }
        ui.label("This will delete:");
        ui.label("  • Whisper binary (~2.4MB)");
        ui.label("  • All downloaded models (~500MB)");

        ui.separator();
        ui.label("About");
        let version = env!("CARGO_PKG_VERSION");
        ui.horizontal(|ui| {
            ui.label(format!("CoreScribe v{} by lyrx2k", version));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match &self.update_status {
                    UpdateStatus::Idle => {
                        if ui.button("Check for Updates").clicked() {
                            match updater::check_for_update() {
                                Ok(Some(latest)) => {
                                    self.update_status = UpdateStatus::Available(latest);
                                }
                                Ok(None) => {
                                    self.update_status = UpdateStatus::UpToDate;
                                }
                                Err(e) => {
                                    self.update_status = UpdateStatus::Error(e);
                                }
                            }
                        }
                    }
                    UpdateStatus::UpToDate => {
                        ui.colored_label(
                            egui::Color32::from_rgb(100, 200, 100),
                            "You are up to date",
                        );
                    }
                    UpdateStatus::Available(tag) => {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 180, 80),
                            format!("{} available", tag),
                        );
                        if ui.button("Update Now").clicked() {
                            self.pending_dialog = Some(AppDialog {
                                title: "Update CoreScribe".to_string(),
                                message: format!(
                                    "A new version ({}) is available.\n\nThe app will download and restart.\n\nProceed?",
                                    tag
                                ),
                                is_warning: false,
                                action: PendingAction::UpdateApp(tag.clone()),
                            });
                        }
                    }
                    UpdateStatus::Error(e) => {
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 80, 80),
                            format!("Error: {}", e),
                        );
                    }
                }
            });
        });
        ui.label("Local speech-to-text with Whisper.cpp");
    }
}

fn process_audio(
    file_path: &str,
    model_size: ModelSize,
    language: Language,
    show_timestamps: bool,
    cancel: Arc<AtomicBool>,
) -> Result<String, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }
    println!("📥 Decoding audio: {}", file_path);
    let audio_data = audio::decode_audio(file_path)?;

    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }
    println!("🔄 Resampling to 16kHz mono...");
    let resampled = audio::resample_to_whisper(&audio_data)?;

    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }
    println!("🧠 Initializing Whisper model...");
    let config = WhisperConfig { model_size };
    let model = WhisperModel::new(config)?;

    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }
    println!("🎯 Running transcription...");
    model.transcribe(&resampled, language.code(), show_timestamps, cancel)
}

fn classify_error(message: &str) -> (&'static str, bool) {
    let msg = message.to_lowercase();
    if msg.contains("not supported")
        || msg.contains("unsupported")
        || msg.contains("no audio track")
        || msg.contains("format probe")
        || msg.contains("bits_per_sample")
    {
        ("Unsupported Format", true)
    } else if msg.contains("failed to download")
        || msg.contains("internet")
        || msg.contains("connection")
    {
        ("Network Error", false)
    } else if msg.contains("no audio samples") || msg.contains("empty") || msg.contains("too short")
    {
        ("Empty Audio", true)
    } else if msg.contains("whisper failed")
        || msg.contains("failed to run")
        || msg.contains("not found")
    {
        ("Whisper Error", false)
    } else {
        ("Error", false)
    }
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

    println!("🗑️ Deleting cache directories...");

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
