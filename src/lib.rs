//! Evidence-driven project inspection and VHS tape generation.
//!
//! Viz deliberately separates observation, planning, and rendering. Static
//! inspection never executes project code. Rendering is an explicit boundary
//! that invokes the external `vhs` executable.

pub mod cli;
pub mod error;
pub mod inspect;
mod manifest;
pub mod model;
pub mod render;
pub mod tape;

pub use error::{Error, Result};
pub use inspect::inspect_project;
pub use model::{CommandCandidate, Evidence, Project, ProjectKind};
pub use tape::{generate_tape, TapeOptions};
