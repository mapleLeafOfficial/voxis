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
