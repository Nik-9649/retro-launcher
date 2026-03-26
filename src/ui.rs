//! Terminal UI rendering module.
//!
//! This module handles all rendering of the terminal user interface.
//! Theme and layout utilities have been extracted to separate submodules.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

use crate::app::{AddSourceMode, App, AppTab};
use crate::emulator::Availability;
use crate::presentation::{build_header_stats, build_hero_summary, build_system_status, StatColor};
use crate::terminal::{FocusPane, ViewportMode};
use crate::toast::{AnimationState, Toast, ToastType};
use crate::ui::layout::{centered_rect, mini_bar, panel_block, pill, pill_row, truncate};
use crate::ui::theme::{status_color, Theme};

pub mod layout;
pub mod theme;

/// Highlight matching portions of text in a search result.
/// Returns a vector of spans with the matched portion styled differently.
fn highlight_match(text: &str, query: &str, theme: &Theme) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(
            format!("{:<22}", truncate(text, 22)),
            Style::default().fg(theme.fg),
        )];
    }

    let text_upper = text.to_ascii_uppercase();
    let query_upper = query.to_ascii_uppercase();
    let mut spans = Vec::new();
    let mut last_end = 0;
    let display_width = 22;

    // Find all matches and create spans
    for (start, _) in text_upper.match_indices(&query_upper) {
        // Add text before match
        if start > last_end {
            spans.push(Span::styled(
                text[last_end..start].to_string(),
                Style::default().fg(theme.fg),
            ));
        }
        // Add matched text with highlight
        let match_end = (start + query.len()).min(text.len());
        spans.push(Span::styled(
            text[start..match_end].to_string(),
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ));
        last_end = match_end;
    }

    // Add remaining text
    if last_end < text.len() {
        spans.push(Span::styled(
            text[last_end..].to_string(),
            Style::default().fg(theme.fg),
        ));
    }

    // Pad to fixed width if needed
    let current_len: usize = spans.iter().map(|s| s.content.len()).sum();
    if current_len < display_width {
        spans.push(Span::styled(
            " ".repeat(display_width - current_len),
            Style::default().fg(theme.fg),
        ));
    }

    spans
}

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    app.set_viewport_mode(ViewportMode::from_area(area));
    let theme = Theme::from_app(app);

    if !ViewportMode::minimum_supported(area) {
        render_too_small(frame, area, &theme);
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(layout::HEADER_HEIGHT), // Header
            Constraint::Min(layout::MIN_DASHBOARD_HEIGHT), // Dashboard
            Constraint::Length(layout::FOOTER_HEIGHT), // Footer
        ])
        .split(area);

    render_header(frame, layout[0], app, &theme);
    render_dashboard(frame, layout[1], app, &theme);
    render_footer(frame, layout[2], app, &theme);

    if app.show_help {
        render_help(frame, centered_rect(72, 70, area), &theme);
    }

    if app.add_source_mode.is_some() || app.search_mode {
        render_input_overlay(frame, centered_rect(60, 20, area), app, &theme);
    }

    if app.emu_land_search.is_some() {
        render_emu_land_search_overlay(frame, centered_rect(82, 60, area), app, &theme);
    }

    if app.add_url_preview.is_some() {
        render_url_preview_overlay(frame, centered_rect(72, 52, area), app, &theme);
    }

    if app.emulator_picker.is_some() {
        render_emulator_picker(frame, centered_rect(64, 46, area), app, &theme);
    }

    // Render toasts on top of everything
    render_toasts(frame, app, area, &theme);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let stats = build_header_stats(app);
    let system_status = build_system_status(app);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_style(app.focus_pane == FocusPane::Library))
        .style(Style::default().bg(theme.bg));

    // Calculate inner area first
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // 3-row layout: title+tabs, stats, system status
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    // Row 0: Title (left) + Tabs (right)
    let title_tabs_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(32)])
        .split(rows[0]);

    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            "RETRO TERMINAL ENGINE",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        )])),
        title_tabs_layout[0],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            theme.tab_pill("LIBRARY", app.active_tab == AppTab::Library),
            Span::raw(" "),
            theme.tab_pill("INSTALLED", app.active_tab == AppTab::Installed),
            Span::raw(" "),
            theme.tab_pill("BROWSE", app.active_tab == AppTab::Browse),
        ]))
        .alignment(Alignment::Right),
        title_tabs_layout[1],
    );

    // Row 1: Contextual stats (left-aligned, colored by semantic meaning)
    let stats_spans = build_stats_spans(&stats, theme);
    frame.render_widget(
        Paragraph::new(Line::from(stats_spans)),
        rows[1],
    );

    // Row 2: System status (right-aligned, muted)
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            system_status,
            Style::default().fg(theme.muted),
        )]))
        .alignment(Alignment::Right),
        rows[2],
    );
}

fn build_stats_spans(stats: &crate::presentation::HeaderStats, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = vec![];

    for (i, (label, count, color)) in stats.primary_counts.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let color = match color {
            StatColor::Success => theme.success,
            StatColor::Warning => theme.warning,
            StatColor::Error => theme.error,
            StatColor::Info => theme.primary,
            StatColor::Muted => theme.muted,
        };
        // Handle text-only labels (like "No games in library") vs count labels
        let text = if *count == 0 && label.chars().next().map(|c| c.is_alphabetic() && c.is_uppercase()).unwrap_or(false) {
            label.clone()
        } else {
            format!("{count} {label}")
        };
        spans.push(Span::styled(
            text,
            Style::default().fg(color),
        ));
    }

    if let Some(secondary) = &stats.secondary_info {
        if !spans.is_empty() {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            secondary.clone(),
            Style::default().fg(theme.muted),
        ));
    }

    spans
}

fn render_dashboard(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: &Theme) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(match app.viewport_mode {
            ViewportMode::Compact => [Constraint::Percentage(46), Constraint::Percentage(54)],
            ViewportMode::Standard => [Constraint::Percentage(42), Constraint::Percentage(58)],
            ViewportMode::Wide => [Constraint::Percentage(38), Constraint::Percentage(62)],
        })
        .split(area);

    render_library(frame, columns[0], app, theme);
    render_hero(frame, columns[1], app, theme);
}

fn render_library(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let (item_count, total_count) = match app.active_tab {
        AppTab::Library => (app.filtered_games.len(), app.all_games.len()),
        AppTab::Installed => (app.installed_games.len(), app.all_games.len()),
        AppTab::Browse => (app.browse_items.len(), app.browse_items.len()),
    };
    let title = if app.active_tab == AppTab::Browse {
        format!(" {} [{} TITLES] [PAGE {}] ", app.active_tab.label(), item_count, app.browse_page)
    } else if !app.search_query.is_empty() {
        format!(" {} [{}/{} TITLES] ", app.active_tab.label(), item_count, total_count)
    } else {
        format!(" {} [{} TITLES] ", app.active_tab.label(), item_count)
    };
    let block = panel_block(&title, app.focus_pane == FocusPane::Library, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Render filter badge if active
    if !app.search_query.is_empty() {
        let filter_text = format!("FILTER: {} ✕", app.search_query);
        let filter_pill = Span::styled(
            format!(" {filter_text} "),
            Style::default()
                .fg(theme.bg)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD),
        );
        let filter_line = Line::from(filter_pill).alignment(Alignment::Right);
        let filter_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        frame.render_widget(Paragraph::new(filter_line), filter_area);
    }

    let items: Vec<ListItem<'_>> = if item_count == 0 {
        vec![ListItem::new(Line::from(Span::styled(
            if app.active_tab == AppTab::Browse {
                "NO BROWSE ENTRIES LOADED YET"
            } else if !app.search_query.is_empty() {
                "NO MATCHES FOUND"
            } else {
                "NO ROMS INDEXED YET"
            },
            Style::default().fg(theme.muted),
        )))]
    } else if app.active_tab == AppTab::Browse {
        app.browse_items
            .iter()
            .map(|item| {
                let left = format!("{:<22}", truncate(&item.title.to_ascii_uppercase(), 22));
                let meta = format!(
                    "{} {}",
                    item.platform.short_label(),
                    item.downloads
                        .as_deref()
                        .map(|value| format!("TOP {value}"))
                        .unwrap_or_else(|| "TOP".to_string())
                );
                ListItem::new(Line::from(vec![
                    Span::styled(left, Style::default().fg(theme.fg)),
                    Span::raw(" "),
                    Span::styled(meta, Style::default().fg(theme.secondary)),
                ]))
            })
            .collect()
    } else {
        let games = if app.active_tab == AppTab::Installed {
            &app.installed_games
        } else {
            &app.filtered_games
        };
        let query_lower = app.search_query.to_ascii_lowercase();
        games.iter()
            .map(|game| {
                let title = app.display_title_for(game).to_ascii_uppercase();
                let left_spans = if query_lower.is_empty() {
                    vec![Span::styled(
                        format!("{:<22}", truncate(&title, 22)),
                        Style::default().fg(theme.fg),
                    )]
                } else {
                    highlight_match(&title, &query_lower, theme)
                };
                let meta = format!(
                    "{} {} {}",
                    game.platform.short_label(),
                    game.source_kind.badge(),
                    game.install_state.badge()
                );
                let mut spans = left_spans;
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    meta,
                    Style::default().fg(status_color(game.install_state, theme)),
                ));
                ListItem::new(Line::from(spans))
            })
            .collect()
    };

    let list = List::new(items).highlight_symbol("▌").highlight_style(
        Style::default()
            .bg(theme.selection)
            .fg(theme.emphasis)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED),
    );
    let mut state = ListState::default().with_selected(app.selection());
    frame.render_stateful_widget(list, inner, &mut state);
}

fn render_hero(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: &Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(match app.viewport_mode {
            ViewportMode::Compact => [Constraint::Length(12), Constraint::Min(10)],
            ViewportMode::Standard => [Constraint::Length(16), Constraint::Min(12)],
            ViewportMode::Wide => [Constraint::Length(18), Constraint::Min(12)],
        })
        .split(area);

    render_artwork(frame, rows[0], app, theme);
    if app.active_tab == AppTab::Browse {
        render_browse_summary(frame, rows[1], app, theme);
    } else {
        render_summary(frame, rows[1], app, theme);
    }
}

fn render_artwork(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: &Theme) {
    let selected = app.selected_game();
    let fallback = if let Some(game) = selected {
        vec![
            Line::from(""),
            layout::centered_line("╔══════════════╗", theme.muted),
            layout::centered_line("║    NO ART    ║", theme.warning),
            layout::centered_line("╚══════════════╝", theme.muted),
            Line::from(""),
            layout::centered_line(&game.title.to_ascii_uppercase(), theme.emphasis),
            layout::centered_line(game.platform.short_label(), theme.primary),
            layout::centered_line(game.source_kind.badge(), theme.secondary),
            Line::from(""),
            layout::centered_line(app.artwork.source_label(), theme.muted),
        ]
    } else if let Some(item) = app.selected_browse_item() {
        vec![
            Line::from(""),
            layout::centered_line("╔══════════════╗", theme.muted),
            layout::centered_line("║  BROWSE ART  ║", theme.warning),
            layout::centered_line("╚══════════════╝", theme.muted),
            Line::from(""),
            layout::centered_line(&item.title.to_ascii_uppercase(), theme.emphasis),
            layout::centered_line(item.platform.short_label(), theme.primary),
            layout::centered_line("EMU-LAND TOP", theme.secondary),
            Line::from(""),
            layout::centered_line(app.artwork.source_label(), theme.muted),
        ]
    } else {
        vec![
            Line::from(""),
            layout::centered_line("╔══════════════╗", theme.muted),
            layout::centered_line("║   NO GAME    ║", theme.warning),
            layout::centered_line("╚══════════════╝", theme.muted),
            Line::from(""),
            layout::centered_line("SELECT A TITLE", theme.emphasis),
            layout::centered_line("BOX ART WILL RENDER HERE", theme.muted),
        ]
    };

    let block = panel_block(" ARTWORK ", app.focus_pane == FocusPane::Artwork, theme);
    app.artwork
        .render(frame, area, block, fallback, Style::default().fg(theme.fg));
}

fn render_summary(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let wide = matches!(app.viewport_mode, ViewportMode::Wide);
    let cards = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if wide {
            [
                Constraint::Length(8),
                Constraint::Length(5),
                Constraint::Min(6),
            ]
        } else {
            [
                Constraint::Length(8),
                Constraint::Length(5),
                Constraint::Min(4),
            ]
        })
        .split(area);

    let selected_metadata = app
        .selected_game()
        .and_then(|game| app.metadata_for_game(&game.id));
    let summary = build_hero_summary(app.selected_game(), selected_metadata, app.focus_pane, wide);
    let top = panel_block(" SUMMARY ", app.focus_pane == FocusPane::Summary, theme);
    let top_inner = top.inner(cards[0]);
    frame.render_widget(top, cards[0]);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    summary.title,
                    Style::default()
                        .fg(theme.emphasis)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                pill(&summary.platform, theme.primary, theme.surface),
                Span::raw(" "),
                pill(&summary.generation, theme.secondary, theme.surface),
            ]),
            Line::from(vec![
                pill_row(&summary.badges, theme.primary, theme.surface),
                Span::raw(" "),
                pill_row(&summary.source_badges, theme.warning, theme.surface),
            ]),
            Line::from(Span::styled(
                summary.vibe_line,
                Style::default().fg(theme.fg),
            )),
            Line::from(Span::styled(
                summary.play_line,
                Style::default().fg(theme.muted),
            )),
            Line::from(Span::styled(
                summary.metadata_line,
                Style::default().fg(theme.secondary),
            )),
            Line::from(Span::styled(
                summary.state_line,
                Style::default().fg(theme.warning),
            )),
        ])
        .wrap(Wrap { trim: true }),
        top_inner,
    );

    let stats = panel_block(" RUNTIME ", false, theme);
    let stats_inner = stats.inner(cards[1]);
    frame.render_widget(stats, cards[1]);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!("ART  {}", app.artwork.source_label())),
            Line::from(format!(
                "LOAD {}",
                mini_bar(
                    app.selected_game()
                        .and_then(|game| game.progress)
                        .unwrap_or(if app.selected_game().is_some() {
                            100
                        } else {
                            0
                        }),
                    12
                )
            )),
            Line::from(format!(
                "STATE {}",
                app.selected_game()
                    .map(|game| game.install_state.badge())
                    .unwrap_or("EMPTY")
            )),
            Line::from("FLOW READY"),
        ])
        .wrap(Wrap { trim: true }),
        stats_inner,
    );

    let detail_title = if wide || matches!(app.focus_pane, FocusPane::Summary) {
        " TECHNICAL "
    } else {
        " FILE STATE "
    };
    let detail = panel_block(detail_title, false, theme);
    let detail_inner = detail.inner(cards[2]);
    frame.render_widget(detail, cards[2]);
    let mut lines = vec![Line::from(Span::styled(
        summary.path_line,
        Style::default().fg(theme.muted),
    ))];
    if let Some(hash) = summary.hash_line {
        lines.push(Line::from(Span::styled(
            format!("HASH {hash}"),
            Style::default().fg(theme.muted),
        )));
    }
    if let Some(path) = app.artwork.path_label() {
        lines.push(Line::from(Span::styled(
            path,
            Style::default().fg(theme.muted),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }),
        detail_inner,
    );
}

fn render_browse_summary(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let cards = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(5),
            Constraint::Min(5),
        ])
        .split(area);
    let top = panel_block(" BROWSE SUMMARY ", app.focus_pane == FocusPane::Summary, theme);
    let top_inner = top.inner(cards[0]);
    frame.render_widget(top, cards[0]);
    if let Some(item) = app.selected_browse_item() {
        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    item.title.as_str(),
                    Style::default()
                        .fg(theme.emphasis)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                pill(item.platform.short_label(), theme.primary, theme.surface),
                Span::raw(" "),
                pill("EMU-LAND TOP", theme.secondary, theme.surface),
            ]),
            Line::from(Span::styled(
                if item.genres.is_empty() {
                    "Top-ranked browse entry".to_string()
                } else {
                    item.genres.join("  •  ")
                },
                Style::default().fg(theme.fg),
            )),
        ];
        if let Some(dev) = &item.developer {
            lines.push(Line::from(Span::styled(
                format!("Developer: {dev}"),
                Style::default().fg(theme.muted),
            )));
        }
        if let Some(year) = &item.year {
            lines.push(Line::from(Span::styled(
                format!("Year: {year}"),
                Style::default().fg(theme.muted),
            )));
        }
        if let Some(downloads) = &item.downloads {
            lines.push(Line::from(Span::styled(
                format!("Downloads: {downloads}"),
                Style::default().fg(theme.secondary),
            )));
        }
        lines.push(Line::from(Span::styled(
            "Press Enter to preview and add this title.",
            Style::default().fg(theme.warning),
        )));
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), top_inner);
    } else {
        frame.render_widget(
            Paragraph::new("No browse selection").style(Style::default().fg(theme.muted)),
            top_inner,
        );
    }

    let stats = panel_block(" BROWSE RUNTIME ", false, theme);
    let stats_inner = stats.inner(cards[1]);
    frame.render_widget(stats, cards[1]);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!("ART  {}", app.artwork.source_label())),
            Line::from("LOAD [████████████]"),
            Line::from("STATE READY TO PREVIEW"),
            Line::from(Span::styled(
                "BROWSE READY",
                Style::default().fg(theme.muted),
            )),
        ])
        .wrap(Wrap { trim: true }),
        stats_inner,
    );

    let detail = panel_block(" BROWSE NOTES ", false, theme);
    let detail_inner = detail.inner(cards[2]);
    frame.render_widget(detail, cards[2]);
    let text = app
        .selected_browse_item()
        .and_then(|item| item.description.clone())
        .unwrap_or_else(|| "Select a browse entry to inspect its description before previewing.".to_string());
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(theme.muted))
            .wrap(Wrap { trim: true }),
        detail_inner,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let footer = Paragraph::new(vec![Line::from(Span::styled(
        app.footer_hint(),
        Style::default().fg(theme.fg),
    ))])
    .style(Style::default().fg(theme.fg).bg(theme.surface))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.muted)),
    );
    frame.render_widget(footer, area);
}

fn render_help(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    frame.render_widget(Clear, area);
    let help = Paragraph::new(vec![
        Line::from(Span::styled(
            "KEYBOARD CONTROLS",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("j / k / arrows   Move selection"),
        Line::from("Tab / Shift+Tab  Cycle focus zones"),
        Line::from("h / l            Move focus left/right"),
        Line::from("p / n            Browse prev/next page (Browse tab)"),
        Line::from("/                Search titles"),
        Line::from("a                Add URL or import manifest"),
        Line::from("Enter            Launch / download / picker / retry"),
        Line::from("?                Toggle help"),
        Line::from("q / Esc          Quit or close overlay"),
    ])
    .style(Style::default().fg(theme.fg).bg(theme.overlay))
    .block(panel_block(" HELP ", true, theme))
    .wrap(Wrap { trim: true });
    frame.render_widget(help, area);
}

fn render_emulator_picker(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let Some(picker) = app.emulator_picker.as_ref() else {
        return;
    };
    frame.render_widget(Clear, area);

    let mut items: Vec<ListItem<'_>> = picker
        .candidates
        .iter()
        .map(|candidate| {
            let badge = match candidate.availability {
                Availability::Installed => "INSTALLED",
                Availability::Downloadable => "DOWNLOAD",
                Availability::Unavailable => "UNAVAILABLE",
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<12}", candidate.emulator.label()),
                    Style::default().fg(theme.emphasis),
                ),
                Span::raw(" "),
                Span::styled(
                    badge,
                    Style::default().fg(match candidate.availability {
                        Availability::Installed => theme.success,
                        Availability::Downloadable => theme.warning,
                        Availability::Unavailable => theme.error,
                    }),
                ),
            ]))
        })
        .collect();
    items.push(ListItem::new(Line::from(vec![Span::styled(
        "Cancel",
        Style::default().fg(theme.muted),
    )])));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(6), Constraint::Min(4)])
        .split(area);

    let block = panel_block(" EMULATOR PICKER ", true, theme);
    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                format!("Choose emulator for {}", picker.title),
                Style::default().fg(theme.primary).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Enter confirms. Esc closes without launching.",
                Style::default().fg(theme.muted),
            )),
        ]),
        rows[0],
    );

    let mut state = ListState::default().with_selected(Some(picker.selected.min(items.len() - 1)));
    let list = List::new(items).highlight_symbol("▌").highlight_style(
        Style::default()
            .bg(theme.selection)
            .fg(theme.emphasis)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, rows[1], &mut state);

    let note = if picker.selected >= picker.candidates.len() {
        "Cancel and return to the dashboard.".to_string()
    } else {
        picker.candidates[picker.selected].note.clone()
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(note, Style::default().fg(theme.fg))),
        ])
        .wrap(Wrap { trim: true }),
        Rect {
            x: inner.x,
            y: rows[2].y,
            width: inner.width,
            height: rows[2].height,
        },
    );
}

fn render_input_overlay(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    frame.render_widget(Clear, area);
    let title = if app.search_mode {
        " SEARCH "
    } else if matches!(app.add_source_mode, Some(AddSourceMode::Choose)) {
        " ADD SOURCE "
    } else if matches!(app.add_source_mode, Some(AddSourceMode::Url)) {
        " ADD URL "
    } else if matches!(app.add_source_mode, Some(AddSourceMode::EmuLandSearch)) {
        " SEARCH EMU-LAND "
    } else {
        " IMPORT MANIFEST "
    };
    let body = if app.search_mode {
        let input_text = if app.input_buffer.is_empty() {
            Span::styled(
                "type to filter...",
                Style::default().fg(theme.muted).add_modifier(Modifier::ITALIC),
            )
        } else {
            Span::styled(
                format!("{}{}", app.input_buffer, "▌"),
                Style::default()
                    .fg(theme.emphasis)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )
        };
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("/  ", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
                input_text,
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Enter", Style::default().fg(theme.primary)),
                Span::styled(" to search  ", Style::default().fg(theme.muted)),
                Span::styled("•", Style::default().fg(theme.muted)),
                Span::styled("  Esc", Style::default().fg(theme.primary)),
                Span::styled(" to cancel", Style::default().fg(theme.muted)),
            ]),
        ]
    } else {
        match app.add_source_mode {
            Some(AddSourceMode::Choose) => vec![
                Line::from("1  ADD DIRECT URL"),
                Line::from("2  SEARCH EMU-LAND"),
                Line::from("3  IMPORT JSON/TOML MANIFEST"),
                Line::from("Esc  CANCEL"),
            ],
            Some(AddSourceMode::Url) => vec![
                Line::from("FORMAT  URL|TITLE(optional)|PLATFORM(optional)"),
                Line::from(""),
                Line::from(Span::styled(
                    app.input_buffer.as_str(),
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                )),
            ],
            Some(AddSourceMode::EmuLandSearch) => vec![
                Line::from("SEARCH CURRENTLY LAUNCHABLE EMU-LAND PLATFORMS"),
                Line::from(""),
                Line::from(Span::styled(
                    app.input_buffer.as_str(),
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                )),
            ],
            Some(AddSourceMode::Manifest) => vec![
                Line::from("ENTER MANIFEST PATH (.json or .toml)"),
                Line::from(""),
                Line::from(Span::styled(
                    app.input_buffer.as_str(),
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                )),
            ],
            None => Vec::new(),
        }
    };
    let overlay = Paragraph::new(body)
        .style(Style::default().fg(theme.fg).bg(theme.overlay))
        .block(panel_block(title, true, theme))
        .wrap(Wrap { trim: true });
    frame.render_widget(overlay, area);
}

fn render_emu_land_search_overlay(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: &Theme) {
    let Some(state) = app.emu_land_search.as_ref() else {
        return;
    };
    frame.render_widget(Clear, area);
    let block = panel_block(" EMU-LAND RESULTS ", true, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(8)])
        .split(inner);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(rows[1]);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!("QUERY {}", state.query)),
            Line::from(format!("{} RESULT(S)", state.results.len())),
        ])
        .style(Style::default().fg(theme.muted)),
        rows[0],
    );
    let items = if state.results.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "NO MATCHES",
            Style::default().fg(theme.warning),
        )))]
    } else {
        state
            .results
            .iter()
            .map(|result| {
                let subtitle = if result.genres.is_empty() {
                    result
                        .players
                        .map(|players| format!("{}  Players: {players}", result.platform.display_name()))
                        .unwrap_or_else(|| result.platform.display_name().to_string())
                } else {
                    let mut subtitle =
                        format!("{}  {}", result.platform.display_name(), result.genres.join(" • "));
                    if let Some(players) = result.players {
                        subtitle.push_str(&format!("  Players: {players}"));
                    }
                    subtitle
                };
                ListItem::new(vec![
                    Line::from(Span::styled(
                        result.title.as_str(),
                        Style::default().fg(theme.emphasis),
                    )),
                    Line::from(Span::styled(
                        subtitle,
                        Style::default().fg(theme.muted),
                    )),
                ])
            })
            .collect()
    };
    let list = List::new(items).highlight_symbol("▌").highlight_style(
        Style::default()
            .bg(theme.selection)
            .fg(theme.emphasis)
            .add_modifier(Modifier::BOLD),
    );
    let mut list_state = ListState::default().with_selected(Some(
        state.selected.min(state.results.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, columns[0], &mut list_state);

    let selected = state.results.get(state.selected);
    let fallback = if let Some(result) = selected {
        vec![
            Line::from(""),
            layout::centered_line("╔══════════════╗", theme.muted),
            layout::centered_line("║ SEARCH ART   ║", theme.warning),
            layout::centered_line("╚══════════════╝", theme.muted),
            Line::from(""),
            layout::centered_line(result.title.as_str(), theme.emphasis),
            layout::centered_line(result.platform.display_name(), theme.primary),
            layout::centered_line(
                if result.preview_image_url.is_some() {
                    "REMOTE PREVIEW"
                } else {
                    "NO PREVIEW IMAGE"
                },
                theme.muted,
            ),
        ]
    } else {
        vec![layout::centered_line("NO RESULT SELECTED", theme.muted)]
    };
    let art_block = panel_block(" RESULT PREVIEW ", false, theme);
    app.preview_artwork.render(
        frame,
        columns[1],
        art_block,
        fallback,
        Style::default().fg(theme.fg),
    );
}

fn render_url_preview_overlay(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: &Theme) {
    let Some(state) = app.add_url_preview.as_ref() else {
        return;
    };
    frame.render_widget(Clear, area);
    let preview = &state.preview;
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);
    let fallback = vec![
        Line::from(""),
        layout::centered_line("╔══════════════╗", theme.muted),
        layout::centered_line("║ PREVIEW ART  ║", theme.warning),
        layout::centered_line("╚══════════════╝", theme.muted),
        Line::from(""),
        layout::centered_line(preview.resolved_title.as_str(), theme.emphasis),
        layout::centered_line(
            preview
                .selected_variant
                .as_deref()
                .unwrap_or("Default"),
            theme.warning,
        ),
        layout::centered_line(
            if preview.cached_artwork_path.is_some() {
                "CACHED"
            } else {
                "NO ART"
            },
            theme.muted,
        ),
    ];
    let art_block = panel_block(" PREVIEW ART ", true, theme);
    app.preview_artwork.render(
        frame,
        columns[0],
        art_block,
        fallback,
        Style::default().fg(theme.fg),
    );

    let actions = ["ADD TO LIBRARY", "DISCARD"];
    let action_lines = actions
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let selected = state.selected == index;
            Line::from(Span::styled(
                format!(" {} ", label),
                Style::default()
                    .fg(if selected { theme.emphasis } else { theme.muted })
                    .bg(if selected { theme.selection } else { theme.overlay })
                    .add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ))
        })
        .collect::<Vec<_>>();

    let mut body = vec![
        Line::from(vec![
            Span::styled("TITLE ", Style::default().fg(theme.muted)),
            Span::styled(
                preview.resolved_title.as_str(),
                Style::default().fg(theme.emphasis).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("PLATFORM ", Style::default().fg(theme.muted)),
            Span::styled(
                preview.entry.platform.display_name(),
                Style::default().fg(theme.primary),
            ),
        ]),
        Line::from(vec![
            Span::styled("SOURCE ", Style::default().fg(theme.muted)),
            Span::styled(preview.provider_label.as_str(), Style::default().fg(theme.secondary)),
            Span::raw(" "),
            Span::styled(
                format!("{}%", preview.confidence),
                Style::default().fg(theme.warning),
            ),
        ]),
        Line::from(vec![
            Span::styled("FILE ", Style::default().fg(theme.muted)),
            Span::styled(preview.selected_file.as_str(), Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("ENTITY ", Style::default().fg(theme.muted)),
            Span::styled(
                preview.entity_id.as_deref().unwrap_or("N/A"),
                Style::default().fg(theme.secondary),
            ),
            Span::raw(" "),
            Span::styled("FID ", Style::default().fg(theme.muted)),
            Span::styled(
                preview.selected_fid.as_deref().unwrap_or("AUTO"),
                Style::default().fg(theme.warning),
            ),
        ]),
        Line::from(vec![
            Span::styled("VARIANT ", Style::default().fg(theme.muted)),
            Span::styled(
                preview
                    .selected_variant
                    .as_deref()
                    .unwrap_or("Default"),
                Style::default().fg(theme.warning),
            ),
        ]),
        Line::from(vec![
            Span::styled("ART ", Style::default().fg(theme.muted)),
            Span::styled(
                if preview.artwork_url.is_some() {
                    "FOUND"
                } else {
                    "NONE"
                },
                Style::default().fg(if preview.artwork_url.is_some() {
                    theme.success
                } else {
                    theme.warning
                }),
            ),
        ]),
    ];
    if !preview.genres.is_empty() {
        body.push(Line::from(format!("GENRES {}", preview.genres.join("  •  "))));
    }
    if !preview.available_variants.is_empty() {
        body.push(Line::from(format!(
            "VARIANTS {}",
            truncate(&preview.available_variants.join("  •  "), 70)
        )));
    }
    if !preview.tags.is_empty() {
        body.push(Line::from(format!("TAGS {}", truncate(&preview.tags.join("  •  "), 70))));
    }
    body.push(Line::from(format!(
        "TARGET {}",
        truncate(&preview.final_url, 70)
    )));
    if let Some(warning) = &preview.warning {
        body.push(Line::from(Span::styled(
            warning.as_str(),
            Style::default().fg(theme.warning),
        )));
    }
    body.push(Line::from(""));
    body.extend(action_lines);

    let overlay = Paragraph::new(body)
        .style(Style::default().fg(theme.fg).bg(theme.overlay))
        .block(panel_block(" URL PREVIEW ", true, theme))
        .wrap(Wrap { trim: true });
    frame.render_widget(overlay, columns[1]);
}

fn render_too_small(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let message = Paragraph::new(vec![
        Line::from("TERMINAL TOO SMALL"),
        Line::from(format!(
            "RESIZE TO AT LEAST {}x{}",
            layout::MIN_TERMINAL_WIDTH,
            layout::MIN_TERMINAL_HEIGHT
        )),
    ])
    .alignment(Alignment::Center)
    .style(Style::default().fg(theme.warning).bg(theme.bg))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.warning)),
    );
    frame.render_widget(message, area);
}

fn render_toasts(frame: &mut Frame<'_>, app: &App, area: Rect, theme: &Theme) {
    let toasts = app.toast_manager.toasts();
    if toasts.is_empty() {
        return;
    }

    // Position: top-right, stacked vertically
    let toast_height = 3u16;
    let max_visible = 5usize;
    let visible_toasts: Vec<_> = toasts.iter().take(max_visible).collect();
    let total_height = (visible_toasts.len() as u16 * toast_height).min(area.height / 3);

    let toast_area = Rect {
        x: area.x.saturating_add(area.width).saturating_sub(42),
        y: area.y + 1,
        width: 40,
        height: total_height,
    };

    // Render each toast as a compact banner
    for (i, toast) in visible_toasts.iter().enumerate() {
        let y_offset = i as u16 * toast_height;
        render_single_toast(frame, toast, toast_area, y_offset, theme);
    }
}

fn render_single_toast(
    frame: &mut Frame<'_>,
    toast: &Toast,
    container: Rect,
    y_offset: u16,
    theme: &Theme,
) {
    // Calculate slide offset based on animation state
    let slide_offset = match toast.animation_state {
        AnimationState::SlidingIn { progress } => {
            // Ease-out: starts fast, slows down
            let eased = 1.0 - (1.0 - progress).powi(3);
            ((1.0 - eased) * container.width as f32) as u16
        }
        AnimationState::Visible => 0,
        AnimationState::SlidingOut { progress } => {
            // Ease-in: starts slow, speeds up
            let eased = progress.powi(3);
            (eased * container.width as f32) as u16
        }
    };

    let toast_area = Rect {
        x: container.x + slide_offset,
        y: container.y + y_offset,
        width: container.width.saturating_sub(slide_offset),
        height: 3,
    };

    // Get colors based on toast type
    let (icon, color) = match toast.toast_type {
        ToastType::Info => ("ℹ", theme.primary),
        ToastType::Success => ("✓", theme.success),
        ToastType::Warning => ("⚠", theme.warning),
        ToastType::Error => ("✗", theme.error),
    };

    // Render toast block
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .style(Style::default().bg(theme.surface));

    frame.render_widget(block.clone(), toast_area);

    // Render toast content
    let inner = block.inner(toast_area);
    if inner.width > 4 && inner.height > 0 {
        let content = Paragraph::new(Line::from(vec![
            Span::styled(format!(" {icon} "), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled(truncate(&toast.message, inner.width as usize - 4), Style::default().fg(theme.fg)),
        ]));
        frame.render_widget(content, inner);
    }
}
