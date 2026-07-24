//! Encode a rendered mix (interleaved `f32`) to WAV, FLAC, or (optionally) MP3.

use std::io::Cursor;

use thiserror::Error;

/// Output container/codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Wav,
    Flac,
    #[cfg(feature = "mp3")]
    Mp3,
}

/// Output bit depth for the PCM formats (WAV/FLAC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitDepth {
    Sixteen,
    TwentyFour,
}

impl BitDepth {
    fn bits(self) -> u32 {
        match self {
            BitDepth::Sixteen => 16,
            BitDepth::TwentyFour => 24,
        }
    }

    /// Max positive integer amplitude for this depth.
    fn scale(self) -> f32 {
        // e.g. 16-bit -> 32767, 24-bit -> 8388607
        ((1i64 << (self.bits() - 1)) - 1) as f32
    }
}

#[derive(Debug, Error)]
pub enum EncodeError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("wav encode error: {0}")]
    Wav(#[from] hound::Error),
    #[error("flac encode error: {0}")]
    Flac(String),
    #[error("mp3 encode error: {0}")]
    Mp3(String),
    #[error("invalid channel count: {0}")]
    Channels(u16),
}

/// Clamp and convert a single `f32` sample to an integer at the given depth.
fn to_int(sample: f32, scale: f32) -> i32 {
    (sample.clamp(-1.0, 1.0) * scale).round() as i32
}

/// Encode interleaved `f32` samples to the requested format.
///
/// * `samples` — interleaved by frame: `[c0,c1, c0,c1, ...]` for stereo.
/// * `channels` — 1 (mono) or 2 (stereo).
pub fn encode_interleaved(
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    format: ExportFormat,
    bit_depth: BitDepth,
) -> Result<Vec<u8>, EncodeError> {
    if channels == 0 || channels > 2 {
        return Err(EncodeError::Channels(channels));
    }
    match format {
        ExportFormat::Wav => encode_wav(samples, channels, sample_rate, bit_depth),
        ExportFormat::Flac => encode_flac(samples, channels, sample_rate, bit_depth),
        #[cfg(feature = "mp3")]
        ExportFormat::Mp3 => encode_mp3(samples, channels, sample_rate),
    }
}

fn encode_wav(
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    bit_depth: BitDepth,
) -> Result<Vec<u8>, EncodeError> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: bit_depth.bits() as u16,
        sample_format: hound::SampleFormat::Int,
    };
    let scale = bit_depth.scale();
    let mut cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)?;
        match bit_depth {
            BitDepth::Sixteen => {
                for &s in samples {
                    writer.write_sample(to_int(s, scale) as i16)?;
                }
            }
            BitDepth::TwentyFour => {
                for &s in samples {
                    // hound writes the low 24 bits when bits_per_sample == 24.
                    writer.write_sample(to_int(s, scale))?;
                }
            }
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

fn encode_flac(
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    bit_depth: BitDepth,
) -> Result<Vec<u8>, EncodeError> {
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;

    let bits = bit_depth.bits() as usize;
    let scale = bit_depth.scale();
    let ints: Vec<i32> = samples.iter().map(|&s| to_int(s, scale)).collect();

    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|e| EncodeError::Flac(format!("{e:?}")))?;

    let source = flacenc::source::MemSource::from_samples(
        &ints,
        channels as usize,
        bits,
        sample_rate as usize,
    );

    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| EncodeError::Flac(format!("{e:?}")))?;

    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|e| EncodeError::Flac(format!("{e:?}")))?;
    Ok(sink.as_slice().to_vec())
}

/// Headroom (linear gain) to try, in order, when an MP3 encode fails. A
/// full-scale, energetic passage can drive Shine's fixed-point MDCT to
/// `i32::MIN`, where its `.abs()` overflows — a panic in a checked build, a
/// wrapped (mis-quantized) coefficient otherwise. Pulling the input down keeps
/// the transform off that edge.
///
/// The ladder starts imperceptibly gentle: real music encodes at full scale or
/// clears within ~0.5 dB (a hard-panned quartet mix that peaked at 0 dBFS needed
/// -0.2 dB). The deeper steps only ever apply to near-pure-tone material a
/// listener would never mistake for a normal recording, and exist so *something*
/// usable always comes out rather than a hard failure.
#[cfg(feature = "mp3")]
const MP3_HEADROOM_STEPS: [f32; 6] = [
    1.0, 0.977, // -0.2 dB
    0.944, // -0.5 dB
    0.891, // -1 dB
    0.794, // -2 dB
    0.707, // -3 dB
];

#[cfg(feature = "mp3")]
fn encode_mp3(samples: &[f32], channels: u16, sample_rate: u32) -> Result<Vec<u8>, EncodeError> {
    // Shine's overflow is expected and handled below, so keep its panic message
    // out of the console — otherwise a successful (retried) export looks like a
    // crash. Restored when this returns. Encodes don't overlap in this app, so
    // swapping the process-global hook here is safe.
    let _silence = SilencedPanics::new();

    let mut last: Option<String> = None;
    for headroom in MP3_HEADROOM_STEPS {
        match encode_mp3_at(samples, channels, sample_rate, headroom) {
            Ok(bytes) => return Ok(bytes),
            Err(e) => last = Some(e),
        }
    }
    Err(EncodeError::Mp3(format!(
        "shine encoder failed even with headroom ({})",
        last.unwrap_or_else(|| "unknown".into())
    )))
}

#[cfg(feature = "mp3")]
type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>;

/// Suppresses the default panic hook for its lifetime, restoring it on drop, so
/// a caught-and-retried Shine overflow doesn't spam the console.
#[cfg(feature = "mp3")]
struct SilencedPanics(Option<PanicHook>);

#[cfg(feature = "mp3")]
impl SilencedPanics {
    fn new() -> Self {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        SilencedPanics(Some(previous))
    }
}

#[cfg(feature = "mp3")]
impl Drop for SilencedPanics {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            std::panic::set_hook(previous);
        }
    }
}

/// One MP3 encode attempt at the given headroom. Shine is pure Rust and each
/// call builds a fresh encoder, so catching a panic here leaves nothing in a bad
/// state — we can simply try again with more headroom.
#[cfg(feature = "mp3")]
fn encode_mp3_at(
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    headroom: f32,
) -> Result<Vec<u8>, String> {
    use shine_rs::{encode_pcm_to_mp3, Mp3EncoderConfig, StereoMode};

    // Shine expects interleaved i16 PCM.
    let pcm: Vec<i16> = samples
        .iter()
        .map(|&s| ((s * headroom).clamp(-1.0, 1.0) * 32767.0).round() as i16)
        .collect();

    let config = Mp3EncoderConfig {
        sample_rate,
        bitrate: 256,
        channels: channels as u8,
        stereo_mode: if channels >= 2 {
            StereoMode::JointStereo
        } else {
            StereoMode::Mono
        },
        copyright: false,
        original: true,
    };

    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        encode_pcm_to_mp3(config, &pcm).map_err(|e| format!("{e:?}"))
    }))
    .unwrap_or_else(|_| Err("panicked (fixed-point overflow)".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_int_clamps_and_scales() {
        let scale = BitDepth::Sixteen.scale();
        assert_eq!(to_int(0.0, scale), 0);
        assert_eq!(to_int(1.0, scale), 32767);
        assert_eq!(to_int(2.0, scale), 32767); // clamped
        assert_eq!(to_int(-2.0, scale), -32767); // clamped
    }

    #[test]
    fn rejects_bad_channel_count() {
        let r = encode_interleaved(&[0.0], 3, 44_100, ExportFormat::Wav, BitDepth::Sixteen);
        assert!(matches!(r, Err(EncodeError::Channels(3))));
    }

    #[test]
    fn wav_has_riff_header() {
        let bytes = encode_interleaved(
            &[0.0, 0.1, -0.1, 0.2],
            2,
            44_100,
            ExportFormat::Wav,
            BitDepth::Sixteen,
        )
        .expect("wav encode");
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
    }

    #[test]
    fn flac_has_magic() {
        let samples: Vec<f32> = (0..4096).map(|i| ((i as f32) * 0.01).sin() * 0.5).collect();
        let bytes = encode_interleaved(&samples, 1, 44_100, ExportFormat::Flac, BitDepth::Sixteen)
            .expect("flac encode");
        assert_eq!(&bytes[0..4], b"fLaC");
    }

    #[cfg(feature = "mp3")]
    #[test]
    fn mp3_produces_a_frame() {
        let samples: Vec<f32> = (0..44_100)
            .map(|i| ((i as f32) * 0.05).sin() * 0.5)
            .collect();
        let bytes = encode_interleaved(&samples, 1, 44_100, ExportFormat::Mp3, BitDepth::Sixteen)
            .expect("mp3 encode");
        assert!(!bytes.is_empty());
        // MP3 stream begins with an ID3 tag ("ID3") or an MPEG frame sync (0xFF 0xEx).
        let starts_with_id3 = bytes.starts_with(b"ID3");
        let starts_with_sync = bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0;
        assert!(starts_with_id3 || starts_with_sync, "not an MP3 stream");
    }

    #[cfg(feature = "mp3")]
    #[test]
    fn mp3_survives_a_full_scale_signal() {
        // A sustained full-scale tone drives Shine's fixed-point MDCT to
        // `i32::MIN`, where its `.abs()` overflows — this used to crash the app
        // (checked build) or silently corrupt a coefficient (release). The
        // encoder now backs the level off until it clears, so a hot export still
        // produces a valid MP3 instead of failing. (A hard-panned quartet mix
        // that peaked at 0 dBFS was the real-world trigger.)
        let sr = 44_100;
        let sine: Vec<f32> = (0..sr * 2)
            .map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / sr as f32).sin())
            .collect();

        let bytes = encode_interleaved(&sine, 1, sr, ExportFormat::Mp3, BitDepth::Sixteen)
            .expect("full-scale signal should still encode");
        assert!(!bytes.is_empty());
        let starts_with_id3 = bytes.starts_with(b"ID3");
        let starts_with_sync = bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0;
        assert!(starts_with_id3 || starts_with_sync, "not an MP3 stream");
    }
}
