// 与 Rust 端 serde 输出一一对应（字段保持 snake_case），修改任一端都要同步另一端。

export type Direction = "incoming" | "outgoing";
export type Tone = "casual" | "formal";
export type EngineKind = "local" | "online";
export type EnginePreference = "local" | "online";
export type Speaker = "me" | "them";

export interface Turn {
  speaker: Speaker;
  text: string;
}

export interface LanguagePair {
  user: string;
  foreign: string;
}

export interface LocalEngineSettings {
  server_path: string | null;
  model_path: string | null;
  threads: number;
  context_size: number;
  port: number;
}

export type OnlineChannel = "api_key" | "opencode";

export interface OpencodeSettings {
  /** 为空时自动查找 opencode.exe。 */
  program: string | null;
  /** 提供方/模型，例如 opencode-go/deepseek-v4-flash。 */
  model: string;
  variant: string | null;
}

export interface OnlineEngineSettings {
  enabled: boolean;
  channel: OnlineChannel;
  display_name: string;
  base_url: string;
  model: string;
  reasoning_effort: string | null;
  api_key_secret: string;
  opencode: OpencodeSettings;
}

export interface RoutingSettings {
  incoming: EnginePreference;
  outgoing: EnginePreference;
}

export interface CalibrationSettings {
  context_turns: number;
}

export interface EngineSettings {
  local: LocalEngineSettings;
  online: OnlineEngineSettings;
  routing: RoutingSettings;
  calibration: CalibrationSettings;
}

export interface AppSettings {
  schema_version: number;
  languages: LanguagePair;
  engines: EngineSettings;
}

export type ValidationIssue =
  | { kind: "wrong_script"; expected: "han" | "latin" | "unknown" }
  | { kind: "length_ratio"; ratio: number; min: number; max: number }
  | { kind: "missing_numbers"; numbers: string[] };

export interface TranslationOutcome {
  translation: string;
  issues: ValidationIssue[];
  notes: string[];
  engine: EngineKind;
  model: string;
  elapsed_ms: number;
  fell_back: boolean;
  prompt_version: string;
}

export type EngineState =
  | { state: "not_configured" }
  | { state: "starting" }
  | { state: "ready"; model: string }
  | { state: "failed"; message: string };

export interface EngineStatus {
  local: EngineState;
  online: EngineState;
}

export interface ModelFile {
  path: string;
  file_name: string;
  size_bytes: number;
}

export interface PresetSummary {
  id: string;
  name: string;
}

export interface TranslateArgs {
  direction: Direction;
  text: string;
  tone?: Tone;
  preset_id?: string | null;
}

export interface CalibrateArgs {
  direction: Direction;
  source: string;
  draft: string;
  tone?: Tone;
  context: Turn[];
  preset_id?: string | null;
}

export interface CommandErrorPayload {
  code: string;
  message: string;
  detail?: string;
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface WindowInfo {
  id: number;
  title: string;
  process_name: string;
}

export interface ParsedMessage {
  speaker: Speaker;
  sender: string | null;
  text: string;
}

export interface CaptureInfo {
  /** 取景框显示尺寸，跟随窗口显示窗口标题。 */
  label: string;
  preset_id: string;
  preset_name: string;
}

export interface CaptureStatus {
  running: CaptureInfo | null;
  last_error: string | null;
  last_seen: number;
}

export interface CapturePreview {
  region: Rect;
  messages: ParsedMessage[];
}
