import clsx from "clsx";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "../../shared/ui/Button";
import { ErrorNotice } from "../../shared/ui/ErrorNotice";
import { LineItem } from "./LineItem";
import { ReplyBox } from "./ReplyBox";
import type { PanelSide } from "./useLensLayout";
import { REFRESH_OPTIONS, useLensStore, type LensRect } from "./useLensStore";

/** 取景框旁的面板：场景 → 开始/停止 → 识别到的句子（点哪句翻哪句）→ 我的回复。可放在取景区左边或右边。 */
export function LensPanel({
  captureRect,
  width,
  side,
}: {
  captureRect: () => Promise<LensRect | null>;
  width: number;
  side: PanelSide;
}) {
  const { t } = useTranslation();
  // 逐项订阅：只订阅面板自己要显示的字段，某一句翻译状态变化不会连带面板头部重渲染。
  const presets = useLensStore((s) => s.presets);
  const presetId = useLensStore((s) => s.presetId);
  const status = useLensStore((s) => s.status);
  const lines = useLensStore((s) => s.lines);
  const busy = useLensStore((s) => s.busy);
  const error = useLensStore((s) => s.error);
  const refreshMs = useLensStore((s) => s.refreshMs);
  const { setPreset, setRefreshMs, refreshNow, start: startCapture, stop, clearLines } = useLensStore.getState();
  const listEnd = useRef<HTMLDivElement>(null);
  const running = status?.running != null;

  // 必须用花括号：scrollIntoView 返回值不能交给 React 当清理函数（首次拉起黑屏就是这么来的）。
  useEffect(() => {
    listEnd.current?.scrollIntoView({ behavior: "smooth" });
  }, [lines.length]);

  const start = async () => {
    const rect = await captureRect();
    if (rect) await startCapture(rect);
  };

  return (
    <aside
      style={{ width }}
      className={clsx(
        "flex shrink-0 flex-col border-zinc-800 bg-zinc-950",
        side === "right" ? "rounded-r-md border-l" : "rounded-l-md border-r",
      )}
    >
      <header data-tauri-drag-region className="space-y-2 border-b border-zinc-800 p-3">
        <div className="flex items-center gap-2">
          <select
            className="input py-1 text-xs"
            value={presetId ?? ""}
            onChange={(e) => setPreset(e.target.value)}
            disabled={running}
            title={t("lens.presetHint")}
          >
            {presets.map((preset) => (
              <option key={preset.id} value={preset.id}>
                {preset.name}
              </option>
            ))}
          </select>
          {running ? (
            <Button variant="danger" onClick={() => void stop()} disabled={busy} className="shrink-0">
              {t("lens.stop")}
            </Button>
          ) : (
            <Button variant="primary" onClick={() => void start()} disabled={busy} className="shrink-0">
              {t("lens.start")}
            </Button>
          )}
        </div>
        <div className="flex items-center gap-2 text-xs text-zinc-400">
          <span className="shrink-0">{t("lens.refresh")}</span>
          <select
            className="input py-1 text-xs"
            value={refreshMs ?? "manual"}
            onChange={(e) => setRefreshMs(e.target.value === "manual" ? null : Number(e.target.value))}
            title={t("lens.refreshHint")}
          >
            {REFRESH_OPTIONS.map((ms) => (
              <option key={ms ?? "manual"} value={ms ?? "manual"}>
                {ms === null ? t("lens.refreshManual") : t("lens.refreshEvery", { seconds: ms / 1000 })}
              </option>
            ))}
          </select>
          <Button onClick={() => void refreshNow()} disabled={!running} className="shrink-0">
            {t("lens.refreshNow")}
          </Button>
        </div>
        <p className="text-xs text-zinc-500">
          {running
            ? t("lens.seen", { count: status?.last_seen ?? 0 })
            : t("lens.idleHint")}
        </p>
        {status?.last_error && <p className="text-xs text-amber-300">{status.last_error}</p>}
        {error && <ErrorNotice error={error} />}
      </header>

      <div className="min-h-0 flex-1 space-y-2 overflow-y-auto p-3">
        {lines.length === 0 ? (
          <p className="py-6 text-center text-xs text-zinc-500">{running ? t("lens.waiting") : t("lens.empty")}</p>
        ) : (
          <>
            <div className="flex justify-end">
              <button type="button" className="text-xs text-zinc-500 hover:text-zinc-300" onClick={clearLines}>
                {t("lens.clear")}
              </button>
            </div>
            {lines.map((line) => (
              <LineItem key={line.id} line={line} />
            ))}
          </>
        )}
        <div ref={listEnd} />
      </div>

      <ReplyBox />
    </aside>
  );
}
