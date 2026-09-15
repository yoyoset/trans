import { call } from "./client";
import type { CalibrateArgs, PresetSummary, TranslateArgs, TranslationOutcome } from "./types";

export const translateText = (request: TranslateArgs) => call<TranslationOutcome>("translate_text", { request });
export const calibrateTranslation = (request: CalibrateArgs) =>
  call<TranslationOutcome>("calibrate_translation", { request });
export const listPresets = () => call<PresetSummary[]>("list_presets");
