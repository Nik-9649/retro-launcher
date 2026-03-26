//! Worker event handling.
//!
//! This module handles events from background worker threads including
//! scan completion, metadata resolution, and download progress.

use crate::app::App;
use crate::models::{InstallState, ResolvedMetadata};

/// Events sent from worker threads to the main application thread.
#[derive(Debug)]
pub(crate) enum WorkerEvent {
    Status(String),
    ScanFinished(Result<Vec<crate::models::GameEntry>, String>),
    MetadataResolved {
        game_id: String,
        metadata: Result<ResolvedMetadata, String>,
    },
    DownloadProgress { id: String, progress: u8 },
    DownloadFinished { game: crate::models::GameEntry, message: String },
    BrowseLoaded(Result<Vec<crate::catalog::EmuLandBrowseItem>, String>),
    DownloadFailed { id: String, message: String },
}

impl App {
    /// Drain all pending worker events from the channel.
    pub(crate) fn drain_worker_events(&mut self) {
        while let Ok(event) = self.worker_rx.try_recv() {
            match event {
                WorkerEvent::Status(_message) => {
                    // Activity messages now shown via toast on completion
                }
                WorkerEvent::ScanFinished(result) => match result {
                    Ok(_) => {
                        self.toast_success("LOCAL LIBRARY SCAN COMPLETE");
                        if let Ok(games) = self.db.all_games() {
                            self.all_games = games;
                            self.recompute_filtered_games();
                            self.spawn_metadata_jobs_for_all();
                        }
                    }
                    Err(message) => self.toast_error(format!("SCAN FAILED: {message}")),
                },
                WorkerEvent::MetadataResolved { game_id, metadata } => match metadata {
                    Ok(metadata) => {
                        self.resolved_metadata.insert(game_id, metadata);
                        self.sync_artwork();
                    }
                    Err(message) => {
                        self.toast_error(format!("METADATA FAILED: {message}").to_ascii_uppercase());
                    }
                },
                WorkerEvent::DownloadProgress { id, progress } => {
                    if let Some(game) = self.all_games.iter_mut().find(|game| game.id == id) {
                        game.progress = Some(progress);
                        game.install_state = InstallState::Downloading;
                        game.updated_at = chrono::Utc::now();
                    }
                    // Progress updates are transient, only show completion via toast
                    self.recompute_filtered_games();
                }
                WorkerEvent::DownloadFinished { game, message } => {
                    self.toast_success(message.to_ascii_uppercase());
                    let resolved_target = game.clone();
                    self.replace_or_push(game);
                    if let Ok(games) = self.db.all_games() {
                        self.all_games = games;
                        self.recompute_filtered_games();
                        self.spawn_metadata_job(resolved_target);
                    }
                }
                WorkerEvent::BrowseLoaded(result) => match result {
                    Ok(items) => {
                        self.browse_items = items;
                        self.browse_selected = self
                            .browse_selected
                            .min(self.browse_items.len().saturating_sub(1));
                        self.toast_success(format!("BROWSE READY {} TITLES", self.browse_items.len()));
                        self.sync_artwork();
                    }
                    Err(message) => {
                        self.toast_error(format!("BROWSE FAILED: {message}"));
                    }
                },
                WorkerEvent::DownloadFailed { id, message } => {
                    if let Some(game) = self.all_games.iter_mut().find(|game| game.id == id) {
                        game.install_state = InstallState::Error;
                        game.error_message = Some(message.clone());
                    }
                    self.toast_error(message.to_ascii_uppercase());
                    self.recompute_filtered_games();
                }
            }
        }

        // Update toast animations
        self.toast_manager.tick();
    }
}
