import clsx from "clsx";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import type { TranslationOutcome } from "../../api/types";
import { Button } from "../../shared/ui/Button";
import { ErrorNotice } from "../../shared/ui/ErrorNotice";
import { IssueList } from "./IssueList";
import { type Entry, finalText, useConversationStore } from "./useConversationStore";

export function TranslationCard({ entry, actions }: { entry: Entry; actions?: React.ReactNode }) {
  const { t } = useTranslation();
  const calibrate = useConversationStore((s) => s.calibrate);
  const [copied, setCopied] = useState(false);
  const shown = entry.calibrated ?? entry.local;
  const incoming = entry.direction === "incoming";

  const copy = async () => {
    await navigator.clipboard.writeText(finalText(entry));
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <article
      className={clsx(
        "space-y-2 rounded-lg border p-3",
        incoming ? "mr-8 border-zinc-800 bg-zinc-900" : "ml-8 border-emerald-900/60 bg-emerald-950/30",
      )}
    >
      <p className="text-xs text-zinc-500">
        {incoming ? t("composer.them") : t("composer.me")} · {entry.source}
      </p>

      {entry.busy === "translating" && <p className="text-zinc-400">{t("composer.translating")}</p>}
      {shown && <p className="whitespace-pre-wrap text-base leading-relaxed text-zinc-50">{shown.translation}</p>}
      {entry.calibrated && entry.local && entry.local.translation !== entry.calibrated.translation && (
        <p className="text-xs text-zinc-500 line-through">{entry.local.translation}</p>
      )}
      {entry.calibrated?.notes.map((note, i) => (
        <p key={i} className="text-xs text-sky-300">
          {note}
        </p>
      ))}

      {shown && <IssueList issues={shown.issues} canCalibrate={!entry.calibrated} />}
      {entry.error && <ErrorNotice error={entry.error} />}

      <footer className="flex flex-wrap items-center justify-between gap-2">
        {shown ? <Meta outcome={shown} calibrated={entry.calibrated !== null} /> : <span />}
        <div className="flex gap-2">
          {shown && (
            <Button variant="ghost" onClick={() => void copy()}>
              {copied ? t("composer.copied") : t("composer.copy")}
            </Button>
          )}
          <Button disabled={entry.busy !== null || !entry.local} onClick={() => void calibrate(entry.id)}>
            {entry.busy === "calibrating" ? t("composer.calibrating") : t("composer.calibrate")}
          </Button>
          {actions}
        </div>
      </footer>
    </article>
  );
}

function Meta({ outcome, calibrated }: { outcome: TranslationOutcome; calibrated: boolean }) {
  const { t } = useTranslation();
  return (
    <span className="text-xs text-zinc-500" title={`${outcome.model} · ${outcome.prompt_version}`}>
      {calibrated ? t("composer.calibrated") : t(`engine.${outcome.engine}`)} ·{" "}
      {t("composer.elapsed", { ms: outcome.elapsed_ms })}
      {outcome.fell_back && <span className="ml-1 text-amber-400">· {t("composer.fellBack")}</span>}
    </span>
  );
}
