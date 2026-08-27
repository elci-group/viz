//! Command-line parsing and presentation.

use crate::error::{Error, Result};
use crate::inspect::inspect_project;
use crate::model::Project;
use crate::render::{render_with_vhs, validate_with_vhs};
use crate::tape::{
    describe_candidate, generate_tape, validate_tape_source, write_tape, TapeOptions,
};
use form3::{Mode, Ui};
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Renders a section heading, styled through 3form when connected to an
/// interactive terminal and left as plain text otherwise, so piped, CI, and
/// test output stays byte-identical to the undecorated form.
fn heading_line(text: &str) -> String {
    if Mode::auto() == Mode::Ui {
        Ui::new().heading("▶", text)
    } else {
        text.to_string()
    }
}

/// Renders a completion message; see [`heading_line`] for the styling rule.
fn success_line(text: &str) -> String {
    if Mode::auto() == Mode::Ui {
        Ui::new().success(text)
    } else {
        text.to_string()
    }
}

const HELP: &str = r"Viz — compile project evidence into reproducible terminal demonstrations

Usage:
  viz inspect [PATH] [--json]
  viz generate [PATH] [OPTIONS]
  viz check <TAPE>
  viz render <TAPE> [-o MEDIA]
  viz help

Commands:
  inspect   Explain detected project facts and ranked demo commands
  generate  Write a deterministic VHS tape (aliases: init, new)
  check     Validate tape structure and parse it with VHS
  render    Explicitly execute a tape through the external VHS renderer

Generate options:
  -o, --output <TAPE>     Tape path (default: <project>/demo.tape)
      --media <PATH>      GIF, MP4, or WebM output embedded in the tape
      --command <SHELL>   Record this command instead of an inferred command; repeatable
      --all               Include every inferred command
      --title <TEXT>      Override the displayed project title
      --width <PX>        Terminal width (default: 1200)
      --height <PX>       Terminal height (default: 720)
      --font-size <PX>    Font size from 8 to 72 (default: 18)
      --typing-speed <MS> Delay per typed character (default: 35)
      --stdout            Print the tape instead of writing it
  -f, --force             Replace an existing tape atomically

Safety:
  inspect and generate never execute project code. render is the explicit
  execution boundary. Always review generated commands before rendering.
";

/// Runs Viz with injectable output streams for testability.
///
/// # Errors
///
/// Returns an actionable error for invalid arguments, inspection failures,
/// filesystem failures, or unsuccessful external VHS operations.
pub fn run<I, O, E>(arguments: I, output: &mut O, error_output: &mut E) -> Result<()>
where
    I: IntoIterator<Item = OsString>,
    O: Write,
    E: Write,
{
    let arguments = arguments
        .into_iter()
        .map(|argument| {
            argument
                .into_string()
                .map_err(|_| Error::Usage("arguments must be valid UTF-8 on this platform".into()))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    let Some(command) = arguments.next() else {
        output
            .write_all(HELP.as_bytes())
            .map_err(|source| Error::io("write", "stdout", source))?;
        return Ok(());
    };
    let rest: Vec<String> = arguments.collect();
    if matches!(
        command.as_str(),
        "inspect" | "generate" | "init" | "new" | "check" | "render"
    ) && rest
        .iter()
        .any(|argument| matches!(argument.as_str(), "-h" | "--help"))
    {
        return write_text(output, HELP);
    }
    match command.as_str() {
        "help" | "-h" | "--help" => write_text(output, HELP),
        "version" | "-V" | "--version" => writeln!(output, "viz {}", env!("CARGO_PKG_VERSION"))
            .map_err(|source| Error::io("write", "stdout", source)),
        "inspect" => inspect_command(&rest, output),
        "generate" | "init" | "new" => generate_command(&rest, output),
        "check" => check_command(&rest, output),
        "render" => render_command(&rest, output),
        unknown => {
            let _ignored = writeln!(error_output, "unknown command `{unknown}`\n");
            Err(Error::Usage("run `viz help` for usage".into()))
        }
    }
}

fn inspect_command<W: Write>(arguments: &[String], output: &mut W) -> Result<()> {
    let mut path = PathBuf::from(".");
    let mut path_seen = false;
    let mut json = false;
    for argument in arguments {
        match argument.as_str() {
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(Error::Usage(format!("unknown inspect option `{value}`")));
            }
            value if path_seen => {
                return Err(Error::Usage(format!(
                    "inspect accepts one project path; unexpected `{value}`"
                )));
            }
            value => {
                path = PathBuf::from(value);
                path_seen = true;
            }
        }
    }
    let project = inspect_project(&path)?;
    if json {
        write_text(output, &project_json(&project))
    } else {
        writeln!(output, "{} ({})", project.name, project.kind)
            .and_then(|()| writeln!(output, "root: {}", project.root.display()))
            .and_then(|()| writeln!(output, "\n{}", heading_line("evidence:")))
            .map_err(|source| Error::io("write", "stdout", source))?;
        for evidence in &project.evidence {
            let line = evidence
                .line
                .map_or_else(String::new, |line| format!(":{line}"));
            writeln!(
                output,
                "  - {}{}: {}",
                evidence.source.display(),
                line,
                evidence.fact
            )
            .map_err(|source| Error::io("write", "stdout", source))?;
        }
        writeln!(
            output,
            "\n{}",
            heading_line("ranked demonstration commands:")
        )
        .map_err(|source| Error::io("write", "stdout", source))?;
        for candidate in &project.commands {
            writeln!(
                output,
                "  {}",
                describe_candidate(candidate).replace('\n', "\n  ")
            )
            .map_err(|source| Error::io("write", "stdout", source))?;
        }
        Ok(())
    }
}

fn generate_command<W: Write>(arguments: &[String], output: &mut W) -> Result<()> {
    let mut path = PathBuf::from(".");
    let mut path_seen = false;
    let mut tape_path: Option<PathBuf> = None;
    let mut options = TapeOptions::default();
    let mut force = false;
    let mut stdout = false;
    let mut cursor = 0;
    while cursor < arguments.len() {
        match arguments[cursor].as_str() {
            "-o" | "--output" => {
                tape_path = Some(PathBuf::from(value_after(arguments, &mut cursor)?));
            }
            "--media" => value_after(arguments, &mut cursor)?.clone_into(&mut options.media_output),
            "--command" => options
                .commands
                .push(value_after(arguments, &mut cursor)?.to_owned()),
            "--title" => options.title = Some(value_after(arguments, &mut cursor)?.to_owned()),
            "--width" => {
                options.width = parse_number("width", value_after(arguments, &mut cursor)?)?;
            }
            "--height" => {
                options.height = parse_number("height", value_after(arguments, &mut cursor)?)?;
            }
            "--font-size" => {
                options.font_size =
                    parse_number("font size", value_after(arguments, &mut cursor)?)?;
            }
            "--typing-speed" => {
                options.typing_speed_ms =
                    parse_number("typing speed", value_after(arguments, &mut cursor)?)?;
            }
            "--all" => options.include_all = true,
            "--stdout" => stdout = true,
            "-f" | "--force" => force = true,
            value if value.starts_with('-') => {
                return Err(Error::Usage(format!("unknown generate option `{value}`")));
            }
            value if path_seen => {
                return Err(Error::Usage(format!(
                    "generate accepts one project path; unexpected `{value}`"
                )));
            }
            value => {
                path = PathBuf::from(value);
                path_seen = true;
            }
        }
        cursor += 1;
    }
    if stdout && tape_path.is_some() {
        return Err(Error::Usage(
            "--stdout cannot be combined with --output".into(),
        ));
    }
    if stdout && force {
        return Err(Error::Usage("--force has no effect with --stdout".into()));
    }
    let project = inspect_project(&path)?;
    if stdout {
        let tape = generate_tape(&project, &options)?;
        return write_text(output, &tape);
    }
    let tape_path = tape_path.unwrap_or_else(|| project.root.join("demo.tape"));
    let parent = tape_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(Error::Inspection(format!(
            "output directory does not exist: {}",
            parent.display()
        )));
    }
    let tape_directory =
        fs::canonicalize(parent).map_err(|error| Error::io("resolve", parent, error))?;
    options.working_directory = working_directory_from(&project.root, &tape_directory);
    let tape = generate_tape(&project, &options)?;
    write_tape(&tape_path, &tape, force)?;
    let message = format!(
        "generated {} from {} evidence item(s); review it, then run `viz render {}`",
        tape_path.display(),
        project.evidence.len(),
        tape_path.display()
    );
    writeln!(output, "{}", success_line(&message))
        .map_err(|source| Error::io("write", "stdout", source))
}

fn working_directory_from(project_root: &Path, tape_directory: &Path) -> Option<PathBuf> {
    if project_root == tape_directory {
        return None;
    }
    if let Ok(descendant) = tape_directory.strip_prefix(project_root) {
        let depth = descendant.components().count();
        let mut relative = PathBuf::new();
        for _ in 0..depth {
            relative.push("..");
        }
        return Some(relative);
    }
    Some(project_root.to_path_buf())
}

fn check_command<W: Write>(arguments: &[String], output: &mut W) -> Result<()> {
    if arguments.len() != 1 {
        return Err(Error::Usage("usage: viz check <TAPE>".into()));
    }
    let tape = Path::new(&arguments[0]);
    let source = fs::read_to_string(tape).map_err(|error| Error::io("read", tape, error))?;
    validate_tape_source(&source)?;
    validate_with_vhs(tape)?;
    writeln!(
        output,
        "{}",
        success_line(&format!("valid VHS tape: {}", tape.display()))
    )
    .map_err(|source| Error::io("write", "stdout", source))
}

fn render_command<W: Write>(arguments: &[String], output: &mut W) -> Result<()> {
    let mut tape: Option<PathBuf> = None;
    let mut media: Option<PathBuf> = None;
    let mut cursor = 0;
    while cursor < arguments.len() {
        match arguments[cursor].as_str() {
            "-o" | "--output" => media = Some(PathBuf::from(value_after(arguments, &mut cursor)?)),
            value if value.starts_with('-') => {
                return Err(Error::Usage(format!("unknown render option `{value}`")));
            }
            value if tape.is_some() => {
                return Err(Error::Usage(format!("unexpected argument `{value}`")));
            }
            value => tape = Some(PathBuf::from(value)),
        }
        cursor += 1;
    }
    let tape = tape.ok_or_else(|| Error::Usage("usage: viz render <TAPE> [-o MEDIA]".into()))?;
    let source = fs::read_to_string(&tape).map_err(|error| Error::io("read", &tape, error))?;
    validate_tape_source(&source)?;
    validate_with_vhs(&tape)?;
    render_with_vhs(&tape, media.as_deref())?;
    writeln!(
        output,
        "{}",
        success_line(&format!("rendered {}", tape.display()))
    )
    .map_err(|source| Error::io("write", "stdout", source))
}

fn value_after<'a>(arguments: &'a [String], cursor: &mut usize) -> Result<&'a str> {
    *cursor += 1;
    arguments
        .get(*cursor)
        .map(String::as_str)
        .ok_or_else(|| Error::Usage("option requires a value".into()))
}

fn parse_number<T>(label: &str, value: &str) -> Result<T>
where
    T: std::str::FromStr,
{
    value
        .parse()
        .map_err(|_| Error::Usage(format!("invalid {label}: `{value}`")))
}

fn write_text<W: Write>(output: &mut W, text: &str) -> Result<()> {
    output
        .write_all(text.as_bytes())
        .map_err(|source| Error::io("write", "stdout", source))
}

fn project_json(project: &Project) -> String {
    let evidence = project
        .evidence
        .iter()
        .map(|item| {
            format!(
                "{{\"source\":\"{}\",\"line\":{},\"fact\":\"{}\"}}",
                json_escape(&item.source.to_string_lossy()),
                item.line
                    .map_or_else(|| "null".into(), |line| line.to_string()),
                json_escape(&item.fact)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let commands = project
        .commands
        .iter()
        .map(|item| {
            format!(
                "{{\"command\":\"{}\",\"reason\":\"{}\",\"confidence\":{}}}",
                json_escape(&item.command),
                json_escape(&item.reason),
                item.confidence
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"root\":\"{}\",\"name\":\"{}\",\"kind\":\"{}\",\"evidence\":[{}],\"commands\":[{}]}}\n",
        json_escape(&project.root.to_string_lossy()),
        json_escape(&project.name),
        project.kind,
        evidence,
        commands
    )
}

fn json_escape(input: &str) -> String {
    let mut escaped = String::new();
    for character in input.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            value if value.is_control() => {
                use std::fmt::Write as _;
                let _ignored = write!(escaped, "\\u{:04x}", u32::from(value));
            }
            value => escaped.push(value),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_is_the_default() {
        let mut output = Vec::new();
        run([OsString::from("viz")], &mut output, &mut Vec::new()).expect("show help");
        assert!(String::from_utf8(output)
            .expect("UTF-8")
            .contains("compile project evidence"));
    }

    #[test]
    fn rejects_conflicting_output_modes() {
        let result = run(
            ["viz", "generate", "--stdout", "--output", "x.tape"].map(OsString::from),
            &mut Vec::new(),
            &mut Vec::new(),
        );
        assert!(matches!(result, Err(Error::Usage(_))));
    }

    #[test]
    fn json_escape_handles_control_characters() {
        assert_eq!(json_escape("a\n\"b"), "a\\n\\\"b");
    }

    #[test]
    fn computes_portable_working_directory_for_nested_tapes() {
        assert_eq!(
            working_directory_from(Path::new("/project"), Path::new("/project/docs/assets")),
            Some(PathBuf::from("../.."))
        );
        assert_eq!(
            working_directory_from(Path::new("/project"), Path::new("/project")),
            None
        );
    }

    #[test]
    fn subcommand_help_is_available_without_inspection() {
        let mut output = Vec::new();
        run(
            ["viz", "generate", "--help"].map(OsString::from),
            &mut output,
            &mut Vec::new(),
        )
        .expect("show generate help");
        assert!(String::from_utf8(output)
            .expect("UTF-8")
            .contains("Generate options:"));
    }
}
