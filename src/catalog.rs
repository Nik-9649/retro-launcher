use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use url::Url;

use crate::config::AppPaths;
use crate::metadata;
use crate::models::{CatalogEntry, MetadataMatchState, Platform, SourceKind};

#[derive(Debug, Clone)]
pub struct UserUrlPreview {
    pub entry: CatalogEntry,
    pub resolved_title: String,
    pub entity_id: Option<String>,
    pub selected_fid: Option<String>,
    pub selected_file: String,
    pub selected_variant: Option<String>,
    pub available_variants: Vec<String>,
    pub match_state: MetadataMatchState,
    pub provider_ids: Vec<String>,
    pub provider_label: String,
    pub confidence: u8,
    pub tags: Vec<String>,
    pub genres: Vec<String>,
    pub artwork_url: Option<String>,
    pub cached_artwork_path: Option<PathBuf>,
    pub final_url: String,
    pub warning: Option<String>,
    pub unmatched_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EmuLandSearchResult {
    pub title: String,
    pub href: String,
    pub platform: Platform,
    pub preview_image_url: Option<String>,
    pub genres: Vec<String>,
    pub players: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct EmuLandBrowseItem {
    pub title: String,
    pub href: String,
    pub platform: Platform,
    pub preview_image_url: Option<String>,
    pub genres: Vec<String>,
    pub players: Option<u8>,
    pub developer: Option<String>,
    pub year: Option<String>,
    pub downloads: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EmuLandVariant {
    pub fid: String,
    pub label: String,
    pub variant: String,
    pub exact_url: String,
}

#[derive(Debug, Clone)]
pub struct EmuLandEntity {
    pub id: String,
    pub title: String,
    pub platform: Platform,
    pub variants: Vec<EmuLandVariant>,
}

pub fn load_catalog(manifest_paths: &[impl AsRef<Path>]) -> Result<Vec<CatalogEntry>> {
    let mut entries = Vec::new();
    for path in manifest_paths {
        let path = path.as_ref();
        if path.exists() {
            let raw = fs::read_to_string(path)
                .with_context(|| format!("failed to read manifest {}", path.display()))?;
            let mut manifest_entries: Vec<CatalogEntry> =
                match path.extension().and_then(|ext| ext.to_str()) {
                    Some("toml") => toml::from_str(&raw)?,
                    _ => serde_json::from_str(&raw)?,
                };
            entries.append(&mut manifest_entries);
        }
    }
    for entry in &mut entries {
        entry.url = normalize_download_url(&entry.url);
    }
    Ok(entries)
}

pub fn parse_user_url(input: &str) -> Option<CatalogEntry> {
    let mut parts = input.split('|').map(str::trim);
    let raw_url = parts.next()?;
    let url = normalize_download_url(raw_url);
    if url.is_empty() {
        return None;
    }
    let parsed_url = Url::parse(&url).ok();
    let title = parts
        .next()
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| parsed_url.as_ref().and_then(infer_title_from_url))
        .unwrap_or_else(|| "Downloaded Game".to_string());
    let platform_hint = parts.next().unwrap_or_default().to_ascii_lowercase();
    let filename = parsed_url
        .as_ref()
        .map(infer_filename_from_url)
        .unwrap_or_else(|| "download.rom".to_string());
    let platform = match platform_hint.as_str() {
        "gb" | "gameboy" => Platform::GameBoy,
        "gbc" => Platform::GameBoyColor,
        "gba" => Platform::GameBoyAdvance,
        "nes" => Platform::Nes,
        "ps1" | "cue" | "chd" => Platform::Ps1,
        "" => infer_platform_from_url(parsed_url.as_ref(), &filename),
        _ => Platform::Unknown,
    };

    Some(CatalogEntry {
        title,
        url,
        platform,
        filename,
        checksum: None,
        legal_label: "User Source".to_string(),
        source_kind: SourceKind::UserUrl,
        developer: None,
        publisher: None,
        year: None,
        players: None,
        genres: None,
        artwork_url: None,
    })
}

pub fn preview_user_url(input: &str, paths: &AppPaths) -> Result<UserUrlPreview> {
    preview_user_url_inner(input, paths, false)
}

fn preview_user_url_inner(input: &str, paths: &AppPaths, skip_emu_land: bool) -> Result<UserUrlPreview> {
    let mut entry = parse_user_url(input).context("invalid URL entry")?;
    let entity_hint = resolve_emu_land_entity_hint(&entry.url)?;
    if let Some(hint) = &entity_hint {
        entry.url = hint.selected_variant.exact_url.clone();
        entry.filename = hint.selected_variant.label.clone();
        entry.title = hint.entity.title.clone();
    }
    let resolved = resolve_preview_target(&entry.url)?;
    let inferred_title = entity_hint
        .as_ref()
        .map(|hint| hint.entity.title.clone())
        .or_else(|| infer_title_from_download_target(&resolved.final_url, &entry.title))
        .unwrap_or_else(|| entry.title.clone());
    let platform = if entry.platform == Platform::Unknown {
        entity_hint
            .as_ref()
            .map(|hint| hint.entity.platform)
            .unwrap_or_else(|| {
                infer_platform_from_url(
                    Url::parse(&resolved.original_url).ok().as_ref(),
                    &resolved.filename,
                )
            })
    } else {
        entry.platform
    };
    entry.platform = platform;
    entry.filename = entity_hint
        .as_ref()
        .map(|hint| hint.selected_variant.label.clone())
        .unwrap_or_else(|| resolved.filename.clone());

    let matched = if skip_emu_land {
        metadata::preview_metadata_match_local(&inferred_title, platform, Some(&entry.url))?
    } else {
        metadata::preview_metadata_match(&inferred_title, platform, Some(&entry.url))?
    };
    let (
        resolved_title,
        match_state,
        provider_ids,
        provider_label,
        confidence,
        tags,
        genres,
        artwork_url,
        warning,
        unmatched_reason,
    ) =
        if let Some(matched) = matched {
            let mut warning = if matches!(
                matched.match_state,
                MetadataMatchState::Unmatched | MetadataMatchState::Ambiguous
            ) {
                Some("Metadata match is weak. Discard if this does not look right.".to_string())
            } else {
                None
            };
            if let Some(hint) = &entity_hint {
                if normalize_preview_title(&hint.selected_variant.label)
                    != normalize_preview_title(&matched.canonical_title)
                {
                    warning = Some(format!(
                        "Selected file resolves to '{}' but entity metadata resolves to '{}'. Discard unless this variant is intentional.",
                        hint.selected_variant.label, matched.canonical_title
                    ));
                }
            }
            (
                matched.canonical_title,
                matched.match_state,
                matched.provider_ids.clone(),
                matched.provider_ids.join(", "),
                (matched.match_confidence * 100.0).round() as u8,
                matched.tags,
                matched.genres,
                matched.artwork_url,
                warning,
                matched.unmatched_reason,
            )
        } else {
            (
                inferred_title.clone(),
                MetadataMatchState::Unmatched,
                vec!["preflight".to_string()],
                "preflight".to_string(),
                0,
                Vec::new(),
                Vec::new(),
                None,
                Some("Could not resolve metadata. Discard if this link looks wrong.".to_string()),
                Some("Could not resolve metadata before adding source.".to_string()),
            )
        };

    entry.title = resolved_title.clone();
    let cached_artwork_path = artwork_url
        .as_ref()
        .and_then(|url| cache_preview_artwork(paths, url, &resolved_title).ok());

    Ok(UserUrlPreview {
        entry,
        resolved_title,
        entity_id: entity_hint.as_ref().map(|hint| hint.entity.id.clone()),
        selected_fid: entity_hint
            .as_ref()
            .map(|hint| hint.selected_variant.fid.clone()),
        selected_file: entity_hint
            .as_ref()
            .map(|hint| hint.selected_variant.label.clone())
            .unwrap_or_else(|| resolved.filename.clone()),
        selected_variant: entity_hint
            .as_ref()
            .map(|hint| hint.selected_variant.variant.clone()),
        available_variants: entity_hint
            .as_ref()
            .map(|hint| {
                hint.entity
                    .variants
                    .iter()
                    .map(|variant| variant.variant.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        match_state,
        provider_ids,
        provider_label,
        confidence,
        tags,
        genres,
        artwork_url,
        cached_artwork_path,
        final_url: resolved.final_url,
        warning,
        unmatched_reason,
    })
}

pub fn search_emu_land(query: &str) -> Result<Vec<EmuLandSearchResult>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36")
        .build()?;
    let html = client
        .get(format!(
            "https://www.emu-land.net/en/search_games?id=all&genre=--&players=--&q={}",
            urlencoding::encode(query)
        ))
        .header("Referer", "https://www.emu-land.net/en/search_games")
        .header("Cookie", "lang=en; acceptCookie=true")
        .send()
        .context("emu-land search failed")?
        .text()
        .context("emu-land search decode failed")?;
    Ok(parse_emu_land_search_results(&html)
        .into_iter()
        .filter(|result| matches!(
            result.platform,
            Platform::Nes | Platform::GameBoy | Platform::GameBoyColor | Platform::GameBoyAdvance | Platform::Ps1
        ))
        .collect())
}

pub fn preview_emu_land_search_result(
    result: &EmuLandSearchResult,
    paths: &AppPaths,
) -> Result<UserUrlPreview> {
    let detail_html = fetch_emu_land_detail_html(&result.href)?;
    let base_download_url = extract_emu_land_base_download_url(&detail_html)
        .context("emu-land entity has no downloadable variants")?;
    let full_url = format!("https://www.emu-land.net{}", html_decode(&base_download_url));
    let mut preview = preview_user_url_inner(&full_url, paths, true)?;
    if preview.artwork_url.is_none() {
        preview.artwork_url = result.preview_image_url.clone();
        preview.cached_artwork_path = preview
            .artwork_url
            .as_ref()
            .and_then(|url| cache_preview_artwork(paths, url, &preview.resolved_title).ok());
    }
    if preview.genres.is_empty() {
        preview.genres = result.genres.clone();
    }
    Ok(preview)
}

pub fn load_emu_land_top(page: usize) -> Result<Vec<EmuLandBrowseItem>> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(15))
        .build()?;
    let url = if page <= 1 {
        "https://www.emu-land.net/en/consoles/dendy/roms/top".to_string()
    } else {
        format!("https://www.emu-land.net/en/consoles/dendy/roms/top/{page}")
    };
    let html = client
        .get(&url)
        .header("Referer", "https://www.emu-land.net/en/consoles/dendy/roms")
        .header("Cookie", "lang=en; acceptCookie=true")
        .send()
        .context("emu-land top page failed")?
        .text()
        .context("emu-land top decode failed")?;
    let items = parse_emu_land_top_results(&html);
    anyhow::ensure!(
        !items.is_empty(),
        "emu-land top page returned no browse entries"
    );
    Ok(items)
}

pub fn cache_search_result_artwork(
    paths: &AppPaths,
    title: &str,
    remote_url: &str,
) -> Result<PathBuf> {
    cache_preview_artwork(paths, remote_url, title)
}

fn infer_title_from_url(url: &Url) -> Option<String> {
    let leaf = url.path_segments()?.next_back()?;
    let leaf = leaf.trim();
    if leaf.is_empty() || matches!(leaf, "roms" | "games" | "download") {
        return None;
    }
    if leaf.contains('.') {
        return Some(leaf.to_string());
    }
    Some(
        leaf.replace('-', " ")
            .replace('_', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn infer_filename_from_url(url: &Url) -> String {
    let leaf = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .unwrap_or_default()
        .trim();
    if leaf.contains('.') && !leaf.is_empty() {
        return leaf.to_string();
    }
    if is_emu_land_download(url) {
        return "download.zip".to_string();
    }
    "download.rom".to_string()
}

fn infer_platform_from_url(url: Option<&Url>, filename: &str) -> Platform {
    let by_extension = filename
        .rsplit('.')
        .next()
        .map(Platform::from_extension)
        .unwrap_or(Platform::Unknown);
    if by_extension != Platform::Unknown {
        return by_extension;
    }
    let Some(url) = url else {
        return Platform::Unknown;
    };
    let path = url.path().to_ascii_lowercase();
    if path.contains("/consoles/dendy/") {
        Platform::Nes
    } else if path.contains("/consoles/super_nintendo/") {
        Platform::Snes
    } else if path.contains("/consoles/gba/") {
        Platform::GameBoyAdvance
    } else if path.contains("/portable/gameboy/") || path.contains("/consoles/gameboy/") {
        Platform::GameBoy
    } else if path.contains("/portable/gameboy_color/") || path.contains("/consoles/gbc/") {
        Platform::GameBoyColor
    } else if path.contains("/consoles/psx/") {
        Platform::Ps1
    } else if path.contains("/portable/nds/") || path.contains("/consoles/nds/") {
        Platform::NintendoDs
    } else if path.contains("/consoles/n64/") {
        Platform::N64
    } else if path.contains("/consoles/genesis/") {
        Platform::SegaGenesis
    } else {
        Platform::Unknown
    }
}

fn is_emu_land_download(url: &Url) -> bool {
    url.host_str() == Some("www.emu-land.net")
        && url
            .query_pairs()
            .any(|(key, value)| key == "act" && value == "getmfl")
}

fn parse_emu_land_search_results(html: &str) -> Vec<EmuLandSearchResult> {
    let mut results = Vec::new();
    for chunk in html.split(r#"<div class="fcontainer">"#).skip(1) {
        let mut rest = chunk;
        while let Some(anchor_idx) = rest.find(r#"<a href=""#) {
            let after_anchor = &rest[anchor_idx + 9..];
            let Some(href_end) = after_anchor.find('"') else {
                break;
            };
            let href = &after_anchor[..href_end];
            rest = &after_anchor[href_end..];
            if !href.contains("/roms/") && !href.contains("/games/") {
                continue;
            }
            let open_tag = format!(r#"<a href="{href}">"#);
            let title = extract_between(chunk, &open_tag, "</a>")
                .map(|value| html_decode(value.trim()))
                .unwrap_or_else(|| "Unknown".to_string());
            let platform = infer_platform_from_url(
                Url::parse(&format!("https://www.emu-land.net{href}")).ok().as_ref(),
                "download.rom",
            );
            let preview_image_url = extract_preview_image_near_link(chunk, href);
            let fields = extract_small_fields_near_link(chunk, href);
            let genres = fields
                .iter()
                .filter(|field| !field.starts_with("Players:"))
                .cloned()
                .collect::<Vec<_>>();
            let players = fields.iter().find_map(|field| {
                field
                    .strip_prefix("Players: ")
                    .and_then(|value| value.trim().parse::<u8>().ok())
            });
            results.push(EmuLandSearchResult {
                title,
                href: href.to_string(),
                platform,
                preview_image_url,
                genres,
                players,
            });
        }
    }
    results
}

fn parse_emu_land_top_results(html: &str) -> Vec<EmuLandBrowseItem> {
    let mut items = Vec::new();
    for chunk in html.split(r#"<div class="fcontainer""#).skip(1) {
        let Some(href) = extract_between(chunk, r#"<a href=""#, "\"") else {
            continue;
        };
        if !href.contains("/roms/") {
            continue;
        }
        let open = format!(r#"<a href="{href}">"#);
        let title = extract_between(chunk, &open, "</a>")
            .map(|value| html_decode(value.trim()))
            .unwrap_or_else(|| "Unknown".to_string());
        let preview_image_url = extract_attr(chunk, "img", "src").map(normalize_emu_land_url);
        let genres = extract_between(chunk, "&bull; Genre:", "</li>")
            .map(|value| {
                html_decode(&strip_tags_local(value))
                    .split(',')
                    .map(|part| part.trim().to_string())
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let players = extract_between(chunk, "&bull; Players:", "</li>").and_then(|value| {
            strip_tags_local(value)
                .trim()
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
                .parse::<u8>()
                .ok()
        });
        let developer = extract_between(chunk, "&bull; Developer:", "</li>")
            .map(|value| html_decode(&strip_tags_local(value)).trim().to_string())
            .filter(|value| !value.is_empty());
        let year = extract_between(chunk, "&bull; Year of release:", "</li>")
            .map(|value| {
                strip_tags_local(value)
                    .split_whitespace()
                    .find(|part| part.chars().all(|ch| ch.is_ascii_digit()) && part.len() == 4)
                    .unwrap_or_default()
                    .to_string()
            })
            .filter(|value| !value.is_empty());
        let downloads = extract_between(chunk, "Downloads:", "</li>")
            .map(|value| strip_tags_local(value).trim().to_string())
            .filter(|value| !value.is_empty());
        let description = extract_between(chunk, r#"<div class="ftext""#, "</div>")
            .and_then(|value| value.find('>').map(|idx| &value[idx + 1..]))
            .map(|value| html_decode(&strip_tags_local(value)).trim().to_string())
            .filter(|value| !value.is_empty());
        items.push(EmuLandBrowseItem {
            title,
            href: href.to_string(),
            platform: Platform::Nes,
            preview_image_url,
            genres,
            players,
            developer,
            year,
            downloads,
            description,
        });
    }
    items
}

fn strip_tags_local(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn extract_small_fields(chunk: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut rest = chunk;
    let marker = r#"<small class="muted-text">| "#;
    while let Some(idx) = rest.find(marker) {
        let after = &rest[idx + marker.len()..];
        if let Some(end) = after.find("</small>") {
            fields.push(html_decode(after[..end].trim()));
            rest = &after[end + "</small>".len()..];
        } else {
            break;
        }
    }
    fields
}

fn extract_preview_image_near_link(chunk: &str, href: &str) -> Option<String> {
    let marker = format!(r#"<a href="{href}">"#);
    let idx = chunk.find(&marker)?;
    let after = &chunk[idx..];
    let scope_end = after.find("</p>").unwrap_or(after.len());
    let scope = &after[..scope_end];
    extract_attr(scope, "img", "src").map(normalize_emu_land_url)
}

fn extract_small_fields_near_link(chunk: &str, href: &str) -> Vec<String> {
    let marker = format!(r#"<a href="{href}">"#);
    let Some(idx) = chunk.find(&marker) else {
        return Vec::new();
    };
    let after = &chunk[idx..];
    let scope_end = after.find("</p>").unwrap_or(after.len());
    extract_small_fields(&after[..scope_end])
}

fn fetch_emu_land_detail_html(href: &str) -> Result<String> {
    let url = if href.starts_with("http") {
        href.to_string()
    } else {
        format!("https://www.emu-land.net{href}")
    };
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36")
        .build()?;
    let html = client
        .get(url)
        .header("Referer", "https://www.emu-land.net/en/search_games")
        .header("Cookie", "lang=en; acceptCookie=true")
        .send()
        .context("emu-land detail fetch failed")?
        .text()
        .context("emu-land detail decode failed")?;
    Ok(html)
}

fn extract_emu_land_base_download_url(html: &str) -> Option<String> {
    let marker = "onclick=\"mgame('";
    let idx = html.find(marker)?;
    let rest = &html[idx + marker.len()..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

#[derive(Debug, Clone)]
struct ResolvedPreviewTarget {
    original_url: String,
    final_url: String,
    filename: String,
}

#[derive(Debug, Clone)]
struct EmuLandEntityHint {
    entity: EmuLandEntity,
    selected_variant: EmuLandVariant,
}

fn resolve_preview_target(url: &str) -> Result<ResolvedPreviewTarget> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36")
        .build()?;
    let response = client
        .get(url)
        .header("Referer", "https://www.emu-land.net/en/search_games")
        .header("Cookie", "lang=en; acceptCookie=true")
        .send()
        .with_context(|| format!("failed to preflight {url}"))?;
    let final_url = response.url().to_string();
    let parsed = Url::parse(&final_url).ok();
    let filename = parsed
        .as_ref()
        .map(infer_filename_from_url)
        .unwrap_or_else(|| "download.rom".to_string());
    Ok(ResolvedPreviewTarget {
        original_url: url.to_string(),
        final_url,
        filename,
    })
}

fn infer_title_from_download_target(final_url: &str, fallback: &str) -> Option<String> {
    let parsed = Url::parse(final_url).ok()?;
    let leaf = parsed.path_segments()?.next_back()?.trim();
    if leaf.is_empty() {
        return Some(fallback.to_string());
    }
    let decoded = percent_decode(leaf);
    let stem = decoded.rsplit_once('.').map(|(head, _)| head).unwrap_or(&decoded);
    Some(stem.replace('_', " ").split_whitespace().collect::<Vec<_>>().join(" "))
}

fn percent_decode(input: &str) -> String {
    match urlencoding::decode(input) {
        Ok(value) => value.into_owned(),
        Err(_) => input.to_string(),
    }
}

fn resolve_emu_land_entity_hint(url: &str) -> Result<Option<EmuLandEntityHint>> {
    let parsed = match Url::parse(url) {
        Ok(parsed) => parsed,
        Err(_) => return Ok(None),
    };
    if !is_emu_land_download(&parsed) {
        return Ok(None);
    }
    let entity_id = parsed
        .query_pairs()
        .find(|(key, _)| key == "id")
        .map(|(_, value)| value.into_owned())
        .context("missing emu-land id")?;
    let selected_fid = parsed
        .query_pairs()
        .find(|(key, _)| key == "fid")
        .map(|(_, value)| value.into_owned());
    let base_url = {
        let mut listing = parsed.clone();
        {
            let mut query = listing.query_pairs_mut();
            query.clear();
            query.append_pair("act", "getmfl");
            query.append_pair("id", &entity_id);
        }
        listing.to_string()
    };
    let platform = infer_platform_from_url(Some(&parsed), "download.zip");
    let listing_html = fetch_emu_land_file_listing(&parsed, &entity_id)?;
    let files = parse_emu_land_file_listing(&listing_html);
    Ok(build_emu_land_entity_hint(
        entity_id,
        platform,
        base_url,
        files,
        selected_fid,
    ))
}

fn build_emu_land_entity_hint(
    entity_id: String,
    platform: Platform,
    base_url: String,
    files: Vec<EmuLandFileEntry>,
    selected_fid: Option<String>,
) -> Option<EmuLandEntityHint> {
    if files.is_empty() {
        return None;
    }
    let selected = selected_fid
        .as_ref()
        .and_then(|fid| files.iter().find(|file| file.fid == *fid))
        .cloned()
        .or_else(|| {
            files.iter()
                .find(|file| file.variant.eq_ignore_ascii_case("Main"))
                .cloned()
        })
        .unwrap_or_else(|| files[0].clone());
    let entity_title = files
        .iter()
        .find(|file| file.variant.eq_ignore_ascii_case("Main"))
        .map(|file| canonicalize_archive_title(&file.filename))
        .or_else(|| files.first().map(|file| canonicalize_archive_title(&file.filename)))
        .unwrap_or_else(|| "Downloaded Game".to_string());
    let variants = files
        .into_iter()
        .map(|file| EmuLandVariant {
            exact_url: format!("{base_url}&fid={}", file.fid),
            fid: file.fid,
            label: file.filename,
            variant: file.variant,
        })
        .collect::<Vec<_>>();
    let selected_variant = variants
        .iter()
        .find(|variant| variant.fid == selected.fid)
        .cloned()
        .unwrap_or_else(|| variants[0].clone());
    Some(EmuLandEntityHint {
        entity: EmuLandEntity {
            id: entity_id,
            title: entity_title,
            platform,
            variants,
        },
        selected_variant,
    })
}

fn fetch_emu_land_file_listing(parsed: &Url, entity_id: &str) -> Result<String> {
    let mut listing = parsed.clone();
    {
        let mut query = listing.query_pairs_mut();
        query.clear();
        query.append_pair("act", "getmfl");
        query.append_pair("id", entity_id);
    }
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36")
        .build()?;
    let html = client
        .get(listing.as_str())
        .header("Referer", "https://www.emu-land.net/en/search_games")
        .header("Cookie", "lang=en; acceptCookie=true")
        .send()
        .with_context(|| format!("failed to resolve emu-land listing {}", listing.as_str()))?
        .text()
        .context("failed to decode emu-land file listing")?;
    Ok(html)
}

#[derive(Debug, Clone)]
struct EmuLandFileEntry {
    fid: String,
    filename: String,
    variant: String,
}

fn parse_emu_land_file_listing(html: &str) -> Vec<EmuLandFileEntry> {
    let mut entries = Vec::new();
    let mut current_variant = "Main".to_string();
    for line in html.lines() {
        if let Some(title) = extract_between(line, "<span>", "</span>") {
            let title = title.trim();
            if !title.is_empty() {
                current_variant = title.to_string();
            }
        }
        if let Some(fid_idx) = line.find("fid=") {
            let fid_raw = &line[fid_idx + 4..];
            let fid = fid_raw
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            if fid.is_empty() {
                continue;
            }
            if let Some(filename) = extract_between(line, "rel=\"nofollow\">", "</a>") {
                entries.push(EmuLandFileEntry {
                    fid,
                    filename: html_decode(filename.trim()),
                    variant: current_variant.clone(),
                });
            }
        }
    }
    entries
}

fn canonicalize_archive_title(filename: &str) -> String {
    let stem = filename.rsplit_once('.').map(|(head, _)| head).unwrap_or(filename);
    let mut out = String::new();
    let mut depth = 0usize;
    for ch in stem.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_string()
}

fn normalize_preview_title(input: &str) -> String {
    let base = input.rsplit_once('.').map(|(head, _)| head).unwrap_or(input);
    canonicalize_archive_title(base).to_ascii_lowercase()
}

fn cache_preview_artwork(paths: &AppPaths, remote_url: &str, title: &str) -> Result<PathBuf> {
    let ext = remote_url
        .rsplit('.')
        .next()
        .filter(|value| value.len() <= 4)
        .unwrap_or("png");
    let path = paths
        .data_dir
        .join("artwork")
        .join(format!("preview_{}.{}", sanitize_preview_stem(title), ext));
    if path.exists() {
        return Ok(path);
    }
    fs::create_dir_all(paths.data_dir.join("artwork"))?;
    let bytes = reqwest::blocking::get(remote_url)
        .with_context(|| format!("failed to fetch preview artwork {remote_url}"))?
        .bytes()?;
    fs::write(&path, bytes)?;
    Ok(path)
}

fn sanitize_preview_stem(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn extract_between<'a>(haystack: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = haystack.find(start)? + start.len();
    let rest = &haystack[from..];
    let to = rest.find(end)?;
    Some(&rest[..to])
}

fn extract_attr<'a>(html: &'a str, tag: &str, attr: &str) -> Option<&'a str> {
    let tag_start = html.find(&format!("<{tag}"))?;
    let fragment = &html[tag_start..];
    let attr_start = fragment.find(&format!(r#"{attr}=""#))? + attr.len() + 2;
    let rest = &fragment[attr_start..];
    let attr_end = rest.find('"')?;
    Some(&rest[..attr_end])
}

fn html_decode(input: &str) -> String {
    input
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&bull;", "•")
}

fn normalize_emu_land_url(input: &str) -> String {
    if input.starts_with("//") {
        format!("https:{input}")
    } else if input.starts_with('/') {
        format!("https://www.emu-land.net{input}")
    } else {
        input.to_string()
    }
}

pub fn normalize_download_url(input: &str) -> String {
    let trimmed = input.trim();
    let Ok(url) = Url::parse(trimmed) else {
        return trimmed.to_string();
    };
    if url.scheme() != "https" {
        return trimmed.to_string();
    }
    if url.host_str() != Some("github.com") {
        return trimmed.to_string();
    }
    let segments = match url.path_segments() {
        Some(segments) => segments.collect::<Vec<_>>(),
        None => return trimmed.to_string(),
    };
    if segments.len() >= 5 && segments[2] == "blob" {
        let owner = segments[0];
        let repo = segments[1];
        let branch = segments[3];
        let path = segments[4..].join("/");
        return format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}");
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_user_url_line() {
        let parsed = parse_user_url("https://example.com/poke.gba|Pokemon|gba").unwrap();
        assert_eq!(parsed.platform, Platform::GameBoyAdvance);
        assert_eq!(parsed.source_kind, SourceKind::UserUrl);
    }

    #[test]
    fn infers_emu_land_download_as_nes_zip() {
        let parsed = parse_user_url(
            "https://www.emu-land.net/en/consoles/dendy/roms?act=getmfl&id=25065&fid=10033",
        )
        .unwrap();
        assert_eq!(parsed.platform, Platform::Nes);
        assert_eq!(parsed.filename, "download.zip");
        assert_eq!(parsed.title, "Downloaded Game");
    }

    #[test]
    fn infers_title_from_redirect_target() {
        let title = infer_title_from_download_target(
            "https://dl.emu-land.net/roms/nes/no-intro/Lion%20King%2C%20The%20%281995%29%20%28Asia%29%20%28En%29%20%28Pirate%29.zip",
            "Downloaded Game",
        )
        .unwrap();
        assert_eq!(title, "Lion King, The (1995) (Asia) (En) (Pirate)");
    }

    #[test]
    fn normalizes_github_blob_urls() {
        let url = normalize_download_url(
            "https://github.com/retrobrews/gba-games/blob/master/3weeksinparadise.gba",
        );
        assert_eq!(
            url,
            "https://raw.githubusercontent.com/retrobrews/gba-games/master/3weeksinparadise.gba"
        );
    }

    #[test]
    fn parses_emu_land_file_listing_and_canonical_title() {
        let html = r#"
        <div class="collaps-item open">
          <div class="title"><span>Main</span></div>
          <div class="body">
            <div class="item">
              <div class="file"><a href="/en/consoles/dendy/roms?act=getmfl&amp;id=25065&amp;fid=10031" rel="nofollow">Jungle Book, The (USA).zip</a></div>
            </div>
          </div>
        </div>
        <div class="collaps-item">
          <div class="title"><span>Pirate</span></div>
          <div class="body">
            <div class="item">
              <div class="file"><a href="/en/consoles/dendy/roms?act=getmfl&amp;id=25065&amp;fid=10033" rel="nofollow">Lion King, The (1995) (Asia) (En) (Pirate).zip</a></div>
            </div>
          </div>
        </div>
        "#;
        let files = parse_emu_land_file_listing(html);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].variant, "Main");
        assert_eq!(files[0].fid, "10031");
        assert_eq!(canonicalize_archive_title(&files[0].filename), "Jungle Book, The");
        assert_eq!(files[1].variant, "Pirate");
        assert_eq!(files[1].fid, "10033");
    }

    #[test]
    fn selects_main_variant_exact_url_for_entity_preview() {
        let files = vec![
            EmuLandFileEntry {
                fid: "10031".to_string(),
                filename: "Jungle Book, The (USA).zip".to_string(),
                variant: "Main".to_string(),
            },
            EmuLandFileEntry {
                fid: "10033".to_string(),
                filename: "Lion King, The (1995) (Asia) (En) (Pirate).zip".to_string(),
                variant: "Pirate".to_string(),
            },
        ];
        let hint = build_emu_land_entity_hint(
            "25065".to_string(),
            Platform::Nes,
            "https://www.emu-land.net/en/consoles/dendy/roms?act=getmfl&id=25065".to_string(),
            files,
            None,
        )
        .unwrap();
        assert_eq!(hint.entity.title, "Jungle Book, The");
        assert_eq!(hint.selected_variant.fid, "10031");
        assert_eq!(
            hint.selected_variant.exact_url,
            "https://www.emu-land.net/en/consoles/dendy/roms?act=getmfl&id=25065&fid=10031"
        );
        assert_eq!(hint.entity.variants.len(), 2);
    }

    #[test]
    fn parses_search_results_when_breadcrumb_links_come_first() {
        let html = r#"
        <div class="fcontainer">
            <div class="fheader"><a href="/en/consoles">Consoles</a> &gt; <a href="/en/consoles/dendy">NES / Famicom</a> &gt; <a href="/en/consoles/dendy/roms">ROMs / Games</a></div>
            <div class="fllinks">
                <p>
                    <a href="/en/consoles/dendy/roms/contra">Contra</a>
                    <img class="preview pixelated" src="//ss.emu-land.net/nes_pict/Contra-07.png" alt="Contra" loading="lazy">
                    <small class="muted-text">| platform, shooter, run'n'gun </small>
                    <small class="muted-text">| Players: 2 </small>
                </p>
                <p>
                    <a href="/en/consoles/dendy/roms/super-c">Super Contra</a>
                    <img class="preview pixelated" src="//ss.emu-land.net/nes_pict/SuperContra.png" alt="Super Contra" loading="lazy">
                    <small class="muted-text">| platform, shooter, run'n'gun </small>
                    <small class="muted-text">| Players: 2 </small>
                </p>
            </div>
        </div>
        "#;
        let results = parse_emu_land_search_results(html);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Contra");
        assert_eq!(results[0].href, "/en/consoles/dendy/roms/contra");
        assert_eq!(results[0].platform, Platform::Nes);
        assert_eq!(results[0].players, Some(2));
        assert_eq!(
            results[0].preview_image_url.as_deref(),
            Some("https://ss.emu-land.net/nes_pict/Contra-07.png")
        );
    }
}
