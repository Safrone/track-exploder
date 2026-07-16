import type { Action } from "svelte/action";

/**
 * Reset a range/number `<input>` to a default value on double-click (desktop)
 * or double-tap (touch), firing an `input` event so any bound handler updates.
 * Touch needs explicit handling because a WebView doesn't synthesize `dblclick`
 * from two quick taps on a range thumb.
 *
 * Usage: `<input type="range" use:resetOnDblClick={defaultValue} … />`
 */
export const resetOnDblClick: Action<HTMLInputElement, number> = (node, defaultValue) => {
  let value = defaultValue;
  const reset = () => {
    node.value = String(value);
    node.dispatchEvent(new Event("input", { bubbles: true }));
    node.dispatchEvent(new Event("change", { bubbles: true }));
  };

  let lastTap = 0;
  const onTouchEnd = (e: TouchEvent) => {
    const now = e.timeStamp;
    if (now - lastTap < 300) {
      e.preventDefault(); // don't let the second tap nudge the slider
      reset();
      lastTap = 0;
    } else {
      lastTap = now;
    }
  };

  node.addEventListener("dblclick", reset);
  node.addEventListener("touchend", onTouchEnd);
  return {
    update(next: number) {
      value = next;
    },
    destroy() {
      node.removeEventListener("dblclick", reset);
      node.removeEventListener("touchend", onTouchEnd);
    },
  };
};
