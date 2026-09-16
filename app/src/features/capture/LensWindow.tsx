import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";

import { closeLens, onCaptureStatus, previewCapture, startCapture, stopCapture } from "../../api/capture";
import { AppError, toAppError } from "../../api/client";
import type { CaptureStatus } from "../../api/types";
import { useConversationStore } from "../composer/useConversationStore";

/** 可以拖动缩放的取景框：框到哪里就识别哪里。中间镂空，显示底下的内容。 */
export function LensWindow() {
  const hollow = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<CaptureStatus | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  const [recognized, setRecognized] = useState<number | null>(null);
  const presetId = useConversationStore((s) => s.presetId);
  const running = status?.running != null;

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onCaptureStatus(setStatus)
      .then((fn) => (unlisten = fn))
      .catch((e) => console.error("capture-status subscription failed", e));
    return () => unlisten?.();
  }, []);

  /** 镂空区域在屏幕上的绝对物理像素位置——后端就按这块区域截图。 */
  const captureRect = useCallback(async () => {
    const box = hollow.current?.getBoundingClientRect();
    if (!box) throw new AppError({ code: "lens_not_ready", message: "取景框还没准备好" });
    const window = getCurrentWindow();
    const [position, scale] = await Promise.all([window.outerPosition(), window.scaleFactor()]);
    return {
      mode: "lens" as const,
      x: position.x + Math.round(box.left * scale),
      y: position.y + Math.round(box.top * scale),
      width: Math.round(box.width * scale),
      height: Math.round(box.height * scale),
    };
  }, []);

  const run = async (action: (presetId: string) => Promise<void>) => {
    if (!presetId) {
      setError(new AppError({ code: "no_preset", message: "请先在主窗口的翻译页选择术语场景" }));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await action(presetId);
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setBusy(false);
    }
  };

  const preview = () =>
    void run(async (preset) => {
      const result = await previewCapture(await captureRect(), preset);
      setRecognized(result.messages.length);
    });

  const start = () =>
    void run(async (preset) => {
      setStatus(await startCapture(await captureRect(), preset));
    });

  const stop = () =>
    void run(async () => {
      setStatus(await stopCapture());
    });

  return (
    <div className="flex h-screen w-screen flex-col bg-transparent">
      <header
        data-tauri-drag-region
        className="flex cursor-move items-center gap-2 rounded-t bg-zinc-900/95 px-2 py-1 text-xs text-zinc-200"
      >
        <span data-tauri-drag-region className="select-none font-medium">
          取景框
        </span>
        <LensButton onClick={preview} disabled={busy}>
          预览
        </LensButton>
        {running ? (
          <LensButton onClick={stop} disabled={busy} tone="danger">
            停止
          </LensButton>
        ) : (
          <LensButton onClick={start} disabled={busy} tone="primary">
            开始识别
          </LensButton>
        )}
        <LensButton onClick={() => void closeLens()}>关闭</LensButton>
        <span className="ml-auto truncate text-zinc-400">
          {error
            ? error.message
            : running
              ? `识别中 · 屏上 ${status?.last_seen ?? 0} 条`
              : recognized === null
                ? "拖动标题栏移动，拖角落改大小"
                : `预览识别到 ${recognized} 条`}
        </span>
      </header>

      {/* 镂空区域：窗口透明，这里能看到底下的内容。已知限制：这块区域仍会挡住鼠标点击。 */}
      <div ref={hollow} className="relative flex-1 border-2 border-emerald-500/80">
        <ResizeHandle corner="southEast" className="bottom-0 right-0 cursor-nwse-resize" />
        <ResizeHandle corner="southWest" className="bottom-0 left-0 cursor-nesw-resize" />
        <ResizeHandle corner="northEast" className="right-0 top-0 cursor-nesw-resize" />
        <ResizeHandle corner="northWest" className="left-0 top-0 cursor-nwse-resize" />
      </div>
    </div>
  );
}

function LensButton({
  children,
  onClick,
  disabled,
  tone = "plain",
}: {
  children: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
  tone?: "plain" | "primary" | "danger";
}) {
  const tones = {
    plain: "bg-zinc-800 hover:bg-zinc-700",
    primary: "bg-emerald-600 hover:bg-emerald-500",
    danger: "bg-rose-700 hover:bg-rose-600",
  } as const;
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={`rounded px-2 py-0.5 text-xs text-white disabled:opacity-50 ${tones[tone]}`}
    >
      {children}
    </button>
  );
}

/** 四角的缩放把手，交给系统做窗口缩放。 */
function ResizeHandle({ corner, className }: { corner: string; className: string }) {
  return (
    <div
      className={`absolute h-4 w-4 bg-emerald-500/70 ${className}`}
      onMouseDown={(e) => {
        e.preventDefault();
        void getCurrentWindow().startResizeDragging(corner as never);
      }}
    />
  );
}
