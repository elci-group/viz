//! Explicit integration boundary with the external VHS renderer.

use crate::error::{Error, Result};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

/// Validates a tape with the installed VHS parser.
///
/// # Errors
///
/// Returns an error when VHS is unavailable or rejects the tape.
pub fn validate_with_vhs(tape: &Path) -> Result<()> {
    let tape = fs::canonicalize(tape).map_err(|error| Error::io("resolve", tape, error))?;
    let output = invoke_vhs(["validate", &tape.to_string_lossy()], None)?;
    process_result("VHS validation", &output)
}

/// Renders a tape with VHS, optionally overriding its media output.
///
/// # Errors
///
/// Returns an error when VHS cannot start or rendering fails.
pub fn render_with_vhs(tape: &Path, output: Option<&Path>) -> Result<()> {
    let tape = fs::canonicalize(tape).map_err(|error| Error::io("resolve", tape, error))?;
    let working_directory = tape
        .parent()
        .ok_or_else(|| Error::Process("tape has no parent directory".into()))?;
    let tape_string = tape.to_string_lossy();
    let result = if let Some(output) = output {
        let output = if output.is_absolute() {
            output.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|error| Error::io("resolve", ".", error))?
                .join(output)
        };
        let output_string = output.to_string_lossy();
        invoke_vhs(
            [tape_string.as_ref(), "--output", output_string.as_ref()],
            Some(working_directory),
        )?
    } else {
        invoke_vhs([tape_string.as_ref()], Some(working_directory))?
    };
    process_result("VHS rendering", &result)
}

fn invoke_vhs<const N: usize>(arguments: [&str; N], directory: Option<&Path>) -> Result<Output> {
    let mut command = Command::new("vhs");
    command.args(arguments);
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    command.output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::Process(
                "VHS is not installed; install it from https://github.com/charmbracelet/vhs".into(),
            )
        } else {
            Error::Process(format!("could not start VHS: {error}"))
        }
    })
}

fn process_result(operation: &str, output: &Output) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let detail = if stderr.is_empty() { stdout } else { stderr };
    Err(Error::Process(if detail.is_empty() {
        format!("{operation} failed with {}", output.status)
    } else {
        format!("{operation} failed: {detail}")
    }))
}
