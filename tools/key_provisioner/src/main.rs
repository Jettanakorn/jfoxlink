//! JFOXLink pre-flight key provisioner.
//!
//! Generates ephemeral ECDH key pairs and derives session keys, using the same
//! `jfl-core` key-agreement path the runtime uses, and writes the pre-shared
//! keys that authenticate a peer pair's handshake (`crypto::handshake`). Key
//! material is never printed — only short fingerprints — so the tool is safe
//! to run in logs.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use jfl_core::crypto::ecdh::EcdhSession;
use jfl_core::crypto::handshake::{Psk, PSK_LEN};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};

#[derive(Parser)]
#[command(
    name = "key_provisioner",
    about = "JFOXLink ECDH key provisioner",
    version
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate an ephemeral key pair and print the public key (hex).
    Pubkey,
    /// Run a full two-party ECDH handshake in-process and show that both sides
    /// derive identical session keys (by fingerprint).
    Handshake {
        /// HKDF context string both peers must share.
        #[arg(long, default_value = "jfl-session")]
        info: String,
    },
    /// Write a new pre-shared key for one peer pair (PSK-bound ECDH
    /// handshake). Copy the file to both peers; it is never printed.
    Psk {
        /// Where to write it, e.g. `sim-to-studio.psk`. Refuses to overwrite.
        path: PathBuf,
        /// A note stored as a comment in the file (who the pair is).
        #[arg(long, default_value = "")]
        label: String,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("key_provisioner error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.cmd {
        Cmd::Pubkey => {
            let (_secret, public) =
                EcdhSession::generate_ephemeral(&mut OsRng).map_err(|e| format!("{e:?}"))?;
            println!("Ephemeral public key ({} bytes):", public.len());
            println!("{}", to_hex(&public));
            println!("(the matching secret is held in memory only for this process)");
            Ok(())
        }
        Cmd::Handshake { info } => {
            let salt = b"jfl-provision-salt";
            // GCS side.
            let (sec_g, pub_g) =
                EcdhSession::generate_ephemeral(&mut OsRng).map_err(|e| format!("{e:?}"))?;
            // Aircraft side.
            let (sec_a, pub_a) =
                EcdhSession::generate_ephemeral(&mut OsRng).map_err(|e| format!("{e:?}"))?;

            let keys_g = EcdhSession::derive_session_keys(&sec_g, &pub_a, salt, info.as_bytes())
                .map_err(|e| format!("{e:?}"))?;
            let keys_a = EcdhSession::derive_session_keys(&sec_a, &pub_g, salt, info.as_bytes())
                .map_err(|e| format!("{e:?}"))?;

            println!(
                "GCS      aes fp {}  hmac fp {}",
                fp(&keys_g.aes_key),
                fp(&keys_g.hmac_key)
            );
            println!(
                "Aircraft aes fp {}  hmac fp {}",
                fp(&keys_a.aes_key),
                fp(&keys_a.hmac_key)
            );

            if keys_g.aes_key == keys_a.aes_key && keys_g.hmac_key == keys_a.hmac_key {
                println!("[ok] both peers derived identical session keys");
                Ok(())
            } else {
                Err("key agreement mismatch".into())
            }
        }
        Cmd::Psk { path, label } => {
            if path.exists() {
                return Err(format!(
                    "{} exists; a PSK is never overwritten (remove it first)",
                    path.display()
                ));
            }
            let mut key = [0u8; PSK_LEN];
            OsRng.fill_bytes(&mut key);
            let text = format!(
                "# JFOXLink pre-shared key (handshake v1). Keep private; copy to both peers.
                 # {label}
{}
",
                to_hex(&key)
            );
            // Round-trip through the parser the peers use, so a file this
            // tool wrote is one they can read.
            Psk::from_hex(&text).map_err(|e| format!("self-check failed: {e:?}"))?;
            write_private(&path, &text)?;
            println!("wrote {}  fingerprint {}", path.display(), fp(&key));
            key.iter_mut().for_each(|b| *b = 0);
            Ok(())
        }
    }
}

/// Create the file readable by the owner only (where the OS has modes).
fn write_private(path: &PathBuf, text: &str) -> Result<(), String> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(text.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// SHA-256 fingerprint (first 8 bytes) of a key — safe to display.
fn fp(key: &[u8; 32]) -> String {
    let digest = Sha256::digest(key);
    to_hex(&digest[..8])
}

fn to_hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for &x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}
