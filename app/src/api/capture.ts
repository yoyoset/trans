import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { call } from "./client";
import type { CapturePreview, CaptureStatus, ParsedMessage, WindowInfo } from "./types";

const CAPTURE_LINES_EVENT = "capture-lines";
const CAPTURE_STATUS_EVENT = "capture-status";

/** 识别哪块画面：取景框给屏幕绝对坐标，跟随窗口给窗口句柄。 */
export type CaptureSourceArgs =
  | { mode: "lens"; x: number; y: number; width: number; height: number }
  | { mode: "window"; window: number };

export const captureSupported = () => call<boolean>("capture_supported");
export const getCaptureStatus = () => call<CaptureStatus>("capture_status");
export const listCaptureWindows = () => call<WindowInfo[]>("list_capture_windows");
export const openLens = () => call<void>("open_lens");
export const closeLens = () => call<void>("close_lens");

export const previewCapture = (source: CaptureSourceArgs, presetId: string) =>
  call<CapturePreview>("preview_capture", { request: { source, preset_id: presetId } });

export const startCapture = (source: CaptureSourceArgs, presetId: string) =>
  call<CaptureStatus>("start_capture", { request: { source, preset_id: presetId } });

export const stopCapture = () => call<CaptureStatus>("stop_capture");

/** 新识别到的对方消息（还没翻译，用户点哪句翻哪句）。 */
export const onCapturedLines = (handler: (lines: ParsedMessage[]) => void): Promise<UnlistenFn> =>
  listen<ParsedMessage[]>(CAPTURE_LINES_EVENT, (event) => handler(event.payload));

export const onCaptureStatus = (handler: (status: CaptureStatus) => void): Promise<UnlistenFn> =>
  listen<CaptureStatus>(CAPTURE_STATUS_EVENT, (event) => handler(event.payload));
