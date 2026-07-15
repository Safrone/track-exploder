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

#[cfg(feature = "mp3")]
fn encode_mp3(samples: &[f32], channels: u16, sample_rate: u32) -> Result<Vec<u8>, EncodeError> {
    use mp3lame_encoder::{Builder, FlushNoGap, Id3Tag, InterleavedPcm};

    let mut builder = Builder::new().ok_or_else(|| EncodeError::Mp3("builder init".into()))?;
    builder
        .set_num_channels(channels as u8)
        .map_err(|e| EncodeError::Mp3(format!("channels: {e:?}")))?;
    builder
        .set_sample_rate(sample_rate)
        .map_err(|e| EncodeError::Mp3(format!("sample_rate: {e:?}")))?;
    builder
        .set_brate(mp3lame_encoder::Bitrate::Kbps256)
        .map_err(|e| EncodeError::Mp3(format!("bitrate: {e:?}")))?;
    let _ = builder.set_id3_tag(Id3Tag {
        title: b"",
        artist: b"",
        album: b"",
        year: b"",
        comment: b"Track Exploder",
        album_art: b"",
    });
    let mut encoder = builder
        .build()
        .map_err(|e| EncodeError::Mp3(format!("build: {e:?}")))?;

    // LAME expects i16 PCM.
    let pcm: Vec<i16> = samples
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0).round() as i16)
        .collect();

    let mut out = Vec::with_capacity(mp3lame_encoder::max_required_buffer_size(pcm.len()));
    let encoded = encoder
        .encode(InterleavedPcm(&pcm), out.spare_capacity_mut())
        .map_err(|e| EncodeError::Mp3(format!("encode: {e:?}")))?;
    unsafe { out.set_len(encoded) };

    let flushed = encoder
        .flush::<FlushNoGap>(out.spare_capacity_mut())
        .map_err(|e| EncodeError::Mp3(format!("flush: {e:?}")))?;
    unsafe { out.set_len(out.len() + flushed) };
    Ok(out)
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
}
