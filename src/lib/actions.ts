import type { Action } from "svelte/action";

export interface RangeGestureOptions {
  /** Value to snap to on double-tap / double-click. Omit to disable reset. */
  resetValue?: number;
  /** Movement (px) before a drag's axis is decided. */
  threshold?: number;
}

/**
 * Touch-friendly range slider behaviour for `<input type="range">`.
 *
 * The problem: in the Android WebView (Chromium), a touch that lands on a
 * slider changes its value immediately, so a vertical scroll that happens to
 * start on a fader nudges it before the browser decides the gesture was a
 * scroll. `touch-action: pan-y` only *permits* vertical scrolling; it doesn't
 * stop the native value change, and (tested on-device) neither does
 * `preventDefault()` on `pointerdown`.
 *
 * The fix works one level down, on the value itself: while a touch gesture's
 * axis is still undecided (or has been judged vertical), we revert every native
 * value change and stop its `input` event, so the fader can't move and the page
 * scrolls freely. Once the drag is judged horizontal we step aside and let the
 * native slider do its normal thing. Mouse and keyboard stay fully native. A
 * double-tap (or desktop double-click) resets to `resetValue`.
 *
 * Usage: `<input type="range" use:rangeGesture={{ resetValue: 1 }} … />`
 */
export const rangeGesture: Action<HTMLInputElement, RangeGestureOptions | undefined> = (
  node,
  options = {},
) => {
  let resetValue = options.resetValue;
  let threshold = options.threshold ?? 8;

  let touchActive = false;
  let pointerId: number | null = null;
  let startX = 0;
  let startY = 0;
  let startValue = "";
  let intent: "pending" | "horizontal" | "vertical" = "pending";
  let selfDispatch = false;
  let lastTap = 0;

  /** Fire input/change ourselves; flagged so the suppressor lets it through. */
  const emit = (type: "input" | "change") => {
    selfDispatch = true;
    node.dispatchEvent(new Event(type, { bubbles: true }));
    selfDispatch = false;
  };

  // Revert + swallow any native value change until the drag is horizontal.
  const onInput = (event: Event) => {
    if (touchActive && intent !== "horizontal" && !selfDispatch) {
      node.value = startValue;
      event.stopImmediatePropagation();
    }
  };

  /** Set the value from an x-coordinate (used for tap-to-position). */
  const setFromX = (clientX: number) => {
    const rect = node.getBoundingClientRect();
    if (rect.width <= 0) return;

    const min = Number(node.min || 0);
    const max = Number(node.max || 100);
    const step = node.step === "any" ? 0 : Number(node.step || 1);

    let ratio = (clientX - rect.left) / rect.width;
    ratio = Math.max(0, Math.min(1, ratio));
    if (getComputedStyle(node).direction === "rtl") ratio = 1 - ratio;

    let value = min + ratio * (max - min);
    if (step > 0) {
      value = min + Math.round((value - min) / step) * step;
      // Trim floating-point tails such as 0.30000000000000004.
      const decimals = (String(step).split(".")[1] ?? "").length;
      value = Number(value.toFixed(decimals));
    }
    value = Math.max(min, Math.min(max, value));

    if (String(value) !== node.value) {
      node.value = String(value);
      emit("input");
    }
  };

  const reset = () => {
    if (resetValue === undefined) return;
    node.value = String(resetValue);
    emit("input");
    emit("change");
  };

  const cleanup = () => {
    touchActive = false;
    pointerId = null;
    intent = "pending";
  };

  const onPointerDown = (event: PointerEvent) => {
    // Mouse stays fully native (normal range + dblclick behaviour).
    if (event.pointerType === "mouse" || node.disabled) return;
    touchActive = true;
    pointerId = event.pointerId;
    startX = event.clientX;
    startY = event.clientY;
    startValue = node.value;
    intent = "pending";
  };

  const onPointerMove = (event: PointerEvent) => {
    if (!touchActive || event.pointerId !== pointerId || intent !== "pending") return;
    const dx = event.clientX - startX;
    const dy = event.clientY - startY;
    if (Math.hypot(dx, dy) < threshold) return;
    // Horizontal → hand control to the native slider (the suppressor steps
    // aside). Vertical → keep suppressing so the browser scrolls the page.
    intent = Math.abs(dy) >= Math.abs(dx) ? "vertical" : "horizontal";
  };

  const onPointerUp = (event: PointerEvent) => {
    if (event.pointerId !== pointerId) return;
    if (intent === "pending") {
      // A tap (no real drag). Double-tap resets; a single tap sets position.
      const now = event.timeStamp;
      if (resetValue !== undefined && now - lastTap < 300) {
        reset();
        lastTap = 0;
      } else {
        setFromX(event.clientX);
        lastTap = now;
      }
    }
    cleanup();
  };

  const onPointerCancel = (event: PointerEvent) => {
    if (event.pointerId === pointerId) cleanup();
  };

  const onDoubleClick = () => reset();

  // Capture phase so we run before Svelte's (root-delegated) oninput handler.
  node.addEventListener("input", onInput, { capture: true });
  node.addEventListener("pointerdown", onPointerDown);
  node.addEventListener("pointermove", onPointerMove);
  node.addEventListener("pointerup", onPointerUp);
  node.addEventListener("pointercancel", onPointerCancel);
  node.addEventListener("dblclick", onDoubleClick);

  return {
    update(next: RangeGestureOptions = {}) {
      resetValue = next.resetValue;
      threshold = next.threshold ?? 8;
    },
    destroy() {
      node.removeEventListener("input", onInput, true);
      node.removeEventListener("pointerdown", onPointerDown);
      node.removeEventListener("pointermove", onPointerMove);
      node.removeEventListener("pointerup", onPointerUp);
      node.removeEventListener("pointercancel", onPointerCancel);
      node.removeEventListener("dblclick", onDoubleClick);
    },
  };
};
