use anyhow::{Context, Result};
use facet::Facet;
use std::path::{Path, PathBuf};

#[derive(Facet, Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub path: String,
    pub recursive: bool,
}

#[derive(Facet, Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub port: u16,
    pub friendly_name: String,
    pub folders: Vec<Folder>,
}

impl Config {
    pub fn with_defaults() -> Self {
        let host = gethostname::gethostname().to_string_lossy().into_owned();
        Self {
            port: 9119,
            friendly_name: format!("dlna-rs ({host})"),
            folders: Vec::new(),
        }
    }

    pub fn default_path() -> PathBuf {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(".config/dlna-rs/config.styx")
    }

    pub fn load_or_default(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => facet_styx::from_str(&s).with_context(|| format!("parse {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::with_defaults()),
            Err(e) => Err(e).with_context(|| format!("read {}", path.display())),
        }
    }

    pub fn save_atomic(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = facet_styx::to_string(self).context("serialize config")?;
        let tmp = path.with_extension("styx.tmp");
        std::fs::write(&tmp, body)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}
