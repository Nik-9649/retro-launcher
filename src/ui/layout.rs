//! UI layout utilities and constants.
//!
//! This module provides layout calculations and constants for the terminal UI.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Style, Modifier};
use ratatui::text::Span;
use ratatui::widgets::Block;

use crate::ui::theme::Theme;

/// Layout constants for the terminal UI.
pub const HEADER_HEIGHT: u16 = 5; // 3 content rows + 2 border rows
pub const FOOTER_HEIGHT: u16 = 3;
pub const MIN_DASHBOARD_HEIGHT: u16 = 19;
pub const MIN_TERMINAL_WIDTH: u16 = 80;
pub const MIN_TERMINAL_HEIGHT: u16 = 24;

/// Create a centered rectangle for overlays/popups.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

/// Create a panel block with title and focus styling.
pub fn panel_block<'a>(title: &'a str, focused: bool, theme: &Theme) -> Block<'a> {
    Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(if focused { theme.primary } else { theme.muted })
                .add_modifier(if focused {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ))
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(theme.border_style(focused))
        .style(Style::default().bg(theme.surface))
}

/// Create a pill/label span.
pub fn pill(label: &str, color: ratatui::style::Color, background: ratatui::style::Color) -> Span<'static> {
    Span::styled(
        format!(" {label} "),
        Style::default()
            .fg(color)
            .bg(background)
            .add_modifier(Modifier::BOLD),
    )
}

/// Create a row of pills from labels.
pub fn pill_row(labels: &[String], color: ratatui::style::Color, background: ratatui::style::Color) -> Span<'static> {
    Span::styled(
        labels
            .iter()
            .map(|label| format!("[{label}]"))
            .collect::<Vec<_>>()
            .join(" "),
        Style::default()
            .fg(color)
            .bg(background)
            .add_modifier(Modifier::BOLD),
    )
}

/// Create a centered line for display.
pub fn centered_line(text: &str, color: ratatui::style::Color) -> ratatui::text::Line<'static> {
    ratatui::text::Line::from(Span::styled(text.to_string(), Style::default().fg(color)))
        .alignment(ratatui::layout::Alignment::Center)
}

/// Create a mini progress bar.
pub fn mini_bar(progress: u8, width: usize) -> String {
    let filled = ((progress as usize * width) / 100).min(width);
    let mut output = String::from("[");
    output.push_str(&"█".repeat(filled));
    output.push_str(&"░".repeat(width - filled));
    output.push(']');
    output
}

/// Truncate a string to a maximum number of characters.
pub fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}
