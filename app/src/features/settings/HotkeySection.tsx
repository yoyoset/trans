import { useTranslation } from "react-i18next";

import { Field } from "../../shared/ui/Field";
import { Section } from "../../shared/ui/Section";
import { useSettingsStore } from "./useSettingsStore";

/** 三个热键各自的设置项：字段名 → 说明用的翻译键。 */
const KEYS = ["toggle_lens", "click_through", "refresh"] as const;

/**
 * 全局热键。系统里同一个组合只能有一个主人，被别的软件占了就注册不上，
 * 所以这里必须能改——保存时后端会重新注册，冲突会直接报错。
 */
export function HotkeySection() {
  const { t } = useTranslation();
  const hotkeys = useSettingsStore((s) => s.draft?.hotkeys);
  const update = useSettingsStore((s) => s.update);
  if (!hotkeys) return null;

  return (
    <Section title={t("settings.hotkeys.title")}>
      <p className="text-xs text-zinc-400">{t("settings.hotkeys.hint")}</p>
      {KEYS.map((key) => (
        <Field key={key} label={t(`settings.hotkeys.${key}`)}>
          <input
            className="input font-mono text-xs"
            placeholder={t("settings.hotkeys.placeholder")}
            value={hotkeys[key]}
            onChange={(e) => update((d) => void (d.hotkeys[key] = e.target.value))}
          />
        </Field>
      ))}
    </Section>
  );
}
