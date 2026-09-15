import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { call } from "./client";
import type { EngineStatus, ModelFile } from "./types";

const ENGINE_STATUS_EVENT = "engine-status";

export const getEngineStatus = () => call<EngineStatus>("engine_status");
export const restartEngines = () => call<void>("restart_engines");
export const discoverLocalModels = () => call<ModelFile[]>("discover_local_models");
export const takeStartupWarnings = () => call<string[]>("take_startup_warnings");

export const onEngineStatus = (handler: (status: EngineStatus) => void): Promise<UnlistenFn> =>
  listen<EngineStatus>(ENGINE_STATUS_EVENT, (event) => handler(event.payload));
