use clap::{Args, Parser, Subcommand};

use crate::config::Config;
use crate::db::Database;

#[derive(Parser)]
#[command(name = "retro-launcher")]
#[command(about = "A TUI-based retro game launcher and library manager")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(author = env!("CARGO_PKG_AUTHORS"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Launch the interactive TUI (default)
    #[command(name = "tui")]
    Tui,

    /// Maintenance operations for database and cache
    #[command(name = "maintenance")]
    Maintenance {
        #[command(subcommand)]
        action: MaintenanceAction,
    },

    /// List games in the library
    #[command(name = "list")]
    List(ListArgs),

    /// Show configuration paths and settings
    #[command(name = "config")]
    Config,

    /// Scan ROM directories for new games
    #[command(name = "scan")]
    Scan,
}

#[derive(Subcommand, Clone)]
pub enum MaintenanceAction {
    /// Repair database and normalize state
    #[command(name = "repair")]
    Repair,
    /// Clear metadata and artwork cache
    #[command(name = "clear-metadata")]
    ClearMetadata,
    /// Reset launcher-managed downloads
    #[command(name = "reset-downloads")]
    ResetDownloads,
    /// Complete system reset
    #[command(name = "reset-all")]
    ResetAll,
}

#[derive(Args, Clone)]
pub struct ListArgs {
    /// Filter by platform (e.g., GBA, NES, PS1)
    #[arg(short, long)]
    pub platform: Option<String>,
    /// Output format (table, json)
    #[arg(short, long, default_value = "table")]
    pub format: String,
}

/// List games in the library
pub fn list_games(args: ListArgs) -> anyhow::Result<()> {
    let (_config, paths) = Config::load_or_create()?;
    let db = Database::new(&paths.db_path)?;
    let games = db.all_games()?;

    // Filter by platform if specified
    let games: Vec<_> = match args.platform {
        Some(p) => games
            .into_iter()
            .filter(|g| {
                g.platform.short_label().eq_ignore_ascii_case(&p)
                    || g.platform.display_name().eq_ignore_ascii_case(&p)
            })
            .collect(),
        None => games,
    };

    match args.format.as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(&games)?),
        _ => {
            // Table format
            println!("{:<8} {:<35} {:<10} {:<15}", "ID", "Title", "Platform", "Status");
            println!("{}", "-".repeat(72));
            for game in games {
                let id_short = if game.id.len() > 6 {
                    &game.id[..6]
                } else {
                    &game.id
                };
                println!(
                    "{:<8} {:<35} {:<10} {:<15}",
                    id_short,
                    truncate(&game.title, 33),
                    game.platform.short_label(),
                    game.install_state.badge()
                );
            }
        }
    }
    Ok(())
}

/// Show configuration paths and settings
pub fn show_config() -> anyhow::Result<()> {
    let (config, paths) = Config::load_or_create()?;

    println!("Configuration Paths:");
    println!("  Config directory: {}", paths.config_dir.display());
    println!("  Data directory:   {}", paths.data_dir.display());
    println!("  Database:         {}", paths.db_path.display());
    println!("  Downloads:        {}", paths.downloads_dir.display());
    println!();
    println!("Settings:");
    println!("  Scan on startup:  {}", config.scan_on_startup);
    println!("  Show hidden:      {}", config.show_hidden_files);
    println!("  ROM roots:");
    for root in &config.rom_roots {
        println!("    - {}", root.display());
    }
    println!();
    println!("Preferred emulators:");
    for pref in &config.preferred_emulators {
        println!("  {} -> {}", pref.platform.display_name(), pref.emulator);
    }

    Ok(())
}

/// Scan ROM directories for new games
pub fn run_scan() -> anyhow::Result<()> {
    use crate::scanner::scan_rom_roots;

    println!("Scanning ROM directories...");

    let (config, paths) = Config::load_or_create()?;
    let db = Database::new(&paths.db_path)?;

    let discoveries = scan_rom_roots(&db, &config.rom_roots, config.show_hidden_files)?;
    let count = discoveries.len();

    if count == 0 {
        println!("No new ROM files found.");
        return Ok(());
    }

    println!("Scan complete. {} game(s) discovered.", count);
    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
