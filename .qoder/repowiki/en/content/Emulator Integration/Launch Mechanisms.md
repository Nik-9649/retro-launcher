# Launch Mechanisms

<cite>
**Referenced Files in This Document**
- [emulator.rs](file://src/emulator.rs)
- [launcher.rs](file://src/launcher.rs)
- [models.rs](file://src/models.rs)
- [db.rs](file://src/db.rs)
- [app/mod.rs](file://src/app/mod.rs)
- [lib.rs](file://src/lib.rs)
- [error.rs](file://src/error.rs)
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
This document explains how the emulator launch pipeline works in the application. It covers how launch commands are constructed per emulator type, how launch candidates are prepared and presented, how emulators are detected and installed automatically, and how the launcher executes processes and records outcomes. It also documents the relationship between emulator detection and launch execution, and provides guidance for diagnosing failures.

## Project Structure
The launch mechanism spans several modules:
- Emulator detection, availability, and command building live in the emulator module.
- The high-level launch orchestration resides in the launcher module.
- Game metadata and platform/emulator mappings are defined in the models module.
- Database updates for launch tracking and emulator assignment occur in the db module.
- The UI integrates launch decisions and invokes the launcher in the app module.
- The library entry points expose the application runtime.

```mermaid
graph TB
subgraph "Application Layer"
APP["app/mod.rs<br/>UI and launch orchestration"]
end
subgraph "Launcher Layer"
LAUNCHER["launcher.rs<br/>launch_game()"]
end
subgraph "Emulation Layer"
EMU["emulator.rs<br/>detect, availability,<br/>ensure_installed, build_command"]
MODELS["models.rs<br/>EmulatorKind, Platform,<br/>GameEntry"]
end
subgraph "Persistence Layer"
DB["db.rs<br/>record_launch, set_game_emulator_kind"]
end
subgraph "Runtime"
LIB["lib.rs<br/>run()/run_cli()"]
end
APP --> LAUNCHER
LAUNCHER --> EMU
LAUNCHER --> DB
EMU --> MODELS
APP --> MODELS
LIB --> APP
```

**Diagram sources**
- [app/mod.rs:434-465](file://src/app/mod.rs#L434-L465)
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)
- [emulator.rs:27-127](file://src/emulator.rs#L27-L127)
- [models.rs:150-173](file://src/models.rs#L150-L173)
- [db.rs:739-759](file://src/db.rs#L739-L759)
- [lib.rs:20-38](file://src/lib.rs#L20-L38)

**Section sources**
- [lib.rs:1-39](file://src/lib.rs#L1-L39)
- [app/mod.rs:434-465](file://src/app/mod.rs#L434-L465)
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)
- [emulator.rs:27-127](file://src/emulator.rs#L27-L127)
- [models.rs:150-173](file://src/models.rs#L150-L173)
- [db.rs:739-759](file://src/db.rs#L739-L759)

## Core Components
- Emulator detection and availability:
  - Detects installed emulators by name or known paths.
  - Determines whether an emulator is installed, downloadable, or unavailable.
- Launch candidate preparation:
  - Builds a list of candidates per game, considering preferences and platform defaults.
- Automatic installation:
  - Installs missing emulators via a package manager when applicable.
- Command construction:
  - Builds the correct command-line arguments for each emulator type.
- Process execution:
  - Spawns the emulator process and validates exit status.
- Post-launch persistence:
  - Records the launch and assigns the emulator kind to the game.

**Section sources**
- [emulator.rs:27-127](file://src/emulator.rs#L27-L127)
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)
- [db.rs:739-759](file://src/db.rs#L739-L759)
- [app/mod.rs:451-465](file://src/app/mod.rs#L451-L465)

## Architecture Overview
The launch flow connects UI choices to emulator detection, installation, command construction, process execution, and database updates.

```mermaid
sequenceDiagram
participant UI as "UI (app/mod.rs)"
participant Launcher as "launch_game (launcher.rs)"
participant Emu as "emulator.rs"
participant DB as "db.rs"
UI->>Launcher : "run_launch_choice(game, emulator_kind)"
Launcher->>Launcher : "resolve rom_path"
Launcher->>Emu : "ensure_installed(kind)"
Emu-->>Launcher : "EmulatorInfo { command }"
Launcher->>Emu : "build_command(kind, command, rom_path)"
Emu-->>Launcher : "Command"
Launcher->>Launcher : "spawn process and wait"
alt "success"
Launcher->>DB : "set_game_emulator_kind(id, kind)"
DB-->>Launcher : "OK"
Launcher->>DB : "record_launch(id)"
DB-->>Launcher : "OK"
Launcher-->>UI : "Ok"
else "failure"
Launcher-->>UI : "Error"
end
```

**Diagram sources**
- [app/mod.rs:402-431](file://src/app/mod.rs#L402-L431)
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)
- [emulator.rs:102-127](file://src/emulator.rs#L102-L127)
- [db.rs:739-759](file://src/db.rs#L739-L759)

## Detailed Component Analysis

### Emulator Detection and Availability
- Detection:
  - Searches for executables by name or known absolute paths.
  - Uses a platform-aware fallback for a specific emulator.
- Availability:
  - Installed if detected.
  - Downloadable if not installed but supported on the host.
  - Unavailable if not supported (e.g., specific OS/arch combinations).
- Unavailable reasons:
  - Provides a user-facing reason string for unsupported configurations.

```mermaid
flowchart TD
Start(["Detect(kind)"]) --> Candidates["Resolve candidate names/paths"]
Candidates --> TryWhich["Try 'which' for each candidate"]
TryWhich --> Found{"Found?"}
Found --> |Yes| ReturnInfo["Return EmulatorInfo { command }"]
Found --> |No| NoneFound["Return None"]
```

**Diagram sources**
- [emulator.rs:27-43](file://src/emulator.rs#L27-L43)
- [emulator.rs:153-168](file://src/emulator.rs#L153-L168)

**Section sources**
- [emulator.rs:27-43](file://src/emulator.rs#L27-L43)
- [emulator.rs:83-100](file://src/emulator.rs#L83-L100)

### Launch Candidate Preparation
- Candidate composition:
  - Combines preferred emulators with platform-defaults.
  - Inserts the last-used emulator first if present.
  - Wraps each into a LaunchCandidate with availability and a note.
- Presentation:
  - The UI uses these candidates to render choices and toasts.

```mermaid
flowchart TD
Start(["launch_candidates_for(game)"]) --> Preferred["Collect preferred emulators for platform"]
Preferred --> Defaults["Collect platform-default emulators"]
Defaults --> Merge["Merge and deduplicate"]
Merge --> LastUsed{"Has last_used?"}
LastUsed --> |Yes| Reorder["Move last_used to front"]
LastUsed --> |No| BuildCandidates["Map to LaunchCandidate"]
Reorder --> BuildCandidates
BuildCandidates --> End(["Vec<LaunchCandidate>"])
```

**Diagram sources**
- [app/mod.rs:451-465](file://src/app/mod.rs#L451-L465)
- [emulator.rs:63-81](file://src/emulator.rs#L63-L81)

**Section sources**
- [app/mod.rs:451-465](file://src/app/mod.rs#L451-L465)
- [emulator.rs:63-81](file://src/emulator.rs#L63-L81)

### Automatic Installation Workflow
- ensure_installed():
  - If already installed, returns immediately.
  - Otherwise, installs via a package manager and re-detects.
- install():
  - Delegates to a brew formula per emulator.
  - RetroArch is intentionally unavailable on specific platforms.
- brew_install():
  - Executes brew install and checks the exit status.

```mermaid
flowchart TD
Start(["ensure_installed(kind)"]) --> Detected{"Already installed?"}
Detected --> |Yes| ReturnInfo["Return EmulatorInfo"]
Detected --> |No| Install["install(kind)"]
Install --> Brew["brew_install(formula)"]
Brew --> Success{"Status success?"}
Success --> |Yes| DetectAgain["detect(kind)"]
DetectAgain --> Found{"Found?"}
Found --> |Yes| ReturnInfo
Found --> |No| Error["Bail with context"]
Success --> |No| Error
```

**Diagram sources**
- [emulator.rs:102-108](file://src/emulator.rs#L102-L108)
- [emulator.rs:129-151](file://src/emulator.rs#L129-L151)

**Section sources**
- [emulator.rs:102-108](file://src/emulator.rs#L102-L108)
- [emulator.rs:129-151](file://src/emulator.rs#L129-L151)

### Command Construction and Argument Formatting
- build_command():
  - Creates a Command with the emulator executable path.
  - Adds arguments per emulator:
    - mGBA: a flag plus the ROM path.
    - Mednafen: ROM path as a positional argument.
    - FCEUX: ROM path as a positional argument.
    - RetroArch: currently not supported (bails with a message).
- Platform-specific notes:
  - mGBA uses a dedicated flag for fullscreen mode in the command.
  - Mednafen and FCEUX accept the ROM path directly.

```mermaid
flowchart TD
Start(["build_command(kind, exe, rom)"]) --> NewCmd["Command::new(exe)"]
NewCmd --> Switch{"match kind"}
Switch --> |Mgba| AddMgbaArgs["Add '-f' and ROM path"]
Switch --> |Mednafen| AddMednafenArg["Add ROM path"]
Switch --> |Fceux| AddFceuxArg["Add ROM path"]
Switch --> |RetroArch| Bail["Bail with 'core selection not configured'"]
AddMgbaArgs --> ReturnCmd["Return Command"]
AddMednafenArg --> ReturnCmd
AddFceuxArg --> ReturnCmd
Bail --> End(["Error"])
ReturnCmd --> End(["Ok"])
```

**Diagram sources**
- [emulator.rs:110-127](file://src/emulator.rs#L110-L127)

**Section sources**
- [emulator.rs:110-127](file://src/emulator.rs#L110-L127)

### Process Execution and Post-Launch Management
- launch_game():
  - Resolves the ROM path from either a direct path or a managed download path.
  - Ensures the emulator is installed and builds the command.
  - Spawns the process and waits for completion.
  - On success, persists the emulator kind and increments the launch count.
- Terminal suspension:
  - The UI temporarily suspends the terminal during launch to allow the emulator to take over.

```mermaid
sequenceDiagram
participant UI as "UI"
participant Launcher as "launch_game()"
participant Proc as "Process"
participant DB as "Database"
UI->>Launcher : "launch_game(db, game, emulator_kind)"
Launcher->>Launcher : "resolve rom_path"
Launcher->>Launcher : "ensure_installed()"
Launcher->>Launcher : "build_command()"
Launcher->>Proc : "spawn and wait"
alt "success"
Launcher->>DB : "set_game_emulator_kind(id, kind)"
DB-->>Launcher : "OK"
Launcher->>DB : "record_launch(id)"
DB-->>Launcher : "OK"
Launcher-->>UI : "Ok"
else "failure"
Launcher-->>UI : "Error"
end
```

**Diagram sources**
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)
- [db.rs:739-759](file://src/db.rs#L739-L759)
- [app/mod.rs:434-449](file://src/app/mod.rs#L434-L449)

**Section sources**
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)
- [db.rs:739-759](file://src/db.rs#L739-L759)
- [app/mod.rs:434-449](file://src/app/mod.rs#L434-L449)

### Relationship Between Emulator Detection and Launch Execution
- Detection precedes launch:
  - ensure_installed() relies on detect() to confirm presence.
- Availability gates UI actions:
  - Unavailable emulators are filtered out of candidate lists.
- Consistency:
  - After installation, detection is re-run to ensure the newly installed binary is discoverable.

```mermaid
graph LR
Detect["detect(kind)"] --> Ensure["ensure_installed(kind)"]
Ensure --> Build["build_command(kind, exe, rom)"]
Build --> Spawn["spawn process"]
Spawn --> Persist["record_launch(), set_game_emulator_kind()"]
```

**Diagram sources**
- [emulator.rs:27-43](file://src/emulator.rs#L27-L43)
- [emulator.rs:102-108](file://src/emulator.rs#L102-L108)
- [emulator.rs:110-127](file://src/emulator.rs#L110-L127)
- [db.rs:739-759](file://src/db.rs#L739-L759)

**Section sources**
- [emulator.rs:27-43](file://src/emulator.rs#L27-L43)
- [emulator.rs:102-108](file://src/emulator.rs#L102-L108)
- [emulator.rs:110-127](file://src/emulator.rs#L110-L127)
- [db.rs:739-759](file://src/db.rs#L739-L759)

## Dependency Analysis
- Module coupling:
  - launcher.rs depends on emulator.rs for detection/installation/command building and on db.rs for persistence.
  - app/mod.rs orchestrates UI and delegates to launcher.rs.
  - models.rs defines enums and types used across modules.
- External dependencies:
  - Process spawning via std::process::Command.
  - Package manager invocation via brew (assumes macOS/Homebrew).
- Potential circular dependencies:
  - None observed among the analyzed modules.

```mermaid
graph TB
APP["app/mod.rs"] --> LAUNCHER["launcher.rs"]
LAUNCHER --> EMU["emulator.rs"]
LAUNCHER --> DB["db.rs"]
EMU --> MODELS["models.rs"]
APP --> MODELS
```

**Diagram sources**
- [app/mod.rs:402-431](file://src/app/mod.rs#L402-L431)
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)
- [emulator.rs:27-127](file://src/emulator.rs#L27-L127)
- [db.rs:739-759](file://src/db.rs#L739-L759)
- [models.rs:150-173](file://src/models.rs#L150-L173)

**Section sources**
- [app/mod.rs:402-431](file://src/app/mod.rs#L402-L431)
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)
- [emulator.rs:27-127](file://src/emulator.rs#L27-L127)
- [db.rs:739-759](file://src/db.rs#L739-L759)
- [models.rs:150-173](file://src/models.rs#L150-L173)

## Performance Considerations
- Process spawning overhead:
  - Launching an emulator is inherently lightweight compared to emulation itself; the cost is dominated by process creation and waiting.
- Command construction:
  - Minimal allocations; arguments are appended directly to the Command builder.
- Database writes:
  - Two small writes per successful launch; negligible overhead.
- Recommendations:
  - Avoid frequent repeated detection calls by caching results at the call site if needed.
  - Keep the UI responsive by deferring heavy operations off the main thread (already handled by the UI’s terminal suspend/resume pattern).

[No sources needed since this section provides general guidance]

## Troubleshooting Guide
Common issues and diagnostics:
- Emulator not found:
  - The UI displays a friendly message indicating the missing emulator; pressing Enter triggers installation when available.
- Installation failure:
  - ensure_installed() will fail if the package manager fails or if the installed binary is not found on PATH afterward.
- Command validation:
  - build_command() bails for unsupported emulators (e.g., RetroArch) until core selection is configured.
- Process execution failure:
  - launch_game() checks the process exit status and surfaces it as an error.
- Database errors:
  - Errors during persistence are surfaced as structured errors with context.

```mermaid
flowchart TD
Start(["Launch Attempt"]) --> ResolveROM["Resolve ROM path"]
ResolveROM --> Ensure["ensure_installed()"]
Ensure --> Build["build_command()"]
Build --> Spawn["spawn process"]
Spawn --> Status{"Exit success?"}
Status --> |Yes| Persist["record_launch(), set_game_emulator_kind()"]
Status --> |No| Fail["Bail with error"]
Persist --> End(["Success"])
Fail --> End
```

**Diagram sources**
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)
- [emulator.rs:102-127](file://src/emulator.rs#L102-L127)
- [db.rs:739-759](file://src/db.rs#L739-L759)

**Section sources**
- [error.rs:61-98](file://src/error.rs#L61-L98)
- [launcher.rs:9-27](file://src/launcher.rs#L9-L27)
- [emulator.rs:102-127](file://src/emulator.rs#L102-L127)
- [db.rs:739-759](file://src/db.rs#L739-L759)

## Conclusion
The launch mechanism is modular and robust:
- Emulator detection and availability gate launch readiness.
- Automatic installation streamlines setup for missing emulators.
- Command construction is explicit and platform-appropriate.
- Process execution is straightforward with clear error propagation.
- Post-launch persistence ensures accurate tracking of emulator usage and play counts.

[No sources needed since this section summarizes without analyzing specific files]