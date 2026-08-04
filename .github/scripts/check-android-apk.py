#!/usr/bin/env python3
"""Assert the properties of a release APK that F-Droid enforces.

Both checks here started as F-Droid pipeline failures on an already-published
release, which is an expensive way to find out. They cost a build second each.

Usage: check-android-apk.py <apk>
"""

import struct
import subprocess
import sys

# APK signing block pair IDs we expect to see. Anything else is what F-Droid's
# scanner calls an "extra signing block" and refuses to publish.
KNOWN_BLOCK_IDS = {
    0x7109871A: "APK Signature Scheme v2",
    0xF05368C0: "APK Signature Scheme v3",
    0x1B93AD61: "APK Signature Scheme v3.1",
    0x42726577: "padding",
}
DEPENDENCY_METADATA = 0x504B4453

MAGIC = b"APK Sig Block 42"


def signing_block_ids(path: str) -> list[int]:
    with open(path, "rb") as fh:
        data = fh.read()
    magic = data.rfind(MAGIC)
    if magic < 0:
        sys.exit("check-android-apk: no APK signing block; is the APK signed?")
    (block_size,) = struct.unpack_from("<Q", data, magic - 8)
    # The block is [size][pairs...][size][magic]; the leading size field sits
    # block_size + 8 bytes before the end, so the pairs start 8 bytes after that.
    off = magic + len(MAGIC) - block_size
    ids = []
    while off < magic - 8:
        (pair_len,) = struct.unpack_from("<Q", data, off)
        if pair_len < 4 or off + 8 + pair_len > magic:
            sys.exit("check-android-apk: malformed signing block")
        (pair_id,) = struct.unpack_from("<I", data, off + 8)
        ids.append(pair_id)
        off += 8 + pair_len
    return ids


def permissions(apk: str, aapt2: str) -> list[str]:
    out = subprocess.run(
        [aapt2, "dump", "permissions", apk],
        capture_output=True, text=True, check=True,
    ).stdout
    return [
        line.split("name='", 1)[1].rstrip("'\n")
        for line in out.splitlines()
        if line.startswith("uses-permission:")
    ]


def main() -> None:
    if len(sys.argv) < 3:
        sys.exit("usage: check-android-apk.py <apk> <aapt2>")
    apk, aapt2 = sys.argv[1], sys.argv[2]
    failures = []

    ids = signing_block_ids(apk)
    for pair_id in ids:
        if pair_id not in KNOWN_BLOCK_IDS:
            name = (
                "AGP dependency metadata"
                if pair_id == DEPENDENCY_METADATA
                else "unrecognised"
            )
            failures.append(
                f"extra signing block 0x{pair_id:08x} ({name}) — F-Droid's "
                "scanner rejects APKs carrying one"
            )
    print("signing block:", ", ".join(
        KNOWN_BLOCK_IDS.get(i, f"0x{i:08x}") for i in ids
    ))

    perms = permissions(apk, aapt2)
    if "android.permission.INTERNET" in perms:
        failures.append(
            "the INTERNET permission is present — the app does no networking, "
            "and patch-android-signing.py should have removed it"
        )
    print("permissions:", ", ".join(perms) or "(none)")

    if failures:
        for f in failures:
            print(f"::error::{f}", file=sys.stderr)
        sys.exit(1)
    print("APK checks passed")


if __name__ == "__main__":
    main()
