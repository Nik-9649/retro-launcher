use std::cmp::Ordering;
use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Platform {
    GameBoy,
    GameBoyColor,
    GameBoyAdvance,
    Ps1,
    NintendoDs,
    Ps2,
    SegaGenesis,
    Nes,
    Snes,
    N64,
    Wii,
    Xbox360,
    Unknown,
}

impl Platform {
    pub fn short_label(self) -> &'static str {
        match self {
            Self::GameBoy => "GB",
            Self::GameBoyColor => "GBC",
            Self::GameBoyAdvance => "GBA",
            Self::Ps1 => "PS1",
            Self::NintendoDs => "NDS",
            Self::Ps2 => "PS2",
            Self::SegaGenesis => "GEN",
            Self::Nes => "NES",
            Self::Snes => "SNES",
            Self::N64 => "N64",
            Self::Wii => "WII",
            Self::Xbox360 => "X360",
            Self::Unknown => "UNK",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::GameBoy => "Game Boy",
            Self::GameBoyColor => "Game Boy Color",
            Self::GameBoyAdvance => "Game Boy Advance",
            Self::Ps1 => "PlayStation 1",
            Self::NintendoDs => "Nintendo DS",
            Self::Ps2 => "PlayStation 2",
            Self::SegaGenesis => "SEGA Genesis",
            Self::Nes => "NES",
            Self::Snes => "SNES",
            Self::N64 => "Nintendo 64",
            Self::Wii => "Wii",
            Self::Xbox360 => "Xbox 360",
            Self::Unknown => "Unknown",
        }
    }

    pub fn from_extension(ext: &str) -> Self {
        match ext.to_ascii_lowercase().as_str() {
            "gb" => Self::GameBoy,
            "gbc" => Self::GameBoyColor,
            "gba" => Self::GameBoyAdvance,
            "cue" | "chd" | "m3u" | "bin" | "img" | "iso" => Self::Ps1,
            "nds" => Self::NintendoDs,
            "nes" => Self::Nes,
            "sfc" | "smc" => Self::Snes,
            "gen" | "md" | "smd" => Self::SegaGenesis,
            "n64" | "z64" | "v64" => Self::N64,
            "iso.ps2" => Self::Ps2,
            _ => Self::Unknown,
        }
    }

    pub fn generation(self) -> GenerationTag {
        match self {
            Self::Nes | Self::SegaGenesis | Self::Snes => GenerationTag::GenX,
            Self::GameBoy
            | Self::GameBoyColor
            | Self::GameBoyAdvance
            | Self::Ps1
            | Self::Ps2
            | Self::N64 => GenerationTag::Millennials,
            Self::NintendoDs | Self::Wii | Self::Xbox360 => GenerationTag::GenZ,
            Self::Unknown => GenerationTag::Millennials,
        }
    }

    pub fn default_vibes(self) -> Vec<VibeTag> {
        match self {
            Self::GameBoy | Self::GameBoyColor | Self::GameBoyAdvance => {
                vec![VibeTag::Tactile, VibeTag::Simple]
            }
            Self::Ps1 | Self::Ps2 => vec![VibeTag::Social, VibeTag::CouchCoOp],
            Self::Nes | Self::Snes | Self::SegaGenesis | Self::N64 => {
                vec![VibeTag::CouchCoOp, VibeTag::Simple]
            }
            Self::NintendoDs => vec![VibeTag::Tactile, VibeTag::Social],
            Self::Wii | Self::Xbox360 => vec![VibeTag::Social, VibeTag::CouchCoOp],
            Self::Unknown => vec![VibeTag::Simple],
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenerationTag {
    GenX,
    Millennials,
    GenZ,
}

impl fmt::Display for GenerationTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GenX => f.write_str("Gen X"),
            Self::Millennials => f.write_str("Millennials"),
            Self::GenZ => f.write_str("Gen Z"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VibeTag {
    Tactile,
    CouchCoOp,
    Simple,
    Social,
}

impl fmt::Display for VibeTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tactile => f.write_str("Tactile"),
            Self::CouchCoOp => f.write_str("Couch Co-op"),
            Self::Simple => f.write_str("Simple"),
            Self::Social => f.write_str("Social"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmulatorKind {
    Mgba,
    Mednafen,
    Fceux,
    RetroArch,
}

impl EmulatorKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mgba => "mGBA",
            Self::Mednafen => "Mednafen",
            Self::Fceux => "FCEUX",
            Self::RetroArch => "RetroArch",
        }
    }
}

impl fmt::Display for EmulatorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    LocalScan,
    Catalog,
    UserUrl,
}

impl SourceKind {
    pub fn badge(self) -> &'static str {
        match self {
            Self::LocalScan => "LOCAL",
            Self::Catalog => "CATALOG",
            Self::UserUrl => "URL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallState {
    Ready,
    DownloadAvailable,
    Downloading,
    DownloadedNeedsImport,
    MissingEmulator,
    Unsupported,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataMatchState {
    Imported,
    Identifying,
    Resolved,
    Ambiguous,
    Unmatched,
    RepairNeeded,
}

impl MetadataMatchState {
    pub fn badge(self) -> &'static str {
        match self {
            Self::Imported => "IMPORTED",
            Self::Identifying => "IDENTIFYING",
            Self::Resolved => "RESOLVED",
            Self::Ambiguous => "AMBIGUOUS",
            Self::Unmatched => "UNMATCHED",
            Self::RepairNeeded => "REPAIR",
        }
    }
}

impl InstallState {
    pub fn badge(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::DownloadAvailable => "DOWNLOAD",
            Self::Downloading => "INSTALLING",
            Self::DownloadedNeedsImport => "PENDING",
            Self::MissingEmulator => "MISSING EMU",
            Self::Unsupported => "UNSUPPORTED",
            Self::Error => "ERROR",
        }
    }

    pub fn sort_bucket(self) -> u8 {
        match self {
            Self::Ready | Self::MissingEmulator => 0,
            Self::Unsupported | Self::Error => 1,
            Self::DownloadAvailable | Self::Downloading | Self::DownloadedNeedsImport => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameSourceRef {
    pub kind: SourceKind,
    pub label: Option<String>,
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEntry {
    pub id: String,
    pub title: String,
    pub filename: Option<String>,
    pub platform: Platform,
    pub generation: GenerationTag,
    pub vibe_tags: Vec<VibeTag>,
    pub source_kind: SourceKind,
    pub install_state: InstallState,
    pub managed_path: Option<PathBuf>,
    pub origin_url: Option<String>,
    pub origin_label: Option<String>,
    pub rom_path: Option<PathBuf>,
    pub hash: Option<String>,
    pub emulator_kind: Option<EmulatorKind>,
    pub checksum: Option<String>,
    pub size_bytes: Option<u64>,
    pub play_count: u32,
    pub last_played_at: Option<DateTime<Utc>>,
    pub discovered_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub source_refs: Vec<GameSourceRef>,
    pub error_message: Option<String>,
    pub progress: Option<u8>,
}

impl GameEntry {
    pub fn status_line(&self) -> String {
        match self.install_state {
            InstallState::Ready => "Ready to launch".to_string(),
            InstallState::DownloadAvailable => "Press Enter to download".to_string(),
            InstallState::Downloading => match self.progress {
                Some(progress) => format!("Downloading... {progress}%"),
                None => "Downloading...".to_string(),
            },
            InstallState::DownloadedNeedsImport => {
                "Download finished, finalizing import".to_string()
            }
            InstallState::MissingEmulator => {
                "Emulator missing; Enter installs and launches".to_string()
            }
            InstallState::Unsupported => "No configured emulator for this platform".to_string(),
            InstallState::Error => self
                .error_message
                .clone()
                .unwrap_or_else(|| "Last operation failed".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub title: String,
    pub url: String,
    pub platform: Platform,
    pub filename: String,
    pub checksum: Option<String>,
    pub legal_label: String,
    pub source_kind: SourceKind,
    /// Curated / UI-only fields (optional). Not all runners consume these yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub players: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genres: Option<Vec<String>>,
    /// HTTPS URL to box art; same shape as `starter_metadata.artwork_url` for tooling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtworkRecord {
    pub cached_path: Option<PathBuf>,
    pub remote_url: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedMetadata {
    pub game_id: String,
    pub canonical_title: String,
    pub normalized_title: String,
    pub match_state: MetadataMatchState,
    pub match_confidence: f32,
    pub provider_ids: Vec<String>,
    pub artwork: ArtworkRecord,
    pub tags: Vec<String>,
    pub genres: Vec<String>,
    pub unmatched_reason: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub fn default_emulator_for(platform: Platform) -> Option<EmulatorKind> {
    match platform {
        Platform::GameBoy | Platform::GameBoyColor | Platform::GameBoyAdvance => {
            Some(EmulatorKind::Mgba)
        }
        Platform::Ps1 => Some(EmulatorKind::Mednafen),
        Platform::Nes => Some(EmulatorKind::Fceux),
        Platform::Snes
        | Platform::SegaGenesis
        | Platform::N64
        | Platform::NintendoDs
        | Platform::Ps2
        | Platform::Wii
        | Platform::Xbox360 => Some(EmulatorKind::RetroArch),
        Platform::Unknown => None,
    }
}

pub fn sort_games(games: &mut [GameEntry]) {
    games.sort_by(|left, right| {
        match left
            .install_state
            .sort_bucket()
            .cmp(&right.install_state.sort_bucket())
        {
            Ordering::Equal => left
                .title
                .to_ascii_lowercase()
                .cmp(&right.title.to_ascii_lowercase()),
            other => other,
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_platform_from_extension() {
        assert_eq!(Platform::from_extension("gba"), Platform::GameBoyAdvance);
        assert_eq!(Platform::from_extension("cue"), Platform::Ps1);
        assert_eq!(Platform::from_extension("zip"), Platform::Unknown);
    }

    #[test]
    fn default_emulator_mapping() {
        assert_eq!(
            default_emulator_for(Platform::GameBoy),
            Some(EmulatorKind::Mgba)
        );
        assert_eq!(
            default_emulator_for(Platform::Nes),
            Some(EmulatorKind::Fceux)
        );
        assert_eq!(
            default_emulator_for(Platform::Ps1),
            Some(EmulatorKind::Mednafen)
        );
        assert_eq!(default_emulator_for(Platform::Unknown), None);
    }
}
