import { getCurrentWebview } from "@tauri-apps/api/webview";
import { PARTS, type Part } from "../types";
import { isAudioFile } from "./load";

export interface DropHandlers {
  /**
   * Audio files are being dragged over the window. `part` is the part strip a
   * single file would land on, or null when the drop would auto-assign by name.
   */
  onHover: (part: Part | null) => void;
  /** The drag left the window or was cancelled. */
  onLeave: () => void;
  /** Files were dropped; `part` as for {@link DropHandlers.onHover}. */
  onDrop: (paths: string[], part: Part | null) => void;
}

/**
 * Listen for files dragged onto the window from the OS file manager. Tauri
 * intercepts the drop and hands over real paths, which the DOM's own drop event
 * never exposes. Part strips opt in as targets with a `data-drop-part` attribute.
 * Resolves to an unlisten function.
 */
export function listenForFileDrops(handlers: DropHandlers): Promise<() => void> {
  let audioCount = 0;
  return getCurrentWebview().onDragDropEvent(({ payload }) => {
    switch (payload.type) {
      case "enter":
        audioCount = payload.paths.filter(isAudioFile).length;
        if (audioCount) handlers.onHover(audioCount === 1 ? partAt(payload.position) : null);
        break;
      case "over":
        if (audioCount) handlers.onHover(audioCount === 1 ? partAt(payload.position) : null);
        break;
      case "drop": {
        const paths = payload.paths.filter(isAudioFile);
        handlers.onDrop(paths, paths.length === 1 ? partAt(payload.position) : null);
        audioCount = 0;
        break;
      }
      case "leave":
        audioCount = 0;
        handlers.onLeave();
        break;
    }
  });
}

/** The part strip under a drag position, if any. */
function partAt(position: { x: number; y: number }): Part | null {
  // Despite the PhysicalPosition type, only WebView2 (Windows) reports physical
  // pixels; WebKitGTK and WKWebView already report CSS pixels.
  const scale = /Windows/.test(navigator.userAgent) ? window.devicePixelRatio : 1;
  const el = document.elementFromPoint(position.x / scale, position.y / scale);
  const part = el?.closest<HTMLElement>("[data-drop-part]")?.dataset.dropPart;
  return PARTS.find((p) => p === part) ?? null;
}
