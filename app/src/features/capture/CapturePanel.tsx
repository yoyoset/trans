import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "../../shared/ui/Button";
import { ErrorNotice } from "../../shared/ui/ErrorNotice";
import { Field } from "../../shared/ui/Field";
import { Section } from "../../shared/ui/Section";
import { useConversationStore } from "../composer/useConversationStore";
import { useCaptureStore } from "./useCaptureStore";

export function CapturePanel() {
  const { t } = useTranslation();
  const capture = useCaptureStore();
  const presetId = useConversationStore((s) => s.presetId);
  const running = capture.status?.running ?? null;

  useEffect(() => {
    void capture.refreshWindows();
    // 只在进入本页时刷新一次窗口列表，避免下拉框在选择过程中被刷掉。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (!capture.supported) {
    return <p className="text-zinc-400">{t("capture.unsupported")}</p>;
  }

  return (
    <div className="space-y-4">
      <Section
        title={t("capture.title")}
        actions={
          <Button onClick={() => void capture.refreshWindows()} disabled={capture.busy}>
            {t("capture.refresh")}
          </Button>
        }
      >
        <Field label={t("capture.window")} hint={t("capture.windowHint")}>
          <select
            className="input"
            value={capture.selectedWindow ?? ""}
            onChange={(e) => capture.selectWindow(e.target.value === "" ? null : Number(e.target.value))}
          >
            <option value="">{t("capture.pickWindow")}</option>
            {capture.windows.map((w) => (
              <option key={w.id} value={w.id}>
                {w.process_name} — {w.title}
              </option>
            ))}
          </select>
        </Field>

        <p className="text-xs text-zinc-500">
          {presetId ? t("capture.presetInUse", { preset: presetId }) : t("capture.presetMissing")}
        </p>

        <div className="flex flex-wrap gap-2">
          <Button onClick={() => void capture.runPreview()} disabled={capture.busy}>
            {t("capture.preview")}
          </Button>
          {running ? (
            <Button variant="danger" onClick={() => void capture.stop()} disabled={capture.busy}>
              {t("capture.stop")}
            </Button>
          ) : (
            <Button variant="primary" onClick={() => void capture.start()} disabled={capture.busy}>
              {t("capture.start")}
            </Button>
          )}
        </div>

        {capture.error && <ErrorNotice error={capture.error} />}
      </Section>

      {capture.status && (
        <Section title={t("capture.status")}>
          <p className="text-sm text-zinc-300">
            {running
              ? t("capture.runningOn", { window: running.window_title || running.window, preset: running.preset_name })
              : t("capture.idle")}
          </p>
          <p className="text-xs text-zinc-500">{t("capture.lastSeen", { count: capture.status.last_seen })}</p>
          {capture.status.last_error && (
            <p className="text-xs text-amber-300">{t("capture.lastError", { error: capture.status.last_error })}</p>
          )}
          {running && <p className="text-xs text-zinc-500">{t("capture.messagesGoToComposer")}</p>}
        </Section>
      )}

      {capture.preview && (
        <Section title={t("capture.previewTitle")}>
          <p className="text-xs text-zinc-500">
            {t("capture.previewRegion", {
              x: capture.preview.region.x,
              y: capture.preview.region.y,
              width: capture.preview.region.width,
              height: capture.preview.region.height,
            })}
          </p>
          {capture.preview.messages.length === 0 ? (
            <p className="text-sm text-amber-300">{t("capture.previewEmpty")}</p>
          ) : (
            <ul className="space-y-1 text-sm">
              {capture.preview.messages.map((message, i) => (
                <li key={i} className="rounded border border-zinc-800 bg-zinc-900 px-2 py-1">
                  <span className="mr-2 text-xs text-zinc-500">
                    {message.speaker === "them" ? t("composer.them") : t("composer.me")}
                    {message.sender ? ` · ${message.sender}` : ""}
                  </span>
                  {message.text}
                </li>
              ))}
            </ul>
          )}
        </Section>
      )}
    </div>
  );
}
