# Contributing

Keep observation, planning, tape composition, and execution as separate layers.
New inference must cite concrete evidence and must not execute project code during
inspection or generation.

Run the release gate before submitting a change:

```sh
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo doc --no-deps
cargo build --release
deliver --spec deliver.toml --strict
```

Tests should use deterministic local fixtures, avoid network access, and verify
negative behavior as well as success. New dependencies require a written reason
and an Amber report.

