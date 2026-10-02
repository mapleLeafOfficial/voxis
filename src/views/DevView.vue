<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  ping,
  getConfig,
  setConfig,
  showMainWindow,
  listInputDevices,
  devCaptureStart,
  devCaptureStop,
  startSession,
  stopSession,
  EV,
  type VoxisConfig,
  type InputDevice,
  type SessionState,
  type PermissionStatus,
  getPermissionStatus,
} from "../lib/ipc";

// ---- todo6：热键调试 ----
const perm = ref<PermissionStatus | null>(null);
const heldKeys = ref("");
const hotkeyLog = ref("");

const output = ref<string>("# DevView — 临时开发面板\n");
const log = (label: string, data: unknown) => {
  output.value += `\n[${label}]\n${JSON.stringify(data, null, 2)}\n`;
};

const doPing = async () => {
  try {
    log("ping", await ping());
  } catch (e) {
    log("ping 错误", String(e));
  }
};

let cached: VoxisConfig | null = null;

const doGetConfig = async () => {
  try {
    cached = await getConfig();
    log("get_config", cached);
  } catch (e) {
    log("get_config 错误", String(e));
  }
};

const doSetConfig = async () => {
  try {
    if (!cached) cached = await getConfig();
    await setConfig(cached);
    log("set_config", { ok: true });
  } catch (e) {
    log("set_config 错误", String(e));
  }
};

// ---- todo2：采集测试 ----
const devices = ref<InputDevice[]>([]);
const selected = ref<string>("default");
const capturing = ref(false);
const volume = ref(0);
let unlisteners: UnlistenFn[] = [];

const refreshDevices = async () => {
  try {
    devices.value = await listInputDevices();
    const def = devices.value.find((d) => d.is_default);
    if (def) selected.value = def.id;
    log("list_input_devices", devices.value);
  } catch (e) {
    log("list_input_devices 错误", String(e));
  }
};

const startCapture = async () => {
  try {
    await devCaptureStart(selected.value === "default" ? undefined : selected.value);
    capturing.value = true;
  } catch (e) {
    log("dev_capture_start 错误", String(e));
  }
};

const stopCapture = async () => {
  try {
    await devCaptureStop();
  } catch (e) {
    log("dev_capture_stop 错误", String(e));
  } finally {
    capturing.value = false;
    volume.value = 0;
  }
};

// ---- todo4：会话测试 ----
const sessionState = ref<SessionState>("idle");
const partialText = ref("");
const finalText = ref("");
const sessionSentences = ref<string[]>([]);

const startSessionClick = async () => {
  try {
    sessionSentences.value = [];
    partialText.value = "";
    finalText.value = "";
    await startSession(selected.value === "default" ? undefined : selected.value);
    log("start_session", "已请求");
  } catch (e) {
    log("start_session 错误", String(e));
  }
};

const stopSessionClick = async () => {
  try {
    await stopSession();
  } catch (e) {
    log("stop_session 错误", String(e));
  }
};

const stateLabel: Record<SessionState, string> = {
  idle: "空闲",
  recording: "录音中",
  committing: "收尾中",
};
const stateClass: Record<SessionState, string> = {
  idle: "bg-neutral-700 text-neutral-300",
  recording: "bg-red-600 text-white animate-pulse",
  committing: "bg-amber-600 text-white",
};

onMounted(async () => {
  unlisteners.push(
    await listen<number>(EV.volume, (e) => (volume.value = e.payload)),
    await listen<{ source: string; message: string }>(EV.error, (e) =>
      log("采集错误", e.payload),
    ),
    await listen(EV.maxDuration, () => {
      capturing.value = false;
      volume.value = 0;
      log("max_duration", "达到最长录音时长，已自动停止");
    }),
    // ---- todo4：会话事件 ----
    await listen<SessionState>(EV.state, (e) => {
      sessionState.value = e.payload;
      if (e.payload === "idle") capturing.value = false;
      log("session://state", e.payload);
    }),
    await listen<string>(EV.partial, (e) => (partialText.value = e.payload)),
    await listen<string>(EV.sentence, (e) => {
      sessionSentences.value.push(e.payload);
      partialText.value = "";
    }),
    await listen<{ text: string; result: string }>(EV.committed, (e) => {
      finalText.value = e.payload.text;
      log("session://committed", `[${e.payload.result}] ${e.payload.text || "（空文本）"}`);
    }),
    // ---- todo6：热键调试事件 ----
    await listen<string>(EV.hotkeyDebug, (e) => {
      if (e.payload.includes("↓") || e.payload.includes("↑")) {
        heldKeys.value = e.payload.split("（按住: ")[1]?.replace("）", "") ?? "";
      } else {
        hotkeyLog.value = e.payload;
      }
    }),
    await listen<PermissionStatus>(EV.hotkeyPermission, (e) => {
      perm.value = e.payload;
      log("hotkey://permission", e.payload);
    }),
  );
  getPermissionStatus().then((p) => (perm.value = p)).catch(() => {});
  await refreshDevices();
  doGetConfig();
});

onBeforeUnmount(() => unlisteners.forEach((u) => u()));
</script>

<template>
  <div class="min-h-screen bg-neutral-950 text-neutral-100 p-6 font-mono">
    <h1 class="text-xl font-bold mb-4">Voxis DevView <span class="text-neutral-500 text-sm">（临时开发面板）</span></h1>

    <div class="flex gap-3 flex-wrap mb-4">
      <button class="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-sm" @click="doPing">ping</button>
      <button class="px-3 py-1.5 rounded bg-neutral-700 hover:bg-neutral-600 text-sm" @click="doGetConfig">get_config</button>
      <button class="px-3 py-1.5 rounded bg-neutral-700 hover:bg-neutral-600 text-sm" @click="doSetConfig">set_config（保存当前值）</button>
      <button class="px-3 py-1.5 rounded bg-emerald-700 hover:bg-emerald-600 text-sm" @click="showMainWindow">show_main_window</button>
      <router-link to="/settings" class="px-3 py-1.5 rounded bg-neutral-800 hover:bg-neutral-700 text-sm">→ /settings 占位</router-link>
    </div>

    <!-- 采集测试面板（todo2） -->
    <div class="border border-neutral-800 rounded-lg p-4 mb-4">
      <div class="flex items-center gap-3 flex-wrap mb-3">
        <span class="text-sm text-neutral-400">采集测试：</span>
        <select v-model="selected" class="bg-neutral-900 border border-neutral-700 rounded px-2 py-1 text-sm max-w-md">
          <option value="default">系统默认输入</option>
          <option v-for="d in devices" :key="d.id" :value="d.id">{{ d.name }}{{ d.is_default ? "（默认）" : "" }}</option>
        </select>
        <button
          class="px-3 py-1.5 rounded text-sm"
          :class="capturing ? 'bg-red-600 hover:bg-red-500' : 'bg-sky-600 hover:bg-sky-500'"
          @click="capturing ? stopCapture() : startCapture()"
        >
          {{ capturing ? "■ 停止采集" : "● 开始采集" }}
        </button>
        <span v-if="capturing" class="text-xs text-red-400 animate-pulse">REC</span>
      </div>
      <div class="flex items-center gap-3">
        <span class="text-xs text-neutral-500 w-10">音量</span>
        <div class="flex-1 h-2 bg-neutral-800 rounded overflow-hidden">
          <div class="h-full bg-gradient-to-r from-emerald-500 to-red-500 transition-[width] duration-75" :style="{ width: `${(volume * 100).toFixed(0)}%` }" />
        </div>
        <span class="text-xs text-neutral-500 w-12 text-right">{{ (volume * 100).toFixed(0) }}%</span>
      </div>
    </div>

    <!-- 会话测试面板（todo4，M1） -->
    <div class="border border-neutral-800 rounded-lg p-4 mb-4">
      <div class="flex items-center gap-3 flex-wrap mb-3">
        <span class="text-sm text-neutral-400">会话测试：</span>
        <span class="px-2 py-0.5 rounded text-xs" :class="stateClass[sessionState]">{{ stateLabel[sessionState] }}</span>
        <button
          class="px-3 py-1.5 rounded text-sm"
          :class="sessionState === 'idle' ? 'bg-violet-600 hover:bg-violet-500' : 'bg-red-600 hover:bg-red-500'"
          :disabled="sessionState === 'committing'"
          @click="sessionState === 'idle' ? startSessionClick() : stopSessionClick()"
        >
          {{ sessionState === "idle" ? "🎤 开始会话" : "⏹ 结束会话" }}
        </button>
        <span class="text-xs text-neutral-600">使用上方选中的设备；说话 → partial 实时刷新，结束 → 2s 内出全文</span>
      </div>
      <div class="space-y-2 text-sm">
        <div v-if="sessionSentences.length || partialText" class="bg-neutral-900 rounded p-3">
          <div class="text-neutral-500 text-xs mb-1">实时（已断句 + 当前 partial）</div>
          <span class="text-neutral-200">{{ sessionSentences.join(" ") }}</span>
          <span class="text-neutral-500">{{ partialText }}</span>
        </div>
        <div v-if="finalText" class="bg-neutral-900 rounded p-3">
          <div class="text-neutral-500 text-xs mb-1">最终结果（committed）</div>
          <span class="text-emerald-300">{{ finalText }}</span>
        </div>
      </div>
    </div>

    <!-- 热键调试面板（todo6） -->
    <div class="border border-neutral-800 rounded-lg p-4 mb-4">
      <div class="flex items-center gap-3 flex-wrap mb-2">
        <span class="text-sm text-neutral-400">热键：</span>
        <span class="px-2 py-0.5 rounded text-xs" :class="perm ? (perm.input_ok ? 'bg-emerald-800 text-emerald-200' : 'bg-red-800 text-red-200') : 'bg-neutral-800 text-neutral-400'">
          {{ perm ? (perm.input_ok ? "input ✓" : "input ✗") : "检查中…" }}
        </span>
        <span class="px-2 py-0.5 rounded text-xs" :class="perm?.uinput_ok ? 'bg-emerald-800 text-emerald-200' : 'bg-neutral-800 text-neutral-500'">uinput {{ perm?.uinput_ok ? "✓" : "✗" }}</span>
        <span class="px-2 py-0.5 rounded text-xs" :class="perm?.ydotoold_ok ? 'bg-emerald-800 text-emerald-200' : 'bg-neutral-800 text-neutral-500'">ydotoold {{ perm?.ydotoold_ok ? "✓" : "✗" }}</span>
        <span v-if="heldKeys" class="font-mono text-xs text-amber-300">按住: {{ heldKeys }}</span>
        <span class="text-xs text-neutral-600">按住 Ctrl+Win 说话 → 松开结束；Ctrl+Win+Shift toggle</span>
      </div>
      <div class="text-xs text-neutral-500">最近触发：<span class="text-neutral-300">{{ hotkeyLog || "（无）" }}</span></div>
    </div>

    <pre class="bg-black rounded p-4 text-xs overflow-auto max-h-[50vh] whitespace-pre-wrap">{{ output }}</pre>
  </div>
</template>
