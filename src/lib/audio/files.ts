import { writeFile, remove, mkdir } from "@tauri-apps/plugin-fs";
import { appCacheDir, join } from "@tauri-apps/api/path";

/**
 * File byte output that works on desktop (paths) and mobile (content:// URIs).
 * Input files are read inside the Rust core (via the fs plugin) so large stems
 * never cross the IPC boundary.
 */

export async function writeBytes(path: string, data: Uint8Array): Promise<void> {
  await writeFile(path, data);
}

let tempSeq = 0;

/**
 * Write raw PCM bytes to a temp file in the app cache dir and return its path.
 * Used to hand large buffers to Rust commands: passing a path avoids the raw
 * ArrayBuffer IPC body, which the Android WebView delivers as JSON (not raw).
 * Caller should {@link removeTemp} the file once Rust has read it.
 */
export async function writeTempPcm(data: Uint8Array): Promise<string> {
  const dir = await appCacheDir();
  // The app cache dir may not exist yet (e.g. a fresh Linux profile), which
  // would make writeFile fail with ENOENT — create it first.
  await mkdir(dir, { recursive: true }).catch(() => {});
  const path = await join(dir, `te-pcm-${tempSeq++}.bin`);
  await writeFile(path, data);
  return path;
}

/** Delete a temp file, ignoring errors. */
export async function removeTemp(path: string): Promise<void> {
  try {
    await remove(path);
  } catch {
    /* best-effort cleanup */
  }
}

/** Lower-case file extension (no dot), or undefined. Used as a decode hint. */
export function extOf(path: string): string | undefined {
  const base = path.split(/[/\\]/).pop() ?? path;
  const dot = base.lastIndexOf(".");
  return dot > 0 ? base.slice(dot + 1).toLowerCase() : undefined;
}

/** True for a real filesystem path (desktop), false for a URI (mobile). */
export function isRealPath(path: string): boolean {
  return !path.includes("://");
}
