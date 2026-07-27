# Changelog

All notable changes to Track Exploder are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.5] - 2026-07-27

### Added

- **Debug builds: "Load sample tracks"** in About → Developer — loads a synthetic
  four-part set so the mixer, waveforms, transport, tempo and export can be
  exercised without picking files.

### Changed

- **Export defaults to MP3** when the MP3 encoder is compiled in.
- **Playback time shows the tempo-adjusted length.** With time-stretch on, the
  clock shows how long the mix is actually heard, with the original length in
  parentheses (e.g. `4:00 (3:00)`).

### Fixed

- **Android: opening an exported file now offers a media player** instead of an
  "octet-stream / Save as" dialog — the Open action passes the file's real MIME
  type.
- **Touch: sliders no longer move when you scroll past them.** Starting a
  vertical swipe on a gain/pan/tempo/master/scrub slider now scrolls the page
  instead of changing its value; a horizontal drag still adjusts it and a
  double-tap still resets it.
- **The About dialog shows the real app version** (it was hardcoded to 1.0.0).

## [1.1.4] - 2026-07-26

### Fixed

- **Android: page content no longer sits under the system status and navigation
  bars** — the edge-to-edge viewport now respects the safe-area insets.
- **Android: exported files can be opened** from the Recent exports list; the
  Open action was previously desktop-only.
- **The 1.1.3 Android release build didn't compile its signing config** (a Gradle
  Kotlin-script error), so it produced no APK. It now builds the intended signed
  ~14 MB release APK.

### Changed

- Renamed the preset button to "Save current mix as preset".
- CI runs on Node 22 (Node 20 reached end of life).

## [1.1.3] - 2026-07-26

### Changed

- **The Android APK is now a signed release build** rather than a debug one: it's
  about **10× smaller** (the debug APK carried ~200 MB of unstripped native debug
  symbols), and the release CI now caches the Android NDK and Gradle so builds are
  quicker. Signed with the same key as 1.1.2, so updates between them install in
  place.

## [1.1.2] - 2026-07-26

### Fixed

- **Android updates now install over a previous version.** Each release's APK was
  signed with a fresh, randomly-generated debug key, so Android rejected the
  in-place update (signature mismatch) and forced an uninstall first. Releases are
  now signed with one stable key. (Upgrading from an earlier build still needs a
  single uninstall, because that build carries the old random key; updates after
  that install cleanly.)

## [1.1.1] - 2026-07-26

### Fixed

- **Android exports were pitch-shifted** (and their pitch-preserving tempo change
  came out wrong). The mixdown stamped the exported file with the WebView's
  `AudioContext` rate — the device rate (e.g. 48 kHz) on Android — while the audio
  it contained kept the stems' decoded 44.1 kHz. The export now takes its rate
  from the stems themselves, so it's correct on every platform. Desktop was
  unaffected when its context rate already matched the files, and the preview was
  never affected.

## [1.1.0] - 2026-07-24

### Added

- **Automatic track alignment.** Publishers don't always paste the song into all
  four part files at the same spot, so the parts can play tens (occasionally
  hundreds) of milliseconds apart. Track Exploder now measures the set on load
  and lines it up, absorbing the correction in the silent gap after the pitch
  pipe so the spoken intro stays in sync. Every part strip shows its shift and can
  be nudged by hand; a drift through the song is flagged. Applies to bulk export
  too, and there's a `check_alignment` example for scanning a whole library.
- **Per-part waveform lanes.** The single summed stereo view is replaced by one
  lane per voice: left draws upward and right downward (each shaded differently)
  so panning reads at a glance, gain scales each lane against its own stem, and
  muted parts are greyed out.
- **Playhead returns to the start** when a new set of tracks is loaded.
- **Generated barbershop test fixtures** and audio-core / frontend test suites,
  built from a synthetic circle-of-fifths arrangement written to the Barbershop
  Harmony Society's style guide. No copyrighted audio is stored.

### Fixed

- **MP3 export no longer crashes on full-scale input.** A hot, hard-panned mix
  could drive the Shine encoder's fixed-point math to an overflow; the encoder now
  backs the level off imperceptibly and retries rather than failing.
- **Exported mixes now pan correctly.** WebKitGTK renders `StereoPannerNode` as a
  passthrough when mixing offline, so exports came out centred regardless of pan;
  the mixdown is now done directly and matches the preview on every platform.

## [1.0.0] - 2026-07-18

### Added

- Initial release. Load the four part-predominant tracks for a song, auto-extract
  each isolated voice, and build any mix: include/exclude, gain, pan, solo, mute,
  per-track left/right channel selection.
- Preview with transport and waveform.
- Pitch-preserving tempo change (0.5×–1.5×) for slow practice, via Signalsmith
  Stretch.
- Export to WAV / FLAC (MP3 optional), carrying the tags shared across the source
  files, plus save-able export presets and bulk export.
- Desktop and Android builds (Tauri v2).

[1.1.4]: https://github.com/Safrone/track-exploder/releases/tag/v1.1.4
[1.1.3]: https://github.com/Safrone/track-exploder/releases/tag/v1.1.3
[1.1.2]: https://github.com/Safrone/track-exploder/releases/tag/v1.1.2
[1.1.1]: https://github.com/Safrone/track-exploder/releases/tag/v1.1.1
[1.1.0]: https://github.com/Safrone/track-exploder/releases/tag/v1.1.0
[1.0.0]: https://github.com/Safrone/track-exploder/releases/tag/v1.0.0
