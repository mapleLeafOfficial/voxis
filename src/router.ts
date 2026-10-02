import { createRouter, createWebHashHistory } from "vue-router";
import DevView from "./views/DevView.vue";

// Tauri 打包后以 asset 协议加载，必须用 hash 模式
const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "dev", component: DevView },
    {
      path: "/bubble",
      name: "bubble",
      // 气泡浮窗（由 bubble 窗口加载 index.html#/bubble）
      component: () => import("./views/BubbleView.vue"),
    },
    {
      path: "/settings",
      name: "settings",
      // todo8 实现完整设置页，此处占位
      component: () => import("./views/SettingsPlaceholder.vue"),
    },
  ],
});

export default router;
