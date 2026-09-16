import { create } from "zustand";

import {
  captureSupported,
  getCaptureStatus,
  listCaptureWindows,
  onCapturedMessage,
  onCaptureStatus,
  previewCapture,
  startCapture,
  stopCapture,
} from "../../api/capture";
import { AppError, toAppError } from "../../api/client";
import type { CapturePreview, CaptureStatus, WindowInfo } from "../../api/types";
import { useConversationStore } from "../composer/useConversationStore";

interface CaptureState {
  supported: boolean;
  status: CaptureStatus | null;
  windows: WindowInfo[];
  selectedWindow: number | null;
  preview: CapturePreview | null;
  busy: boolean;
  error: AppError | null;
  /** 订阅后端推送；返回取消订阅函数。识别到的消息直接进入翻译列表。 */
  subscribe: () => () => void;
  refreshWindows: () => Promise<void>;
  selectWindow: (window: number | null) => void;
  runPreview: () => Promise<void>;
  start: () => Promise<void>;
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

  const target = () => {
    const window = get().selectedWindow;
    const presetId = useConversationStore.getState().presetId;
    if (window === null) throw new AppError({ code: "no_window", message: "请先选择要识别的窗口" });
    if (!presetId) throw new AppError({ code: "no_preset", message: "请先在翻译页选择术语场景" });
    return { window, presetId };
  };

  return {
    supported: false,
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

    refreshWindows: () => run(async () => set({ windows: await listCaptureWindows() })),

    selectWindow: (selectedWindow) => set({ selectedWindow, preview: null }),

    runPreview: () =>
      run(async () => {
        const { window, presetId } = target();
        set({ preview: await previewCapture(window, presetId) });
      }),

    start: () =>
      run(async () => {
        const { window, presetId } = target();
        set({ status: await startCapture(window, presetId) });
      }),

    stop: () => run(async () => set({ status: await stopCapture() })),
  };
});
