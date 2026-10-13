import { describe, it, expect } from "vitest";
import { EXPORT_CREDIT, exportTags } from "./tags";
import { REPO_URL } from "../links";

describe("exportTags", () => {
  it("credits Track Exploder and links the repo", () => {
    expect(EXPORT_CREDIT).toContain(REPO_URL);
    expect(exportTags({ album: "Brigade" })).toEqual({ album: "Brigade", comment: EXPORT_CREDIT });
  });

  it("keeps a comment the sources share, ahead of the credit", () => {
    expect(exportTags({ comment: "© Tim Waurick" }).comment).toBe(
      `© Tim Waurick\n${EXPORT_CREDIT}`,
    );
  });

  it("doesn't credit twice when re-exporting an export", () => {
    const once = exportTags({});
    expect(exportTags(once)).toEqual(once);
  });
});
