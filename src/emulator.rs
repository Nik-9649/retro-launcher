use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::models::{EmulatorKind, Platform};

#[derive(Debug, Clone)]
pub struct EmulatorInfo {
    pub command: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Installed,
    Downloadable,
    Unavailable,
}

#[derive(Debug, Clone)]
pub struct LaunchCandidate {
    pub emulator: EmulatorKind,
    pub availability: Availability,
    pub note: String,
}

pub fn detect(kind: EmulatorKind) -> Option<EmulatorInfo> {
    let candidates = match kind {
        EmulatorKind::Mgba => vec!["mgba"],
        EmulatorKind::Mednafen => vec!["mednafen"],
        EmulatorKind::Fceux => vec!["fceux"],
        EmulatorKind::RetroArch => vec![
            "retroarch",
            "/Applications/RetroArch.app/Contents/MacOS/RetroArch",
        ],
    };
    for candidate in candidates {
        if let Some(path) = which(candidate) {
            return Some(EmulatorInfo { command: path });
        }
    }
    None
}

pub fn emulators_for_platform(platform: Platform) -> Vec<EmulatorKind> {
    match platform {
        Platform::GameBoy | Platform::GameBoyColor | Platform::GameBoyAdvance => {
            vec![EmulatorKind::Mgba]
        }
        Platform::Ps1 => vec![EmulatorKind::Mednafen],
        Platform::Nes => vec![EmulatorKind::Fceux, EmulatorKind::RetroArch],
        Platform::Snes
        | Platform::SegaGenesis
        | Platform::N64
        | Platform::NintendoDs
        | Platform::Ps2
        | Platform::Wii
        | Platform::Xbox360 => vec![EmulatorKind::RetroArch],
        Platform::Unknown => Vec::new(),
    }
}

pub fn candidate(kind: EmulatorKind) -> LaunchCandidate {
    match availability(kind) {
        Availability::Installed => LaunchCandidate {
            emulator: kind,
            availability: Availability::Installed,
            note: "Installed and ready to launch".to_string(),
        },
        Availability::Downloadable => LaunchCandidate {
            emulator: kind,
            availability: Availability::Downloadable,
            note: format!("Missing; Enter installs {}", kind.label()),
        },
        Availability::Unavailable => LaunchCandidate {
            emulator: kind,
            availability: Availability::Unavailable,
            note: unavailable_reason(kind),
        },
    }
}

pub fn availability(kind: EmulatorKind) -> Availability {
    if detect(kind).is_some() {
        Availability::Installed
    } else if matches!(kind, EmulatorKind::RetroArch) && cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        Availability::Unavailable
    } else {
        Availability::Downloadable
    }
}

pub fn unavailable_reason(kind: EmulatorKind) -> String {
    match kind {
        EmulatorKind::RetroArch if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") => {
            "Optional on Apple Silicon; Homebrew cask requires Rosetta 2".to_string()
        }
        _ => "Unavailable on this host".to_string(),
    }
}

pub fn ensure_installed(kind: EmulatorKind) -> Result<EmulatorInfo> {
    if let Some(info) = detect(kind) {
        return Ok(info);
    }
    install(kind)?;
    detect(kind).with_context(|| format!("{} was installed but not found on PATH", kind.label()))
}

pub fn build_command(kind: EmulatorKind, emulator_path: &Path, rom_path: &Path) -> Result<Command> {
    let mut command = Command::new(emulator_path);
    match kind {
        EmulatorKind::Mgba => {
            command.arg("-f").arg(rom_path);
        }
        EmulatorKind::Mednafen => {
            command.arg(rom_path);
        }
        EmulatorKind::Fceux => {
            command.arg(rom_path);
        }
        EmulatorKind::RetroArch => {
            bail!("RetroArch core selection is not configured yet")
        }
    }
    Ok(command)
}

fn install(kind: EmulatorKind) -> Result<()> {
    match kind {
        EmulatorKind::Mgba => brew_install("mgba"),
        EmulatorKind::Mednafen => brew_install("mednafen"),
        EmulatorKind::Fceux => brew_install("fceux"),
        EmulatorKind::RetroArch => bail!(
            "RetroArch is optional on Apple Silicon because the Homebrew cask requires Rosetta 2"
        ),
    }
}

fn brew_install(formula: &str) -> Result<()> {
    let status = Command::new("brew")
        .arg("install")
        .arg(formula)
        .status()
        .with_context(|| format!("failed to execute brew install {formula}"))?;
    if status.success() {
        Ok(())
    } else {
        bail!("brew install {formula} failed with status {status}");
    }
}

fn which(candidate: &str) -> Option<PathBuf> {
    let path = PathBuf::from(candidate);
    if path.is_absolute() && path.exists() {
        return Some(path);
    }
    let output = Command::new("which").arg(candidate).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(PathBuf::from(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_nes_candidates() {
        assert_eq!(
            emulators_for_platform(Platform::Nes),
            vec![EmulatorKind::Fceux, EmulatorKind::RetroArch]
        );
    }
}
