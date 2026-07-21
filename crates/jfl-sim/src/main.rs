//! JFOXLink deterministic RF/threat simulator.
//!
//! Runs a dual-channel link through the RF impairment model and threat injector,
//! driving the real `jfl-core` channel arbitration and jam detector, then reports
//! delivery, corruption, failover, and jam-detection statistics. Seeded, so a
//! given scenario is reproducible.

mod channel_emulator;
mod threat_injector;

use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use jfl_core::anti_jam::detector::JamDetector;
use jfl_core::channel::manager::{ChannelId, DualChannelManager};

use channel_emulator::ChannelEmulator;
use threat_injector::ThreatInjector;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum JamKind {
    None,
    Narrow,
    Wide,
}

#[derive(Parser)]
#[command(name = "jfl-sim", about = "JFOXLink RF/threat simulator", version)]
struct Cli {
    /// Scenario label (informational).
    #[arg(long, default_value = "commercial-high")]
    scenario: String,
    /// RNG seed for reproducibility.
    #[arg(long, default_value_t = 0xC0FFEE)]
    seed: u64,
    /// Number of frames to simulate.
    #[arg(long, default_value_t = 1000)]
    frames: usize,
    /// Per-frame drop probability [0.0, 1.0].
    #[arg(long, default_value_t = 0.02)]
    frame_loss: f32,
    /// Jamming to inject.
    #[arg(long, value_enum, default_value_t = JamKind::None)]
    jam: JamKind,
    /// Link distance in metres (drives path loss / RSSI).
    #[arg(long, default_value_t = 500.0)]
    distance_m: f32,
    /// Jam detector trip threshold (dBm-scaled energy).
    #[arg(long, default_value_t = -85)]
    jam_threshold: i8,
}

#[derive(Default, Debug, PartialEq, Eq)]
struct Stats {
    sent: usize,
    delivered: usize,
    dropped: usize,
    corrupted: usize,
    failovers: usize,
    jam_detections: usize,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let stats = run_scenario(&cli);
    report(&cli, &stats);
    ExitCode::SUCCESS
}

fn run_scenario(cli: &Cli) -> Stats {
    let mut ch_a = ChannelEmulator::new(ChannelId::A, -60, cli.seed);
    let mut ch_b = ChannelEmulator::new(ChannelId::B, -65, cli.seed ^ 0xFFFF);
    ch_a.drop_probability = cli.frame_loss;
    ch_b.drop_probability = cli.frame_loss;

    let mut manager = DualChannelManager::new();
    let mut injector = ThreatInjector::new(cli.seed);
    let mut detector = JamDetector {
        threshold_dbm: cli.jam_threshold,
        spectral_energy: [i16::MIN; 64],
    };

    // Inject jamming once at the start of the run. The detector's energy model
    // uses a positive power scale, so a jammer is represented by a positive
    // relative jam-energy figure.
    let jam_energy = 6.0;
    match cli.jam {
        JamKind::None => {}
        JamKind::Narrow => injector.inject_narrowband_jam(12.5, jam_energy),
        JamKind::Wide => injector.inject_wideband_jam(40.0, jam_energy),
    }

    let mut stats = Stats::default();
    let mut active = ChannelId::A;

    for i in 0..cli.frames {
        stats.sent += 1;

        // A representative frame (content is opaque to the RF layer).
        let frame = injector.generate_spoof_frame(b"TELEMETRY", 0x01, (i & 0xFF) as u8);

        ch_a.update_environment(cli.distance_m, 2.0);
        ch_b.update_environment(cli.distance_m, 2.0);

        let out_a = ch_a.apply_impairments(&frame).unwrap_or(None);
        let out_b = ch_b.apply_impairments(&frame).unwrap_or(None);

        // Health-scored arbitration between the two channels.
        let chosen = manager.arbitrate(ch_a.health(), ch_b.health());
        if chosen != active {
            stats.failovers += 1;
            active = chosen;
        }

        let received = match active {
            ChannelId::A => out_a,
            ChannelId::B => out_b,
        };
        match received {
            None => stats.dropped += 1,
            Some(bytes) => {
                stats.delivered += 1;
                if bytes != frame {
                    stats.corrupted += 1;
                }
            }
        }

        // Jam detection each tick.
        if cli.jam != JamKind::None {
            injector.apply_to_detector(&mut detector);
            if detector.evaluate() {
                stats.jam_detections += 1;
            }
        }
    }

    stats
}

fn report(cli: &Cli, s: &Stats) {
    let pct = |n: usize| {
        if s.sent > 0 {
            100.0 * n as f32 / s.sent as f32
        } else {
            0.0
        }
    };
    println!("== JFOXLink simulation: {} ==", cli.scenario);
    println!(
        "seed={}  frames={}  distance={}m  jam={}",
        cli.seed,
        cli.frames,
        cli.distance_m,
        jam_label(cli.jam)
    );
    println!("  sent           : {}", s.sent);
    println!(
        "  delivered      : {} ({:.1}%)",
        s.delivered,
        pct(s.delivered)
    );
    println!("  dropped        : {} ({:.1}%)", s.dropped, pct(s.dropped));
    println!(
        "  corrupted      : {} ({:.1}%)",
        s.corrupted,
        pct(s.corrupted)
    );
    println!("  failovers      : {}", s.failovers);
    println!("  jam detections : {}", s.jam_detections);
}

fn jam_label(k: JamKind) -> &'static str {
    match k {
        JamKind::None => "none",
        JamKind::Narrow => "narrowband",
        JamKind::Wide => "wideband",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_cli() -> Cli {
        Cli {
            scenario: "test".into(),
            seed: 42,
            frames: 500,
            frame_loss: 0.0,
            jam: JamKind::None,
            distance_m: 300.0,
            jam_threshold: -85,
        }
    }

    #[test]
    fn no_loss_delivers_everything() {
        let s = run_scenario(&base_cli());
        assert_eq!(s.sent, 500);
        assert_eq!(s.dropped, 0);
        assert_eq!(s.delivered, 500);
    }

    #[test]
    fn deterministic_for_fixed_seed() {
        let a = run_scenario(&base_cli());
        let b = run_scenario(&base_cli());
        assert_eq!(a, b);
    }

    #[test]
    fn frame_loss_drops_some_frames() {
        let mut cli = base_cli();
        cli.frame_loss = 0.5;
        let s = run_scenario(&cli);
        assert!(s.dropped > 0, "expected some drops at 50% loss");
    }

    #[test]
    fn jamming_is_detected() {
        let mut cli = base_cli();
        cli.jam = JamKind::Wide;
        cli.jam_threshold = -120; // low threshold -> jam energy trips it
        let s = run_scenario(&cli);
        assert!(s.jam_detections > 0, "wideband jam should be detected");
    }
}
