use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::models::{EmulatorKind, Platform};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub downloads_dir: PathBuf,
    pub db_path: PathBuf,
    pub config_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulatorPreference {
    pub platform: Platform,
    pub emulator: EmulatorKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub rom_roots: Vec<PathBuf>,
    pub managed_download_dir: PathBuf,
    pub scan_on_startup: bool,
    pub show_hidden_files: bool,
    pub preferred_emulators: Vec<EmulatorPreference>,
}

impl Config {
    pub fn load_or_create() -> Result<(Self, AppPaths)> {
        let dirs = ProjectDirs::from("dev", "retrolauncher", "retro-launcher")
            .context("unable to resolve user config directories")?;
        let paths = AppPaths {
            config_dir: dirs.config_dir().to_path_buf(),
            data_dir: dirs.data_dir().to_path_buf(),
            downloads_dir: dirs.data_dir().join("downloads"),
            db_path: dirs.data_dir().join("library.sqlite3"),
            config_path: dirs.config_dir().join("config.toml"),
        };

        fs::create_dir_all(&paths.config_dir)?;
        fs::create_dir_all(&paths.data_dir)?;
        fs::create_dir_all(&paths.downloads_dir)?;

        if paths.config_path.exists() {
            let raw = fs::read_to_string(&paths.config_path)?;
            let mut config: Self = toml::from_str(&raw)?;
            if config.managed_download_dir.as_os_str().is_empty() {
                config.managed_download_dir = paths.downloads_dir.clone();
            }
            fs::create_dir_all(&config.managed_download_dir)?;
            Ok((config, paths))
        } else {
            let config = Self::default_with_paths(&paths);
            let rendered = toml::to_string_pretty(&config)?;
            fs::write(&paths.config_path, rendered)?;
            Ok((config, paths))
        }
    }

    fn default_with_paths(paths: &AppPaths) -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let rom_roots = vec![
            home.join("ROMs"),
            home.join("Games").join("ROMs"),
            home.join("Downloads").join("ROMs"),
            paths.downloads_dir.clone(),
        ];
        Self {
            rom_roots,
            managed_download_dir: paths.downloads_dir.clone(),
            scan_on_startup: true,
            show_hidden_files: false,
            preferred_emulators: vec![
                EmulatorPreference {
                    platform: Platform::GameBoy,
                    emulator: EmulatorKind::Mgba,
                },
                EmulatorPreference {
                    platform: Platform::GameBoyColor,
                    emulator: EmulatorKind::Mgba,
                },
                EmulatorPreference {
                    platform: Platform::GameBoyAdvance,
                    emulator: EmulatorKind::Mgba,
                },
                EmulatorPreference {
                    platform: Platform::Ps1,
                    emulator: EmulatorKind::Mednafen,
                },
                EmulatorPreference {
                    platform: Platform::Nes,
                    emulator: EmulatorKind::Fceux,
                },
            ],
        }
    }

    pub fn preferred_emulators_for(&self, platform: Platform) -> Vec<EmulatorKind> {
        self.preferred_emulators
            .iter()
            .filter(|entry| entry.platform == platform)
            .map(|entry| entry.emulator)
            .collect()
    }
}
