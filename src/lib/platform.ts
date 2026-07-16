/** True on Android/iOS webviews. Used to hide desktop-only features. */
export function isMobile(): boolean {
  if (typeof navigator === "undefined") return false;
  return /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
}

/** Convenience inverse. */
export function isDesktop(): boolean {
  return !isMobile();
}
