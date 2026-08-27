//! Application error types.

use std::fmt::{self, Display, Formatter};
use std::io;
use std::path::PathBuf;

/// Result type used throughout Viz.
pub type Result<T> = std::result::Result<T, Error>;

/// An actionable Viz failure.
#[derive(Debug)]
pub enum Error {
    /// Invalid command-line input.
    Usage(String),
    /// A filesystem operation failed.
    Io {
        /// Operation being attempted.
        operation: &'static str,
        /// Path involved in the failure.
        path: PathBuf,
        /// Original I/O error.
        source: io::Error,
    },
    /// The requested output already exists.
    OutputExists(PathBuf),
    /// Project evidence was insufficient for the requested operation.
    Inspection(String),
    /// An external process failed.
    Process(String),
}

impl Error {
    /// Wraps an I/O error with its operation and path.
    #[must_use]
    pub fn io(operation: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) | Self::Inspection(message) | Self::Process(message) => {
                formatter.write_str(message)
            }
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "could not {operation} {}: {source}",
                path.display()
            ),
            Self::OutputExists(path) => write!(
                formatter,
                "refusing to overwrite {}; pass --force to replace it",
                path.display()
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
