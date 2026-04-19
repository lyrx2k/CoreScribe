# 🎙️ CoreScribe

**Blazingly fast, local-first voice transcription for students and researchers.** Built with 🦀 Rust, powered by OpenAI's Whisper via `WGPU`.

---

## 🎓 Why CoreScribe?

Transcribing lectures, interviews, or study group recordings can be expensive and time-consuming. Most services charge by the minute or require monthly subscriptions. **CoreScribe** changes that by putting the power of AI directly on your computer.

- **💰 Zero Cost:** No API keys, no subscriptions. It uses your own hardware.
- **🔒 Privacy First:** Your recordings never leave your device. Perfect for sensitive research interviews.
- **🚀 Student-Proof Performance:** Works on high-end gaming rigs (NVIDIA/AMD) and standard study laptops (Intel iGPU) alike.
- **⚡ Built for Speed:** No Python overhead. Native Windows performance.

---

## ✨ Features

- **Universal GPU Acceleration:** Optimized for NVIDIA, AMD, and Intel graphics via `WGPU`.
- **Offline Mode:** Once the model is downloaded, no internet connection is required.
- **Simple Workflow:** Drag and drop your audio files and get your text in seconds.
- **Automated Setup:** CoreScribe handles the heavy lifting, including model downloading and extraction.

---

## 🛠️ Tech Stack

CoreScribe is built on a modern, high-performance Rust stack:

- **UI:** `eframe` / `egui` for a responsive, hardware-accelerated interface.
- **Inference:** Whisper-based transcription with `WGPU` 0.19 backend.
- **Audio:** `hound` for precise WAV processing.
- **Async:** `tokio` to keep the UI fluid during heavy computations.
- **Systems:** `windows-rs` for deep OS integration.

---

## 🚀 Getting Started

1. **Download:** Grab the latest `.exe` from the [Releases](https://github.com/lyrx2k/CoreScribe/releases) page.
2. **Run:** Open `CoreScribe.exe`. 
3. **Setup:** On the first run, the app will automatically download the necessary AI models (approx. 150MB - 500MB depending on your choice).
4. **Transcribe:** Select your `.wav` file and let your GPU do the work!

*Note: Currently, CoreScribe supports `.wav` files. Support for `.mp3` and `.m4a` is coming soon.*

---

## 🤝 Contributing

This is an Alpha release, and we love feedback! Whether you are a fellow student developer or a Rust enthusiast:
- Report bugs via Issues.
- Suggest features (like PDF export or summary generation).
- Submit Pull Requests to improve the audio pipeline.

---

## 📄 License & Attribution

This project is licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0)**.

### Branding & Trademark Policy
CoreScribe is open-source, but its identity is protected. If you redistribute this software:
- **Attribution:** You must retain the original "About" section and a link to this repository.
- **Naming:** You may not use the name "CoreScribe" for derivative works without clear distinction.
- **Integrity:** Credits to the original author must remain visible in the UI.

---
*Made with ❤️ for students who value their time and privacy.*
