use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::db::Database;
use crate::emulator;
use crate::models::{EmulatorKind, GameEntry};

pub fn launch_game(db: &Database, game: &GameEntry, emulator_kind: EmulatorKind) -> Result<()> {
    let rom_path = game
        .rom_path
        .as_deref()
        .or(game.managed_path.as_deref())
        .context("selected entry is not installed yet")?;

    let info = emulator::ensure_installed(emulator_kind)?;
    let mut command = emulator::build_command(emulator_kind, &info.command, rom_path)?;
    let status = command
        .status()
        .context("failed to spawn emulator process")?;
    if !status.success() {
        bail!("emulator exited with status {status}");
    }
    db.set_game_emulator_kind(&game.id, Some(emulator_kind))?;
    db.record_launch(&game.id)?;
    Ok(())
}

pub fn destination_for_download(download_root: &Path, filename: &str) -> std::path::PathBuf {
    download_root.join(filename)
}
