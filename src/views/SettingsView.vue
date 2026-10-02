<template>
  <div class="min-h-screen bg-neutral-50 dark:bg-neutral-950 text-neutral-900 dark:text-neutral-100">
    <div class="mx-auto max-w-3xl px-6 py-6">
      <header class="flex items-center justify-between mb-5">
        <h1 class="text-lg font-bold">Voxis 设置</h1>
        <router-link to="/" class="text-sm text-blue-500 hover:underline">← 调试台</router-link>
      </header>

      <div v-if="!ready" class="text-sm text-neutral-400 py-16 text-center">加载配置…</div>

      <div v-else-if="cfg" class="flex gap-6">
        <!-- 左侧导航 -->
        <nav class="w-36 shrink-0 space-y-1">
          <button
            v-for="t in TABS"
            :key="t.id"
            type="button"
            class="w-full rounded-md px-3 py-2 text-left text-sm transition-colors"
            :class="
              tab === t.id
                ? 'bg-blue-500/10 text-blue-600 dark:text-blue-400 font-medium'
                : 'text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800'
            "
            @click="tab = t.id"
          >
            {{ t.label }}
          </button>
        </nav>

        <!-- 右侧表单 -->
        <main class="flex-1 space-y-5 min-w-0">
          <!-- 账户 -->
          <section v-show="tab === 'account'" class="card">
            <h2 class="card-title">账户</h2>
            <label class="field-label">DashScope API Key</label>
            <div class="flex gap-2">
              <input
                v-model="cfg.api_key"
                :type="showKey ? 'text' : 'password'"
                placeholder="sk-…（留空则读 QWEN_API_KEY 环境变量 / ini）"
                class="input flex-1"
                spellcheck="false"
              />
              <button type="button" class="btn" @click="showKey = !showKey">{{ showKey ? "隐藏" : "显示" }}</button>
              <button type="button" class="btn" :disabled="testing" @click="onTestKey">
                {{ testing ? "测试中…" : "测试" }}
              </button>
            </div>
            <p v-if="testResult" :class="testOk ? 'text-emerald-500' : 'text-red-500'" class="text-xs mt-1.5">
              {{ testResult }}
            </p>
            <p class="hint">优先级：本页 Key &gt; QWEN_API_KEY 环境变量 &gt; ~/.config/qwen-voice-input/config.ini</p>
          </section>

          <!-- 快捷键 -->
          <section v-show="tab === 'hotkey'" class="space-y-5">
            <div class="card space-y-4">
              <h2 class="card-title">全局快捷键</h2>
              <div>
                <label class="field-label">按住说话（hold）</label>
                <HotkeyRecorder v-model="cfg.hotkey.hold" label="点击录制" @reset="cfg.hotkey.hold = ['Ctrl', 'Super']" />
              </div>
              <div>
                <label class="field-label">开关切换（lock，为 hold 全集 + 附加键，如再含 Shift）</label>
                <HotkeyRecorder v-model="cfg.hotkey.lock" label="点击录制" @reset="cfg.hotkey.lock = ['Ctrl', 'Super', 'Shift']" />
              </div>
              <p class="hint">修改保存后立即生效（引擎自动重启）；lock 必须包含 hold 的全部键并额外加键。</p>
            </div>
            <PermissionPanel />
          </section>

          <!-- 识别 -->
          <section v-show="tab === 'asr'" class="card space-y-4">
            <h2 class="card-title">语音识别</h2>
            <div>
              <label class="field-label">模型</label>
              <select v-model="asrModelPreset" class="input w-full">
                <option value="qwen-audio-3.0-asr-flash-streaming">qwen-audio-3.0-asr-flash-streaming（默认）</option>
                <option value="qwen3-asr-flash-realtime">qwen3-asr-flash-realtime</option>
                <option value="__custom">自定义…</option>
              </select>
              <input v-if="asrModelPreset === '__custom'" v-model="cfg.asr.model" class="input w-full mt-2" placeholder="模型名" />
            </div>
            <div>
              <label class="field-label">断句静音 {{ cfg.asr.silence_ms }} ms</label>
              <input v-model.number="cfg.asr.silence_ms" type="range" min="500" max="3000" step="100" class="w-full accent-blue-500" />
            </div>
            <label class="flex items-center gap-2 text-sm">
              <input v-model="cfg.asr.semantic_punct" type="checkbox" class="accent-blue-500" />
              语义断句（智能标点）
            </label>
            <div>
              <label class="field-label">识别语言</label>
              <select v-model="cfg.asr.language" class="input w-full">
                <option value="">自动检测</option>
                <option value="zh">中文</option>
                <option value="en">英文</option>
              </select>
            </div>
            <div>
              <label class="field-label">单次最长录音（秒）</label>
              <input v-model.number="cfg.asr.max_duration" type="number" min="5" max="300" class="input w-32" />
            </div>
          </section>

          <!-- 上屏 -->
          <section v-show="tab === 'commit'" class="card space-y-4">
            <h2 class="card-title">上屏</h2>
            <div class="space-y-2">
              <label class="flex items-center gap-2 text-sm">
                <input v-model="cfg.commit.mode" type="radio" value="auto" class="accent-blue-500" />
                自动粘贴到光标（写剪贴板 → 注入 Ctrl+V）
              </label>
              <label class="flex items-center gap-2 text-sm">
                <input v-model="cfg.commit.mode" type="radio" value="clipboard_only" class="accent-blue-500" />
                仅复制（手动 Ctrl+V）
              </label>
            </div>
            <div>
              <label class="field-label">输入设备</label>
              <select v-model="cfg.input.device" class="input w-full">
                <option value="">系统默认</option>
                <option v-for="d in inputDevices" :key="d.id" :value="d.id">{{ d.name }}{{ d.is_default ? "（默认）" : "" }}</option>
              </select>
              <p class="hint">切换后下一次会话生效</p>
            </div>
          </section>

          <!-- 外观 -->
          <section v-show="tab === 'appearance'" class="card space-y-4">
            <h2 class="card-title">外观</h2>
            <div>
              <label class="field-label">气泡位置</label>
              <select v-model="cfg.ui.bubble_pos" class="input w-full">
                <option value="bottom_center">底部居中</option>
                <option value="bottom_left">左下角</option>
                <option value="bottom_right">右下角</option>
                <option value="top_left">左上角</option>
                <option value="top_right">右上角</option>
              </select>
            </div>
            <div>
              <label class="field-label">主题</label>
              <div class="flex gap-4 text-sm">
                <label v-for="t in ['light', 'dark', 'system']" :key="t" class="flex items-center gap-1.5">
                  <input v-model="cfg.ui.theme" type="radio" :value="t" class="accent-blue-500" />
                  {{ { light: "浅色", dark: "深色", system: "跟随系统" }[t] }}
                </label>
              </div>
            </div>
            <label class="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                class="accent-blue-500"
                :checked="autostart"
                @change="onAutostart(($event.target as HTMLInputElement).checked)"
              />
              开机自启（GNOME 启动应用可见）
            </label>
          </section>
        </main>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
// 设置主窗口：左导航 + 右表单；任何变更防抖 600ms 走 setConfig 即时持久化（Rust 侧 diff 触发热键重启/主题广播）
import { onBeforeUnmount, onMounted, ref, watch } from "@vue/runtime-core";
import {
  getConfig, setConfig, listInputDevices, testApiKey, getAutostart, setAutostart,
  type InputDevice, type VoxisConfig,
} from "../lib/ipc";
import HotkeyRecorder from "../components/settings/HotkeyRecorder.vue";
import PermissionPanel from "../components/settings/PermissionPanel.vue";

const TABS = [
  { id: "account", label: "账户" },
  { id: "hotkey", label: "快捷键" },
  { id: "asr", label: "识别" },
  { id: "commit", label: "上屏" },
  { id: "appearance", label: "外观" },
] as const;

const ready = ref(false);
const tab = ref<string>("account");
const cfg = ref<VoxisConfig | null>(null);
const showKey = ref(false);
const testing = ref(false);
const testResult = ref("");
const testOk = ref(false);
const inputDevices = ref<InputDevice[]>([]);
const autostart = ref(false);
const asrModelPreset = ref("qwen-audio-3.0-asr-flash-streaming");

let saveTimer: ReturnType<typeof setTimeout> | null = null;

watch(cfg, () => {
  if (!cfg.value || !ready.value) return;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    try {
      await setConfig(JSON.parse(JSON.stringify(cfg.value)));
    } catch (e) {
      console.error("保存配置失败", e);
    }
  }, 600);
}, { deep: true });

watch(asrModelPreset, (v) => {
  if (cfg.value && v !== "__custom") cfg.value.asr.model = v;
});

async function onTestKey() {
  if (!cfg.value) return;
  testing.value = true;
  testResult.value = "";
  // 先保存当前输入的 key 再测（test_api_key 读的是入参 key，直接传）
  try {
    const msg = await testApiKey(cfg.value.api_key);
    testOk.value = true;
    testResult.value = msg;
  } catch (e) {
    testOk.value = false;
    testResult.value = String(e);
  } finally {
    testing.value = false;
  }
}

async function onAutostart(enable: boolean) {
  try {
    await setAutostart(enable);
    autostart.value = enable;
  } catch (e) {
    console.error(e);
    autostart.value = await getAutostart().catch(() => false);
  }
}

onMounted(async () => {
  const [config, devices, auto] = await Promise.all([
    getConfig(),
    listInputDevices().catch(() => [] as InputDevice[]),
    getAutostart().catch(() => false),
  ]);
  cfg.value = config;
  inputDevices.value = devices;
  autostart.value = auto;
  asrModelPreset.value = ["qwen-audio-3.0-asr-flash-streaming", "qwen3-asr-flash-realtime"].includes(config.asr.model)
    ? config.asr.model
    : "__custom";
  ready.value = true;
});

onBeforeUnmount(() => {
  if (saveTimer) clearTimeout(saveTimer);
});
</script>

<style scoped>
@reference "../style.css";

.card {
  @apply rounded-lg border border-neutral-200 dark:border-neutral-700 bg-white dark:bg-neutral-900 p-5 space-y-3;
}
.card-title {
  @apply text-sm font-semibold text-neutral-700 dark:text-neutral-200;
}
.field-label {
  @apply block text-xs text-neutral-500 dark:text-neutral-400 mb-1.5;
}
.input {
  @apply rounded-md border border-neutral-300 dark:border-neutral-600 bg-white dark:bg-neutral-800 px-3 py-1.5 text-sm outline-none focus:border-blue-400;
}
.btn {
  @apply rounded-md border border-neutral-300 dark:border-neutral-600 px-3 py-1.5 text-sm hover:bg-neutral-100 dark:hover:bg-neutral-700 disabled:opacity-50;
}
.hint {
  @apply text-xs text-neutral-400 dark:text-neutral-500;
}
</style>
