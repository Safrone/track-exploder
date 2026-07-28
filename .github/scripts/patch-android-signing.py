#!/usr/bin/env python3
"""Inject a release signing config into Tauri's generated Android Gradle build.

Used by both Android workflows: the sideload APK cut in `release.yml` and the
Play Store AAB cut in `android-aab.yml`. They differ only in which keystore
`keystore.properties` points at.

`tauri android init` scaffolds `app/build.gradle.kts` fresh on every CI run (the
`gen/` tree is git-ignored), and its release build type ships with *no* signing
config — so a release build would be unsigned and rejected. This adds a
`signingConfigs.release` that reads `keystore.properties`, points the release
build type at it, and disables R8/minification (R8 would save on the order of
1-2 MB of a ~14 MB download, but Tauri's Kotlin classes are reached from Rust
over JNI, where R8 can strip or rename them in ways that only fail at runtime).

It also pins `ndkVersion`. The template leaves it unset, so AGP looks for its own
default NDK, which is not the one CI installs; it then can't run the tasks that
strip native libraries and extract their debug symbols, and silently packages the
libraries as-is. That shipped a 15 MB unstripped library in the 1001006 bundle.

Anchored on structural tokens from the Tauri template; if the template changes
shape the script fails loudly rather than emitting a broken build file.
"""

import os
import sys

GRADLE = "src-tauri/gen/android/app/build.gradle.kts"

# In a Gradle Kotlin script `java` resolves to the Gradle `java` extension, not
# the JDK package, so `java.util.Properties` / `java.io.File` don't compile. Use
# the imported `Properties` and Gradle's `rootProject.file(...)` instead. The
# `!!` asserts non-null (getProperty is nullable; the signing setters aren't).
#
# `storePassword`/`keyPassword` fall back to a single `password` key: the debug
# keystore used for the sideload APK shares one password, while a real Play
# upload keystore usually has separate store and key passwords.
SIGNING_CONFIG = """
    signingConfigs {
        create("release") {
            val props = Properties()
            val f = rootProject.file("keystore.properties")
            if (f.exists()) f.inputStream().use { props.load(it) }
            storeFile = rootProject.file(props.getProperty("storeFile")!!)
            storePassword = props.getProperty("storePassword") ?: props.getProperty("password")!!
            keyAlias = props.getProperty("keyAlias")!!
            keyPassword = props.getProperty("keyPassword") ?: props.getProperty("password")!!
        }
    }
"""

PROPERTIES_IMPORT = "import java.util.Properties"

# Must match the `sdkmanager "ndk;<version>"` line in both Android workflows.
# AGP 8.11's own default is 27.0.12077973, which CI does not install.
NDK_VERSION = "27.2.12479018"

NDK_PIN = f"""
    ndkVersion = "{NDK_VERSION}"
"""

# Play warns when a bundle carries native code without symbols, and a Rust panic
# aborts, so an unsymbolicated crash report is a bare address in
# libtrack_exploder_lib.so. SYMBOL_TABLE rather than FULL: the library carries
# DWARF, and function names are enough to read a stack trace without paying to
# upload several MB of debug_info.
#
# AGP extracts these into BUNDLE-METADATA, which Play consumes and never delivers
# to devices, and separately strips the packaged library. Both tasks need the NDK
# pinned above. Symbols only exist to extract if cargo left them in — see
# CARGO_PROFILE_RELEASE_STRIP in android-aab.yml.
DEBUG_SYMBOLS = """
            ndk {
                debugSymbolLevel = "SYMBOL_TABLE"
            }"""


def fail(msg: str) -> None:
    sys.exit(f"patch-android-signing: {msg}")


def check_ndk() -> None:
    """Fail if the NDK we pin isn't installed.

    AGP's own response to a missing NDK is a warning buried in Gradle output and
    a silently unstripped library, which is how 1001006 shipped. Catch it here.
    """
    home = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    if not home:
        print("warning: ANDROID_HOME unset, not checking for NDK", NDK_VERSION)
        return
    path = os.path.join(home, "ndk", NDK_VERSION)
    if not os.path.isdir(path):
        fail(
            f"NDK {NDK_VERSION} is not installed at {path}. The workflow's "
            "`sdkmanager \"ndk;<version>\"` and NDK_VERSION here must agree, or "
            "AGP cannot strip native libraries or extract their debug symbols."
        )


def main() -> None:
    debug_symbols = "--debug-symbols" in sys.argv[1:]
    check_ndk()

    with open(GRADLE) as fh:
        src = fh.read()

    # `Properties` must be imported (the template does, but be robust).
    if PROPERTIES_IMPORT not in src:
        src = PROPERTIES_IMPORT + "\n" + src

    anchor = "\nandroid {"
    if anchor not in src:
        fail("could not find the `android {` block")
    # If the template starts pinning this itself, its assignment would come after
    # ours and win silently. Stop and re-check rather than guess which is right.
    if "ndkVersion" in src:
        fail("the template now sets ndkVersion itself; reconcile with NDK_VERSION")
    at = src.index(anchor) + len(anchor)
    src = src[:at] + NDK_PIN + SIGNING_CONFIG + src[at:]

    release = 'getByName("release") {'
    if release not in src:
        fail("could not find the release build type")
    injected = '\n            signingConfig = signingConfigs.getByName("release")'
    if debug_symbols:
        injected += DEBUG_SYMBOLS
    src = src.replace(release, release + injected, 1)

    if "isMinifyEnabled = true" not in src:
        fail("release build type no longer sets isMinifyEnabled = true")
    src = src.replace("isMinifyEnabled = true", "isMinifyEnabled = false", 1)

    with open(GRADLE, "w") as fh:
        fh.write(src)
    what = f"release signing (NDK {NDK_VERSION})"
    if debug_symbols:
        what += " + native debug symbols"
    print("patched", GRADLE, "for", what)


if __name__ == "__main__":
    main()
