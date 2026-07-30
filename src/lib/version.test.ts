import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));

function read(path: string) {
  return JSON.parse(readFileSync(repoRoot + path, "utf8"));
}

// Tauri's own formula when `bundle.android.versionCode` is unset.
function derivedVersionCode(version: string): number {
  const [major, minor, patch] = version.split(".").map(Number);
  return major * 1000000 + minor * 1000 + patch;
}

describe("release version metadata", () => {
  // Google Play rejects a bundle whose versionCode it has already accepted, and
  // every release so far used Tauri's derived value. `bundle.android.versionCode`
  // is pinned so F-Droid's checkupdates can read it out of the source tree
  // (it needs a literal integer, and nothing else in the repo has one), which
  // also means it no longer follows the app version on its own.
  it("pins the Android versionCode to the value Tauri would derive", () => {
    const conf = read("src-tauri/tauri.conf.json");
    expect(conf.bundle.android.versionCode).toBe(derivedVersionCode(conf.version));
  });

  it("keeps package.json in step with tauri.conf.json", () => {
    expect(read("package.json").version).toBe(read("src-tauri/tauri.conf.json").version);
  });
});
