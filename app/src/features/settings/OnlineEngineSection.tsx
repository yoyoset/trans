import { useTranslation } from "react-i18next";

import type { OnlineChannel } from "../../api/types";
import { Section } from "../../shared/ui/Section";
import { ApiKeyChannel } from "./ApiKeyChannel";
import { OpencodeChannel } from "./OpencodeChannel";
import { useSettingsStore } from "./useSettingsStore";

const CHANNELS: Array<{ value: OnlineChannel; label: string }> = [
  { value: "opencode", label: "settings.online.channelOpencode" },
  { value: "api_key", label: "settings.online.channelApiKey" },
];

/** 在线引擎：启用开关 + 接入方式（opencode 订阅 / API Key），各方式的字段在各自组件里。 */
export function OnlineEngineSection() {
  const { t } = useTranslation();
  const online = useSettingsStore((s) => s.draft?.engines.online);
  const update = useSettingsStore((s) => s.update);
  if (!online) return null;

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
      <fieldset className="flex flex-wrap items-center gap-4 text-sm">
        <legend className="mb-1 text-xs text-zinc-400">{t("settings.online.channel")}</legend>
        {CHANNELS.map((channel) => (
          <label key={channel.value} className="flex items-center gap-1.5">
            <input
              type="radio"
              name="online-channel"
              checked={online.channel === channel.value}
              onChange={() => update((d) => void (d.engines.online.channel = channel.value))}
            />
            {t(channel.label)}
          </label>
        ))}
      </fieldset>
      {online.channel === "opencode" ? <OpencodeChannel /> : <ApiKeyChannel />}
    </Section>
  );
}
