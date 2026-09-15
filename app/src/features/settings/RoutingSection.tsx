import { useTranslation } from "react-i18next";

import type { EnginePreference } from "../../api/types";
import { Field } from "../../shared/ui/Field";
import { Section } from "../../shared/ui/Section";
import { NumberInput } from "./LocalEngineSection";
import { useSettingsStore } from "./useSettingsStore";

export function RoutingSection() {
  const { t } = useTranslation();
  const draft = useSettingsStore((s) => s.draft);
  const update = useSettingsStore((s) => s.update);
  if (!draft) return null;

  const preferenceSelect = (value: EnginePreference, onChange: (value: EnginePreference) => void) => (
    <select className="input" value={value} onChange={(e) => onChange(e.target.value as EnginePreference)}>
      <option value="local">{t("settings.routing.local")}</option>
      <option value="online">{t("settings.routing.online")}</option>
    </select>
  );

  return (
    <Section title={t("settings.routing.title")}>
      <div className="grid grid-cols-2 gap-3">
        <Field label={t("settings.routing.incoming")}>
          {preferenceSelect(draft.engines.routing.incoming, (v) => update((d) => void (d.engines.routing.incoming = v)))}
        </Field>
        <Field label={t("settings.routing.outgoing")}>
          {preferenceSelect(draft.engines.routing.outgoing, (v) => update((d) => void (d.engines.routing.outgoing = v)))}
        </Field>
        <Field label={t("settings.routing.userLang")}>
          <input
            className="input"
            value={draft.languages.user}
            onChange={(e) => update((d) => void (d.languages.user = e.target.value))}
          />
        </Field>
        <Field label={t("settings.routing.foreignLang")}>
          <input
            className="input"
            value={draft.languages.foreign}
            onChange={(e) => update((d) => void (d.languages.foreign = e.target.value))}
          />
        </Field>
      </div>
      <Field label={t("settings.routing.contextTurns")}>
        <NumberInput
          value={draft.engines.calibration.context_turns}
          min={0}
          max={20}
          onChange={(v) => update((d) => void (d.engines.calibration.context_turns = v))}
        />
      </Field>
    </Section>
  );
}
