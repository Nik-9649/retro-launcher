use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use crate::catalog;
use crate::config::AppPaths;
use crate::emulator;
use crate::metadata::MetadataQuery;
use crate::models::{
    CatalogEntry, EmulatorKind, GameEntry, InstallState, MetadataMatchState, ResolvedMetadata,
    sort_games,
};

const CURRENT_SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone)]
pub struct Database {
    path: PathBuf,
}

#[derive(Debug, Default, Clone)]
pub struct RepairReport {
    pub removed_missing_payloads: usize,
    pub normalized_urls: usize,
    pub removed_legacy_demo_rows: usize,
    pub removed_bundled_catalog_rows: usize,
    pub reset_broken_downloads: usize,
    pub reset_emulator_assignments: usize,
}

impl Database {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let db = Self {
            path: path.as_ref().to_path_buf(),
        };
        db.init()?;
        Ok(db)
    }

    fn connect(&self) -> Result<Connection> {
        Ok(Connection::open(&self.path)?)
    }

    fn init(&self) -> Result<()> {
        let conn = self.connect()?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS games (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                filename TEXT,
                platform_json TEXT NOT NULL,
                generation_json TEXT NOT NULL,
                vibe_tags_json TEXT NOT NULL,
                source_kind_json TEXT NOT NULL,
                install_state_json TEXT NOT NULL,
                managed_path TEXT,
                origin_url TEXT,
                origin_label TEXT,
                rom_path TEXT,
                hash TEXT UNIQUE,
                emulator_kind_json TEXT,
                checksum TEXT,
                size_bytes INTEGER,
                play_count INTEGER NOT NULL DEFAULT 0,
                last_played_at TEXT,
                discovered_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                source_refs_json TEXT NOT NULL,
                error_message TEXT,
                progress INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_games_hash ON games(hash);
            CREATE INDEX IF NOT EXISTS idx_games_title ON games(title);
            CREATE TABLE IF NOT EXISTS schema_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS resolved_metadata (
                game_id TEXT PRIMARY KEY,
                canonical_title TEXT NOT NULL,
                normalized_title TEXT NOT NULL,
                match_state_json TEXT NOT NULL,
                match_confidence REAL NOT NULL,
                provider_ids_json TEXT NOT NULL,
                artwork_json TEXT NOT NULL,
                tags_json TEXT NOT NULL,
                genres_json TEXT NOT NULL,
                unmatched_reason TEXT,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS metadata_cache (
                cache_key TEXT PRIMARY KEY,
                hash TEXT,
                normalized_title TEXT NOT NULL,
                platform_json TEXT NOT NULL,
                canonical_title TEXT NOT NULL,
                match_state_json TEXT NOT NULL,
                match_confidence REAL NOT NULL,
                provider_ids_json TEXT NOT NULL,
                artwork_json TEXT NOT NULL,
                tags_json TEXT NOT NULL,
                genres_json TEXT NOT NULL,
                unmatched_reason TEXT,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_metadata_cache_hash ON metadata_cache(hash);
            CREATE INDEX IF NOT EXISTS idx_metadata_cache_title ON metadata_cache(normalized_title);
        "#,
        )?;
        self.set_schema_version(CURRENT_SCHEMA_VERSION)?;
        Ok(())
    }

    fn set_schema_version(&self, version: i64) -> Result<()> {
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO schema_meta(key, value) VALUES('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![version.to_string()],
        )?;
        Ok(())
    }

    pub fn repair_and_migrate_state(&self, paths: &AppPaths) -> Result<RepairReport> {
        let mut report = RepairReport::default();
        let conn = self.connect()?;

        report.removed_legacy_demo_rows = conn.execute(
            "DELETE FROM games WHERE origin_label = ?1",
            params!["Bundled demo catalog"],
        )?;
        report.removed_bundled_catalog_rows = conn.execute(
            "DELETE FROM games
             WHERE source_kind_json = ?1
               AND origin_label = ?2
               AND managed_path IS NULL
               AND rom_path IS NULL",
            params![
                serde_json::to_string(&crate::models::SourceKind::Catalog)?,
                "retrobrews/gba-games"
            ],
        )?;

        let rows: Vec<(
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            String,
            Option<String>,
        )> = {
            let mut stmt = conn.prepare(
                "SELECT id, origin_url, managed_path, rom_path, source_kind_json, platform_json, emulator_kind_json FROM games",
            )?;
            let iter = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })?;
            iter.collect::<std::result::Result<Vec<_>, _>>()?
        };

        for (
            id,
            origin_url,
            managed_path,
            rom_path,
            source_kind_json,
            platform_json,
            emulator_kind_json,
        ) in rows
        {
            if let Some(url) = origin_url {
                let normalized = catalog::normalize_download_url(&url);
                if normalized != url {
                    conn.execute(
                        "UPDATE games SET origin_url = ?2 WHERE id = ?1",
                        params![id, normalized],
                    )?;
                    report.normalized_urls += 1;
                }
            }

            let payload_path = managed_path.clone().or(rom_path.clone()).map(PathBuf::from);
            if let Some(path) = payload_path {
                if !path.exists() {
                    let source_kind =
                        serde_json::from_str::<crate::models::SourceKind>(&source_kind_json)?;
                    if matches!(source_kind, crate::models::SourceKind::LocalScan) {
                        conn.execute("DELETE FROM games WHERE id = ?1", params![id])?;
                        conn.execute(
                            "DELETE FROM resolved_metadata WHERE game_id = ?1",
                            params![id],
                        )?;
                        report.removed_missing_payloads += 1;
                    } else {
                        conn.execute(
                            "UPDATE games
                             SET managed_path = NULL,
                                 rom_path = NULL,
                                 hash = NULL,
                                 progress = NULL,
                                 install_state_json = ?2,
                                 error_message = ?3
                             WHERE id = ?1",
                            params![
                                id,
                                serde_json::to_string(&InstallState::DownloadAvailable)?,
                                "Launcher-managed payload missing; download again."
                            ],
                        )?;
                        conn.execute(
                            "INSERT INTO resolved_metadata(game_id, canonical_title, normalized_title, match_state_json, match_confidence, provider_ids_json, artwork_json, tags_json, genres_json, unmatched_reason, updated_at)
                             SELECT id, title, '', ?2, 0.0, '[]', '{\"cached_path\":null,\"remote_url\":null,\"source\":null}', '[]', '[]', 'Legacy row needs re-identification', ?3
                             FROM games WHERE id = ?1
                             ON CONFLICT(game_id) DO UPDATE SET
                               match_state_json = excluded.match_state_json,
                               unmatched_reason = excluded.unmatched_reason,
                               updated_at = excluded.updated_at",
                            params![
                                id,
                                serde_json::to_string(&MetadataMatchState::RepairNeeded)?,
                                Utc::now().to_rfc3339()
                            ],
                        )?;
                    report.reset_broken_downloads += 1;
                }
            }

            let platform = serde_json::from_str(&platform_json)?;
            let preferred = crate::models::default_emulator_for(platform);
            if let Some(raw) = emulator_kind_json {
                let emulator_kind: EmulatorKind = serde_json::from_str(&raw)?;
                let supported = emulator::emulators_for_platform(platform);
                let should_reset = !supported.contains(&emulator_kind)
                    || (supported.len() > 1 && Some(emulator_kind) != preferred);
                if should_reset {
                    conn.execute(
                        "UPDATE games SET emulator_kind_json = ?2, updated_at = ?3 WHERE id = ?1",
                        params![
                            id,
                            preferred.map(|value| serde_json::to_string(&value)).transpose()?,
                            Utc::now().to_rfc3339()
                        ],
                    )?;
                    report.reset_emulator_assignments += 1;
                }
            }
        }
        }

        fs::create_dir_all(paths.data_dir.join("artwork"))?;
        fs::create_dir_all(&paths.downloads_dir)?;
        Ok(report)
    }

    pub fn all_games(&self) -> Result<Vec<GameEntry>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id, title, filename, platform_json, generation_json, vibe_tags_json, source_kind_json,
                install_state_json, managed_path, origin_url, origin_label, rom_path, hash,
                emulator_kind_json, checksum, size_bytes, play_count, last_played_at,
                discovered_at, updated_at, source_refs_json, error_message, progress
            FROM games
        "#,
        )?;

        let mut rows = stmt.query([])?;
        let mut games = Vec::new();
        while let Some(row) = rows.next()? {
            let last_played_at: Option<String> = row.get(17)?;
            let discovered_at: String = row.get(18)?;
            let updated_at: String = row.get(19)?;
            let size_bytes: Option<i64> = row.get(15)?;
            let progress: Option<i64> = row.get(22)?;
            games.push(GameEntry {
                id: row.get(0)?,
                title: row.get(1)?,
                filename: row.get(2)?,
                platform: serde_json::from_str(&row.get::<_, String>(3)?)?,
                generation: serde_json::from_str(&row.get::<_, String>(4)?)?,
                vibe_tags: serde_json::from_str(&row.get::<_, String>(5)?)?,
                source_kind: serde_json::from_str(&row.get::<_, String>(6)?)?,
                install_state: serde_json::from_str(&row.get::<_, String>(7)?)?,
                managed_path: row.get::<_, Option<String>>(8)?.map(PathBuf::from),
                origin_url: row.get(9)?,
                origin_label: row.get(10)?,
                rom_path: row.get::<_, Option<String>>(11)?.map(PathBuf::from),
                hash: row.get(12)?,
                emulator_kind: match row.get::<_, Option<String>>(13)? {
                    Some(raw) => Some(serde_json::from_str(&raw)?),
                    None => None,
                },
                checksum: row.get(14)?,
                size_bytes: size_bytes.map(|value| value as u64),
                play_count: row.get::<_, i64>(16)? as u32,
                last_played_at: last_played_at
                    .map(|value| {
                        DateTime::parse_from_rfc3339(&value).map(|dt| dt.with_timezone(&Utc))
                    })
                    .transpose()?,
                discovered_at: DateTime::parse_from_rfc3339(&discovered_at)?.with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&Utc),
                source_refs: serde_json::from_str(&row.get::<_, String>(20)?)?,
                error_message: row.get(21)?,
                progress: progress.map(|value| value as u8),
            });
        }
        sort_games(&mut games);
        Ok(games)
    }

    /// Load all games with their resolved metadata in a single query using JOIN.
    /// This eliminates the N+1 query pattern when fetching metadata for each game.
    pub fn all_games_with_metadata(&self) -> Result<Vec<(GameEntry, Option<ResolvedMetadata>)>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                g.id, g.title, g.filename, g.platform_json, g.generation_json, g.vibe_tags_json,
                g.source_kind_json, g.install_state_json, g.managed_path, g.origin_url,
                g.origin_label, g.rom_path, g.hash, g.emulator_kind_json, g.checksum, g.size_bytes,
                g.play_count, g.last_played_at, g.discovered_at, g.updated_at, g.source_refs_json,
                g.error_message, g.progress,
                rm.canonical_title, rm.normalized_title, rm.match_state_json, rm.match_confidence,
                rm.provider_ids_json, rm.artwork_json, rm.tags_json, rm.genres_json,
                rm.unmatched_reason, rm.updated_at as rm_updated_at
            FROM games g
            LEFT JOIN resolved_metadata rm ON g.id = rm.game_id
            "#,
        )?;

        let mut rows = stmt.query([])?;
        let mut results = Vec::new();

        while let Some(row) = rows.next()? {
            // Parse GameEntry fields (0-22)
            let last_played_at: Option<String> = row.get(17)?;
            let discovered_at: String = row.get(18)?;
            let updated_at: String = row.get(19)?;
            let size_bytes: Option<i64> = row.get(15)?;
            let progress: Option<i64> = row.get(22)?;

            let game = GameEntry {
                id: row.get(0)?,
                title: row.get(1)?,
                filename: row.get(2)?,
                platform: serde_json::from_str(&row.get::<_, String>(3)?)?,
                generation: serde_json::from_str(&row.get::<_, String>(4)?)?,
                vibe_tags: serde_json::from_str(&row.get::<_, String>(5)?)?,
                source_kind: serde_json::from_str(&row.get::<_, String>(6)?)?,
                install_state: serde_json::from_str(&row.get::<_, String>(7)?)?,
                managed_path: row.get::<_, Option<String>>(8)?.map(PathBuf::from),
                origin_url: row.get(9)?,
                origin_label: row.get(10)?,
                rom_path: row.get::<_, Option<String>>(11)?.map(PathBuf::from),
                hash: row.get(12)?,
                emulator_kind: match row.get::<_, Option<String>>(13)? {
                    Some(raw) => Some(serde_json::from_str(&raw)?),
                    None => None,
                },
                checksum: row.get(14)?,
                size_bytes: size_bytes.map(|value| value as u64),
                play_count: row.get::<_, i64>(16)? as u32,
                last_played_at: last_played_at
                    .map(|value| {
                        DateTime::parse_from_rfc3339(&value).map(|dt| dt.with_timezone(&Utc))
                    })
                    .transpose()?,
                discovered_at: DateTime::parse_from_rfc3339(&discovered_at)?.with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&Utc),
                source_refs: serde_json::from_str(&row.get::<_, String>(20)?)?,
                error_message: row.get(21)?,
                progress: progress.map(|value| value as u8),
            };

            // Parse ResolvedMetadata fields (23-31) - all are nullable due to LEFT JOIN
            let metadata = if row.get::<_, Option<String>>(23)?.is_some() {
                Some(ResolvedMetadata {
                    game_id: game.id.clone(),
                    canonical_title: row.get(23)?,
                    normalized_title: row.get(24)?,
                    match_state: serde_json::from_str(&row.get::<_, String>(25)?)?,
                    match_confidence: row.get(26)?,
                    provider_ids: serde_json::from_str(&row.get::<_, String>(27)?)?,
                    artwork: serde_json::from_str(&row.get::<_, String>(28)?)?,
                    tags: serde_json::from_str(&row.get::<_, String>(29)?)?,
                    genres: serde_json::from_str(&row.get::<_, String>(30)?)?,
                    unmatched_reason: row.get(31)?,
                    updated_at: row.get::<_, String>(32)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                })
            } else {
                None
            };

            results.push((game, metadata));
        }

        // Sort by game title using the same logic as sort_games
        results.sort_by(|(a, _), (b, _)| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

        Ok(results)
    }

    /// Load games and metadata into a HashMap for efficient lookup.
    /// Returns (games_vec, metadata_hashmap) for compatibility with existing code.
    pub fn load_games_and_metadata(&self) -> Result<(Vec<GameEntry>, HashMap<String, ResolvedMetadata>)> {
        let results = self.all_games_with_metadata()?;
        let mut games = Vec::with_capacity(results.len());
        let mut metadata_map = HashMap::new();

        for (game, metadata) in results {
            if let Some(meta) = metadata {
                metadata_map.insert(game.id.clone(), meta);
            }
            games.push(game);
        }

        Ok((games, metadata_map))
    }

    pub fn all_resolved_metadata(&self) -> Result<HashMap<String, ResolvedMetadata>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT game_id, canonical_title, normalized_title, match_state_json, match_confidence, provider_ids_json, artwork_json, tags_json, genres_json, unmatched_reason, updated_at FROM resolved_metadata",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ResolvedMetadata {
                game_id: row.get(0)?,
                canonical_title: row.get(1)?,
                normalized_title: row.get(2)?,
                match_state: serde_json::from_str(&row.get::<_, String>(3)?).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?,
                match_confidence: row.get(4)?,
                provider_ids: serde_json::from_str(&row.get::<_, String>(5)?).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?,
                artwork: serde_json::from_str(&row.get::<_, String>(6)?).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?,
                tags: serde_json::from_str(&row.get::<_, String>(7)?).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?,
                genres: serde_json::from_str(&row.get::<_, String>(8)?).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?,
                unmatched_reason: row.get(9)?,
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(10)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?,
            })
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let metadata = row?;
            map.insert(metadata.game_id.clone(), metadata);
        }
        Ok(map)
    }

    pub fn find_resolved_metadata(&self, game_id: &str) -> Result<Option<ResolvedMetadata>> {
        Ok(self.all_resolved_metadata()?.remove(game_id))
    }

    pub fn upsert_resolved_metadata(&self, metadata: &ResolvedMetadata) -> Result<()> {
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO resolved_metadata(game_id, canonical_title, normalized_title, match_state_json, match_confidence, provider_ids_json, artwork_json, tags_json, genres_json, unmatched_reason, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(game_id) DO UPDATE SET
               canonical_title = excluded.canonical_title,
               normalized_title = excluded.normalized_title,
               match_state_json = excluded.match_state_json,
               match_confidence = excluded.match_confidence,
               provider_ids_json = excluded.provider_ids_json,
               artwork_json = excluded.artwork_json,
               tags_json = excluded.tags_json,
               genres_json = excluded.genres_json,
               unmatched_reason = excluded.unmatched_reason,
               updated_at = excluded.updated_at",
            params![
                metadata.game_id,
                metadata.canonical_title,
                metadata.normalized_title,
                serde_json::to_string(&metadata.match_state)?,
                metadata.match_confidence,
                serde_json::to_string(&metadata.provider_ids)?,
                serde_json::to_string(&metadata.artwork)?,
                serde_json::to_string(&metadata.tags)?,
                serde_json::to_string(&metadata.genres)?,
                metadata.unmatched_reason,
                metadata.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn upsert_metadata_cache(
        &self,
        query: &MetadataQuery,
        metadata: &ResolvedMetadata,
    ) -> Result<()> {
        let conn = self.connect()?;
        let keys = cache_keys(query);
        for key in keys {
            conn.execute(
                "INSERT INTO metadata_cache(cache_key, hash, normalized_title, platform_json, canonical_title, match_state_json, match_confidence, provider_ids_json, artwork_json, tags_json, genres_json, unmatched_reason, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT(cache_key) DO UPDATE SET
                   hash = excluded.hash,
                   normalized_title = excluded.normalized_title,
                   platform_json = excluded.platform_json,
                   canonical_title = excluded.canonical_title,
                   match_state_json = excluded.match_state_json,
                   match_confidence = excluded.match_confidence,
                   provider_ids_json = excluded.provider_ids_json,
                   artwork_json = excluded.artwork_json,
                   tags_json = excluded.tags_json,
                   genres_json = excluded.genres_json,
                   unmatched_reason = excluded.unmatched_reason,
                   updated_at = excluded.updated_at",
                params![
                    key,
                    query.hash,
                    query.normalized_title,
                    serde_json::to_string(&query.platform)?,
                    metadata.canonical_title,
                    serde_json::to_string(&metadata.match_state)?,
                    metadata.match_confidence,
                    serde_json::to_string(&metadata.provider_ids)?,
                    serde_json::to_string(&metadata.artwork)?,
                    serde_json::to_string(&metadata.tags)?,
                    serde_json::to_string(&metadata.genres)?,
                    metadata.unmatched_reason,
                    metadata.updated_at.to_rfc3339(),
                ],
            )?;
        }
        Ok(())
    }

    pub fn find_cached_metadata(&self, query: &MetadataQuery) -> Result<Option<ResolvedMetadata>> {
        let conn = self.connect()?;
        for key in cache_keys(query) {
            let row = conn
                .query_row(
                    "SELECT canonical_title, normalized_title, match_state_json, match_confidence, provider_ids_json, artwork_json, tags_json, genres_json, unmatched_reason, updated_at FROM metadata_cache WHERE cache_key = ?1",
                    params![key],
                    |row| {
                        Ok(ResolvedMetadata {
                            game_id: query.game_id.clone(),
                            canonical_title: row.get(0)?,
                            normalized_title: row.get(1)?,
                            match_state: serde_json::from_str(&row.get::<_, String>(2)?)
                                .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err)))?,
                            match_confidence: row.get(3)?,
                            provider_ids: serde_json::from_str(&row.get::<_, String>(4)?)
                                .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err)))?,
                            artwork: serde_json::from_str(&row.get::<_, String>(5)?)
                                .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err)))?,
                            tags: serde_json::from_str(&row.get::<_, String>(6)?)
                                .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err)))?,
                            genres: serde_json::from_str(&row.get::<_, String>(7)?)
                                .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err)))?,
                            unmatched_reason: row.get(8)?,
                            updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                                .map(|dt| dt.with_timezone(&Utc))
                                .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err)))?,
                        })
                    },
                )
                .optional()?;
            if row.is_some() {
                return Ok(row);
            }
        }
        Ok(None)
    }

    pub fn upsert_game(&self, game: &GameEntry) -> Result<()> {
        let conn = self.connect()?;
        conn.execute(
            r#"
            INSERT INTO games (
                id, title, filename, platform_json, generation_json, vibe_tags_json, source_kind_json,
                install_state_json, managed_path, origin_url, origin_label, rom_path, hash,
                emulator_kind_json, checksum, size_bytes, play_count, last_played_at,
                discovered_at, updated_at, source_refs_json, error_message, progress
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                ?19, ?20, ?21, ?22, ?23
            )
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                filename = excluded.filename,
                platform_json = excluded.platform_json,
                generation_json = excluded.generation_json,
                vibe_tags_json = excluded.vibe_tags_json,
                source_kind_json = excluded.source_kind_json,
                install_state_json = excluded.install_state_json,
                managed_path = excluded.managed_path,
                origin_url = excluded.origin_url,
                origin_label = excluded.origin_label,
                rom_path = excluded.rom_path,
                hash = COALESCE(excluded.hash, games.hash),
                emulator_kind_json = excluded.emulator_kind_json,
                checksum = excluded.checksum,
                size_bytes = excluded.size_bytes,
                play_count = excluded.play_count,
                last_played_at = excluded.last_played_at,
                discovered_at = excluded.discovered_at,
                updated_at = excluded.updated_at,
                source_refs_json = excluded.source_refs_json,
                error_message = excluded.error_message,
                progress = excluded.progress
        "#,
            params![
                game.id,
                game.title,
                game.filename,
                serde_json::to_string(&game.platform)?,
                serde_json::to_string(&game.generation)?,
                serde_json::to_string(&game.vibe_tags)?,
                serde_json::to_string(&game.source_kind)?,
                serde_json::to_string(&game.install_state)?,
                game.managed_path.as_ref().map(|path| path.to_string_lossy().to_string()),
                game.origin_url,
                game.origin_label,
                game.rom_path.as_ref().map(|path| path.to_string_lossy().to_string()),
                game.hash,
                game.emulator_kind.map(|value| serde_json::to_string(&value)).transpose()?,
                game.checksum,
                game.size_bytes.map(|value| value as i64),
                game.play_count,
                game.last_played_at.map(|value| value.to_rfc3339()),
                game.discovered_at.to_rfc3339(),
                game.updated_at.to_rfc3339(),
                serde_json::to_string(&game.source_refs)?,
                game.error_message,
                game.progress.map(i64::from),
            ],
        )?;
        Ok(())
    }

    pub fn remove_game(&self, id: &str) -> Result<()> {
        let conn = self.connect()?;
        conn.execute("DELETE FROM games WHERE id = ?1", params![id])?;
        conn.execute(
            "DELETE FROM resolved_metadata WHERE game_id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn transfer_resolved_metadata(&self, from_game_id: &str, to_game_id: &str) -> Result<()> {
        if from_game_id == to_game_id {
            return Ok(());
        }
        let Some(mut metadata) = self.find_resolved_metadata(from_game_id)? else {
            return Ok(());
        };
        metadata.game_id = to_game_id.to_string();
        metadata.updated_at = Utc::now();
        self.upsert_resolved_metadata(&metadata)?;
        let conn = self.connect()?;
        conn.execute(
            "DELETE FROM resolved_metadata WHERE game_id = ?1",
            params![from_game_id],
        )?;
        Ok(())
    }

    pub fn find_by_hash(&self, hash: &str) -> Result<Option<GameEntry>> {
        let conn = self.connect()?;
        let id: Option<String> = conn
            .query_row(
                "SELECT id FROM games WHERE hash = ?1 LIMIT 1",
                params![hash],
                |row| row.get(0),
            )
            .optional()?;
        match id {
            Some(id) => self.find_by_id(&id),
            None => Ok(None),
        }
    }

    pub fn find_by_id(&self, id: &str) -> Result<Option<GameEntry>> {
        let games = self.all_games()?;
        Ok(games.into_iter().find(|game| game.id == id))
    }

    pub fn record_launch(&self, id: &str) -> Result<()> {
        let conn = self.connect()?;
        conn.execute(
            "UPDATE games SET play_count = play_count + 1, last_played_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![id, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn set_game_emulator_kind(&self, id: &str, emulator_kind: Option<EmulatorKind>) -> Result<()> {
        let conn = self.connect()?;
        conn.execute(
            "UPDATE games SET emulator_kind_json = ?2, updated_at = ?3 WHERE id = ?1",
            params![
                id,
                emulator_kind.map(|value| serde_json::to_string(&value)).transpose()?,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn clear_metadata_cache(&self) -> Result<()> {
        let conn = self.connect()?;
        conn.execute("DELETE FROM resolved_metadata", [])?;
        conn.execute("DELETE FROM metadata_cache", [])?;
        Ok(())
    }

    pub fn merge_catalog_entries(&self, entries: &[CatalogEntry]) -> Result<()> {
        for entry in entries {
            if entry.url.trim().is_empty() {
                continue;
            }
            let id = format!(
                "catalog:{}",
                blake3::hash(format!("{}::{}", entry.url, entry.filename).as_bytes()).to_hex()
            );
            if self.find_by_id(&id)?.is_some() {
                continue;
            }
            let now = Utc::now();
            let game = GameEntry {
                id,
                title: entry.title.clone(),
                filename: Some(entry.filename.clone()),
                platform: entry.platform,
                generation: entry.platform.generation(),
                vibe_tags: entry.platform.default_vibes(),
                source_kind: entry.source_kind,
                install_state: if crate::models::default_emulator_for(entry.platform).is_some() {
                    InstallState::DownloadAvailable
                } else {
                    InstallState::Unsupported
                },
                managed_path: None,
                origin_url: Some(entry.url.clone()),
                origin_label: Some(entry.legal_label.clone()),
                rom_path: None,
                hash: None,
                emulator_kind: crate::models::default_emulator_for(entry.platform),
                checksum: entry.checksum.clone(),
                size_bytes: None,
                play_count: 0,
                last_played_at: None,
                discovered_at: now,
                updated_at: now,
                source_refs: vec![crate::models::GameSourceRef {
                    kind: entry.source_kind,
                    label: Some(entry.legal_label.clone()),
                    origin: Some(entry.url.clone()),
                }],
                error_message: None,
                progress: None,
            };
            self.upsert_game(&game)?;
        }
        Ok(())
    }
}

fn cache_keys(query: &MetadataQuery) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(hash) = &query.hash {
        keys.push(format!("hash:{hash}"));
    }
    keys.push(format!(
        "title:{}:{}",
        serde_json::to_string(&query.platform).unwrap_or_else(|_| "\"Unknown\"".to_string()),
        query.normalized_title
    ));
    keys
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::metadata::normalize_title;
    use crate::models::{
        ArtworkRecord, EmulatorKind, GameSourceRef, GenerationTag, Platform, SourceKind, VibeTag,
    };

    #[test]
    fn stores_and_loads_games() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.sqlite")).unwrap();
        let now = Utc::now();
        let game = GameEntry {
            id: "1".into(),
            title: "Pokemon".into(),
            filename: Some("poke.gba".into()),
            platform: Platform::GameBoyAdvance,
            generation: GenerationTag::Millennials,
            vibe_tags: vec![VibeTag::Tactile],
            source_kind: SourceKind::LocalScan,
            install_state: InstallState::Ready,
            managed_path: None,
            origin_url: None,
            origin_label: None,
            rom_path: Some(PathBuf::from("/tmp/poke.gba")),
            hash: Some("abc".into()),
            emulator_kind: crate::models::default_emulator_for(Platform::GameBoyAdvance),
            checksum: None,
            size_bytes: Some(128),
            play_count: 0,
            last_played_at: None,
            discovered_at: now,
            updated_at: now,
            source_refs: vec![GameSourceRef {
                kind: SourceKind::LocalScan,
                label: None,
                origin: None,
            }],
            error_message: None,
            progress: None,
        };
        db.upsert_game(&game).unwrap();
        let games = db.all_games().unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Pokemon");
    }

    #[test]
    fn caches_resolved_metadata() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.sqlite")).unwrap();
        let query = MetadataQuery {
            game_id: "1".into(),
            raw_title: "Contra".into(),
            normalized_title: normalize_title("Contra"),
            platform: Platform::Nes,
            hash: Some("deadbeef".into()),
            origin_url: None,
        };
        let metadata = ResolvedMetadata {
            game_id: "1".into(),
            canonical_title: "Contra".into(),
            normalized_title: normalize_title("Contra"),
            match_state: MetadataMatchState::Resolved,
            match_confidence: 0.9,
            provider_ids: vec!["starter-pack".into()],
            artwork: ArtworkRecord {
                cached_path: None,
                remote_url: None,
                source: None,
            },
            tags: vec!["Classic".into()],
            genres: vec!["Run and Gun".into()],
            unmatched_reason: None,
            updated_at: Utc::now(),
        };
        db.upsert_metadata_cache(&query, &metadata).unwrap();
        let cached = db.find_cached_metadata(&query).unwrap().unwrap();
        assert_eq!(cached.canonical_title, "Contra");
    }

    #[test]
    fn repair_resets_legacy_nes_retroarch_assignment() {
        let dir = tempdir().unwrap();
        let paths = AppPaths {
            config_dir: dir.path().join("cfg"),
            data_dir: dir.path().join("data"),
            downloads_dir: dir.path().join("downloads"),
            db_path: dir.path().join("data/library.sqlite3"),
            config_path: dir.path().join("cfg/config.toml"),
        };
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::create_dir_all(&paths.data_dir).unwrap();
        fs::create_dir_all(&paths.downloads_dir).unwrap();
        let rom_path = dir.path().join("lion_king.nes");
        fs::write(&rom_path, b"nes").unwrap();

        let db = Database::new(&paths.db_path).unwrap();
        let now = Utc::now();
        db.upsert_game(&GameEntry {
            id: "game:1".into(),
            title: "Lion King".into(),
            filename: Some("lion_king.nes".into()),
            platform: Platform::Nes,
            generation: GenerationTag::GenX,
            vibe_tags: vec![VibeTag::Simple],
            source_kind: SourceKind::LocalScan,
            install_state: InstallState::Ready,
            managed_path: None,
            origin_url: None,
            origin_label: None,
            rom_path: Some(rom_path),
            hash: Some("abc".into()),
            emulator_kind: Some(EmulatorKind::RetroArch),
            checksum: None,
            size_bytes: None,
            play_count: 0,
            last_played_at: None,
            discovered_at: now,
            updated_at: now,
            source_refs: vec![GameSourceRef {
                kind: SourceKind::LocalScan,
                label: None,
                origin: None,
            }],
            error_message: None,
            progress: None,
        })
        .unwrap();

        let report = db.repair_and_migrate_state(&paths).unwrap();
        let repaired = db.find_by_id("game:1").unwrap().unwrap();
        assert_eq!(report.reset_emulator_assignments, 1);
        assert_eq!(repaired.emulator_kind, crate::models::default_emulator_for(Platform::Nes));
    }
}
