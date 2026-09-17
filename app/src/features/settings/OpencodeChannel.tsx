import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";

import { Button } from "../../shared/ui/Button";
import { Field } from "../../shared/ui/Field";
import { useSettingsStore } from "./useSettingsStore";

const VARIANTS = ["", "low", "medium", "high"] as const;

/** 在线引擎接入方式二：通过本机 opencode 使用它已登录的订阅。 */
export function OpencodeChannel() {
  const { t } = useTranslation();
  const opencode = useSettingsStore((s) => s.draft?.engines.online.opencode);
  const update = useSettingsStore((s) => s.update);
  if (!opencode) return null;

  const browse = async () => {
    const picked = await open({ multiple: false, directory: false, filters: [{ name: "opencode", extensions: ["exe"] }] });
    if (typeof picked === "string") update((d) => void (d.engines.online.opencode.program = picked));
  };

  return (
    <>
      <p className="text-xs text-zinc-400">{t("settings.online.opencodeHint")}</p>
      <div className="grid grid-cols-2 gap-3">
        <Field label={t("settings.online.opencodeModel")} hint={t("settings.online.opencodeModelHint")}>
          <input
            className="input font-mono text-xs"
            value={opencode.model}
            onChange={(e) => update((d) => void (d.engines.online.opencode.model = e.target.value))}
          />
        </Field>
        <Field label={t("settings.online.opencodeVariant")}>
          <select
            className="input"
            value={opencode.variant ?? ""}
            onChange={(e) => update((d) => void (d.engines.online.opencode.variant = e.target.value || null))}
          >
            {VARIANTS.map((variant) => (
              <option key={variant} value={variant}>
                {variant || t("settings.online.reasoningDefault")}
              </option>
            ))}
          </select>
        </Field>
      </div>
      <Field label={t("settings.online.opencodeProgram")} hint={t("settings.online.opencodeProgramHint")}>
        <div className="flex gap-2">
          <input
            className="input font-mono text-xs"
            placeholder={t("settings.online.opencodeProgramAuto")}
            value={opencode.program ?? ""}
            onChange={(e) => update((d) => void (d.engines.online.opencode.program = e.target.value.trim() || null))}
          />
          <Button onClick={() => void browse()}>{t("settings.online.browse")}</Button>
        </div>
      </Field>
    </>
  );
}
