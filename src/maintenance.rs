use std::fs;

use anyhow::Result;

use crate::config::Config;
use crate::db::{Database, RepairReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintenanceAction {
    Repair,
    ClearMetadata,
    ResetDownloads,
    ResetAll,
}

impl MaintenanceAction {
    pub fn parse(input: &str) -> Option<Self> {
        match input {
            "repair" | "repair-state" => Some(Self::Repair),
            "clear-metadata" => Some(Self::ClearMetadata),
            "reset-downloads" => Some(Self::ResetDownloads),
            "reset-all" => Some(Self::ResetAll),
            _ => None,
        }
    }
}

impl From<crate::cli::MaintenanceAction> for MaintenanceAction {
    fn from(action: crate::cli::MaintenanceAction) -> Self {
        match action {
            crate::cli::MaintenanceAction::Repair => Self::Repair,
            crate::cli::MaintenanceAction::ClearMetadata => Self::ClearMetadata,
            crate::cli::MaintenanceAction::ResetDownloads => Self::ResetDownloads,
            crate::cli::MaintenanceAction::ResetAll => Self::ResetAll,
        }
    }
}

pub fn run(action: MaintenanceAction) -> Result<String> {
    let (_config, paths) = Config::load_or_create()?;
    let db = Database::new(&paths.db_path)?;
    let message = match action {
        MaintenanceAction::Repair => {
            let report = db.repair_and_migrate_state(&paths)?;
            format_report("repair", report)
        }
        MaintenanceAction::ClearMetadata => {
            db.clear_metadata_cache()?;
            if paths.data_dir.join("artwork").exists() {
                for entry in fs::read_dir(paths.data_dir.join("artwork"))? {
                    let entry = entry?;
                    if entry.path().is_file() {
                        fs::remove_file(entry.path())?;
                    }
                }
            }
            "Cleared metadata cache and artwork cache.".to_string()
        }
        MaintenanceAction::ResetDownloads => {
            let download_prefix = format!("{}/", paths.downloads_dir.display());
            let conn = rusqlite::Connection::open(&paths.db_path)?;
            conn.execute(
                "DELETE FROM games WHERE coalesce(managed_path, rom_path, '') LIKE ?1",
                rusqlite::params![download_prefix],
            )?;
            for entry in fs::read_dir(&paths.downloads_dir)? {
                let entry = entry?;
                if entry.path().is_file() {
                    fs::remove_file(entry.path())?;
                }
            }
            "Cleared launcher-managed downloads and related DB rows.".to_string()
        }
        MaintenanceAction::ResetAll => {
            if paths.db_path.exists() {
                fs::remove_file(&paths.db_path)?;
            }
            if paths.downloads_dir.exists() {
                for entry in fs::read_dir(&paths.downloads_dir)? {
                    let entry = entry?;
                    if entry.path().is_file() {
                        fs::remove_file(entry.path())?;
                    }
                }
            }
            let artwork_dir = paths.data_dir.join("artwork");
            if artwork_dir.exists() {
                for entry in fs::read_dir(artwork_dir)? {
                    let entry = entry?;
                    if entry.path().is_file() {
                        fs::remove_file(entry.path())?;
                    }
                }
            }
            "Reset launcher database, artwork cache, and managed downloads.".to_string()
        }
    };
    Ok(message)
}

fn format_report(prefix: &str, report: RepairReport) -> String {
    format!(
        "{} complete: removed_missing_payloads={} normalized_urls={} removed_legacy_demo_rows={} reset_broken_downloads={} reset_emulator_assignments={}",
        prefix,
        report.removed_missing_payloads,
        report.normalized_urls,
        report.removed_legacy_demo_rows,
        report.reset_broken_downloads,
        report.reset_emulator_assignments
    )
}
