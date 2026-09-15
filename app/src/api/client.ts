import { invoke } from "@tauri-apps/api/core";

import type { CommandErrorPayload } from "./types";

/** 前端统一的错误类型：`message` 给人看，`detail` 放技术细节，`code` 决定界面给出什么操作建议。 */
export class AppError extends Error {
  readonly code: string;
  readonly detail?: string;

  constructor(payload: CommandErrorPayload) {
    super(payload.message);
    this.name = "AppError";
    this.code = payload.code;
    this.detail = payload.detail;
  }
}

/** 整个前端只有这里直接调用 Tauri invoke。 */
export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (raw) {
    throw toAppError(raw);
  }
}

export function toAppError(raw: unknown): AppError {
  if (raw instanceof AppError) return raw;
  if (typeof raw === "object" && raw !== null && "code" in raw && "message" in raw) {
    return new AppError(raw as CommandErrorPayload);
  }
  return new AppError({ code: "unknown", message: raw instanceof Error ? raw.message : String(raw) });
}
