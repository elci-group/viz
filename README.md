# Viz

Viz turns observed project structure into a reproducible terminal demonstration.
It inspects a repository, explains the evidence it found, chooses a credible
command, and compiles that plan into a human-readable [VHS](https://github.com/charmbracelet/vhs)
`.tape` file.

```text
project → observed behaviour → .tape → executable demo
```

Viz is not primarily a tape-writing assistant. It is a project-understanding
tool whose output happens to be a presentation artifact. Generation is static
and safe by default: no package scripts, build scripts, test suites, or project
binaries run while Viz inspects or generates.

## Status

This repository contains the `0.1.0-rc.1` release candidate. The core has no
third-party dependencies beyond the first-party `form3` output library, forbids
unsafe Rust, and supports Cargo, Node package manifests, Python `pyproject.toml`,
Go modules, and conservative generic-project fallbacks.

Terminal output (headings, completion messages) is rendered through `form3`,
which negotiates styling from the terminal: colored and iconified on an
interactive TTY, plain text when piped, redirected, or run in CI, so scripted
and machine consumption stay unaffected.

## Install

Build from a checkout with Rust 1.74 or newer. `form3` is not published to a
registry; it is consumed as a sibling checkout at `../3form` relative to this
repository (the same convention other ELCI-group tools use), so it must be
checked out alongside `viz` before building:

```sh
git clone <viz-repo-url> viz
git clone <3form-repo-url> 3form   # sibling checkout, referenced by path
cd viz
cargo install --path .
```

Rendering additionally requires the separate `vhs` executable. Viz can inspect
and generate without it; `viz check` and `viz render` report an actionable error
when it is absent.

## Quick start

First inspect what Viz believes about the project:

```sh
viz inspect .
```

Example output:

```text
viz (rust)
root: /work/viz

evidence:
  - Cargo.toml:2: Cargo package `viz`
  - src/main.rs: runnable binary target

ranked demonstration commands:
  [95%] cargo run --quiet -- --help
         show the binary's own command-line interface
```

Then generate, review, validate, and render:

```sh
viz generate .
sed -n '1,120p' demo.tape
viz check demo.tape
viz render demo.tape
```

A checked-in self-demonstration is available at
[`examples/viz.tape`](examples/viz.tape).

The generated file records the evidence as comments, pins presentation settings,
requires the inferred runtime, clears the terminal without recording setup, and
types the chosen command. Given the same project evidence and options, generation
is byte-for-byte deterministic.

## Commands

### `viz inspect [PATH] [--json]`

Reports the detected ecosystem, metadata evidence, and ranked commands. `--json`
emits one compact JSON object suitable for scripts. Inspection reads only a small,
fixed set of manifests and shallow target/example directories; it does not walk
arbitrary source trees or follow symlinks.

### `viz generate [PATH] [OPTIONS]`

Writes `<project>/demo.tape` by default. Useful options include:

```text
-o, --output <TAPE>     choose the tape path
    --media <PATH>      set demo.gif, demo.mp4, or demo.webm output
    --command <SHELL>   override inference; repeat for a sequence
    --all               include all inferred candidates
    --title <TEXT>      set the displayed title
    --width <PX>        terminal width
    --height <PX>       terminal height
    --font-size <PX>    font size from 8 to 72
    --typing-speed <MS> delay per typed character
    --stdout            print without writing
-f, --force             atomically replace an existing tape
```

`viz init` and `viz new` are aliases for `viz generate`. Existing tapes are never
overwritten unless `--force` is explicit. Parent directories are not created
implicitly. If a tape is written below the project root, Viz emits a hidden
relative `cd`; if it is written elsewhere, it emits a hidden absolute `cd` so the
inferred project command still runs in the directory that supplied its evidence.

Custom commands are recorded verbatim after VHS string escaping. Viz does not run
them during generation. They *will* run if the resulting tape is rendered, so
reviewing the tape is an intentional part of the workflow.

### `viz check <TAPE>`

Performs a fast structural check and then invokes `vhs validate`. Validation parses
the tape but does not execute its commands.

### `viz render <TAPE> [-o MEDIA]`

This is the explicit execution boundary. Viz checks the source first, then invokes
VHS. `-o` overrides output media without changing the tape. Commands execute with
the permissions and working directory of the caller.

## Evidence and command policy

| Project evidence | Primary inferred demonstration |
| --- | --- |
| Cargo binary | `cargo run --quiet -- --help` |
| Cargo examples | `cargo run --quiet --example NAME -- --help` |
| Cargo library only | `cargo test --quiet` |
| Node `demo`, `example`, or `cli` script | matching package-manager script |
| Other Node package | list declared scripts without starting a server |
| Python console script | `python -m MODULE --help` |
| Other Python project | `python -m pytest -q` |
| Go command | `go run … --help` |
| Go library only | `go test ./...` |
| Generic project | explicit Makefile `help` target when present, otherwise `ls` |

Candidates describe runtime behavior but remain proposals. `viz inspect` is the
place to audit them; `--command` is the escape hatch when domain knowledge is
better than inference.

## Design boundaries

- Observation is static. Viz does not infer truth from commands it has not run.
- Generated evidence cites a relative source and line where practical.
- Manifests larger than 1 MiB are rejected to bound memory and surprise.
- Output media must use a VHS-supported extension and a whitespace-free path.
- Commands containing NUL or line breaks are rejected.
- VHS is invoked directly with argument arrays, never through a shell.
- Rendering runs from the tape's directory, so project-local commands and relative
  media paths behave consistently regardless of the caller's current directory.
- Viz has no third-party Rust dependencies and no network behavior.

See [SECURITY.md](SECURITY.md) for the complete trust model and
[CONTRIBUTING.md](CONTRIBUTING.md) for the release gate.

## Non-goals for 0.1

Viz does not synthesize narration, scrape full source trees, mutate project code,
install tools, publish media, or claim that a generated command succeeded. It
does not replace VHS: Viz understands projects and authors tapes; VHS parses and
executes them.

## License

MIT
# viz
