import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { AppError, toAppError } from "../../api/client";
import { clearOnlineApiKey, hasOnlineApiKey, setOnlineApiKey } from "../../api/settings";
import { Button } from "../../shared/ui/Button";
import { ErrorNotice } from "../../shared/ui/ErrorNotice";
import { Field } from "../../shared/ui/Field";
import { Section } from "../../shared/ui/Section";
import { useSettingsStore } from "./useSettingsStore";

const EFFORTS = ["", "low", "medium", "high"] as const;

export function OnlineEngineSection() {
  const { t } = useTranslation();
  const online = useSettingsStore((s) => s.draft?.engines.online);
  const update = useSettingsStore((s) => s.update);
  const [hasKey, setHasKey] = useState<boolean | null>(null);
  const [keyInput, setKeyInput] = useState("");
  const [error, setError] = useState<AppError | null>(null);

  const refreshKey = useCallback(() => {
    hasOnlineApiKey()
      .then(setHasKey)
      .catch((e) => setError(toAppError(e)));
  }, []);
  useEffect(refreshKey, [refreshKey]);

  if (!online) return null;

  const run = (action: () => Promise<void>) =>
    void action()
      .then(() => {
        setError(null);
        refreshKey();
      })
      .catch((e) => setError(toAppError(e)));

  return (
    <Section
      title={t("settings.online.title")}
      actions={
        <label className="flex items-center gap-2 text-xs text-zinc-300">
          <input
            type="checkbox"
            checked={online.enabled}
            onChange={(e) => update((d) => void (d.engines.online.enabled = e.target.checked))}
          />
          {t("settings.online.enabled")}
        </label>
      }
    >
      <div className="grid grid-cols-2 gap-3">
        <Field label={t("settings.online.displayName")}>
          <input
            className="input"
            value={online.display_name}
            onChange={(e) => update((d) => void (d.engines.online.display_name = e.target.value))}
          />
        </Field>
        <Field label={t("settings.online.model")}>
          <input
            className="input font-mono text-xs"
            value={online.model}
            onChange={(e) => update((d) => void (d.engines.online.model = e.target.value))}
          />
        </Field>
      </div>
      <Field label={t("settings.online.baseUrl")}>
        <input
          className="input font-mono text-xs"
          value={online.base_url}
          onChange={(e) => update((d) => void (d.engines.online.base_url = e.target.value))}
        />
      </Field>
      <Field label={t("settings.online.reasoningEffort")}>
        <select
          className="input"
          value={online.reasoning_effort ?? ""}
          onChange={(e) => update((d) => void (d.engines.online.reasoning_effort = e.target.value || null))}
        >
          {EFFORTS.map((effort) => (
            <option key={effort} value={effort}>
              {effort || t("settings.online.reasoningDefault")}
            </option>
          ))}
        </select>
      </Field>

      <Field
        label={`${t("settings.online.apiKey")}：${hasKey ? t("settings.online.apiKeySaved") : t("settings.online.apiKeyMissing")}`}
        hint={t("settings.online.keyHint")}
      >
        <div className="flex gap-2">
          <input
            className="input font-mono text-xs"
            type="password"
            autoComplete="off"
            placeholder={t("settings.online.apiKeyPlaceholder")}
            value={keyInput}
            onChange={(e) => setKeyInput(e.target.value)}
          />
          <Button
            variant="primary"
            disabled={keyInput.trim() === ""}
            onClick={() => run(() => setOnlineApiKey(keyInput).then(() => setKeyInput("")))}
          >
            {t("settings.online.saveKey")}
          </Button>
          <Button variant="ghost" disabled={!hasKey} onClick={() => run(clearOnlineApiKey)}>
            {t("settings.online.clearKey")}
          </Button>
        </div>
      </Field>
      {error && <ErrorNotice error={error} />}
    </Section>
  );
}
