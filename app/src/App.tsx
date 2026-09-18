import clsx from "clsx";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { takeStartupWarnings } from "./api/engines";
import { CapturePanel } from "./features/capture/CapturePanel";
import { ComposerPanel } from "./features/composer/ComposerPanel";
import { EngineStatusBar } from "./features/engines/EngineStatusBar";
import { SettingsPanel } from "./features/settings/SettingsPanel";
import { useSettingsStore } from "./features/settings/useSettingsStore";
import { ErrorBoundary } from "./shared/ui/ErrorBoundary";
import { OneTimeHint } from "./shared/ui/OneTimeHint";

type Tab = "composer" | "capture" | "settings";

export default function App() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>("composer");
  const [warnings, setWarnings] = useState<string[]>([]);
  const loadSettings = useSettingsStore((s) => s.load);

  useEffect(() => {
    void loadSettings();
    takeStartupWarnings()
      .then(setWarnings)
      .catch((error) => console.error("take_startup_warnings failed", error));
  }, [loadSettings]);

  return (
    <div className="flex h-full flex-col">
      <header className="space-y-2 border-b border-zinc-800 px-4 pb-2 pt-3">
        <div className="flex items-center justify-between">
          <h1 className="text-base font-semibold">{t("app.title")}</h1>
          <nav className="flex gap-1">
            {(["composer", "capture", "settings"] as const).map((name) => (
              <button
                key={name}
                onClick={() => setTab(name)}
                className={clsx(
                  "rounded-md px-3 py-1 text-sm",
                  tab === name ? "bg-zinc-800 text-zinc-50" : "text-zinc-400 hover:text-zinc-200",
                )}
              >
                {t(`app.tabs.${name}`)}
              </button>
            ))}
          </nav>
        </div>
        <EngineStatusBar />
        <OneTimeHint id="close-to-bubble">{t("app.closeToBubble")}</OneTimeHint>
        {warnings.map((warning) => (
          <p key={warning} className="rounded-md bg-amber-950/60 px-3 py-1.5 text-xs text-amber-200">
            {warning}
          </p>
        ))}
      </header>

      <main className="min-h-0 flex-1 overflow-y-auto p-4">
        <ErrorBoundary key={tab}>
          {tab === "composer" && <ComposerPanel />}
          {tab === "capture" && <CapturePanel />}
          {tab === "settings" && <SettingsPanel />}
        </ErrorBoundary>
      </main>
    </div>
  );
}
