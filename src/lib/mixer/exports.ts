import { writable } from "svelte/store";

export interface ExportRecord {
  path: string;
  name: string;
  format: string;
  at: number;
}

const LIST_KEY = "trackexploder.exports";
const DIR_KEY = "trackexploder.lastExportDir";
const COUNT_KEY = "trackexploder.exportCount";
const THANKS_KEY = "trackexploder.thanksShown";
const MAX_RECORDS = 20;
/** Lifetime exports that earn the (one and only) thank-you note. */
const THANKS_AT = 1;

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

/** Open state of the one-time thank-you note. */
export const showThanks = writable<boolean>(false);

export function dismissThanks(): void {
  showThanks.set(false);
}

/**
 * Count the export and open the thank-you note once the user has a file to show
 * for it. The flag is written before showing it so it can't come back on a later
 * export or a later run.
 */
function tallyForThanks(): void {
  let count: number;
  try {
    if (localStorage.getItem(THANKS_KEY)) return;
    count = (Number(localStorage.getItem(COUNT_KEY)) || 0) + 1;
    localStorage.setItem(COUNT_KEY, String(count));
    if (count < THANKS_AT) return;
    localStorage.setItem(THANKS_KEY, "1");
  } catch {
    return; // without storage there's no way to ask only once
  }
  showThanks.set(true);
}

/** Record a successful export (most recent first in the UI). */
export function addExport(rec: ExportRecord): void {
  exportsList.update((list) => [...list, rec].slice(-MAX_RECORDS));
  tallyForThanks();
}

/** Clear the exported-files history (does not touch the files on disk). */
export function clearExports(): void {
  exportsList.set([]);
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
