# Artwork Rendering and Display

<cite>
**Referenced Files in This Document**
- [artwork.rs](file://src/artwork.rs)
- [ui.rs](file://src/ui.rs)
- [terminal.rs](file://src/terminal.rs)
- [presentation.rs](file://src/presentation.rs)
- [models.rs](file://src/models.rs)
- [ui/layout.rs](file://src/ui/layout.rs)
- [ui/theme.rs](file://src/ui/theme.rs)
- [app/mod.rs](file://src/app/mod.rs)
- [config.rs](file://src/config.rs)
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
10. [Appendices](#appendices)

## Introduction
This document explains the artwork rendering and display system used in the terminal-based application. It covers how artwork is detected, loaded, scaled, and rendered to the terminal using ratatui-image’s StatefulImage widget and Resize protocol. It documents the rendering pipeline from artwork discovery to terminal display, including error handling and fallback rendering with text alternatives. It also details terminal capability detection for image protocol support, scaling algorithms, aspect ratio preservation, configuration options, styling customization, and performance optimization strategies.

## Project Structure
The artwork rendering system is composed of:
- Artwork discovery and protocol creation in the artwork module
- UI integration and fallback rendering in the UI module
- Terminal capability detection and protocol identification in the terminal module
- Presentation helpers and theme integration in the presentation and UI theme modules
- Application orchestration and artwork synchronization in the app module
- Configuration and paths in the config module

```mermaid
graph TB
subgraph "Application Orchestration"
APP["App (app/mod.rs)"]
end
subgraph "Artwork Layer"
AC["ArtworkController (artwork.rs)"]
RES["Resolve Artwork Paths (artwork.rs)"]
end
subgraph "UI Layer"
UI["UI Renderer (ui.rs)"]
LYT["Layout Utils (ui/layout.rs)"]
THM["Theme (ui/theme.rs)"]
PRES["Presentation (presentation.rs)"]
end
subgraph "Terminal Layer"
TERM["Terminal Capabilities (terminal.rs)"]
end
subgraph "Config"
CFG["AppPaths (config.rs)"]
end
APP --> AC
APP --> UI
UI --> LYT
UI --> THM
UI --> PRES
AC --> TERM
AC --> CFG
AC --> RES
```

**Diagram sources**
- [app/mod.rs:172-177](file://src/app/mod.rs#L172-L177)
- [artwork.rs:35-208](file://src/artwork.rs#L35-L208)
- [ui.rs:294-337](file://src/ui.rs#L294-L337)
- [terminal.rs:86-133](file://src/terminal.rs#L86-L133)
- [config.rs:10-17](file://src/config.rs#L10-L17)

**Section sources**
- [Cargo.toml:6-24](file://Cargo.toml#L6-L24)

## Core Components
- ArtworkController: Manages artwork state, protocol creation, and rendering with ratatui-image fallbacks.
- TerminalCapabilities: Detects terminal image protocol support and color tier.
- UI rendering: Renders artwork panels with fallback text blocks and integrates with theme and layout.
- Presentation helpers: Provide contextual labels and status lines for artwork state.
- App orchestration: Synchronizes artwork selection with the active game or browse item and initializes controllers.

**Section sources**
- [artwork.rs:35-208](file://src/artwork.rs#L35-L208)
- [terminal.rs:86-133](file://src/terminal.rs#L86-L133)
- [ui.rs:294-337](file://src/ui.rs#L294-L337)
- [presentation.rs:161-170](file://src/presentation.rs#L161-L170)
- [app/mod.rs:172-177](file://src/app/mod.rs#L172-L177)

## Architecture Overview
The artwork rendering pipeline integrates terminal capability detection, artwork resolution, protocol creation, and UI rendering with graceful fallbacks.

```mermaid
sequenceDiagram
participant App as "App (app/mod.rs)"
participant Term as "TerminalCapabilities (terminal.rs)"
participant AC as "ArtworkController (artwork.rs)"
participant Picker as "Picker (ratatui-image)"
participant Frame as "Frame (ratatui)"
participant UI as "UI Renderer (ui.rs)"
App->>Term : detect()
Term-->>App : TerminalCapabilities
App->>AC : new(capabilities)
App->>AC : sync_to_game(paths, game, metadata)
alt Image protocol supported
AC->>Picker : from_query_stdio()
AC->>Picker : new_resize_protocol(image)
Picker-->>AC : StatefulProtocol
AC->>UI : render(frame, area, block, fallback_lines, style)
UI->>Frame : render_stateful_widget(StatefulImage, Resize : : Scale(None))
else Unsupported
AC->>UI : render(frame, area, block, fallback_lines, style)
UI->>Frame : render_widget(Paragraph with fallback)
end
```

**Diagram sources**
- [app/mod.rs:172-177](file://src/app/mod.rs#L172-L177)
- [terminal.rs:111-126](file://src/terminal.rs#L111-L126)
- [artwork.rs:52-63](file://src/artwork.rs#L52-L63)
- [artwork.rs:100-112](file://src/artwork.rs#L100-L112)
- [artwork.rs:146-178](file://src/artwork.rs#L146-L178)
- [ui.rs:294-337](file://src/ui.rs#L294-L337)

## Detailed Component Analysis

### ArtworkController
ArtworkController encapsulates artwork discovery, protocol creation, and rendering. It maintains state indicating whether rendering is supported, missing, ready, or failed. It uses ratatui-image’s Picker to create a resize protocol and renders via StatefulImage with Resize::Scale(None) for automatic scaling.

Key behaviors:
- Initialization depends on terminal capabilities; if unsupported, a Picker is not created.
- sync_to_game resolves artwork from cached metadata, companion files, or cache directory.
- sync_to_path allows direct path-based artwork rendering.
- render dispatches to StatefulImage when ready, otherwise falls back to text paragraphs or shows “no art” placeholders.

```mermaid
classDiagram
class ArtworkController {
-picker : Option<Picker>
-selected_key : Option<String>
-selected_art_path : Option<PathBuf>
+state : ArtworkState
+new(capabilities) ArtworkController
+unsupported() ArtworkController
+sync_to_game(paths, game, metadata) void
+sync_to_path(key, path) void
+render(frame, area, block, fallback_lines, style) void
+source_label() &str
+path_label() Option<String>
}
class ArtworkState {
<<enumeration>>
Unsupported
Missing
Ready(source, path, protocol)
Failed(message)
}
class Picker {
+from_query_stdio() Result<Picker>
+new_resize_protocol(image) StatefulProtocol
}
class StatefulImage {
+resize(resize) Self
}
class Resize {
<<enumeration>>
Scale(None)
}
ArtworkController --> ArtworkState : "maintains"
ArtworkController --> Picker : "uses when supported"
ArtworkController --> StatefulImage : "renders with"
StatefulImage --> Resize : "configured with"
```

**Diagram sources**
- [artwork.rs:35-208](file://src/artwork.rs#L35-L208)
- [artwork.rs:12](file://src/artwork.rs#L12)
- [artwork.rs:146-178](file://src/artwork.rs#L146-L178)

**Section sources**
- [artwork.rs:35-208](file://src/artwork.rs#L35-L208)
- [artwork.rs:210-213](file://src/artwork.rs#L210-L213)

### Terminal Capability Detection
TerminalCapabilities detects image protocol support and color tier. The image protocol detection checks environment variables for iTerm.app and Kitty/Ghostty compatibility, defaulting to Unsupported otherwise. Color tier detection respects NO_COLOR and COLORTERM/TERM heuristics.

```mermaid
flowchart TD
Start(["Detect Terminal Capabilities"]) --> EnvTERM["Read TERM and TERM_PROGRAM"]
EnvTERM --> iTerm{"TERM_PROGRAM == iTerm.app?"}
iTerm --> |Yes| SetIterm["Set ImageProtocol = Iterm2"]
iTerm --> |No| KittyCheck["Check KITTY_WINDOW_ID or TERM contains kitty<br/>or TERM_PROGRAM contains ghostty"]
KittyCheck --> |Yes| SetKitty["Set ImageProtocol = Kitty"]
KittyCheck --> |No| SetUnsupported["Set ImageProtocol = Unsupported"]
SetIterm --> ColorTier["Determine ColorTier from NO_COLOR, COLORTERM, TERM"]
SetKitty --> ColorTier
SetUnsupported --> ColorTier
ColorTier --> End(["Return TerminalCapabilities"])
```

**Diagram sources**
- [terminal.rs:111-126](file://src/terminal.rs#L111-L126)
- [terminal.rs:94-109](file://src/terminal.rs#L94-L109)

**Section sources**
- [terminal.rs:86-133](file://src/terminal.rs#L86-L133)

### Rendering Pipeline and Fallbacks
The rendering pipeline follows a deterministic flow:
- UI constructs a panel block and calculates inner area.
- ArtworkController.render decides the rendering path:
  - Ready: renders StatefulImage with Resize::Scale(None).
  - Failed: renders a paragraph with an error message.
  - Unsupported/Missing: renders fallback lines (e.g., “NO ART” placeholders).
- Fallback lines are built in the UI renderer and styled via the current theme.

```mermaid
sequenceDiagram
participant UI as "UI Renderer (ui.rs)"
participant AC as "ArtworkController (artwork.rs)"
participant Frame as "Frame (ratatui)"
participant SI as "StatefulImage (ratatui-image)"
participant Para as "Paragraph (ratatui)"
UI->>AC : render(frame, area, block, fallback_lines, style)
AC->>AC : match state
alt Ready
AC->>SI : default().resize(Scale(None))
AC->>Frame : render_stateful_widget(SI, inner, protocol)
else Failed
AC->>Para : new([error lines]).wrap(trim=true)
AC->>Frame : render_widget(Para, inner)
else Unsupported/Missing
AC->>Para : new(fallback_lines).wrap(trim=true)
AC->>Frame : render_widget(Para, inner)
end
```

**Diagram sources**
- [ui.rs:294-337](file://src/ui.rs#L294-L337)
- [artwork.rs:146-178](file://src/artwork.rs#L146-L178)

**Section sources**
- [ui.rs:294-337](file://src/ui.rs#L294-L337)
- [artwork.rs:146-178](file://src/artwork.rs#L146-L178)

### Artwork Resolution and Scaling
Artwork resolution prioritizes:
- Cached metadata artwork path if present and exists.
- Companion artwork files adjacent to the ROM path (common variants).
- Cache directory artwork named by sanitized game ID.

Scaling uses Resize::Scale(None) which delegates to the terminal image protocol’s scaling algorithm. Aspect ratio is preserved by the protocol; the widget does not alter proportions.

```mermaid
flowchart TD
Start(["Resolve Artwork"]) --> CheckMeta["Check metadata.artwork.cached_path"]
CheckMeta --> MetaExists{"Exists?"}
MetaExists --> |Yes| UseMeta["Use cached artwork"]
MetaExists --> |No| CheckROM["Get ROM path (rom_path or managed_path)"]
CheckROM --> Companions["Generate companion candidates"]
Companions --> Exists{"Any exists?"}
Exists --> |Yes| UseComp["Use companion artwork"]
Exists --> |No| CacheDir["Check cache dir with sanitized ID"]
CacheDir --> CacheExists{"Exists?"}
CacheExists --> |Yes| UseCache["Use cache artwork"]
CacheExists --> |No| Missing["Mark as Missing"]
UseMeta --> Done(["Return path"])
UseComp --> Done
UseCache --> Done
Missing --> Done
```

**Diagram sources**
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)
- [artwork.rs:248-263](file://src/artwork.rs#L248-L263)
- [artwork.rs:265-270](file://src/artwork.rs#L265-L270)

**Section sources**
- [artwork.rs:215-246](file://src/artwork.rs#L215-L246)
- [artwork.rs:248-263](file://src/artwork.rs#L248-L263)
- [artwork.rs:265-270](file://src/artwork.rs#L265-L270)

### UI Integration and Styling
The UI builds a panel block with focus-aware borders and background, computes inner area, and renders either the artwork or fallback text. Styling is derived from the current theme, including focus indicators and color tiers.

```mermaid
classDiagram
class UI {
+render_artwork(frame, area, app, theme) void
+panel_block(title, focused, theme) Block
}
class Theme {
+from_app(app) Theme
+border_style(focused) Style
+pill(label, color, background) Span
+tab_pill(label, active) Span
}
UI --> Theme : "uses"
UI --> ArtworkController : "renders"
```

**Diagram sources**
- [ui.rs:294-337](file://src/ui.rs#L294-L337)
- [ui/layout.rs:46-61](file://src/ui/layout.rs#L46-L61)
- [ui/theme.rs:28-75](file://src/ui/theme.rs#L28-L75)

**Section sources**
- [ui.rs:294-337](file://src/ui.rs#L294-L337)
- [ui/layout.rs:46-61](file://src/ui/layout.rs#L46-L61)
- [ui/theme.rs:28-75](file://src/ui/theme.rs#L28-L75)

### Application Orchestration
The App initializes terminal capabilities and creates two ArtworkControllers: one for the main view and one for previews. It synchronizes artwork whenever selection or tabs change.

```mermaid
sequenceDiagram
participant App as "App (app/mod.rs)"
participant AC as "ArtworkController (artwork.rs)"
App->>App : initialize_terminal_ui()
App->>AC : new(terminal_caps)
App->>AC : new(terminal_caps) [preview]
App->>App : sync_artwork()
alt Browse selected
App->>AC : sync_to_path(browse_key, cached_path)
else Game selected
App->>AC : sync_to_game(paths, game, metadata)
end
```

**Diagram sources**
- [app/mod.rs:172-177](file://src/app/mod.rs#L172-L177)
- [app/mod.rs:331-347](file://src/app/mod.rs#L331-L347)

**Section sources**
- [app/mod.rs:172-177](file://src/app/mod.rs#L172-L177)
- [app/mod.rs:331-347](file://src/app/mod.rs#L331-L347)

## Dependency Analysis
External dependencies relevant to artwork rendering:
- ratatui-image: Provides StatefulImage, Picker, and StatefulProtocol for terminal image rendering.
- image: Decodes images for protocol creation.
- ratatui: Provides widgets, frames, and layout primitives.

```mermaid
graph TB
Cargo["Cargo.toml"]
RATIMG["ratatui-image"]
IMG["image"]
RATATUI["ratatui"]
Cargo --> RATIMG
Cargo --> IMG
Cargo --> RATATUI
RATIMG --> RATATUI
IMG --> RATIMG
```

**Diagram sources**
- [Cargo.toml:6-24](file://Cargo.toml#L6-L24)

**Section sources**
- [Cargo.toml:6-24](file://Cargo.toml#L6-L24)

## Performance Considerations
- Protocol creation cost: Creating a StatefulProtocol involves decoding the image and preparing terminal-specific commands. Minimizing redundant loads by caching paths and avoiding repeated decode operations improves responsiveness.
- Resize algorithm: Using Resize::Scale(None) defers scaling to the terminal’s protocol implementation, which is typically optimized. Avoid manual scaling in application code to reduce CPU overhead.
- Fallback rendering: Text fallbacks are lightweight and suitable for unsupported terminals. They prevent heavy image rendering attempts and keep the UI responsive.
- Drawing cadence: The UI draws at a fixed tick rate; ensure artwork synchronization occurs only when selection or state changes to avoid unnecessary redraws.

[No sources needed since this section provides general guidance]

## Troubleshooting Guide
Common issues and resolutions:
- Unsupported terminal: If the terminal does not support the image protocol, artwork falls back to text placeholders. Verify terminal environment variables and consider switching to iTerm.app or Kitty-compatible terminals.
- Artwork load errors: When protocol creation fails, the controller reports a Failed state with an error message. Check file permissions, image format support, and path correctness.
- Missing artwork: If no artwork is found, the controller marks Missing and displays “NO ART” placeholders. Ensure companion artwork filenames match expected patterns or that cached artwork exists under the data directory.
- Aspect ratio distortion: If images appear stretched, confirm the terminal image protocol preserves aspect ratio. Avoid manual scaling in application code; rely on the protocol’s scaling.
- Terminal compatibility: Some terminals may not fully support ratatui-image. Use terminals known to support the protocols (e.g., iTerm.app, Kitty, Ghostty) for optimal results.

**Section sources**
- [artwork.rs:146-178](file://src/artwork.rs#L146-L178)
- [terminal.rs:111-126](file://src/terminal.rs#L111-L126)

## Conclusion
The artwork rendering system integrates terminal capability detection, robust artwork resolution, and ratatui-image’s StatefulImage with graceful fallbacks. It balances performance with visual fidelity by delegating scaling to the terminal protocol and providing text alternatives for unsupported environments. Proper configuration of paths and terminal settings ensures smooth, responsive artwork display.

[No sources needed since this section summarizes without analyzing specific files]

## Appendices

### Rendering Configuration and Custom Styling
- Terminal protocol selection: Controlled by environment variables; see terminal capability detection for supported protocols.
- Theme integration: UI styling derives from the current theme, including focus indicators and color tiers.
- Panel customization: Panels use panel_block with dynamic focus styling and background colors.

**Section sources**
- [terminal.rs:111-126](file://src/terminal.rs#L111-L126)
- [ui/theme.rs:28-75](file://src/ui/theme.rs#L28-L75)
- [ui/layout.rs:46-61](file://src/ui/layout.rs#L46-L61)

### Example Workflows
- Selecting a game: App.sync_artwork resolves artwork and updates the controller state; UI renders either the image or fallback.
- Previewing browse entries: App.sync_emu_land_search_artwork caches and renders preview artwork with a dedicated controller.

**Section sources**
- [app/mod.rs:331-347](file://src/app/mod.rs#L331-L347)
- [app/mod.rs:314-329](file://src/app/mod.rs#L314-L329)