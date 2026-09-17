import { create } from "zustand";

import { captureSupported, closeLens, openLens } from "../../api/capture";
import { AppError, toAppError } from "../../api/client";

/** 主窗口只负责打开/关闭取景框；识别、翻译、回复都在取景框自己的面板里完成。 */
interface CaptureState {
  supported: boolean;
  busy: boolean;
  error: AppError | null;
  load: () => Promise<void>;
  showLens: () => Promise<void>;
  hideLens: () => Promise<void>;
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
    busy: false,
    error: null,
    load: () => run(async () => set({ supported: await captureSupported() })),
    showLens: () => run(openLens),
    hideLens: () => run(closeLens),
  };
});
