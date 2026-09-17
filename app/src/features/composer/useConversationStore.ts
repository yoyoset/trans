import { create } from "zustand";

import { AppError, toAppError } from "../../api/client";
import { calibrateTranslation, translateText } from "../../api/translate";
import type { Direction, TranslationOutcome, Turn } from "../../api/types";

export interface Entry {
  id: string;
  direction: Direction;
  source: string;
  /** 游戏聊天能解析出发送者名字；网页气泡一般没有。 */
  sender?: string | null;
  local: TranslationOutcome | null;
  calibrated: TranslationOutcome | null;
  busy: "translating" | "calibrating" | null;
  error: AppError | null;
}

interface ConversationState {
  presetId: string | null;
  /** 已进入对话的消息（对方发来的、我已发送的），同时是在线校对的上下文。 */
  entries: Entry[];
  /** 正在编辑的回复。 */
  draft: Entry | null;
  setPresetId: (id: string | null) => void;
  addIncoming: (text: string) => Promise<void>;
  translateDraft: (text: string) => Promise<void>;
  calibrate: (id: string) => Promise<void>;
  commitDraft: () => void;
}

let nextId = 0;
const newEntry = (direction: Direction, source: string): Entry => ({
  id: `e${++nextId}`,
  direction,
  source,
  local: null,
  calibrated: null,
  busy: "translating",
  error: null,
});

/** 译文优先用在线校对结果，其次本地结果。 */
export const finalText = (entry: Entry) => entry.calibrated?.translation ?? entry.local?.translation ?? "";

/** 上下文统一用对方的语言：对方消息取原文，我的消息取发出去的译文。 */
const toTurn = (entry: Entry): Turn =>
  entry.direction === "incoming" ? { speaker: "them", text: entry.source } : { speaker: "me", text: finalText(entry) };

export const useConversationStore = create<ConversationState>((set, get) => {
  const patch = (id: string, change: Partial<Entry>) =>
    set((state) => ({
      entries: state.entries.map((e) => (e.id === id ? { ...e, ...change } : e)),
      draft: state.draft?.id === id ? { ...state.draft, ...change } : state.draft,
    }));

  const find = (id: string) => (get().draft?.id === id ? get().draft : get().entries.find((e) => e.id === id));

  return {
    presetId: null,
    entries: [],
    draft: null,

    setPresetId: (presetId) => set({ presetId }),

    addIncoming: async (text) => {
      const entry = newEntry("incoming", text.trim());
      if (!entry.source) return;
      set((state) => ({ entries: [...state.entries, entry] }));
      try {
        const local = await translateText({ direction: "incoming", text: entry.source, preset_id: get().presetId });
        patch(entry.id, { local, busy: null });
      } catch (error) {
        patch(entry.id, { error: toAppError(error), busy: null });
      }
    },

    translateDraft: async (text) => {
      const source = text.trim();
      if (!source) {
        set({ draft: null });
        return;
      }
      const entry = newEntry("outgoing", source);
      set({ draft: entry });
      try {
        const local = await translateText({ direction: "outgoing", text: source, preset_id: get().presetId });
        patch(entry.id, { local, busy: null }); // 用户在此期间继续输入时 draft.id 已变，结果自动丢弃
      } catch (error) {
        patch(entry.id, { error: toAppError(error), busy: null });
      }
    },

    calibrate: async (id) => {
      const entry = find(id);
      if (!entry) return;
      const context = get()
        .entries.filter((e) => e.id !== id)
        .map(toTurn)
        .filter((turn) => turn.text !== "");
      patch(id, { busy: "calibrating", error: null });
      try {
        const calibrated = await calibrateTranslation({
          direction: entry.direction,
          source: entry.source,
          draft: entry.local?.translation ?? "",
          context,
          preset_id: get().presetId,
        });
        patch(id, { calibrated, busy: null });
      } catch (error) {
        patch(id, { error: toAppError(error), busy: null });
      }
    },

    commitDraft: () => {
      const { draft } = get();
      if (!draft || finalText(draft) === "") return;
      set((state) => ({ entries: [...state.entries, { ...draft, busy: null }], draft: null }));
    },
  };
});
