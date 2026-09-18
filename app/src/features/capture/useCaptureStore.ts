import { create } from "zustand";

import { captureSupported, closeLens, getCaptureStatus, onCaptureStatus, openLens, stopCapture } from "../../api/capture";
import { AppError, toAppError } from "../../api/client";
import type { CaptureStatus } from "../../api/types";

/**
 * 主窗口只负责打开/关闭取景框，识别、翻译、回复都在取景框自己的面板里完成；
 * 但要显示识别是否还在后台进行——取景框关掉后识别并不会停，用户得能看到、能停掉。
 */
interface CaptureState {
  supported: boolean;
  status: CaptureStatus | null;
  busy: boolean;
  error: AppError | null;
  /** 读取当前状态并订阅后续变化；返回取消订阅函数。 */
  load: () => () => void;
  showLens: () => Promise<void>;
  hideLens: () => Promise<void>;
  stop: () => Promise<void>;
}

export const useCaptureStore = create<CaptureState>((set) => {
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

  return {
    supported: false,
    status: null,
    busy: false,
    error: null,

    load: () => {
      let disposed = false;
      let unlisten: (() => void) | undefined;
      void run(async () => set({ supported: await captureSupported() }));
      getCaptureStatus()
        .then((status) => !disposed && set({ status }))
        .catch((error) => console.error("capture_status failed", error));
      onCaptureStatus((status) => set({ status }))
        .then((fn) => (disposed ? fn() : (unlisten = fn)))
        .catch((error) => console.error("capture status subscription failed", error));
      return () => {
        disposed = true;
        unlisten?.();
      };
    },

    showLens: () => run(openLens),
    hideLens: () => run(closeLens),
    stop: () => run(async () => set({ status: await stopCapture() })),
  };
});
