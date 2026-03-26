mod app;
mod artwork;
mod catalog;
mod config;
mod db;
mod emulator;
mod error;
mod launcher;
mod maintenance;
mod metadata;
mod models;
mod presentation;
mod scanner;
mod terminal;
mod toast;
mod ui;

pub mod cli;

pub use error::{LauncherError, Result};

use clap::Parser;

pub fn run() -> anyhow::Result<()> {
    app::run()
}

pub fn run_cli(args: Vec<String>) -> anyhow::Result<()> {
    // Parse arguments - if no subcommand provided, default to TUI mode
    let cli = if args.len() <= 1 {
        return app::run();
    } else {
        cli::Cli::parse_from(&args)
    };

    match cli.command {
        Some(cli::Commands::Tui) | None => app::run(),
        Some(cli::Commands::Maintenance { action }) => {
            let message = maintenance::run(action.into())?;
            println!("{message}");
            Ok(())
        }
        Some(cli::Commands::List(args)) => cli::list_games(args),
        Some(cli::Commands::Config) => cli::show_config(),
        Some(cli::Commands::Scan) => cli::run_scan(),
    }
}
