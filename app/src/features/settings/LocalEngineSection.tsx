import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { discoverLocalModels } from "../../api/engines";
import type { ModelFile } from "../../api/types";
import { Button } from "../../shared/ui/Button";
import { Field } from "../../shared/ui/Field";
import { Section } from "../../shared/ui/Section";
import { useSettingsStore } from "./useSettingsStore";

const CUSTOM = "__custom__";

export function LocalEngineSection() {
  const { t } = useTranslation();
  const local = useSettingsStore((s) => s.draft?.engines.local);
  const update = useSettingsStore((s) => s.update);
  const [models, setModels] = useState<ModelFile[]>([]);

  useEffect(() => {
    discoverLocalModels()
      .then(setModels)
      .catch((error) => console.error("model discovery failed", error));
  }, []);

  if (!local) return null;
  const known = models.some((m) => m.path === local.model_path);

  const browse = async (extensions: string[]) => {
    const picked = await open({ multiple: false, directory: false, filters: [{ name: extensions.join("/"), extensions }] });
    return typeof picked === "string" ? picked : null;
  };

  return (
    <Section title={t("settings.local.title")}>
      <Field label={t("settings.local.serverPath")} hint={t("settings.local.serverPathHint")}>
        <PathInput
          value={local.server_path}
          onChange={(path) => update((d) => void (d.engines.local.server_path = path))}
          onBrowse={() => browse(["exe"])}
        />
      </Field>

      <Field label={t("settings.local.model")} hint={t("settings.local.modelHint")}>
        <select
          className="input"
          value={known ? (local.model_path ?? "") : CUSTOM}
          onChange={(e) => {
            const value = e.target.value;
            if (value !== CUSTOM) update((d) => void (d.engines.local.model_path = value));
          }}
        >
          {models.map((m) => (
            <option key={m.path} value={m.path}>
              {m.file_name}（{(m.size_bytes / 1024 ** 3).toFixed(2)} GB）
            </option>
          ))}
          <option value={CUSTOM}>{t("settings.local.customModel")}</option>
        </select>
        {!known && (
          <PathInput
            value={local.model_path}
            onChange={(path) => update((d) => void (d.engines.local.model_path = path))}
            onBrowse={() => browse(["gguf"])}
          />
        )}
      </Field>

      <div className="grid grid-cols-3 gap-3">
        <Field label={t("settings.local.threads")}>
          <NumberInput value={local.threads} min={1} max={128} onChange={(v) => update((d) => void (d.engines.local.threads = v))} />
        </Field>
        <Field label={t("settings.local.port")}>
          <NumberInput value={local.port} min={1024} max={65535} onChange={(v) => update((d) => void (d.engines.local.port = v))} />
        </Field>
        <Field label={t("settings.local.contextSize")}>
          <NumberInput
            value={local.context_size}
            min={1024}
            max={32768}
            onChange={(v) => update((d) => void (d.engines.local.context_size = v))}
          />
        </Field>
      </div>
      <p className="text-xs text-zinc-500">{t("settings.local.threadsHint")}</p>
    </Section>
  );
}

function PathInput({
  value,
  onChange,
  onBrowse,
}: {
  value: string | null;
  onChange: (path: string | null) => void;
  onBrowse: () => Promise<string | null>;
}) {
  const { t } = useTranslation();
  return (
    <div className="mt-1 flex gap-2">
      <input
        className="input font-mono text-xs"
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value.trim() === "" ? null : e.target.value)}
      />
      <Button
        onClick={() =>
          void onBrowse().then((path) => {
            if (path) onChange(path);
          })
        }
      >
        {t("settings.local.browse")}
      </Button>
    </div>
  );
}

export function NumberInput({
  value,
  min,
  max,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}) {
  return (
    <input
      className="input"
      type="number"
      value={value}
      min={min}
      max={max}
      onChange={(e) => {
        const parsed = Number.parseInt(e.target.value, 10);
        if (Number.isFinite(parsed)) onChange(Math.min(max, Math.max(min, parsed)));
      }}
    />
  );
}
