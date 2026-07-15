# Contributing to Track Exploder

Thanks for your interest! This project is in early development, so issues, ideas, and PRs are all welcome.

## Ground rules

- Be kind — see the [Code of Conduct](CODE_OF_CONDUCT.md).
- **Never commit copyrighted audio** (learning tracks, commercial recordings). Use your own synthetic or licensed test files; `samples/` is git-ignored.

## Getting set up

See the [Development](README.md#development) section of the README for prerequisites and commands.

## Before opening a PR

Please make sure the following pass locally:

```bash
npm run check          # Svelte/TypeScript type-check
npm test               # frontend unit tests
cargo test -p audio-core   # Rust DSP unit tests
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

CI runs the same checks on macOS, Windows, and Linux.

## Project layout

```
src/                     Svelte UI + Web Audio engine
  lib/audio/             preview graph, decode/export bridges, stretch
  lib/mixer/             mixer state store + presets
  lib/components/        UI components
crates/audio-core/       pure-Rust DSP (decode, channel extract, encode) + tests
src-tauri/               thin Tauri app crate exposing commands to the UI
```

## Commit / PR style

- Keep PRs focused. Describe *why*, not just *what*.
- Reference any related issue.
- New audio behavior should come with a test in `crates/audio-core` (Rust) or `src/**/*.test.ts` (mix math) where practical.
