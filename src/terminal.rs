use std::env;

use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Library,
    Artwork,
    Summary,
}

impl FocusPane {
    pub fn next(self) -> Self {
        match self {
            Self::Library => Self::Artwork,
            Self::Artwork => Self::Summary,
            Self::Summary => Self::Library,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Library => Self::Summary,
            Self::Artwork => Self::Library,
            Self::Summary => Self::Artwork,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Library => "LIBRARY",
            Self::Artwork => "ARTWORK",
            Self::Summary => "SUMMARY",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportMode {
    Compact,
    Standard,
    Wide,
}

impl ViewportMode {
    pub fn from_area(area: Rect) -> Self {
        if area.width < 120 || area.height < 32 {
            Self::Compact
        } else if area.width >= 160 {
            Self::Wide
        } else {
            Self::Standard
        }
    }

    pub fn minimum_supported(area: Rect) -> bool {
        area.width >= 80 && area.height >= 24
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorTier {
    NoColor,
    Ansi16,
    Ansi256,
    TrueColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageProtocol {
    Iterm2,
    Kitty,
    Unsupported,
}

impl ImageProtocol {
    pub fn label(self) -> &'static str {
        match self {
            Self::Iterm2 => "ITERM2",
            Self::Kitty => "KITTY",
            Self::Unsupported => "FALLBACK",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCapabilities {
    pub color_tier: ColorTier,
    pub image_protocol: ImageProtocol,
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        let color_tier = if env::var_os("NO_COLOR").is_some() {
            ColorTier::NoColor
        } else if matches!(
            env::var("COLORTERM").ok().as_deref(),
            Some("truecolor") | Some("24bit")
        ) {
            ColorTier::TrueColor
        } else if env::var("TERM")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("256color")
        {
            ColorTier::Ansi256
        } else {
            ColorTier::Ansi16
        };

        let image_protocol = if env::var("TERM_PROGRAM").ok().as_deref() == Some("iTerm.app") {
            ImageProtocol::Iterm2
        } else {
            let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
            let term_program = env::var("TERM_PROGRAM")
                .unwrap_or_default()
                .to_ascii_lowercase();
            if env::var_os("KITTY_WINDOW_ID").is_some()
                || term.contains("kitty")
                || term_program.contains("ghostty")
            {
                ImageProtocol::Kitty
            } else {
                ImageProtocol::Unsupported
            }
        };

        Self {
            color_tier,
            image_protocol,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_thresholds_work() {
        assert_eq!(
            ViewportMode::from_area(Rect::new(0, 0, 100, 30)),
            ViewportMode::Compact
        );
        assert_eq!(
            ViewportMode::from_area(Rect::new(0, 0, 130, 40)),
            ViewportMode::Standard
        );
        assert_eq!(
            ViewportMode::from_area(Rect::new(0, 0, 180, 50)),
            ViewportMode::Wide
        );
    }

    #[test]
    fn focus_cycles_both_directions() {
        assert_eq!(FocusPane::Library.next(), FocusPane::Artwork);
        assert_eq!(FocusPane::Library.previous(), FocusPane::Summary);
    }
}
