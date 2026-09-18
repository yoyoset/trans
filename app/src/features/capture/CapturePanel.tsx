import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "../../shared/ui/Button";
import { ErrorNotice } from "../../shared/ui/ErrorNotice";
import { Section } from "../../shared/ui/Section";
import { useCaptureStore } from "./useCaptureStore";

/** 主窗口里的入口：说明用法并打开取景框。 */
export function CapturePanel() {
  const { t } = useTranslation();
  const capture = useCaptureStore();
  const load = capture.load;

  useEffect(() => load(), [load]);

  const running = capture.status?.running;

  if (!capture.supported) {
    return <p className="text-zinc-400">{t("capture.unsupported")}</p>;
  }

  return (
    <Section title={t("capture.title")}>
      <p className="text-sm text-zinc-300">{t("capture.howTo")}</p>
      {capture.hotkeys.length > 0 && (
        <div className="rounded-md border border-zinc-800 bg-zinc-900/60 px-3 py-2 text-xs text-zinc-300">
          <p className="mb-1 text-zinc-400">{t("capture.hotkeys")}</p>
          <ul className="space-y-0.5">
            {capture.hotkeys.map(([keys, what]) => (
              <li key={keys}>
                <kbd className="rounded bg-zinc-800 px-1.5 py-0.5 font-mono text-[11px]">
                  {keys}
                </kbd>
                <span className="ml-2">{what}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
      {running && (
        <div className="flex items-center gap-3 rounded-md border border-emerald-900 bg-emerald-950/40 px-3 py-2 text-xs text-emerald-200">
          <span className="h-2 w-2 shrink-0 rounded-full bg-emerald-400" />
          <span className="flex-1">
            {t("capture.running", {
              target: running.label,
              count: capture.status?.last_seen ?? 0,
            })}
          </span>
          <Button
            variant="danger"
            onClick={() => void capture.stop()}
            disabled={capture.busy}
          >
            {t("capture.stop")}
          </Button>
        </div>
      )}
      <div className="flex flex-wrap gap-2">
        <Button
          variant="primary"
          onClick={() => void capture.showLens()}
          disabled={capture.busy}
        >
          {t("capture.openLens")}
        </Button>
        <Button onClick={() => void capture.hideLens()} disabled={capture.busy}>
          {t("capture.closeLens")}
        </Button>
      </div>
      {capture.error && <ErrorNotice error={capture.error} />}
    </Section>
  );
}
