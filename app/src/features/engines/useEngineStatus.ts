import { create } from "zustand";

import { getEngineStatus, onEngineStatus } from "../../api/engines";
import type { EngineStatus } from "../../api/types";

interface EngineStatusState {
  status: EngineStatus | null;
  /** 返回取消订阅函数，供 useEffect 清理。 */
  subscribe: () => () => void;
}

export const useEngineStatus = create<EngineStatusState>((set) => ({
  status: null,
  subscribe: () => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    onEngineStatus((status) => set({ status }))
      .then((fn) => (disposed ? fn() : (unlisten = fn)))
      .catch((error) => console.error("engine-status subscription failed", error));
    getEngineStatus()
      .then((status) => !disposed && set({ status }))
      .catch((error) => console.error("engine_status failed", error));
    return () => {
      disposed = true;
      unlisten?.();
    };
  },
}));
