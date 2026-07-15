//! Pitch-preserving time-stretch (Signalsmith Stretch, offline).
//!
//! Used for export so tempo changes are reliable and deterministic — the
//! browser's OfflineAudioContext + AudioWorklet path deadlocks with the JS
//! build, so all export-time stretching happens here.

use signalsmith_stretch::Stretch;

/// Time-stretch interleaved `f32` audio by `tempo` (playback speed: `< 1` is
/// slower/longer), preserving pitch. Returns interleaved `f32`.
///
/// A no-op (returns a copy) when `tempo` is ~1 or the input is empty.
pub fn time_stretch(input: &[f32], channels: u16, sample_rate: u32, tempo: f32) -> Vec<f32> {
    let ch = channels.max(1) as usize;
    if input.is_empty() || (tempo - 1.0).abs() < 1e-4 {
        return input.to_vec();
    }

    let in_frames = input.len() / ch;
    let out_frames = (((in_frames as f64) / (tempo as f64)).round() as usize).max(1);

    let mut stretch = Stretch::preset_default(channels.max(1) as u32, sample_rate);
    let mut output = vec![0.0f32; out_frames * ch];
    stretch.process(input, &mut output);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn passthrough_at_unity() {
        let input = vec![0.1, -0.2, 0.3, -0.4];
        assert_eq!(time_stretch(&input, 2, 44_100, 1.0), input);
    }

    #[test]
    fn half_speed_roughly_doubles_length() {
        let sr = 44_100u32;
        let n = sr as usize; // 1 second, mono
        let input: Vec<f32> = (0..n)
            .map(|i| (2.0 * PI * 440.0 * (i as f32) / sr as f32).sin() * 0.5)
            .collect();

        let out = time_stretch(&input, 1, sr, 0.5);

        // ~2x length (allow 100ms slack for algorithm latency/tail).
        let expected = (n as f64 / 0.5) as usize;
        let slack = (sr as f64 * 0.1) as usize;
        assert!(
            (out.len() as isize - expected as isize).unsigned_abs() < slack,
            "len {} not near {expected}",
            out.len()
        );

        // Output carries energy (not all silence).
        let rms = (out.iter().map(|s| s * s).sum::<f32>() / out.len() as f32).sqrt();
        assert!(rms > 0.05, "output too quiet: rms={rms}");
    }
}
