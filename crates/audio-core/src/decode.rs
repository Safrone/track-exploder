//! Decode any supported audio file into per-channel (planar) `f32` PCM using Symphonia.

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use thiserror::Error;

/// Decoded audio as planar (per-channel) `f32` samples in the range `[-1.0, 1.0]`.
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    pub sample_rate: u32,
    pub channels: usize,
    /// Number of frames (samples per channel).
    pub frames: usize,
    /// `planar[ch]` is the sample buffer for channel `ch`, each of length `frames`.
    pub planar: Vec<Vec<f32>>,
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("audio decode error: {0}")]
    Symphonia(#[from] SymphoniaError),
    #[error("no decodable audio track found in file")]
    NoTrack,
    #[error("file contained no audio samples")]
    Empty,
}

/// Decode the file at `path` into planar `f32` PCM.
pub fn decode_file(path: &Path) -> Result<DecodedAudio, DecodeError> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(DecodeError::NoTrack)?;
    let track_id = track.id;

    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    let mut sample_rate = track.codec_params.sample_rate.unwrap_or(0);
    let mut channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(0);
    let mut planar: Vec<Vec<f32>> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            // Clean end-of-stream.
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                let spec = *audio_buf.spec();
                if sample_rate == 0 {
                    sample_rate = spec.rate;
                }
                if channels == 0 {
                    channels = spec.channels.count();
                }
                let ch = channels.max(1);
                if planar.is_empty() {
                    planar = vec![Vec::new(); ch];
                }

                // Convert this packet's audio to interleaved f32, then de-interleave.
                let mut sbuf = SampleBuffer::<f32>::new(audio_buf.capacity() as u64, spec);
                sbuf.copy_interleaved_ref(audio_buf);
                for (i, s) in sbuf.samples().iter().enumerate() {
                    planar[i % ch].push(*s);
                }
            }
            // A single corrupt packet shouldn't abort the whole decode.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        }
    }

    if planar.is_empty() || planar[0].is_empty() {
        return Err(DecodeError::Empty);
    }

    let frames = planar.iter().map(|c| c.len()).min().unwrap_or(0);
    // Guard against channels of unequal length (truncate to shortest).
    for c in planar.iter_mut() {
        c.truncate(frames);
    }

    Ok(DecodedAudio {
        sample_rate,
        channels: planar.len(),
        frames,
        planar,
    })
}
