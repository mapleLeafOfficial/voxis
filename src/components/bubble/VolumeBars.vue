<script setup lang="ts">
// 音量跳动条：5 根，由 volume(0~1) 驱动高度，不同系数错落跳动（父级节流 ~50ms 更新）
import { computed } from "vue";

const props = defineProps<{ volume: number; active: boolean }>();

// 每根条的响应系数（视觉错落感）
const COEFS = [0.45, 0.75, 1.0, 0.65, 0.85];

const heights = computed(() =>
  COEFS.map((c, i) => {
    if (!props.active) return 18;
    const v = Math.min(1, Math.max(0, props.volume));
    // 底噪抬升 + 波动错落：相邻条交替相位
    const wobble = 0.75 + 0.25 * Math.sin(i * 1.9 + props.volume * 12);
    return Math.round(18 + v * c * wobble * 82);
  }),
);
</script>

<template>
  <div class="flex items-end gap-[3px] h-7 shrink-0" aria-hidden="true">
    <div
      v-for="(h, i) in heights"
      :key="i"
      class="w-[3px] rounded-full transition-[height] duration-100 ease-out"
      :class="active ? 'bg-red-500' : 'bg-neutral-400'"
      :style="{ height: `${h}%` }"
    />
  </div>
</template>
