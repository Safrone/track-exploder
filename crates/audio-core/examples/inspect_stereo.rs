//! Diagnose the stereo layout of learning tracks.
//!
//! Usage:
//!   cargo run -p audio-core --example inspect_stereo -- FILE [FILE ...]
//!
//! For each file it prints per-channel RMS and the left/right correlation. If
//! one of the files has "MIX" in its name, every other file's `L+R` sum is
//! correlated against the mix to check the "part-left" hypothesis: one part on
//! one channel, the other three summed on the opposite channel, so that
//! `L + R` reconstructs the full quartet.

use std::path::Path;

use audio_core::decode_file;

fn rms(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

fn corr(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let (a, b) = (&a[..n], &b[..n]);
    let ma = a.iter().sum::<f32>() / n as f32;
    let mb = b.iter().sum::<f32>() / n as f32;
    let mut num = 0.0f64;
    let mut da = 0.0f64;
    let mut db = 0.0f64;
    for i in 0..n {
        let x = (a[i] - ma) as f64;
        let y = (b[i] - mb) as f64;
        num += x * y;
        da += x * x;
        db += y * y;
    }
    (num / (da.sqrt() * db.sqrt() + 1e-12)) as f32
}

fn sum_lr(l: &[f32], r: &[f32]) -> Vec<f32> {
    let n = l.len().min(r.len());
    (0..n).map(|i| l[i] + r[i]).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: inspect_stereo FILE [FILE ...]");
        std::process::exit(2);
    }

    struct Loaded {
        name: String,
        l: Vec<f32>,
        r: Vec<f32>,
    }
    let mut loaded: Vec<Loaded> = Vec::new();

    for path in &args {
        let name = Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(path)
            .to_string();
        match decode_file(Path::new(path)) {
            Ok(d) => {
                let l = d.planar.first().cloned().unwrap_or_default();
                let r = d.planar.get(1).cloned().unwrap_or_else(|| l.clone());
                println!(
                    "{name}\n  ch={} sr={} frames={} rms_L={:.4} rms_R={:.4} corr_LR={:.3}",
                    d.channels,
                    d.sample_rate,
                    d.frames,
                    rms(&l),
                    rms(&r),
                    corr(&l, &r),
                );
                loaded.push(Loaded { name, l, r });
            }
            Err(e) => eprintln!("{name}: decode failed: {e}"),
        }
    }

    if let Some(mix) = loaded
        .iter()
        .find(|f| f.name.to_uppercase().contains("MIX"))
    {
        let mix_sum = sum_lr(&mix.l, &mix.r);
        println!("\nReference mix: {}", mix.name);
        println!("  (corr of each file's L+R, L, and R against the mix's L+R)");
        for f in loaded.iter().filter(|f| f.name != mix.name) {
            let lr = sum_lr(&f.l, &f.r);
            println!(
                "  {:<52} corr(L+R,mix)={:.3}  corr(L,mix)={:.3}  corr(R,mix)={:.3}",
                f.name,
                corr(&lr, &mix_sum),
                corr(&f.l, &mix_sum),
                corr(&f.r, &mix_sum),
            );
        }
    }
}
