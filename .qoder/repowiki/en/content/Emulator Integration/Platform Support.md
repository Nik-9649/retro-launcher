# Platform Support

<cite>
**Referenced Files in This Document**
- [emulator.rs](file://src/emulator.rs)
- [models.rs](file://src/models.rs)
- [scanner.rs](file://src/scanner.rs)
- [config.rs](file://src/config.rs)
- [db.rs](file://src/db.rs)
- [launcher.rs](file://src/launcher.rs)
- [metadata.rs](file://src/metadata.rs)
- [Cargo.toml](file://Cargo.toml)
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
This document explains how the project supports multiple gaming platforms and maps them to appropriate emulators. It covers:
- The platform detection mechanism based on ROM file extensions
- The emulators_for_platform() mapping system and rationale for each platform
- How ROM platform identification influences emulator selection
- Platform-specific limitations, feature differences, and optimization considerations
- Unknown platform handling and fallback strategies
- Preferred emulator configuration and runtime selection

## Project Structure
The platform and emulator support spans several modules:
- Platform and emulator enums, and platform-to-emulator defaults
- ROM scanning and platform detection from file extensions
- Emulator detection, availability, and launch command building
- User preferences for preferred emulators per platform
- Database persistence of platform and emulator assignments
- Launcher integration for invoking emulators

```mermaid
graph TB
subgraph "Core"
M["models.rs<br/>Platform, EmulatorKind, defaults"]
E["emulator.rs<br/>Detection, mapping, commands"]
C["config.rs<br/>Preferred emulators per platform"]
D["db.rs<br/>Persistence of platform and emulator"]
end
subgraph "Scanning"
S["scanner.rs<br/>Scan roots, import files, platform detection"]
MD["metadata.rs<br/>Metadata enrichment (not platform mapping)"]
end
subgraph "Runtime"
L["launcher.rs<br/>Launch selected emulator"]
end
S --> M
S --> D
E --> M
C --> D
L --> E
L --> D
```

**Diagram sources**
- [models.rs:8-106](file://src/models.rs#L8-L106)
- [emulator.rs:45-61](file://src/emulator.rs#L45-L61)
- [config.rs:106-113](file://src/config.rs#L106-L113)
- [db.rs:52-76](file://src/db.rs#L52-L76)
- [scanner.rs:158-273](file://src/scanner.rs#L158-L273)
- [metadata.rs:237-369](file://src/metadata.rs#L237-L369)
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)

**Section sources**
- [models.rs:8-106](file://src/models.rs#L8-L106)
- [emulator.rs:45-61](file://src/emulator.rs#L45-L61)
- [config.rs:106-113](file://src/config.rs#L106-L113)
- [db.rs:52-76](file://src/db.rs#L52-L76)
- [scanner.rs:158-273](file://src/scanner.rs#L158-L273)
- [metadata.rs:237-369](file://src/metadata.rs#L237-L369)
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)

## Core Components
- Platform enumeration and extension-to-platform mapping
- Emulator kinds and default mapping per platform
- Scanner that detects platform from ROM extension and sets install state
- Emulator detection, availability, and command construction
- User preferences for preferred emulators per platform
- Database persistence of platform, emulator assignment, and install state

**Section sources**
- [models.rs:8-106](file://src/models.rs#L8-L106)
- [scanner.rs:158-273](file://src/scanner.rs#L158-L273)
- [emulator.rs:27-127](file://src/emulator.rs#L27-L127)
- [config.rs:106-113](file://src/config.rs#L106-L113)
- [db.rs:52-76](file://src/db.rs#L52-L76)

## Architecture Overview
The platform-to-emulator mapping is centralized and used across scanning, configuration, and launching.

```mermaid
sequenceDiagram
participant FS as "Filesystem"
participant Scan as "Scanner.import_file()"
participant Model as "Platform.from_extension()"
participant DB as "Database.upsert_game()"
participant Pref as "Config.preferred_emulators_for()"
participant Map as "default_emulator_for()"
participant Launch as "Launcher.launch_game()"
participant Emu as "Emulator.build_command()"
FS-->>Scan : "ROM file path"
Scan->>Model : "Parse extension"
Model-->>Scan : "Platform variant"
Scan->>Map : "default_emulator_for(platform)"
Map-->>Scan : "Default EmulatorKind or None"
Scan->>Pref : "preferred_emulators_for(platform)"
Pref-->>Scan : "User-preferred list"
Scan->>DB : "Persist GameEntry with platform and emulator_kind"
Launch->>Emu : "build_command(kind, path, rom_path)"
Emu-->>Launch : "Command"
Launch-->>FS : "Spawn emulator process"
```

**Diagram sources**
- [scanner.rs:193-265](file://src/scanner.rs#L193-L265)
- [models.rs:62-76](file://src/models.rs#L62-L76)
- [models.rs:353-369](file://src/models.rs#L353-L369)
- [config.rs:106-112](file://src/config.rs#L106-L112)
- [db.rs:625-689](file://src/db.rs#L625-L689)
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)
- [emulator.rs:110-127](file://src/emulator.rs#L110-L127)

## Detailed Component Analysis

### Platform Detection and ROM Identification
- Platforms are represented as an enum with display labels and short labels.
- Platform detection is performed by mapping ROM file extensions to platform variants.
- Supported extensions include Game Boy (.gb, .gbc, .gba), NES (.nes), SNES (.sfc/.smc), Genesis (.gen/.md/.smd), N64 (.n64/.z64/.v64), PS1 (.cue/.chd/.m3u/.bin/.img/.iso), NDS (.nds), PS2 (.iso.ps2), Wii, and Xbox 360.
- Unknown extensions map to Unknown platform.

```mermaid
flowchart TD
Start(["ROM path"]) --> Ext["Extract extension"]
Ext --> Map["Platform.from_extension()"]
Map --> GB{"Game Boy family?"}
Map --> NES{"NES?"}
Map --> SNES{"SNES?"}
Map --> GEN{"Genesis?"}
Map --> N64{"N64?"}
Map --> NDS{"NDS?"}
Map --> PS1{"PS1?"}
Map --> PS2{"PS2?"}
Map --> WII{"Wii?"}
Map --> X360{"Xbox 360?"}
Map --> UNK{"Other/Unknown"}
GB --> SetGB["Set Platform::GameBoy/Color/Advance"]
NES --> SetNES["Set Platform::Nes"]
SNES --> SetSNES["Set Platform::Snes"]
GEN --> SetGEN["Set Platform::SegaGenesis"]
N64 --> SetN64["Set Platform::N64"]
NDS --> SetNDS["Set Platform::NintendoDs"]
PS1 --> SetPS1["Set Platform::Ps1"]
PS2 --> SetPS2["Set Platform::Ps2"]
WII --> SetWII["Set Platform::Wii"]
X360 --> SetX360["Set Platform::Xbox360"]
UNK --> SetUNK["Set Platform::Unknown"]
```

**Diagram sources**
- [models.rs:62-76](file://src/models.rs#L62-L76)

**Section sources**
- [models.rs:8-106](file://src/models.rs#L8-L106)

### Emulator Mapping System (emulators_for_platform)
The mapping determines which emulators are available for a given platform. The function returns a vector of EmulatorKind values.

- Game Boy family (Game Boy, Game Boy Color, Game Boy Advance): mGBA
- PlayStation 1: Mednafen
- NES: FCEUX, RetroArch
- SNES, Genesis, N64, Nintendo DS, PS2, Wii, Xbox 360: RetroArch
- Unknown: empty list

```mermaid
flowchart TD
P["Platform"] --> M["emulators_for_platform()"]
M --> GB["Game Boy family -> [mGBA]"]
M --> PS1["PS1 -> [Mednafen]"]
M --> NES["NES -> [FCEUX, RetroArch]"]
M --> OTHER["Other -> [RetroArch]"]
M --> UNK["Unknown -> []"]
```

**Diagram sources**
- [emulator.rs:45-61](file://src/emulator.rs#L45-L61)

**Section sources**
- [emulator.rs:45-61](file://src/emulator.rs#L45-L61)

### Default Emulator Selection (default_emulator_for)
A convenience function returns a single preferred emulator for a platform, used during import and initial assignment.

- Game Boy family: mGBA
- PS1: Mednafen
- NES: FCEUX
- Others: RetroArch
- Unknown: None

```mermaid
flowchart TD
P["Platform"] --> D["default_emulator_for()"]
D --> GB["Game Boy family -> mGBA"]
D --> PS1["PS1 -> Mednafen"]
D --> NES["NES -> FCEUX"]
D --> OTHER["Others -> RetroArch"]
D --> UNK["Unknown -> None"]
```

**Diagram sources**
- [models.rs:353-369](file://src/models.rs#L353-L369)

**Section sources**
- [models.rs:353-369](file://src/models.rs#L353-L369)

### User Preferences for Preferred Emulators
Users can configure preferred emulators per platform. The configuration loader initializes defaults for Game Boy family and PS1/NES, and exposes a method to query preferred emulators for a given platform.

- Defaults include mGBA for Game Boy family and Mednafen/FCEUX for PS1/NES.
- The method preferred_emulators_for(platform) filters the configured list.

```mermaid
classDiagram
class Config {
+Vec~EmulatorPreference~ preferred_emulators
+preferred_emulators_for(platform) Vec~EmulatorKind~
}
class EmulatorPreference {
+Platform platform
+EmulatorKind emulator
}
Config --> EmulatorPreference : "filters by platform"
```

**Diagram sources**
- [config.rs:19-32](file://src/config.rs#L19-L32)
- [config.rs:106-112](file://src/config.rs#L106-L112)

**Section sources**
- [config.rs:106-113](file://src/config.rs#L106-L113)

### Scanner and Install State Resolution
During import, the scanner:
- Detects platform from ROM extension
- Assigns default emulator via default_emulator_for
- Sets install state to Ready if a default emulator exists, otherwise Unsupported
- Persists the GameEntry to the database

```mermaid
sequenceDiagram
participant Scan as "Scanner.import_file()"
participant Model as "Platform.from_extension()"
participant Def as "default_emulator_for()"
participant DB as "Database.upsert_game()"
Scan->>Model : "extension -> Platform"
Model-->>Scan : "Platform"
Scan->>Def : "platform -> EmulatorKind?"
Def-->>Scan : "Some or None"
Scan->>DB : "upsert_game(..., emulator_kind, install_state)"
```

**Diagram sources**
- [scanner.rs:193-265](file://src/scanner.rs#L193-L265)
- [models.rs:353-369](file://src/models.rs#L353-L369)
- [db.rs:625-689](file://src/db.rs#L625-L689)

**Section sources**
- [scanner.rs:193-273](file://src/scanner.rs#L193-L273)
- [db.rs:625-689](file://src/db.rs#L625-L689)

### Emulator Detection, Availability, and Launch Commands
- Emulator detection attempts multiple candidate names and macOS-specific paths for RetroArch.
- Availability checks whether an emulator is installed, downloadable, or unavailable (e.g., RetroArch on Apple Silicon requires Rosetta).
- Launch command building passes ROM path arguments depending on emulator kind.
- RetroArch currently throws an error indicating core selection is not configured.

```mermaid
flowchart TD
A["detect(kind)"] --> B{"PATH/bin or absolute exists?"}
B --> |Yes| Found["Return EmulatorInfo"]
B --> |No| Next["Try next candidate"]
Next --> Found
A --> Avail["availability(kind)"]
Avail --> Inst["Installed"]
Avail --> DL["Downloadable"]
Avail --> UA["Unavailable (Apple Silicon + RetroArch)"]
LA["launch_game(db, game, kind)"] --> Ens["ensure_installed(kind)"]
Ens --> Cmd["build_command(kind, path, rom_path)"]
Cmd --> Run["spawn process"]
```

**Diagram sources**
- [emulator.rs:27-127](file://src/emulator.rs#L27-L127)

**Section sources**
- [emulator.rs:27-127](file://src/emulator.rs#L27-L127)
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)

### Database Persistence of Platform and Emulator Assignments
- Games table stores platform_json, emulator_kind_json, and install_state_json.
- Repair and migration logic resets emulator_kind to the default if it is unsupported or differs from the preferred default.
- Launcher updates the emulator_kind upon successful launch.

```mermaid
erDiagram
GAMES {
string id PK
string title
string filename
string platform_json
string generation_json
string vibe_tags_json
string source_kind_json
string install_state_json
string managed_path
string origin_url
string origin_label
string rom_path
string hash
string emulator_kind_json
string checksum
int size_bytes
int play_count
string last_played_at
string discovered_at
string updated_at
string source_refs_json
string error_message
int progress
}
```

**Diagram sources**
- [db.rs:52-76](file://src/db.rs#L52-L76)

**Section sources**
- [db.rs:242-261](file://src/db.rs#L242-L261)
- [db.rs:748-759](file://src/db.rs#L748-L759)

## Dependency Analysis
- Platform detection depends on file extension parsing.
- Emulator mapping depends on Platform enum variants.
- Scanner depends on Platform and default_emulator_for to set install state and emulator_kind.
- Emulator detection and command building depend on EmulatorKind and platform mapping.
- Database persists platform and emulator_kind, and repair logic ensures consistency.

```mermaid
graph LR
EXT["File extension"] --> PF["Platform.from_extension()"]
PF --> DEF["default_emulator_for()"]
DEF --> MAP["emulators_for_platform()"]
MAP --> DET["detect()"]
DET --> CMD["build_command()"]
PF --> DB["Database.upsert_game()"]
DB --> REP["repair_and_migrate_state()"]
```

**Diagram sources**
- [models.rs:62-76](file://src/models.rs#L62-L76)
- [models.rs:353-369](file://src/models.rs#L353-L369)
- [emulator.rs:45-61](file://src/emulator.rs#L45-L61)
- [emulator.rs:27-127](file://src/emulator.rs#L27-L127)
- [db.rs:625-689](file://src/db.rs#L625-L689)
- [db.rs:242-261](file://src/db.rs#L242-L261)

**Section sources**
- [models.rs:62-76](file://src/models.rs#L62-L76)
- [models.rs:353-369](file://src/models.rs#L353-L369)
- [emulator.rs:45-61](file://src/emulator.rs#L45-L61)
- [emulator.rs:27-127](file://src/emulator.rs#L27-L127)
- [db.rs:625-689](file://src/db.rs#L625-L689)
- [db.rs:242-261](file://src/db.rs#L242-L261)

## Performance Considerations
- Platform detection is O(1) per file via extension mapping.
- Emulator mapping is O(1) via a single match arm.
- Database writes occur in bulk during import and repair; ensure indexing on hash/title for fast lookups.
- RetroArch command construction currently fails; avoid selecting RetroArch until core selection is implemented.

## Troubleshooting Guide
- Unknown platform mapping: When an extension does not match any supported platform, the platform resolves to Unknown, and default_emulator_for returns None. Install state becomes Unsupported. Users should verify ROM file extensions or add support for new platforms.
- RetroArch on Apple Silicon: Availability is marked Unavailable due to Rosetta requirement. The project intentionally avoids auto-installing RetroArch on Apple Silicon to prevent dependency issues.
- Unsupported emulators: If a previously selected emulator is no longer supported for a platform, the repair routine resets it to the default. After repair, the emulator_kind aligns with emulators_for_platform().
- Launch failures: If build_command returns an error (e.g., RetroArch core selection not configured), ensure the emulator is properly installed and configured. For RetroArch, select a core before launching.

**Section sources**
- [models.rs:62-76](file://src/models.rs#L62-L76)
- [models.rs:353-369](file://src/models.rs#L353-L369)
- [emulator.rs:83-100](file://src/emulator.rs#L83-L100)
- [emulator.rs:110-127](file://src/emulator.rs#L110-L127)
- [db.rs:242-261](file://src/db.rs#L242-L261)
- [db.rs:920-972](file://src/db.rs#L920-L972)

## Conclusion
The platform support system centers on robust ROM extension-to-platform detection and a clear mapping to emulators. The design:
- Uses explicit platform variants and extension mapping for reliability
- Provides sensible defaults per platform
- Allows user preferences to override defaults
- Persists platform and emulator assignments with repair logic to maintain consistency
- Handles Unknown platforms gracefully with Unsupported install state
- Documents limitations (e.g., RetroArch on Apple Silicon) and provides fallback strategies