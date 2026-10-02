<template>
  <div class="flex items-center gap-2">
    <button
      type="button"
      class="min-w-[9.5rem] rounded-md border px-3 py-1.5 text-sm transition-colors"
      :class="
        recording
          ? 'border-red-400 bg-red-500/10 text-red-500 animate-pulse'
          : 'border-neutral-300 bg-neutral-100 text-neutral-700 hover:bg-neutral-200 dark:border-neutral-700 dark:bg-neutral-800 dark:text-neutral-200 dark:hover:bg-neutral-700'
      "
      @click="toggleRecord"
    >
      {{ recording ? "按下组合键…（Esc 取消）" : label }}
    </button>
    <span v-if="!recording" class="text-sm text-neutral-500 dark:text-neutral-400">{{ display }}</span>
    <button
      v-if="!recording && dirty"
      type="button"
      class="text-xs text-blue-500 hover:underline"
      @click="$emit('reset')"
    >
      还原
    </button>
  </div>
</template>

<script setup lang="ts">
// 组合键录制：点击 → 暂停热键引擎 → 捕获 keydown（event.code → 规范名，与 keys.rs 一致）→
// 松开任意键结束 → 恢复引擎 → emit 规范名数组。Esc 取消。
import { computed, onBeforeUnmount, ref } from "@vue/runtime-core";
import { hotkeySuspend } from "../../lib/ipc";

const props = defineProps<{ modelValue: string[]; label: string }>();
const emit = defineEmits<{ (e: "update:modelValue", v: string[]): void; (e: "reset"): void }>();

const recording = ref(false);
const dirty = ref(false);
const held = ref<string[]>([]);

const display = computed(() => (props.modelValue.length ? props.modelValue.join("+") : "未设置"));

// event.code → 规范名（与 src-tauri/src/hotkey/keys.rs 归并规则一致：左右修饰键合并）
function codeToName(code: string): string | null {
  if (code === "ControlLeft" || code === "ControlRight") return "Ctrl";
  if (code === "MetaLeft" || code === "MetaRight") return "Super";
  if (code === "ShiftLeft" || code === "ShiftRight") return "Shift";
  if (code === "AltLeft" || code === "AltRight") return "Alt";
  if (code === "Space") return "Space";
  if (code === "Enter") return "Enter";
  if (code === "Tab") return "Tab";
  const m = code.match(/^Key([A-Z])$/);
  if (m) return m[1];
  const d = code.match(/^Digit(\d)$/);
  if (d) return d[1];
  return null;
}

function onKeyDown(e: KeyboardEvent) {
  e.preventDefault();
  e.stopPropagation();
  if (e.code === "Escape") {
    finish(false);
    return;
  }
  const name = codeToName(e.code);
  if (name && !held.value.includes(name)) held.value.push(name);
}
function onKeyUp(e: KeyboardEvent) {
  e.preventDefault();
  e.stopPropagation();
  finish(true);
}

async function toggleRecord() {
  if (recording.value) {
    finish(false);
    return;
  }
  recording.value = true;
  held.value = [];
  await hotkeySuspend(true).catch(() => {});
  window.addEventListener("keydown", onKeyDown, { capture: true });
  window.addEventListener("keyup", onKeyUp, { capture: true });
}

async function finish(apply: boolean) {
  window.removeEventListener("keydown", onKeyDown, { capture: true });
  window.removeEventListener("keyup", onKeyUp, { capture: true });
  recording.value = false;
  await hotkeySuspend(false).catch(() => {});
  if (apply && held.value.length > 0) {
    emit("update:modelValue", [...held.value]);
    dirty.value = true;
  }
}

onBeforeUnmount(() => {
  if (recording.value) void finish(false);
});
</script>
