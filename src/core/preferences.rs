//! Machine-local display preferences. Task data always remains in the `.antodo` file.
use super::{Result, invalid};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub const MIN_FONT_SIZE: u16 = 12;
pub const MAX_FONT_SIZE: u16 = 28;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub last_file: Option<PathBuf>,
    pub dark_mode: bool,
    pub font_size: u16,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            last_file: None,
            dark_mode: true,
            font_size: 16,
        }
    }
}

impl Preferences {
    pub fn path() -> Result<PathBuf> {
        directories::ProjectDirs::from("io", "tothaa", "antodo")
            .map(|dirs| dirs.config_dir().join("preferences.json"))
            .ok_or_else(|| invalid("Could not locate your configuration directory"))
    }

    pub fn load() -> Result<Self> {
        Self::load_from(&Self::path()?)
    }
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path()?)
    }

    /// Missing preferences use defaults; corrupt preferences return an explicit error.
    pub fn load_from(path: &Path) -> Result<Self> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(error.into()),
        };
        let mut prefs: Self = serde_json::from_slice(&bytes)?;
        prefs.font_size = prefs.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        Ok(prefs)
    }

    /// Write and sync a sibling temporary file before replacing the old preferences.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if !(MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&self.font_size) {
            return Err(invalid(format!(
                "Font size must be {MIN_FONT_SIZE}–{MAX_FONT_SIZE}"
            )));
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temporary, self)?;
        temporary.write_all(b"\n")?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
}
