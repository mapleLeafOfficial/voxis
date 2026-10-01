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
  EV,
  type VoxisConfig,
  type InputDevice,
} from "../lib/ipc";

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
  );
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

    <pre class="bg-black rounded p-4 text-xs overflow-auto max-h-[50vh] whitespace-pre-wrap">{{ output }}</pre>
  </div>
</template>
