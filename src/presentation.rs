use crate::app::{App, AppTab};
use crate::models::{GameEntry, InstallState, MetadataMatchState, ResolvedMetadata};
use crate::terminal::{FocusPane, ViewportMode};

/// Header stats for a specific tab context
pub struct HeaderStats {
    pub primary_counts: Vec<(String, u32, StatColor)>,
    pub secondary_info: Option<String>,
}

pub enum StatColor {
    Success,
    Warning,
    Error,
    Info,
    Muted,
}

pub struct HeroSummary {
    pub title: String,
    pub platform: String,
    pub generation: String,
    pub badges: Vec<String>,
    pub source_badges: Vec<String>,
    pub vibe_line: String,
    pub play_line: String,
    pub state_line: String,
    pub metadata_line: String,
    pub path_line: String,
    pub hash_line: Option<String>,
}

/// Build contextual header stats based on the active tab
pub fn build_header_stats(app: &App) -> HeaderStats {
    match app.active_tab {
        AppTab::Library => build_library_stats(app),
        AppTab::Installed => build_installed_stats(app),
        AppTab::Browse => build_browse_stats(app),
    }
}

fn build_library_stats(app: &App) -> HeaderStats {
    let ready = app
        .all_games
        .iter()
        .filter(|game| matches!(game.install_state, InstallState::Ready))
        .count() as u32;
    let missing_emu = app
        .all_games
        .iter()
        .filter(|game| matches!(game.install_state, InstallState::MissingEmulator))
        .count() as u32;
    let downloads = app
        .all_games
        .iter()
        .filter(|game| {
            matches!(
                game.install_state,
                InstallState::DownloadAvailable | InstallState::Downloading
            )
        })
        .count() as u32;
    let pending_meta = app
        .resolved_metadata
        .values()
        .filter(|meta| !matches!(meta.match_state, MetadataMatchState::Resolved))
        .count() as u32;

    let mut counts = vec![];
    if ready > 0 {
        counts.push(("ready".to_string(), ready, StatColor::Success));
    }
    if missing_emu > 0 {
        counts.push(("need emulator".to_string(), missing_emu, StatColor::Warning));
    }
    if downloads > 0 {
        counts.push(("downloading".to_string(), downloads, StatColor::Info));
    }
    if pending_meta > 0 {
        counts.push(("pending meta".to_string(), pending_meta, StatColor::Muted));
    }

    // If no games at all, show empty state
    if counts.is_empty() {
        if app.all_games.is_empty() {
            return HeaderStats {
                primary_counts: vec![("No games in library".to_string(), 0, StatColor::Muted)],
                secondary_info: Some("Press [a] to add sources".to_string()),
            };
        }
        counts.push(("games".to_string(), app.all_games.len() as u32, StatColor::Muted));
    }

    HeaderStats {
        primary_counts: counts,
        secondary_info: None,
    }
}

fn build_installed_stats(app: &App) -> HeaderStats {
    let installed = app
        .installed_games
        .iter()
        .filter(|game| matches!(game.install_state, InstallState::Ready))
        .count() as u32;
    let missing_emu = app
        .installed_games
        .iter()
        .filter(|game| matches!(game.install_state, InstallState::MissingEmulator))
        .count() as u32;
    let errors = app
        .installed_games
        .iter()
        .filter(|game| matches!(game.install_state, InstallState::Error))
        .count() as u32;
    let total = app.installed_games.len() as u32;

    let mut counts = vec![];
    if installed > 0 {
        counts.push(("playable".to_string(), installed, StatColor::Success));
    }
    if missing_emu > 0 {
        counts.push(("need emulator".to_string(), missing_emu, StatColor::Warning));
    }
    if errors > 0 {
        counts.push(("errors".to_string(), errors, StatColor::Error));
    }
    if counts.is_empty() && total > 0 {
        counts.push(("installed".to_string(), total, StatColor::Muted));
    }

    HeaderStats {
        primary_counts: counts,
        secondary_info: if total > 0 {
            Some(format!("{total} total"))
        } else {
            None
        },
    }
}

fn build_browse_stats(app: &App) -> HeaderStats {
    let total = app.browse_items.len() as u32;

    let counts = if total > 0 {
        vec![("entries".to_string(), total, StatColor::Info)]
    } else {
        vec![]
    };

    HeaderStats {
        primary_counts: counts,
        secondary_info: if total > 0 {
            Some(format!("page {} • emu.land", app.browse_page))
        } else {
            None
        },
    }
}

/// System status line for header (compact, muted)
pub fn build_system_status(app: &App) -> String {
    let view = match app.viewport_mode {
        ViewportMode::Compact => "compact",
        ViewportMode::Standard => "standard",
        ViewportMode::Wide => "wide",
    };
    let protocol = app.terminal_caps.image_protocol.label().to_lowercase();
    format!("{view} view • {protocol} images")
}

pub fn build_hero_summary(
    game: Option<&GameEntry>,
    metadata: Option<&ResolvedMetadata>,
    focus: FocusPane,
    wide: bool,
) -> HeroSummary {
    if let Some(game) = game {
        let mut source_badges = vec![
            game.source_kind.badge().to_string(),
            game.install_state.badge().to_string(),
        ];
        if let Some(emulator) = game.emulator_kind {
            source_badges.push(emulator.label().to_string());
        }

        let title = metadata
            .map(|item| item.canonical_title.clone())
            .unwrap_or_else(|| game.title.clone());
        let metadata_line = if let Some(metadata) = metadata {
            let tags = if metadata.tags.is_empty() {
                "NO TAGS".to_string()
            } else {
                metadata.tags.join("  •  ")
            };
            format!(
                "{}  {:.0}%  {}",
                metadata.match_state.badge(),
                metadata.match_confidence * 100.0,
                tags
            )
        } else {
            "IMPORTED  0%  WAITING FOR IDENTIFICATION".to_string()
        };

        HeroSummary {
            title,
            platform: format!("{} / {}", game.platform, game.platform.short_label()),
            generation: game.generation.to_string(),
            badges: vec![
                game.platform.short_label().to_string(),
                game.generation.to_string(),
            ],
            source_badges,
            vibe_line: if let Some(metadata) = metadata {
                if metadata.genres.is_empty() {
                    game.vibe_tags
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("  •  ")
                } else {
                    metadata.genres.join("  •  ")
                }
            } else {
                game.vibe_tags
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("  •  ")
            },
            play_line: format!(
                "PLAYS {:>3}  LAST {}",
                game.play_count,
                game.last_played_at
                    .map(|value| value.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "NEVER".to_string())
            ),
            state_line: game.status_line(),
            metadata_line,
            path_line: game
                .rom_path
                .as_ref()
                .or(game.managed_path.as_ref())
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<not installed>".to_string()),
            hash_line: if wide || matches!(focus, FocusPane::Summary) {
                Some(game.hash.clone().unwrap_or_else(|| "<pending>".to_string()))
            } else {
                None
            },
        }
    } else {
        HeroSummary {
            title: "NO SELECTION".to_string(),
            platform: "Awaiting library input".to_string(),
            generation: "Millennials".to_string(),
            badges: vec!["NO ART".to_string()],
            source_badges: vec!["EMPTY".to_string()],
            vibe_line: "Simple  •  Tactile".to_string(),
            play_line: "PLAYS   0  LAST NEVER".to_string(),
            state_line: "Drop ROMs into a scan root or add a legal source with [a].".to_string(),
            metadata_line: "IMPORTED  0%  AWAITING LIBRARY INPUT".to_string(),
            path_line: "No payload on disk yet".to_string(),
            hash_line: None,
        }
    }
}
