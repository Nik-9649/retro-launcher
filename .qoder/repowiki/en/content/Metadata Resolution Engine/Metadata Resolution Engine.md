# Metadata Resolution Engine

<cite>
**Referenced Files in This Document**
- [metadata.rs](file://src/metadata.rs)
- [db.rs](file://src/db.rs)
- [models.rs](file://src/models.rs)
- [config.rs](file://src/config.rs)
- [artwork.rs](file://src/artwork.rs)
- [lib.rs](file://src/lib.rs)
- [app/mod.rs](file://src/app/mod.rs)
- [app/workers.rs](file://src/app/workers.rs)
- [Cargo.toml](file://Cargo.toml)
- [starter_metadata.json](file://support/starter_metadata.json)
</cite>

## Table of Contents
1. [Introduction](#introduction)
2. [Project Structure](#project-structure)
3. [Core Components](#core-components)
4. [Architecture Overview](#architecture-overview)
5. [Detailed Component Analysis](#detailed-component-analysis)
6. [Dependency Analysis](#dependency-analysis)
7. [Performance Considerations](#performance-considerations)
8. [Troubleshooting Guide](#troubleshooting-guide)
9. [Conclusion](#conclusion)
10. [Appendices](#appendices)

## Introduction
This document explains the metadata resolution engine that powers title normalization, provider-based matching, caching, and artwork integration. It covers the provider architecture, matching strategies, caching behavior, and integration with the database and artwork subsystems. It also provides configuration guidance, troubleshooting steps, and optimization tips for large libraries.

## Project Structure
The metadata engine is implemented primarily in the metadata module and integrates with the database, models, artwork, and application orchestration layers.

```mermaid
graph TB
subgraph "Application Layer"
APP["App (app/mod.rs)"]
WORKERS["Workers (app/workers.rs)"]
end
subgraph "Metadata Layer"
METASVC["MetadataService (metadata.rs)"]
PROVIDERS["Providers (metadata.rs)"]
TITLE_NORM["Title Normalization (metadata.rs)"]
end
subgraph "Storage Layer"
DB["Database (db.rs)"]
MODELS["Models (models.rs)"]
end
subgraph "Artwork Layer"
ARTWORK["ArtworkController (artwork.rs)"]
end
APP --> WORKERS
WORKERS --> METASVC
METASVC --> PROVIDERS
METASVC --> DB
METASVC --> ARTWORK
DB --> MODELS
```

**Diagram sources**
- [app/mod.rs:125-170](file://src/app/mod.rs#L125-L170)
- [app/workers.rs:42-57](file://src/app/workers.rs#L42-L57)
- [metadata.rs:237-369](file://src/metadata.rs#L237-L369)
- [db.rs:35-117](file://src/db.rs#L35-L117)
- [artwork.rs:35-208](file://src/artwork.rs#L35-L208)

**Section sources**
- [lib.rs:10-22](file://src/lib.rs#L10-L22)
- [Cargo.toml:6-24](file://Cargo.toml#L6-L24)

## Core Components
- MetadataService orchestrates enrichment of GameEntry records by building a MetadataQuery, invoking providers, merging results, and persisting outcomes.
- Providers implement a common trait and return MetadataMatch results with confidence scores and optional artwork URLs.
- Title normalization transforms noisy filenames into stable comparison tokens.
- Caching persists resolved metadata and supports fast retrieval on subsequent runs.
- Artwork integration caches remote artwork locally and resolves artwork sources for UI rendering.

**Section sources**
- [metadata.rs:237-369](file://src/metadata.rs#L237-L369)
- [metadata.rs:40-43](file://src/metadata.rs#L40-L43)
- [metadata.rs:428-459](file://src/metadata.rs#L428-L459)
- [db.rs:543-623](file://src/db.rs#L543-L623)
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)

## Architecture Overview
The engine follows a pipeline:
- Build a MetadataQuery from a GameEntry.
- Invoke providers in order and collect candidates.
- Merge candidates into a single MetadataMatch using confidence and compatibility heuristics.
- Materialize ResolvedMetadata, optionally cache artwork, and persist to database.
- Resolve artwork for UI rendering.

```mermaid
sequenceDiagram
participant App as "App"
participant Workers as "Workers"
participant MetaSvc as "MetadataService"
participant Prov as "Providers"
participant DB as "Database"
participant Art as "ArtworkController"
App->>Workers : spawn_metadata_job(game)
Workers->>MetaSvc : enrich_game(game)
MetaSvc->>MetaSvc : build MetadataQuery
MetaSvc->>Prov : identify(query) x N
Prov-->>MetaSvc : MetadataMatch (confidence, tags, genres, artwork?)
MetaSvc->>MetaSvc : merge_best_match()
MetaSvc->>Art : cache_artwork(remote_url)
Art-->>MetaSvc : cached_path
MetaSvc->>DB : upsert_resolved_metadata(resolved)
MetaSvc->>DB : upsert_metadata_cache(query, resolved)
DB-->>MetaSvc : ok
MetaSvc-->>Workers : ResolvedMetadata
Workers-->>App : MetadataResolved event
```

**Diagram sources**
- [app/workers.rs:42-57](file://src/app/workers.rs#L42-L57)
- [metadata.rs:279-321](file://src/metadata.rs#L279-L321)
- [metadata.rs:371-408](file://src/metadata.rs#L371-L408)
- [metadata.rs:349-368](file://src/metadata.rs#L349-L368)
- [db.rs:510-585](file://src/db.rs#L510-L585)

## Detailed Component Analysis

### Provider Architecture
The engine defines a trait for pluggable metadata providers and ships with several built-in providers:
- StarterPackProvider: Matches against curated starter metadata with alias lists and platform filtering.
- EmuLandProvider: Performs web scraping of Emu-Land search and detail pages to extract metadata and artwork.
- CatalogTitleProvider: Treats the catalog-provided title as imported metadata.
- FilenameHeuristicProvider: Normalizes the raw filename as a fallback.

```mermaid
classDiagram
class MetadataProvider {
<<trait>>
+id() &str
+identify(query) ProviderResult
}
class StarterPackProvider {
+new() Result<StarterPackProvider>
+id() &str
+identify(query) ProviderResult
}
class EmuLandProvider {
+new() Result<EmuLandProvider>
+id() &str
+identify(query) ProviderResult
-fetch_search(query) Result<Option<String>>
-fetch_detail(href) Result<Option<MetadataMatch>>
}
class CatalogTitleProvider {
+id() &str
+identify(query) ProviderResult
}
class FilenameHeuristicProvider {
+id() &str
+identify(query) ProviderResult
}
MetadataProvider <|.. StarterPackProvider
MetadataProvider <|.. EmuLandProvider
MetadataProvider <|.. CatalogTitleProvider
MetadataProvider <|.. FilenameHeuristicProvider
```

**Diagram sources**
- [metadata.rs:40-43](file://src/metadata.rs#L40-L43)
- [metadata.rs:55-112](file://src/metadata.rs#L55-L112)
- [metadata.rs:170-235](file://src/metadata.rs#L170-L235)
- [metadata.rs:147-168](file://src/metadata.rs#L147-L168)
- [metadata.rs:114-145](file://src/metadata.rs#L114-L145)

**Section sources**
- [metadata.rs:55-112](file://src/metadata.rs#L55-L112)
- [metadata.rs:170-235](file://src/metadata.rs#L170-L235)
- [metadata.rs:147-168](file://src/metadata.rs#L147-L168)
- [metadata.rs:114-145](file://src/metadata.rs#L114-L145)
- [starter_metadata.json:1-89](file://support/starter_metadata.json#L1-L89)

### Title Normalization and Matching Strategies
Normalization removes file extensions, discards content inside parentheses/brackets, converts to lowercase, splits on whitespace, and filters common noise words. Matching strategies:
- Exact alias match: high confidence.
- Containment match: moderate confidence.
- Loose token match: checks mutual token inclusion.
- Secondary artwork merging: when the best candidate lacks artwork, pick artwork from compatible titles among candidates.

```mermaid
flowchart TD
Start(["Normalize Title"]) --> StripExt["Remove extension if recognized"]
StripExt --> CleanTokens["Collapse paren/bracket content<br/>Lowercase alphanum only"]
CleanTokens --> FilterNoise["Filter common noise words"]
FilterNoise --> Tokens["Split into tokens"]
Tokens --> Output(["Normalized String"])
```

**Diagram sources**
- [metadata.rs:428-459](file://src/metadata.rs#L428-L459)

**Section sources**
- [metadata.rs:428-459](file://src/metadata.rs#L428-L459)
- [metadata.rs:461-466](file://src/metadata.rs#L461-L466)
- [metadata.rs:384-408](file://src/metadata.rs#L384-L408)

### MetadataService and Enrichment Pipeline
MetadataService builds a MetadataQuery from a GameEntry, checks for existing user-confirmed metadata, consults cache, invokes providers, merges results, materializes artwork, and persists outcomes.

```mermaid
sequenceDiagram
participant Svc as "MetadataService"
participant DB as "Database"
participant Prov as "Providers"
participant Mat as "Materialize"
Svc->>DB : find_resolved_metadata(game_id)
alt Found and confirmed
DB-->>Svc : ResolvedMetadata
Svc-->>Svc : return early
else Not found or not confirmed
Svc->>DB : find_cached_metadata(query)
alt Cached hit
DB-->>Svc : ResolvedMetadata
Svc->>DB : upsert_resolved_metadata(resolved)
Svc-->>Svc : return cached
else No cache
Svc->>Prov : identify(query) x N
Prov-->>Svc : candidates
Svc->>Svc : merge_best_match()
Svc->>Mat : materialize(game_id, matched)
Mat-->>Svc : ResolvedMetadata
Svc->>DB : upsert_resolved_metadata(resolved)
Svc->>DB : upsert_metadata_cache(query, resolved)
Svc-->>Svc : return resolved
end
end
```

**Diagram sources**
- [metadata.rs:279-321](file://src/metadata.rs#L279-L321)
- [metadata.rs:371-408](file://src/metadata.rs#L371-L408)
- [metadata.rs:323-347](file://src/metadata.rs#L323-L347)
- [db.rs:587-623](file://src/db.rs#L587-L623)
- [db.rs:543-585](file://src/db.rs#L543-L585)

**Section sources**
- [metadata.rs:279-321](file://src/metadata.rs#L279-L321)
- [metadata.rs:323-347](file://src/metadata.rs#L323-L347)

### Caching Mechanism
Two caches are maintained:
- Resolved metadata cache: per-game resolved metadata persisted to the resolved_metadata table.
- Query cache: normalized queries mapped to resolved metadata in metadata_cache with composite keys derived from the query.

```mermaid
erDiagram
GAMES ||--o{ RESOLVED_METADATA : "one game -> one resolved metadata"
GAMES ||--o{ METADATA_CACHE : "one game -> many cache rows"
RESOLVED_METADATA {
text game_id PK
text canonical_title
text normalized_title
text match_state_json
float match_confidence
text provider_ids_json
text artwork_json
text tags_json
text genres_json
text unmatched_reason
text updated_at
}
METADATA_CACHE {
text cache_key PK
text hash
text normalized_title
text platform_json
text canonical_title
text match_state_json
float match_confidence
text provider_ids_json
text artwork_json
text tags_json
text genres_json
text unmatched_reason
text updated_at
}
```

**Diagram sources**
- [db.rs:83-110](file://src/db.rs#L83-L110)
- [db.rs:543-585](file://src/db.rs#L543-L585)
- [db.rs:587-623](file://src/db.rs#L587-L623)

**Section sources**
- [db.rs:83-110](file://src/db.rs#L83-L110)
- [db.rs:543-585](file://src/db.rs#L543-L585)
- [db.rs:587-623](file://src/db.rs#L587-L623)

### Artwork Integration
Artwork is cached locally under the data directory’s artwork folder. The service downloads remote artwork when present and stores it with sanitized filenames. ArtworkController resolves artwork from cached files, companion files, or fallbacks.

```mermaid
flowchart TD
A["Remote artwork_url?"] --> |Yes| B["Sanitize stem and derive extension"]
B --> C["Write to data_dir/artwork/<stem>.<ext>"]
C --> D["Store cached_path in ResolvedMetadata"]
A --> |No| E["No artwork cached"]
```

**Diagram sources**
- [metadata.rs:349-368](file://src/metadata.rs#L349-L368)
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)

**Section sources**
- [metadata.rs:349-368](file://src/metadata.rs#L349-L368)
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)

### Preview and Configuration
Preview mode allows evaluating a title and platform without persisting results. Configuration includes ROM roots, download directories, and preferred emulators. Paths are resolved via AppPaths.

```mermaid
sequenceDiagram
participant User as "Caller"
participant Meta as "preview_metadata_match"
participant Prov as "Providers"
User->>Meta : preview_metadata_match(raw_title, platform, origin_url?)
Meta->>Prov : identify(query) x N
Prov-->>Meta : candidates
Meta->>Meta : merge_best_match()
Meta-->>User : MetadataMatch (no persistence)
```

**Diagram sources**
- [metadata.rs:243-263](file://src/metadata.rs#L243-L263)
- [metadata.rs:371-408](file://src/metadata.rs#L371-L408)

**Section sources**
- [metadata.rs:243-263](file://src/metadata.rs#L243-L263)
- [config.rs:34-64](file://src/config.rs#L34-L64)

## Dependency Analysis
External dependencies relevant to metadata resolution:
- reqwest (blocking): HTTP requests for Emu-Land scraping and artwork fetching.
- serde/serde_json: serialization of models and cache payloads.
- rusqlite: SQLite-backed storage for games, resolved metadata, and cache.
- chrono: timestamps for updated_at fields.

```mermaid
graph LR
METADATA["metadata.rs"] --> REQ["reqwest (blocking)"]
METADATA --> SERDE["serde / serde_json"]
DBMOD["db.rs"] --> SQLITE["rusqlite"]
MODELS["models.rs"] --> SERDE
ARTWORK["artwork.rs"] --> IMAGE["image"]
```

**Diagram sources**
- [Cargo.toml:6-24](file://Cargo.toml#L6-L24)
- [metadata.rs:1-12](file://src/metadata.rs#L1-L12)
- [db.rs:1-17](file://src/db.rs#L1-L17)
- [artwork.rs:1-17](file://src/artwork.rs#L1-L17)

**Section sources**
- [Cargo.toml:6-24](file://Cargo.toml#L6-L24)

## Performance Considerations
- Provider invocation is linear in the number of providers; keep the provider list minimal and ordered by expected quality.
- Caching reduces repeated network calls and database writes; leverage cache hits by ensuring consistent query normalization.
- Merging candidates uses a single pass to select best match and secondary artwork; complexity is O(N) for N candidates.
- Database operations use batched creation of tables and indexes; ensure regular maintenance to keep indices effective.
- For large libraries, consider batching metadata enrichment and staggering network requests to avoid rate limiting.

[No sources needed since this section provides general guidance]

## Troubleshooting Guide
Common issues and resolutions:
- Incomplete metadata
  - Symptom: Unmatched or low-confidence results.
  - Actions: Verify normalized title accuracy, confirm provider order, and inspect starter metadata coverage for the platform.
  - References: [metadata.rs:428-459](file://src/metadata.rs#L428-L459), [metadata.rs:384-408](file://src/metadata.rs#L384-L408)

- Provider failures (network or parsing)
  - Symptom: Empty or partial metadata from a provider.
  - Actions: Retry enrichment, check network connectivity, and review Emu-Land scraping logic; consider adding delays or retries.
  - References: [metadata.rs:182-220](file://src/metadata.rs#L182-L220), [metadata.rs:504-547](file://src/metadata.rs#L504-L547)

- Cache corruption or stale data
  - Symptom: Outdated metadata or inconsistent artwork.
  - Actions: Clear metadata cache; use maintenance commands to reset or repair state if available; re-run enrichment.
  - References: [db.rs:761-766](file://src/db.rs#L761-L766), [db.rs:587-623](file://src/db.rs#L587-L623)

- Artwork not appearing
  - Symptom: Missing or failed artwork rendering.
  - Actions: Confirm artwork URL availability, verify cache write permissions, and check ArtworkController state.
  - References: [metadata.rs:349-368](file://src/metadata.rs#L349-L368), [artwork.rs:215-246](file://src/artwork.rs#L215-L246)

- Conflicts between providers
  - Symptom: Mixed tags/genres or conflicting artwork.
  - Actions: Review merge logic; ensure compatible titles are considered for artwork; adjust provider confidence thresholds if needed.
  - References: [metadata.rs:384-408](file://src/metadata.rs#L384-L408), [metadata.rs:415-426](file://src/metadata.rs#L415-L426)

**Section sources**
- [metadata.rs:182-220](file://src/metadata.rs#L182-L220)
- [metadata.rs:504-547](file://src/metadata.rs#L504-L547)
- [db.rs:761-766](file://src/db.rs#L761-L766)
- [db.rs:587-623](file://src/db.rs#L587-L623)
- [metadata.rs:349-368](file://src/metadata.rs#L349-L368)
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)
- [metadata.rs:384-408](file://src/metadata.rs#L384-L408)
- [metadata.rs:415-426](file://src/metadata.rs#L415-L426)

## Conclusion
The metadata resolution engine combines robust title normalization, a flexible provider model, intelligent merging, and persistent caching to deliver accurate metadata and artwork for games. By tuning provider order, leveraging cache, and monitoring for common failure modes, you can achieve reliable and scalable metadata enrichment across large libraries.

[No sources needed since this section summarizes without analyzing specific files]

## Appendices

### Configuration Options
- AppPaths: Paths for config, data, downloads, and database.
- Config: ROM roots, managed download directory, scan preferences, and preferred emulators.
- Maintenance: Commands to clear metadata cache and repair state.

**Section sources**
- [config.rs:10-64](file://src/config.rs#L10-L64)
- [db.rs:761-766](file://src/db.rs#L761-L766)

### Provider Priority and Matching Behavior
- Provider order determines precedence; earlier providers with higher confidence dominate.
- Matching uses normalized titles and token compatibility for artwork merging.
- CatalogTitleProvider and FilenameHeuristicProvider act as safety nets.

**Section sources**
- [metadata.rs:270-276](file://src/metadata.rs#L270-L276)
- [metadata.rs:384-408](file://src/metadata.rs#L384-L408)
- [metadata.rs:147-168](file://src/metadata.rs#L147-L168)
- [metadata.rs:114-145](file://src/metadata.rs#L114-L145)

### Integration Patterns
- Background enrichment: Workers spawn per-game metadata jobs during startup.
- UI integration: App loads games and metadata together, and synchronizes artwork display.

**Section sources**
- [app/workers.rs:33-57](file://src/app/workers.rs#L33-L57)
- [app/mod.rs:125-170](file://src/app/mod.rs#L125-L170)
- [app/mod.rs:331-347](file://src/app/mod.rs#L331-L347)