import { call } from "./client";
import type { AppSettings } from "./types";

export const getSettings = () => call<AppSettings>("get_settings");
export const saveSettings = (settings: AppSettings) => call<void>("save_settings", { settings });
export const hasOnlineApiKey = () => call<boolean>("has_online_api_key");
export const setOnlineApiKey = (key: string) => call<void>("set_online_api_key", { key });
export const clearOnlineApiKey = () => call<void>("clear_online_api_key");
