# Todo1：项目骨架

## 基本信息

- **对应 PRD 模块**：开发约束 / M6（配置层）/ M8（单实例部分）
- **优先级**：P0
- **预估工作量**：~20 个文件（大部分为脚手架生成）
- **依赖**：无
- **状态**：⬜ 未开始

---

## 目标描述

搭建 Tauri 2 + Vue 3 + TS + Vite + Tailwind 项目骨架，跑通 `bun tauri dev`；实现配置层（config.json 读写、0600 权限、默认值）、tracing 日志、单实例约束。为后续所有模块提供地基。

---

## 任务清单

### Rust（src-tauri/）

- [ ] **脚手架**：初始化 Tauri 2 项目（`create-tauri-app` 或手动），Cargo.toml 引入依赖：`tokio`（full）、`serde`/`serde_json`、`tracing`/`tracing-subscriber`/`tracing-appender`、`tauri-plugin-single-instance`、`tauri-plugin-shell`（后续 ydotool 调用备用）
  - `src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src-tauri/capabilities/default.json`
- [ ] **配置模块** `src-tauri/src/config.rs`：
  - `Config` 结构体，字段与 PRD §3.1 完全一致，`Default` 实现
  - `load()`（不存在则写默认）/ `save()`（0600 权限，先写临时文件再 rename 原子替换）
  - 路径：`~/.config/voxis/config.json`
- [ ] **日志模块** `src-tauri/src/logging.rs`：tracing → 控制台 + `~/.local/share/voxis/voxis.log` 滚动文件，级别从配置读取
- [ ] **应用状态** `src-tauri/src/state.rs`：`AppState { config: RwLock<Config>, app_handle }`，managed state
- [ ] **基础命令** `src-tauri/src/commands/mod.rs`：`get_config` / `set_config`（版本校验宽松，未知字段忽略）
- [ ] **单实例**：注册 `tauri-plugin-single-instance`，二次启动 → 显示主窗口
- [ ] `lib.rs` 串起 setup 流程：日志 → 配置 → 状态 → 插件 → 命令注册

### 前端（src/）

- [ ] Vue3 + TS + Vite + Tailwind 脚手架（bun 包管理），目录：`src/views/`、`src/components/`、`src/lib/`（IPC 封装 `invoke`/`listen` 的类型安全包装）
- [ ] `src/views/DevView.vue`：临时开发面板（后续 todo 用来手动触发各模块）
- [ ] 路由：`/` → DevView（`/settings` 占位）
- [ ] `tauri.conf.json`：主窗口 `main`（启动隐藏，宽度 900×650，`visible: false`，仅托盘/命令唤起；todo10 前先给 DevView 一个显式显示入口 `show_main_window` 命令）

---

## 涉及的文件

| 操作 | 文件路径 | 说明 |
|------|----------|------|
| 新增 | `src-tauri/Cargo.toml` | 依赖清单 |
| 新增 | `src-tauri/tauri.conf.json` | 窗口/插件/打包配置 |
| 新增 | `src-tauri/capabilities/default.json` | 权限 |
| 新增 | `src-tauri/src/{main.rs,lib.rs}` | 入口 |
| 新增 | `src-tauri/src/{config.rs,logging.rs,state.rs}` | 地基模块 |
| 新增 | `src-tauri/src/commands/mod.rs` | 基础 IPC |
| 新增 | `package.json`、`vite.config.ts`、`tailwind.config.js`、`tsconfig.json` | 前端工程 |
| 新增 | `src/{main.ts,App.vue,router.ts}` | 前端入口 |
| 新增 | `src/views/DevView.vue`、`src/lib/ipc.ts` | 开发面板与 IPC 封装 |
| 新增 | `.gitignore`、`README.md`（占位） | 工程 |

---

## 验证标准

- [ ] `bun install && bun run tauri dev` 编译通过，窗口可显隐
- [ ] 首次运行自动生成 `~/.config/voxis/config.json`（权限 `-rw-------`），内容为默认值
- [ ] `~/.local/share/voxis/voxis.log` 有日志输出
- [ ] 二次启动不产生新进程，且唤起已有实例的主窗口
- [ ] DevView 调 `get_config`/`set_config` 往返成功

## 备注

（开发中记录）
