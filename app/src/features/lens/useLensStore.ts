import { create } from "zustand";

import { getCaptureStatus, onCapturedLines, onCaptureStatus, startCapture, stopCapture } from "../../api/capture";
import { AppError, toAppError } from "../../api/client";
import { calibrateTranslation, listPresets, translateText } from "../../api/translate";
import type { CaptureStatus, PresetSummary, TranslationOutcome, Turn } from "../../api/types";

/** 面板里最多保留多少句，超出丢最早的，长时间识别也不会无限增长。 */
const MAX_LINES = 100;

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
  /** 加载场景列表并订阅识别推送；返回取消订阅函数。 */
  init: () => () => void;
  setPreset: (id: string) => void;
  start: (rect: LensRect) => Promise<void>;
  stop: () => Promise<void>;
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
      getCaptureStatus()
        .then((status) => !disposed && set({ status }))
        .catch((error) => console.error("capture_status failed", error));

      return () => {
        disposed = true;
        unlisten.forEach((fn) => fn());
      };
    },

    setPreset: (id) => set({ presetId: id }),

    start: async (rect) => {
      const preset = presetId();
      if (!preset) {
        set({ error: new AppError({ code: "no_preset", message: "请先选择场景" }) });
        return;
      }
      set({ busy: true, error: null });
      try {
        set({ status: await startCapture({ mode: "lens", ...rect }, preset) });
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
