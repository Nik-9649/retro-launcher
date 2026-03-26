use std::path::{Path, PathBuf};

use anyhow::Result;
use image::ImageReader;
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
};
use ratatui_image::{Resize, StatefulImage, picker::Picker, protocol::StatefulProtocol};

use crate::config::AppPaths;
use crate::models::{GameEntry, ResolvedMetadata};
use crate::terminal::{ImageProtocol, TerminalCapabilities};

#[derive(Debug, Clone)]
pub enum ArtworkSource {
    CompanionFile,
    CachedFile,
}

pub enum ArtworkState {
    Unsupported,
    Missing,
    Ready {
        source: ArtworkSource,
        path: PathBuf,
        protocol: StatefulProtocol,
    },
    Failed(String),
}

pub struct ArtworkController {
    picker: Option<Picker>,
    selected_key: Option<String>,
    selected_art_path: Option<PathBuf>,
    pub state: ArtworkState,
}

impl ArtworkController {
    pub fn unsupported() -> Self {
        Self {
            picker: None,
            selected_key: None,
            selected_art_path: None,
            state: ArtworkState::Missing,
        }
    }

    pub fn new(capabilities: TerminalCapabilities) -> Self {
        let picker = match capabilities.image_protocol {
            ImageProtocol::Unsupported => None,
            _ => Picker::from_query_stdio().ok(),
        };
        Self {
            picker,
            selected_key: None,
            selected_art_path: None,
            state: ArtworkState::Missing,
        }
    }

    pub fn sync_to_game(
        &mut self,
        paths: &AppPaths,
        game: Option<&GameEntry>,
        metadata: Option<&ResolvedMetadata>,
    ) {
        let next_id = game.map(|value| value.id.clone());
        let next_art_path = game
            .and_then(|game| resolve_artwork(paths, game, metadata).map(|(_, path)| path));
        if self.selected_key == next_id && self.selected_art_path == next_art_path {
            return;
        }
        self.selected_key = next_id;
        self.selected_art_path = next_art_path.clone();
        self.state = if let Some(game) = game {
            match next_art_path.map(|path| {
                let source = metadata
                    .and_then(|metadata| metadata.artwork.cached_path.as_ref())
                    .filter(|cached| *cached == &path)
                    .map(|_| ArtworkSource::CachedFile)
                    .or_else(|| {
                        game.rom_path
                            .as_ref()
                            .or(game.managed_path.as_ref())
                            .and_then(|rom_path| {
                                companion_candidates(rom_path)
                                    .into_iter()
                                    .find(|candidate| candidate == &path)
                                    .map(|_| ArtworkSource::CompanionFile)
                            })
                    })
                    .unwrap_or(ArtworkSource::CachedFile);
                (source, path)
            }) {
                Some((source, path)) => {
                    if let Some(picker) = &self.picker {
                        match load_protocol(picker, &path) {
                            Ok(protocol) => ArtworkState::Ready {
                                source,
                                path,
                                protocol,
                            },
                            Err(error) => ArtworkState::Failed(error.to_string()),
                        }
                    } else {
                        ArtworkState::Unsupported
                    }
                }
                None => ArtworkState::Missing,
            }
        } else {
            ArtworkState::Missing
        };
    }

    pub fn sync_to_path(&mut self, key: Option<String>, path: Option<&Path>) {
        let next_path = path.map(Path::to_path_buf);
        if self.selected_key == key && self.selected_art_path == next_path {
            return;
        }
        self.selected_key = key;
        self.selected_art_path = next_path.clone();
        self.state = match path {
            Some(path) => {
                if let Some(picker) = &self.picker {
                    match load_protocol(picker, path) {
                        Ok(protocol) => ArtworkState::Ready {
                            source: ArtworkSource::CachedFile,
                            path: path.to_path_buf(),
                            protocol,
                        },
                        Err(error) => ArtworkState::Failed(error.to_string()),
                    }
                } else {
                    ArtworkState::Unsupported
                }
            }
            None => ArtworkState::Missing,
        };
    }

    pub fn render(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        block: Block<'_>,
        fallback_lines: Vec<Line<'_>>,
        style: Style,
    ) {
        frame.render_widget(block.clone(), area);
        let inner = block.inner(area);
        match &mut self.state {
            ArtworkState::Ready { protocol, .. } => {
                frame.render_stateful_widget(
                    StatefulImage::default().resize(Resize::Scale(None)),
                    inner,
                    protocol,
                );
            }
            ArtworkState::Failed(message) => {
                let widget = Paragraph::new(vec![
                    Line::from(Span::styled("ARTWORK LOAD ERROR", style)),
                    Line::from(Span::raw("")),
                    Line::from(message.as_str()),
                ])
                .wrap(Wrap { trim: true });
                frame.render_widget(widget, inner);
            }
            ArtworkState::Unsupported | ArtworkState::Missing => {
                let widget = Paragraph::new(fallback_lines).wrap(Wrap { trim: true });
                frame.render_widget(widget, inner);
            }
        }
    }

    pub fn last_encoding_result(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn source_label(&self) -> &'static str {
        match &self.state {
            ArtworkState::Ready { source, .. } => match source {
                ArtworkSource::CompanionFile => "COMPANION ART",
                ArtworkSource::CachedFile => "CACHED ART",
            },
            ArtworkState::Unsupported => "TEXT FALLBACK",
            ArtworkState::Missing => "NO ART",
            ArtworkState::Failed(_) => "ART ERROR",
        }
    }

    pub fn path_label(&self) -> Option<String> {
        match &self.state {
            ArtworkState::Ready { source, path, .. } => {
                let prefix = match source {
                    ArtworkSource::CompanionFile => "LOCAL",
                    ArtworkSource::CachedFile => "CACHE",
                };
                Some(format!("{prefix} {}", path.display()))
            }
            _ => None,
        }
    }
}

fn load_protocol(picker: &Picker, path: &Path) -> Result<StatefulProtocol> {
    let image = ImageReader::open(path)?.decode()?;
    Ok(picker.new_resize_protocol(image))
}

fn resolve_artwork(
    paths: &AppPaths,
    game: &GameEntry,
    metadata: Option<&ResolvedMetadata>,
) -> Option<(ArtworkSource, PathBuf)> {
    if let Some(metadata) = metadata {
        if let Some(cached) = &metadata.artwork.cached_path {
            if cached.exists() {
                return Some((ArtworkSource::CachedFile, cached.clone()));
            }
        }
    }

    let rom_path = game.rom_path.as_ref().or(game.managed_path.as_ref());
    if let Some(rom_path) = rom_path {
        for candidate in companion_candidates(rom_path) {
            if candidate.exists() {
                return Some((ArtworkSource::CompanionFile, candidate));
            }
        }
    }

    let cache_dir = paths.data_dir.join("artwork");
    let stem = sanitize_stem(&game.id);
    for ext in ["png", "jpg", "jpeg", "bmp", "gif"] {
        let candidate = cache_dir.join(format!("{stem}.{ext}"));
        if candidate.exists() {
            return Some((ArtworkSource::CachedFile, candidate));
        }
    }
    None
}

fn companion_candidates(rom_path: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let stem = rom_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("cover");
    let dir = rom_path.parent().unwrap_or_else(|| Path::new("."));
    for ext in ["png", "jpg", "jpeg", "bmp", "gif"] {
        candidates.push(dir.join(format!("{stem}.{ext}")));
        candidates.push(dir.join(format!("{stem}-cover.{ext}")));
        candidates.push(dir.join(format!("{stem}_cover.{ext}")));
        candidates.push(dir.join(format!("cover.{ext}")));
        candidates.push(dir.join(format!("boxart.{ext}")));
    }
    candidates
}

fn sanitize_stem(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::models::{GenerationTag, InstallState, Platform, SourceKind, VibeTag};

    #[test]
    fn finds_companion_artwork() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("game.gba");
        let art = dir.path().join("game.png");
        std::fs::write(&rom, b"rom").unwrap();
        let png = image::DynamicImage::new_rgba8(2, 2);
        png.save(&art).unwrap();
        let game = GameEntry {
            id: "1".into(),
            title: "Game".into(),
            filename: Some("game.gba".into()),
            platform: Platform::GameBoyAdvance,
            generation: GenerationTag::Millennials,
            vibe_tags: vec![VibeTag::Tactile],
            source_kind: SourceKind::LocalScan,
            install_state: InstallState::Ready,
            managed_path: None,
            origin_url: None,
            origin_label: None,
            rom_path: Some(rom),
            hash: None,
            emulator_kind: None,
            checksum: None,
            size_bytes: None,
            play_count: 0,
            last_played_at: None,
            discovered_at: Utc::now(),
            updated_at: Utc::now(),
            source_refs: Vec::new(),
            error_message: None,
            progress: None,
        };
        let paths = AppPaths {
            config_dir: dir.path().join("cfg"),
            data_dir: dir.path().join("data"),
            downloads_dir: dir.path().join("dl"),
            db_path: dir.path().join("db.sqlite"),
            config_path: dir.path().join("config.toml"),
        };
        let resolved = resolve_artwork(&paths, &game, None).unwrap();
        assert!(matches!(resolved.0, ArtworkSource::CompanionFile));
    }
}
