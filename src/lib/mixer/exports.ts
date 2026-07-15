import { writable } from "svelte/store";

export interface ExportRecord {
  path: string;
  name: string;
  format: string;
  at: number;
}

const LIST_KEY = "trackexploder.exports";
const DIR_KEY = "trackexploder.lastExportDir";
const MAX_RECORDS = 20;

function loadList(): ExportRecord[] {
  try {
    const raw = localStorage.getItem(LIST_KEY);
    return raw ? (JSON.parse(raw) as ExportRecord[]) : [];
  } catch {
    return [];
  }
}

export const exportsList = writable<ExportRecord[]>(loadList());

exportsList.subscribe((list) => {
  try {
    localStorage.setItem(LIST_KEY, JSON.stringify(list.slice(-MAX_RECORDS)));
  } catch {
    /* ignore quota / unavailable storage */
  }
});

/** Record a successful export (most recent first in the UI). */
export function addExport(rec: ExportRecord): void {
  exportsList.update((list) => [...list, rec].slice(-MAX_RECORDS));
}

export function getLastExportDir(): string | null {
  try {
    return localStorage.getItem(DIR_KEY);
  } catch {
    return null;
  }
}

export function setLastExportDir(dir: string): void {
  try {
    localStorage.setItem(DIR_KEY, dir);
  } catch {
    /* ignore */
  }
}

/** Directory portion of a path, and the separator it uses. */
export function splitDir(path: string): { dir: string; sep: string } {
  const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (idx < 0) return { dir: "", sep: "/" };
  return { dir: path.slice(0, idx), sep: path[idx] };
}
