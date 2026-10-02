<template>
  <div class="rounded-lg border border-neutral-200 dark:border-neutral-700 p-4 space-y-2">
    <h3 class="text-sm font-semibold text-neutral-700 dark:text-neutral-200">权限自检</h3>
    <div v-if="!status" class="text-sm text-neutral-400">检测中…</div>
    <template v-else>
      <div v-for="item in rows" :key="item.key" class="flex items-center justify-between text-sm">
        <span class="flex items-center gap-2">
          <span :class="item.ok ? 'text-emerald-500' : 'text-red-500'">{{ item.ok ? "✓" : "✗" }}</span>
          <span class="text-neutral-600 dark:text-neutral-300">{{ item.label }}</span>
        </span>
        <button
          v-if="!item.ok && item.cmd"
          type="button"
          class="text-xs text-blue-500 hover:underline"
          title="复制修复命令"
          @click="copy(item.cmd, item.key)"
        >
          {{ copied === item.key ? "已复制" : "复制修复命令" }}
        </button>
      </div>
      <p
        v-for="p in status.problems"
        :key="p"
        class="text-xs text-neutral-400 dark:text-neutral-500 break-all"
      >
        {{ p }}
      </p>
    </template>
  </div>
</template>

<script setup lang="ts">
// 权限自检面板：input / uinput / ydotoold 三项 + 修复命令复制
import { computed, onMounted, ref } from "@vue/runtime-core";
import { getPermissionStatus, type PermissionStatus } from "../../lib/ipc";

const status = ref<PermissionStatus | null>(null);
const copied = ref("");

const FIX: Record<string, { label: string; cmd: string }> = {
  input_ok: { label: "/dev/input 读权限（热键必需）", cmd: "sudo usermod -aG input $USER  # 然后重新登录" },
  uinput_ok: { label: "/dev/uinput 写权限（注入粘贴）", cmd: "sudo usermod -aG uinput $USER  # 然后重新登录" },
  ydotoold_ok: { label: "ydotoold 守护进程（注入粘贴）", cmd: "sudo ydotoold -p $XDG_RUNTIME_DIR/.ydotool_socket -P 0660 -o $(id -u):$(id -g) &" },
};

interface Row { key: string; ok: boolean; label: string; cmd: string }

const rows = computed<Row[]>(() => {
  const s = status.value;
  if (!s) return [];
  return (Object.keys(FIX) as (keyof PermissionStatus)[])
    .filter((k) => typeof s[k] === "boolean")
    .map((k) => ({ key: k, ok: s[k] as boolean, label: FIX[k].label, cmd: FIX[k].cmd }));
});

async function copy(cmd: string, key = "") {
  try {
    await navigator.clipboard.writeText(cmd);
    copied.value = key;
    setTimeout(() => (copied.value = ""), 1500);
  } catch {
    /* 剪贴板不可用则静默 */
  }
}

onMounted(async () => {
  status.value = await getPermissionStatus().catch(() => null);
});
</script>
