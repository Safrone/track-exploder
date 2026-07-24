//! Render [`score`](super::score) into **real media files** on disk.
//!
//! The tests deliberately work off encoded files rather than in-memory buffers:
//! that is what the app actually opens, so decode, channel extraction, tag
//! reading and the export round trip all get exercised end to end.
//!
//! Layout (under `samples/fixtures/` by default, git-ignored):
//!
//! ```text
//! part-left/   four 24-bit stereo WAVs, isolated part on the LEFT channel
//! part-right/  the same song with the isolated part on the RIGHT (16-bit)
//! flac/        the part-left set as 16-bit FLAC, with vendor-style tags
//! mp3/         the part-left set as MP3 (only with `--features mp3`)
//! reference/   each isolated voice as a mono FLAC — the ground truth
//! misaligned/  the part-left set with a publisher-style lead-in, and the song
//!              pasted in late in three of the four files
//! fixtures.json  manifest: filenames, tags, event/rest frame spans
//! ```
//!
//! Generation is cached: a `.stamp` file records the score version, so repeated
//! `cargo test` runs (and the frontend's vitest suite) reuse the same files.

#![allow(dead_code)] // Each consumer uses a subset.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use audio_core::{encode_interleaved, write_tags, BitDepth, ExportFormat, Tags};

use super::score::{self, Part, PARTS};

/// Bumped whenever the score or the file layout changes, forcing a regenerate.
const STAMP_VERSION: u32 = 3;

pub const ALBUM: &str = "Track Exploder Test Set";
pub const DATE: &str = "2026-01-01";
pub const GENRE: &str = "Barbershop";
pub const COMMENT: &str = "Synthetic learning track generated for Track Exploder tests";

/// Paths to one generated fixture set.
pub struct Fixtures {
    pub root: PathBuf,
    part_left: [PathBuf; 4],
    part_right: [PathBuf; 4],
    flac: [PathBuf; 4],
    mp3: Option<[PathBuf; 4]>,
    reference: [PathBuf; 4],
    misaligned: [PathBuf; 4],
}

impl Fixtures {
    /// 24-bit stereo WAV, isolated part on the left channel.
    pub fn part_left(&self, part: Part) -> &Path {
        &self.part_left[part.index()]
    }

    /// 16-bit stereo WAV, isolated part on the right channel.
    pub fn part_right(&self, part: Part) -> &Path {
        &self.part_right[part.index()]
    }

    /// 16-bit FLAC (part-left layout) carrying vendor-style tags.
    pub fn flac(&self, part: Part) -> &Path {
        &self.flac[part.index()]
    }

    /// MP3 (part-left layout), present only when built with `--features mp3`.
    pub fn mp3(&self, part: Part) -> Option<&Path> {
        self.mp3.as_ref().map(|m| m[part.index()].as_path())
    }

    /// Mono FLAC of the isolated voice — the ground truth for extraction tests.
    pub fn reference(&self, part: Part) -> &Path {
        &self.reference[part.index()]
    }

    /// Publisher-shaped copy with a lead-in and a mis-pasted song — the set the
    /// alignment tests work on. See [`MISALIGNED_EXTRA`].
    pub fn misaligned(&self, part: Part) -> &Path {
        &self.misaligned[part.index()]
    }
}

/// The vendor-style filename for a part, e.g.
/// `01 Circle of Fifths [Bb] - TENOR [Track Exploder] [20260101].wav`.
pub fn track_filename(part: Part, ext: &str) -> String {
    format!(
        "{} - {} [{}] [{}].{ext}",
        score::SONG_BASE,
        part.token(),
        score::VENDOR,
        score::RENDER_DATE
    )
}

/// Tags a vendor would put on the file: everything shared except `artist`,
/// which names the voice.
pub fn tags(part: Part) -> Tags {
    let mut tags = Tags::new();
    tags.insert("album".into(), ALBUM.into());
    tags.insert("title".into(), score::SONG.into());
    tags.insert("artist".into(), part.token().into());
    tags.insert("date".into(), DATE.into());
    tags.insert("genre".into(), GENRE.into());
    tags.insert("comment".into(), COMMENT.into());
    tags
}

/// Surplus silence each part of the `misaligned/` set carries in its lead-in
/// gap, in frames — how much later than the tenor its song starts. The numbers
/// are taken from a real publisher's set (152 / 306 / 260 ms).
pub const MISALIGNED_EXTRA: [usize; 4] = [0, 6688, 13507, 11470];

/// Silence between the pitch pipe and the song in the `misaligned/` set.
const GAP_SECS: f64 = 0.9;

/// The lead-in every learning track starts with: someone announces the song,
/// then a pitch pipe sounds the key, then a gap before the singing. Identical in
/// all four files — which is why sliding a whole file to fix the song would pull
/// this part out of line.
fn lead_in() -> Vec<f32> {
    let sr = score::SAMPLE_RATE as f64;
    let mut out = vec![0.0f32; (0.4 * sr) as usize];

    // "Track seven, I Love Jazz Medley" — a formant-ish burst, not real speech,
    // but the same shape: syllables at speaking pitch.
    for syllable in 0..7 {
        let frames = (0.16 * sr) as usize;
        let hz = 105.0 + (syllable % 3) as f64 * 18.0;
        for i in 0..frames {
            let t = i as f64 / sr;
            let envelope = (std::f64::consts::PI * i as f64 / frames as f64).sin();
            let voice: f64 = [1.0, 0.6, 0.35, 0.2]
                .iter()
                .enumerate()
                .map(|(h, a)| a * (2.0 * std::f64::consts::PI * hz * (h + 1) as f64 * t).sin())
                .sum();
            out.push((0.09 * envelope * voice) as f32);
        }
        out.resize(out.len() + (0.04 * sr) as usize, 0.0);
    }
    out.resize(out.len() + (0.35 * sr) as usize, 0.0);

    // The pitch pipe: the tonic chord, held.
    let pipe_frames = (1.1 * sr) as usize;
    let start = out.len();
    out.resize(start + pipe_frames, 0.0);
    for hz in [116.54, 233.08, 293.66, 349.23] {
        for i in 0..pipe_frames {
            let t = i as f64 / sr;
            let fade = (i as f64 / (0.05 * sr))
                .min(1.0)
                .min((pipe_frames - i) as f64 / (0.2 * sr));
            out[start + i] += (0.07 * fade * (2.0 * std::f64::consts::PI * hz * t).sin()) as f32;
        }
    }
    out.resize(out.len() + (GAP_SECS * sr) as usize, 0.0);
    out
}

/// Where fixtures live: `$TRACK_EXPLODER_FIXTURES`, else `<repo>/samples/fixtures`.
pub fn default_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("TRACK_EXPLODER_FIXTURES") {
        return PathBuf::from(dir);
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples/fixtures")
        .components()
        .collect()
}

fn mp3_enabled() -> bool {
    cfg!(feature = "mp3")
}

fn stamp() -> String {
    format!(
        "track-exploder-fixtures v{STAMP_VERSION} score=circle-of-fifths sr={} frames={} mp3={}",
        score::SAMPLE_RATE,
        score::total_frames(),
        mp3_enabled()
    )
}

/// Generate the fixture set (once per process) and return its paths.
///
/// Reuses an existing set whose `.stamp` matches, so tests don't pay for
/// encoding on every run.
pub fn ensure() -> &'static Fixtures {
    static CELL: OnceLock<Fixtures> = OnceLock::new();
    CELL.get_or_init(|| ensure_at(&default_root()).expect("generate audio fixtures"))
}

/// Generate (or reuse) the fixture set at `root`.
pub fn ensure_at(root: &Path) -> std::io::Result<Fixtures> {
    if !is_current(root) {
        // Build into a sibling temp dir and swap it in, so a half-written set is
        // never visible to a concurrent test run.
        let tmp = root.with_file_name(format!(
            "{}.tmp-{}",
            root.file_name().unwrap_or_default().to_string_lossy(),
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        generate_into(&tmp)?;

        if is_current(root) {
            // Another process beat us to it — keep theirs.
            let _ = std::fs::remove_dir_all(&tmp);
        } else {
            if let Some(parent) = root.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let _ = std::fs::remove_dir_all(root);
            if let Err(e) = std::fs::rename(&tmp, root) {
                let _ = std::fs::remove_dir_all(&tmp);
                if !is_current(root) {
                    return Err(e);
                }
            }
        }
    }
    Ok(paths_at(root))
}

fn is_current(root: &Path) -> bool {
    let matches_stamp = std::fs::read_to_string(root.join(".stamp"))
        .map(|s| s.trim() == stamp())
        .unwrap_or(false);
    if !matches_stamp {
        return false;
    }
    let set = paths_at(root);
    PARTS.iter().all(|&p| {
        set.part_left(p).is_file()
            && set.part_right(p).is_file()
            && set.flac(p).is_file()
            && set.reference(p).is_file()
            && set.misaligned(p).is_file()
            && set.mp3(p).map(|m| m.is_file()).unwrap_or(true)
    })
}

fn paths_at(root: &Path) -> Fixtures {
    let by_part = |dir: &str, ext: &str| -> [PathBuf; 4] {
        std::array::from_fn(|i| root.join(dir).join(track_filename(PARTS[i], ext)))
    };
    Fixtures {
        root: root.to_path_buf(),
        part_left: by_part("part-left", "wav"),
        part_right: by_part("part-right", "wav"),
        flac: by_part("flac", "flac"),
        mp3: mp3_enabled().then(|| by_part("mp3", "mp3")),
        reference: std::array::from_fn(|i| {
            root.join("reference")
                .join(format!("{}.flac", PARTS[i].slug()))
        }),
        misaligned: by_part("misaligned", "wav"),
    }
}

fn write_encoded(
    path: &Path,
    samples: &[f32],
    channels: u16,
    format: ExportFormat,
    depth: BitDepth,
    tags: Option<&Tags>,
) -> std::io::Result<()> {
    let bytes = encode_interleaved(samples, channels, score::SAMPLE_RATE, format, depth)
        .unwrap_or_else(|e| panic!("encode {}: {e}", path.display()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &bytes)?;
    if let Some(tags) = tags {
        write_tags(path, format, tags).unwrap_or_else(|e| panic!("tag {}: {e}", path.display()));
    }
    Ok(())
}

/// Render and write a complete fixture set into `dir`.
pub fn generate_into(dir: &Path) -> std::io::Result<()> {
    let stems = score::render_all();
    let set = paths_at(dir);

    for part in PARTS {
        let isolated = &stems[part.index()];
        let others = score::mix_of_others(&stems, part);

        // Part-left: what nearly every publisher ships (24-bit master quality).
        let left = score::interleave_learning_track(isolated, &others, true);
        write_encoded(
            set.part_left(part),
            &left,
            2,
            ExportFormat::Wav,
            BitDepth::TwentyFour,
            None,
        )?;

        // Part-right: the same song with the channels swapped, for the
        // per-track channel selector.
        let right = score::interleave_learning_track(isolated, &others, false);
        write_encoded(
            set.part_right(part),
            &right,
            2,
            ExportFormat::Wav,
            BitDepth::Sixteen,
            None,
        )?;

        // FLAC carries the tags (WAV tag chunks aren't written by the app).
        write_encoded(
            set.flac(part),
            &left,
            2,
            ExportFormat::Flac,
            BitDepth::Sixteen,
            Some(&tags(part)),
        )?;

        #[cfg(feature = "mp3")]
        if let Some(path) = set.mp3(part) {
            write_encoded(
                path,
                &left,
                2,
                ExportFormat::Mp3,
                BitDepth::Sixteen,
                Some(&tags(part)),
            )?;
        }

        // Ground truth: the isolated voice, mono (FLAC — lossless but small).
        write_encoded(
            set.reference(part),
            isolated,
            1,
            ExportFormat::Flac,
            BitDepth::Sixteen,
            Some(&tags(part)),
        )?;

        // A publisher-shaped copy: spoken title, pitch pipe, a gap — and, in
        // three of the four files, surplus silence in that gap, so the song
        // starts late. This is the defect real learning tracks show up with.
        let lead_in = lead_in();
        let extra = MISALIGNED_EXTRA[part.index()];
        let mut skewed = Vec::with_capacity((lead_in.len() + extra + isolated.len()) * 2);
        for (l, r) in lead_in.iter().zip(&lead_in) {
            skewed.push(*l);
            skewed.push(*r);
        }
        skewed.resize(skewed.len() + extra * 2, 0.0);
        skewed.extend(score::interleave_learning_track(isolated, &others, true));
        write_encoded(
            set.misaligned(part),
            &skewed,
            2,
            ExportFormat::Wav,
            BitDepth::Sixteen,
            None,
        )?;
    }

    std::fs::write(dir.join("fixtures.json"), manifest(dir))?;
    std::fs::write(dir.join("README.txt"), README)?;
    std::fs::write(dir.join(".stamp"), stamp())?;
    Ok(())
}

const README: &str = "\
Generated audio fixtures for Track Exploder's tests (see
crates/audio-core/tests/support/). Everything here is synthetic and safe to
delete -- `cargo test -p audio-core` or
`cargo run -p audio-core --example generate_fixtures` recreates it.

The song is a barbershop arrangement in Bb written to the style rules in the
Barbershop Harmony Society's Music Educator Guide: barbershop sevenths resolving
around the circle of fifths, justly tuned so the chords ring, melody in the lead
with the tenor above it, swipes, a word echo, trio bars, and a tag ending on a
held post -- with rests worked in (breaths, and voices dropping out).

The part-left and part-right sets are shaped like real part-predominant learning
tracks, so they can also be loaded into the app by hand for UI testing.
";

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_object(entries: &[(String, String)]) -> String {
    let body: Vec<String> = entries
        .iter()
        .map(|(k, v)| format!("{}:{}", json_string(k), v))
        .collect();
    format!("{{{}}}", body.join(","))
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// A machine-readable description of the set, so other test suites (notably the
/// frontend's vitest run) can use these files without re-deriving the score.
fn manifest(root: &Path) -> String {
    let set = paths_at(root);
    let files = |f: &dyn Fn(Part) -> Option<String>| -> String {
        json_object(
            &PARTS
                .iter()
                .filter_map(|&p| f(p).map(|v| (p.slug().to_string(), json_string(&v))))
                .collect::<Vec<_>>(),
        )
    };

    let events: Vec<String> = score::event_spans()
        .into_iter()
        .enumerate()
        .map(|(index, (start, end))| {
            let event = &score::SCORE[index];
            let notes: Vec<(String, String)> = PARTS
                .iter()
                .filter_map(|&p| {
                    event.notes[p.index()].map(|n| (p.slug().to_string(), json_string(n)))
                })
                .collect();
            let hz: Vec<(String, String)> = PARTS
                .iter()
                .filter_map(|&p| {
                    score::hz(index, p).map(|f| (p.slug().to_string(), format!("{f:.4}")))
                })
                .collect();
            json_object(&[
                ("label".into(), json_string(event.label)),
                (
                    "lyric".into(),
                    event.lyric.map(json_string).unwrap_or("null".into()),
                ),
                (
                    "root".into(),
                    event.root.map(json_string).unwrap_or("null".into()),
                ),
                ("swipe".into(), event.swipe.to_string()),
                ("beats".into(), format!("{}", event.beats)),
                ("startFrame".into(), start.to_string()),
                ("endFrame".into(), end.to_string()),
                ("notes".into(), json_object(&notes)),
                ("hz".into(), json_object(&hz)),
            ])
        })
        .collect();

    let spans = |list: Vec<(usize, usize)>| -> String {
        let items: Vec<String> = list
            .into_iter()
            .map(|(a, b)| format!("[{a},{b}]"))
            .collect();
        format!("[{}]", items.join(","))
    };

    let rests = json_object(
        &PARTS
            .iter()
            .map(|&p| (p.slug().to_string(), spans(score::rests(p))))
            .collect::<Vec<_>>(),
    );

    json_object(&[
        ("version".into(), STAMP_VERSION.to_string()),
        ("song".into(), json_string(score::SONG)),
        ("songBase".into(), json_string(score::SONG_BASE)),
        ("key".into(), json_string(score::KEY)),
        ("sampleRate".into(), score::SAMPLE_RATE.to_string()),
        ("bpm".into(), format!("{}", score::BPM)),
        ("frames".into(), score::total_frames().to_string()),
        (
            "durationSecs".into(),
            format!("{:.6}", score::duration_secs()),
        ),
        (
            "parts".into(),
            format!(
                "[{}]",
                PARTS
                    .iter()
                    .map(|p| json_string(p.slug()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        ),
        (
            "files".into(),
            json_object(&[
                (
                    "partLeft".into(),
                    files(&|p| Some(rel(root, set.part_left(p)))),
                ),
                (
                    "partRight".into(),
                    files(&|p| Some(rel(root, set.part_right(p)))),
                ),
                ("flac".into(), files(&|p| Some(rel(root, set.flac(p))))),
                ("mp3".into(), files(&|p| set.mp3(p).map(|m| rel(root, m)))),
                (
                    "reference".into(),
                    files(&|p| Some(rel(root, set.reference(p)))),
                ),
                (
                    "misaligned".into(),
                    files(&|p| Some(rel(root, set.misaligned(p)))),
                ),
            ]),
        ),
        (
            "tags".into(),
            json_object(
                &PARTS
                    .iter()
                    .map(|&p| {
                        let t: BTreeMap<String, String> = tags(p);
                        let entries: Vec<(String, String)> =
                            t.into_iter().map(|(k, v)| (k, json_string(&v))).collect();
                        (p.slug().to_string(), json_object(&entries))
                    })
                    .collect::<Vec<_>>(),
            ),
        ),
        (
            "misalignedExtraFrames".into(),
            json_object(
                &PARTS
                    .iter()
                    .map(|&p| {
                        (
                            p.slug().to_string(),
                            MISALIGNED_EXTRA[p.index()].to_string(),
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
        ),
        ("events".into(), format!("[{}]", events.join(","))),
        ("rests".into(), rests),
        ("allRest".into(), spans(score::full_rests())),
    ])
}
