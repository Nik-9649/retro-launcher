# Retro Launcher

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-PolyForm--Noncommercial--1.0.0-lightgrey.svg)](LICENSE)

A terminal-based retro game launcher and library manager with integrated catalog browsing, metadata resolution, and artwork support.

## Features

- **Library Management**: Organize your ROM collection with automatic platform detection
- **TUI Interface**: Beautiful terminal UI with artwork previews and intuitive navigation
- **Metadata Resolution**: Automatic game information fetching from multiple providers
- **Catalog Integration**: Browse and download from curated game catalogs
- **Multi-Platform Support**: Game Boy, NES, SNES, Genesis, PS1, N64, and more
- **Emulator Integration**: Auto-detection and launch support for popular emulators

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Usage](#usage)
- [Configuration](#configuration)
- [Supported Platforms](#supported-platforms)
- [Troubleshooting](#troubleshooting)

## Installation

### Prerequisites

- Rust 1.70+ (for building from source)
- At least one supported emulator installed

### Install from Source

```bash
# Clone the repository
git clone https://github.com/Nik-9649/retro-launcher.git
cd retro-launcher

# Build and install
cargo install --path .
```

### Platform-Specific Instructions

#### macOS

```bash
# Install Rust if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install via cargo
cargo install retro-launcher

# Or build from source
git clone https://github.com/Nik-9649/retro-launcher.git
cd retro-launcher
cargo build --release
sudo cp target/release/retro-launcher /usr/local/bin/
```

#### Linux

```bash
# Ubuntu/Debian - install dependencies
sudo apt-get install libsqlite3-dev

# Install via cargo
cargo install retro-launcher

# Or use the pre-built binary
wget https://github.com/Nik-9649/retro-launcher/releases/download/v0.1.0/retro-launcher-linux-x64
chmod +x retro-launcher-linux-x64
sudo mv retro-launcher-linux-x64 /usr/local/bin/retro-launcher
```

#### Windows

```powershell
# Install via cargo
cargo install retro-launcher

# Or download the pre-built executable from releases
# Add to your PATH environment variable
```

## Quick Start

1. **Launch the TUI**:

   ```bash
   retro-launcher
   ```

2. **Configure ROM directories** (optional):
   Edit `~/.config/dev.retrolauncher.retro-launcher/config.toml`

3. **Scan for ROMs**:
   Press `s` in the TUI or run:

   ```bash
   retro-launcher scan
   ```

## Usage

### Commands

```bash
# Launch interactive TUI (default)
retro-launcher
retro-launcher tui

# List games in library
retro-launcher list
retro-launcher list --platform GBA
retro-launcher list --format json

# Show configuration
retro-launcher config

# Scan ROM directories
retro-launcher scan

# Maintenance operations
retro-launcher maintenance repair
retro-launcher maintenance clear-metadata
retro-launcher maintenance reset-downloads
retro-launcher maintenance reset-all

# Get help
retro-launcher --help
retro-launcher maintenance --help
```

### TUI Navigation

| Key            | Action                                 |
| -------------- | -------------------------------------- |
| `q`            | Quit                                   |
| `1/2/3`        | Switch tabs (Library/Installed/Browse) |
| `↑/↓` or `k/j` | Navigate list                          |
| `Tab`          | Cycle focus                            |
| `/`            | Search                                 |
| `Enter`        | Launch game / Download                 |
| `a`            | Add source                             |
| `x`            | Cancel operation                       |
| `?`            | Show help                              |

## Configuration

Configuration is stored in platform-specific directories:

- **macOS**: `~/Library/Application Support/dev.retrolauncher.retro-launcher/`
- **Linux**: `~/.config/dev.retrolauncher.retro-launcher/` or `~/.local/share/dev.retrolauncher.retro-launcher/`
- **Windows**: `%APPDATA%\dev\retrolauncher\retro-launcher\`

### Config File (`config.toml`)

```toml
rom_roots = [
    "/path/to/your/ROMs",
    "/another/ROM/directory",
]
managed_download_dir = "/path/to/downloads"
scan_on_startup = true
show_hidden_files = false

[[preferred_emulators]]
platform = "GameBoy"
emulator = "Mgba"

[[preferred_emulators]]
platform = "Nes"
emulator = "Fceux"
```

## Supported Platforms

| Platform         | Extensions                                     | Default Emulator |
| ---------------- | ---------------------------------------------- | ---------------- |
| Game Boy         | `.gb`                                          | mGBA             |
| Game Boy Color   | `.gbc`                                         | mGBA             |
| Game Boy Advance | `.gba`                                         | mGBA             |
| NES              | `.nes`                                         | FCEUX            |
| SNES             | `.sfc`, `.smc`                                 | RetroArch        |
| SEGA Genesis     | `.gen`, `.md`, `.smd`                          | RetroArch        |
| Nintendo 64      | `.n64`, `.z64`, `.v64`                         | RetroArch        |
| PlayStation 1    | `.cue`, `.chd`, `.m3u`, `.bin`, `.img`, `.iso` | Mednafen         |
| Nintendo DS      | `.nds`                                         | RetroArch        |

## Troubleshooting

### "No emulator found" error

Install one of the supported emulators:

```bash
# macOS with Homebrew
brew install mgba mednafen fceux retroarch

# Ubuntu/Debian
sudo apt-get install mgba-sdl mednafen fceux retroarch
```

### ROMs not appearing

1. Check your `rom_roots` paths in config.toml
2. Run `retro-launcher scan` to trigger a rescan
3. Verify ROM file extensions are supported

### Database corruption

Run repair command:

```bash
retro-launcher maintenance repair
```

For complete reset:

```bash
retro-launcher maintenance reset-all
```

### Artwork not loading

Clear artwork cache:

```bash
retro-launcher maintenance clear-metadata
```

### Terminal display issues

- Ensure your terminal supports Unicode
- Try increasing terminal size (minimum 80x24 recommended)
- For image preview issues, verify your terminal supports sixel or iTerm2 inline images

## Development

### Building

```bash
cargo build --release
```

### Running Tests

```bash
cargo test
```

## License

PolyForm Noncommercial License 1.0.0 - see [LICENSE](LICENSE) file for details.

This software is available for noncommercial use, including personal use, research, and use by charitable organizations, educational institutions, and government entities. Commercial use requires a separate license.

## Acknowledgments

- Built with [Ratatui](https://github.com/ratatui-org/ratatui) for the TUI
- Uses [EmuLand](http://emu-land.net/) catalog for game metadata
