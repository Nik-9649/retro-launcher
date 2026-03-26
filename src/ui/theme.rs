//! UI theme definitions and color schemes.
//!
//! This module provides the Theme struct which defines colors for different
//! terminal capability tiers (NoColor, Ansi16/256, TrueColor).

use ratatui::style::{Color, Modifier, Style};

use crate::app::App;
use crate::terminal::ColorTier;

/// Theme colors for the UI.
#[derive(Clone, Copy)]
pub struct Theme {
    pub fg: Color,
    pub muted: Color,
    pub emphasis: Color,
    pub bg: Color,
    pub surface: Color,
    pub overlay: Color,
    pub primary: Color,
    pub secondary: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub selection: Color,
}

impl Theme {
    /// Create a theme based on the app's terminal capabilities.
    pub fn from_app(app: &App) -> Self {
        match app.terminal_caps.color_tier {
            ColorTier::NoColor => Self {
                fg: Color::White,
                muted: Color::Gray,
                emphasis: Color::White,
                bg: Color::Black,
                surface: Color::Black,
                overlay: Color::Black,
                primary: Color::White,
                secondary: Color::Gray,
                success: Color::White,
                warning: Color::White,
                error: Color::White,
                selection: Color::White,
            },
            ColorTier::Ansi16 | ColorTier::Ansi256 => Self {
                fg: Color::White,
                muted: Color::DarkGray,
                emphasis: Color::White,
                bg: Color::Black,
                surface: Color::Black,
                overlay: Color::DarkGray,
                primary: Color::Cyan,
                secondary: Color::Green,
                success: Color::Green,
                warning: Color::Yellow,
                error: Color::Red,
                selection: Color::Cyan,
            },
            ColorTier::TrueColor => Self {
                fg: Color::Rgb(208, 230, 223),
                muted: Color::Rgb(97, 130, 118),
                emphasis: Color::Rgb(233, 247, 241),
                bg: Color::Rgb(5, 13, 10),
                surface: Color::Rgb(11, 24, 19),
                overlay: Color::Rgb(17, 33, 27),
                primary: Color::Rgb(111, 255, 232),
                secondary: Color::Rgb(160, 255, 170),
                success: Color::Rgb(160, 255, 170),
                warning: Color::Rgb(255, 194, 102),
                error: Color::Rgb(255, 120, 120),
                selection: Color::Rgb(15, 70, 76),
            },
        }
    }

    /// Get a border style based on focus state.
    pub fn border_style(&self, focused: bool) -> Style {
        Style::default().fg(if focused { self.primary } else { self.muted })
    }

    /// Get a pill style for tags/badges.
    pub fn pill(&self, label: &str, color: Color, background: Color) -> ratatui::text::Span {
        ratatui::text::Span::styled(
            format!(" {label} "),
            Style::default()
                .fg(color)
                .bg(background)
                .add_modifier(Modifier::BOLD),
        )
    }

    /// Get a tab pill style.
    pub fn tab_pill(&self, label: &str, active: bool) -> ratatui::text::Span {
        ratatui::text::Span::styled(
            format!(" {label} "),
            Style::default()
                .fg(if active { self.bg } else { self.muted })
                .bg(if active { self.primary } else { self.surface })
                .add_modifier(if active {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        )
    }
}

/// Get the status color for an install state.
pub fn status_color(state: crate::models::InstallState, theme: &Theme) -> Color {
    match state {
        crate::models::InstallState::Ready => theme.success,
        crate::models::InstallState::DownloadAvailable => theme.warning,
        crate::models::InstallState::Downloading | crate::models::InstallState::DownloadedNeedsImport => {
            theme.primary
        }
        crate::models::InstallState::MissingEmulator => theme.secondary,
        crate::models::InstallState::Unsupported => theme.muted,
        crate::models::InstallState::Error => theme.error,
    }
}
