import type { Action } from "svelte/action";

/**
 * Reset a range/number `<input>` to a default value on double-click, firing an
 * `input` event so any bound handler updates too.
 *
 * Usage: `<input type="range" use:resetOnDblClick={defaultValue} … />`
 */
export const resetOnDblClick: Action<HTMLInputElement, number> = (node, defaultValue) => {
  let value = defaultValue;
  const handler = () => {
    node.value = String(value);
    node.dispatchEvent(new Event("input", { bubbles: true }));
  };
  node.addEventListener("dblclick", handler);
  return {
    update(next: number) {
      value = next;
    },
    destroy() {
      node.removeEventListener("dblclick", handler);
    },
  };
};
