//! Report how far apart the part tracks of each song in a folder play.
//!
//! ```bash
//! cargo run --release -p audio-core --example check_alignment -- "/path/to/album"
//! cargo run --release -p audio-core --example check_alignment -- "/path/to/album" "07 I Love"
//! ```
//!
//! Files are grouped by the name before the ` - PART [` token, the way the app
//! groups them. For each song it prints how much later each part's song starts
//! than the earliest one, the edit that would fix it, and how steady the
//! measurement was through the song.
use std::collections::BTreeMap;
use std::path::Path;

fn main() {
    let folder = std::env::args()
        .nth(1)
        .expect("usage: align_check <folder> [song]");
    let filter = std::env::args().nth(2).unwrap_or_default();
    let mut songs: BTreeMap<String, BTreeMap<String, std::path::PathBuf>> = BTreeMap::new();
    for entry in std::fs::read_dir(&folder).expect("read dir") {
        let path = entry.expect("entry").path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.to_lowercase().ends_with(".mp3") {
            continue;
        }
        for part in ["TENOR", "LEAD", "BARI", "BASS"] {
            if let Some(i) = name.find(&format!(" - {part} [")) {
                songs
                    .entry(name[..i].to_string())
                    .or_default()
                    .insert(part.to_string(), path.clone());
            }
        }
    }

    for (song, files) in songs {
        if !filter.is_empty() && !song.contains(&filter) {
            continue;
        }
        if files.len() < 4 {
            continue;
        }
        let order = ["TENOR", "LEAD", "BARI", "BASS"];
        let mut monos = Vec::new();
        let mut sr = 0;
        for part in order {
            let d = audio_core::decode_file(Path::new(&files[part])).expect("decode");
            sr = d.sample_rate;
            let mut mono = vec![0.0f32; d.frames];
            for ch in &d.planar {
                for (m, s) in mono.iter_mut().zip(ch) {
                    *m += *s;
                }
            }
            monos.push(mono);
        }
        let refs: Vec<&[f32]> = monos.iter().map(|m| m.as_slice()).collect();
        let started = std::time::Instant::now();
        let corrections = audio_core::align_set(&refs, sr);
        let ms = started.elapsed().as_millis();
        let detail: Vec<String> = order
            .iter()
            .zip(&corrections)
            .map(|(p, c)| {
                let ms = |frames: i64| frames as f64 / sr as f64 * 1000.0;
                let edit = if c.delta_frames != 0 {
                    format!(
                        " [{} {:.0}ms @ {:.2}s]",
                        if c.delta_frames < 0 { "cut" } else { "pad" },
                        ms(c.delta_frames.abs()),
                        c.splice_at as f64 / sr as f64
                    )
                } else {
                    String::new()
                };
                let steadiness = if c.consistent {
                    String::new()
                } else {
                    format!(", varies ±{:.0}ms", ms(c.spread_frames))
                };
                format!(
                    "{p} {:+7.1}ms{edit} (r={:.2}{steadiness})",
                    ms(c.offset_frames),
                    c.confidence
                )
            })
            .collect();
        println!(
            "{:<44} [{ms:>4} ms]  {}",
            &song[..song.len().min(44)],
            detail.join("  ")
        );
    }
}
