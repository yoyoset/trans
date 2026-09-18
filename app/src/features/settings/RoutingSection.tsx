import { useTranslation } from "react-i18next";

import { LANGUAGES } from "../../api/languages";

import type { EnginePreference } from "../../api/types";
import { Field } from "../../shared/ui/Field";
import { Section } from "../../shared/ui/Section";
import { NumberInput } from "./LocalEngineSection";
import { useSettingsStore } from "./useSettingsStore";

/** 语言下拉。用户手填过列表外的标签时把它作为一个选项保留，不会被下拉框吃掉。 */
function languageSelect(value: string, onChange: (tag: string) => void) {
  const options = LANGUAGES.some((lang) => lang.tag === value)
    ? LANGUAGES
    : [{ tag: value, label: value }, ...LANGUAGES];
  return (
    <select
      className="input"
      value={value}
      onChange={(e) => onChange(e.target.value)}
    >
      {options.map((lang) => (
        <option key={lang.tag} value={lang.tag}>
          {lang.label}
        </option>
      ))}
    </select>
  );
}

export function RoutingSection() {
  const { t } = useTranslation();
  const draft = useSettingsStore((s) => s.draft);
  const update = useSettingsStore((s) => s.update);
  if (!draft) return null;

  const preferenceSelect = (
    value: EnginePreference,
    onChange: (value: EnginePreference) => void,
  ) => (
    <select
      className="input"
      value={value}
      onChange={(e) => onChange(e.target.value as EnginePreference)}
    >
      <option value="local">{t("settings.routing.local")}</option>
      <option value="online">{t("settings.routing.online")}</option>
    </select>
  );

  return (
    <Section title={t("settings.routing.title")}>
      <div className="grid grid-cols-2 gap-3">
        <Field label={t("settings.routing.incoming")}>
          {preferenceSelect(draft.engines.routing.incoming, (v) =>
            update((d) => void (d.engines.routing.incoming = v)),
          )}
        </Field>
        <Field label={t("settings.routing.outgoing")}>
          {preferenceSelect(draft.engines.routing.outgoing, (v) =>
            update((d) => void (d.engines.routing.outgoing = v)),
          )}
        </Field>
        <Field label={t("settings.routing.userLang")}>
          {languageSelect(draft.languages.user, (tag) =>
            update((d) => void (d.languages.user = tag)),
          )}
        </Field>
        <Field
          label={t("settings.routing.foreignLang")}
          hint={t("settings.routing.langHint")}
        >
          {languageSelect(draft.languages.foreign, (tag) =>
            update((d) => void (d.languages.foreign = tag)),
          )}
        </Field>
      </div>
      <Field label={t("settings.routing.contextTurns")}>
        <NumberInput
          value={draft.engines.calibration.context_turns}
          min={0}
          max={20}
          onChange={(v) =>
            update((d) => void (d.engines.calibration.context_turns = v))
          }
        />
      </Field>
    </Section>
  );
}
