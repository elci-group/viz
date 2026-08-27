//! Static, side-effect-free project inspection.

use crate::error::{Error, Result};
use crate::manifest::{
    json_object_keys, json_string_field, line_containing_in_section, line_containing_json_key,
    read_manifest, toml_pairs_in_section, toml_value_in_section, MAX_MANIFEST_BYTES,
};
use crate::model::{CommandCandidate, Evidence, Project, ProjectKind};
use std::fs;
use std::path::{Path, PathBuf};

/// Inspects a project directory without executing project code.
///
/// # Errors
///
/// Returns an error when the root cannot be resolved, is not a directory, or
/// a recognized manifest cannot be read safely.
pub fn inspect_project(path: &Path) -> Result<Project> {
    let root = fs::canonicalize(path).map_err(|error| Error::io("resolve", path, error))?;
    if !root.is_dir() {
        return Err(Error::Inspection(format!(
            "project path is not a directory: {}",
            root.display()
        )));
    }

    if root.join("Cargo.toml").is_file() {
        inspect_rust(root)
    } else if root.join("package.json").is_file() {
        inspect_node(root)
    } else if root.join("pyproject.toml").is_file() {
        inspect_python(root)
    } else if root.join("go.mod").is_file() {
        inspect_go(root)
    } else {
        Ok(inspect_generic(root))
    }
}

fn inspect_rust(root: PathBuf) -> Result<Project> {
    let manifest_path = root.join("Cargo.toml");
    let manifest = read_manifest(&manifest_path)?;
    let name = toml_value_in_section(&manifest, "package", "name")
        .unwrap_or_else(|| directory_name(&root));
    let name_line = line_containing_in_section(&manifest, "package", "name");
    let mut evidence = vec![Evidence {
        source: PathBuf::from("Cargo.toml"),
        line: name_line,
        fact: format!("Cargo package `{name}`"),
    }];
    let mut commands = Vec::new();

    if root.join("src/main.rs").is_file() || manifest.contains("[[bin]]") {
        evidence.push(Evidence {
            source: if root.join("src/main.rs").is_file() {
                PathBuf::from("src/main.rs")
            } else {
                PathBuf::from("Cargo.toml")
            },
            line: None,
            fact: "runnable binary target".into(),
        });
        commands.push(candidate(
            "cargo run --quiet -- --help",
            "show the binary's own command-line interface",
            95,
        ));
    }

    for example in rust_examples(&root)? {
        evidence.push(Evidence {
            source: PathBuf::from("examples").join(format!("{example}.rs")),
            line: None,
            fact: format!("runnable example `{example}`"),
        });
        commands.push(candidate(
            format!("cargo run --quiet --example {example} -- --help"),
            format!("exercise the `{example}` example"),
            85,
        ));
    }

    if commands.is_empty() {
        commands.push(candidate(
            "cargo test --quiet",
            "exercise the library through its test suite",
            70,
        ));
    }
    note_if_exists(
        &root,
        "README.md",
        "existing project documentation",
        &mut evidence,
    );
    note_if_exists(&root, "tests", "integration test suite", &mut evidence);

    Ok(Project {
        root,
        name,
        kind: ProjectKind::Rust,
        evidence,
        commands,
    })
}

fn inspect_node(root: PathBuf) -> Result<Project> {
    let path = root.join("package.json");
    let manifest = read_manifest(&path)?;
    let name = json_string_field(&manifest, "name").unwrap_or_else(|| directory_name(&root));
    let (manager, manager_source, manager_fact) = if root.join("pnpm-lock.yaml").is_file() {
        ("pnpm", "pnpm-lock.yaml", "lockfile selects `pnpm`")
    } else if root.join("yarn.lock").is_file() {
        ("yarn", "yarn.lock", "lockfile selects `yarn`")
    } else if root.join("bun.lock").is_file() || root.join("bun.lockb").is_file() {
        ("bun", "bun.lock", "lockfile selects `bun`")
    } else {
        (
            "npm",
            "package.json",
            "no recognized lockfile; using the npm convention",
        )
    };
    let mut evidence = vec![Evidence {
        source: PathBuf::from("package.json"),
        line: line_containing_json_key(&manifest, "name"),
        fact: format!("Node package `{name}`"),
    }];
    evidence.push(Evidence {
        source: PathBuf::from(manager_source),
        line: None,
        fact: manager_fact.into(),
    });

    let scripts = json_object_keys(&manifest, "scripts");
    let mut commands = Vec::new();
    for preferred in ["demo", "example", "cli"] {
        if scripts.iter().any(|script| script == preferred) {
            commands.push(candidate(
                run_script_command(manager, preferred),
                format!("run the explicit `{preferred}` package script"),
                95,
            ));
        }
    }
    if !scripts.is_empty() {
        evidence.push(Evidence {
            source: PathBuf::from("package.json"),
            line: line_containing_json_key(&manifest, "scripts"),
            fact: format!(
                "{} package script(s): {}",
                scripts.len(),
                scripts.join(", ")
            ),
        });
    }
    if commands.is_empty() {
        commands.push(candidate(
            list_scripts_command(manager),
            "show the project's declared commands without starting a server",
            75,
        ));
    }
    note_if_exists(
        &root,
        "README.md",
        "existing project documentation",
        &mut evidence,
    );

    Ok(Project {
        root,
        name,
        kind: ProjectKind::Node,
        evidence,
        commands,
    })
}

fn inspect_python(root: PathBuf) -> Result<Project> {
    let path = root.join("pyproject.toml");
    let manifest = read_manifest(&path)?;
    let name = toml_value_in_section(&manifest, "project", "name")
        .unwrap_or_else(|| directory_name(&root));
    let mut evidence = vec![Evidence {
        source: PathBuf::from("pyproject.toml"),
        line: line_containing_in_section(&manifest, "project", "name"),
        fact: format!("Python project `{name}`"),
    }];
    let scripts = toml_pairs_in_section(&manifest, "project.scripts");
    let mut commands = Vec::new();
    for (script, target, line) in scripts {
        let module = target.split(':').next().unwrap_or(&target);
        if !module.is_empty() && module.chars().all(is_python_module_char) {
            evidence.push(Evidence {
                source: PathBuf::from("pyproject.toml"),
                line: Some(line),
                fact: format!("console script `{script}` maps to `{target}`"),
            });
            commands.push(candidate(
                format!("python -m {module} --help"),
                format!("show the `{script}` console interface through its module"),
                90,
            ));
        }
    }
    if commands.is_empty() {
        commands.push(candidate(
            "python -m pytest -q",
            "exercise the project through its conventional test runner",
            55,
        ));
    }
    note_if_exists(
        &root,
        "README.md",
        "existing project documentation",
        &mut evidence,
    );

    Ok(Project {
        root,
        name,
        kind: ProjectKind::Python,
        evidence,
        commands,
    })
}

fn inspect_go(root: PathBuf) -> Result<Project> {
    let path = root.join("go.mod");
    let manifest = read_manifest(&path)?;
    let module = manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("module ").map(str::trim))
        .filter(|value| !value.is_empty())
        .map_or_else(|| directory_name(&root), ToOwned::to_owned);
    let name = module.rsplit('/').next().unwrap_or(&module).to_owned();
    let mut evidence = vec![Evidence {
        source: PathBuf::from("go.mod"),
        line: manifest
            .lines()
            .position(|line| line.trim_start().starts_with("module "))
            .map(|index| index + 1),
        fact: format!("Go module `{module}`"),
    }];
    let mut commands = Vec::new();
    if root.join("main.go").is_file() {
        commands.push(candidate(
            "go run . --help",
            "show the root command's interface",
            90,
        ));
    }
    for command in go_commands(&root)? {
        evidence.push(Evidence {
            source: PathBuf::from("cmd").join(&command),
            line: None,
            fact: format!("Go command `{command}`"),
        });
        commands.push(candidate(
            format!("go run ./cmd/{command} --help"),
            format!("show the `{command}` command's interface"),
            90,
        ));
    }
    if commands.is_empty() {
        commands.push(candidate(
            "go test ./...",
            "exercise all module packages",
            65,
        ));
    }
    note_if_exists(
        &root,
        "README.md",
        "existing project documentation",
        &mut evidence,
    );

    Ok(Project {
        root,
        name,
        kind: ProjectKind::Go,
        evidence,
        commands,
    })
}

fn inspect_generic(root: PathBuf) -> Project {
    let name = directory_name(&root);
    let mut evidence = vec![Evidence {
        source: PathBuf::from("."),
        line: None,
        fact: "no supported package manifest found".into(),
    }];
    note_if_exists(
        &root,
        "README.md",
        "existing project documentation",
        &mut evidence,
    );
    let command = if root.join("Makefile").is_file() {
        evidence.push(Evidence {
            source: PathBuf::from("Makefile"),
            line: None,
            fact: "Make-based project automation".into(),
        });
        if make_has_help_target(&root.join("Makefile")) {
            candidate(
                "make help",
                "ask the project's explicit Makefile help target for its interface",
                65,
            )
        } else {
            candidate(
                "ls",
                "show project contents without guessing which Make target is safe",
                30,
            )
        }
    } else {
        candidate(
            "ls",
            "show the project contents as a conservative fallback",
            25,
        )
    };
    Project {
        root,
        name,
        kind: ProjectKind::Generic,
        evidence,
        commands: vec![command],
    }
}

fn make_has_help_target(path: &Path) -> bool {
    let within_limit =
        fs::metadata(path).is_ok_and(|metadata| metadata.len() <= MAX_MANIFEST_BYTES);
    within_limit
        && fs::read_to_string(path).is_ok_and(|makefile| {
            makefile.lines().any(|line| {
                line.split_once(':')
                    .is_some_and(|(target, _)| target.trim() == "help")
            })
        })
}

fn directory_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("project")
        .to_owned()
}

fn candidate(
    command: impl Into<String>,
    reason: impl Into<String>,
    confidence: u8,
) -> CommandCandidate {
    CommandCandidate {
        command: command.into(),
        reason: reason.into(),
        confidence,
    }
}

fn note_if_exists(root: &Path, relative: &str, fact: &str, evidence: &mut Vec<Evidence>) {
    if root.join(relative).exists() {
        evidence.push(Evidence {
            source: PathBuf::from(relative),
            line: None,
            fact: fact.into(),
        });
    }
}

fn rust_examples(root: &Path) -> Result<Vec<String>> {
    named_rs_files(&root.join("examples"))
}

fn named_rs_files(path: &Path) -> Result<Vec<String>> {
    if !path.is_dir() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(path).map_err(|error| Error::io("list", path, error))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| Error::io("list", path, error))?;
        let entry_path = entry.path();
        if entry_path.extension().and_then(|value| value.to_str()) == Some("rs") {
            if let Some(stem) = entry_path.file_stem().and_then(|value| value.to_str()) {
                names.push(stem.to_owned());
            }
        }
    }
    names.sort();
    Ok(names)
}

fn go_commands(root: &Path) -> Result<Vec<String>> {
    let path = root.join("cmd");
    if !path.is_dir() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(&path).map_err(|error| Error::io("list", &path, error))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| Error::io("list", &path, error))?;
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                names.push(name.to_owned());
            }
        }
    }
    names.sort();
    Ok(names)
}

fn list_scripts_command(manager: &str) -> String {
    match manager {
        "pnpm" => "pnpm run".into(),
        "yarn" => "yarn run".into(),
        "bun" => "bun run".into(),
        _ => "npm run".into(),
    }
}

fn run_script_command(manager: &str, script: &str) -> String {
    match manager {
        "yarn" => format!("yarn {script}"),
        _ => format!("{manager} run {script}"),
    }
}

fn is_python_module_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should follow Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("viz-{label}-{nonce}"));
            fs::create_dir_all(&path).expect("create scratch directory");
            Self(path)
        }

        fn write(&self, relative: &str, content: &str) {
            let path = self.0.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create fixture parent");
            }
            fs::write(path, content).expect("write fixture");
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ignored = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn inspects_rust_binary_and_examples_in_stable_order() {
        let fixture = Scratch::new("rust");
        fixture.write("Cargo.toml", "[package]\nname = \"starship\"\n");
        fixture.write("src/main.rs", "fn main() {}\n");
        fixture.write("examples/zebra.rs", "fn main() {}\n");
        fixture.write("examples/alpha.rs", "fn main() {}\n");

        let project = inspect_project(&fixture.0).expect("inspect Rust fixture");

        assert_eq!(project.name, "starship");
        assert_eq!(project.kind, ProjectKind::Rust);
        assert_eq!(
            project.commands[1].command,
            "cargo run --quiet --example alpha -- --help"
        );
        assert_eq!(
            project.commands[2].command,
            "cargo run --quiet --example zebra -- --help"
        );
    }

    #[test]
    fn node_prefers_explicit_demo_script() {
        let fixture = Scratch::new("node");
        fixture.write(
            "package.json",
            r#"{"name":"orbit","scripts":{"start":"serve","demo":"node demo.js"}}"#,
        );
        fixture.write("pnpm-lock.yaml", "lockfileVersion: '9.0'\n");

        let project = inspect_project(&fixture.0).expect("inspect Node fixture");

        assert_eq!(project.kind, ProjectKind::Node);
        assert_eq!(project.commands[0].command, "pnpm run demo");
    }

    #[test]
    fn parses_python_console_script_module() {
        let fixture = Scratch::new("python");
        fixture.write(
            "pyproject.toml",
            "[project]\nname = \"comet\"\n\n[project.scripts]\ncomet = \"comet.cli:main\"\n",
        );

        let project = inspect_project(&fixture.0).expect("inspect Python fixture");

        assert_eq!(project.name, "comet");
        assert_eq!(project.commands[0].command, "python -m comet.cli --help");
    }
}
