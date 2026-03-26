//! Application state and orchestration module.
//!
//! This module provides the main `App` struct which coordinates all application
//! state, event handling, and worker thread management. It has been split from
//! the monolithic app.rs into focused submodules for better maintainability.

mod events;
mod input;
mod state;
mod workers;

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::events::WorkerEvent;
use crate::artwork::ArtworkController;
use crate::catalog;
use crate::catalog::{EmuLandBrowseItem, EmuLandSearchResult, UserUrlPreview};
use crate::config::{AppPaths, Config};
use crate::db::Database;
use crate::emulator::{self, Availability, LaunchCandidate};
use crate::launcher;
use crate::models::{
    default_emulator_for, sort_games, ArtworkRecord, EmulatorKind, GameEntry, InstallState,
    ResolvedMetadata,
};
use crate::scanner;
use crate::terminal::{FocusPane, TerminalCapabilities, ViewportMode};
use crate::toast::ToastManager;
use crate::ui;

const TICK_RATE: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddSourceMode {
    Choose,
    Url,
    EmuLandSearch,
    Manifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Library,
    Installed,
    Browse,
}

impl AppTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Library => "LIBRARY",
            Self::Installed => "INSTALLED",
            Self::Browse => "BROWSE",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmulatorPickerState {
    pub game_id: String,
    pub title: String,
    pub candidates: Vec<LaunchCandidate>,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub struct AddUrlPreviewState {
    pub preview: UserUrlPreview,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub struct EmuLandSearchState {
    pub query: String,
    pub results: Vec<EmuLandSearchResult>,
    pub selected: usize,
}

pub struct App {
    pub config: Config,
    pub paths: AppPaths,
    pub db: Database,
    pub all_games: Vec<GameEntry>,
    pub resolved_metadata: HashMap<String, ResolvedMetadata>,
    pub filtered_games: Vec<GameEntry>,
    pub installed_games: Vec<GameEntry>,
    pub browse_items: Vec<EmuLandBrowseItem>,
    pub browse_page: usize,
    pub(crate) selected: usize,
    pub(crate) browse_selected: usize,
    pub active_tab: AppTab,
    pub show_help: bool,
    pub search_mode: bool,
    pub search_query: String,
    pub add_source_mode: Option<AddSourceMode>,
    pub add_url_preview: Option<AddUrlPreviewState>,
    pub emu_land_search: Option<EmuLandSearchState>,
    pub emulator_picker: Option<EmulatorPickerState>,
    pub input_buffer: String,
    pub focus_pane: FocusPane,
    pub viewport_mode: ViewportMode,
    pub terminal_caps: TerminalCapabilities,
    pub artwork: ArtworkController,
    pub preview_artwork: ArtworkController,
    pub toast_manager: ToastManager,
    pub(crate) worker_tx: Sender<WorkerEvent>,
    pub(crate) worker_rx: Receiver<WorkerEvent>,
}

impl App {
    pub fn new(config: Config, paths: AppPaths, db: Database) -> Result<Self> {
        let repair = db.repair_and_migrate_state(&paths)?;
        // Use the optimized query to load games and metadata in one pass
        let (all_games, resolved_metadata) = db.load_games_and_metadata()?;
        let (worker_tx, worker_rx) = mpsc::channel();
        let mut app = Self {
            config,
            paths,
            db,
            all_games,
            resolved_metadata,
            filtered_games: Vec::new(),
            installed_games: Vec::new(),
            browse_items: Vec::new(),
            browse_page: 1,
            selected: 0,
            browse_selected: 0,
            active_tab: AppTab::Library,
            show_help: false,
            search_mode: false,
            search_query: String::new(),
            add_source_mode: None,
            add_url_preview: None,
            emu_land_search: None,
            emulator_picker: None,
            input_buffer: String::new(),
            focus_pane: FocusPane::Library,
            viewport_mode: ViewportMode::Standard,
            terminal_caps: TerminalCapabilities::detect(),
            artwork: ArtworkController::unsupported(),
            preview_artwork: ArtworkController::unsupported(),
            toast_manager: ToastManager::new(),
            worker_tx,
            worker_rx,
        };
        fs::create_dir_all(app.paths.data_dir.join("artwork"))?;
        app.toast_manager.info(format!(
            "REPAIR {} URLS {} RESET {}",
            repair.normalized_urls,
            repair.removed_missing_payloads,
            repair.reset_broken_downloads + repair.reset_emulator_assignments
        ));
        app.recompute_filtered_games();
        Ok(app)
    }

    pub fn initialize_terminal_ui(&mut self) {
        self.terminal_caps = TerminalCapabilities::detect();
        self.artwork = ArtworkController::new(self.terminal_caps);
        self.preview_artwork = ArtworkController::new(self.terminal_caps);
        self.sync_artwork();
    }

    pub fn set_viewport_mode(&mut self, viewport_mode: ViewportMode) {
        self.viewport_mode = viewport_mode;
    }

    pub fn selection(&self) -> Option<usize> {
        match self.active_tab {
            AppTab::Library => (!self.filtered_games.is_empty())
                .then_some(self.selected.min(self.filtered_games.len().saturating_sub(1))),
            AppTab::Installed => (!self.installed_games.is_empty())
                .then_some(self.selected.min(self.installed_games.len().saturating_sub(1))),
            AppTab::Browse => (!self.browse_items.is_empty())
                .then_some(self.browse_selected.min(self.browse_items.len().saturating_sub(1))),
        }
    }

    pub fn selected_game(&self) -> Option<&GameEntry> {
        match self.active_tab {
            AppTab::Library => self.selection().and_then(|index| self.filtered_games.get(index)),
            AppTab::Installed => self.selection().and_then(|index| self.installed_games.get(index)),
            AppTab::Browse => None,
        }
    }

    pub fn selected_browse_item(&self) -> Option<&EmuLandBrowseItem> {
        if self.active_tab == AppTab::Browse {
            self.selection()
                .and_then(|index| self.browse_items.get(index))
        } else {
            None
        }
    }

    pub fn metadata_for_game(&self, game_id: &str) -> Option<&ResolvedMetadata> {
        self.resolved_metadata.get(game_id)
    }

    pub fn display_title_for(&self, game: &GameEntry) -> String {
        self.metadata_for_game(&game.id)
            .map(|metadata| metadata.canonical_title.clone())
            .unwrap_or_else(|| game.title.clone())
    }

    pub fn next_focus(&mut self) {
        self.focus_pane = self.focus_pane.next();
    }

    pub fn previous_focus(&mut self) {
        self.focus_pane = self.focus_pane.previous();
    }

    pub fn footer_hint(&self) -> String {
        if self.add_url_preview.is_some() {
            return "[↑↓/jk] Review  [Enter] Add to Library  [d/Esc] Discard".to_string();
        }
        if self.emu_land_search.is_some() {
            return "[↑↓/jk] Choose Result  [Enter] Preview  [Esc] Close".to_string();
        }
        if self.emulator_picker.is_some() {
            return "[↑↓/jk] Choose Emulator  [Enter] Launch / Install  [Esc] Cancel".to_string();
        }
        let base = match self.focus_pane {
            FocusPane::Library => {
                "[↑↓/jk] Browse  [1/2/3] Tabs  [Tab] Focus  [Enter] Action  [/] Search  [a] Add Source  [?] Help  [q] Quit"
            }
            FocusPane::Artwork => {
                "[1/2/3] Tabs  [Tab] Focus  [h/l] Pane  [Enter] Action  [/] Search  [?] Help  [q] Quit"
            }
            FocusPane::Summary => {
                "[1/2/3] Tabs  [Tab] Focus  [h/l] Pane  [Enter] Action  [/] Search  [a] Add Source  [q] Quit"
            }
        };

        if self.search_mode {
            format!("{base}  |  SEARCH {}", self.input_buffer)
        } else if !self.search_query.is_empty() {
            format!("{base}  |  FILTER {}", self.search_query)
        } else {
            base.to_string()
        }
    }

    pub(crate) fn recompute_filtered_games(&mut self) {
        let query = self.search_query.to_ascii_lowercase();
        self.filtered_games = self
            .all_games
            .iter()
            .filter(|game| {
                let display_title = self
                    .resolved_metadata
                    .get(&game.id)
                    .map(|meta| meta.canonical_title.as_str())
                    .unwrap_or(game.title.as_str());
                query.is_empty() || display_title.to_ascii_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        sort_games(&mut self.filtered_games);
        self.installed_games = self
            .filtered_games
            .iter()
            .filter(|game| game.rom_path.is_some() || game.managed_path.is_some())
            .cloned()
            .collect();
        if self.selected >= self.filtered_games.len() {
            self.selected = self.filtered_games.len().saturating_sub(1);
        }
        if self.selected >= self.installed_games.len() && self.active_tab == AppTab::Installed {
            self.selected = self.installed_games.len().saturating_sub(1);
        }
        if self.browse_selected >= self.browse_items.len() {
            self.browse_selected = self.browse_items.len().saturating_sub(1);
        }
        self.sync_artwork();
    }

    /// Show an info toast notification
    pub fn toast_info(&mut self, message: impl Into<String>) {
        self.toast_manager.info(message);
    }

    /// Show a success toast notification
    pub fn toast_success(&mut self, message: impl Into<String>) {
        self.toast_manager.success(message);
    }

    /// Show a warning toast notification
    pub fn toast_warning(&mut self, message: impl Into<String>) {
        self.toast_manager.warning(message);
    }

    /// Show an error toast notification
    pub fn toast_error(&mut self, message: impl Into<String>) {
        self.toast_manager.error(message);
    }

    fn sync_emu_land_search_artwork(&mut self) {
        let Some(state) = self.emu_land_search.as_ref() else {
            self.preview_artwork.sync_to_path(None, None);
            return;
        };
        let Some(result) = state.results.get(state.selected) else {
            self.preview_artwork.sync_to_path(None, None);
            return;
        };
        let cached_path = result
            .preview_image_url
            .as_ref()
            .and_then(|url| catalog::cache_search_result_artwork(&self.paths, &result.title, url).ok());
        self.preview_artwork
            .sync_to_path(Some(format!("search:{}", result.href)), cached_path.as_deref());
    }

    pub(crate) fn sync_artwork(&mut self) {
        if let Some(item) = self.selected_browse_item() {
            let cached_path = item
                .preview_image_url
                .as_ref()
                .and_then(|url| catalog::cache_search_result_artwork(&self.paths, &item.title, url).ok());
            self.artwork
                .sync_to_path(Some(format!("browse:{}", item.href)), cached_path.as_deref());
            return;
        }
        let selected = self.selected_game().cloned();
        let metadata = selected
            .as_ref()
            .and_then(|game| self.resolved_metadata.get(&game.id).cloned());
        self.artwork
            .sync_to_game(&self.paths, selected.as_ref(), metadata.as_ref());
    }

    pub(crate) fn replace_or_push(&mut self, game: GameEntry) {
        match self
            .all_games
            .iter()
            .position(|existing| existing.id == game.id)
        {
            Some(index) => self.all_games[index] = game,
            None => self.all_games.push(game),
        }
        sort_games(&mut self.all_games);
        self.recompute_filtered_games();
    }

    pub(crate) fn resolved_metadata_from_preview(
        &self,
        game_id: String,
        preview: &UserUrlPreview,
    ) -> ResolvedMetadata {
        ResolvedMetadata {
            game_id,
            canonical_title: preview.resolved_title.clone(),
            normalized_title: crate::metadata::normalize_title(&preview.resolved_title),
            match_state: preview.match_state,
            match_confidence: (preview.confidence as f32) / 100.0,
            provider_ids: preview.provider_ids.clone(),
            artwork: ArtworkRecord {
                cached_path: preview.cached_artwork_path.clone(),
                remote_url: preview.artwork_url.clone(),
                source: Some("preview-confirmed".to_string()),
            },
            tags: preview.tags.clone(),
            genres: preview.genres.clone(),
            unmatched_reason: preview.unmatched_reason.clone(),
            updated_at: chrono::Utc::now(),
        }
    }

    pub fn spawn_startup_jobs(&self) {
        let tx_scan = self.worker_tx.clone();
        let db = self.db.clone();
        let roots = self.config.rom_roots.clone();
        let show_hidden = self.config.show_hidden_files;
        thread::spawn(move || {
            let _ = tx_scan.send(WorkerEvent::Status("SCANNING LOCAL ROM ROOTS".to_string()));
            let result = scanner::scan_rom_roots(&db, &roots, show_hidden)
                .map_err(|error| error.to_string());
            let _ = tx_scan.send(WorkerEvent::ScanFinished(result));
        });

        self.spawn_browse_job();
        self.spawn_metadata_jobs_for_all();
    }

    pub fn run_launch_choice(
        &mut self,
        game: &GameEntry,
        emulator_kind: EmulatorKind,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        match emulator::availability(emulator_kind) {
            Availability::Unavailable => {
                self.toast_warning(emulator::unavailable_reason(emulator_kind).to_ascii_uppercase());
                return Ok(());
            }
            Availability::Installed => {
                // Launching notification shown on return
            }
            Availability::Downloadable => {
                self.toast_info(format!(
                    "INSTALLING {}",
                    emulator_kind.label().to_ascii_uppercase()
                ));
            }
        }
        self.launch_with_terminal_suspend(game, emulator_kind, terminal)?;
        self.all_games = self.db.all_games()?;
        self.recompute_filtered_games();
        self.toast_success(format!(
            "RETURNED FROM {} VIA {}",
            game.title.to_ascii_uppercase(),
            emulator_kind.label().to_ascii_uppercase()
        ));
        Ok(())
    }

    fn launch_with_terminal_suspend(
        &mut self,
        game: &GameEntry,
        emulator_kind: EmulatorKind,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        let launch_result = launcher::launch_game(&self.db, game, emulator_kind);
        enable_raw_mode()?;
        execute!(terminal.backend_mut(), EnterAlternateScreen)?;
        terminal.hide_cursor()?;
        terminal.clear()?;
        launch_result
    }

    pub fn launch_candidates_for(&self, game: &GameEntry) -> Vec<LaunchCandidate> {
        let mut ordered = self.config.preferred_emulators_for(game.platform);
        for emulator_kind in emulator::emulators_for_platform(game.platform) {
            if !ordered.contains(&emulator_kind) {
                ordered.push(emulator_kind);
            }
        }
        if let Some(last_used) = game.emulator_kind {
            if ordered.contains(&last_used) {
                ordered.retain(|entry| *entry != last_used);
                ordered.insert(0, last_used);
            }
        }
        ordered.into_iter().map(emulator::candidate).collect()
    }

    pub fn open_emulator_picker(&mut self, game: &GameEntry, candidates: Vec<LaunchCandidate>) {
        let selected = self
            .config
            .preferred_emulators_for(game.platform)
            .into_iter()
            .find_map(|emulator_kind| {
                candidates
                    .iter()
                    .position(|candidate| candidate.emulator == emulator_kind)
            })
            .or_else(|| {
                default_emulator_for(game.platform).and_then(|emulator_kind| {
                    candidates
                        .iter()
                        .position(|candidate| candidate.emulator == emulator_kind)
                })
            })
            .unwrap_or(0);
        self.emulator_picker = Some(EmulatorPickerState {
            game_id: game.id.clone(),
            title: self.display_title_for(game),
            candidates,
            selected,
        });
    }

    pub fn activate_selected(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        if self.active_tab == AppTab::Browse {
            if let Some(item) = self.selected_browse_item().cloned() {
                let result = EmuLandSearchResult {
                    title: item.title,
                    href: item.href,
                    platform: item.platform,
                    preview_image_url: item.preview_image_url,
                    genres: item.genres,
                    players: item.players,
                };
                let preview = catalog::preview_emu_land_search_result(&result, &self.paths)?;
                self.preview_artwork.sync_to_path(
                    Some(format!("preview:{}", preview.resolved_title)),
                    preview.cached_artwork_path.as_deref(),
                );
                self.add_url_preview = Some(AddUrlPreviewState {
                    selected: if preview.warning.is_some() { 1 } else { 0 },
                    preview,
                });
                self.toast_info("REVIEW BROWSE PREVIEW");
            }
            return Ok(());
        }
        let Some(game) = self.selected_game().cloned() else {
            return Ok(());
        };
        match game.install_state {
            InstallState::DownloadAvailable => self.start_download(game),
            InstallState::Ready | InstallState::MissingEmulator => {
                let candidates = self.launch_candidates_for(&game);
                if candidates.is_empty() {
                    self.toast_warning("NO CONFIGURED EMULATOR FOR THIS PLATFORM");
                } else if candidates.len() == 1 {
                    self.run_launch_choice(&game, candidates[0].emulator, terminal)?;
                } else {
                    self.open_emulator_picker(&game, candidates);
                }
            }
            InstallState::Unsupported => {
                self.toast_warning("NO CONFIGURED EMULATOR FOR THIS PLATFORM");
            }
            InstallState::Downloading | InstallState::DownloadedNeedsImport => {
                self.toast_info("THIS ITEM IS STILL DOWNLOADING");
            }
            InstallState::Error => {
                if game.origin_url.is_some() {
                    self.start_download(game);
                } else {
                    self.toast_error(game.status_line().to_ascii_uppercase());
                }
            }
        }
        Ok(())
    }
}

pub fn run() -> Result<()> {
    let (config, paths) = Config::load_or_create()?;
    let db = Database::new(&paths.db_path)?;
    let mut app = App::new(config, paths, db)?;
    app.spawn_startup_jobs();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;
    app.initialize_terminal_ui();

    let result = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        app.drain_worker_events();
        terminal.draw(|frame| ui::render(frame, app))?;
        app.artwork.last_encoding_result().ok();
        if event::poll(TICK_RATE)? {
            if let Event::Key(key) = event::read()? {
                if app.show_help && key.code == KeyCode::Esc {
                    app.show_help = false;
                    continue;
                }
                if app.show_help && matches!(key.code, KeyCode::Char('?') | KeyCode::Char('q')) {
                    app.show_help = false;
                    continue;
                }
                if app.add_url_preview.is_some() {
                    app.handle_add_url_preview_key(key)?;
                    continue;
                }
                if app.emu_land_search.is_some() {
                    app.handle_emu_land_search_key(key)?;
                    continue;
                }
                if app.emulator_picker.is_some() {
                    app.handle_emulator_picker_key(key, terminal)?;
                    continue;
                }
                if app.search_mode {
                    app.handle_search_key(key)?;
                    continue;
                }
                if app.add_source_mode.is_some() {
                    app.handle_add_source_key(key)?;
                    continue;
                }
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }
                if app.handle_main_key(key, terminal)? {
                    break;
                }
            }
        }
        io::stdout().flush().ok();
    }
    Ok(())
}

fn fetch_to_path(url: &str, destination: &PathBuf, on_progress: impl Fn(u8)) -> Result<()> {
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

fn ensure_valid_download_payload(path: &PathBuf) -> Result<()> {
    if download_payload_is_invalid(path)? {
        anyhow::bail!("downloaded content was HTML/text, not a ROM payload");
    }
    Ok(())
}

fn download_payload_is_invalid(path: &PathBuf) -> Result<bool> {
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

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::models::{GenerationTag, Platform, SourceKind, VibeTag};

    #[test]
    fn downloads_from_file_url() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source.gba");
        let dest = dir.path().join("dest.gba");
        fs::write(&source, b"gba demo").unwrap();
        fetch_to_path(&format!("file://{}", source.display()), &dest, |_| {}).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"gba demo");
    }

    #[test]
    fn rejects_html_payloads() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("page.gba");
        let dest = dir.path().join("dest.gba");
        fs::write(
            &source,
            "<!DOCTYPE html><html><body>not a rom</body></html>",
        )
        .unwrap();
        let error =
            fetch_to_path(&format!("file://{}", source.display()), &dest, |_| {}).unwrap_err();
        assert!(error.to_string().contains("HTML"));
    }

    #[test]
    fn filters_games_by_query() {
        let dir = tempdir().unwrap();
        let paths = AppPaths {
            config_dir: dir.path().join("cfg"),
            data_dir: dir.path().join("data"),
            downloads_dir: dir.path().join("downloads"),
            db_path: dir.path().join("db.sqlite"),
            config_path: dir.path().join("cfg/config.toml"),
        };
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::create_dir_all(&paths.data_dir).unwrap();
        fs::create_dir_all(&paths.downloads_dir).unwrap();
        let db = Database::new(&paths.db_path).unwrap();
        let now = chrono::Utc::now();
        let rom_path = dir.path().join("poke.gba");
        fs::write(&rom_path, b"rom").unwrap();
        db.upsert_game(&GameEntry {
            id: "1".into(),
            title: "Pokemon Emerald".into(),
            filename: None,
            platform: Platform::GameBoyAdvance,
            generation: GenerationTag::Millennials,
            vibe_tags: vec![VibeTag::Tactile],
            source_kind: SourceKind::LocalScan,
            install_state: InstallState::Ready,
            managed_path: None,
            origin_url: None,
            origin_label: None,
            rom_path: Some(rom_path),
            hash: Some("hash1".into()),
            emulator_kind: default_emulator_for(Platform::GameBoyAdvance),
            checksum: None,
            size_bytes: None,
            play_count: 0,
            last_played_at: None,
            discovered_at: now,
            updated_at: now,
            source_refs: Vec::new(),
            error_message: None,
            progress: None,
        })
        .unwrap();
        let mut app = App::new(
            Config {
                rom_roots: vec![],
                managed_download_dir: paths.downloads_dir.clone(),
                scan_on_startup: true,
                show_hidden_files: false,
                preferred_emulators: vec![],
            },
            paths,
            db,
        )
        .unwrap();
        app.search_query = "emerald".into();
        app.recompute_filtered_games();
        assert_eq!(app.filtered_games.len(), 1);
    }

    #[test]
    fn focus_cycles_with_tab_model() {
        let dir = tempdir().unwrap();
        let paths = AppPaths {
            config_dir: dir.path().join("cfg"),
            data_dir: dir.path().join("data"),
            downloads_dir: dir.path().join("downloads"),
            db_path: dir.path().join("db.sqlite"),
            config_path: dir.path().join("cfg/config.toml"),
        };
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::create_dir_all(&paths.data_dir).unwrap();
        fs::create_dir_all(&paths.downloads_dir).unwrap();
        let db = Database::new(&paths.db_path).unwrap();
        let mut app = App::new(
            Config {
                rom_roots: vec![],
                managed_download_dir: paths.downloads_dir.clone(),
                scan_on_startup: true,
                show_hidden_files: false,
                preferred_emulators: vec![],
            },
            paths,
            db,
        )
        .unwrap();
        assert_eq!(app.focus_pane, FocusPane::Library);
        app.next_focus();
        assert_eq!(app.focus_pane, FocusPane::Artwork);
        app.previous_focus();
        assert_eq!(app.focus_pane, FocusPane::Library);
    }
}
