//! Domain model shared by inspectors and renderers.

use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

/// A supported project ecosystem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectKind {
    /// A Cargo-based Rust project.
    Rust,
    /// A package.json-based JavaScript or TypeScript project.
    Node,
    /// A Python project with `pyproject.toml`.
    Python,
    /// A Go module.
    Go,
    /// A project without a recognized manifest.
    Generic,
}

impl Display for ProjectKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Rust => "rust",
            Self::Node => "node",
            Self::Python => "python",
            Self::Go => "go",
            Self::Generic => "generic",
        })
    }
}

/// A fact used to build a demonstration plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Evidence {
    /// File containing the fact, relative to the project root.
    pub source: PathBuf,
    /// One-based source line, when available.
    pub line: Option<usize>,
    /// Human-readable observation.
    pub fact: String,
}

/// A command inferred from project evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandCandidate {
    /// Shell command suitable for a VHS `Type` directive.
    pub command: String,
    /// Why this command represents the project.
    pub reason: String,
    /// Relative confidence from 0 through 100.
    pub confidence: u8,
}

/// The result of inspecting a project without executing it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Project {
    /// Canonical project root.
    pub root: PathBuf,
    /// Display name inferred from metadata or the directory name.
    pub name: String,
    /// Detected project ecosystem.
    pub kind: ProjectKind,
    /// Facts supporting the classification and plan.
    pub evidence: Vec<Evidence>,
    /// Ranked demonstration command candidates.
    pub commands: Vec<CommandCandidate>,
}
