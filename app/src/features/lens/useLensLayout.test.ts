import { describe, expect, it } from "vitest";

import { clampPanelWidth, PANEL_MAX_WIDTH, PANEL_MIN_WIDTH, parseLayout } from "./useLensLayout";

describe("parseLayout", () => {
  it("uses the default layout when nothing was saved", () => {
    expect(parseLayout(null)).toEqual({ side: "right", panelWidth: 360 });
  });

  it("restores a saved layout", () => {
    expect(parseLayout(JSON.stringify({ side: "left", panelWidth: 420 }))).toEqual({ side: "left", panelWidth: 420 });
  });

  it("falls back field by field when the saved value is corrupt", () => {
    expect(parseLayout("not json")).toEqual({ side: "right", panelWidth: 360 });
    expect(parseLayout("null")).toEqual({ side: "right", panelWidth: 360 });
    expect(parseLayout(JSON.stringify({ side: "top", panelWidth: "wide" }))).toEqual({ side: "right", panelWidth: 360 });
  });

  it("clamps a saved width that is out of range", () => {
    expect(parseLayout(JSON.stringify({ side: "left", panelWidth: 5000 })).panelWidth).toBe(PANEL_MAX_WIDTH);
  });
});

describe("clampPanelWidth", () => {
  it("keeps the panel usable and rounds to whole pixels", () => {
    expect(clampPanelWidth(10)).toBe(PANEL_MIN_WIDTH);
    expect(clampPanelWidth(9999)).toBe(PANEL_MAX_WIDTH);
    expect(clampPanelWidth(333.6)).toBe(334);
  });
});
