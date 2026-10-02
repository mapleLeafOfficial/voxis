<script setup lang="ts">
// 气泡视图：录音状态点 + 音量条 + 实时文本 + 结果徽标。
// 数据全部来自 session:// 广播事件（Rust 侧 app.emit 对所有窗口广播）。
// 生命周期：气泡窗口由 Rust 侧 show/hide 控制，前端只管渲染与淡出过渡。
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { EV, getConfig, type SessionState } from "../lib/ipc";
import StatusDot from "../components/bubble/StatusDot.vue";
import VolumeBars from "../components/bubble/VolumeBars.vue";
import ResultBadge from "../components/bubble/ResultBadge.vue";

const state = ref<SessionState>("idle");
const partial = ref("");
const sentences = ref<string[]>([]);
const volume = ref(0);
const error = ref<{ source: string; message: string } | null>(null);
const resultText = ref<string | null>(null); // committed 全文（null=无结果）
const fading = ref(false); // 结果/错误徽标淡出

let unlisteners: UnlistenFn[] = [];
let lastVolumeAt = 0;
let media: MediaQueryList | null = null;
let mediaHandler: ((e: MediaQueryListEvent) => void) | null = null;

const dark = ref(false);

// dot 状态：error 优先，其次会话态
const dotState = computed<"idle" | "recording" | "committing" | "error">(() =>
  error.value ? "error" : state.value,
);

const fullText = computed(() =>
  [...sentences.value, partial.value].filter(Boolean).join(" "),
);

// 徽标：error > committed 结果
const badge = computed<"none" | "done" | "empty" | "error">(() => {
  if (error.value) return "error";
  if (resultText.value !== null) return resultText.value.trim() ? "done" : "empty";
  return "none";
});

// 会话结束（idle）即开始淡出倒计时（Rust 在 1.8s 后隐藏窗口；error 稍长由 Rust 控制）
function scheduleFade() {
  fading.value = false;
  window.setTimeout(() => (fading.value = true), state.value === "idle" ? 1300 : 1300);
}

const stopFromBubble = () => {
  // 点击气泡结束会话（窗口 focusable:false，验证点击可命中；Wayland 下若无效在备注记录）
  if (state.value === "recording") {
    invoke("stop_session").catch(() => {});
  }
};

function applyTheme(theme: string) {
  if (theme === "dark") dark.value = true;
  else if (theme === "light") dark.value = false;
  else {
    // system：跟随 prefers-color-scheme
    media = window.matchMedia("(prefers-color-scheme: dark)");
    dark.value = media.matches;
    mediaHandler = (e) => (dark.value = e.matches);
    media.addEventListener("change", mediaHandler);
  }
}

onMounted(async () => {
  try {
    const cfg = await getConfig();
    applyTheme(cfg.ui.theme);
  } catch {
    applyTheme("system");
  }

  unlisteners.push(
    await listen<SessionState>(EV.state, (e) => {
      state.value = e.payload;
      if (e.payload === "idle") {
        volume.value = 0;
        scheduleFade();
      } else {
        // 新会话开始：清上一轮结果
        fading.value = false;
        resultText.value = null;
        error.value = null;
      }
    }),
    await listen<string>(EV.partial, (e) => (partial.value = e.payload)),
    await listen<string>(EV.sentence, (e) => {
      sentences.value.push(e.payload);
      partial.value = "";
    }),
    await listen<number>(EV.volume, (e) => {
      // 节流 ~50ms，避免高频事件拖累渲染
      const now = performance.now();
      if (now - lastVolumeAt < 50) return;
      lastVolumeAt = now;
      volume.value = e.payload;
    }),
    await listen<{ text: string }>(EV.committed, (e) => {
      resultText.value = e.payload.text;
    }),
    await listen<{ source: string; message: string }>(EV.error, (e) => {
      error.value = e.payload;
    }),
  );
});

onBeforeUnmount(() => {
  unlisteners.forEach((u) => u());
  if (media && mediaHandler) media.removeEventListener("change", mediaHandler);
});
</script>

<template>
  <div
    class="w-full h-full flex items-center gap-4 px-5 select-none"
    :class="dark ? 'text-neutral-100' : 'text-neutral-900'"
    @click="stopFromBubble"
  >
    <div
      class="flex items-center gap-4 w-full px-5 py-3 rounded-2xl shadow-2xl border"
      :class="[
        dark ? 'bg-neutral-900/95 border-neutral-700' : 'bg-white/95 border-neutral-200',
        badge !== 'none' ? (fading ? 'opacity-0 transition-opacity duration-500' : 'opacity-100 transition-opacity duration-150') : '',
      ]"
    >
      <StatusDot :state="dotState" />

      <!-- 结果徽标态 -->
      <ResultBadge
        v-if="badge !== 'none'"
        :kind="badge"
        :text="badge === 'error' ? error?.message.split('：').pop()?.slice(0, 40) : (resultText ?? '')"
      />

      <!-- 录音/收尾态：音量条 + 实时文本 -->
      <template v-else>
        <VolumeBars :volume="volume" :active="state === 'recording'" />
        <div class="flex-1 min-w-0 h-11 overflow-hidden flex flex-col justify-center">
          <div
            v-if="fullText"
            class="text-[15px] leading-snug break-all"
            :class="dark ? 'text-neutral-100' : 'text-neutral-800'"
          >
            {{ fullText }}
          </div>
          <div
            v-else
            class="text-sm"
            :class="dark ? 'text-neutral-500' : 'text-neutral-400'"
          >
            {{ state === "committing" ? "识别中…" : "请说话…" }}
          </div>
        </div>
      </template>
    </div>
  </div>
</template>
