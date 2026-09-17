import { useCallback, useState } from "react";

/** 面板在取景区的哪一边。 */
export type PanelSide = "left" | "right";

export interface LensLayout {
  side: PanelSide;
  panelWidth: number;
}

export const PANEL_MIN_WIDTH = 280;
export const PANEL_MAX_WIDTH = 640;

const STORAGE_KEY = "trans.lens.layout";
const DEFAULT_LAYOUT: LensLayout = { side: "right", panelWidth: 360 };

export const clampPanelWidth = (width: number) =>
  Math.round(Math.min(PANEL_MAX_WIDTH, Math.max(PANEL_MIN_WIDTH, width)));

/** 解析保存的布局；为空、不是 JSON、字段非法时逐项退回默认值。 */
export function parseLayout(raw: string | null): LensLayout {
  if (!raw) return DEFAULT_LAYOUT;
  try {
    const parsed = JSON.parse(raw) as Partial<LensLayout> | null;
    return {
      side: parsed?.side === "left" ? "left" : "right",
      panelWidth: clampPanelWidth(Number(parsed?.panelWidth) || DEFAULT_LAYOUT.panelWidth),
    };
  } catch {
    return DEFAULT_LAYOUT;
  }
}

/** 读保存的布局；存储不可用或内容损坏时退回默认值，不影响取景框正常使用。 */
function loadLayout(): LensLayout {
  try {
    return parseLayout(localStorage.getItem(STORAGE_KEY));
  } catch {
    return DEFAULT_LAYOUT;
  }
}

function saveLayout(layout: LensLayout) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(layout));
  } catch {
    // 只是布局偏好，存不下就算了，下次用默认布局。
  }
}

/** 取景框布局偏好：面板左右、面板宽度。每次修改都记住，下次打开保持不变。 */
export function useLensLayout() {
  const [layout, setLayout] = useState<LensLayout>(loadLayout);

  const update = useCallback((change: Partial<LensLayout>) => {
    setLayout((current) => {
      const next = { ...current, ...change };
      next.panelWidth = clampPanelWidth(next.panelWidth);
      saveLayout(next);
      return next;
    });
  }, []);

  const swapSide = useCallback(() => {
    setLayout((current) => {
      const next: LensLayout = { ...current, side: current.side === "right" ? "left" : "right" };
      saveLayout(next);
      return next;
    });
  }, []);

  return { layout, update, swapSide };
}
