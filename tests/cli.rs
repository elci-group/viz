//! Black-box tests for user-visible Viz CLI behavior.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should follow Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("viz-cli-{label}-{nonce}"));
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

fn viz(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_viz"))
        .args(arguments)
        .output()
        .expect("run viz")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn inspect_json_reports_evidence_without_running_project() {
    let fixture = Scratch::new("inspect");
    fixture.write("Cargo.toml", "[package]\nname = \"fixture\"\n");
    fixture.write(
        "src/main.rs",
        "fn main() { panic!(\"this must not run during inspection\") }\n",
    );

    let result = viz(&["inspect", path(&fixture.0), "--json"]);

    assert!(result.status.success(), "{}", text(&result.stderr));
    let stdout = text(&result.stdout);
    assert!(stdout.contains("\"kind\":\"rust\""));
    assert!(stdout.contains("cargo run --quiet -- --help"));
    assert!(!stdout.contains("this must not run"));
}

#[test]
fn generate_writes_a_tape_and_refuses_accidental_overwrite() {
    let fixture = Scratch::new("generate");
    fixture.write("go.mod", "module example.test/rocket\n\ngo 1.22\n");
    fixture.write("main.go", "package main\nfunc main() {}\n");
    let tape = fixture.0.join("walkthrough.tape");

    let first = viz(&["generate", path(&fixture.0), "--output", path(&tape)]);
    let second = viz(&["generate", path(&fixture.0), "--output", path(&tape)]);

    assert!(first.status.success(), "{}", text(&first.stderr));
    assert_eq!(second.status.code(), Some(1));
    assert!(text(&second.stderr).contains("refusing to overwrite"));
    let source = fs::read_to_string(tape).expect("read generated tape");
    assert!(source.contains("# Evidence: go.mod:1"));
    assert!(source.contains("Type \"go run . --help\""));
}

#[test]
fn stdout_mode_is_deterministic_and_does_not_write() {
    let fixture = Scratch::new("stdout");
    fixture.write(
        "pyproject.toml",
        "[project]\nname = \"nova\"\n[project.scripts]\nnova = \"nova.cli:main\"\n",
    );

    let first = viz(&["generate", path(&fixture.0), "--stdout"]);
    let second = viz(&["generate", path(&fixture.0), "--stdout"]);

    assert!(first.status.success(), "{}", text(&first.stderr));
    assert_eq!(first.stdout, second.stdout);
    assert!(!fixture.0.join("demo.tape").exists());
    assert!(text(&first.stdout).contains("python -m nova.cli --help"));
}

#[test]
fn invalid_generate_option_uses_usage_exit_code() {
    let result = viz(&["generate", "--wat"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(text(&result.stderr).contains("unknown generate option"));
}

#[cfg(unix)]
#[test]
fn render_runs_vhs_from_the_tape_directory() {
    let fixture = Scratch::new("render-cwd");
    let bin = fixture.0.join("bin");
    fs::create_dir_all(&bin).expect("create fake binary directory");
    let fake_vhs = bin.join("vhs");
    fs::write(
        &fake_vhs,
        "#!/bin/sh\npwd > \"$VIZ_CWD_CAPTURE\"\nprintf '%s\\n' \"$*\" >> \"$VIZ_ARGS_CAPTURE\"\n",
    )
    .expect("write fake VHS executable");
    let mut permissions = fs::metadata(&fake_vhs)
        .expect("read fake VHS metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_vhs, permissions).expect("make fake VHS executable");
    let tape = fixture.0.join("demo.tape");
    fixture.write(
        "demo.tape",
        "Output demo.gif\nSet Shell \"bash\"\nType \"echo ok\"\nEnter\n",
    );
    let cwd_capture = fixture.0.join("cwd.txt");
    let args_capture = fixture.0.join("args.txt");
    let existing_path = std::env::var_os("PATH").unwrap_or_default();
    let paths: Vec<PathBuf> = std::iter::once(bin)
        .chain(std::env::split_paths(&existing_path))
        .collect();
    let joined_path = std::env::join_paths(paths).expect("construct PATH");

    let result = Command::new(env!("CARGO_BIN_EXE_viz"))
        .args(["render", path(&tape)])
        .env("PATH", joined_path)
        .env("VIZ_CWD_CAPTURE", &cwd_capture)
        .env("VIZ_ARGS_CAPTURE", &args_capture)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("run viz render");

    assert!(result.status.success(), "{}", text(&result.stderr));
    let observed = fs::read_to_string(cwd_capture).expect("read captured working directory");
    assert_eq!(observed.trim(), fixture.0.to_string_lossy());
    let arguments = fs::read_to_string(args_capture).expect("read captured arguments");
    let calls: Vec<&str> = arguments.lines().collect();
    assert_eq!(calls.len(), 2, "render should validate before execution");
    assert!(calls[0].starts_with("validate "));
    assert_eq!(calls[1], tape.to_string_lossy());
}

fn path(path: &Path) -> &str {
    path.to_str().expect("fixture path should be UTF-8")
}
