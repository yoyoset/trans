import { getCurrentWindow } from "@tauri-apps/api/window";
import clsx from "clsx";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  closeLens,
  lensClickThrough,
  onClickThrough,
  toggleLensClickThrough,
  updateCaptureRegion,
} from "../../api/capture";
import { ErrorBoundary } from "../../shared/ui/ErrorBoundary";
import { LensPanel } from "./LensPanel";
import { useLensLayout } from "./useLensLayout";
import { useLensStore, type LensRect } from "./useLensStore";

/** 挪动、缩放、拖分隔线时连续触发，停下来这么久之后才把新区域同步给后台。 */
const REGION_SYNC_DELAY_MS = 250;

/**
 * 取景框窗口：透明取景区框到哪里就识别哪里；旁边贴着面板，识别到的话、翻译、回复都在这里完成。
 * 面板可放左边或右边、宽度可拖。整个窗口在打开时已被排除出截图，不会拍到自己。
 */
export function LensWindow() {
  const { t } = useTranslation();
  const hollow = useRef<HTMLDivElement>(null);
  const init = useLensStore((s) => s.init);
  const status = useLensStore((s) => s.status);
  const { layout, update, swapSide } = useLensLayout();
  const running = status?.running != null;
  const panelOnLeft = layout.side === "left";

  useEffect(() => init(), [init]);

  // 让开鼠标时整个取景框不接收点击，只能靠热键恢复，所以要显眼地提示。
  const [clickThrough, setClickThrough] = useState(false);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    lensClickThrough()
      .then((on) => !disposed && setClickThrough(on))
      .catch((error) => console.error("lens_click_through failed", error));
    onClickThrough(setClickThrough)
      .then((fn) => (disposed ? fn() : (unlisten = fn)))
      .catch((error) =>
        console.error("click-through subscription failed", error),
      );
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  /** 取景区在屏幕上的绝对物理像素位置——后端就按这块区域截图。 */
  const captureRect = useCallback(async (): Promise<LensRect | null> => {
    const box = hollow.current?.getBoundingClientRect();
    if (!box) return null;
    const win = getCurrentWindow();
    const [position, scale] = await Promise.all([
      win.outerPosition(),
      win.scaleFactor(),
    ]);
    return {
      x: position.x + Math.round(box.left * scale),
      y: position.y + Math.round(box.top * scale),
      width: Math.round(box.width * scale),
      height: Math.round(box.height * scale),
    };
  }, []);

  // 识别中挪动取景框、改大小、换面板位置或宽度：把新区域同步给后台，否则后台还在读旧位置。
  const runningRef = useRef(running);
  runningRef.current = running;
  const syncTimer = useRef<number | undefined>(undefined);
  const scheduleRegionSync = useCallback(() => {
    if (!runningRef.current) return;
    window.clearTimeout(syncTimer.current);
    syncTimer.current = window.setTimeout(() => {
      void captureRect().then((rect) => {
        if (rect && runningRef.current) {
          updateCaptureRegion(rect).catch((error) =>
            console.error("update_capture_region failed", error),
          );
        }
      });
    }, REGION_SYNC_DELAY_MS);
  }, [captureRect]);

  useEffect(() => {
    const win = getCurrentWindow();
    let disposed = false;
    const unlisten: Array<() => void> = [];
    for (const pending of [
      win.onMoved(scheduleRegionSync),
      win.onResized(scheduleRegionSync),
    ]) {
      pending
        .then((fn) => (disposed ? fn() : unlisten.push(fn)))
        .catch((error) =>
          console.error("window event subscription failed", error),
        );
    }
    return () => {
      disposed = true;
      unlisten.forEach((fn) => fn());
      window.clearTimeout(syncTimer.current);
    };
  }, [scheduleRegionSync]);

  useEffect(() => {
    scheduleRegionSync();
  }, [layout.side, layout.panelWidth, scheduleRegionSync]);

  return (
    <div
      className={clsx(
        "relative flex h-screen w-screen bg-transparent text-zinc-100",
        panelOnLeft && "flex-row-reverse",
      )}
    >
      <section className="flex min-w-0 flex-1 flex-col">
        <header
          data-tauri-drag-region
          className={clsx(
            "flex cursor-move select-none items-center gap-2 bg-zinc-900/95 px-3 py-1.5 text-xs",
            panelOnLeft ? "rounded-tr-md" : "rounded-tl-md",
          )}
        >
          <span data-tauri-drag-region className="font-medium">
            {t("lens.title")}
          </span>
          <span
            data-tauri-drag-region
            className={clsx(
              "truncate",
              clickThrough ? "text-amber-300" : "text-zinc-400",
            )}
          >
            {clickThrough
              ? t("lens.clickThroughOn")
              : running
                ? t("lens.runningHint")
                : t("lens.dragHint")}
          </span>
          <button
            type="button"
            onClick={() => void toggleLensClickThrough()}
            className="shrink-0 rounded px-2 py-0.5 text-zinc-300 hover:bg-zinc-700"
            title={t("lens.clickThroughHint")}
          >
            {t("lens.clickThrough")}
          </button>
          <button
            type="button"
            onClick={swapSide}
            className="ml-auto shrink-0 rounded px-2 py-0.5 text-zinc-300 hover:bg-zinc-700"
          >
            {panelOnLeft ? t("lens.panelToRight") : t("lens.panelToLeft")}
          </button>
          <button
            type="button"
            onClick={() => void closeLens()}
            className="shrink-0 rounded px-2 py-0.5 text-zinc-300 hover:bg-zinc-700"
          >
            {t("lens.close")}
          </button>
        </header>
        {/* 取景区：完全透明，能看到底下的内容。识别中边框变绿并加粗，一眼能看出正在读哪块。 */}
        <div
          ref={hollow}
          className={
            running
              ? "flex-1 border-[3px] border-emerald-400"
              : "flex-1 border-2 border-dashed border-sky-400/90"
          }
        />
      </section>

      <Divider
        title={t("lens.dividerHint")}
        onDrag={(deltaX, startWidth) =>
          update({ panelWidth: startWidth + (panelOnLeft ? deltaX : -deltaX) })
        }
        panelWidth={layout.panelWidth}
      />

      <ErrorBoundary>
        <LensPanel
          captureRect={captureRect}
          width={layout.panelWidth}
          side={layout.side}
        />
      </ErrorBoundary>

      <ResizeHandle
        direction="NorthWest"
        className="left-0 top-0 cursor-nwse-resize"
      />
      <ResizeHandle
        direction="SouthWest"
        className="bottom-0 left-0 cursor-nesw-resize"
      />
      <ResizeHandle
        direction="NorthEast"
        className="right-0 top-0 cursor-nesw-resize"
      />
      <ResizeHandle
        direction="SouthEast"
        className="bottom-0 right-0 cursor-nwse-resize"
      />
    </div>
  );
}

/** 取景区和面板之间的分隔线，拖动调整面板宽度。 */
function Divider({
  title,
  panelWidth,
  onDrag,
}: {
  title: string;
  panelWidth: number;
  onDrag: (deltaX: number, startWidth: number) => void;
}) {
  return (
    <div
      title={title}
      className="w-1.5 shrink-0 cursor-col-resize bg-zinc-800 hover:bg-sky-500"
      onPointerDown={(e) => {
        e.preventDefault();
        const startX = e.clientX;
        const startWidth = panelWidth;
        const target = e.currentTarget;
        target.setPointerCapture(e.pointerId);
        const move = (event: PointerEvent) =>
          onDrag(event.clientX - startX, startWidth);
        const up = () => {
          target.removeEventListener("pointermove", move);
          target.removeEventListener("pointerup", up);
        };
        target.addEventListener("pointermove", move);
        target.addEventListener("pointerup", up);
      }}
    />
  );
}

type ResizeDirection = "NorthWest" | "NorthEast" | "SouthWest" | "SouthEast";

/** 窗口四角的缩放把手，缩放交给系统完成。 */
function ResizeHandle({
  direction,
  className,
}: {
  direction: ResizeDirection;
  className: string;
}) {
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
