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

/** 引擎状态：一行徽标看当前状态；失败时把完整原因摊开，而不是只留一个截断的提示气泡。 */
export function EngineStatusBar() {
  const { t } = useTranslation();
  const status = useEngineStatus((s) => s.status);
  const subscribe = useEngineStatus((s) => s.subscribe);
  useEffect(() => subscribe(), [subscribe]);

  if (!status) return null;
  const failures = (["local", "online"] as const)
    .map((engine) => ({ engine, state: status[engine] }))
    .filter((entry) => entry.state.state === "failed");

  return (
    <div className="space-y-1.5">
      <div className="flex flex-wrap items-center gap-3 text-xs text-zinc-400">
        <Badge label={t("engine.local")} state={status.local} />
        <Badge label={t("engine.online")} state={status.online} />
        {failures.length > 0 && (
          <button className="text-emerald-400 hover:underline" onClick={() => void restartEngines()}>
            {t("engine.restart")}
          </button>
        )}
      </div>
      {failures.map(({ engine, state }) => (
        <Failure key={engine} label={t(`engine.${engine}`)} message={state.state === "failed" ? state.message : ""} />
      ))}
    </div>
  );
}

/**
 * 失败原因通常是多行的（例如 llama-server 的日志末尾）：第一行直接显示，其余折叠起来，
 * 免得把界面挤满，又不至于让用户看不到原因。
 */
function Failure({ label, message }: { label: string; message: string }) {
  const { t } = useTranslation();
  const [summary, ...rest] = message.split("\n");
  return (
    <div className="rounded-md border border-rose-900 bg-rose-950/40 px-3 py-1.5 text-xs text-rose-200">
      <p>
        <span className="font-medium">{label}</span>：{summary}
      </p>
      {rest.length > 0 && (
        <details className="mt-1">
          <summary className="cursor-pointer select-none text-rose-300/80">{t("engine.showDetail")}</summary>
          <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-all font-mono text-[11px]">
            {rest.join("\n")}
          </pre>
        </details>
      )}
    </div>
  );
}

function Badge({ label, state }: { label: string; state: EngineState }) {
  const { t } = useTranslation();
  // 失败原因在下面单独显示，徽标里只留状态词，避免同一句话出现两次。
  const detail = state.state === "ready" ? state.model : undefined;
  return (
    <span className="inline-flex max-w-[16rem] items-center gap-1.5" title={detail}>
      <span className={clsx("h-2 w-2 shrink-0 rounded-full", DOT[state.state])} />
      <span className="shrink-0">{label}</span>
      <span className="truncate text-zinc-500">{detail ?? t(`engine.state.${state.state}`)}</span>
    </span>
  );
}
