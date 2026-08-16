//! JFOXLink link-quality analyzer.
//!
//! Ingests a series of RSSI (dBm) samples — from stdin (whitespace-separated) or
//! a deterministic demo series — and prints link statistics, an RSSI sparkline,
//! and a quality classification. A ground-side monitor of received-signal health.

use std::io::Read;
use std::process::ExitCode;

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "link_analyzer",
    about = "JFOXLink RF link-quality analyzer",
    version
)]
struct Cli {
    /// Read whitespace-separated RSSI dBm samples from stdin instead of the demo series.
    #[arg(long)]
    stdin: bool,
    /// Number of demo samples to synthesize when not reading stdin.
    #[arg(long, default_value_t = 64)]
    samples: usize,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let rssi = if cli.stdin {
        match read_stdin_samples() {
            Ok(v) if !v.is_empty() => v,
            Ok(_) => {
                eprintln!("no samples on stdin");
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("input error: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        demo_series(cli.samples)
    };

    report(&rssi);
    ExitCode::SUCCESS
}

fn read_stdin_samples() -> Result<Vec<i32>, String> {
    let mut s = String::new();
    std::io::stdin()
        .read_to_string(&mut s)
        .map_err(|e| e.to_string())?;
    s.split_whitespace()
        .map(|t| t.parse::<i32>().map_err(|e| e.to_string()))
        .collect()
}

/// Deterministic demo RSSI series with a mild fade in the middle.
fn demo_series(n: usize) -> Vec<i32> {
    let mut out = Vec::with_capacity(n);
    let mut seed: u32 = 0x1234_5678;
    for i in 0..n {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (seed >> 27) as i32 - 16; // -16..+15
        let fade = if i > n / 3 && i < 2 * n / 3 { -20 } else { 0 };
        out.push(-60 + noise + fade);
    }
    out
}

fn report(rssi: &[i32]) {
    let n = rssi.len() as i64;
    let min = *rssi.iter().min().unwrap();
    let max = *rssi.iter().max().unwrap();
    let avg = (rssi.iter().map(|&x| x as i64).sum::<i64>() / n) as i32;
    // Fraction of samples below the -90 dBm usable-link floor -> effective loss.
    let below = rssi.iter().filter(|&&x| x < -90).count();
    let loss_pct = 100.0 * below as f32 / rssi.len() as f32;

    println!("== JFOXLink Link Quality ==");
    println!("samples : {}", rssi.len());
    println!("RSSI    : min {min} / avg {avg} / max {max} dBm");
    println!("est loss: {loss_pct:.1}% (samples < -90 dBm)");
    println!("quality : {}", classify(avg, loss_pct));
    println!("\nRSSI trace (-30 top .. -100 bottom):");
    println!("{}", sparkline(rssi, -30, -100));
}

fn classify(avg_rssi: i32, loss_pct: f32) -> &'static str {
    if avg_rssi >= -70 && loss_pct < 1.0 {
        "EXCELLENT"
    } else if avg_rssi >= -80 && loss_pct < 5.0 {
        "GOOD"
    } else if avg_rssi >= -88 && loss_pct < 15.0 {
        "MARGINAL"
    } else {
        "POOR / LINK AT RISK"
    }
}

/// Maps each sample to one of eight vertical bar glyphs between `top` and `bottom`.
fn sparkline(rssi: &[i32], top: i32, bottom: i32) -> String {
    const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let span = (top - bottom).max(1);
    rssi.iter()
        .map(|&x| {
            let clamped = x.clamp(bottom, top);
            let idx = ((clamped - bottom) * 7 / span).clamp(0, 7) as usize;
            BARS[idx]
        })
        .collect()
}
