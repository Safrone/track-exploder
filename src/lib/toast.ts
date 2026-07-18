import { writable } from "svelte/store";

export type ToastKind = "info" | "success" | "error";

export interface Toast {
  id: number;
  message: string;
  kind: ToastKind;
}

export const toasts = writable<Toast[]>([]);

let seq = 0;
const DEFAULT_MS = 4000;

/**
 * Show a transient toast. Returns its id. `ms <= 0` keeps it until dismissed.
 * Errors linger a little longer since they're more important to read.
 */
export function toast(message: string, kind: ToastKind = "info", ms?: number): number {
  const id = ++seq;
  toasts.update((list) => [...list, { id, message, kind }]);
  const life = ms ?? (kind === "error" ? 6000 : DEFAULT_MS);
  if (life > 0) setTimeout(() => dismiss(id), life);
  return id;
}

export function dismiss(id: number): void {
  toasts.update((list) => list.filter((t) => t.id !== id));
}
