//! End-to-end tests against **real media files**.
//!
//! `support::fixtures` renders a barbershop arrangement — circle-of-fifths
//! sevenths, justly tuned, with swipes, an echo, trios, a tag, and rests — into
//! encoded WAV/FLAC/MP3 part-predominant learning tracks on disk. These tests
//! then put them through the same pipeline the app uses (decode, pull out the
//! isolated voice, mix, time-stretch, encode, read tags), and check that the
//! music itself is in the style the app exists to serve.
//!
//! See `support/score.rs` for the arrangement and the style rules behind it.

mod support;

use audio_core::{
    decode_file, encode_interleaved, extract_channel, read_tags, time_stretch, BitDepth, Channel,
    ExportFormat, Tags,
};
#[cfg(feature = "mp3")]
use support::analysis::correlation;
use support::analysis::{goertzel, max_abs_diff, peak, rms, semitones_from, steady};
use support::fixtures;
use support::score::{self, Part, PARTS, SAMPLE_RATE};

/// A 24-bit round trip is accurate to ~1.2e-7 per sample; allow generous slack
/// for the handful of additions the mixing tests do.
const TOL_24: f32 = 1e-5;
/// 16-bit quantization is ~3e-5 per sample.
const TOL_16: f32 = 2e-4;

/// Decode a file and return one channel, checking the container's shape first.
fn channel_of(path: &std::path::Path, channel: Channel) -> Vec<f32> {
    let decoded = decode_file(path).unwrap_or_else(|e| panic!("decode {}: {e}", path.display()));
    assert_eq!(
        decoded.sample_rate,
        SAMPLE_RATE,
        "unexpected sample rate in {}",
        path.display()
    );
    extract_channel(&decoded, channel)
}

fn slice(samples: &[f32], start: usize, end: usize) -> &[f32] {
    &samples[start.min(samples.len())..end.min(samples.len())]
}

#[cfg(feature = "mp3")]
fn db(level: f32) -> f32 {
    20.0 * level.max(1e-12).log10()
}

// --- the files themselves ---------------------------------------------------

#[test]
fn fixture_set_is_real_encoded_media() {
    let set = fixtures::ensure();

    for part in PARTS {
        for path in [set.part_left(part), set.part_right(part)] {
            let bytes = std::fs::read(path).expect("read wav fixture");
            assert_eq!(&bytes[0..4], b"RIFF", "{} is not a WAV", path.display());
            assert_eq!(&bytes[8..12], b"WAVE", "{} is not a WAV", path.display());
            assert!(bytes.len() > 100_000, "{} looks empty", path.display());
        }

        let flac = std::fs::read(set.flac(part)).expect("read flac fixture");
        assert_eq!(
            &flac[0..4],
            b"fLaC",
            "{} is not FLAC",
            set.flac(part).display()
        );

        // Vendor-style names are what the app's part detection keys off.
        let name = set.part_left(part).file_name().unwrap().to_string_lossy();
        assert!(
            name.contains(part.token()) && name.starts_with(score::SONG_BASE),
            "unexpected fixture name {name}"
        );
    }

    assert!(set.root.join("fixtures.json").is_file(), "missing manifest");
}

#[test]
fn part_left_tracks_decode_to_the_expected_shape() {
    let set = fixtures::ensure();
    let expected_frames = score::total_frames();

    for part in PARTS {
        let path = set.part_left(part);
        let decoded = decode_file(path).expect("decode part-left track");
        assert_eq!(decoded.channels, 2, "{} is not stereo", path.display());
        assert_eq!(decoded.sample_rate, SAMPLE_RATE);
        // All four files are the same length — the app assumes the set is aligned.
        assert_eq!(
            decoded.frames,
            expected_frames,
            "{} has {} frames, expected {expected_frames}",
            path.display(),
            decoded.frames
        );
    }
}

// --- extraction -------------------------------------------------------------

#[test]
fn left_channel_is_the_isolated_voice() {
    let set = fixtures::ensure();

    for part in PARTS {
        let extracted = channel_of(set.part_left(part), Channel::Left);
        let expected = score::render_part(part);
        assert_eq!(extracted.len(), expected.len());
        let err = max_abs_diff(&extracted, &expected);
        assert!(
            err < TOL_24,
            "{} left channel diverged from the {} part: max_err={err}",
            set.part_left(part).display(),
            part.slug()
        );
    }
}

#[test]
fn right_channel_carries_the_other_three_voices() {
    let set = fixtures::ensure();
    let stems = score::render_all();

    for part in PARTS {
        let right = channel_of(set.part_left(part), Channel::Right);
        let expected = score::mix_of_others(&stems, part);
        let err = max_abs_diff(&right, &expected);
        assert!(
            err < TOL_24,
            "{} right channel is not the other three parts: max_err={err}",
            set.part_left(part).display()
        );

        // Sanity: the two channels really are different material.
        let left = channel_of(set.part_left(part), Channel::Left);
        assert!(
            max_abs_diff(&left, &right) > 0.05,
            "{} looks like the same signal on both channels",
            set.part_left(part).display()
        );
    }
}

#[test]
fn part_right_tracks_need_the_other_channel() {
    let set = fixtures::ensure();
    let stems = score::render_all();

    for part in PARTS {
        let path = set.part_right(part);
        let isolated = channel_of(path, Channel::Right);
        let expected = score::render_part(part);
        assert!(
            max_abs_diff(&isolated, &expected) < TOL_16,
            "{}: right channel is not the isolated {} part",
            path.display(),
            part.slug()
        );

        // Picking the wrong channel gives the other three voices, not the part —
        // this is what the per-track channel selector exists to fix.
        let wrong = channel_of(path, Channel::Left);
        assert!(
            max_abs_diff(&wrong, &expected) > 0.05,
            "{}: left channel unexpectedly matched the isolated part",
            path.display()
        );
        assert!(
            max_abs_diff(&wrong, &score::mix_of_others(&stems, part)) < TOL_16,
            "{}: left channel is not the other three parts",
            path.display()
        );
    }
}

// --- rests ------------------------------------------------------------------

#[test]
fn each_voice_is_silent_where_the_score_rests() {
    let set = fixtures::ensure();

    for part in PARTS {
        let isolated = channel_of(set.part_left(part), Channel::Left);

        for (start, end) in score::rests(part) {
            let quiet = peak(slice(&isolated, start, end));
            assert_eq!(
                quiet,
                0.0,
                "{} is not silent during its rest at frames {start}..{end}",
                part.slug()
            );
        }

        for note in score::sounding(part) {
            let level = rms(steady(&isolated, note.start, note.end));
            assert!(
                level > 0.02,
                "{} is too quiet in event {} ({}): rms={level}",
                part.slug(),
                note.event,
                score::SCORE[note.event].label
            );
        }
    }
}

#[test]
fn the_ensemble_breath_is_silent_in_both_channels() {
    let set = fixtures::ensure();
    let breaths = score::full_rests();
    assert!(!breaths.is_empty(), "the score should contain a full rest");

    for part in PARTS {
        for channel in [Channel::Left, Channel::Right] {
            let samples = channel_of(set.part_left(part), channel);
            for &(start, end) in &breaths {
                assert_eq!(
                    peak(slice(&samples, start, end)),
                    0.0,
                    "{} ({channel:?}) is not silent during the ensemble rest at {start}..{end}",
                    part.slug()
                );
            }
        }
    }
}

#[test]
fn a_resting_voice_does_not_leak_into_its_own_channel() {
    let set = fixtures::ensure();

    // Every voice drops out somewhere — the echo, a trio bar, the sustain under
    // the melody's breath. Where one does, its isolated channel must be silent
    // while the mix side keeps singing.
    for part in PARTS {
        let event = score::first_tacet(part);
        let (start, end) = score::event_spans()[event];
        let isolated = channel_of(set.part_left(part), Channel::Left);
        let others = channel_of(set.part_left(part), Channel::Right);
        assert_eq!(
            peak(slice(&isolated, start, end)),
            0.0,
            "{} should rest in '{}'",
            part.slug(),
            score::SCORE[event].label
        );
        assert!(
            rms(steady(&others, start, end)) > 0.02,
            "the rest of the quartet should still be singing in '{}'",
            score::SCORE[event].label
        );
    }
}

// --- musical content --------------------------------------------------------

#[test]
fn sung_pitches_match_the_score() {
    let set = fixtures::ensure();

    for part in PARTS {
        let isolated = channel_of(set.part_left(part), Channel::Left);

        for note in score::sounding(part) {
            let window = steady(&isolated, note.start, note.end);
            let at_pitch = goertzel(window, SAMPLE_RATE, note.hz);
            for offset in [-2.0, -1.0, 1.0, 2.0] {
                let neighbour = goertzel(window, SAMPLE_RATE, semitones_from(note.hz, offset));
                assert!(
                    at_pitch > neighbour * 4.0,
                    "{} in '{}' ({:.1} Hz): {offset:+} semitones is not clearly weaker \
                     ({at_pitch:.5} vs {neighbour:.5})",
                    part.slug(),
                    score::SCORE[note.event].label,
                    note.hz
                );
            }
        }
    }
}

// --- barbershop style -------------------------------------------------------
//
// The fixture is only useful if it is actually the kind of music the app is for.
// These check it against the style rules in the Barbershop Harmony Society's
// Music Educator Guide ("Characteristics of the Barbershop Style").

#[test]
fn the_quartet_is_voiced_tenor_lead_bari_bass() {
    for (event, chord) in score::SCORE.iter().enumerate() {
        let sung: Vec<(Part, f64)> = PARTS
            .iter()
            .filter_map(|&p| score::hz(event, p).map(|hz| (p, hz)))
            .collect();

        // Voices stay in score order, top to bottom: the melody is the second
        // voice down, with the tenor harmonizing above it.
        for pair in sung.windows(2) {
            assert!(
                pair[0].1 > pair[1].1,
                "'{}': {} ({:.1} Hz) is not above {} ({:.1} Hz)",
                chord.label,
                pair[0].0.slug(),
                pair[0].1,
                pair[1].0.slug(),
                pair[1].1
            );
        }

        // And everyone stays inside the range the guide gives for their part.
        for (part, hz) in sung {
            let (low, high) = part.range();
            let (low, high) = (
                score::equal_tempered_hz(low) * 0.99,
                score::equal_tempered_hz(high) * 1.01,
            );
            assert!(
                hz >= low && hz <= high,
                "'{}': {} sings {hz:.1} Hz, outside the {}–{} range",
                chord.label,
                part.slug(),
                part.range().0,
                part.range().1
            );
        }
    }
}

#[test]
fn no_chord_contains_a_minor_second() {
    // "Chords containing a minor second interval are not generally used."
    for (event, chord) in score::SCORE.iter().enumerate() {
        let sung: Vec<(Part, f64)> = PARTS
            .iter()
            .filter_map(|&p| score::hz(event, p).map(|hz| (p, hz)))
            .collect();

        for (i, &(a, a_hz)) in sung.iter().enumerate() {
            for &(b, b_hz) in &sung[i + 1..] {
                // Reduce the interval into one octave and measure it in cents.
                let mut cents = 1200.0 * (a_hz / b_hz).log2();
                while cents >= 1200.0 {
                    cents -= 1200.0;
                }
                let from_semitone = (cents - 100.0).abs().min((cents - 1100.0).abs());
                assert!(
                    from_semitone > 40.0,
                    "'{}': {} and {} are a minor second apart ({cents:.0} cents)",
                    chord.label,
                    a.slug(),
                    b.slug()
                );
            }
        }
    }
}

#[test]
fn sevenths_are_barbershop_sevenths_that_resolve_down_a_fifth() {
    let chords: Vec<(usize, &score::Event)> = score::SCORE
        .iter()
        .enumerate()
        .filter(|(_, e)| e.quality != score::Quality::Rest)
        .collect();

    let sevenths = chords
        .iter()
        .filter(|(_, e)| e.quality == score::Quality::Dominant7)
        .count();
    assert!(
        sevenths * 3 >= chords.len(),
        "only {sevenths} of {} chords are barbershop sevenths",
        chords.len()
    );

    // Every dominant seventh resolves around the circle — root down a fifth.
    for window in chords.windows(2) {
        let (_, chord) = window[0];
        let (_, next) = window[1];
        if chord.quality != score::Quality::Dominant7 {
            continue;
        }
        let root = score::midi(chord.root.unwrap()).rem_euclid(12);
        let next_root = score::midi(next.root.unwrap()).rem_euclid(12);
        assert_eq!(
            (root + 5).rem_euclid(12),
            next_root,
            "{} does not resolve down a fifth (it goes to {})",
            chord.label,
            next.label
        );
    }
}

#[test]
fn dominant_sevenths_are_tuned_four_five_six_seven() {
    // The ring: the chord tones of a barbershop seventh are 4:5:6:7, so their
    // overtones coincide instead of beating.
    for (event, chord) in score::SCORE.iter().enumerate() {
        if chord.quality != score::Quality::Dominant7 {
            continue;
        }
        for part in PARTS {
            let Some(hz) = score::hz(event, part) else {
                continue;
            };
            let root = score::hz(event, Part::Bass).expect("the bass sings the root");
            // Fold the voice into the root's octave and match it to 4:5:6:7.
            let mut ratio = hz / root;
            while ratio >= 2.0 {
                ratio /= 2.0;
            }
            let expected = [4.0 / 4.0, 5.0 / 4.0, 6.0 / 4.0, 7.0 / 4.0];
            let closest = expected
                .iter()
                .copied()
                .min_by(|a, b| {
                    (a - ratio)
                        .abs()
                        .partial_cmp(&(b - ratio).abs())
                        .expect("finite")
                })
                .unwrap();
            let cents = 1200.0 * (ratio / closest).log2();
            assert!(
                cents.abs() < 1.0,
                "'{}': the {} is {cents:.1} cents off the {closest:.2} of a 4:5:6:7 chord",
                chord.label,
                part.slug()
            );
        }
    }
}

#[test]
fn the_recorded_sevenths_are_justly_tuned_not_tempered() {
    // Measured off the actual audio: where a voice sings the seventh of a
    // barbershop seventh, the recorded pitch matches the just 7/4 (31 cents
    // below the piano's minor seventh), not the tempered one.
    let set = fixtures::ensure();
    let mut checked = 0;

    for part in PARTS {
        let isolated = channel_of(set.part_left(part), Channel::Left);
        for note in score::sounding(part) {
            let chord = &score::SCORE[note.event];
            if chord.quality != score::Quality::Dominant7 {
                continue;
            }
            let written = chord.notes[part.index()].unwrap();
            let tempered = score::equal_tempered_hz(written);
            // Only the seventh moves far enough to resolve in a 1-second window.
            if (1200.0 * (note.hz / tempered).log2()).abs() < 25.0 {
                continue;
            }

            let window = steady(&isolated, note.start, note.end);
            let just_level = goertzel(window, SAMPLE_RATE, note.hz);
            let tempered_level = goertzel(window, SAMPLE_RATE, tempered);
            assert!(
                just_level > tempered_level * 3.0,
                "{} in '{}': recorded pitch looks tempered ({written} = {tempered:.1} Hz) \
                 rather than justly tuned ({:.1} Hz)",
                part.slug(),
                chord.label,
                note.hz
            );
            checked += 1;
        }
    }

    assert!(
        checked >= 4,
        "expected several sung sevenths, checked {checked}"
    );
}

#[test]
fn the_quartet_is_balanced_with_headroom() {
    let set = fixtures::ensure();

    for part in PARTS {
        let isolated = peak(&channel_of(set.part_left(part), Channel::Left));
        let others = peak(&channel_of(set.part_left(part), Channel::Right));
        // A soloed part has to be clearly audible…
        assert!(
            isolated > 0.1,
            "the {} track is too quiet on its own (peak {isolated:.3})",
            part.slug()
        );
        // …and, like a real learning track, nothing is slammed against 0 dBFS.
        assert!(
            isolated < 0.95 && others < 0.95,
            "{}: no headroom left (part {isolated:.3}, mix {others:.3})",
            part.slug()
        );
    }

    // The pyramid: the bass anchors the chord and the tenor floats lightest on
    // top, which is how the guide asks a quartet to balance.
    let level = |part| rms(&channel_of(set.part_left(part), Channel::Left));
    let (tenor, lead, bari, bass) = (
        level(Part::Tenor),
        level(Part::Lead),
        level(Part::Baritone),
        level(Part::Bass),
    );
    assert!(
        tenor < bari && bari < bass && lead > bari,
        "unbalanced quartet: tenor {tenor:.3}, lead {lead:.3}, bari {bari:.3}, bass {bass:.3}"
    );
}

#[test]
fn the_quartet_sings_in_homorhythm() {
    // "The same words at the same time": within an event every sounding voice
    // covers the identical span, and a rest is a rest for everybody in it.
    for (event, (start, end)) in score::event_spans().into_iter().enumerate() {
        let chord = &score::SCORE[event];
        let singing = PARTS.iter().filter(|&&p| score::hz(event, p).is_some());
        for part in singing {
            let note = score::sounding(*part)
                .into_iter()
                .find(|n| n.event == event)
                .expect("the voice sings here");
            assert_eq!(
                (note.start, note.end),
                (start, end),
                "'{}': {} is not moving with the quartet",
                chord.label,
                part.slug()
            );
        }
    }
}

// --- the whole pipeline -----------------------------------------------------

#[test]
fn extracted_voices_add_back_up_to_the_published_mix() {
    let set = fixtures::ensure();

    // Extract tenor, bari and bass from their own files and sum them: that is
    // exactly the "everything but the lead" side of the lead's own track.
    let mut sum = vec![0.0f32; score::total_frames()];
    for part in [Part::Tenor, Part::Baritone, Part::Bass] {
        for (out, s) in sum
            .iter_mut()
            .zip(channel_of(set.part_left(part), Channel::Left))
        {
            *out += s;
        }
    }

    let published = channel_of(set.part_left(Part::Lead), Channel::Right);
    let err = max_abs_diff(&sum, &published);
    assert!(
        err < TOL_24,
        "re-mixing the extracted voices didn't reproduce the lead track's mix side: max_err={err}"
    );
}

#[test]
fn exporting_a_mix_without_the_lead_round_trips() {
    let set = fixtures::ensure();

    // The classic "sing along with your part out" mix: lead muted, the rest at
    // unity, panned across the stereo field.
    let pans = [-0.5f32, 0.0, 0.25, 0.0];
    let mut interleaved = vec![0.0f32; score::total_frames() * 2];
    let mut expected_peak = 0.0f32;

    for part in PARTS {
        if part == Part::Lead {
            continue;
        }
        let voice = channel_of(set.part_left(part), Channel::Left);
        // Constant-power pan, as a mono source through a stereo panner.
        let angle = (pans[part.index()] + 1.0) * std::f32::consts::FRAC_PI_4;
        let (gl, gr) = (angle.cos(), angle.sin());
        for (i, s) in voice.iter().enumerate() {
            interleaved[i * 2] += s * gl;
            interleaved[i * 2 + 1] += s * gr;
        }
    }
    for s in &interleaved {
        expected_peak = expected_peak.max(s.abs());
    }
    assert!(expected_peak < 1.0, "test mix clips: peak={expected_peak}");

    let bytes = encode_interleaved(
        &interleaved,
        2,
        SAMPLE_RATE,
        ExportFormat::Wav,
        BitDepth::TwentyFour,
    )
    .expect("encode the exported mix");

    let out = std::env::temp_dir().join(format!("te_export_{}.wav", std::process::id()));
    std::fs::write(&out, &bytes).expect("write exported mix");
    let decoded = decode_file(&out).expect("decode exported mix");
    std::fs::remove_file(&out).ok();

    assert_eq!(decoded.channels, 2);
    assert_eq!(decoded.frames, score::total_frames());

    let (left, right) = (
        extract_channel(&decoded, Channel::Left),
        extract_channel(&decoded, Channel::Right),
    );
    for (i, chunk) in interleaved.chunks_exact(2).enumerate() {
        let err = (left[i] - chunk[0]).abs().max((right[i] - chunk[1]).abs());
        assert!(
            err < TOL_24,
            "export round trip diverged at frame {i}: {err}"
        );
    }

    // The muted lead really is gone. Measure it on a barbershop seventh where the
    // lead sings the seventh: that pitch belongs to no other voice in the chord
    // (a fifth or an octave would just show up as somebody else's overtone), so
    // its energy in the exported mix should have collapsed.
    let event = score::SCORE
        .iter()
        .position(|e| {
            e.quality == score::Quality::Dominant7
                && e.notes.iter().all(Option::is_some)
                && e.notes[Part::Lead.index()]
                    .map(|n| (score::midi(n) - score::midi(e.root.unwrap())).rem_euclid(12) == 10)
                    .unwrap_or(false)
        })
        .expect("the lead sings a seventh somewhere");
    let (start, end) = score::event_spans()[event];
    let mono: Vec<f32> = left[start..end]
        .iter()
        .zip(&right[start..end])
        .map(|(l, r)| l + r)
        .collect();
    let window = steady(&mono, 0, mono.len());
    let lead_level = goertzel(window, SAMPLE_RATE, score::hz(event, Part::Lead).unwrap());
    let bari_level = goertzel(
        window,
        SAMPLE_RATE,
        score::hz(event, Part::Baritone).unwrap(),
    );
    assert!(
        lead_level < bari_level * 0.1,
        "the muted lead is still audible in '{}': {lead_level:.5} at its pitch vs \
         {bari_level:.5} at the baritone's",
        score::SCORE[event].label
    );
}

#[test]
fn slowing_a_real_track_down_keeps_the_pitch() {
    let set = fixtures::ensure();
    let lead = channel_of(set.part_left(Part::Lead), Channel::Left);

    let tempo = 0.75f32; // practice speed
    let slowed = time_stretch(&lead, 1, SAMPLE_RATE, tempo);

    let expected_len = (lead.len() as f32 / tempo) as usize;
    let slack = (SAMPLE_RATE as f32 * 0.2) as usize;
    assert!(
        slowed.len().abs_diff(expected_len) < slack,
        "stretched length {} is not near {expected_len}",
        slowed.len()
    );

    // The longest lead note (the tag's Bb6) should still be a D4 afterwards —
    // stretched in time, unchanged in pitch.
    let note = score::sounding(Part::Lead)
        .into_iter()
        .max_by_key(|n| n.end - n.start)
        .expect("the lead sings");
    let scale = |frame: usize| (frame as f32 / tempo) as usize;
    let window = steady(&slowed, scale(note.start), scale(note.end));
    assert!(!window.is_empty(), "no stretched audio to measure");

    let at_pitch = goertzel(window, SAMPLE_RATE, note.hz);
    // A naive resample would have shifted the pitch by 1/tempo.
    let resampled = goertzel(window, SAMPLE_RATE, note.hz / tempo as f64);
    assert!(
        at_pitch > resampled * 4.0,
        "slowing down shifted the pitch: {:.1} Hz={at_pitch:.5}, {:.1} Hz={resampled:.5}",
        note.hz,
        note.hz / tempo as f64
    );
    for offset in [-1.0, 1.0] {
        let neighbour = goertzel(window, SAMPLE_RATE, semitones_from(note.hz, offset));
        assert!(
            at_pitch > neighbour * 3.0,
            "stretched note is not clearly a {:.1} Hz pitch ({offset:+} semitones: {neighbour:.5})",
            note.hz
        );
    }
}

// --- alignment --------------------------------------------------------------

#[test]
fn a_misaligned_set_is_measured_and_put_back_together() {
    let set = fixtures::ensure();

    // The `misaligned/` set is shaped like a publisher's: spoken title, pitch
    // pipe, a gap — and the song pasted in late in three of the four files.
    let files: Vec<audio_core::DecodedAudio> = PARTS
        .iter()
        .map(|&p| decode_file(set.misaligned(p)).expect("decode misaligned fixture"))
        .collect();
    let monos: Vec<Vec<f32>> = files
        .iter()
        .map(|d| {
            let mut mono = vec![0.0f32; d.frames];
            for channel in &d.planar {
                for (m, s) in mono.iter_mut().zip(channel) {
                    *m += *s;
                }
            }
            mono
        })
        .collect();

    let refs: Vec<&[f32]> = monos.iter().map(|m| m.as_slice()).collect();
    let corrections = audio_core::align_set(&refs, SAMPLE_RATE);

    for part in PARTS {
        let expected = fixtures::MISALIGNED_EXTRA[part.index()] as i64;
        let c = corrections[part.index()];
        assert!(
            (c.offset_frames - expected).abs() <= 2,
            "{}: measured {} frames late, built {expected}",
            part.slug(),
            c.offset_frames
        );
        assert!(c.consistent, "{}: measurement wandered", part.slug());
        assert_eq!(
            c.delta_frames,
            -expected,
            "{}: should have that much silence cut",
            part.slug()
        );

        // The cut has to land in the gap, not in the pitch pipe or the singing.
        let cut = (-c.delta_frames) as usize;
        let region = &monos[part.index()][c.splice_at..c.splice_at + cut];
        assert!(
            region.iter().all(|s| s.abs() < 1e-4),
            "{}: the correction would cut audio, not silence",
            part.slug()
        );
    }

    // Corrected, the isolated voices line up with each other again — and with
    // the intro, which never moved.
    let corrected: Vec<Vec<f32>> = PARTS
        .iter()
        .map(|&p| {
            let stem = extract_channel(&files[p.index()], Channel::Left);
            let c = corrections[p.index()];
            audio_core::splice(&stem, c.splice_at, c.delta_frames)
        })
        .collect();

    let lead_in_frames = corrected[0].len() - score::total_frames();
    for part in PARTS {
        let expected = score::render_part(part);
        let stem = &corrected[part.index()];
        assert_eq!(
            stem.len(),
            corrected[0].len(),
            "{}: corrected stems should all be the same length",
            part.slug()
        );
        let err = max_abs_diff(&stem[lead_in_frames..], &expected);
        assert!(
            err < TOL_16,
            "{}: after correction the voice doesn't line up with the score (max_err={err})",
            part.slug()
        );
    }
}

// --- tags -------------------------------------------------------------------

#[test]
fn flac_tracks_are_lossless_and_carry_vendor_tags() {
    let set = fixtures::ensure();
    let mut per_part: Vec<Tags> = Vec::new();

    for part in PARTS {
        let path = set.flac(part);
        let isolated = channel_of(path, Channel::Left);
        assert!(
            max_abs_diff(&isolated, &score::render_part(part)) < TOL_16,
            "{}: FLAC did not preserve the isolated part",
            path.display()
        );

        let tags = read_tags(path).expect("read flac tags");
        // Vendors put the voice in `artist` and the song in `title` — the app's
        // fallback part detection depends on it.
        assert_eq!(tags.get("artist").map(String::as_str), Some(part.token()));
        assert_eq!(tags.get("title").map(String::as_str), Some(score::SONG));
        assert_eq!(tags.get("album").map(String::as_str), Some(fixtures::ALBUM));
        per_part.push(tags);
    }

    // What the exporter carries over: tags identical across all four sources.
    let common: Tags = per_part[0]
        .iter()
        .filter(|(k, v)| per_part.iter().all(|t| t.get(*k) == Some(*v)))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    assert!(
        common.contains_key("album")
            && common.contains_key("title")
            && common.contains_key("genre"),
        "expected the shared tags to survive: {common:?}"
    );
    assert!(
        !common.contains_key("artist"),
        "the per-part `artist` tag should drop out of the common set"
    );
}

#[cfg(feature = "mp3")]
#[test]
fn mp3_export_is_a_complete_stream_with_tags() {
    let set = fixtures::ensure();

    for part in PARTS {
        let path = set.mp3(part).expect("mp3 fixtures with --features mp3");
        let bytes = std::fs::read(path).expect("read mp3 fixture");
        assert!(
            bytes.starts_with(b"ID3"),
            "{} should start with its ID3 tag",
            path.display()
        );

        // Every MPEG frame the encoder wrote is present and well-formed: 1152
        // samples each, covering the whole song (the last frame is zero-padded).
        let frames = mp3_frame_count(&bytes);
        let expected = score::total_frames().div_ceil(1152);
        assert!(
            frames >= expected,
            "{}: {frames} MPEG frames cover less than the {expected} the song needs",
            path.display()
        );

        let tags = read_tags(path).expect("read mp3 tags");
        assert_eq!(tags.get("artist").map(String::as_str), Some(part.token()));
        assert_eq!(tags.get("title").map(String::as_str), Some(score::SONG));
        assert_eq!(tags.get("album").map(String::as_str), Some(fixtures::ALBUM));
    }
}

/// Number of MPEG audio frames in an MP3 file (skipping a leading ID3v2 tag).
#[cfg(feature = "mp3")]
fn mp3_frame_count(bytes: &[u8]) -> usize {
    const BITRATES: [u32; 16] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
    ];
    let mut i = if bytes.starts_with(b"ID3") {
        let size = bytes[6..10]
            .iter()
            .fold(0usize, |acc, b| (acc << 7) | (*b as usize & 0x7f));
        10 + size
    } else {
        0
    };
    let mut frames = 0;
    while i + 4 <= bytes.len() {
        if bytes[i] != 0xFF || bytes[i + 1] & 0xE0 != 0xE0 {
            i += 1;
            continue;
        }
        let bitrate = BITRATES[(bytes[i + 2] >> 4) as usize & 0xF];
        let rate = match (bytes[i + 2] >> 2) & 3 {
            0 => 44_100,
            1 => 48_000,
            2 => 32_000,
            _ => 0,
        };
        if bitrate == 0 || rate == 0 {
            i += 1;
            continue;
        }
        let padding = ((bytes[i + 2] >> 1) & 1) as usize;
        frames += 1;
        i += (144 * bitrate as usize * 1000) / rate as usize + padding;
    }
    frames
}

/// Known defect, kept as executable documentation: Symphonia rejects ~17% of the
/// packets the Shine encoder writes ("mpa: invalid main_data offset" — Shine's
/// bit-reservoir bookkeeping; ffmpeg decodes the same files in full), and
/// `decode_file` skips undecodable packets silently. The result is an exported
/// MP3 that loses ~2.5 s spread through the file when it is read back in.
/// Un-ignore once Shine or Symphonia is fixed.
#[cfg(feature = "mp3")]
#[test]
#[ignore = "shine/symphonia bit-reservoir mismatch drops packets; see comment"]
fn mp3_export_survives_a_symphonia_round_trip() {
    let set = fixtures::ensure();

    for part in PARTS {
        let path = set.mp3(part).expect("mp3 fixtures with --features mp3");
        let decoded = decode_file(path).expect("decode mp3 fixture");
        assert_eq!(decoded.channels, 2);
        // MP3 pads the stream, so it is never shorter than the source.
        assert!(
            decoded.frames >= score::total_frames(),
            "{}: decoded {} frames, expected at least {}",
            path.display(),
            decoded.frames,
            score::total_frames()
        );

        let isolated = extract_channel(&decoded, Channel::Left);
        let reference = score::render_part(part);
        let note = score::sounding(part)
            .into_iter()
            .max_by_key(|n| n.end - n.start)
            .expect("every part sings");

        // Lossy, so compare shape rather than samples. MP3 also delays the
        // signal, so search a small range of offsets for the best alignment.
        let window = steady(&reference, note.start, note.end);
        let best = (0..2400)
            .map(|lag| correlation(&isolated[note.start + lag..], window))
            .fold(0.0f32, f32::max);
        assert!(
            best > 0.9,
            "{}: decoded MP3 doesn't match the {} part (best correlation {best})",
            path.display(),
            part.slug()
        );

        let (start, end) = score::full_rests()[0];
        let quiet = peak(slice(&isolated, start + 4800, end.saturating_sub(4800)));
        assert!(
            db(quiet) < -50.0,
            "{}: MP3 rest is not quiet ({:.1} dBFS)",
            path.display(),
            db(quiet)
        );
    }
}
