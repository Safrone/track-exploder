//! Print the normalized tags Track Exploder reads from input files.
//!
//! Usage: cargo run -p audio-core --example dump_tags -- FILE [FILE ...]

use std::path::Path;

use audio_core::read_tags;

fn main() {
    for path in std::env::args().skip(1) {
        match read_tags(Path::new(&path)) {
            Ok(tags) => {
                println!("{path}");
                if tags.is_empty() {
                    println!("  (no tags)");
                }
                for (k, v) in tags {
                    println!("  {k} = {v}");
                }
            }
            Err(e) => eprintln!("{path}: {e}"),
        }
    }
}
