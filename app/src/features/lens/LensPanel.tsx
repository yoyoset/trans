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
  const store = useLensStore();
  const listEnd = useRef<HTMLDivElement>(null);
  const running = store.status?.running != null;

  // 必须用花括号：scrollIntoView 返回值不能交给 React 当清理函数（首次拉起黑屏就是这么来的）。
  useEffect(() => {
    listEnd.current?.scrollIntoView({ behavior: "smooth" });
  }, [store.lines.length]);

  const start = async () => {
    const rect = await captureRect();
    if (rect) await store.start(rect);
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
            value={store.presetId ?? ""}
            onChange={(e) => store.setPreset(e.target.value)}
            disabled={running}
            title={t("lens.presetHint")}
          >
            {store.presets.map((preset) => (
              <option key={preset.id} value={preset.id}>
                {preset.name}
              </option>
            ))}
          </select>
          {running ? (
            <Button variant="danger" onClick={() => void store.stop()} disabled={store.busy} className="shrink-0">
              {t("lens.stop")}
            </Button>
          ) : (
            <Button variant="primary" onClick={() => void start()} disabled={store.busy} className="shrink-0">
              {t("lens.start")}
            </Button>
          )}
        </div>
        <div className="flex items-center gap-2 text-xs text-zinc-400">
          <span className="shrink-0">{t("lens.refresh")}</span>
          <select
            className="input py-1 text-xs"
            value={store.refreshMs ?? "manual"}
            onChange={(e) => store.setRefreshMs(e.target.value === "manual" ? null : Number(e.target.value))}
            title={t("lens.refreshHint")}
          >
            {REFRESH_OPTIONS.map((ms) => (
              <option key={ms ?? "manual"} value={ms ?? "manual"}>
                {ms === null ? t("lens.refreshManual") : t("lens.refreshEvery", { seconds: ms / 1000 })}
              </option>
            ))}
          </select>
          <Button onClick={() => void store.refreshNow()} disabled={!running} className="shrink-0">
            {t("lens.refreshNow")}
          </Button>
        </div>
        <p className="text-xs text-zinc-500">
          {running
            ? t("lens.seen", { count: store.status?.last_seen ?? 0 })
            : t("lens.idleHint")}
        </p>
        {store.status?.last_error && <p className="text-xs text-amber-300">{store.status.last_error}</p>}
        {store.error && <ErrorNotice error={store.error} />}
      </header>

      <div className="min-h-0 flex-1 space-y-2 overflow-y-auto p-3">
        {store.lines.length === 0 ? (
          <p className="py-6 text-center text-xs text-zinc-500">{running ? t("lens.waiting") : t("lens.empty")}</p>
        ) : (
          <>
            <div className="flex justify-end">
              <button type="button" className="text-xs text-zinc-500 hover:text-zinc-300" onClick={store.clearLines}>
                {t("lens.clear")}
              </button>
            </div>
            {store.lines.map((line) => (
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
