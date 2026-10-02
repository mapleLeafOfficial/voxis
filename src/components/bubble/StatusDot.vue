<script setup lang="ts">
// 会话状态点：Recording 呼吸红点 / Committing 琥珀 / Error 红叉；Idle 不渲染
defineProps<{ state: "idle" | "recording" | "committing" | "error" }>();
</script>

<template>
  <div v-if="state !== 'idle'" class="relative flex items-center justify-center w-5 h-5 shrink-0">
    <!-- 呼吸扩散环（仅录音） -->
    <span v-if="state === 'recording'" class="absolute inline-flex w-full h-full rounded-full bg-red-500/40 animate-ping" />
    <span
      class="relative inline-flex rounded-full"
      :class="{
        'w-3 h-3 bg-red-500': state === 'recording',
        'w-3 h-3 bg-amber-400': state === 'committing',
        'w-4 h-4 text-red-500': state === 'error',
      }"
    />
    <!-- 错误红叉覆盖 -->
    <svg v-if="state === 'error'" class="absolute w-4 h-4 text-red-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
      <path d="M6 6l12 12M18 6L6 18" stroke-linecap="round" />
    </svg>
  </div>
</template>
