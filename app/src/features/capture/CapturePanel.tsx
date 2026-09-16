import clsx from "clsx";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "../../shared/ui/Button";
import { ErrorNotice } from "../../shared/ui/ErrorNotice";
import { Field } from "../../shared/ui/Field";
import { Section } from "../../shared/ui/Section";
import { useConversationStore } from "../composer/useConversationStore";
import { useCaptureStore, type CaptureMode } from "./useCaptureStore";

export function CapturePanel() {
  const { t } = useTranslation();
  const capture = useCaptureStore();
  const presetId = useConversationStore((s) => s.presetId);
  const running = capture.status?.running ?? null;
  const refreshWindows = capture.refreshWindows;
  const mode = capture.mode;

  useEffect(() => {
    if (mode === "window") void refreshWindows();
  }, [mode, refreshWindows]);

  if (!capture.supported) {
    return <p className="text-zinc-400">{t("capture.unsupported")}</p>;
  }

  return (
    <div className="space-y-4">
      <Section title={t("capture.title")}>
        <div className="flex gap-1">
          {(["lens", "window"] as const).map((name) => (
            <ModeButton key={name} mode={name} active={mode === name} onClick={() => capture.setMode(name)} />
          ))}
        </div>

        <p className="text-xs text-zinc-500">
          {presetId ? t("capture.presetInUse", { preset: presetId }) : t("capture.presetMissing")}
        </p>

        {mode === "lens" ? (
          <>
            <p className="text-sm text-zinc-300">{t("capture.lensHowTo")}</p>
            <p className="text-xs text-amber-300">{t("capture.lensClickWarning")}</p>
            <div className="flex flex-wrap gap-2">
              <Button variant="primary" onClick={() => void capture.showLens()} disabled={capture.busy}>
                {t("capture.openLens")}
              </Button>
              <Button onClick={() => void capture.hideLens()} disabled={capture.busy}>
                {t("capture.closeLens")}
              </Button>
              {running && (
                <Button variant="danger" onClick={() => void capture.stop()} disabled={capture.busy}>
                  {t("capture.stop")}
                </Button>
              )}
            </div>
          </>
        ) : (
          <>
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
            <div className="flex flex-wrap gap-2">
              <Button onClick={() => void refreshWindows()} disabled={capture.busy}>
                {t("capture.refresh")}
              </Button>
              <Button onClick={() => void capture.previewWindow()} disabled={capture.busy}>
                {t("capture.preview")}
              </Button>
              {running ? (
                <Button variant="danger" onClick={() => void capture.stop()} disabled={capture.busy}>
                  {t("capture.stop")}
                </Button>
              ) : (
                <Button variant="primary" onClick={() => void capture.startWindow()} disabled={capture.busy}>
                  {t("capture.start")}
                </Button>
              )}
            </div>
          </>
        )}

        {capture.error && <ErrorNotice error={capture.error} />}
      </Section>

      {capture.status && (
        <Section title={t("capture.status")}>
          <p className="text-sm text-zinc-300">
            {running
              ? t("capture.runningOn", { target: running.label, preset: running.preset_name })
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

function ModeButton({ mode, active, onClick }: { mode: CaptureMode; active: boolean; onClick: () => void }) {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      onClick={onClick}
      className={clsx(
        "rounded-md px-3 py-1 text-sm",
        active ? "bg-zinc-800 text-zinc-50" : "text-zinc-400 hover:text-zinc-200",
      )}
    >
      {t(`capture.mode.${mode}`)}
    </button>
  );
}
