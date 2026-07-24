//! Detect (and correct) timing offsets between the four part tracks of a song.
//!
//! Publishers assemble a learning track as *spoken title → pitch pipe → a gap →
//! the song*, and the four part files don't always get the song pasted in at the
//! same spot: the gap ends up holding a different amount of digital silence in
//! each file, so the parts play tens — sometimes hundreds — of milliseconds
//! apart even though their intros line up perfectly.
//!
//! The fix has to respect that shape. Sliding a whole file would drag its spoken
//! title and pitch pipe out of line with the others, so instead the correction is
//! absorbed **inside the gap**: delete the surplus silence (or add silence to the
//! files that start early). Intro and song both end up aligned.
//!
//! [`align_set`] measures the offsets and returns the exact edit to make per
//! file. The measurement runs on the *song*, never the intro — the intros
//! already agree, so they'd report an offset of zero.

/// Longest offset we look for. Publisher slips are milliseconds to a second.
const MAX_OFFSET_SECS: f64 = 2.0;
/// Envelope resolution for the coarse search.
const HOP_MS: f64 = 2.0;
/// Second pass: a finer envelope, searched either side of the coarse answer.
const FINE_HOP_MS: f64 = 0.25;
const FINE_RANGE_MS: f64 = 40.0;
/// Third pass: the waveform itself, either side of that.
const EXACT_RANGE_MS: f64 = 2.0;
const EXACT_WINDOW_SECS: f64 = 2.0;
/// Length of each measurement window.
const WINDOW_SECS: f64 = 12.0;
/// Windows must agree within this to call an offset constant through the song.
const CONSISTENT_MS: f64 = 10.0;
/// Correlation below this is noise, and the track is left alone.
const MIN_CONFIDENCE: f32 = 0.35;
/// Anything under this is silence for gap-finding purposes (~-72 dBFS).
const SILENCE: f32 = 0.00025;
/// Shortest run of silence we'll treat as a gap.
const MIN_GAP_MS: f64 = 120.0;
/// Silence left untouched on each side when editing inside a gap.
const GAP_MARGIN_MS: f64 = 15.0;

/// What to do to one track to line it up with the rest of the set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Correction {
    /// How much later this track's song starts than the earliest one, in frames.
    /// Informational — the edit below already accounts for it.
    pub offset_frames: i64,
    /// Frame position at which to apply [`Correction::delta_frames`].
    pub splice_at: usize,
    /// Frames to remove (negative) or insert as silence (positive) at that point.
    pub delta_frames: i64,
    /// Peak correlation behind the measurement, 0..1. Below ~0.25 is a guess.
    pub confidence: f32,
    /// Whether the offset held steady across the whole song. When false the
    /// track drifts (or the measurement failed) and one edit can't fix it.
    pub consistent: bool,
    /// How much the measurement varied between the windows, in frames — how
    /// steady that offset is through the song.
    pub spread_frames: i64,
}

impl Correction {
    /// A track that needs no change.
    pub fn none() -> Self {
        Correction {
            offset_frames: 0,
            splice_at: 0,
            delta_frames: 0,
            confidence: 1.0,
            consistent: true,
            spread_frames: 0,
        }
    }
}

/// Apply a correction's edit to a stem: remove frames (`delta < 0`) or insert
/// silence (`delta > 0`) at `at`.
pub fn splice(samples: &[f32], at: usize, delta: i64) -> Vec<f32> {
    let at = at.min(samples.len());
    match delta {
        0 => samples.to_vec(),
        d if d > 0 => {
            let mut out = Vec::with_capacity(samples.len() + d as usize);
            out.extend_from_slice(&samples[..at]);
            out.resize(at + d as usize, 0.0);
            out.extend_from_slice(&samples[at..]);
            out
        }
        d => {
            let cut = (-d) as usize;
            let mut out = Vec::with_capacity(samples.len().saturating_sub(cut));
            out.extend_from_slice(&samples[..at]);
            if at + cut < samples.len() {
                out.extend_from_slice(&samples[at + cut..]);
            }
            out
        }
    }
}

/// Runs of silence at least `min_frames` long, as `[start, end)` frame ranges.
pub fn silence_runs(samples: &[f32], min_frames: usize) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start = None;
    for (i, s) in samples.iter().enumerate() {
        if s.abs() <= SILENCE {
            start.get_or_insert(i);
        } else if let Some(from) = start.take() {
            if i - from >= min_frames {
                runs.push((from, i));
            }
        }
    }
    if let Some(from) = start {
        if samples.len() - from >= min_frames {
            runs.push((from, samples.len()));
        }
    }
    runs
}

/// Where the song itself starts: the end of the last gap in the lead-in (the one
/// after the spoken title and the pitch pipe).
///
/// Only the opening of the file is considered, so a fermata or a breath in the
/// middle of the song is never mistaken for the lead-in.
pub fn song_start(samples: &[f32], sample_rate: u32) -> usize {
    let horizon = (samples.len() / 5).min(45 * sample_rate as usize);
    let min_gap = ms_to_frames(MIN_GAP_MS, sample_rate);
    silence_runs(&samples[..horizon.min(samples.len())], min_gap)
        .last()
        .map(|&(_, end)| end)
        .unwrap_or(0)
}

fn ms_to_frames(ms: f64, sample_rate: u32) -> usize {
    (ms / 1000.0 * sample_rate as f64).round() as usize
}

/// Peak-envelope of `samples` at one point per `hop` frames, log-compressed so
/// quiet detail counts too.
fn envelope(samples: &[f32], hop: usize) -> Vec<f32> {
    samples
        .chunks(hop)
        .map(|c| {
            let peak = c.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            (1.0 + 50.0 * peak).ln()
        })
        .collect()
}

/// A peak has to reach this share of the strongest one to be preferred for
/// being closer to zero. Music repeats, so a whole-beat slip can correlate
/// almost as well as the true offset; publisher slips are small, so when two
/// peaks are near-equals the smaller offset is the better bet.
const RIVAL_PEAK: f32 = 0.88;

/// Best lag of `b` relative to `a` (positive = `b` is later), by normalized
/// cross-correlation over `-max_lag..=max_lag`.
///
/// `a` is the fixed window; `b` must extend `max_lag` beyond it on both sides,
/// i.e. `b[max_lag]` is the sample lined up with `a[0]` at lag zero.
fn best_lag(a: &[f32], b: &[f32], max_lag: usize) -> (isize, f32) {
    let n = a.len();
    if n == 0 || b.len() < n + 2 * max_lag {
        return (0, 0.0);
    }
    let mean_a = a.iter().map(|x| *x as f64).sum::<f64>() / n as f64;
    let energy_a: f64 = a.iter().map(|x| (*x as f64 - mean_a).powi(2)).sum();
    if energy_a <= 0.0 {
        return (0, 0.0);
    }

    // Prefix sums over b make each candidate window's mean and energy O(1).
    let mut sum = vec![0.0f64; b.len() + 1];
    let mut sum_sq = vec![0.0f64; b.len() + 1];
    for (i, x) in b.iter().enumerate() {
        sum[i + 1] = sum[i] + *x as f64;
        sum_sq[i + 1] = sum_sq[i] + (*x as f64) * (*x as f64);
    }

    let mut curve = vec![0.0f32; 2 * max_lag + 1];
    for (lag, slot) in curve.iter_mut().enumerate() {
        let window = &b[lag..lag + n];
        let s = sum[lag + n] - sum[lag];
        let ss = sum_sq[lag + n] - sum_sq[lag];
        let mean_b = s / n as f64;
        let energy_b = ss - n as f64 * mean_b * mean_b;
        if energy_b <= 0.0 {
            continue;
        }
        let dot: f64 = a
            .iter()
            .zip(window)
            .map(|(x, y)| *x as f64 * *y as f64)
            .sum();
        let covariance = dot - n as f64 * mean_a * mean_b;
        *slot = (covariance / (energy_a * energy_b).sqrt()) as f32;
    }

    let best_r = curve.iter().copied().fold(0.0f32, f32::max);
    if best_r <= 0.0 {
        return (0, 0.0);
    }
    // Walk outwards from zero and take the first peak that rivals the best one.
    let zero = max_lag;
    let mut order: Vec<usize> = (0..curve.len()).collect();
    order.sort_by_key(|i| i.abs_diff(zero));
    for i in order {
        let rising_edge = i == 0 || curve[i - 1] <= curve[i];
        let falling_edge = i + 1 == curve.len() || curve[i + 1] <= curve[i];
        if curve[i] >= best_r * RIVAL_PEAK && rising_edge && falling_edge {
            return (i as isize - zero as isize, curve[i]);
        }
    }
    let i = curve
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).expect("finite"))
        .map(|(i, _)| i)
        .unwrap_or(zero);
    (i as isize - zero as isize, curve[i])
}

/// Measure how much later `candidate` plays than `reference`.
///
/// Both are mono mixes of a whole file (sum the channels — the part-predominant
/// side and the other-three side together carry the same performance).
pub fn measure_offset(reference: &[f32], candidate: &[f32], sample_rate: u32) -> Correction {
    let hop = ms_to_frames(HOP_MS, sample_rate).max(1);
    let max_lag = ms_to_frames(MAX_OFFSET_SECS * 1000.0, sample_rate);
    let window = ms_to_frames(WINDOW_SECS * 1000.0, sample_rate);

    let start = song_start(reference, sample_rate).max(song_start(candidate, sample_rate));
    let usable = reference.len().min(candidate.len());
    if usable <= start + window + 2 * max_lag {
        return Correction {
            confidence: 0.0,
            consistent: false,
            ..Correction::none()
        };
    }

    // Three windows spread through the song; a real offset is the same in all of
    // them, a mis-measurement or a drifting track is not.
    let span = usable - start - window - 2 * max_lag;
    let spots: Vec<usize> = [0.15, 0.45, 0.75]
        .iter()
        .map(|f| start + max_lag + (span as f64 * f) as usize)
        .collect();

    let fine_hop = ms_to_frames(FINE_HOP_MS, sample_rate).max(1);
    let fine_range = ms_to_frames(FINE_RANGE_MS, sample_rate);
    let exact_range = ms_to_frames(EXACT_RANGE_MS, sample_rate);
    let exact_window = ms_to_frames(EXACT_WINDOW_SECS * 1000.0, sample_rate).min(window);

    let (mut lags, mut confidence) = (Vec::new(), 1.0f32);
    for &at in &spots {
        // Three passes, each narrowing the search: a coarse envelope over the
        // whole ±2 s, a fine envelope, then the waveform itself for sample
        // accuracy. Anything less and the answer lands a few milliseconds out —
        // enough to still hear.
        let a_env = envelope(&reference[at..at + window], hop);
        let b_env = envelope(&candidate[at - max_lag..at + window + max_lag], hop);
        let (coarse_hops, coarse_r) = best_lag(&a_env, &b_env, max_lag / hop);
        let mut lag = coarse_hops * hop as isize;
        let mut best_r = coarse_r;

        for (pass_hop, range, span) in [
            (fine_hop, fine_range, window),
            (1usize, exact_range, exact_window),
        ] {
            let from = at as isize + lag - range as isize;
            if from < 0 || from as usize + span + 2 * range > candidate.len() {
                continue;
            }
            let (a_slice, b_slice) = (
                &reference[at..at + span],
                &candidate[from as usize..from as usize + span + 2 * range],
            );
            // The last pass correlates the waveform itself; earlier ones use the
            // envelope, which is what survives two different MP3 encodes.
            let (a_pass, b_pass) = if pass_hop == 1 {
                (a_slice.to_vec(), b_slice.to_vec())
            } else {
                (envelope(a_slice, pass_hop), envelope(b_slice, pass_hop))
            };
            let (shift, r) = best_lag(&a_pass, &b_pass, range / pass_hop);
            lag += shift * pass_hop as isize;
            best_r = best_r.max(r);
        }

        lags.push(lag);
        confidence = confidence.min(best_r);
    }

    lags.sort_unstable();
    let median = lags[lags.len() / 2];
    let spread = (lags[lags.len() - 1] - lags[0]).unsigned_abs();
    Correction {
        offset_frames: median as i64,
        confidence,
        consistent: spread <= ms_to_frames(CONSISTENT_MS, sample_rate) && confidence > 0.25,
        spread_frames: spread as i64,
        ..Correction::none()
    }
}

/// Pick where to absorb `frames` of edit in a track: inside the gap before the
/// song if it's roomy enough, else in the leading silence, else at the very
/// start.
fn absorb_point(mono: &[f32], sample_rate: u32, remove: usize) -> Option<usize> {
    if remove == 0 {
        return Some(0);
    }
    let margin = ms_to_frames(GAP_MARGIN_MS, sample_rate);
    let need = remove + 2 * margin;
    let horizon = (mono.len() / 5).min(45 * sample_rate as usize);
    silence_runs(&mono[..horizon.min(mono.len())], need)
        .last()
        .map(|&(from, _)| from + margin)
}

/// Measure a whole set and return the edit each track needs.
///
/// `tracks` are the mono mixes of the files, in a stable order (the result is in
/// the same order). Everything is aligned to whichever track starts its song
/// earliest, by cutting the surplus silence out of the others' gaps — unless one
/// of them has no gap to cut, in which case the set is aligned the other way, by
/// padding the early tracks instead. Both keep the spoken intro in line.
pub fn align_set(tracks: &[&[f32]], sample_rate: u32) -> Vec<Correction> {
    if tracks.len() < 2 {
        return tracks.iter().map(|_| Correction::none()).collect();
    }

    let mut corrections: Vec<Correction> = tracks
        .iter()
        .map(|t| measure_offset(tracks[0], t, sample_rate))
        .collect();

    // A wandering measurement still points the right way — the median offset is
    // the best single answer, and the caller gets `spread_frames` to judge it
    // by. Only a measurement we don't believe at all is dropped.
    let usable: Vec<i64> = corrections
        .iter()
        .map(|c| {
            if c.confidence > MIN_CONFIDENCE {
                c.offset_frames
            } else {
                0
            }
        })
        .collect();
    let earliest = *usable.iter().min().unwrap_or(&0);
    let latest = *usable.iter().max().unwrap_or(&0);

    // Preferred: delete each track's surplus silence, restoring the intended
    // timing. Only possible if every track has a gap big enough to lose.
    let can_trim = tracks.iter().zip(&usable).all(|(t, offset)| {
        absorb_point(t, sample_rate, (offset - earliest).max(0) as usize).is_some()
    });

    for (i, correction) in corrections.iter_mut().enumerate() {
        if correction.confidence <= MIN_CONFIDENCE {
            correction.delta_frames = 0;
            correction.splice_at = 0;
            correction.offset_frames = 0;
            continue;
        }
        let (delta, remove) = if can_trim {
            (-(usable[i] - earliest), (usable[i] - earliest) as usize)
        } else {
            (latest - usable[i], 0)
        };
        correction.delta_frames = delta;
        correction.splice_at = absorb_point(tracks[i], sample_rate, remove).unwrap_or(0);
        correction.offset_frames = usable[i] - earliest;
    }
    corrections
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 44_100;

    fn tone(out: &mut Vec<f32>, hz: f64, frames: usize, amp: f64) {
        for i in 0..frames {
            let t = i as f64 / SR as f64;
            out.push((amp * (2.0 * std::f64::consts::PI * hz * t).sin()) as f32);
        }
    }

    /// A little "learning track", shaped like the real thing: spoken title, pitch
    /// pipe, a gap of `gap_frames`, then the song.
    ///
    /// `voice` picks which part is predominant. As with real part tracks it is
    /// the *same performance* in every file — same notes, same rhythm — with one
    /// voice pushed forward, which is exactly the material the offset has to be
    /// measured from.
    fn learning_track(gap_frames: usize, voice: usize) -> Vec<f32> {
        let mut out = vec![0.0f32; SR as usize / 4]; // leading silence
        tone(&mut out, 180.0, SR as usize, 0.2); // "spoken" title
        out.resize(out.len() + SR as usize / 10, 0.0);
        tone(&mut out, 233.0, SR as usize / 2, 0.25); // pitch pipe
        out.resize(out.len() + gap_frames, 0.0); // the gap
        out.extend(song(voice));
        out
    }

    /// The song on its own — no spoken title, no pitch pipe, no gap to edit in.
    fn song(voice: usize) -> Vec<f32> {
        let mut out = Vec::new();
        // An uneven phrase (so it isn't a metronomic loop) sung by a quartet,
        // with one voice pushed forward.
        let chords: [[f64; 4]; 6] = [
            [466.0, 293.0, 196.0, 116.0],
            [440.0, 277.0, 220.0, 146.0],
            [392.0, 311.0, 233.0, 130.0],
            [415.0, 261.0, 174.0, 155.0],
            [466.0, 349.0, 196.0, 116.0],
            [440.0, 293.0, 233.0, 174.0],
        ];
        let lengths = [0.6, 0.35, 0.5, 0.8, 0.4, 0.7];
        for bar in 0..30 {
            let chord = chords[bar % chords.len()];
            let frames = (lengths[(bar * 5) % lengths.len()] * SR as f64) as usize;
            let start = out.len();
            out.resize(start + frames, 0.0);
            for (v, hz) in chord.iter().enumerate() {
                let amp = if v == voice { 0.30 } else { 0.12 };
                let mut voiced = Vec::with_capacity(frames);
                tone(&mut voiced, *hz, frames, amp);
                for (o, s) in out[start..].iter_mut().zip(&voiced) {
                    *o += *s;
                }
            }
            out.resize(out.len() + SR as usize / 20, 0.0);
        }
        out
    }

    #[test]
    fn splices_in_and_out() {
        let s = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(splice(&s, 2, -1), vec![1.0, 2.0, 4.0]);
        assert_eq!(splice(&s, 2, 2), vec![1.0, 2.0, 0.0, 0.0, 3.0, 4.0]);
        assert_eq!(splice(&s, 2, 0), s);
    }

    #[test]
    fn finds_the_gap_before_the_song() {
        let track = learning_track(SR as usize / 2, 0);
        let start = song_start(&track, SR);
        let expected =
            SR as usize / 4 + SR as usize + SR as usize / 10 + SR as usize / 2 + SR as usize / 2;
        assert!(
            (start as isize - expected as isize).abs() < 100,
            "song starts at {start}, expected about {expected}"
        );
    }

    #[test]
    fn measures_a_known_offset() {
        let reference = learning_track(SR as usize / 2, 0);
        for shift in [441usize, 6688, 13507] {
            let candidate = splice(&reference, SR as usize / 4, shift as i64);
            let c = measure_offset(&reference, &candidate, SR);
            assert!(
                c.consistent,
                "offset of {shift} was not measured consistently"
            );
            assert!(
                (c.offset_frames - shift as i64).abs() <= 2,
                "measured {} frames, expected {shift}",
                c.offset_frames
            );
        }
    }

    #[test]
    fn aligned_tracks_measure_as_aligned() {
        let a = learning_track(SR as usize / 2, 0);
        let b = learning_track(SR as usize / 2, 2);
        let c = measure_offset(&a, &b, SR);
        assert_eq!(c.offset_frames, 0);
    }

    #[test]
    fn align_set_trims_the_surplus_silence_from_the_gap() {
        // Three "files" whose songs start 0, 6688 and 13507 frames apart because
        // their gaps hold different amounts of silence — the publisher bug.
        let base = SR as usize / 2;
        let tracks: Vec<Vec<f32>> = [0usize, 6688, 13507]
            .iter()
            .enumerate()
            .map(|(voice, extra)| learning_track(base + extra, voice))
            .collect();
        let refs: Vec<&[f32]> = tracks.iter().map(|t| t.as_slice()).collect();
        let corrections = align_set(&refs, SR);

        for (i, expected) in [0i64, 6688, 13507].iter().enumerate() {
            let c = corrections[i];
            assert!(c.consistent, "track {i} inconsistent");
            assert!(
                (c.offset_frames - expected).abs() <= 2,
                "track {i}: offset {} != {expected}",
                c.offset_frames
            );
            assert_eq!(
                c.delta_frames, -c.offset_frames,
                "track {i} should be trimmed"
            );
            // The edit must land in silence, or it would cut the music.
            let cut = (-c.delta_frames) as usize;
            let region = &tracks[i][c.splice_at..c.splice_at + cut];
            assert!(
                region.iter().all(|s| s.abs() <= SILENCE),
                "track {i} would cut audio, not silence"
            );
        }

        // And after the edit, everything really is aligned.
        let fixed: Vec<Vec<f32>> = tracks
            .iter()
            .zip(&corrections)
            .map(|(t, c)| splice(t, c.splice_at, c.delta_frames))
            .collect();
        for i in 1..fixed.len() {
            let c = measure_offset(&fixed[0], &fixed[i], SR);
            assert!(
                c.offset_frames.abs() <= 2,
                "track {i} still off by {} frames",
                c.offset_frames
            );
        }
    }

    #[test]
    fn pads_when_there_is_no_silence_to_trim() {
        // Songs that start immediately, with no lead-in gap to edit inside.
        let a = song(0);
        let b = splice(&song(2), 0, 2000); // b's song starts 2000 frames later
        let refs: Vec<&[f32]> = vec![&a, &b];
        let corrections = align_set(&refs, SR);

        assert!(corrections[1].consistent);
        assert!(
            (corrections[1].offset_frames - 2000).abs() <= 2,
            "measured {}",
            corrections[1].offset_frames
        );
        // Track 0 gets padded instead of track 1 being cut.
        assert!(corrections[0].delta_frames >= 1998 && corrections[0].delta_frames <= 2002);
        assert_eq!(corrections[1].delta_frames, 0);
    }
}
