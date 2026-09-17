import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { closeLens } from "../../api/capture";
import { ErrorBoundary } from "../../shared/ui/ErrorBoundary";
import { LensPanel } from "./LensPanel";
import { useLensStore, type LensRect } from "./useLensStore";

/**
 * 取景框窗口：左边是透明取景区，框到哪里就识别哪里；右边贴着面板，识别到的话、翻译、回复都在这里完成。
 * 整个窗口在打开时已被排除出截图，所以不会拍到自己。
 */
export function LensWindow() {
  const { t } = useTranslation();
  const hollow = useRef<HTMLDivElement>(null);
  const init = useLensStore((s) => s.init);
  const status = useLensStore((s) => s.status);

  useEffect(() => init(), [init]);

  /** 取景区在屏幕上的绝对物理像素位置——后端就按这块区域截图。 */
  const captureRect = useCallback(async (): Promise<LensRect | null> => {
    const box = hollow.current?.getBoundingClientRect();
    if (!box) return null;
    const window = getCurrentWindow();
    const [position, scale] = await Promise.all([window.outerPosition(), window.scaleFactor()]);
    return {
      x: position.x + Math.round(box.left * scale),
      y: position.y + Math.round(box.top * scale),
      width: Math.round(box.width * scale),
      height: Math.round(box.height * scale),
    };
  }, []);

  const running = status?.running != null;

  return (
    <div className="relative flex h-screen w-screen bg-transparent text-zinc-100">
      <section className="flex min-w-0 flex-1 flex-col">
        <header
          data-tauri-drag-region
          className="flex cursor-move select-none items-center gap-2 rounded-tl-md bg-zinc-900/95 px-3 py-1.5 text-xs"
        >
          <span data-tauri-drag-region className="font-medium">
            {t("lens.title")}
          </span>
          <span data-tauri-drag-region className="truncate text-zinc-400">
            {running ? t("lens.runningHint") : t("lens.dragHint")}
          </span>
          <button
            type="button"
            onClick={() => void closeLens()}
            className="ml-auto rounded px-2 py-0.5 text-zinc-300 hover:bg-zinc-700"
          >
            {t("lens.close")}
          </button>
        </header>
        {/* 取景区：完全透明，能看到底下的内容。识别中边框变绿并加粗，一眼能看出正在读哪块。 */}
        <div
          ref={hollow}
          className={running ? "flex-1 border-[3px] border-emerald-400" : "flex-1 border-2 border-dashed border-sky-400/90"}
        />
      </section>

      <ErrorBoundary>
        <LensPanel captureRect={captureRect} />
      </ErrorBoundary>

      <ResizeHandle direction="NorthWest" className="left-0 top-0 cursor-nwse-resize" />
      <ResizeHandle direction="SouthWest" className="bottom-0 left-0 cursor-nesw-resize" />
      <ResizeHandle direction="NorthEast" className="right-0 top-0 cursor-nesw-resize" />
      <ResizeHandle direction="SouthEast" className="bottom-0 right-0 cursor-nwse-resize" />
    </div>
  );
}

type ResizeDirection = "NorthWest" | "NorthEast" | "SouthWest" | "SouthEast";

/** 窗口四角的缩放把手，缩放交给系统完成。 */
function ResizeHandle({ direction, className }: { direction: ResizeDirection; className: string }) {
  return (
    <div
      className={`absolute z-10 h-3 w-3 bg-sky-400/80 ${className}`}
      onMouseDown={(e) => {
        e.preventDefault();
        void getCurrentWindow().startResizeDragging(direction);
      }}
    />
  );
}
