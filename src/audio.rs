use std::path::Path;

#[derive(Debug, Clone)]
pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub fn decode_audio<P: AsRef<Path>>(path: P) -> Result<AudioData, String> {
    let path = path.as_ref();
    let path_str = path.to_string_lossy();

    if path_str.ends_with(".wav") || path_str.ends_with(".WAV") {
        decode_wav(path)
    } else {
        Err("Only .wav files are supported. Please convert your audio to WAV format.".to_string())
    }
}

fn decode_wav<P: AsRef<Path>>(path: P) -> Result<AudioData, String> {
    let path = path.as_ref();
    let reader = hound::WavReader::open(path).map_err(|e| format!("Failed to open WAV: {}", e))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels;

    let mut samples = Vec::new();

    match spec.bits_per_sample {
        16 => {
            let reader =
                hound::WavReader::open(path).map_err(|e| format!("Failed to reopen WAV: {}", e))?;
            for sample in reader.into_samples::<i16>() {
                let s = sample.map_err(|e| format!("Sample error: {}", e))?;
                samples.push(s as f32 / i16::MAX as f32);
            }
        }
        24 => {
            let reader =
                hound::WavReader::open(path).map_err(|e| format!("Failed to reopen WAV: {}", e))?;
            for sample in reader.into_samples::<i32>() {
                let s = sample.map_err(|e| format!("Sample error: {}", e))?;
                samples.push((s >> 8) as f32 / I24::MAX as f32);
            }
        }
        32 => {
            let reader =
                hound::WavReader::open(path).map_err(|e| format!("Failed to reopen WAV: {}", e))?;
            for sample in reader.into_samples::<i32>() {
                let s = sample.map_err(|e| format!("Sample error: {}", e))?;
                samples.push(s as f32 / i32::MAX as f32);
            }
        }
        _ => return Err(format!("Unsupported bit depth: {}", spec.bits_per_sample)),
    }

    if samples.is_empty() {
        return Err("No audio samples found".to_string());
    }

    Ok(AudioData {
        samples,
        sample_rate,
        channels,
    })
}


struct I24;
impl I24 {
    const MAX: f32 = 8388607.0;
}

pub fn resample_to_whisper(audio: &AudioData) -> Result<Vec<f32>, String> {
    const TARGET_SAMPLE_RATE: u32 = 16000;

    if audio.sample_rate == TARGET_SAMPLE_RATE && audio.channels == 1 {
        return Ok(audio.samples.clone());
    }

    // Step 1: Convert to mono FIRST
    let mono = if audio.channels > 1 {
        let mut mono = Vec::with_capacity(audio.samples.len() / audio.channels as usize);
        for chunk in audio.samples.chunks(audio.channels as usize) {
            let avg: f32 = chunk.iter().sum::<f32>() / chunk.len() as f32;
            mono.push(avg);
        }
        mono
    } else {
        audio.samples.clone()
    };

    // Step 2: Resample to 16kHz
    let resampled = if audio.sample_rate != TARGET_SAMPLE_RATE {
        let ratio = TARGET_SAMPLE_RATE as f32 / audio.sample_rate as f32;
        let new_len = (mono.len() as f32 * ratio) as usize;
        let mut resampled = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let src_pos = i as f32 / ratio;
            let idx = src_pos.floor() as usize;
            let frac = src_pos.fract();

            let sample = if idx + 1 < mono.len() {
                let s1 = mono[idx];
                let s2 = mono[idx + 1];
                s1 * (1.0 - frac) + s2 * frac
            } else if idx < mono.len() {
                mono[idx]
            } else {
                0.0
            };

            resampled.push(sample);
        }
        resampled
    } else {
        mono
    };

    Ok(resampled)
}
