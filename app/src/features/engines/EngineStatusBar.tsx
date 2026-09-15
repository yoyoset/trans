import clsx from "clsx";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { restartEngines } from "../../api/engines";
import type { EngineState } from "../../api/types";
import { useEngineStatus } from "./useEngineStatus";

const DOT: Record<EngineState["state"], string> = {
  not_configured: "bg-zinc-500",
  starting: "bg-amber-400 animate-pulse",
  ready: "bg-emerald-400",
  failed: "bg-rose-500",
};

export function EngineStatusBar() {
  const { t } = useTranslation();
  const status = useEngineStatus((s) => s.status);
  const subscribe = useEngineStatus((s) => s.subscribe);
  useEffect(() => subscribe(), [subscribe]);

  if (!status) return null;
  return (
    <div className="flex flex-wrap items-center gap-3 text-xs text-zinc-400">
      <Badge label={t("engine.local")} state={status.local} />
      <Badge label={t("engine.online")} state={status.online} />
      {(status.local.state === "failed" || status.online.state === "failed") && (
        <button className="text-emerald-400 hover:underline" onClick={() => void restartEngines()}>
          {t("engine.restart")}
        </button>
      )}
    </div>
  );
}

function Badge({ label, state }: { label: string; state: EngineState }) {
  const { t } = useTranslation();
  const detail = state.state === "ready" ? state.model : state.state === "failed" ? state.message : undefined;
  return (
    <span className="inline-flex max-w-[16rem] items-center gap-1.5" title={detail}>
      <span className={clsx("h-2 w-2 shrink-0 rounded-full", DOT[state.state])} />
      <span className="shrink-0">{label}</span>
      <span className="truncate text-zinc-500">{detail ?? t(`engine.state.${state.state}`)}</span>
    </span>
  );
}
