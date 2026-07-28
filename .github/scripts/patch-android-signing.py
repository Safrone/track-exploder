#!/usr/bin/env python3
"""Inject a release signing config into Tauri's generated Android Gradle build.

Used by both Android workflows, which differ only in which keystore
`keystore.properties` points at. `tauri android init` scaffolds
`app/build.gradle.kts` fresh on every CI run (the `gen/` tree is git-ignored) and
its release build type has no signing config, so a release build would be
unsigned. This adds one, pins `ndkVersion` (left unset by the template, which
stops AGP stripping native libraries and extracting their debug symbols), and
turns off R8. See docs/android-play-release.md for the R8 trade-off.

Anchored on structural tokens from the template; if it changes shape the script
fails rather than emitting a broken build file.
"""

import os
import sys

GRADLE = "src-tauri/gen/android/app/build.gradle.kts"

# In a Gradle Kotlin script `java` resolves to the Gradle extension, not the JDK
# package, so `java.util.Properties` doesn't compile — hence the imported
# `Properties` and `rootProject.file(...)`.
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
NDK_VERSION = "27.2.12479018"

NDK_PIN = f"""
    ndkVersion = "{NDK_VERSION}"
"""

# SYMBOL_TABLE rather than FULL: function names are enough to read a native stack
# trace without also uploading the DWARF. Only has symbols to extract if cargo
# left them in — see CARGO_PROFILE_RELEASE_STRIP in android-aab.yml. AGP already
# defaults to this for non-debuggable variants; set it so a default change can't
# silently turn symbols off.
DEBUG_SYMBOLS = """
            ndk {
                debugSymbolLevel = "SYMBOL_TABLE"
            }"""

# The template nests `packaging { jniLibs.keepDebugSymbols += ... }` inside the
# debug build type, but `BuildType` has no `packaging` member — Kotlin resolves it
# against the enclosing `android` receiver, so it applies to every variant. Release
# then keeps its debug symbols, AGP skips stripping, and because the "stripped"
# copy is byte-identical to the original AGP concludes the symbols were already
# stripped and extracts nothing. Scope it back to debug via the variant API.
RELEASE_PACKAGING = """
androidComponents {
    onVariants(selector().withBuildType("release")) { variant ->
        variant.packaging.jniLibs.keepDebugSymbols.set(emptySet<String>())
    }
}
"""


def fail(msg: str) -> None:
    sys.exit(f"patch-android-signing: {msg}")


def check_ndk() -> None:
    """Fail if the pinned NDK isn't installed.

    AGP's own response is a warning buried in Gradle output and a silently
    unstripped library.
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
    # A template-set ndkVersion would come after ours and win silently.
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

    src = src.rstrip("\n") + "\n" + RELEASE_PACKAGING

    with open(GRADLE, "w") as fh:
        fh.write(src)
    what = f"release signing (NDK {NDK_VERSION})"
    if debug_symbols:
        what += " + native debug symbols"
    print("patched", GRADLE, "for", what)


if __name__ == "__main__":
    main()
