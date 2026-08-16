//! JFOXLink Ground Control Station.
//!
//! Loads a radio profile, provisions session keys, and either runs an
//! end-to-end self-test of the secure receive path or decodes captured frames.

mod config;
mod decoder;
mod key_store;
mod tx;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use jfl_core::anti_jam::fhss::{HopKey, KeyedHopSchedule};

use config::Profile;
use decoder::GcsDecoder;
use key_store::KeyStore;
use tx::FrameTx;

/// Hop-channel count used by the self-test (design target for the FHSS band).
const FHSS_CHANNELS: u32 = 100;

#[derive(Parser)]
#[command(name = "jfl-gcs", about = "JFOXLink Ground Control Station", version)]
struct Cli {
    /// Path to a radio profile (config/*.toml).
    #[arg(long, default_value = "config/commercial-high.toml")]
    config: PathBuf,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Load and print the selected profile.
    Profile,
    /// Provision keys via ECDH and run an encrypt -> decode self-test.
    Selftest,
    /// Decode a raw frame file using hex-encoded session keys.
    Decode {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        aes_hex: String,
        #[arg(long)]
        hmac_hex: String,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("jfl-gcs error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.cmd {
        Cmd::Profile => {
            let p = Profile::load(&cli.config).map_err(|e| e.to_string())?;
            print_profile(&p);
            Ok(())
        }
        Cmd::Selftest => {
            let p = Profile::load(&cli.config).map_err(|e| e.to_string())?;
            print_profile(&p);
            selftest(&p)
        }
        Cmd::Decode {
            file,
            aes_hex,
            hmac_hex,
        } => {
            let p = Profile::load(&cli.config).map_err(|e| e.to_string())?;
            let aes = parse_key32(&aes_hex)?;
            let hmac = parse_key32(&hmac_hex)?;
            let raw = std::fs::read(&file).map_err(|e| e.to_string())?;
            let mut dec = GcsDecoder::new(&aes, &hmac, p.replay_window);
            match dec.decode_frame(&raw) {
                Ok(msg) => {
                    println!(
                        "Decoded frame: sysid={} compid={} msgid={:02x?}",
                        msg.sysid, msg.compid, msg.msgid
                    );
                    println!(
                        "Payload ({} bytes): {}",
                        msg.payload.len(),
                        to_hex(&msg.payload)
                    );
                    Ok(())
                }
                Err(e) => Err(format!("decode failed: {e:?}")),
            }
        }
    }
}

fn selftest(p: &Profile) -> Result<(), String> {
    println!("\n== Secure link self-test ==");

    // Establish a shared session via a full ECDH handshake between two key
    // stores (stands in for GCS <-> aircraft provisioning).
    let gcs = KeyStore::new(std::env::temp_dir().join("jfl-gcs-selftest-a.bin"));
    let uav = KeyStore::new(std::env::temp_dir().join("jfl-gcs-selftest-b.bin"));
    let pk_gcs = gcs.begin_handshake().map_err(|e| format!("{e:?}"))?;
    let pk_uav = uav.begin_handshake().map_err(|e| format!("{e:?}"))?;
    gcs.complete_handshake(&pk_uav, b"jfl-salt", b"jfl-session")
        .map_err(|e| format!("{e:?}"))?;
    uav.complete_handshake(&pk_gcs, b"jfl-salt", b"jfl-session")
        .map_err(|e| format!("{e:?}"))?;
    let keys = gcs.load_keys().map_err(|e| format!("{e:?}"))?;
    println!(
        "[ok] ECDH session established (aes fp {}, hmac fp {}, hop fp {})",
        fp(&keys.aes_key),
        fp(&keys.hmac_key),
        fp(&keys.hop_key)
    );

    // Both sides derive the keyed FHSS hop schedule from the session hop key
    // and must agree on every channel from a shared slot counter.
    let uav_keys = uav.load_keys().map_err(|e| format!("{e:?}"))?;
    let mut hop_gcs =
        KeyedHopSchedule::new(HopKey(keys.hop_key), FHSS_CHANNELS).map_err(|e| format!("{e:?}"))?;
    let mut hop_uav = KeyedHopSchedule::new(HopKey(uav_keys.hop_key), FHSS_CHANNELS)
        .map_err(|e| format!("{e:?}"))?;
    let mut first_hops = [0u32; 8];
    for (i, h) in first_hops.iter_mut().enumerate() {
        let (a, b) = (hop_gcs.next_channel(), hop_uav.next_channel());
        if a != b {
            return Err(format!("hop schedule diverged at slot {i}: {a} vs {b}"));
        }
        *h = a;
    }
    // Simulate a UAV dropout and GPS-time resync.
    for _ in 0..1_000 {
        hop_gcs.next_channel();
    }
    hop_uav.sync_to(hop_gcs.current_slot());
    if hop_gcs.next_channel() != hop_uav.next_channel() {
        return Err("hop schedule failed to resync".into());
    }
    println!("[ok] Keyed FHSS schedule agreed ({FHSS_CHANNELS} ch), first hops {first_hops:?}, resync ok");

    // UAV side encrypts a telemetry frame; GCS side decodes it.
    let mut tx = FrameTx::new(&keys.aes_key, &keys.hmac_key, keys.nonce_prefix);
    let payload = b"HEARTBEAT:alt=120,batt=87%";
    let frame = tx
        .encode(0x01, 0x01, 0x01, [0x00, 0x00, 0x00], payload)
        .map_err(|e| format!("{e:?}"))?;
    println!("[ok] Encrypted frame built: {} bytes", frame.len());

    let mut dec = GcsDecoder::new(&keys.aes_key, &keys.hmac_key, p.replay_window);
    let msg = dec
        .decode_frame(&frame)
        .map_err(|e| format!("decode: {e:?}"))?;
    let text = core::str::from_utf8(&msg.payload).unwrap_or("<binary>");
    println!("[ok] Decoded payload: \"{text}\"");
    if msg.payload.as_slice() != payload {
        return Err("payload mismatch".into());
    }

    // Replaying the same frame must be rejected.
    match dec.decode_frame(&frame) {
        Err(decoder::GcsDecodeError::ReplayDetected) => {
            println!("[ok] Replay of the same frame correctly rejected");
        }
        Err(e) => return Err(format!("replay produced unexpected error: {e:?}")),
        Ok(_) => return Err("replay was NOT rejected — frame decoded twice".into()),
    }

    println!("== self-test PASSED ==");
    Ok(())
}

fn print_profile(p: &Profile) {
    println!("Profile: {}", p.name);
    println!("  crypto_suite : {}", p.crypto_suite);
    println!("  channel_a    : {}", p.channel_a);
    println!("  channel_b    : {}", p.channel_b);
    println!("  anti_jam     : {}", p.anti_jam);
    println!("  cert_target  : {}", p.cert_target);
    println!("  replay_window: {}", p.replay_window);
    println!("  key_rotation : {}s", p.key_rotation_s);
    println!("  jam_threshold: {} dBm", p.jam_threshold_dbm);
}

/// 4-byte hex fingerprint of a key (never prints the key itself).
fn fp(key: &[u8; 32]) -> String {
    let mut acc: u32 = 0x811c_9dc5;
    for &b in key {
        acc = (acc ^ b as u32).wrapping_mul(0x0100_0193);
    }
    format!("{acc:08x}")
}

fn to_hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for &x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return Err("hex string has odd length".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn parse_key32(hex: &str) -> Result<[u8; 32], String> {
    let bytes = from_hex(hex)?;
    if bytes.len() != 32 {
        return Err(format!("expected 32-byte key, got {}", bytes.len()));
    }
    let mut k = [0u8; 32];
    k.copy_from_slice(&bytes);
    Ok(k)
}
