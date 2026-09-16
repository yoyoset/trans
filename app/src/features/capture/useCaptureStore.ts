import { create } from "zustand";

import {
  captureSupported,
  closeLens,
  getCaptureStatus,
  listCaptureWindows,
  onCapturedMessage,
  onCaptureStatus,
  openLens,
  previewCapture,
  startCapture,
  stopCapture,
  type CaptureSourceArgs,
} from "../../api/capture";
import { AppError, toAppError } from "../../api/client";
import type { CapturePreview, CaptureStatus, WindowInfo } from "../../api/types";
import { useConversationStore } from "../composer/useConversationStore";

/** 取景框：拖到哪识别哪。跟随窗口：区域按预设随窗口尺寸换算，窗口移动也不用重框。 */
export type CaptureMode = "lens" | "window";

interface CaptureState {
  supported: boolean;
  mode: CaptureMode;
  status: CaptureStatus | null;
  windows: WindowInfo[];
  selectedWindow: number | null;
  preview: CapturePreview | null;
  busy: boolean;
  error: AppError | null;
  /** 订阅后端推送；返回取消订阅函数。识别到的消息直接进入翻译列表。 */
  subscribe: () => () => void;
  setMode: (mode: CaptureMode) => void;
  refreshWindows: () => Promise<void>;
  selectWindow: (window: number | null) => void;
  showLens: () => Promise<void>;
  hideLens: () => Promise<void>;
  /** 跟随窗口模式的预览与启动；取景框模式在取景框窗口自己的工具条上操作。 */
  previewWindow: () => Promise<void>;
  startWindow: () => Promise<void>;
  stop: () => Promise<void>;
}

export const useCaptureStore = create<CaptureState>((set, get) => {
  const run = async (action: () => Promise<void>) => {
    set({ busy: true, error: null });
    try {
      await action();
    } catch (error) {
      set({ error: toAppError(error) });
    } finally {
      set({ busy: false });
    }
  };

  const windowSource = (): { source: CaptureSourceArgs; presetId: string } => {
    const window = get().selectedWindow;
    const presetId = useConversationStore.getState().presetId;
    if (window === null) throw new AppError({ code: "no_window", message: "请先选择要识别的窗口" });
    if (!presetId) throw new AppError({ code: "no_preset", message: "请先在翻译页选择术语场景" });
    return { source: { mode: "window", window }, presetId };
  };

  return {
    supported: false,
    mode: "lens",
    status: null,
    windows: [],
    selectedWindow: null,
    preview: null,
    busy: false,
    error: null,

    subscribe: () => {
      let disposed = false;
      const unlisten: Array<() => void> = [];
      const track = (pending: Promise<() => void>) =>
        pending
          .then((fn) => (disposed ? fn() : unlisten.push(fn)))
          .catch((error) => console.error("capture subscription failed", error));

      track(onCaptureStatus((status) => set({ status })));
      track(onCapturedMessage((message) => useConversationStore.getState().addCaptured(message)));
      captureSupported()
        .then((supported) => !disposed && set({ supported }))
        .catch((error) => console.error("capture_supported failed", error));
      getCaptureStatus()
        .then((status) => !disposed && set({ status }))
        .catch((error) => console.error("capture_status failed", error));

      return () => {
        disposed = true;
        unlisten.forEach((fn) => fn());
      };
    },

    setMode: (mode) => set({ mode, preview: null, error: null }),

    refreshWindows: () => run(async () => set({ windows: await listCaptureWindows() })),

    selectWindow: (selectedWindow) => set({ selectedWindow, preview: null }),

    showLens: () => run(openLens),

    hideLens: () => run(closeLens),

    previewWindow: () =>
      run(async () => {
        const { source, presetId } = windowSource();
        set({ preview: await previewCapture(source, presetId) });
      }),

    startWindow: () =>
      run(async () => {
        const { source, presetId } = windowSource();
        set({ status: await startCapture(source, presetId) });
      }),

    stop: () => run(async () => set({ status: await stopCapture() })),
  };
});
