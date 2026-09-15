import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { listPresets } from "../../api/translate";
import type { PresetSummary } from "../../api/types";
import { Button } from "../../shared/ui/Button";
import { TranslationCard } from "./TranslationCard";
import { useConversationStore } from "./useConversationStore";

/** 停止输入多久后自动翻译草稿。 */
const DRAFT_DEBOUNCE_MS = 600;

export function ComposerPanel() {
  const { t } = useTranslation();
  const { entries, draft, presetId, setPresetId, addIncoming, translateDraft, commitDraft } = useConversationStore();
  const [presets, setPresets] = useState<PresetSummary[]>([]);
  const [incoming, setIncoming] = useState("");
  const [draftText, setDraftText] = useState("");
  const listEnd = useRef<HTMLDivElement>(null);

  useEffect(() => {
    listPresets()
      .then(setPresets)
      .catch((error) => console.error("list_presets failed", error));
  }, []);

  useEffect(() => {
    const timer = setTimeout(() => void translateDraft(draftText), DRAFT_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [draftText, translateDraft]);

  // 必须用花括号：WebView2 的 scrollIntoView 返回 Promise，直接作为 effect 返回值会让 React 当成清理函数调用而崩溃。
  useEffect(() => {
    listEnd.current?.scrollIntoView({ behavior: "smooth" });
  }, [entries.length]);

  const submitIncoming = () => {
    void addIncoming(incoming);
    setIncoming("");
  };

  return (
    <div className="flex h-full flex-col gap-3">
      <label className="flex items-center gap-2 text-xs text-zinc-400">
        {t("composer.preset")}
        <select className="input w-auto" value={presetId ?? ""} onChange={(e) => setPresetId(e.target.value || null)}>
          <option value="">{t("composer.noPreset")}</option>
          {presets.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </label>

      <div className="min-h-0 flex-1 space-y-3 overflow-y-auto pr-1">
        {entries.length === 0 && <p className="py-8 text-center text-zinc-500">{t("composer.empty")}</p>}
        {entries.map((entry) => (
          <TranslationCard key={entry.id} entry={entry} />
        ))}
        <div ref={listEnd} />
      </div>

      <div className="space-y-1">
        <span className="text-xs font-medium text-zinc-400">{t("composer.incomingLabel")}</span>
        <div className="flex gap-2">
          <input
            className="input"
            value={incoming}
            placeholder={t("composer.incomingPlaceholder")}
            onChange={(e) => setIncoming(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && !e.nativeEvent.isComposing && submitIncoming()}
          />
          <Button disabled={incoming.trim() === ""} onClick={submitIncoming}>
            {t("composer.translate")}
          </Button>
        </div>
      </div>

      <div className="space-y-2">
        <span className="text-xs font-medium text-zinc-400">{t("composer.draftLabel")}</span>
        <textarea
          className="input h-20 resize-none"
          value={draftText}
          placeholder={t("composer.draftPlaceholder")}
          onChange={(e) => setDraftText(e.target.value)}
        />
        {draft && (
          <TranslationCard
            entry={draft}
            actions={
              <Button
                variant="primary"
                disabled={draft.busy !== null || !draft.local}
                onClick={() => {
                  commitDraft();
                  setDraftText("");
                }}
              >
                {t("composer.markSent")}
              </Button>
            }
          />
        )}
      </div>
    </div>
  );
}
