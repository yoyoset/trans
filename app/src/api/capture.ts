import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { call } from "./client";
import type { CapturedMessage, CapturePreview, CaptureStatus, WindowInfo } from "./types";

const CAPTURE_MESSAGE_EVENT = "capture-message";
const CAPTURE_STATUS_EVENT = "capture-status";

export const captureSupported = () => call<boolean>("capture_supported");
export const getCaptureStatus = () => call<CaptureStatus>("capture_status");
export const listCaptureWindows = () => call<WindowInfo[]>("list_capture_windows");
export const previewCapture = (window: number, presetId: string) =>
  call<CapturePreview>("preview_capture", { request: { window, preset_id: presetId } });
export const startCapture = (window: number, presetId: string) =>
  call<CaptureStatus>("start_capture", { request: { window, preset_id: presetId } });
export const stopCapture = () => call<CaptureStatus>("stop_capture");

export const onCapturedMessage = (handler: (message: CapturedMessage) => void): Promise<UnlistenFn> =>
  listen<CapturedMessage>(CAPTURE_MESSAGE_EVENT, (event) => handler(event.payload));

export const onCaptureStatus = (handler: (status: CaptureStatus) => void): Promise<UnlistenFn> =>
  listen<CaptureStatus>(CAPTURE_STATUS_EVENT, (event) => handler(event.payload));
