#!/bin/sh
# RUSTFLAGS for Android release builds, shared by our workflows and F-Droid's
# from-source build. Both must use the exact same flags or the two APKs stop
# being byte-identical and F-Droid drops the reproducible-build verification.
#
# Cargo treats its flag sources as mutually exclusive rather than merging them:
# setting RUSTFLAGS silently discards `target.*.rustflags` from .cargo/config.toml,
# so the 16 KB alignment link-arg has to be repeated here. Keep the two in sync.
#
# The remaps are what make the build reproducible. std ships pre-remapped to
# /rustc/<hash>, but both the registry checkout and the workspace itself are
# passed to rustc as absolute paths, and the panic locations built from them
# survive `strip = true` in .rodata — a 1.1.12 build has 299, spelling out
# whichever home directory did the compiling.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
printf '%s' "-C link-arg=-Wl,-z,max-page-size=16384 --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}/registry=/cargo-registry --remap-path-prefix=$root=/src"
