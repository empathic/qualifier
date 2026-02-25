# Repository Guidelines

## Project Structure & Module Organization
- `src/` holds the Rust library plus CLI entry at `src/bin/qualifier.rs`; key modules include `attestation`, `qual_file`, `scoring`, and `graph`.
- `tests/` contains integration coverage (`integration.rs`) and CLI/system tests (`cli_integration.rs`).
- `scripts/` offers helpers: `dev.sh` (serve docs site with pnpm + Eleventy) and `publish.sh` (test, lint, and publish flow).
- `site/` is the marketing/docs site; `README.md` and `SPEC.md` describe concepts and the format.
- Build artifacts land in `target/`; keep it out of commits.

## Build, Test, and Development Commands
- `cargo fmt` — format with rustfmt defaults.
- `cargo clippy --all-targets --all-features -- -D warnings` — lint; keep warning-free.
- `cargo test --all-features` — run library + CLI integration tests.
- `cargo run --bin qualifier -- <args>` — run the CLI locally (e.g., `cargo run --bin qualifier -- score`).
- `./scripts/dev.sh` — serve the Eleventy site locally; installs pnpm deps on first run.
- `./scripts/publish.sh [--execute] [--allow-dirty]` — dry-run publish by default; `--execute` actually publishes after tests/clippy.

## Coding Style & Naming Conventions
- Rust 2024; prefer small, deterministic functions and explicit error handling via `Result` + `thiserror` types.
- Keep modules/files snake_case; types/traits CamelCase; CLI flags and subcommands in kebab-case.
- Respect default `.qual` layout: directory-level `.qual` is preferred unless a 1:1 file already exists or `--file` is set.
- Use borrowed types when possible (`&str`, `&Path`), and keep I/O predictable for testability.

## Testing Guidelines
- Tests live in `tests/`; name cases `test_*` with focused scenarios.
- `cargo test --all-features` is the expected pre-PR run; CLI tests rely on the built `target/debug/qualifier` binary.
- Add regressions to `tests/integration.rs` for library logic and `tests/cli_integration.rs` for end-to-end CLI behavior; use `tempfile`/`std::fs` fixtures.
- Cover edge cases: supersession cycles, score clamping, layout discovery, JSON output structure, and CLI exit codes.

## Commit & Pull Request Guidelines
- Follow conventional commits (e.g., `feat:`, `fix:`, `chore:`) as used in history.
- PRs should describe behavior changes, link issues, and call out impacts to `.qual` layout, scoring, or CLI output.
- Include results for `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test --all-features`; attach CLI examples when changing text output.
- For release work, note whether `./scripts/publish.sh --execute` should be run.

## Slash Command Discovery
- Unrecognized slash commands should be looked up as files under `.claude/commands/` (e.g., `/foo` looks for `.claude/commands/foo.md`).
- If a matching file exists, treat its contents as the command definition; otherwise continue without adding anything to context.
