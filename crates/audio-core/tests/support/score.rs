//! The synthetic test song: a barbershop arrangement built to the style rules in
//! the Barbershop Harmony Society's *Music Educator Guide and Songbook* (v3.5,
//! "Characteristics of the Barbershop Style").
//!
//! Real learning tracks are copyrighted, so the test suite sings its own song.
//! What the guide asks for, and what this score does:
//!
//! * **Four voices, melody in the second one down** — tenor above the lead, then
//!   baritone, then bass. No voice crossings here, and every note sits inside the
//!   ranges the guide gives (bass G2–D4, baritone B2–D♯4, lead within the
//!   changing-voice D3–F4, tenor within A3–F5).
//! * **Barbershop sevenths resolving around the circle of fifths** — the chords
//!   are majors, sixths and dominant sevenths, and *every* seventh resolves down
//!   a perfect fifth (`G7 → C7 → F7 → B♭`). No chord contains a minor second.
//! * **Just intonation** — pitches are tuned as ratios, not equal temperament, so
//!   the chords lock and ring. Chord roots come from the key lattice (major 2nd
//!   9/8, major 3rd 5/4, fourth 4/3, fifth 3/2, major 6th 27/16 — the guide's
//!   "2nds, 6ths and 5ths higher, 3rds lower than the piano"), and each chord's
//!   tones are just ratios above its own root, which makes a dominant seventh a
//!   4:5:6:7 chord with the seventh a full 31 cents under the tempered one
//!   ("the minor 7th … quite lower").
//! * **Balance** — roots and fifths carry the chord, thirds and sevenths are sung
//!   lighter, and the voices are weighted bass-heaviest to tenor-lightest, the
//!   guide's 4:3:3:1 pyramid.
//! * **Homorhythm** — all sounding voices change together, on the same syllable.
//! * **Embellishments** — swipes (a chord-tone change *inside* a held word), a
//!   word echo, trio textures, and a four-bar tag ending on a held post.
//!
//! The rests are deliberate, and they are the kind a quartet actually sings: the
//! breath before the pickup, the breath between phrases, the voices that drop out
//! of the echo and the trios, and the harmony sustaining under the melody's
//! breath. They give the tests spans that must be bit-exact silence in one part
//! while the other three keep singing.
//!
//! Everything here is deterministic, so the rendered stems double as the ground
//! truth the decode/extract tests compare against.

#![allow(dead_code)] // Each consumer (tests, the generator example) uses a subset.

use std::f64::consts::PI;

/// Sample rate of every generated fixture.
pub const SAMPLE_RATE: u32 = 44_100;
/// Tempo of the test song.
pub const BPM: f64 = 120.0;
/// Seconds per beat at [`BPM`].
pub const SECONDS_PER_BEAT: f64 = 60.0 / BPM;

/// Song title used in fixture filenames and tags.
pub const SONG: &str = "Circle of Fifths";
/// Track number + title + key, matching how vendors name learning tracks.
pub const SONG_BASE: &str = "01 Circle of Fifths [Bb]";
/// Stand-in for the vendor/arranger name in fixture filenames.
pub const VENDOR: &str = "Track Exploder";
/// Stand-in render date in fixture filenames.
pub const RENDER_DATE: &str = "20260101";

/// The key. B♭ major is a barbershop home key.
pub const KEY: &str = "Bb";
/// MIDI number of the tonic reference (B♭1).
const TONIC_MIDI: i32 = 34;
/// Frequency of that tonic (equal-tempered B♭1 with A4 = 440 Hz); everything
/// else is tuned justly *from* it.
const TONIC_HZ: f64 = 58.270_47;

/// The four barbershop voices, in score order (matches the frontend's `PARTS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Tenor,
    Lead,
    Baritone,
    Bass,
}

pub const PARTS: [Part; 4] = [Part::Tenor, Part::Lead, Part::Baritone, Part::Bass];

impl Part {
    /// Index into an [`Event`]'s note array (and into `PARTS`).
    pub fn index(self) -> usize {
        match self {
            Part::Tenor => 0,
            Part::Lead => 1,
            Part::Baritone => 2,
            Part::Bass => 3,
        }
    }

    /// Lower-case identifier, as the frontend names the part.
    pub fn slug(self) -> &'static str {
        match self {
            Part::Tenor => "tenor",
            Part::Lead => "lead",
            Part::Baritone => "baritone",
            Part::Bass => "bass",
        }
    }

    /// The upper-case token vendors put in filenames and the `artist` tag.
    pub fn token(self) -> &'static str {
        match self {
            Part::Tenor => "TENOR",
            Part::Lead => "LEAD",
            Part::Baritone => "BARI",
            Part::Bass => "BASS",
        }
    }

    /// Comfortable range for the part, per the guide's voice-range section.
    pub fn range(self) -> (&'static str, &'static str) {
        match self {
            Part::Tenor => ("A3", "F5"),
            Part::Lead => ("D3", "F4"),
            Part::Baritone => ("B2", "D#4"),
            Part::Bass => ("G2", "D4"),
        }
    }

    /// Section weight: the guide's suggested chorus balance is four basses, three
    /// baritones, three leads and one tenor, and high voices carry further than
    /// low ones — so the bass anchors and the tenor floats on top.
    fn weight(self) -> f64 {
        match self {
            Part::Tenor => 0.19,
            Part::Lead => 0.30,
            Part::Baritone => 0.23,
            Part::Bass => 0.33,
        }
    }
}

/// Chord quality. The style lives on majors, sixths and dominant sevenths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    /// Nobody is singing.
    Rest,
    Major,
    Major6,
    /// The barbershop seventh.
    Dominant7,
}

/// One chord of the arrangement — a syllable sung by everybody at once.
///
/// `notes` is indexed by [`Part::index`]; `None` means that voice rests.
pub struct Event {
    /// Chord name as an arranger would write it.
    pub label: &'static str,
    /// The syllable being sung, or `None` for a rest. A [`Event::swipe`] event
    /// keeps the previous syllable.
    pub lyric: Option<&'static str>,
    pub beats: f64,
    /// Chord root pitch class (`None` while resting).
    pub root: Option<&'static str>,
    pub quality: Quality,
    pub notes: [Option<&'static str>; 4],
    /// A chord-tone change *inside* the held word — the barbershop swipe. The
    /// voices don't re-articulate; one of them just moves.
    pub swipe: bool,
}

const REST: Event = Event {
    label: "(breath)",
    lyric: None,
    beats: 1.0,
    root: None,
    quality: Quality::Rest,
    notes: [None, None, None, None],
    swipe: false,
};

/// The arrangement.
///
/// Phrase: `Bb → Bb6 → G7 → C7 → F7 → Bb → Bb6` — the wheel, I – VI7 – II7 – V7 – I.
/// Middle: a word echo (lead and bass alone) and two trio bars.
/// Tag: `D7 → G7 → C7 → F7 → Bb`, swiped to Bb6 and back onto a held post.
pub const SCORE: &[Event] = &[
    // A file that starts silent is normal, and it catches decoders that trim
    // leading zeros.
    Event {
        label: "(breath)",
        beats: 1.0,
        ..REST
    },
    // The opening chord is the guide's own tuning stack: lead on the tonic, bass
    // an octave below, bari the fifth between them, tenor the third above.
    Event {
        label: "Bb",
        lyric: Some("Ring"),
        beats: 2.0,
        root: Some("Bb"),
        quality: Quality::Major,
        notes: [Some("D4"), Some("Bb3"), Some("F3"), Some("Bb2")],
        swipe: false,
    },
    // Swipe: the bari walks the fifth up to the sixth while the word holds.
    Event {
        label: "Bb6",
        lyric: None,
        beats: 1.0,
        root: Some("Bb"),
        quality: Quality::Major6,
        notes: [Some("D4"), Some("Bb3"), Some("G3"), Some("Bb2")],
        swipe: true,
    },
    Event {
        label: "G7",
        lyric: Some("out,"),
        beats: 2.0,
        root: Some("G"),
        quality: Quality::Dominant7,
        notes: [Some("F4"), Some("D4"), Some("B3"), Some("G2")],
        swipe: false,
    },
    Event {
        label: "C7",
        lyric: Some("sweet"),
        beats: 2.0,
        root: Some("C"),
        quality: Quality::Dominant7,
        notes: [Some("E4"), Some("Bb3"), Some("G3"), Some("C3")],
        swipe: false,
    },
    Event {
        label: "F7",
        lyric: Some("chords"),
        beats: 2.0,
        root: Some("F"),
        quality: Quality::Dominant7,
        notes: [Some("Eb4"), Some("C4"), Some("A3"), Some("F3")],
        swipe: false,
    },
    Event {
        label: "Bb",
        lyric: Some("mine,"),
        beats: 2.0,
        root: Some("Bb"),
        quality: Quality::Major,
        notes: [Some("D4"), Some("Bb3"), Some("F3"), Some("Bb2")],
        swipe: false,
    },
    Event {
        label: "Bb6",
        lyric: None,
        beats: 2.0,
        root: Some("Bb"),
        quality: Quality::Major6,
        notes: [Some("D4"), Some("Bb3"), Some("G3"), Some("Bb2")],
        swipe: true,
    },
    // Quartets breathe together: both channels of every file go silent here.
    Event {
        label: "(breath)",
        beats: 2.0,
        ..REST
    },
    Event {
        label: "Bb",
        lyric: Some("Hold"),
        beats: 2.0,
        root: Some("Bb"),
        quality: Quality::Major,
        notes: [Some("D4"), Some("Bb3"), Some("F3"), Some("Bb2")],
        swipe: false,
    },
    // Word echo: the lead and bass answer alone, an octave apart.
    Event {
        label: "Bb (echo)",
        lyric: Some("hold"),
        beats: 2.0,
        root: Some("Bb"),
        quality: Quality::Major,
        notes: [None, Some("Bb3"), None, Some("Bb2")],
        swipe: false,
    },
    // Upper trio — the bass sits out.
    Event {
        label: "Eb",
        lyric: Some("the"),
        beats: 2.0,
        root: Some("Eb"),
        quality: Quality::Major,
        notes: [Some("G4"), Some("Eb4"), Some("Bb3"), None],
        swipe: false,
    },
    // Lower trio — the tenor sits out.
    Event {
        label: "F",
        lyric: Some("chord"),
        beats: 2.0,
        root: Some("F"),
        quality: Quality::Major,
        notes: [None, Some("C4"), Some("A3"), Some("F3")],
        swipe: false,
    },
    Event {
        label: "(breath)",
        beats: 1.0,
        ..REST
    },
    // Harmony sustains the root and fifth on a neutral vowel while the melody
    // breathes — the guide's own keynote exercise.
    Event {
        label: "Bb (bass & bari)",
        lyric: Some("ooh"),
        beats: 2.0,
        root: Some("Bb"),
        quality: Quality::Major,
        notes: [None, None, Some("F3"), Some("Bb2")],
        swipe: false,
    },
    // --- the tag: four bars around the wheel, then the post ---
    Event {
        label: "D7",
        lyric: Some("Let"),
        beats: 2.0,
        root: Some("D"),
        quality: Quality::Dominant7,
        notes: [Some("F#4"), Some("C4"), Some("A3"), Some("D3")],
        swipe: false,
    },
    Event {
        label: "G7",
        lyric: Some("it"),
        beats: 2.0,
        root: Some("G"),
        quality: Quality::Dominant7,
        notes: [Some("F4"), Some("D4"), Some("B3"), Some("G2")],
        swipe: false,
    },
    Event {
        label: "C7",
        lyric: Some("ring"),
        beats: 2.0,
        root: Some("C"),
        quality: Quality::Dominant7,
        notes: [Some("E4"), Some("Bb3"), Some("G3"), Some("C3")],
        swipe: false,
    },
    Event {
        label: "F7",
        lyric: Some("for"),
        beats: 2.0,
        root: Some("F"),
        quality: Quality::Dominant7,
        notes: [Some("Eb4"), Some("C4"), Some("A3"), Some("F3")],
        swipe: false,
    },
    Event {
        label: "Bb",
        lyric: Some("you."),
        beats: 2.0,
        root: Some("Bb"),
        quality: Quality::Major,
        notes: [Some("D4"), Some("Bb3"), Some("F3"), Some("Bb2")],
        swipe: false,
    },
    Event {
        label: "Bb6",
        lyric: None,
        beats: 2.0,
        root: Some("Bb"),
        quality: Quality::Major6,
        notes: [Some("D4"), Some("Bb3"), Some("G3"), Some("Bb2")],
        swipe: true,
    },
    // The post: back onto the tonic chord and hold it.
    Event {
        label: "Bb (post)",
        lyric: None,
        beats: 3.0,
        root: Some("Bb"),
        quality: Quality::Major,
        notes: [Some("D4"), Some("Bb3"), Some("F3"), Some("Bb2")],
        swipe: true,
    },
    Event {
        label: "(release)",
        beats: 2.0,
        ..REST
    },
];

// --- tuning -----------------------------------------------------------------

/// MIDI number of a note name like `Bb2`, `F#4`, `C4`.
pub fn midi(name: &str) -> i32 {
    let bytes = name.as_bytes();
    let mut semitones: i32 = match bytes[0] {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        other => panic!("bad note letter {:?} in {name}", other as char),
    };
    let mut i = 1;
    while i < bytes.len() && (bytes[i] == b'#' || bytes[i] == b'b') {
        semitones += if bytes[i] == b'#' { 1 } else { -1 };
        i += 1;
    }
    if i == bytes.len() {
        // A bare pitch class: treat it as octave 0.
        return 12 + semitones;
    }
    let octave: i32 = name[i..]
        .parse()
        .unwrap_or_else(|_| panic!("bad octave in {name}"));
    (octave + 1) * 12 + semitones
}

/// Equal-tempered frequency (A4 = 440 Hz) — what a piano would give you, and the
/// reference the tuning tests measure the justly-tuned fixtures against.
pub fn equal_tempered_hz(name: &str) -> f64 {
    440.0 * 2f64.powf((midi(name) as f64 - 69.0) / 12.0)
}

/// Just ratio of a scale degree above the tonic, following the guide: 2nds, 6ths
/// and 5ths sit *above* their tempered pitch, 3rds below.
fn key_ratio(semitones_above_tonic: i32) -> f64 {
    match semitones_above_tonic {
        0 => 1.0,
        2 => 9.0 / 8.0,
        4 => 5.0 / 4.0,
        5 => 4.0 / 3.0,
        7 => 3.0 / 2.0,
        9 => 27.0 / 16.0,
        11 => 15.0 / 8.0,
        other => panic!("no just tuning for {other} semitones above the tonic"),
    }
}

/// Just ratio of a chord tone above its own root. A dominant seventh built from
/// these is a 4:5:6:7 chord — the ratio that makes barbershop ring.
fn chord_tone_ratio(semitones_above_root: i32) -> f64 {
    match semitones_above_root {
        0 => 1.0,
        2 => 9.0 / 8.0,
        3 => 6.0 / 5.0,
        4 => 5.0 / 4.0,
        5 => 4.0 / 3.0,
        7 => 3.0 / 2.0,
        9 => 5.0 / 3.0,
        10 => 7.0 / 4.0,
        other => panic!("{other} semitones above the root is not a chord tone we sing"),
    }
}

/// Frequency of a chord root at a given MIDI pitch, tuned from the key's tonic.
fn root_hz(root_midi: i32) -> f64 {
    let relative = root_midi - TONIC_MIDI;
    TONIC_HZ * key_ratio(relative.rem_euclid(12)) * 2f64.powi(relative.div_euclid(12))
}

/// Justly-tuned frequency of `note` sung as part of a chord on `root`.
pub fn just_hz(root: &str, note: &str) -> f64 {
    let note_midi = midi(note);
    let root_pc = midi(root).rem_euclid(12);
    // The chord root at or below the note.
    let root_midi = note_midi - (note_midi - root_pc).rem_euclid(12);
    root_hz(root_midi) * chord_tone_ratio(note_midi - root_midi)
}

/// Frequency the given voice sings in the given event, if it isn't resting.
pub fn hz(event: usize, part: Part) -> Option<f64> {
    let e = &SCORE[event];
    e.notes[part.index()].map(|note| just_hz(e.root.expect("a sung chord has a root"), note))
}

// --- timing -----------------------------------------------------------------

fn frames_for(beats: f64) -> usize {
    (beats * SECONDS_PER_BEAT * SAMPLE_RATE as f64).round() as usize
}

/// Frame range `[start, end)` of each event, in order.
pub fn event_spans() -> Vec<(usize, usize)> {
    let mut spans = Vec::with_capacity(SCORE.len());
    let mut at = 0usize;
    for event in SCORE {
        let end = at + frames_for(event.beats);
        spans.push((at, end));
        at = end;
    }
    spans
}

/// Total length of the song in frames.
pub fn total_frames() -> usize {
    event_spans().last().map(|(_, end)| *end).unwrap_or(0)
}

/// Song duration in seconds.
pub fn duration_secs() -> f64 {
    total_frames() as f64 / SAMPLE_RATE as f64
}

/// A note the given part sings: the event index, its frame span, and its pitch.
pub struct Sounding {
    pub event: usize,
    pub start: usize,
    pub end: usize,
    pub hz: f64,
}

/// Every note `part` sings, in order.
pub fn sounding(part: Part) -> Vec<Sounding> {
    event_spans()
        .into_iter()
        .enumerate()
        .filter_map(|(event, (start, end))| {
            hz(event, part).map(|hz| Sounding {
                event,
                start,
                end,
                hz,
            })
        })
        .collect()
}

/// Frame spans where `part` rests (exact silence in that part's isolated channel).
pub fn rests(part: Part) -> Vec<(usize, usize)> {
    event_spans()
        .into_iter()
        .enumerate()
        .filter(|(i, _)| SCORE[*i].notes[part.index()].is_none())
        .map(|(_, span)| span)
        .collect()
}

/// Frame spans where *every* part rests — both channels of every file are silent.
pub fn full_rests() -> Vec<(usize, usize)> {
    event_spans()
        .into_iter()
        .enumerate()
        .filter(|(i, _)| SCORE[*i].notes.iter().all(Option::is_none))
        .map(|(_, span)| span)
        .collect()
}

/// The first event where `part` rests while the rest of the quartet sings.
pub fn first_tacet(part: Part) -> usize {
    SCORE
        .iter()
        .position(|e| e.notes[part.index()].is_none() && e.notes.iter().any(Option::is_some))
        .unwrap_or_else(|| panic!("{} never drops out", part.slug()))
}

// --- synthesis --------------------------------------------------------------

/// Sung vowels, as formant triples (frequency, bandwidth) — the thing that makes
/// a tone sound like a voice rather than an organ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vowel {
    Ah,
    Ee,
    Eh,
    Oh,
    Oo,
}

impl Vowel {
    fn formants(self) -> [(f64, f64); 3] {
        match self {
            Vowel::Ah => [(730.0, 90.0), (1090.0, 110.0), (2440.0, 140.0)],
            Vowel::Ee => [(270.0, 60.0), (2290.0, 110.0), (3010.0, 160.0)],
            Vowel::Eh => [(530.0, 80.0), (1840.0, 110.0), (2480.0, 140.0)],
            Vowel::Oh => [(570.0, 80.0), (840.0, 100.0), (2410.0, 140.0)],
            Vowel::Oo => [(300.0, 60.0), (870.0, 90.0), (2240.0, 140.0)],
        }
    }
}

/// The vowel of a sung syllable.
fn vowel_of(lyric: &str) -> Vowel {
    match lyric.trim_end_matches([',', '.']) {
        "Ring" | "ring" | "it" => Vowel::Ee,
        "out" | "Hold" | "hold" | "chord" | "chords" | "for" => Vowel::Oh,
        "sweet" => Vowel::Ee,
        "mine" | "the" => Vowel::Ah,
        "Let" => Vowel::Eh,
        "ooh" | "you" => Vowel::Oo,
        _ => Vowel::Ah,
    }
}

/// Formants shift with the size of the singer: the tenor's sit higher than the
/// bass's.
fn formant_shift(part: Part) -> f64 {
    match part {
        Part::Tenor => 1.14,
        Part::Lead => 1.04,
        Part::Baritone => 1.0,
        Part::Bass => 0.94,
    }
}

/// How loudly a chord tone is sung. The guide: unisons, octaves and fifths
/// reinforce the chord and should be emphasized; major thirds and minor sevenths
/// throw off dissonant harmonics and should be de-emphasized. The lead is never
/// held back — the harmony balances to the melody, not the other way round.
fn chord_tone_weight(part: Part, semitones_above_root: i32) -> f64 {
    if part == Part::Lead {
        return 1.0;
    }
    match semitones_above_root {
        0 | 7 => 1.0,
        9 => 0.85,
        4 => 0.8,
        10 => 0.72,
        _ => 0.85,
    }
}

fn amplitude(event: usize, part: Part) -> f64 {
    let e = &SCORE[event];
    let Some(note) = e.notes[part.index()] else {
        return 0.0;
    };
    let root_pc = midi(e.root.expect("a sung chord has a root")).rem_euclid(12);
    let degree = (midi(note) - root_pc).rem_euclid(12);
    part.weight() * chord_tone_weight(part, degree)
}

/// Sung onset/release. A swipe keeps the word going, so it neither ends the
/// previous note nor re-articulates.
const ATTACK_SECS: f64 = 0.028;
const RELEASE_SECS: f64 = 0.110;
/// Gap after a released note, so a rest is exact digital silence and consecutive
/// syllables are articulated rather than slurred.
const NOTE_GAP_SECS: f64 = 0.035;
/// How long a swipe takes to move between chord tones.
const SWIPE_SECS: f64 = 0.030;

/// The harmonic amplitude of the glottal source, before the vowel shapes it
/// (about -12 dB per octave, like a sung tone).
fn source_level(harmonic: usize) -> f64 {
    1.0 / (harmonic as f64).powi(2)
}

/// Magnitude of a two-pole formant resonance at `freq`.
fn resonance(freq: f64, center: f64, bandwidth: f64) -> f64 {
    let d = (freq * freq - center * center) / (freq * bandwidth);
    1.0 / (1.0 + d * d).sqrt()
}

/// Most harmonics we synthesize per voice.
const MAX_HARMONICS: usize = 24;

/// Harmonic amplitudes of one sung note, normalized so they sum to 1.
fn spectrum(hz: f64, vowel: Vowel, part: Part) -> Vec<f64> {
    let shift = formant_shift(part);
    let formants = vowel.formants();
    let nyquist = SAMPLE_RATE as f64 / 2.0;
    let count = ((4800.0 / hz).floor() as usize).clamp(4, MAX_HARMONICS);

    let mut levels = Vec::with_capacity(count);
    for h in 1..=count {
        let freq = hz * h as f64;
        if freq >= nyquist {
            break;
        }
        let shaped: f64 = formants
            .iter()
            .zip([1.0, 0.65, 0.4])
            .map(|(&(center, bw), gain)| gain * resonance(freq, center * shift, bw))
            .sum();
        levels.push(source_level(h) * (shaped + 0.02));
    }
    let total: f64 = levels.iter().sum();
    levels.iter_mut().for_each(|l| *l /= total);
    levels
}

/// One continuous stretch of singing for a voice: a syllable, plus any swipes
/// that keep it going.
struct Phrase {
    /// `(event, start, end)` per chord within the syllable.
    segments: Vec<(usize, usize, usize)>,
    vowel: Vowel,
}

/// Group a part's events into sung syllables (a swipe continues the one before).
fn phrases(part: Part) -> Vec<Phrase> {
    let spans = event_spans();
    let mut out: Vec<Phrase> = Vec::new();

    for (event, &(start, end)) in spans.iter().enumerate() {
        let e = &SCORE[event];
        if e.notes[part.index()].is_none() {
            continue;
        }
        let continues = e.swipe
            && out.last().is_some_and(|p| {
                p.segments
                    .last()
                    .is_some_and(|&(prev, _, prev_end)| prev + 1 == event && prev_end == start)
            });
        if continues {
            out.last_mut().unwrap().segments.push((event, start, end));
        } else {
            out.push(Phrase {
                segments: vec![(event, start, end)],
                vowel: vowel_of(e.lyric.unwrap_or("ah")),
            });
        }
    }
    out
}

/// Render one voice as a mono buffer — the ground truth an extracted part must
/// match, sample for sample.
pub fn render_part(part: Part) -> Vec<f32> {
    let mut buf = vec![0.0f32; total_frames()];
    let gap = (NOTE_GAP_SECS * SAMPLE_RATE as f64) as usize;
    let attack = (ATTACK_SECS * SAMPLE_RATE as f64) as usize;
    let release = (RELEASE_SECS * SAMPLE_RATE as f64) as usize;
    let swipe_ramp = (SWIPE_SECS * SAMPLE_RATE as f64) as usize;

    for phrase in phrases(part) {
        let start = phrase.segments[0].1;
        let end = phrase.segments.last().unwrap().2.saturating_sub(gap);
        if end <= start {
            continue;
        }
        let len = end - start;
        let attack = attack.min(len / 4);
        let release = release.min(len / 4);

        // Phase runs continuously through the whole syllable, so a swipe changes
        // pitch without a click.
        let mut phases = [0.0f64; MAX_HARMONICS];

        for (index, &(event, seg_start, seg_end)) in phrase.segments.iter().enumerate() {
            let pitch = hz(event, part).expect("the voice sings in this event");
            let levels = spectrum(pitch, phrase.vowel, part);
            let level = amplitude(event, part);
            // Level glides across a swipe rather than stepping.
            let previous_level = index
                .checked_sub(1)
                .map(|p| amplitude(phrase.segments[p].0, part))
                .unwrap_or(level);

            for (offset, out) in buf[seg_start..seg_end.min(end)].iter_mut().enumerate() {
                let frame = seg_start + offset;
                let i = frame - start;
                let envelope = if i < attack {
                    i as f64 / attack as f64
                } else if i >= len - release {
                    (len - i) as f64 / release as f64
                } else {
                    1.0
                };
                let glide = if index > 0 && frame - seg_start < swipe_ramp {
                    let t = (frame - seg_start) as f64 / swipe_ramp as f64;
                    previous_level * (1.0 - t) + level * t
                } else {
                    level
                };

                let mut sample = 0.0;
                for (h, &amp) in levels.iter().enumerate() {
                    phases[h] += 2.0 * PI * pitch * (h + 1) as f64 / SAMPLE_RATE as f64;
                    sample += amp * phases[h].sin();
                }
                *out += (sample * envelope * glide) as f32;
            }
        }
    }
    buf
}

/// Render all four voices, indexed by [`Part::index`].
pub fn render_all() -> [Vec<f32>; 4] {
    [
        render_part(Part::Tenor),
        render_part(Part::Lead),
        render_part(Part::Baritone),
        render_part(Part::Bass),
    ]
}

/// Sum of the three voices *other* than `part` — what a part-left track carries
/// on its right channel.
pub fn mix_of_others(stems: &[Vec<f32>; 4], part: Part) -> Vec<f32> {
    let mut out = vec![0.0f32; total_frames()];
    for other in PARTS {
        if other == part {
            continue;
        }
        for (o, s) in out.iter_mut().zip(&stems[other.index()]) {
            *o += *s;
        }
    }
    out
}

/// Interleave a learning track: the isolated part on one channel, the other
/// three summed on the opposite one.
pub fn interleave_learning_track(isolated: &[f32], others: &[f32], part_on_left: bool) -> Vec<f32> {
    let mut out = Vec::with_capacity(isolated.len() * 2);
    for (a, b) in isolated.iter().zip(others) {
        if part_on_left {
            out.push(*a);
            out.push(*b);
        } else {
            out.push(*b);
            out.push(*a);
        }
    }
    out
}
