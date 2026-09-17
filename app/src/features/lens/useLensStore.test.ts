import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ParsedMessage, TranslationOutcome } from "../../api/types";

// store 只通过 api 模块和后端打交道：替换掉这些模块，就能不启动 Tauri 测试状态逻辑。
const api = vi.hoisted(() => ({
  translateText: vi.fn(),
  calibrateTranslation: vi.fn(),
  listPresets: vi.fn(),
  capturedLines: null as null | ((lines: ParsedMessage[]) => void),
}));

vi.mock("../../api/translate", () => ({
  translateText: api.translateText,
  calibrateTranslation: api.calibrateTranslation,
  listPresets: api.listPresets,
}));

vi.mock("../../api/capture", () => ({
  getCaptureStatus: vi.fn(async () => ({ running: null, last_error: null, last_seen: 0 })),
  onCaptureStatus: vi.fn(async () => () => {}),
  onCapturedLines: vi.fn(async (handler: (lines: ParsedMessage[]) => void) => {
    api.capturedLines = handler;
    return () => {};
  }),
  refreshCapture: vi.fn(async () => {}),
  setCaptureInterval: vi.fn(async () => {}),
  startCapture: vi.fn(),
  stopCapture: vi.fn(),
}));

const { MAX_LINES, parseRefreshMs, useLensStore } = await import("./useLensStore");

const outcome = (translation: string) => ({ translation, issues: [] }) as unknown as TranslationOutcome;
const message = (text: string) => ({ speaker: "them", sender: null, text }) as unknown as ParsedMessage;

/** 让挂起的 promise 回调跑完。 */
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

/** 手动控制何时完成的 promise，用来制造"旧请求比新请求晚返回"。 */
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

describe("parseRefreshMs", () => {
  it("accepts manual and the offered intervals", () => {
    expect(parseRefreshMs("manual")).toBeNull();
    expect(parseRefreshMs("5000")).toBe(5000);
  });

  it("falls back to the default for missing or unknown values", () => {
    expect(parseRefreshMs(null)).toBe(2000);
    expect(parseRefreshMs("1234")).toBe(2000);
    expect(parseRefreshMs("fast")).toBe(2000);
  });
});

describe("useLensStore", () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    api.listPresets.mockResolvedValue([]);
    useLensStore.setState(useLensStore.getInitialState(), true);
    const dispose = useLensStore.getState().init();
    await flush();
    return dispose;
  });

  it("keeps at most MAX_LINES captured lines, dropping the oldest", () => {
    api.capturedLines?.(Array.from({ length: MAX_LINES + 5 }, (_, i) => message(`line ${i}`)));
    const { lines } = useLensStore.getState();
    expect(lines).toHaveLength(MAX_LINES);
    expect(lines[0]?.text).toBe("line 5");
  });

  it("drops a reply translation that finishes after the user already typed something else", async () => {
    const slow = deferred<TranslationOutcome>();
    api.translateText.mockReturnValueOnce(slow.promise).mockResolvedValueOnce(outcome("See you tomorrow"));

    const first = useLensStore.getState().translateReply("明天见吗");
    await useLensStore.getState().translateReply("明天见");
    slow.resolve(outcome("stale"));
    await first;

    const { reply } = useLensStore.getState();
    expect(reply.text).toBe("明天见");
    expect(reply.translation?.translation).toBe("See you tomorrow");
  });

  it("records a failed line translation on that line only", async () => {
    api.capturedLines?.([message("hello"), message("how much?")]);
    const [first, second] = useLensStore.getState().lines;
    api.translateText.mockRejectedValueOnce({ code: "engine_unavailable", message: "本地引擎未就绪" });

    await useLensStore.getState().translateLine(first!.id);

    const [failed, untouched] = useLensStore.getState().lines;
    expect(failed?.error?.code).toBe("engine_unavailable");
    expect(failed?.busy).toBeNull();
    expect(untouched).toBe(second);
  });

  it("sends the other lines as calibration context, not the line being calibrated", async () => {
    api.capturedLines?.([message("hello"), message("how much?")]);
    const [first] = useLensStore.getState().lines;
    api.calibrateTranslation.mockResolvedValueOnce(outcome("你好"));

    await useLensStore.getState().calibrateLine(first!.id);

    expect(api.calibrateTranslation).toHaveBeenCalledWith(
      expect.objectContaining({ source: "hello", context: [{ speaker: "them", text: "how much?" }] }),
    );
  });
});
