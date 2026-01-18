# Repository Guidelines

## Project Structure & Module Organization
This is a small Rust desktop overlay project.
- `src/main.rs` contains the entry point and core logic.
- `Cargo.toml` defines the package metadata and dependencies.
- `target/` is Cargo build output (generated).

If the codebase grows, prefer adding modules under `src/` (e.g., `src/lib.rs`, `src/foo/mod.rs`) and keep `main.rs` focused on wiring.

## Product & Tech Stack Overview
The goal is a cross-platform, always-on-top transparent overlay that displays the system's current FPS and stays visible above other applications.
- Windowing: `winit`
- Rendering: `wgpu`
- UI: `egui`

## Architecture Overview
Expect a transparent, click-through overlay surface that renders a lightweight UI each frame. Keep FPS sampling separate from rendering, and pass the latest value into the UI layer for display.

## Build, Test, and Development Commands
Use standard Cargo commands from the repo root:
- After each development step, run `cargo fmt`, `cargo clippy`, and `cargo test` to keep the code clean and verified.
- `cargo fmt` formats code using Rustfmt.
- `cargo clippy` runs lints and suggests improvements.
- `cargo run` builds and runs the binary.
- `cargo build` compiles the project without running it.
- `cargo test` runs unit/integration tests (none exist yet).

## Coding Style & Naming Conventions
- Indentation: 4 spaces (Rust default).
- Rust 2024 edition is enabled; follow idiomatic Rust style.
- Use `snake_case` for functions/modules and `CamelCase` for types.
- Keep `main.rs` minimal; move reusable logic into modules.

## Testing Guidelines
There are currently no tests. When adding them:
- Use Rust’s built-in test framework with `#[cfg(test)]` and `#[test]`.
- Name tests descriptively (e.g., `parses_overlay_config`).
- Run with `cargo test`.

## Commit & Pull Request Guidelines
This repository has no commit history yet, so no conventions are established.
- Prefer clear, imperative commit messages (e.g., "Add overlay parser").
- PRs should include a concise description, testing notes, and any relevant output or screenshots for CLI behavior changes.

## Configuration & Tooling Notes
- Dependencies are managed in `Cargo.toml`.
- Generated artifacts live in `target/`; do not commit build output.
