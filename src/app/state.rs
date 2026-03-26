//! Application state management.
//!
//! This module contains methods for managing application state such as
//! selection navigation, tab switching, and bounds checking.

use crate::app::{App, AppTab};

impl App {
    /// Move selection to the next item in the current tab.
    pub(crate) fn next(&mut self) {
        match self.active_tab {
            AppTab::Library => {
                if self.filtered_games.is_empty() {
                    return;
                }
                self.selected = (self.selected + 1).min(self.filtered_games.len() - 1);
            }
            AppTab::Installed => {
                if self.installed_games.is_empty() {
                    return;
                }
                self.selected = (self.selected + 1).min(self.installed_games.len() - 1);
            }
            AppTab::Browse => {
                if self.browse_items.is_empty() {
                    return;
                }
                self.browse_selected = (self.browse_selected + 1).min(self.browse_items.len() - 1);
            }
        }
        self.sync_artwork();
    }

    /// Move selection to the previous item in the current tab.
    pub(crate) fn previous(&mut self) {
        match self.active_tab {
            AppTab::Library | AppTab::Installed => {
                let empty = matches!(self.active_tab, AppTab::Library)
                    && self.filtered_games.is_empty()
                    || matches!(self.active_tab, AppTab::Installed)
                        && self.installed_games.is_empty();
                if empty {
                    return;
                }
                self.selected = self.selected.saturating_sub(1);
            }
            AppTab::Browse => {
                if self.browse_items.is_empty() {
                    return;
                }
                self.browse_selected = self.browse_selected.saturating_sub(1);
            }
        }
        self.sync_artwork();
    }

    /// Navigate to the next page in browse mode.
    pub(crate) fn browse_next_page(&mut self) {
        if self.browse_page < 10 {
            self.browse_page += 1;
            self.browse_selected = 0;
            self.spawn_browse_job();
        }
    }

    /// Navigate to the previous page in browse mode.
    pub(crate) fn browse_prev_page(&mut self) {
        if self.browse_page > 1 {
            self.browse_page -= 1;
            self.browse_selected = 0;
            self.spawn_browse_job();
        }
    }

    /// Switch to a different tab.
    pub(crate) fn switch_tab(&mut self, next: AppTab) {
        self.active_tab = next;
        if next == AppTab::Browse && self.browse_items.is_empty() {
            self.spawn_browse_job();
        }
        self.sync_artwork();
    }
}
