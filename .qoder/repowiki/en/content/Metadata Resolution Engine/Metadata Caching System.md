# Metadata Caching System

<cite>
**Referenced Files in This Document**
- [metadata.rs](file://src/metadata.rs)
- [db.rs](file://src/db.rs)
- [artwork.rs](file://src/artwork.rs)
- [config.rs](file://src/config.rs)
- [models.rs](file://src/models.rs)
- [workers.rs](file://src/app/workers.rs)
- [maintenance.rs](file://src/maintenance.rs)
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

## Introduction
This document explains the metadata caching and resolution workflow in the application. It covers how metadata is looked up from cache, validated, and refreshed when needed; how artwork is fetched and stored locally; and how the system ensures consistency and performance. It also documents configuration options, storage locations, cache invalidation strategies, and troubleshooting guidance.

## Project Structure
The metadata caching system spans several modules:
- Metadata service orchestrates enrichment and cache operations
- Database layer persists resolved metadata and maintains a separate metadata cache table
- Artwork subsystem resolves and renders artwork from cached or companion files
- Configuration defines storage locations for data and artwork
- Workers coordinate background metadata resolution tasks
- Maintenance utilities provide cache clearing and repair operations

```mermaid
graph TB
subgraph "Application"
UI["UI Layer"]
Workers["Workers Module"]
end
subgraph "Metadata Subsystem"
MetadataService["MetadataService<br/>enrich_game, materialize"]
Providers["Providers<br/>StarterPack, EmuLand, CatalogTitle, FilenameHeuristic"]
Cache["SQLite Cache<br/>metadata_cache table"]
Resolved["Resolved Metadata<br/>resolved_metadata table"]
end
subgraph "Artwork Subsystem"
ArtworkCtrl["ArtworkController<br/>resolve_artwork, sync_to_game"]
Storage["Local Storage<br/>data_dir/artwork"]
end
subgraph "Configuration"
Paths["AppPaths<br/>data_dir, downloads_dir"]
end
UI --> Workers
Workers --> MetadataService
MetadataService --> Providers
MetadataService --> Cache
MetadataService --> Resolved
ArtworkCtrl --> Storage
MetadataService --> ArtworkCtrl
Paths --> Storage
```

**Diagram sources**
- [metadata.rs:237-369](file://src/metadata.rs#L237-L369)
- [db.rs:96-113](file://src/db.rs#L96-L113)
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)
- [config.rs:10-17](file://src/config.rs#L10-L17)
- [workers.rs:33-57](file://src/app/workers.rs#L33-L57)

**Section sources**
- [metadata.rs:237-369](file://src/metadata.rs#L237-L369)
- [db.rs:96-113](file://src/db.rs#L96-L113)
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)
- [config.rs:10-17](file://src/config.rs#L10-L17)
- [workers.rs:33-57](file://src/app/workers.rs#L33-L57)

## Core Components
- MetadataService: Central coordinator for metadata enrichment, cache lookup, and materialization
- Database: Persistent storage for resolved metadata and metadata cache with indexing
- ArtworkController: Resolves artwork from cached files, companion files, or remote URLs
- AppPaths: Defines storage locations for data and artwork
- Workers: Spawns background tasks to enrich metadata for all games

Key responsibilities:
- Cache lookup in enrich_game using composite keys
- Materialization of provider matches into ResolvedMetadata with artwork caching
- Local artwork storage with extension handling and existence checks
- Cache invalidation via maintenance actions and explicit clearing

**Section sources**
- [metadata.rs:279-321](file://src/metadata.rs#L279-L321)
- [db.rs:587-623](file://src/db.rs#L587-L623)
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)
- [config.rs:10-17](file://src/config.rs#L10-L17)
- [workers.rs:33-57](file://src/app/workers.rs#L33-L57)

## Architecture Overview
The metadata workflow integrates providers, cache, and artwork subsystems:

```mermaid
sequenceDiagram
participant Worker as "Worker Thread"
participant Service as "MetadataService"
participant DB as "Database"
participant Provider as "Providers"
participant Artwork as "ArtworkController"
Worker->>Service : enrich_game(game)
Service->>DB : find_resolved_metadata(game_id)
DB-->>Service : existing metadata or none
alt Existing confirmed metadata
Service-->>Worker : return existing metadata
else No existing resolved metadata
Service->>DB : find_cached_metadata(query)
DB-->>Service : cached metadata or none
alt Cache hit
Service->>DB : upsert_resolved_metadata(resolved)
Service-->>Worker : return cached metadata
else Cache miss
Service->>Provider : identify(query) x N providers
Provider-->>Service : best candidate(s)
Service->>Service : materialize(game_id, matched)
Service->>DB : upsert_resolved_metadata(resolved)
Service->>DB : upsert_metadata_cache(query, resolved)
Service-->>Worker : return resolved metadata
end
end
```

**Diagram sources**
- [workers.rs:42-57](file://src/app/workers.rs#L42-L57)
- [metadata.rs:279-321](file://src/metadata.rs#L279-L321)
- [db.rs:587-623](file://src/db.rs#L587-L623)
- [db.rs:543-585](file://src/db.rs#L543-L585)

## Detailed Component Analysis

### MetadataService: Cache Lookup, Validation, and Invalidation
- enrich_game performs:
  - Early exit if user-confirmed metadata exists
  - Cache lookup using find_cached_metadata with composite keys
  - Upsert resolved metadata upon cache hit
  - Provider identification and merging when cache misses
  - Materialization and persistence of resolved metadata and cache
- Cache keys are generated per query to support hash and title+platform lookups
- Invalidation occurs via maintenance actions and explicit clearing

```mermaid
flowchart TD
Start(["enrich_game(game)"]) --> CheckExisting["Check existing resolved metadata by game_id"]
CheckExisting --> HasExisting{"Existing exists<br/>and marked confirmed?"}
HasExisting --> |Yes| ReturnExisting["Return existing metadata"]
HasExisting --> |No| CheckCache["Find cached metadata by query keys"]
CheckCache --> CacheHit{"Cache hit?"}
CacheHit --> |Yes| UpsertResolved["Upsert resolved metadata"]
UpsertResolved --> ReturnCached["Return cached metadata"]
CacheHit --> |No| Identify["Resolve best match from providers"]
Identify --> Materialize["Materialize to ResolvedMetadata"]
Materialize --> PersistResolved["Upsert resolved metadata"]
PersistResolved --> PersistCache["Upsert metadata cache"]
PersistCache --> ReturnResolved["Return resolved metadata"]
```

**Diagram sources**
- [metadata.rs:279-321](file://src/metadata.rs#L279-L321)
- [db.rs:587-623](file://src/db.rs#L587-L623)
- [db.rs:543-585](file://src/db.rs#L543-L585)

**Section sources**
- [metadata.rs:279-321](file://src/metadata.rs#L279-L321)
- [db.rs:587-623](file://src/db.rs#L587-L623)
- [db.rs:820-831](file://src/db.rs#L820-L831)

### materialize: Converting Provider Matches to ResolvedMetadata
- Creates ResolvedMetadata from MetadataMatch
- Optionally caches artwork via cache_artwork and sets cached_path
- Sets artwork source and updated timestamps

```mermaid
flowchart TD
Start(["materialize(game_id, matched)"]) --> HasArtwork{"matched.artwork_url present?"}
HasArtwork --> |Yes| CacheArtwork["cache_artwork(remote_url, normalized_title)"]
HasArtwork --> |No| SkipCache["Skip artwork caching"]
CacheArtwork --> BuildResolved["Build ResolvedMetadata with cached_path"]
SkipCache --> BuildResolved
BuildResolved --> ReturnResolved["Return ResolvedMetadata"]
```

**Diagram sources**
- [metadata.rs:323-347](file://src/metadata.rs#L323-L347)
- [metadata.rs:349-368](file://src/metadata.rs#L349-L368)

**Section sources**
- [metadata.rs:323-347](file://src/metadata.rs#L323-L347)
- [metadata.rs:349-368](file://src/metadata.rs#L349-L368)

### cache_artwork: URL Processing, Extension Handling, and Local Storage
- Extracts file extension from URL tail; defaults to png if invalid
- Sanitizes title stem and constructs a stable filename
- Checks local cache existence; if absent, downloads and writes bytes
- Returns the path to the cached artwork

```mermaid
flowchart TD
Start(["cache_artwork(remote_url, stem)"]) --> Ext["Extract extension from URL tail"]
Ext --> Sanitize["Sanitize stem to safe filename"]
Sanitize --> Construct["Construct cache path under data_dir/artwork"]
Construct --> Exists{"Path exists?"}
Exists --> |Yes| ReturnPath["Return existing path"]
Exists --> |No| Download["Fetch bytes from remote URL"]
Download --> Write["Write bytes to disk"]
Write --> ReturnPath
```

**Diagram sources**
- [metadata.rs:349-368](file://src/metadata.rs#L349-L368)
- [config.rs:10-17](file://src/config.rs#L10-L17)

**Section sources**
- [metadata.rs:349-368](file://src/metadata.rs#L349-L368)
- [config.rs:10-17](file://src/config.rs#L10-L17)

### Database Cache Keys and Storage
- Composite cache keys:
  - hash:<hash> when available
  - title:<platform>:<normalized_title>
- metadata_cache table stores serialized ResolvedMetadata fields
- resolved_metadata table stores final resolved metadata for UI and rendering
- Indexes on hash and normalized_title optimize lookups

```mermaid
erDiagram
METADATA_CACHE {
text cache_key PK
text hash
text normalized_title
text platform_json
text canonical_title
text match_state_json
real match_confidence
text provider_ids_json
text artwork_json
text tags_json
text genres_json
text unmatched_reason
text updated_at
}
RESOLVED_METADATA {
text game_id PK
text canonical_title
text normalized_title
text match_state_json
real match_confidence
text provider_ids_json
text artwork_json
text tags_json
text genres_json
text unmatched_reason
text updated_at
}
```

**Diagram sources**
- [db.rs:96-113](file://src/db.rs#L96-L113)
- [db.rs:543-585](file://src/db.rs#L543-L585)
- [db.rs:587-623](file://src/db.rs#L587-L623)

**Section sources**
- [db.rs:820-831](file://src/db.rs#L820-L831)
- [db.rs:96-113](file://src/db.rs#L96-L113)
- [db.rs:543-585](file://src/db.rs#L543-L585)
- [db.rs:587-623](file://src/db.rs#L587-L623)

### Artwork Resolution and Rendering
- ArtworkController.resolve_artwork prioritizes:
  - Cached artwork from resolved metadata
  - Companion artwork files adjacent to ROMs
  - Fallback cache files under data_dir/artwork with sanitized stems
- Supports terminal rendering via image protocols when available

```mermaid
flowchart TD
Start(["resolve_artwork(paths, game, metadata)"]) --> CheckMeta["Check metadata.artwork.cached_path"]
CheckMeta --> MetaExists{"Exists?"}
MetaExists --> |Yes| ReturnMeta["Return (CachedFile, path)"]
MetaExists --> |No| CheckROM["Locate ROM path and check companions"]
CheckROM --> Companions{"Any companion exists?"}
Companions --> |Yes| ReturnComp["Return (CompanionFile, path)"]
Companions --> |No| CheckFallback["Check data_dir/artwork/<sanitized_id>.{png,jpg,...}"]
CheckFallback --> FallbackExists{"Exists?"}
FallbackExists --> |Yes| ReturnFallback["Return (CachedFile, path)"]
FallbackExists --> |No| None["Return None"]
```

**Diagram sources**
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)
- [config.rs:10-17](file://src/config.rs#L10-L17)

**Section sources**
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)
- [config.rs:10-17](file://src/config.rs#L10-L17)

### Background Metadata Enrichment
- Workers spawn a metadata job per game during startup
- Each job constructs a MetadataService and calls enrich_game
- Results are sent back via WorkerEvent for UI updates

```mermaid
sequenceDiagram
participant App as "App"
participant Worker as "Worker Thread"
participant Service as "MetadataService"
App->>Worker : spawn_metadata_job(game)
Worker->>Service : new(Database, AppPaths)
Worker->>Service : enrich_game(game)
Service-->>Worker : ResolvedMetadata or error
Worker-->>App : WorkerEvent : : MetadataResolved
```

**Diagram sources**
- [workers.rs:33-57](file://src/app/workers.rs#L33-L57)
- [metadata.rs:279-321](file://src/metadata.rs#L279-L321)

**Section sources**
- [workers.rs:33-57](file://src/app/workers.rs#L33-L57)

## Dependency Analysis
- MetadataService depends on:
  - Database for cache and resolved metadata persistence
  - AppPaths for artwork storage location
  - Providers for candidate matching
- ArtworkController depends on:
  - AppPaths for data_dir/artwork
  - GameEntry and ResolvedMetadata for resolution logic
- Workers depend on MetadataService for enrichment tasks

```mermaid
graph LR
MetadataService --> Database
MetadataService --> AppPaths
MetadataService --> Providers
ArtworkController --> AppPaths
ArtworkController --> GameEntry
ArtworkController --> ResolvedMetadata
Workers --> MetadataService
```

**Diagram sources**
- [metadata.rs:237-369](file://src/metadata.rs#L237-L369)
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)
- [workers.rs:33-57](file://src/app/workers.rs#L33-L57)

**Section sources**
- [metadata.rs:237-369](file://src/metadata.rs#L237-L369)
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)
- [workers.rs:33-57](file://src/app/workers.rs#L33-L57)

## Performance Considerations
- Cache hit scenarios:
  - Resolved metadata by game_id avoids provider lookups
  - Cached metadata by hash/title keys avoid network requests
- Performance benefits:
  - Single-pass join loads games and metadata efficiently
  - Indexes on metadata_cache hash and normalized_title reduce lookup cost
- Storage optimization:
  - Local artwork caching reduces repeated network fetches
  - Sanitized filenames prevent filesystem issues and collisions
- Concurrency:
  - Background worker threads isolate blocking operations (network and IO)
  - SQLite transactions and upserts minimize contention

[No sources needed since this section provides general guidance]

## Troubleshooting Guide
Common cache-related issues and resolutions:
- Cache appears stale or incorrect
  - Trigger maintenance action to clear metadata cache and artwork cache
  - Re-run metadata enrichment to repopulate cache
- Artwork not displaying
  - Verify artwork exists in data_dir/artwork with sanitized filename
  - Confirm companion artwork files exist alongside ROMs
  - Check terminal image protocol support
- Provider matches not persisting
  - Ensure resolved metadata upsert succeeds after cache miss
  - Validate cache keys include both hash and title+platform
- Storage location misconfiguration
  - Confirm AppPaths.data_dir points to writable directory
  - Ensure data_dir/artwork exists and is accessible

Operational commands:
- Clear metadata cache and artwork cache
  - maintenance clear-metadata
- Repair state and reset broken downloads/emulators
  - maintenance repair

**Section sources**
- [maintenance.rs:28-47](file://src/maintenance.rs#L28-L47)
- [db.rs:761-766](file://src/db.rs#L761-L766)
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)

## Conclusion
The metadata caching system combines provider-driven enrichment with robust local caching and artwork storage. It optimizes performance through cache hits, composite keys, and background processing, while ensuring reliability via maintenance tools and graceful fallbacks. Proper configuration of storage locations and understanding of cache invalidation strategies help maintain a responsive and accurate metadata experience.