//! Worker thread management.
//!
//! This module handles spawning background worker threads for
//! scanning, metadata resolution, downloads, and browsing.

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::thread;

use anyhow::{Context, Result};

use crate::app::events::WorkerEvent;
use crate::app::App;
use crate::catalog;
use crate::launcher;
use crate::metadata::MetadataService;
use crate::models::{default_emulator_for, GameEntry, InstallState};
use crate::scanner;

impl App {
    /// Spawn the browse job to load Emu-Land top titles.
    pub(crate) fn spawn_browse_job(&self) {
        let tx_browse = self.worker_tx.clone();
        let page = self.browse_page;
        thread::spawn(move || {
            let _ = tx_browse.send(WorkerEvent::Status(format!("LOADING EMU-LAND TOP PAGE {page}")));
            let result = catalog::load_emu_land_top(page).map_err(|error| error.to_string());
            let _ = tx_browse.send(WorkerEvent::BrowseLoaded(result));
        });
    }

    /// Spawn metadata resolution jobs for all games.
    pub(crate) fn spawn_metadata_jobs_for_all(&self) {
        // Clone only the IDs to avoid cloning the entire Vec
        for game in &self.all_games {
            self.spawn_metadata_job(game.clone());
        }
    }

    /// Spawn a single metadata resolution job for a game.
    pub(crate) fn spawn_metadata_job(&self, game: GameEntry) {
        let tx = self.worker_tx.clone();
        let service = MetadataService::new(self.db.clone(), self.paths.clone());
        thread::spawn(move || {
            let result = match service {
                Ok(service) => service
                    .enrich_game(&game)
                    .map_err(|error| error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            let _ = tx.send(WorkerEvent::MetadataResolved {
                game_id: game.id,
                metadata: result,
            });
        });
    }

    /// Start a download for a game.
    pub(crate) fn start_download(&mut self, game: GameEntry) {
        let tx = self.worker_tx.clone();
        let db = self.db.clone();
        let download_root = self.paths.downloads_dir.clone();
        if let Some(existing) = self.all_games.iter_mut().find(|entry| entry.id == game.id) {
            existing.install_state = InstallState::Downloading;
            existing.progress = Some(0);
        }
        self.recompute_filtered_games();
        thread::spawn(move || {
            let result = (|| -> Result<(GameEntry, String)> {
                let origin_url = game.origin_url.clone().context("missing download URL")?;
                let normalized_url = catalog::normalize_download_url(&origin_url);
                let catalog_filename = game
                    .filename
                    .clone()
                    .unwrap_or_else(|| format!("{}.rom", game.title.replace(' ', "_")));
                let download_name =
                    scanner::download_filename_for_url(&normalized_url, &catalog_filename);
                let destination =
                    launcher::destination_for_download(&download_root, &download_name);
                fetch_to_path(&normalized_url, &destination, |progress| {
                    let _ = tx.send(WorkerEvent::DownloadProgress {
                        id: game.id.clone(),
                        progress,
                    });
                })?;
                if let Some(expected) = &game.checksum {
                    let actual = blake3::hash(&fs::read(&destination)?).to_hex().to_string();
                    if actual != *expected {
                        anyhow::bail!("checksum mismatch for {}", destination.display());
                    }
                }
                let rom_path = scanner::resolve_downloaded_rom_path(
                    &destination,
                    &download_root,
                    &catalog_filename,
                )?;
                let mut imported = scanner::import_file(
                    &db,
                    &rom_path,
                    game.source_kind,
                    Some(normalized_url.clone()),
                    game.origin_label.clone(),
                )?;
                db.transfer_resolved_metadata(&game.id, &imported.id)?;
                if let Some(metadata) = db.find_resolved_metadata(&imported.id)? {
                    imported.title = metadata.canonical_title.clone();
                }
                db.upsert_game(&imported)?;
                let deduped_into_existing = imported.id != game.id;
                if deduped_into_existing {
                    let adopted_download_path = imported.rom_path.as_ref() == Some(&rom_path)
                        || imported.managed_path.as_ref() == Some(&rom_path);
                    if !adopted_download_path {
                        let _ = fs::remove_file(&rom_path);
                    }
                } else {
                    imported.managed_path = Some(rom_path.clone());
                    imported.origin_url = Some(normalized_url);
                    imported.origin_label = game.origin_label.clone();
                    imported.source_kind = game.source_kind;
                    imported.progress = None;
                    imported.install_state = match default_emulator_for(imported.platform) {
                        Some(kind) => {
                            imported.emulator_kind = Some(kind);
                            if crate::emulator::detect(kind).is_some() {
                                InstallState::Ready
                            } else {
                                InstallState::MissingEmulator
                            }
                        }
                        None => InstallState::Unsupported,
                    };
                    db.upsert_game(&imported)?;
                }
                if deduped_into_existing {
                    db.remove_game(&game.id)?;
                    Ok((
                        imported.clone(),
                        format!(
                            "Downloaded payload matched existing game: {}",
                            imported.title
                        ),
                    ))
                } else {
                    Ok((imported, format!("Downloaded {}", game.title)))
                }
            })();

            match result {
                Ok((game, message)) => {
                    let _ = tx.send(WorkerEvent::DownloadFinished { game, message });
                }
                Err(error) => {
                    let _ = tx.send(WorkerEvent::DownloadFailed {
                        id: game.id,
                        message: error.to_string(),
                    });
                }
            }
        });
    }
}

/// Fetch a URL to a local path with progress callbacks.
pub(crate) fn fetch_to_path(
    url: &str,
    destination: &PathBuf,
    on_progress: impl Fn(u8),
) -> Result<()> {
    if url.starts_with("file://") {
        let source = PathBuf::from(url.trim_start_matches("file://"));
        fs::copy(&source, destination)?;
        ensure_valid_download_payload(destination)?;
        on_progress(100);
        return Ok(());
    }

    let mut response =
        reqwest::blocking::get(url).with_context(|| format!("failed to GET {url}"))?;
    if !response.status().is_success() {
        anyhow::bail!("download failed with status {}", response.status());
    }
    if let Some(content_type) = response.headers().get(reqwest::header::CONTENT_TYPE) {
        let content_type = content_type
            .to_str()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if content_type.starts_with("text/html") || content_type.starts_with("application/xhtml") {
            anyhow::bail!("download URL resolved to HTML instead of ROM data");
        }
    }
    let total = response.content_length();
    let mut file = fs::File::create(destination)?;
    let mut downloaded = 0u64;
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = response.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])?;
        downloaded += read as u64;
        if let Some(total) = total {
            let progress = ((downloaded as f64 / total as f64) * 100.0).round() as u8;
            on_progress(progress.min(100));
        }
    }
    if total.is_none() {
        on_progress(100);
    }
    ensure_valid_download_payload(destination)?;
    Ok(())
}

/// Ensure the downloaded payload is valid (not HTML).
pub(crate) fn ensure_valid_download_payload(path: &PathBuf) -> Result<()> {
    if download_payload_is_invalid(path)? {
        anyhow::bail!("downloaded content was HTML/text, not a ROM payload");
    }
    Ok(())
}

/// Check if the downloaded payload appears to be HTML instead of a ROM.
pub(crate) fn download_payload_is_invalid(path: &PathBuf) -> Result<bool> {
    let mut file = fs::File::open(path)?;
    let mut sample = [0u8; 512];
    let read = file.read(&mut sample)?;
    let sniff = String::from_utf8_lossy(&sample[..read]).to_ascii_lowercase();
    Ok(sniff.contains("<!doctype html")
        || sniff.contains("<html")
        || sniff.contains("<head")
        || sniff.contains("<body")
        || sniff.contains("github") && sniff.contains("blob"))
}
