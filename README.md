<p align="center"><img src="app-icon.svg" alt="Track Exploder" width="96" height="96" /></p>

# Track Exploder

**Turn barbershop "part-predominant" learning tracks into a full mixing desk.**

Many a cappella learning tracks (especially barbershop) are sold as **"part-left" / "part-right"** files: one voice is hard-panned to a single stereo channel while the other three parts are summed on the opposite channel. Track Exploder loads the four files for a song (tenor, lead, baritone, bass), pulls out each isolated voice, and lets you build **any mix you want** — solo a part, drop your own part out to sing along, rebalance levels, re-pan, slow it down for practice **without changing the pitch**, and export.

> Status: early development (v0.x). Desktop first (macOS / Windows / Linux); Android & iOS are on the roadmap via the same codebase.

## How it works

Each source file is stereo, with one part isolated on one channel:

```
tenor.wav   →  L = TENOR      R = lead + bari + bass
lead.wav    →  L = LEAD       R = tenor + bari + bass
bari.wav    →  L = BARITONE   R = tenor + lead + bass
bass.wav    →  L = BASS       R = tenor + lead + bari
```

Extracting a clean part is therefore just **taking the isolated channel** from each file. Track Exploder does this for all four (the isolated channel is selectable **left or right** per file, since some publishers pan the other way), giving four clean **mono stems**. Everything after that is a mixer over those stems.

## Features

- **Load the four part tracks** and auto-extract each voice.
- **Per-track channel selection** (left / right) in case the isolated part is panned the other way.
- **Mixer per part**: include/exclude, gain, pan, solo, mute.
- **Barbershop presets**:
  - *Solo* a single part.
  - *Part-off* — everyone **except** one part (sing the missing voice).
  - *Part-predominant* — one part louder, the rest quieter.
  - *Learning-track layout* — recreate the original "your part on one side" format for any voice.
  - *Full mix*.
- **Preview before export** with transport + waveform.
- **Tempo change without pitch change** (0.5×–1.5×) for slow practice — powered by [Signalsmith Stretch](https://signalsmith-audio.co.uk/code/stretch/) (MIT).
- **Export** to WAV / FLAC (MP3 optional; see licensing note below).
- **Keeps common tags** (album, title, date, genre, …) that all four source files share, writing them into FLAC and MP3 exports. Per-part tags like the voice name drop out automatically. (WAV tag chunks are not yet written.)

## Tech stack

| Layer | Choice |
| --- | --- |
| App shell | [Tauri v2](https://v2.tauri.app/) — one codebase for desktop **and** mobile |
| UI | Svelte 5 + TypeScript + Vite |
| Preview / mixing | Web Audio API |
| Time-stretch | Signalsmith Stretch (MIT) — WASM AudioWorklet |
| Decode / encode | Rust — [Symphonia](https://github.com/pdeljanov/Symphonia) (decode), `hound` / `flacenc` (encode) |

The Rust side is split into a pure-DSP crate (`crates/audio-core`) with no GUI dependencies (unit-testable in isolation) and a thin Tauri app crate (`src-tauri`).

## Development

Prerequisites:

- **Node.js** ≥ 20 and npm
- **Rust** (stable) — install via <https://rustup.rs>
- **Tauri system dependencies** for your OS — see <https://v2.tauri.app/start/prerequisites/>.
  On Debian/Ubuntu: `libwebkit2gtk-4.1-dev build-essential libssl-dev libayatana-appindicator3-dev librsvg2-dev`.
  On openSUSE: `zypper install webkit2gtk3-devel libopenssl-devel gtk3-devel libappindicator3-devel librsvg-devel`.

```bash
npm install

# Run the desktop app in dev mode (hot reload):
npm run tauri dev

# Type-check the frontend:
npm run check

# Unit tests (frontend mix math):
npm test

# Unit tests (Rust DSP core — no webview deps needed):
cargo test -p audio-core

# Production build / installers:
npm run tauri build
```

## Licensing

Track Exploder is **MIT** licensed. The time-stretch library (Signalsmith Stretch) is also MIT.

MP3 export is an **optional** feature using the pure-Rust Shine encoder (LGPL); it is disabled by default to keep the core dependency graph permissive. WAV and FLAC export have no such restriction.

To enable MP3:

```bash
npm run tauri dev   -- --features mp3   # dev
npm run tauri build -- --features mp3   # release
cargo test -p audio-core --features mp3 # tests
```

MP3 uses the **pure-Rust** [Shine](https://github.com/wshon/shine-rs) encoder — no C toolchain, and it cross-compiles to any target (including Android). It's a fixed-point CBR encoder: great for practice tracks, a notch below LAME quality. Shine is **LGPL-2.0**, so it stays behind the optional `mp3` feature to keep the core dependency graph permissive; if you redistribute an MP3-enabled build, comply with the LGPL for that component. WAV and FLAC export have no such restriction.

**Do not** commit copyrighted learning tracks to this repository. The `samples/` directory is git-ignored for your local test audio.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).
