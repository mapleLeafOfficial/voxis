// 类型安全的 IPC 封装（todo1 基础命令集）
import { invoke } from "@tauri-apps/api/core";

export interface VoxisConfig {
  api_key: string;
  asr: {
    model: string;
    ws_url: string;
    silence_ms: number;
    semantic_punct: boolean;
    language: string;
    max_duration: number;
  };
  hotkey: { hold: string[]; lock: string[] };
  input: { device: string };
  commit: { mode: "auto" | "clipboard_only" };
  polish: {
    mode: "off" | "auto" | "manual";
    model: string;
    hotkey: string;
    prompt_override: string | null;
  };
  ui: { bubble_pos: string; theme: "light" | "dark" | "system" };
  general: { autostart: boolean; log_level: string };
}

export const ping = () => invoke<string>("ping");
export const getConfig = () => invoke<VoxisConfig>("get_config");
export const setConfig = (config: VoxisConfig) => invoke<void>("set_config", { config });
export const showMainWindow = () => invoke<void>("show_main_window");

// ---- todo2：录音 ----
export interface InputDevice {
  id: string;
  name: string;
  is_default: boolean;
}
export const listInputDevices = () => invoke<InputDevice[]>("list_input_devices");
export const devCaptureStart = (device?: string) =>
  invoke<void>("dev_capture_start", { device: device ?? null });
export const devCaptureStop = () => invoke<void>("dev_capture_stop");

// ---- todo4：会话 ----
export type SessionState = "idle" | "recording" | "committing";
export const startSession = (device?: string, lock = false) =>
  invoke<void>("start_session", { device: device ?? null, lock });
export const stopSession = () => invoke<void>("stop_session");

export interface PermissionStatus {
  input_ok: boolean;
  uinput_ok: boolean;
  ydotoold_ok: boolean;
  problems: string[];
}

export const getPermissionStatus = () =>
  invoke<PermissionStatus>("get_permission_status");

// ---- 事件名常量 ----
export const EV = {
  volume: "session://volume",
  error: "session://error",
  maxDuration: "session://max_duration_reached",
  partial: "session://partial",
  sentence: "session://sentence",
  state: "session://state",
  committed: "session://committed",
  hotkeyDebug: "hotkey://debug",
  hotkeyPermission: "hotkey://permission",
} as const;

/** 提交结果三态（session://committed payload） */
export interface CommittedPayload {
  text: string;
  result: "pasted" | "copied" | "none";
}
