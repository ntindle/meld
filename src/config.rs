use crate::errors::Result;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Meld configuration. Lives at ~/.meld/config.toml; every field has a
/// sensible default so the tool works with no config file at all.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Parent directory that contains one folder per account.
    pub sessions_root: PathBuf,
    /// Where meld keeps its manifest, backups and lock file.
    pub state_dir: PathBuf,
    /// File-watcher debounce in milliseconds.
    pub debounce_ms: u64,
    /// Keep at most this many snapshots (oldest pruned first).
    pub max_snapshots: usize,
    /// File name patterns to ignore (exact basename match).
    pub ignore: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        let home = home_dir();
        Config {
            sessions_root: home
                .join("Library/Application Support/Claude/claude-code-sessions"),
            state_dir: home.join(".meld"),
            debounce_ms: 1500,
            max_snapshots: 10,
            ignore: vec![".DS_Store".into()],
        }
    }
}

pub fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

pub fn config_path() -> PathBuf {
    home_dir().join(".meld/config.toml")
}

impl Config {
    /// Load from ~/.meld/config.toml, or defaults if absent.
    pub fn load() -> Result<Config> {
        let path = config_path();
        if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("reading config {}", path.display()))?;
            let cfg: Config = toml::from_str(&raw)
                .with_context(|| format!("parsing config {}", path.display()))?;
            Ok(cfg)
        } else {
            Ok(Config::default())
        }
    }

    /// Write the default config to ~/.meld/config.toml (no overwrite).
    pub fn init() -> Result<PathBuf> {
        let path = config_path();
        if path.exists() {
            anyhow::bail!("config already exists at {}", path.display());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let cfg = Config::default();
        std::fs::write(&path, toml::to_string_pretty(&cfg)?)?;
        Ok(path)
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.state_dir.join("manifest.json")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.state_dir.join("backups")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.state_dir.join("meld.lock")
    }

    pub fn is_ignored(&self, path: &Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| self.ignore.iter().any(|i| i == n))
            .unwrap_or(false)
    }
}
