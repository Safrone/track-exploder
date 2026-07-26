#!/usr/bin/env python3
"""Inject a release signing config into Tauri's generated Android Gradle build.

`tauri android init` scaffolds `app/build.gradle.kts` fresh on every CI run (the
`gen/` tree is git-ignored), and its release build type ships with *no* signing
config — so a release APK would be unsigned and un-installable. This adds a
`signingConfigs.release` that reads `keystore.properties`, points the release
build type at it, and disables R8/minification (the size win comes from the
stripped native library, not from shrinking the small Kotlin layer, and leaving
minification off avoids an untestable class of runtime breakage).

Anchored on structural tokens from the Tauri template; if the template changes
shape the script fails loudly rather than emitting a broken build file.
"""

import sys

GRADLE = "src-tauri/gen/android/app/build.gradle.kts"

# In a Gradle Kotlin script `java` resolves to the Gradle `java` extension, not
# the JDK package, so `java.util.Properties` / `java.io.File` don't compile. Use
# the imported `Properties` and Gradle's `rootProject.file(...)` instead. The
# `!!` asserts non-null (getProperty is nullable; the signing setters aren't).
SIGNING_CONFIG = """
    signingConfigs {
        create("release") {
            val props = Properties()
            val f = rootProject.file("keystore.properties")
            if (f.exists()) f.inputStream().use { props.load(it) }
            storeFile = rootProject.file(props.getProperty("storeFile")!!)
            storePassword = props.getProperty("password")!!
            keyAlias = props.getProperty("keyAlias")!!
            keyPassword = props.getProperty("password")!!
        }
    }
"""

PROPERTIES_IMPORT = "import java.util.Properties"


def fail(msg: str) -> None:
    sys.exit(f"patch-android-signing: {msg}")


def main() -> None:
    with open(GRADLE) as fh:
        src = fh.read()

    # `Properties` must be imported (the template does, but be robust).
    if PROPERTIES_IMPORT not in src:
        src = PROPERTIES_IMPORT + "\n" + src

    anchor = "\nandroid {"
    if anchor not in src:
        fail("could not find the `android {` block")
    at = src.index(anchor) + len(anchor)
    src = src[:at] + "\n" + SIGNING_CONFIG + src[at:]

    release = 'getByName("release") {'
    if release not in src:
        fail("could not find the release build type")
    src = src.replace(
        release,
        release + '\n            signingConfig = signingConfigs.getByName("release")',
        1,
    )

    if "isMinifyEnabled = true" not in src:
        fail("release build type no longer sets isMinifyEnabled = true")
    src = src.replace("isMinifyEnabled = true", "isMinifyEnabled = false", 1)

    with open(GRADLE, "w") as fh:
        fh.write(src)
    print("patched", GRADLE, "for release signing")


if __name__ == "__main__":
    main()
