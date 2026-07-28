import { openUrl } from "@tauri-apps/plugin-opener";

export const REPO_URL: string = "https://github.com/Safrone/track-exploder";
export const AUTHOR = "Eric Blum";
export const CONTACT: string = "eblumster@gmail.com"; // empty hides the row
export const KOFI_URL: string = "https://ko-fi.com/safrone"; // empty hides the row

/** Open a URL in the system browser; only works inside the app, not a plain browser. */
export async function openExternal(url: string): Promise<void> {
  try {
    await openUrl(url);
  } catch {
    /* no opener outside the app */
  }
}
