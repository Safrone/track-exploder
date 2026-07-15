//! End-to-end test of the core conceit: build synthetic "part-left" tracks the
//! way barbershop learning-track publishers do, then confirm that decoding +
//! extracting the isolated channel recovers each original part cleanly.

use std::path::PathBuf;

use audio_core::{
    decode_file, encode_interleaved, extract_channel, BitDepth, Channel, ExportFormat,
};

const SAMPLE_RATE: u32 = 44_100;
const FRAMES: usize = 8_192;

/// Four distinct mono "voices" (tenor, lead, bari, bass) as sine waves.
fn make_parts() -> Vec<Vec<f32>> {
    let freqs = [440.0f32, 330.0, 220.0, 110.0];
    freqs
        .iter()
        .map(|&f| {
            (0..FRAMES)
                .map(|i| {
                    let t = i as f32 / SAMPLE_RATE as f32;
                    0.2 * (2.0 * std::f32::consts::PI * f * t).sin()
                })
                .collect()
        })
        .collect()
}

/// Interleave a part-left file: L = isolated part, R = sum of the other three.
fn interleave_part_left(isolated: &[f32], others: [&Vec<f32>; 3]) -> Vec<f32> {
    let mut out = Vec::with_capacity(isolated.len() * 2);
    for i in 0..isolated.len() {
        let right = others[0][i] + others[1][i] + others[2][i];
        out.push(isolated[i]);
        out.push(right);
    }
    out
}

fn temp_path(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "track_exploder_test_{}_{}.wav",
        std::process::id(),
        name
    ));
    p
}

#[test]
fn extracts_each_part_from_synthetic_part_left_tracks() {
    let parts = make_parts();
    let names = ["tenor", "lead", "bari", "bass"];

    for p in 0..4 {
        let others: Vec<&Vec<f32>> = (0..4).filter(|&j| j != p).map(|j| &parts[j]).collect();
        let interleaved = interleave_part_left(&parts[p], [others[0], others[1], others[2]]);

        // Encode a 24-bit stereo WAV, write it, then decode it back.
        let bytes = encode_interleaved(
            &interleaved,
            2,
            SAMPLE_RATE,
            ExportFormat::Wav,
            BitDepth::TwentyFour,
        )
        .expect("encode part-left wav");

        let path = temp_path(names[p]);
        std::fs::write(&path, &bytes).expect("write temp wav");

        let decoded = decode_file(&path).expect("decode wav");
        std::fs::remove_file(&path).ok();

        assert_eq!(decoded.channels, 2);
        assert_eq!(decoded.sample_rate, SAMPLE_RATE);

        let extracted = extract_channel(&decoded, Channel::Left);
        assert_eq!(extracted.len(), parts[p].len());

        // 24-bit quantization error is ~1.2e-7; allow generous slack.
        let mut max_err = 0.0f32;
        for (a, b) in extracted.iter().zip(parts[p].iter()) {
            max_err = max_err.max((a - b).abs());
        }
        assert!(
            max_err < 1e-4,
            "part {} (channel Left) diverged: max_err={max_err}",
            names[p]
        );
    }
}

#[test]
fn flac_roundtrip_recovers_signal() {
    let parts = make_parts();
    // Mono FLAC round trip on the first part.
    let bytes = encode_interleaved(
        &parts[0],
        1,
        SAMPLE_RATE,
        ExportFormat::Flac,
        BitDepth::TwentyFour,
    )
    .expect("encode flac");

    let mut path = std::env::temp_dir();
    path.push(format!(
        "track_exploder_test_{}_flac.flac",
        std::process::id()
    ));
    std::fs::write(&path, &bytes).expect("write flac");

    let decoded = decode_file(&path).expect("decode flac");
    std::fs::remove_file(&path).ok();

    let recovered = extract_channel(&decoded, Channel::Left);
    assert_eq!(recovered.len(), parts[0].len());

    let mut max_err = 0.0f32;
    for (a, b) in recovered.iter().zip(parts[0].iter()) {
        max_err = max_err.max((a - b).abs());
    }
    assert!(
        max_err < 1e-4,
        "flac round trip diverged: max_err={max_err}"
    );
}
