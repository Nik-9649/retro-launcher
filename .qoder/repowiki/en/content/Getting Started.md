# Getting Started

<cite>
**Referenced Files in This Document**
- [Cargo.toml](file://Cargo.toml)
- [src/main.rs](file://src/main.rs)
- [src/lib.rs](file://src/lib.rs)
- [src/cli.rs](file://src/cli.rs)
- [src/config.rs](file://src/config.rs)
- [src/emulator.rs](file://src/emulator.rs)
- [src/db.rs](file://src/db.rs)
- [src/scanner.rs](file://src/scanner.rs)
- [src/launcher.rs](file://src/launcher.rs)
- [src/models.rs](file://src/models.rs)
- [src/app/mod.rs](file://src/app/mod.rs)
- [src/maintenance.rs](file://src/maintenance.rs)
- [src/ui.rs](file://src/ui.rs)
- [src/terminal.rs](file://src/terminal.rs)
- [support/starter_metadata.json](file://support/starter_metadata.json)
- [README.md](file://README.md)
</cite>

## Table of Contents
1. [Introduction](#introduction)
2. [Prerequisites](#prerequisites)
3. [Installation](#installation)
4. [Initial Setup](#initial-setup)
5. [CLI Commands Overview](#cli-commands-overview)
6. [First-Time Usage Tutorial](#first-time-usage-tutorial)
7. [Navigation and Controls](#navigation-and-controls)
8. [Essential Commands](#essential-commands)
9. [Adding ROMs and Managing Your Library](#adding-roms-and-managing-your-library)
10. [Configuring Emulators](#configuring-emulators)
11. [Launching Games](#launching-games)
12. [Troubleshooting](#troubleshooting)
13. [Conclusion](#conclusion)

## Introduction
Retro Launcher is a terminal-based application that catalogs, organizes, and launches classic games across multiple platforms. It indexes ROMs from local directories, manages downloads, resolves metadata, and integrates with emulators to provide a streamlined gaming experience in the terminal.

## Prerequisites
- Rust toolchain (stable, 1.70+): The project targets edition 2021 and uses modern Rust features. Install via rustup.
- Operating system: Tested on macOS and Linux; Windows is not explicitly supported by the emulator integrations.
- Terminal with UTF-8 support and sufficient dimensions for the UI.
- Optional: Homebrew (on macOS) for automatic emulator installation.

**Section sources**
- [Cargo.toml:1-35](file://Cargo.toml#L1-L35)

## Installation
You can install Retro Launcher in three ways:

- Via cargo install from crates.io
- Via cargo install from git
- From source

### Install from crates.io
```bash
cargo install retro-launcher
```

### Install via cargo from git
Ensure you have the Rust toolchain installed, then run:
```bash
cargo install --git https://github.com/openai/retro-launcher.git
```

After installation, launch the app:
```bash
retro-launcher
```

### Build from source
1. Clone the repository.
2. Change into the project directory.
3. Build and run:
```bash
cargo run
```

Or build the release binary:
```bash
cargo build --release
# Binary will be at target/release/retro-launcher
```

Notes:
- The CLI entry point uses clap for argument parsing and supports multiple subcommands.
- Run `retro-launcher --help` to see all available commands.

**Section sources**
- [src/main.rs:1-12](file://src/main.rs#L1-L12)
- [src/lib.rs:1-45](file://src/lib.rs#L1-L45)
- [src/cli.rs:1-185](file://src/cli.rs#L1-L185)

## Initial Setup
On first run, the application creates configuration and data directories and initializes the database. It also scans configured ROM roots automatically if enabled.

- Configuration location:
  - macOS: `~/Library/Application Support/dev.retrolauncher.retro-launcher/`
  - Linux: `~/.config/dev.retrolauncher.retro-launcher/` or `~/.local/share/dev.retrolauncher.retro-launcher/`
  - Windows: `%APPDATA%\dev\retrolauncher\retro-launcher\`
- Data and downloads live under the data directory.
- The database file is named `library.sqlite3`.

What happens on first run:
- Configuration TOML is created if absent.
- Managed download directory defaults to the data directory's `downloads` subfolder.
- The database schema is initialized and migrated.
- Startup scanning runs against configured ROM roots (see "ROM Directory Setup" below).

**Section sources**
- [src/config.rs:34-64](file://src/config.rs#L34-L64)
- [src/db.rs:48-117](file://src/db.rs#L48-L117)
- [src/app/mod.rs:553-573](file://src/app/mod.rs#L553-L573)

## CLI Commands Overview
Retro Launcher provides a comprehensive CLI with the following commands:

### Global Options
```bash
retro-launcher --help      # Show help information
retro-launcher --version   # Show version information
```

### Available Commands

#### `tui` - Launch Interactive TUI (default)
```bash
retro-launcher           # Default mode - launches TUI
retro-launcher tui       # Explicitly launch TUI
```

#### `list` - List Games in Library
```bash
retro-launcher list                           # List all games in table format
retro-launcher list --platform GBA            # Filter by platform
retro-launcher list --platform NES            # Filter by platform name
retro-launcher list --format json             # Output as JSON
retro-launcher list -p GBA -f json            # Short options
```

Supported platforms for filtering: GB, GBC, GBA, NES, SNES, GEN, N64, PS1, NDS, etc.

#### `config` - Show Configuration
```bash
retro-launcher config    # Display config paths and settings
```
Shows:
- Configuration directory path
- Data directory path
- Database file location
- Downloads directory
- ROM roots
- Preferred emulators

#### `scan` - Scan ROM Directories
```bash
retro-launcher scan      # Scan configured ROM roots for new games
```

#### `maintenance` - Maintenance Operations
```bash
retro-launcher maintenance repair           # Repair database and normalize state
retro-launcher maintenance clear-metadata   # Clear metadata and artwork cache
retro-launcher maintenance reset-downloads  # Reset launcher-managed downloads
retro-launcher maintenance reset-all        # Complete system reset
```

**Section sources**
- [src/cli.rs:9-69](file://src/cli.rs#L9-L69)
- [src/cli.rs:71-112](file://src/cli.rs#L71-L112)
- [src/cli.rs:114-138](file://src/cli.rs#L114-L138)
- [src/cli.rs:140-155](file://src/cli.rs#L140-L155)

## Initial Configuration
Retro Launcher reads and writes configuration in TOML format. The default configuration includes:
- rom_roots: Directories scanned for ROMs (common locations like ~/ROMs are included by default).
- managed_download_dir: Directory for launcher-managed downloads.
- scan_on_startup: Whether to scan ROM roots on startup.
- show_hidden_files: Whether hidden files are included during scanning.
- preferred_emulators: Preferred emulators per platform.

You can edit the configuration file created at the config path noted above. The application will create the file if it does not exist.

**Section sources**
- [src/config.rs:25-32](file://src/config.rs#L25-L32)
- [src/config.rs:66-104](file://src/config.rs#L66-L104)

## First-Time Usage Tutorial
Follow this end-to-end walkthrough to set up your library and launch your first game.

### Step 1: Verify configuration and database
- On first run, the app prints a repair summary indicating normalized URLs and repaired rows.
- The database is initialized with the current schema.

**Section sources**
- [src/app/mod.rs:126-170](file://src/app/mod.rs#L126-L170)
- [src/db.rs:48-117](file://src/db.rs#L48-L117)

### Step 2: Prepare ROMs
- Place ROMs into one of the configured ROM roots (e.g., ~/ROMs).
- Supported extensions include common platforms such as Game Boy, NES, SNES, Genesis/Megadrive, N64, DS, PS1, PS2, Wii, Xbox 360, and others.

Tip: If you have ROMs in archives, the scanner can extract single-ROM archives during import.

**Section sources**
- [src/scanner.rs:15-18](file://src/scanner.rs#L15-L18)
- [src/scanner.rs:158-191](file://src/scanner.rs#L158-L191)

### Step 3: Run the scanner and build the library
You can scan for ROMs either via CLI or wait for automatic scan on startup:

```bash
retro-launcher scan
```

Or launch the TUI which will scan on startup if configured:
```bash
retro-launcher
```

After scanning completes, your library appears with indexed games.

**Section sources**
- [src/app/mod.rs:386-400](file://src/app/mod.rs#L386-L400)
- [src/scanner.rs:158-191](file://src/scanner.rs#L158-L191)
- [src/cli.rs:140-155](file://src/cli.rs#L140-L155)

### Step 4: Resolve metadata and artwork
- The app loads resolved metadata and artwork for each game.
- Titles may be normalized and matched to curated metadata.

**Section sources**
- [src/db.rs:329-421](file://src/db.rs#L329-L421)

### Step 5: Launch a game
- Select a game from the Library tab.
- Press Enter to launch. If an emulator is missing, the app can install it (on macOS via Homebrew) and then launch.

**Section sources**
- [src/app/mod.rs:523-550](file://src/app/mod.rs#L523-L550)
- [src/emulator.rs:102-108](file://src/emulator.rs#L102-L108)

## Navigation and Controls
The terminal UI supports keyboard navigation and overlays. The footer hint updates based on the active mode.

Common controls:
- j/k or arrow keys: Move selection
- Tab / Shift+Tab: Cycle focus zones
- h/l: Move focus left/right
- 1/2/3: Switch tabs (Library, Installed, Browse)
- /: Open search/filter
- a: Add source (URL, Emu-Land search, or manifest)
- Enter: Activate selected action (launch/download/picker)
- ?: Toggle help
- q or Esc: Quit or close overlays

These hints are generated dynamically and reflect the current state of the app.

**Section sources**
- [src/ui.rs:563-575](file://src/ui.rs#L563-L575)
- [src/app/mod.rs:229-258](file://src/app/mod.rs#L229-L258)

## Essential Commands

### CLI Commands
- `retro-launcher --help`: Show all available commands and options
- `retro-launcher --version`: Show version information
- `retro-launcher list`: List all games in the library
- `retro-launcher list --platform GBA`: List games for a specific platform
- `retro-launcher config`: Show configuration paths and current settings
- `retro-launcher scan`: Scan ROM directories for new games

### Maintenance Commands
- `retro-launcher maintenance repair`: Repair and migrate state.
- `retro-launcher maintenance clear-metadata`: Clear metadata and artwork caches.
- `retro-launcher maintenance reset-downloads`: Remove launcher-managed downloads and related DB rows.
- `retro-launcher maintenance reset-all`: Reset database, artwork cache, and managed downloads.

These commands operate on the same data directories and database used by the main application.

**Section sources**
- [src/cli.rs:9-69](file://src/cli.rs#L9-L69)
- [src/maintenance.rs:28-101](file://src/maintenance.rs#L28-L101)

## Adding ROMs and Managing Your Library
- Local ROMs:
  - Place ROMs in one of the configured ROM roots. On startup, the app scans these directories and imports supported files.
  - Hidden files can be included/excluded via configuration.
  - Use `retro-launcher scan` to manually trigger a scan.

- Downloaded ROMs:
  - The app can download and import ROMs from URLs. It validates payloads and extracts single-ROM archives when needed.
  - Downloads are placed in the managed download directory.

- Import manifests:
  - You can import curated lists of ROMs via a manifest file (JSON or TOML). The starter manifest is available in the repository.

- Removing games:
  - Games can be removed from the database; associated metadata is cleaned up.

- Viewing library:
  - Use `retro-launcher list` to see all games from the command line.
  - Use `retro-launcher list --format json` for machine-readable output.

**Section sources**
- [src/config.rs:25-32](file://src/config.rs#L25-L32)
- [src/scanner.rs:158-191](file://src/scanner.rs#L158-L191)
- [src/scanner.rs:52-108](file://src/scanner.rs#L52-L108)
- [src/db.rs:691-699](file://src/db.rs#L691-L699)
- [support/starter_metadata.json](file://support/starter_metadata.json)
- [src/cli.rs:71-112](file://src/cli.rs#L71-L112)

## Configuring Emulators
Retro Launcher detects and launches emulators per platform. Supported emulators include mGBA, Mednafen, FCEUX, and RetroArch.

- Detection:
  - The app checks PATH and common locations for installed emulators.
  - On macOS, RetroArch availability is platform-specific.

- Preferred emulators:
  - Configure preferred emulators per platform in the configuration file. Defaults are set for common platforms.

- Installing emulators:
  - On macOS, the app can install missing emulators via Homebrew when launching.

- Launch command construction:
  - The app builds appropriate command-line arguments for each emulator family.

**Section sources**
- [src/emulator.rs:27-43](file://src/emulator.rs#L27-L43)
- [src/emulator.rs:45-61](file://src/emulator.rs#L45-L61)
- [src/emulator.rs:102-127](file://src/emulator.rs#L102-L127)
- [src/config.rs:81-103](file://src/config.rs#L81-L103)
- [src/models.rs:353-369](file://src/models.rs#L353-L369)

## Launching Games
To launch a game:
1. Select a game in the Library tab.
2. Press Enter.
3. If multiple emulators are available, choose one from the picker.
4. The app suspends the terminal, launches the emulator, and resumes after exit.
5. Play counts and timestamps are recorded in the database.

If an emulator is missing:
- The app attempts to install it (where applicable) and then launches.
- If installation is not supported, you will be prompted to install manually.

**Section sources**
- [src/app/mod.rs:523-550](file://src/app/mod.rs#L523-L550)
- [src/app/mod.rs:402-432](file://src/app/mod.rs#L402-L432)
- [src/launcher.rs:9-27](file://src/launcher.rs#L9-L27)

## Troubleshooting
- Terminal too small:
  - The UI requires a minimum size; if the terminal is too small, a message is shown. Resize your terminal window.

- Emulator not detected:
  - Ensure the emulator executable is installed and on PATH. On macOS, some emulators may require Rosetta depending on architecture.

- HTML content downloaded instead of ROM:
  - The app validates payloads and rejects HTML responses. If a download fails, verify the URL and network connectivity.

- Repair and reset:
  - Use the maintenance subcommand to repair state, clear metadata caches, reset downloads, or reset all data.

- Hidden files:
  - Adjust the configuration to include or exclude hidden files during scanning.

- CLI issues:
  - Run `retro-launcher --help` to verify available commands
  - Check that you're using the correct subcommand syntax
  - Use `retro-launcher config` to verify paths are correctly set

**Section sources**
- [src/ui.rs:28-31](file://src/ui.rs#L28-L31)
- [src/emulator.rs:83-100](file://src/emulator.rs#L83-L100)
- [src/app/mod.rs:623-686](file://src/app/mod.rs#L623-L686)
- [src/maintenance.rs:28-101](file://src/maintenance.rs#L28-L101)
- [src/config.rs:29-31](file://src/config.rs#L29-L31)

## Conclusion
You are now ready to use Retro Launcher. Add ROMs, configure emulators, and enjoy launching classic games directly from the terminal. Use the CLI commands to manage your library from the command line, and explore the TUI for an interactive experience. Use the maintenance commands to keep your library healthy and explore the Browse tab for curated suggestions.
