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
