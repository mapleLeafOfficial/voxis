import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
import "./style.css";
import { listen } from "@tauri-apps/api/event";
import { EV } from "./lib/ipc";

// 托盘「设置…/显示主窗口」跳转路由
void listen<string>(EV.navigate, (e) => {
  void router.push(e.payload).catch(() => {});
});

createApp(App).use(router).mount("#app");
