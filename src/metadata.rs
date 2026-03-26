use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::config::AppPaths;
use crate::db::Database;
use crate::models::{ArtworkRecord, GameEntry, MetadataMatchState, Platform, ResolvedMetadata};

const STARTER_METADATA: &str = include_str!("../support/starter_metadata.json");

#[derive(Debug, Clone)]
pub struct MetadataQuery {
    pub game_id: String,
    pub raw_title: String,
    pub normalized_title: String,
    pub platform: Platform,
    pub hash: Option<String>,
    pub origin_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MetadataMatch {
    pub canonical_title: String,
    pub normalized_title: String,
    pub provider_ids: Vec<String>,
    pub match_state: MetadataMatchState,
    pub match_confidence: f32,
    pub tags: Vec<String>,
    pub genres: Vec<String>,
    pub artwork_url: Option<String>,
    pub unmatched_reason: Option<String>,
}

pub type ProviderResult = Option<MetadataMatch>;

pub trait MetadataProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn identify(&self, query: &MetadataQuery) -> ProviderResult;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StarterMetadataEntry {
    canonical_title: String,
    platform: Platform,
    aliases: Vec<String>,
    tags: Vec<String>,
    genres: Vec<String>,
    artwork_url: Option<String>,
}

pub struct StarterPackProvider {
    entries: Vec<StarterMetadataEntry>,
}

impl StarterPackProvider {
    pub fn new() -> Result<Self> {
        Ok(Self {
            entries: serde_json::from_str(STARTER_METADATA)?,
        })
    }
}

impl MetadataProvider for StarterPackProvider {
    fn id(&self) -> &'static str {
        "starter-pack"
    }

    fn identify(&self, query: &MetadataQuery) -> ProviderResult {
        let mut best: Option<MetadataMatch> = None;
        for entry in &self.entries {
            if entry.platform != query.platform && entry.platform != Platform::Unknown {
                continue;
            }
            let mut confidence = 0.0f32;
            for alias in &entry.aliases {
                let alias = normalize_title(alias);
                if query.normalized_title == alias {
                    confidence = confidence.max(0.98);
                } else if query.normalized_title.contains(&alias)
                    || alias.contains(&query.normalized_title)
                {
                    confidence = confidence.max(0.84);
                } else if loose_token_match(&query.normalized_title, &alias) {
                    confidence = confidence.max(0.76);
                }
            }
            if confidence
                > best
                    .as_ref()
                    .map(|item| item.match_confidence)
                    .unwrap_or(0.0)
            {
                best = Some(MetadataMatch {
                    canonical_title: entry.canonical_title.clone(),
                    normalized_title: normalize_title(&entry.canonical_title),
                    provider_ids: vec![self.id().to_string()],
                    match_state: MetadataMatchState::Resolved,
                    match_confidence: confidence,
                    tags: entry.tags.clone(),
                    genres: entry.genres.clone(),
                    artwork_url: entry.artwork_url.clone(),
                    unmatched_reason: None,
                });
            }
        }
        best
    }
}

pub struct FilenameHeuristicProvider;

impl MetadataProvider for FilenameHeuristicProvider {
    fn id(&self) -> &'static str {
        "filename-heuristic"
    }

    fn identify(&self, query: &MetadataQuery) -> ProviderResult {
        if query.normalized_title.is_empty() {
            return None;
        }
        let title = query
            .raw_title
            .split_whitespace()
            .map(capitalize_word)
            .collect::<Vec<_>>()
            .join(" ");
        Some(MetadataMatch {
            canonical_title: title,
            normalized_title: query.normalized_title.clone(),
            provider_ids: vec![self.id().to_string()],
            match_state: MetadataMatchState::Unmatched,
            match_confidence: 0.25,
            tags: Vec::new(),
            genres: Vec::new(),
            artwork_url: None,
            unmatched_reason: Some(
                "No strong metadata match; using normalized filename.".to_string(),
            ),
        })
    }
}

pub struct CatalogTitleProvider;

impl MetadataProvider for CatalogTitleProvider {
    fn id(&self) -> &'static str {
        "catalog-title"
    }

    fn identify(&self, query: &MetadataQuery) -> ProviderResult {
        query.origin_url.as_ref()?;
        Some(MetadataMatch {
            canonical_title: query.raw_title.clone(),
            normalized_title: query.normalized_title.clone(),
            provider_ids: vec![self.id().to_string()],
            match_state: MetadataMatchState::Imported,
            match_confidence: 0.4,
            tags: Vec::new(),
            genres: Vec::new(),
            artwork_url: None,
            unmatched_reason: None,
        })
    }
}

pub struct EmuLandProvider {
    client: Client,
}

impl EmuLandProvider {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36")
            .build()?;
        Ok(Self { client })
    }

    fn fetch_search(&self, query: &MetadataQuery) -> Result<Option<String>> {
        let search_title = if query.normalized_title.is_empty() {
            query.raw_title.clone()
        } else {
            query.normalized_title
                .split_whitespace()
                .map(capitalize_word)
                .collect::<Vec<_>>()
                .join(" ")
        };
        let title = search_title.replace(' ', "+");
        let url = format!(
            "https://www.emu-land.net/en/search_games?id=all&genre=--&players=--&q={title}"
        );
        let html = self
            .client
            .get(url)
            .header("Referer", "https://www.emu-land.net/en/search_games")
            .header("Cookie", "lang=en; acceptCookie=true")
            .send()
            .context("emu-land search request failed")?
            .text()
            .context("emu-land search response decode failed")?;
        Ok(parse_emu_land_search_href(&html, query.platform))
    }

    fn fetch_detail(&self, href: &str) -> Result<Option<MetadataMatch>> {
        let url = format!("https://www.emu-land.net{href}");
        let html = self
            .client
            .get(&url)
            .header("Referer", "https://www.emu-land.net/en/search_games")
            .header("Cookie", "lang=en; acceptCookie=true")
            .send()
            .with_context(|| format!("emu-land detail request failed for {url}"))?
            .text()
            .with_context(|| format!("emu-land detail response decode failed for {url}"))?;
        Ok(parse_emu_land_detail(&html))
    }
}

impl MetadataProvider for EmuLandProvider {
    fn id(&self) -> &'static str {
        "emu-land"
    }

    fn identify(&self, query: &MetadataQuery) -> ProviderResult {
        let href = self.fetch_search(query).ok().flatten()?;
        let mut matched = self.fetch_detail(&href).ok().flatten()?;
        matched.provider_ids.push(self.id().to_string());
        matched.match_confidence = matched.match_confidence.max(0.9);
        Some(matched)
    }
}

pub struct MetadataService {
    db: Database,
    paths: AppPaths,
    providers: Vec<Box<dyn MetadataProvider>>,
}

pub fn preview_metadata_match(
    raw_title: &str,
    platform: Platform,
    origin_url: Option<&str>,
) -> Result<Option<MetadataMatch>> {
    preview_metadata_match_inner(raw_title, platform, origin_url, false)
}

pub fn preview_metadata_match_local(
    raw_title: &str,
    platform: Platform,
    origin_url: Option<&str>,
) -> Result<Option<MetadataMatch>> {
    preview_metadata_match_inner(raw_title, platform, origin_url, true)
}

fn preview_metadata_match_inner(
    raw_title: &str,
    platform: Platform,
    origin_url: Option<&str>,
    skip_emu_land: bool,
) -> Result<Option<MetadataMatch>> {
    let query = MetadataQuery {
        game_id: "preview".to_string(),
        raw_title: raw_title.to_string(),
        normalized_title: normalize_title(raw_title),
        platform,
        hash: None,
        origin_url: origin_url.map(ToString::to_string),
    };
    let mut providers: Vec<Box<dyn MetadataProvider>> = vec![
        Box::new(StarterPackProvider::new()?),
    ];
    if !skip_emu_land {
        providers.push(Box::new(EmuLandProvider::new()?));
    }
    providers.push(Box::new(CatalogTitleProvider));
    providers.push(Box::new(FilenameHeuristicProvider));
    Ok(resolve_best_match_from_providers(&providers, &query))
}

impl MetadataService {
    pub fn new(db: Database, paths: AppPaths) -> Result<Self> {
        Ok(Self {
            db,
            paths,
            providers: vec![
                Box::new(StarterPackProvider::new()?),
                Box::new(EmuLandProvider::new()?),
                Box::new(CatalogTitleProvider),
                Box::new(FilenameHeuristicProvider),
            ],
        })
    }

    pub fn enrich_game(&self, game: &GameEntry) -> Result<ResolvedMetadata> {
        let query = MetadataQuery {
            game_id: game.id.clone(),
            raw_title: game.title.clone(),
            normalized_title: normalize_title(&game.title),
            platform: game.platform,
            hash: game.hash.clone(),
            origin_url: game.origin_url.clone(),
        };

        // Check if game already has user-confirmed metadata - never overwrite it
        if let Some(existing) = self.db.find_resolved_metadata(&game.id)? {
            if existing.artwork.source.as_deref() == Some("preview-confirmed") {
                return Ok(existing);
            }
        }

        if let Some(cached) = self.db.find_cached_metadata(&query)? {
            let mut resolved = cached;
            resolved.game_id = game.id.clone();
            self.db.upsert_resolved_metadata(&resolved)?;
            return Ok(resolved);
        }

        let best = resolve_best_match_from_providers(&self.providers, &query);

        let matched = best.unwrap_or(MetadataMatch {
            canonical_title: game.title.clone(),
            normalized_title: query.normalized_title.clone(),
            provider_ids: vec!["fallback".to_string()],
            match_state: MetadataMatchState::Unmatched,
            match_confidence: 0.0,
            tags: Vec::new(),
            genres: Vec::new(),
            artwork_url: None,
            unmatched_reason: Some("No metadata providers matched this title.".to_string()),
        });

        let resolved = self.materialize(game.id.clone(), matched)?;
        self.db.upsert_resolved_metadata(&resolved)?;
        self.db.upsert_metadata_cache(&query, &resolved)?;
        Ok(resolved)
    }

    fn materialize(&self, game_id: String, matched: MetadataMatch) -> Result<ResolvedMetadata> {
        let cached_path = if let Some(remote_url) = &matched.artwork_url {
            Some(self.cache_artwork(remote_url, &matched.normalized_title)?)
        } else {
            None
        };

        Ok(ResolvedMetadata {
            game_id,
            canonical_title: matched.canonical_title,
            normalized_title: matched.normalized_title,
            match_state: matched.match_state,
            match_confidence: matched.match_confidence,
            provider_ids: matched.provider_ids,
            artwork: ArtworkRecord {
                cached_path,
                remote_url: matched.artwork_url,
                source: Some("metadata-service".to_string()),
            },
            tags: matched.tags,
            genres: matched.genres,
            unmatched_reason: matched.unmatched_reason,
            updated_at: Utc::now(),
        })
    }

    fn cache_artwork(&self, remote_url: &str, stem: &str) -> Result<PathBuf> {
        let ext = remote_url
            .rsplit('.')
            .next()
            .filter(|value| value.len() <= 4)
            .unwrap_or("png");
        let path =
            self.paths
                .data_dir
                .join("artwork")
                .join(format!("{}.{}", sanitize_stem(stem), ext));
        if path.exists() {
            return Ok(path);
        }
        let bytes = reqwest::blocking::get(remote_url)
            .with_context(|| format!("failed to fetch artwork {remote_url}"))?
            .bytes()?;
        fs::write(&path, bytes)?;
        Ok(path)
    }
}

fn resolve_best_match_from_providers(
    providers: &[Box<dyn MetadataProvider>],
    query: &MetadataQuery,
) -> Option<MetadataMatch> {
    let mut candidates = Vec::new();
    for provider in providers {
        if let Some(candidate) = provider.identify(query) {
            candidates.push(candidate);
        }
    }
    merge_best_match(candidates)
}

fn merge_best_match(candidates: Vec<MetadataMatch>) -> Option<MetadataMatch> {
    let mut best = candidates
        .iter()
        .max_by(|left, right| left.match_confidence.total_cmp(&right.match_confidence))
        .cloned()?;
    if best.artwork_url.is_none() {
        if let Some(art_candidate) = candidates
            .iter()
            .filter(|candidate| candidate.artwork_url.is_some())
            .filter(|candidate| titles_compatible(&best, candidate))
            .max_by(|left, right| left.match_confidence.total_cmp(&right.match_confidence))
        {
            best.artwork_url = art_candidate.artwork_url.clone();
            for provider_id in &art_candidate.provider_ids {
                if !best.provider_ids.contains(provider_id) {
                    best.provider_ids.push(provider_id.clone());
                }
            }
        }
    }
    best.tags = merge_text_lists(best.tags, candidates.iter().flat_map(|item| item.tags.clone()));
    best.genres =
        merge_text_lists(best.genres, candidates.iter().flat_map(|item| item.genres.clone()));
    Some(best)
}

fn titles_compatible(primary: &MetadataMatch, candidate: &MetadataMatch) -> bool {
    primary.normalized_title == candidate.normalized_title
        || loose_token_match(&primary.normalized_title, &candidate.normalized_title)
}

fn merge_text_lists(
    seed: Vec<String>,
    extras: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let mut merged = seed;
    for item in extras {
        if !merged.iter().any(|existing| existing.eq_ignore_ascii_case(&item)) {
            merged.push(item);
        }
    }
    merged
}

pub fn normalize_title(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut depth_paren = 0usize;
    let mut depth_bracket = 0usize;
    let mut no_ext = input.to_string();
    if let Some((head, _tail)) = no_ext.rsplit_once('.') {
        if _tail.len() <= 5 {
            no_ext = head.to_string();
        }
    }
    for ch in no_ext.chars() {
        match ch {
            '(' => depth_paren += 1,
            ')' => depth_paren = depth_paren.saturating_sub(1),
            '[' => depth_bracket += 1,
            ']' => depth_bracket = depth_bracket.saturating_sub(1),
            _ if depth_paren > 0 || depth_bracket > 0 => {}
            _ if ch.is_ascii_alphanumeric() => output.push(ch.to_ascii_lowercase()),
            _ => output.push(' '),
        }
    }
    output
        .split_whitespace()
        .filter(|part| {
            !matches!(
                *part,
                "usa" | "world" | "europe" | "rev" | "beta" | "proto" | "v1" | "v2" | "the"
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn loose_token_match(left: &str, right: &str) -> bool {
    let left_tokens = left.split_whitespace().collect::<Vec<_>>();
    let right_tokens = right.split_whitespace().collect::<Vec<_>>();
    left_tokens.iter().all(|token| right_tokens.contains(token))
        || right_tokens.iter().all(|token| left_tokens.contains(token))
}

fn sanitize_stem(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn capitalize_word(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => {
            let mut result = String::new();
            result.push(first.to_ascii_uppercase());
            result.push_str(chars.as_str());
            result
        }
        None => String::new(),
    }
}

fn parse_emu_land_search_href(html: &str, platform: Platform) -> Option<String> {
    let platform_hint = emu_land_platform_hint(platform);
    let mut cursor = 0usize;
    while let Some(anchor_start) = html[cursor..].find("<a href=\"/en/") {
        let start = cursor + anchor_start + "<a href=\"".len();
        let rest = &html[start..];
        let end = rest.find('"')?;
        let href = &rest[..end];
        if href.contains("/roms/") && platform_hint.is_none_or(|hint| href.contains(hint)) {
            return Some(href.to_string());
        }
        cursor = start + end;
    }
    None
}

fn parse_emu_land_detail(html: &str) -> Option<MetadataMatch> {
    let title = extract_between(html, "<h1>", "</h1>")
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let genre_line = extract_between(html, "Genre:", "</li>").unwrap_or_default();
    let players_line = extract_between(html, "Players:", "</li>").unwrap_or_default();
    let developer_line = extract_between(html, "Developer:", "</li>").unwrap_or_default();
    let publisher_line = extract_between(html, "Published:", "</li>").unwrap_or_default();
    let year_line = extract_between(html, "Year of release:", "</li>").unwrap_or_default();
    let artwork_url = extract_emu_land_artwork_url(html);

    let mut tags = Vec::new();
    if !players_line.is_empty() {
        tags.push(format!("Players: {}", clean_emu_land_field(&players_line)));
    }
    if !developer_line.is_empty() {
        tags.push(format!("Developer: {}", clean_emu_land_field(&developer_line)));
    }
    if !publisher_line.is_empty() {
        tags.push(format!("Publisher: {}", clean_emu_land_field(&publisher_line)));
    }
    if !year_line.is_empty() {
        tags.push(format!("Year: {}", clean_emu_land_field(&year_line)));
    }

    let genres = clean_emu_land_field(&genre_line)
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    Some(MetadataMatch {
        canonical_title: html_decode(title),
        normalized_title: normalize_title(title),
        provider_ids: Vec::new(),
        match_state: MetadataMatchState::Resolved,
        match_confidence: 0.86,
        tags,
        genres,
        artwork_url,
        unmatched_reason: None,
    })
}

fn extract_emu_land_artwork_url(html: &str) -> Option<String> {
    let gallery_start = html.find(r#"<div class="ss-area">"#).unwrap_or(0);
    let gallery = &html[gallery_start..];
    let src = extract_attr(gallery, "img", "src")?;
    Some(normalize_emu_land_asset_url(src))
}

fn extract_attr<'a>(html: &'a str, tag: &str, attr: &str) -> Option<&'a str> {
    let tag_start = html.find(&format!("<{tag}"))?;
    let fragment = &html[tag_start..];
    let attr_start = fragment.find(&format!(r#"{attr}=""#))? + attr.len() + 2;
    let rest = &fragment[attr_start..];
    let attr_end = rest.find('"')?;
    Some(&rest[..attr_end])
}

fn clean_emu_land_field(input: &str) -> String {
    let without_tags = strip_tags(input);
    html_decode(without_tags.trim()).replace('\n', " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_between<'a>(haystack: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = haystack.find(start)? + start.len();
    let rest = &haystack[from..];
    let to = rest.find(end)?;
    Some(&rest[..to])
}

fn strip_tags(input: &str) -> String {
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

fn html_decode(input: &str) -> String {
    input
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&bull;", "•")
        .replace("&nbsp;", " ")
}

fn normalize_emu_land_asset_url(input: &str) -> String {
    if input.starts_with("//") {
        format!("https:{input}")
    } else if input.starts_with('/') {
        format!("https://www.emu-land.net{input}")
    } else {
        input.to_string()
    }
}

fn emu_land_platform_hint(platform: Platform) -> Option<&'static str> {
    match platform {
        Platform::Nes => Some("/dendy/"),
        Platform::Snes => Some("/super_nintendo/"),
        Platform::GameBoy => Some("/gameboy/"),
        Platform::GameBoyColor => Some("/gbc/"),
        Platform::GameBoyAdvance => Some("/gba/"),
        Platform::Ps1 => Some("/psx/"),
        Platform::NintendoDs => Some("/nds/"),
        Platform::N64 => Some("/n64/"),
        Platform::SegaGenesis => Some("/genesis/"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_noisy_titles() {
        assert_eq!(
            normalize_title("Super Mario Bros. (USA) [!].nes"),
            "super mario bros"
        );
    }

    #[test]
    fn starter_pack_matches_common_classics() {
        let provider = StarterPackProvider::new().unwrap();
        let query = MetadataQuery {
            game_id: "1".into(),
            raw_title: "Airball (USA)".into(),
            normalized_title: normalize_title("Airball (USA)"),
            platform: Platform::GameBoyAdvance,
            hash: None,
            origin_url: None,
        };
        let matched = provider.identify(&query).unwrap();
        assert_eq!(matched.canonical_title, "Airball");
    }

    #[test]
    fn parses_emu_land_search_result_href() {
        let html = r#"
            <a href="/en/consoles/dendy/roms/the-jungle-book">Jungle Book, The</a>
            <a href="/en/consoles/gba/roms/airball">Airball</a>
        "#;
        let href = parse_emu_land_search_href(html, Platform::Nes).unwrap();
        assert_eq!(href, "/en/consoles/dendy/roms/the-jungle-book");
    }

    #[test]
    fn parses_emu_land_detail_fields() {
        let html = r#"
            <div class="fcontainer">
              <div class="rheader"><h1>Jungle Book, The </h1></div>
              <div class="ss-area">
                <div class="wrapper">
                  <div class="item">
                    <a class="ss" href="//ss.emu-land.net/nes_pict/Jungle Book, The (U)_title.png">
                      <img class="pixelated" src="//ss.emu-land.net/nes_pict/Jungle Book, The (U)_title.png" alt="Jungle Book, The: Title">
                    </a>
                  </div>
                </div>
              </div>
              <ul class="finfo">
                <li>&bull; Genre: platform, puzzle</li>
                <li>&bull; Players: 1 </li>
                <li>&bull; Developer: Eurocom Entertainment Software</li>
                <li>&bull; Year of release: <span>  1994</span> &bull; Released</li>
                <li>&bull; Published: <span>Virgin Interactive Entertainment</span></li>
              </ul>
            </div>
        "#;
        let matched = parse_emu_land_detail(html).unwrap();
        assert_eq!(matched.canonical_title, "Jungle Book, The");
        assert_eq!(matched.genres, vec!["platform", "puzzle"]);
        assert_eq!(
            matched.artwork_url.as_deref(),
            Some("https://ss.emu-land.net/nes_pict/Jungle Book, The (U)_title.png")
        );
        assert!(matched
            .tags
            .iter()
            .any(|tag| tag == "Developer: Eurocom Entertainment Software"));
        assert!(matched.tags.iter().any(|tag| tag == "Year: 1994 • Released"));
    }

    #[test]
    fn merges_artwork_from_secondary_provider_when_best_match_has_none() {
        let merged = merge_best_match(vec![
            MetadataMatch {
                canonical_title: "Super Mario Bros.".to_string(),
                normalized_title: normalize_title("Super Mario Bros."),
                provider_ids: vec!["starter-pack".to_string()],
                match_state: MetadataMatchState::Resolved,
                match_confidence: 0.98,
                tags: vec!["Classic".to_string()],
                genres: vec!["Platformer".to_string()],
                artwork_url: None,
                unmatched_reason: None,
            },
            MetadataMatch {
                canonical_title: "Super Mario Bros.".to_string(),
                normalized_title: normalize_title("Super Mario Bros."),
                provider_ids: vec!["emu-land".to_string()],
                match_state: MetadataMatchState::Resolved,
                match_confidence: 0.90,
                tags: vec!["Nintendo".to_string()],
                genres: vec!["Platformer".to_string()],
                artwork_url: Some(
                    "https://ss.emu-land.net/nes_pict/Super Mario Bros-01.png".to_string(),
                ),
                unmatched_reason: None,
            },
        ])
        .unwrap();
        assert_eq!(merged.canonical_title, "Super Mario Bros.");
        assert_eq!(
            merged.artwork_url.as_deref(),
            Some("https://ss.emu-land.net/nes_pict/Super Mario Bros-01.png")
        );
        assert!(merged.provider_ids.iter().any(|id| id == "starter-pack"));
        assert!(merged.provider_ids.iter().any(|id| id == "emu-land"));
        assert!(merged.tags.iter().any(|tag| tag == "Classic"));
        assert!(merged.tags.iter().any(|tag| tag == "Nintendo"));
    }

    #[test]
    fn emu_land_search_prefers_normalized_title_over_noisy_raw_title() {
        let query = MetadataQuery {
            game_id: "1".into(),
            raw_title: "Super Contra II (Asia) (En) (Pirate)".into(),
            normalized_title: normalize_title("Super Contra II (Asia) (En) (Pirate)"),
            platform: Platform::Nes,
            hash: None,
            origin_url: Some(
                "https://www.emu-land.net/en/consoles/dendy/roms?act=getmfl&id=25738&fid=10311"
                    .into(),
            ),
        };
        let search_title = if query.normalized_title.is_empty() {
            query.raw_title.clone()
        } else {
            query.normalized_title
                .split_whitespace()
                .map(capitalize_word)
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(search_title, "Super Contra Ii");
    }
}
