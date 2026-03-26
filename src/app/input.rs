//! Input handling for keyboard events.
//!
//! This module handles all keyboard input across different modes
//! including main navigation, search, add source, and overlays.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::{AddSourceMode, App, AppTab};
use crate::catalog;
use crate::terminal::FocusPane;

impl App {
    /// Handle main navigation keys.
    pub(crate) fn handle_main_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> anyhow::Result<bool> {
        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char('1') => self.switch_tab(AppTab::Library),
            KeyCode::Char('2') => self.switch_tab(AppTab::Installed),
            KeyCode::Char('3') => self.switch_tab(AppTab::Browse),
            KeyCode::Down | KeyCode::Char('j') => self.next(),
            KeyCode::Up | KeyCode::Char('k') => self.previous(),
            KeyCode::Left | KeyCode::Char('h') => self.previous_focus(),
            KeyCode::Right | KeyCode::Char('l') => self.next_focus(),
            KeyCode::Tab => self.next_focus(),
            KeyCode::BackTab => self.previous_focus(),
            KeyCode::Char('?') => self.show_help = !self.show_help,
            KeyCode::Char('p') => {
                if self.active_tab == AppTab::Browse {
                    self.browse_prev_page();
                }
            }
            KeyCode::Char('n') => {
                if self.active_tab == AppTab::Browse {
                    self.browse_next_page();
                }
            }
            KeyCode::Char('/') => {
                self.search_mode = true;
                self.input_buffer = self.search_query.clone();
            }
            KeyCode::Char('a') => {
                self.add_source_mode = Some(AddSourceMode::Choose);
                self.input_buffer.clear();
            }
            KeyCode::Char('x') => {
                self.toast_manager.dismiss_latest();
            }
            KeyCode::Enter => self.activate_selected(terminal)?,
            _ => {}
        }
        Ok(false)
    }

    /// Handle search mode keys.
    pub(crate) fn handle_search_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.search_mode = false;
                self.input_buffer.clear();
            }
            KeyCode::Enter => {
                self.search_query = self.input_buffer.clone();
                self.search_mode = false;
                self.recompute_filtered_games();
                if self.filtered_games.is_empty() && !self.search_query.trim().is_empty() {
                    let query = self.search_query.clone();
                    let results = catalog::search_emu_land(&query)?;
                    self.search_query.clear();
                    self.recompute_filtered_games();
                    self.emu_land_search = Some(crate::app::EmuLandSearchState {
                        query,
                        results,
                        selected: 0,
                    });
                    self.sync_emu_land_search_artwork();
                    self.toast_info("NO LOCAL MATCHES. SHOWING EMU-LAND RESULTS");
                } else if self.search_query.trim().is_empty() {
                    self.toast_success("CLEARED LIBRARY FILTER");
                } else {
                    self.toast_success(format!(
                        "FILTERED LIBRARY TO {} MATCH{}",
                        self.filtered_games.len(),
                        if self.filtered_games.len() == 1 { "" } else { "ES" }
                    ));
                }
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(character) => {
                self.input_buffer.push(character);
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle add source mode keys.
    pub(crate) fn handle_add_source_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        match self.add_source_mode {
            Some(AddSourceMode::Choose) => match key.code {
                KeyCode::Esc => self.add_source_mode = None,
                KeyCode::Char('1') => {
                    self.add_source_mode = Some(AddSourceMode::Url);
                    self.input_buffer.clear();
                }
                KeyCode::Char('2') => {
                    self.add_source_mode = Some(AddSourceMode::EmuLandSearch);
                    self.input_buffer.clear();
                }
                KeyCode::Char('3') => {
                    self.add_source_mode = Some(AddSourceMode::Manifest);
                    self.input_buffer.clear();
                }
                _ => {}
            },
            Some(AddSourceMode::Url) => match key.code {
                KeyCode::Esc => {
                    self.add_source_mode = None;
                    self.input_buffer.clear();
                }
                KeyCode::Enter => {
                    match catalog::preview_user_url(&self.input_buffer, &self.paths) {
                        Ok(preview) => {
                            self.preview_artwork.sync_to_path(
                                Some(format!("preview:{}", preview.resolved_title)),
                                preview.cached_artwork_path.as_deref(),
                            );
                            self.add_url_preview = Some(crate::app::AddUrlPreviewState {
                                preview,
                                selected: 0,
                            });
                            self.toast_info("REVIEW URL PREVIEW");
                        }
                        Err(_) => {
                            self.toast_error(
                                "INVALID URL ENTRY. USE URL|TITLE(optional)|PLATFORM(optional)",
                            );
                        }
                    }
                    self.add_source_mode = None;
                    self.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Char(character) => {
                    self.input_buffer.push(character);
                }
                _ => {}
            },
            Some(AddSourceMode::EmuLandSearch) => match key.code {
                KeyCode::Esc => {
                    self.add_source_mode = None;
                    self.input_buffer.clear();
                }
                KeyCode::Enter => {
                    let results = catalog::search_emu_land(&self.input_buffer)?;
                    self.emu_land_search = Some(crate::app::EmuLandSearchState {
                        query: self.input_buffer.clone(),
                        results,
                        selected: 0,
                    });
                    self.sync_emu_land_search_artwork();
                    self.add_source_mode = None;
                    self.input_buffer.clear();
                    self.toast_success("EMU-LAND RESULTS READY");
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Char(character) => {
                    self.input_buffer.push(character);
                }
                _ => {}
            },
            Some(AddSourceMode::Manifest) => match key.code {
                KeyCode::Esc => {
                    self.add_source_mode = None;
                    self.input_buffer.clear();
                }
                KeyCode::Enter => {
                    let path = std::path::PathBuf::from(self.input_buffer.trim());
                    let entries = catalog::load_catalog(&[path])?;
                    self.db.merge_catalog_entries(&entries)?;
                    self.all_games = self.db.all_games()?;
                    self.recompute_filtered_games();
                    self.toast_success("MANIFEST IMPORTED");
                    self.spawn_metadata_jobs_for_all();
                    self.add_source_mode = None;
                    self.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Char(character) => {
                    self.input_buffer.push(character);
                }
                _ => {}
            },
            None => {}
        }
        Ok(())
    }

    /// Handle EmuLand search overlay keys.
    pub(crate) fn handle_emu_land_search_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        let Some(mut state) = self.emu_land_search.clone() else {
            return Ok(());
        };
        match key.code {
            KeyCode::Esc => {
                self.emu_land_search = None;
                self.preview_artwork.sync_to_path(None, None);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !state.results.is_empty() {
                    state.selected = (state.selected + 1).min(state.results.len() - 1);
                }
                self.emu_land_search = Some(state);
                self.sync_emu_land_search_artwork();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                state.selected = state.selected.saturating_sub(1);
                self.emu_land_search = Some(state);
                self.sync_emu_land_search_artwork();
            }
            KeyCode::Enter => {
                if let Some(result) = state.results.get(state.selected).cloned() {
                    let preview = catalog::preview_emu_land_search_result(&result, &self.paths)?;
                    self.preview_artwork.sync_to_path(
                        Some(format!("preview:{}", preview.resolved_title)),
                        preview.cached_artwork_path.as_deref(),
                    );
                    self.add_url_preview = Some(crate::app::AddUrlPreviewState {
                        selected: if preview.warning.is_some() { 1 } else { 0 },
                        preview,
                    });
                    self.emu_land_search = None;
                } else {
                    self.emu_land_search = None;
                    self.preview_artwork.sync_to_path(None, None);
                }
            }
            _ => {
                self.emu_land_search = Some(state);
            }
        }
        Ok(())
    }

    /// Handle emulator picker overlay keys.
    pub(crate) fn handle_emulator_picker_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> anyhow::Result<()> {
        let Some(mut picker) = self.emulator_picker.clone() else {
            return Ok(());
        };
        match key.code {
            KeyCode::Esc => {
                self.emulator_picker = None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                picker.selected = (picker.selected + 1).min(picker.candidates.len());
                self.emulator_picker = Some(picker);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                picker.selected = picker.selected.saturating_sub(1);
                self.emulator_picker = Some(picker);
            }
            KeyCode::Enter => {
                if picker.selected >= picker.candidates.len() {
                    self.emulator_picker = None;
                    return Ok(());
                }
                let emulator_kind = picker.candidates[picker.selected].emulator;
                self.emulator_picker = None;
                if let Some(game) = self.db.find_by_id(&picker.game_id)? {
                    self.run_launch_choice(&game, emulator_kind, terminal)?;
                }
            }
            _ => {
                self.emulator_picker = Some(picker);
            }
        }
        Ok(())
    }

    /// Handle URL preview overlay keys.
    pub(crate) fn handle_add_url_preview_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        let Some(mut state) = self.add_url_preview.clone() else {
            return Ok(());
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('d') => {
                self.preview_artwork.sync_to_path(None, None);
                self.add_url_preview = None;
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Up | KeyCode::Char('k') => {
                state.selected = 1 - state.selected.min(1);
                self.add_url_preview = Some(state);
            }
            KeyCode::Enter => {
                if state.selected == 1 {
                    self.preview_artwork.sync_to_path(None, None);
                    self.add_url_preview = None;
                } else {
                    let preview = state.preview.clone();
                    let entry = preview.entry.clone();
                    self.db.merge_catalog_entries(&[entry])?;
                    let game_id = format!(
                        "catalog:{}",
                        blake3::hash(
                            format!("{}::{}", preview.entry.url, preview.entry.filename)
                                .as_bytes()
                        )
                        .to_hex()
                    );
                    let resolved = self.resolved_metadata_from_preview(game_id.clone(), &preview);
                    self.db.upsert_resolved_metadata(&resolved)?;
                    self.resolved_metadata.insert(game_id, resolved);
                    self.all_games = self.db.all_games()?;
                    self.recompute_filtered_games();
                    self.toast_success(format!(
                        "READY TO DOWNLOAD {}",
                        preview.resolved_title.to_ascii_uppercase()
                    ));
                    self.preview_artwork.sync_to_path(None, None);
                    self.add_url_preview = None;
                }
            }
            _ => {
                self.add_url_preview = Some(state);
            }
        }
        Ok(())
    }
}
