# Viz 0.1.0-rc.1 release evidence

Status: release candidate, verified 2026-08-09.

## Product gate

- Static evidence inspection for Cargo, Node, Python, Go, and generic projects.
- Deterministic VHS generation with source provenance and safe string handling.
- No project code executes during inspection or generation.
- Rendering validates first, then executes VHS from the tape directory.
- Existing tapes require explicit `--force` for atomic replacement.

## Verification gate

- Deliver: 14 of 14 checks passed.
- Rust tests: 24 passed across unit and black-box suites.
- Clippy: all targets with warnings denied.
- Rustdoc: generated without warnings.
- Release build: passed; stripped binary size 487,960 bytes.
- VHS: checked-in example parsed by the installed real parser.
- Amber: zero direct third-party Rust dependencies.
- RustSec: zero vulnerabilities in the offline advisory audit.
- Fract: 92% architectural health, no critical modules.

Release binary SHA-256:

```text
1f44e00135939e9e203c71c21fe07b6d3d5fbe415582011898d0a216f7fd62ba
```

The hash identifies this local optimized build, not a published distribution.
