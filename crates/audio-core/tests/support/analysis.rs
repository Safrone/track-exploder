//! Measurement helpers for asserting things about real decoded audio.

#![allow(dead_code)] // Each consumer uses a subset.

use std::f64::consts::PI;

/// Largest absolute sample in `samples`.
pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

/// Root-mean-square level of `samples`.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples
        .iter()
        .map(|s| (*s as f64) * (*s as f64))
        .sum::<f64>()
        / samples.len() as f64)
        .sqrt() as f32
}

/// Largest absolute difference between two signals (compared over the shorter one).
pub fn max_abs_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .fold(0.0f32, |m, (x, y)| m.max((x - y).abs()))
}

/// Normalized cross-correlation at zero lag: 1.0 = identical shape, 0.0 = unrelated.
/// Used where quantization is lossy (MP3) and sample-exact comparison is too strict.
pub fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let (mut dot, mut ea, mut eb) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let (x, y) = (a[i] as f64, b[i] as f64);
        dot += x * y;
        ea += x * x;
        eb += y * y;
    }
    if ea == 0.0 || eb == 0.0 {
        return 0.0;
    }
    (dot / (ea.sqrt() * eb.sqrt())) as f32
}

/// Energy at `hz` in `samples` (Goertzel, Hann-windowed so that a neighbouring
/// semitone doesn't just pick up spectral leakage from the note being measured).
/// Magnitudes are only meaningful relative to each other, which is all the tests
/// need.
pub fn goertzel(samples: &[f32], sample_rate: u32, hz: f64) -> f64 {
    let n = samples.len();
    if n == 0 {
        return 0.0;
    }
    let k = hz / sample_rate as f64;
    let coeff = 2.0 * (2.0 * PI * k).cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for (i, &sample) in samples.iter().enumerate() {
        let window = 0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos();
        let s0 = sample as f64 * window + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    (s1 * s1 + s2 * s2 - coeff * s1 * s2).sqrt() / n as f64
}

/// Frequency of the note `semitones` away from `hz`.
pub fn semitones_from(hz: f64, semitones: f64) -> f64 {
    hz * 2f64.powf(semitones / 12.0)
}

/// A slice of `samples` clear of note attacks and releases, so measurements see
/// the steady part of the note.
pub fn steady(samples: &[f32], start: usize, end: usize) -> &[f32] {
    let len = end.saturating_sub(start);
    let margin = len / 5;
    let (a, b) = (start + margin, end.saturating_sub(margin));
    if b <= a || b > samples.len() {
        return &[];
    }
    &samples[a..b]
}
