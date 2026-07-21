//! Minimal loader for the `config/*.toml` profile files.
//!
//! The profiles are flat `key = value` tables under `[profile]`, so we parse
//! them directly rather than pull in a full TOML dependency.

use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct Profile {
    pub name: String,
    pub crypto_suite: String,
    pub channel_a: String,
    pub channel_b: String,
    pub anti_jam: String,
    pub cert_target: String,
    pub replay_window: u64,
    pub key_rotation_s: u64,
    pub jam_threshold_dbm: i32,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    MissingField(&'static str),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "config I/O error: {e}"),
            ConfigError::MissingField(k) => write!(f, "config missing required field: {k}"),
        }
    }
}

impl Profile {
    /// Load and parse a profile TOML. Numeric fields default to the safe
    /// values used by `defense-full` when a profile omits them.
    pub fn load(path: &Path) -> Result<Profile, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
        let mut p = Profile {
            replay_window: 64,
            key_rotation_s: 3600,
            jam_threshold_dbm: -85,
            ..Default::default()
        };
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                continue;
            }
            let Some((key, val)) = line.split_once('=') else { continue };
            let key = key.trim();
            let val = val.trim().trim_matches('"');
            match key {
                "name" => p.name = val.to_string(),
                "crypto_suite" => p.crypto_suite = val.to_string(),
                "channel_a" => p.channel_a = val.to_string(),
                "channel_b" => p.channel_b = val.to_string(),
                "anti_jam" => p.anti_jam = val.to_string(),
                "cert_target" => p.cert_target = val.to_string(),
                "replay_window" => p.replay_window = val.parse().unwrap_or(p.replay_window),
                "key_rotation_s" => p.key_rotation_s = val.parse().unwrap_or(p.key_rotation_s),
                "jam_threshold_dbm" => {
                    p.jam_threshold_dbm = val.parse().unwrap_or(p.jam_threshold_dbm)
                }
                _ => {}
            }
        }
        if p.name.is_empty() {
            return Err(ConfigError::MissingField("name"));
        }
        if p.crypto_suite.is_empty() {
            return Err(ConfigError::MissingField("crypto_suite"));
        }
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_profile_with_defaults() {
        let mut f = std::env::temp_dir();
        f.push("jfl-cfg-test.toml");
        let mut fh = std::fs::File::create(&f).unwrap();
        writeln!(fh, "[profile]").unwrap();
        writeln!(fh, "name = \"TEST\"").unwrap();
        writeln!(fh, "crypto_suite = \"AES-256-GCM\"").unwrap();
        writeln!(fh, "replay_window = 128").unwrap();
        drop(fh);

        let p = Profile::load(&f).unwrap();
        assert_eq!(p.name, "TEST");
        assert_eq!(p.crypto_suite, "AES-256-GCM");
        assert_eq!(p.replay_window, 128);
        assert_eq!(p.jam_threshold_dbm, -85); // default applied
        let _ = std::fs::remove_file(&f);
    }
}
