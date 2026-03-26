//! Error types for the retro-launcher application.
//!
//! This module provides structured error types that replace the generic `anyhow::Result`
//! pattern, enabling better error context and user-friendly error messages.

use std::io;
use std::path::PathBuf;

/// Structured error types for the game launcher application.
#[derive(Debug, thiserror::Error)]
pub enum LauncherError {
    /// Failed to scan ROM directory.
    #[error("Failed to scan ROM directory: {path}")]
    ScanError {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Download failed for a game.
    #[error("Download failed for {game}: {reason}")]
    DownloadError { game: String, reason: String },

    /// Emulator not found or not available.
    #[error("Emulator not found: {emulator}")]
    EmulatorNotFound { emulator: String },

    /// Metadata resolution failed.
    #[error("Metadata resolution failed: {message}")]
    MetadataError { message: String },

    /// Database operation failed.
    #[error("Database operation '{operation}' failed")]
    DatabaseError {
        operation: String,
        #[source]
        source: rusqlite::Error,
    },

    /// Configuration error.
    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    /// Catalog/URL parsing error.
    #[error("Catalog error: {message}")]
    CatalogError { message: String },

    /// I/O error.
    #[error("I/O error: {message}")]
    IoError {
        message: String,
        #[source]
        source: io::Error,
    },

    /// Generic error with context.
    #[error("{context}: {message}")]
    Context { context: String, message: String },
}

impl LauncherError {
    /// Returns a user-friendly error message suitable for display in the UI.
    pub fn user_message(&self) -> String {
        match self {
            Self::ScanError { path, .. } => {
                format!(
                    "Could not scan '{}'. Check directory permissions.",
                    path.display()
                )
            }
            Self::DownloadError { game, .. } => {
                format!("Could not download '{}'. Check your internet connection.", game)
            }
            Self::EmulatorNotFound { emulator } => {
                format!("'{}' is not installed. Press Enter to install it.", emulator)
            }
            Self::MetadataError { .. } => {
                "Could not fetch game information. Will retry later.".to_string()
            }
            Self::DatabaseError { operation, .. } => {
                format!("Database error during '{}'. Data may be corrupted.", operation)
            }
            Self::ConfigError { .. } => {
                "Configuration error. Check your settings file.".to_string()
            }
            Self::CatalogError { .. } => {
                "Could not load catalog. Check the URL or file path.".to_string()
            }
            Self::IoError { message, .. } => message.clone(),
            Self::Context { context, .. } => context.clone(),
        }
    }

    /// Returns a technical error message suitable for logging.
    pub fn technical_message(&self) -> String {
        self.to_string()
    }
}

/// Convenience type alias for results with LauncherError.
pub type Result<T> = std::result::Result<T, LauncherError>;

/// Extension trait for converting errors to LauncherError.
pub trait IntoLauncherError<T> {
    /// Converts an error into a LauncherError with context.
    fn with_launcher_context(self, context: impl Into<String>) -> Result<T>;
}

impl<T, E: std::error::Error + 'static> IntoLauncherError<T> for std::result::Result<T, E> {
    fn with_launcher_context(self, context: impl Into<String>) -> Result<T> {
        self.map_err(|e| LauncherError::Context {
            context: context.into(),
            message: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_message_for_download_error() {
        let err = LauncherError::DownloadError {
            game: "Super Mario".to_string(),
            reason: "Connection timeout".to_string(),
        };
        let msg = err.user_message();
        assert!(msg.contains("Super Mario"));
        assert!(msg.contains("internet connection"));
    }

    #[test]
    fn user_message_for_emulator_not_found() {
        let err = LauncherError::EmulatorNotFound {
            emulator: "mGBA".to_string(),
        };
        let msg = err.user_message();
        assert!(msg.contains("mGBA"));
        assert!(msg.contains("not installed"));
    }

    #[test]
    fn technical_message_includes_full_details() {
        let err = LauncherError::DownloadError {
            game: "Test Game".to_string(),
            reason: "Network unreachable".to_string(),
        };
        let msg = err.technical_message();
        assert!(msg.contains("Test Game"));
        assert!(msg.contains("Network unreachable"));
    }
}
