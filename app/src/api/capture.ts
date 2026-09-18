import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { call } from "./client";
import type {
  CapturePreview,
  CaptureStatus,
  ParsedMessage,
  WindowInfo,
} from "./types";

const CAPTURE_LINES_EVENT = "capture-lines";
const CLICK_THROUGH_EVENT = "lens-click-through";
const CAPTURE_STATUS_EVENT = "capture-status";

/** 识别哪块画面：取景框给屏幕绝对坐标，跟随窗口给窗口句柄。 */
export type CaptureSourceArgs =
  | { mode: "lens"; x: number; y: number; width: number; height: number }
  | { mode: "window"; window: number };

export const captureSupported = () => call<boolean>("capture_supported");
export const getCaptureStatus = () => call<CaptureStatus>("capture_status");
export const listCaptureWindows = () =>
  call<WindowInfo[]>("list_capture_windows");
export const openLens = () => call<void>("open_lens");
export const closeLens = () => call<void>("close_lens");
/** 开关取景框，返回切换后是否可见。 */
export const toggleLens = () => call<boolean>("toggle_lens");
export const showMainWindow = () => call<void>("show_main_window");
/** 真正退出应用（主窗口的关闭按钮只是收起成悬浮按钮）。 */
export const quitApp = () => call<void>("quit_app");

/** 取景框让开鼠标 / 恢复，返回切换后是否在让开鼠标。 */
export const toggleLensClickThrough = () =>
  call<boolean>("toggle_lens_click_through");
export const lensClickThrough = () => call<boolean>("lens_click_through");
export const onClickThrough = (
  handler: (ignoring: boolean) => void,
): Promise<UnlistenFn> =>
  listen<boolean>(CLICK_THROUGH_EVENT, (event) => handler(event.payload));

/** 全局热键说明：[按键, 作用]。 */
export const listHotkeys = () => call<Array<[string, string]>>("list_hotkeys");

/** 系统装了哪些 OCR 语言；没装对应语言就什么都识别不出来。 */
export const listOcrLanguages = () => call<string[]>("list_ocr_languages");

export const previewCapture = (
  source: CaptureSourceArgs,
  presetId: string,
  lang: string | null,
) =>
  call<CapturePreview>("preview_capture", {
    request: { source, preset_id: presetId, lang },
  });

export const startCapture = (
  source: CaptureSourceArgs,
  presetId: string,
  lang: string | null,
) =>
  call<CaptureStatus>("start_capture", {
    request: { source, preset_id: presetId, lang },
  });

export const stopCapture = () => call<CaptureStatus>("stop_capture");

/** 刷新节奏：传 null 为手动模式，只在调用 refreshCapture 时识别。 */
export const setCaptureInterval = (intervalMs: number | null) =>
  call<void>("set_capture_interval", { intervalMs });

/** 马上识别一轮。 */
export const refreshCapture = () => call<void>("refresh_capture");

/** 识别中挪动取景框或改布局后同步新区域；不清空已识别的句子。 */
export const updateCaptureRegion = (rect: {
  x: number;
  y: number;
  width: number;
  height: number;
}) => call<CaptureStatus>("update_capture_region", rect);

/** 新识别到的对方消息（还没翻译，用户点哪句翻哪句）。 */
export const onCapturedLines = (
  handler: (lines: ParsedMessage[]) => void,
): Promise<UnlistenFn> =>
  listen<ParsedMessage[]>(CAPTURE_LINES_EVENT, (event) =>
    handler(event.payload),
  );

export const onCaptureStatus = (
  handler: (status: CaptureStatus) => void,
): Promise<UnlistenFn> =>
  listen<CaptureStatus>(CAPTURE_STATUS_EVENT, (event) =>
    handler(event.payload),
  );
