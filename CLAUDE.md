# CLAUDE.md - moq-rs Project Guide

## Build/Test/Lint Commands
- Build: `cargo build` or `just build` (for NPM package)
- Check: `just check` (runs cargo check, clippy, fmt, and npm check)
- Test: `cargo test` or `just test`
- Test single file: `cargo test --test <test_name> -- <test_function>`
- Run relay: `just relay`
- Fix issues: `just fix` (runs cargo fix, clippy --fix, fmt, npm fix)
- Web dev: `npm run dev` or `just web`

## Code Style
- **Rust**: Use `rustfmt` (enforced in CI)
  - Error handling: Use `anyhow::Result` for application errors
  - Use `?` operator for error propagation
  - Prefer trait impls over free functions where appropriate
- **TypeScript/JavaScript**:
  - Formatting: Use Biome (indentation: tabs, line width: 120)
  - Quotes: Double quotes
  - Imports: Use the `organizeImports` feature of Biome
- Use meaningful type and variable names
- Structure: Module-per-file, re-export via mod.rs or lib.rs

## Project Structure
This workspace contains multiple crates implementing the MoQ protocol stack:
- `moq-proto`: Runtime-agnostic protocol components
- `moq-transfork`: MoQ Transfork implementation
- `moq-relay`: Relay server implementation
- `moq-web`: Web-based UI components