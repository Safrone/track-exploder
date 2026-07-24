# samples/

Put your local test audio here. **Audio files in this directory are git-ignored** so
you don't accidentally commit copyrighted learning tracks.

## Generated test fixtures

`samples/fixtures/` holds a *synthetic* part-predominant learning-track set that the
test suites decode — nothing copyrighted is ever needed or stored. It is written by

```bash
cargo run -p audio-core --example generate_fixtures        # add --features mp3 for MP3s
```

and is regenerated automatically by `cargo test -p audio-core` (and by `npm test`,
which shells out to the same generator) whenever it's missing or out of date.

The song is a barbershop arrangement in B♭ written to the style rules in the
Barbershop Harmony Society's [Music Educator Guide and
Songbook](https://files.barbershop.org/PDFs/Education/Music-Educator-Guide-and-Songbook_v3.5.pdf)
("Characteristics of the Barbershop Style"):

- **Barbershop sevenths around the circle of fifths** — `Bb → Bb6 → G7 → C7 → F7 →
  Bb`, and a tag that runs `D7 → G7 → C7 → F7 → Bb` onto a held post. Every seventh
  resolves down a perfect fifth, and no chord contains a minor second.
- **Just intonation**, so the chords lock and ring: chord tones are tuned as ratios
  above their root (a dominant seventh is a 4:5:6:7 chord, its seventh 31 cents
  under the piano's), with roots tuned from the key's own lattice.
- **Melody in the lead**, tenor harmonizing above it, then baritone and bass, all
  within the ranges the guide gives, moving in **homorhythm** — same syllable, same
  time.
- **Balance** the guide's way: roots and fifths carry, thirds and sevenths sit back,
  bass heaviest and tenor lightest.
- **Embellishments**: swipes (the chord changes inside a held word), a word echo,
  and trio bars.
- **Rests** a quartet would actually sing: the breath before the pickup, the breath
  between phrases, voices dropping out of the echo and the trios, and the harmony
  sustaining under the melody's breath.

See `crates/audio-core/tests/support/score.rs` for the score itself.

```text
part-left/   four 24-bit stereo WAVs, isolated part on the LEFT channel
part-right/  the same song with the isolated part on the RIGHT (16-bit)
flac/        the part-left set as 16-bit FLAC, with vendor-style tags
mp3/         the part-left set as MP3 (only with --features mp3)
reference/   each isolated voice as a mono FLAC — the ground truth
misaligned/  the part-left set with a publisher-style lead-in (spoken title,
             pitch pipe, gap) and the song pasted in late in three of the four
             files — what the alignment detection is tested against
fixtures.json  manifest: filenames, tags, event/rest frame spans
```

The files are named the way publishers name learning tracks
(`01 Circle of Fifths [Bb] - TENOR [Track Exploder] [20260101].wav`), so you can also
load `part-left/` straight into the app to exercise the UI by hand.
