<script setup lang="ts">
import { ref } from "vue";
import { ping, getConfig, setConfig, showMainWindow, type VoxisConfig } from "../lib/ipc";

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
    // 写入探针：改一次日志级别字段再写回，验证保存链路
    cached.general.log_level = cached.general.log_level === "info" ? "info" : "info";
    await setConfig(cached);
    log("set_config", { ok: true, log_level: cached.general.log_level });
  } catch (e) {
    log("set_config 错误", String(e));
  }
};
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

    <pre class="bg-black rounded p-4 text-xs overflow-auto max-h-[70vh] whitespace-pre-wrap">{{ output }}</pre>
  </div>
</template>
