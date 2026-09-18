import { create } from "zustand";

import {
  getCaptureStatus,
  listOcrLanguages,
  onCapturedLines,
  onCaptureStatus,
  refreshCapture,
  setCaptureInterval,
  startCapture,
  stopCapture,
} from "../../api/capture";
import { AppError, toAppError } from "../../api/client";
import { calibrateTranslation, listPresets, translateText } from "../../api/translate";
import type { CaptureStatus, PresetSummary, TranslationOutcome, Turn } from "../../api/types";

/** 面板里最多保留多少句，超出丢最早的，长时间识别也不会无限增长。 */
export const MAX_LINES = 100;

const LANG_STORAGE_KEY = "trans.lens.ocrLang";

/** 读上次选的识别语言；没选过返回 null（用设置里的"对方语言"）。 */
function loadLang(): string | null {
  try {
    return localStorage.getItem(LANG_STORAGE_KEY);
  } catch {
    return null;
  }
}

/** 刷新节奏选项（毫秒）；null 为手动。 */
export const REFRESH_OPTIONS: Array<number | null> = [null, 1000, 2000, 3000, 5000, 10000];
const REFRESH_STORAGE_KEY = "trans.lens.refreshMs";
const DEFAULT_REFRESH_MS = 2000;

/** 解析保存的刷新节奏（"manual" 或毫秒数）；不在选项里的值一律用默认值。 */
export function parseRefreshMs(raw: string | null): number | null {
  if (raw === null) return DEFAULT_REFRESH_MS;
  const value = raw === "manual" ? null : Number(raw);
  return REFRESH_OPTIONS.includes(value) ? value : DEFAULT_REFRESH_MS;
}

/** 读保存的刷新节奏；存储不可用时用默认值。 */
function loadRefreshMs(): number | null {
  try {
    return parseRefreshMs(localStorage.getItem(REFRESH_STORAGE_KEY));
  } catch {
    return DEFAULT_REFRESH_MS;
  }
}

function saveRefreshMs(value: number | null) {
  try {
    localStorage.setItem(REFRESH_STORAGE_KEY, value === null ? "manual" : String(value));
  } catch {
    // 只是偏好，存不下下次用默认值。
  }
}

export interface LensRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** 识别到的一句对方消息。翻译是用户点了才做的。 */
export interface Line {
  id: string;
  text: string;
  sender: string | null;
  translation: TranslationOutcome | null;
  calibrated: TranslationOutcome | null;
  busy: "translating" | "calibrating" | null;
  error: AppError | null;
}

export interface Reply {
  text: string;
  translation: TranslationOutcome | null;
  calibrated: TranslationOutcome | null;
  busy: "translating" | "calibrating" | null;
  error: AppError | null;
}

interface LensState {
  presets: PresetSummary[];
  presetId: string | null;
  status: CaptureStatus | null;
  lines: Line[];
  reply: Reply;
  busy: boolean;
  error: AppError | null;
  /** 刷新间隔（毫秒），null 为手动。 */
  refreshMs: number | null;
  /** 系统装了哪些 OCR 语言。 */
  languages: string[];
  /** 选定的识别语言；null 表示用设置里的"对方语言"。 */
  lang: string | null;
  /** 加载场景列表并订阅识别推送；返回取消订阅函数。 */
  init: () => () => void;
  setPreset: (id: string) => void;
  setLang: (lang: string | null) => void;
  start: (rect: LensRect) => Promise<void>;
  stop: () => Promise<void>;
  setRefreshMs: (value: number | null) => void;
  refreshNow: () => Promise<void>;
  clearLines: () => void;
  translateLine: (id: string) => Promise<void>;
  calibrateLine: (id: string) => Promise<void>;
  translateReply: (text: string) => Promise<void>;
  calibrateReply: () => Promise<void>;
}

const emptyReply: Reply = { text: "", translation: null, calibrated: null, busy: null, error: null };

let nextId = 0;

export const useLensStore = create<LensState>((set, get) => {
  const patchLine = (id: string, change: Partial<Line>) =>
    set((state) => ({ lines: state.lines.map((line) => (line.id === id ? { ...line, ...change } : line)) }));

  const patchReply = (text: string, change: Partial<Reply>) =>
    set((state) => (state.reply.text === text ? { reply: { ...state.reply, ...change } } : state));

  /** 在线校对的上下文：面板里已识别的对方消息。 */
  const context = (excludeId?: string): Turn[] =>
    get()
      .lines.filter((line) => line.id !== excludeId)
      .slice(-20)
      .map((line) => ({ speaker: "them", text: line.text }));

  const presetId = () => get().presetId;

  return {
    presets: [],
    presetId: null,
    status: null,
    lines: [],
    reply: emptyReply,
    busy: false,
    error: null,
    refreshMs: loadRefreshMs(),
    languages: [],
    lang: loadLang(),

    init: () => {
      let disposed = false;
      const unlisten: Array<() => void> = [];
      const track = (pending: Promise<() => void>) =>
        pending
          .then((fn) => (disposed ? fn() : unlisten.push(fn)))
          .catch((error) => console.error("lens subscription failed", error));

      track(onCaptureStatus((status) => set({ status })));
      track(
        onCapturedLines((captured) =>
          set((state) => ({
            lines: [
              ...state.lines,
              ...captured.map((message) => ({
                id: `l${++nextId}`,
                text: message.text,
                sender: message.sender,
                translation: null,
                calibrated: null,
                busy: null,
                error: null,
              })),
            ].slice(-MAX_LINES),
          })),
        ),
      );
      listPresets()
        .then((presets) => !disposed && set((state) => ({ presets, presetId: state.presetId ?? presets[0]?.id ?? null })))
        .catch((error) => set({ error: toAppError(error) }));
      listOcrLanguages()
        .then((languages) => !disposed && set({ languages }))
        .catch((error) => console.error("list_ocr_languages failed", error));
      setCaptureInterval(get().refreshMs).catch((error) => set({ error: toAppError(error) }));
      getCaptureStatus()
        .then((status) => !disposed && set({ status }))
        .catch((error) => console.error("capture_status failed", error));

      return () => {
        disposed = true;
        unlisten.forEach((fn) => fn());
      };
    },

    setPreset: (id) => set({ presetId: id }),

    setLang: (lang) => {
      set({ lang });
      try {
        if (lang === null) localStorage.removeItem(LANG_STORAGE_KEY);
        else localStorage.setItem(LANG_STORAGE_KEY, lang);
      } catch {
        // 只是偏好，存不下就下次重新选。
      }
    },

    start: async (rect) => {
      const preset = presetId();
      if (!preset) {
        set({ error: new AppError({ code: "no_preset", message: "请先选择场景" }) });
        return;
      }
      set({ busy: true, error: null });
      try {
        set({ status: await startCapture({ mode: "lens", ...rect }, preset, get().lang) });
      } catch (error) {
        set({ error: toAppError(error) });
      } finally {
        set({ busy: false });
      }
    },

    stop: async () => {
      set({ busy: true, error: null });
      try {
        set({ status: await stopCapture() });
      } catch (error) {
        set({ error: toAppError(error) });
      } finally {
        set({ busy: false });
      }
    },

    setRefreshMs: (value) => {
      set({ refreshMs: value });
      saveRefreshMs(value);
      setCaptureInterval(value).catch((error) => set({ error: toAppError(error) }));
    },

    refreshNow: async () => {
      set({ error: null });
      try {
        await refreshCapture();
      } catch (error) {
        set({ error: toAppError(error) });
      }
    },

    clearLines: () => set({ lines: [] }),

    translateLine: async (id) => {
      const line = get().lines.find((l) => l.id === id);
      if (!line) return;
      patchLine(id, { busy: "translating", error: null });
      try {
        const translation = await translateText({ direction: "incoming", text: line.text, preset_id: presetId() });
        patchLine(id, { translation, busy: null });
      } catch (error) {
        patchLine(id, { error: toAppError(error), busy: null });
      }
    },

    calibrateLine: async (id) => {
      const line = get().lines.find((l) => l.id === id);
      if (!line) return;
      patchLine(id, { busy: "calibrating", error: null });
      try {
        const calibrated = await calibrateTranslation({
          direction: "incoming",
          source: line.text,
          draft: line.translation?.translation ?? "",
          context: context(id),
          preset_id: presetId(),
        });
        patchLine(id, { calibrated, busy: null });
      } catch (error) {
        patchLine(id, { error: toAppError(error), busy: null });
      }
    },

    translateReply: async (text) => {
      const source = text.trim();
      if (!source) {
        set({ reply: emptyReply });
        return;
      }
      set({ reply: { ...emptyReply, text: source, busy: "translating" } });
      try {
        const translation = await translateText({ direction: "outgoing", text: source, preset_id: presetId() });
        patchReply(source, { translation, busy: null }); // 用户已经改了输入时，旧结果直接丢弃
      } catch (error) {
        patchReply(source, { error: toAppError(error), busy: null });
      }
    },

    calibrateReply: async () => {
      const { reply } = get();
      if (!reply.text) return;
      patchReply(reply.text, { busy: "calibrating", error: null });
      try {
        const calibrated = await calibrateTranslation({
          direction: "outgoing",
          source: reply.text,
          draft: reply.translation?.translation ?? "",
          context: context(),
          preset_id: presetId(),
        });
        patchReply(reply.text, { calibrated, busy: null });
      } catch (error) {
        patchReply(reply.text, { error: toAppError(error), busy: null });
      }
    },
  };
});
