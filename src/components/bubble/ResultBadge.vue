<script setup lang="ts">
// 结果徽标：成功（已识别 N 字）/ 空文本（未识别）/ 错误摘要；由父级控制淡出
import { computed } from "vue";

const props = defineProps<{
  kind: "pasted" | "copied" | "empty" | "error";
  text?: string;
}>();

const label = computed(() => {
  if (props.kind === "error") return props.text || "出错了";
  if (props.kind === "empty") return "未识别到语音";
  const n = (props.text || "").trim().length;
  if (props.kind === "copied") return `⧉ 已复制 ${n} 字`;
  return `✓ 已上屏 ${n} 字`;
});
</script>

<template>
  <div
    class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm whitespace-nowrap overflow-hidden"
    :class="{
      'bg-emerald-500/15 text-emerald-600 dark:text-emerald-300': kind === 'pasted',
      'bg-sky-500/15 text-sky-600 dark:text-sky-300': kind === 'copied',
      'bg-neutral-500/15 text-neutral-500': kind === 'empty',
      'bg-red-500/15 text-red-600 dark:text-red-300': kind === 'error',
    }"
  >
    <span class="truncate max-w-[420px]">{{ label }}</span>
  </div>
</template>
