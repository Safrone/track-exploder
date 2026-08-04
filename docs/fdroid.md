# Publishing on F-Droid

F-Droid builds from source on its own infrastructure and signs with its own key.
Nothing here is triggered from this repo — the build recipe lives in F-Droid's
[fdroiddata] repository as `metadata/com.safrone.trackexploder.yml`, and a
release reaches users only once that recipe points at a tag.

[fdroiddata]: https://gitlab.com/fdroid/fdroiddata

## What a release needs from this repo

1. **A tag.** The recipe's `commit:` is a tag name (`v1.1.10`), not a branch.
2. **`bundle.android.versionCode` bumped** in `src-tauri/tauri.conf.json` — see
   below.
3. **A changelog entry** at
   `fastlane/metadata/android/en-US/changelogs/<versionCode>.txt`, e.g.
   `.../changelogs/1001012.txt`. F-Droid shows this on the app page.

The store listing (title, descriptions, icon, screenshots) is read from
`fastlane/metadata/android/en-US/` in this repo, not from the recipe. F-Droid
also scans a bare `metadata/<locale>/` at the repo root, but the fastlane path
is what its maintainers expect — keep the listing here and out of the recipe,
which should carry nothing but build metadata.

## Why versionCode is pinned rather than derived

Tauri normally computes the Android `versionCode` from the app version
(`major * 1000000 + minor * 1000 + patch`) and writes it into the generated
`src-tauri/gen/android/app/tauri.properties`. That tree is git-ignored, so the
number exists nowhere a tool can read without running a full Android build.

F-Droid's `checkupdates` is what notices new releases. Given
`UpdateCheckMode: Tags` it checks out each tag and looks for a versionCode in
the source — by default in `AndroidManifest.xml` or `build.gradle`, which Tauri
also generates into `src-tauri/gen/`, so the default search finds nothing and
fails with *"Couldn't find any version information"*. Pointing it at another
file instead requires `UpdateCheckData`, and the value it captures must parse as
an integer (`int(vercode, 0)`) — a version string like `1.1.10` will not do.

So `bundle.android.versionCode` is set explicitly, giving `checkupdates` a
literal integer in a tracked file:

```yaml
UpdateCheckMode: Tags ^v[\d.]+$
UpdateCheckData: src-tauri/tauri.conf.json|"versionCode":\s*(\d+)|src-tauri/tauri.conf.json|"version":\s*"([\d.]+)"
```

The cost is that the versionCode no longer follows the app version on its own.
Keep it equal to Tauri's formula so Play's accepted-versionCode history stays
consistent; `src/lib/version.test.ts` fails if the two drift.

## The sideload signing key

`release.yml` signs the GitHub release APK with a dedicated keystore, held in
three repository secrets: `ANDROID_SIDELOAD_KEYSTORE_BASE64`,
`ANDROID_SIDELOAD_KEY_ALIAS` and `ANDROID_SIDELOAD_STORE_PASSWORD`. It is a
different key from the Play upload keystore, and both differ from F-Droid's.

**This key can never be rotated.** Android identifies an app by its signature, so
a new key means every sideload and F-Droid user has to uninstall and lose their
data. Under reproducible builds it is also the key F-Droid republishes under, so
it is a permanent public identity. Back the keystore and its password up
wherever the Play upload keystore lives; `.gitignore` covers `*.keystore` and
`*.jks`, and neither is in the repo.

It replaced the Android debug keystore in 1.1.13. The debug key worked — it is
generated per machine, not a shared secret — but the SDK treats that file as
disposable and regenerates it silently, which is a poor foundation for a key
that has to outlive the project.

## Reproducible builds

F-Droid can verify that its from-source build matches the APK attached to the
GitHub release, and then distribute *our* signed binary instead of one signed
with F-Droid's key. That is what `Binaries:` in the recipe requests, and it is
what lets a user move between the GitHub APK and F-Droid without uninstalling.

It only works if the two builds agree byte for byte, which is why so much is
pinned. Anything that differs between our runner and F-Droid's buildserver has
to be nailed down in both places:

| Pinned | Where |
| --- | --- |
| Rust 1.97.0 | `rust-toolchain.toml`, and `rustup default` in the recipe |
| Tauri CLI 2.11.4 | `package-lock.json`, and `cargo install` in the recipe |
| NDK 27.2.12479018 | `patch-android-signing.py`, workflows, `ndk: r27c` |
| JDK 21, Node 20 | `release.yml`, and `JAVA_HOME` in the recipe |
| libclang 19 | `LIBCLANG_PATH` in both — bindgen's output depends on it |
| RUSTFLAGS | `.github/scripts/android-release-rustflags.sh`, run by both |

The JDK is the one pin we don't get to choose. `buildserver-trixie` installs
`default-jdk-headless` and then switches to the highest JDK present, and Debian
trixie has no openjdk-17 package at all, so 21 is the only version both sides can
agree on. The recipe installs `openjdk-21-jdk-headless` explicitly rather than
relying on it arriving via `default-jdk`, so a future image bumping its default
doesn't silently change the compiler.

The Gradle side matters too: `tauri android init` scaffolds a release build type
with R8 **on** and no `ndkVersion`, which leaves AGP unable to strip the native
library. Our workflows fix both via `patch-android-signing.py`, so the recipe
runs the same script with `--no-signing` — without it F-Droid would ship a
differently-optimised, unstripped APK, quite apart from reproducibility.

### The dependency-metadata block

AGP appends a "Dependency metadata" entry to the APK signing block: a blob
listing the app's dependencies, encrypted to a Google key, for Play Console's
vulnerability warnings. F-Droid's scanner rejects *any* signing block it does
not recognise, and it scans the binary named by `Binaries:` — so an APK carrying
one fails the pipeline outright, with the build itself perfectly fine.

`patch-android-signing.py` turns it off with `dependenciesInfo { includeInApk =
false }`. `includeInBundle` is deliberately left alone: the bundle goes to Play,
which is the only consumer that reads it.

### The INTERNET permission

Tauri's manifest template requests `android.permission.INTERNET`
unconditionally, because `tauri android dev` loads the frontend from a dev
server. A packaged build never does: every request goes through
`RustWebViewClient.shouldInterceptRequest` and is answered from assets inside the
APK. The app has no HTTP client, no updater, no `fetch`/`XHR`/`WebSocket`, and a
CSP with no external origins; the Ko-fi link hands a URL to the system browser,
which networks under its own permission.

`patch-android-signing.py` removes it. Verified by installing a build without it
on an Android 35 emulator: the app launches, the WebView renders the full UI, and
logcat shows no `ERR_ACCESS_DENIED` or `SecurityException`. `tauri android dev`
is unaffected — the dev flow does not run this script.

`check-android-apk.py`, run by the release job before upload, fails the build if
either the permission or an unrecognised signing block comes back.

### Why the release job builds from /home/vagrant/build/…

Tauri's `generate_context!` reads `$CARGO_MANIFEST_DIR` at compile time and
bakes the absolute path into the binary (`tauri-macros/src/context.rs`). That is
an environment variable's value rather than a source span, so
`--remap-path-prefix` cannot reach it — it was the single remaining
environment-specific string in an otherwise clean library.

Rather than have F-Droid build somewhere unusual, `release.yml` copies the
checkout to `/home/vagrant/build/com.safrone.trackexploder`, which is where the
F-Droid buildserver puts it, and compiles there. The recipe needs nothing
special, and the baked path matches.

Measured on 1.1.12, cross-compiling `libtrack_exploder_lib.so` for
`aarch64-linux-android`:

* before the remap — 299 absolute registry paths in `.rodata`
* after — 447 paths rewritten to `/cargo-registry/…`, one leak left, the
  `CARGO_MANIFEST_DIR` string
* two independent cold builds from the same fixed path — byte-identical
  (`sha256 6ac9c3b9…`)

If the buildserver's layout ever changes, that copy step is the thing to update.

Verifying a match needs a real `fdroid build` followed by `fdroid verify`; there
is no shortcut, and the first attempt on any new release is worth checking.

## Notes on the build recipe

The recipe is a fully custom build — F-Droid has no Tauri template, so it drives
`cargo tauri android build` itself. Points that are easy to get wrong:

- **The Tauri CLI must be `cargo install`ed**, not taken from npm. The npm
  package ships a prebuilt x86-64 binary, which F-Droid's scanner rejects.
- **`npm ci` belongs in the `build:` phase, not `prebuild:`.** The scanner runs
  between the two and deletes native binaries it finds — esbuild and Rollup's
  `.node` — from anything already unpacked.
- **`libclang-dev` is required.** `signalsmith-stretch` runs `bindgen` in its
  build script; the NDK does not ship a host `libclang` and the buildserver has
  none by default. GitHub's runners do, which is why CI never sees this.
- **The launcher icons must be copied over** `src-tauri/gen/android/app/src/main/res`
  after `cargo tauri android init`, exactly as the workflows in
  `.github/workflows/` do, or the APK ships Tauri's default icon.
- **The release build type is left unsigned.** F-Droid signs the APK itself, so
  the recipe must not apply `.github/scripts/patch-android-signing.py`; the
  expected output is `app-universal-release-unsigned.apk`.

The F-Droid build and the GitHub release APK have different signatures, so users
cannot update in place between the two — the same caveat that applies to Play,
noted in [android-play-release.md](android-play-release.md).
