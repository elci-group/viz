# Changelog

All notable changes are documented here. Viz follows semantic versioning.

## Unreleased

- Rendered `inspect` headings and `generate`/`check`/`render` completion messages
  through the first-party `form3` output library, styled on an interactive
  terminal and left as plain text otherwise so piped, CI, and machine output is
  unaffected. `form3` is consumed as a sibling checkout (`../3form`).

## 0.1.0-rc.1 — 2026-08-09

- Added evidence-driven inspection for Rust, Node, Python, Go, and generic projects.
- Added deterministic VHS generation with provenance comments and presentation controls.
- Added JSON inspection output, overwrite protection, structural checks, VHS validation,
  and an explicit rendering boundary.
- Added a dependency-free Rust core, strict linting, security documentation, and
  unit plus black-box CLI coverage.

