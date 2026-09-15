import { create } from "zustand";

import { AppError, toAppError } from "../../api/client";
import { getSettings, saveSettings } from "../../api/settings";
import type { AppSettings } from "../../api/types";

interface SettingsState {
  saved: AppSettings | null;
  draft: AppSettings | null;
  saving: boolean;
  error: AppError | null;
  justSaved: boolean;
  load: () => Promise<void>;
  /** 在草稿副本上修改，不影响已保存的设置。 */
  update: (mutate: (draft: AppSettings) => void) => void;
  save: () => Promise<void>;
  discard: () => void;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  saved: null,
  draft: null,
  saving: false,
  error: null,
  justSaved: false,

  load: async () => {
    try {
      const settings = await getSettings();
      set({ saved: settings, draft: structuredClone(settings), error: null });
    } catch (error) {
      set({ error: toAppError(error) });
    }
  },

  update: (mutate) => {
    const { draft } = get();
    if (!draft) return;
    const next = structuredClone(draft);
    mutate(next);
    set({ draft: next, justSaved: false });
  },

  save: async () => {
    const { draft } = get();
    if (!draft) return;
    set({ saving: true, error: null });
    try {
      await saveSettings(draft);
      set({ saved: structuredClone(draft), justSaved: true });
    } catch (error) {
      set({ error: toAppError(error) });
    } finally {
      set({ saving: false });
    }
  },

  discard: () => {
    const { saved } = get();
    if (saved) set({ draft: structuredClone(saved), justSaved: false });
  },
}));

export const isDirty = (state: Pick<SettingsState, "saved" | "draft">) =>
  state.saved !== null && state.draft !== null && JSON.stringify(state.saved) !== JSON.stringify(state.draft);
