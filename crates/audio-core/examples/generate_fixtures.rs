//! Write the synthetic test song to disk as real media files.
//!
//! ```bash
//! cargo run -p audio-core --example generate_fixtures            # -> samples/fixtures
//! cargo run -p audio-core --example generate_fixtures -- /tmp/te # -> /tmp/te
//! cargo run -p audio-core --example generate_fixtures --features mp3
//! ```
//!
//! `cargo test -p audio-core` generates the same set on demand, so you only need
//! this to refresh the files or to load them into the app for UI testing.

// The tests and this example share one definition of the song.
#[path = "../tests/support/mod.rs"]
mod support;

use support::{fixtures, score};

fn main() {
    let root = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(fixtures::default_root);

    // Always rebuild when invoked explicitly, so editing the score is a one-liner.
    let _ = std::fs::remove_dir_all(&root);
    fixtures::generate_into(&root).expect("write fixtures");

    println!(
        "Wrote {} ({:.1}s at {} Hz, {} events) to {}",
        score::SONG_BASE,
        score::duration_secs(),
        score::SAMPLE_RATE,
        score::SCORE.len(),
        root.display()
    );
    for part in score::PARTS {
        println!(
            "  {:<8} {}",
            part.slug(),
            fixtures::track_filename(part, "wav")
        );
    }
}
