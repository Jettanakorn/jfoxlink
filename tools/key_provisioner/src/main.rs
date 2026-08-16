//! JFOXLink pre-flight key provisioner.
//!
//! Generates ephemeral ECDH key pairs and derives session keys, using the same
//! `jfl-core` key-agreement path the runtime uses. Key material is never
//! printed — only short fingerprints — so the tool is safe to run in logs.

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use jfl_core::crypto::ecdh::EcdhSession;
use rand::rngs::OsRng;
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
    }
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
