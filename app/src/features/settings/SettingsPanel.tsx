import { useTranslation } from "react-i18next";

import { Button } from "../../shared/ui/Button";
import { ErrorNotice } from "../../shared/ui/ErrorNotice";
import { LocalEngineSection } from "./LocalEngineSection";
import { HotkeySection } from "./HotkeySection";
import { OnlineEngineSection } from "./OnlineEngineSection";
import { RoutingSection } from "./RoutingSection";
import { isDirty, useSettingsStore } from "./useSettingsStore";

export function SettingsPanel() {
  const { t } = useTranslation();
  const store = useSettingsStore();
  const dirty = isDirty(store);

  if (!store.draft) return store.error ? <ErrorNotice error={store.error} /> : null;

  return (
    <div className="space-y-4 pb-20">
      <LocalEngineSection />
      <OnlineEngineSection />
      <HotkeySection />
      <RoutingSection />

      <footer className="fixed inset-x-0 bottom-0 flex items-center justify-between gap-3 border-t border-zinc-800 bg-zinc-950/95 px-4 py-3">
        <span className="text-xs text-zinc-400">
          {dirty ? t("settings.unsaved") : store.justSaved ? t("settings.saved") : ""}
        </span>
        <div className="flex gap-2">
          <Button variant="ghost" disabled={!dirty || store.saving} onClick={store.discard}>
            {t("settings.discard")}
          </Button>
          <Button variant="primary" disabled={!dirty || store.saving} onClick={() => void store.save()}>
            {store.saving ? t("settings.saving") : t("settings.save")}
          </Button>
        </div>
      </footer>
      {store.error && <ErrorNotice error={store.error} />}
    </div>
  );
}
