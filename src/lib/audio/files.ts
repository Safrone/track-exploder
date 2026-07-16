import { readFile, writeFile } from "@tauri-apps/plugin-fs";

/**
 * File byte I/O that works on desktop (paths) and mobile (content:// URIs),
 * so the Rust core never has to touch the filesystem itself.
 */

export async function readBytes(path: string): Promise<Uint8Array> {
  return readFile(path);
}

export async function writeBytes(path: string, data: Uint8Array): Promise<void> {
  await writeFile(path, data);
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
