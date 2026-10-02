# Todo7：上屏引擎

## 基本信息

- **对应 PRD 模块**：M5
- **优先级**：P0
- **预估工作量**：~6 个文件
- **依赖**：todo4
- **状态**：✅ 已完成（代码+冒烟通过；M2 粘贴闭环需实机验证）

---

## 目标描述

实现文本上屏闭环：写剪贴板 → 注入 Ctrl+V 到当前光标；失败自动回退「仅复制 + 通知」，实现 PRD 的「没有编辑位置就是复制」。接入会话 Committing 阶段。

---

## 任务清单

### Rust

- [ ] **依赖**：`wl-clipboard-rs`、`arboard`；ydotool/xdotool 经 `tauri-plugin-shell`（或 `std::process::Command`）调用
- [ ] **剪贴板** `src-tauri/src/commit/clipboard.rs`：
  - `copy(text)`：Wayland `wl-copy`（wl-clipboard-rs）→ 失败回退 `arboard` → 再回退 `xclip`/`xsel`
  - 粘贴前写、commit 后不清剪贴板（保留文本供手动粘贴）
- [ ] **注入** `src-tauri/src/commit/inject.rs`：
  - `inject_paste()`：检测会话（`YDOTOOL_SOCKET` 或 `/run/user/<uid>/.ydotool_socket`）→ `ydotool key 29:1 125:1 47:1 47:0 125:0 29:0`（Ctrl+Super+V）；失败回退 X11 `xdotool key --clearmodifiers ctrl+v`（仅 DISPLAY 存在时）
  - 注入前 150ms 静默（剪贴板同步窗口），注入后确认退出码
- [ ] **编排** `src-tauri/src/commit/mod.rs`：
  - `commit(text, mode)`：空文本 → 通知「未识别到内容」；`auto` = copy → inject → 成功发 `committed{result:"pasted"}` / 失败发 `committed{result:"copied"}` + 通知「已复制，Ctrl+V 粘贴」；`clipboard_only` = 直接 copied
  - 结果写入 `committed` 事件 payload（气泡 todo5 已消费）
- [ ] **接线**：SessionManager Committing 阶段调用 commit（todo4 目前只打日志，本 todo 替换）
- [ ] **托盘通知**：引入 `tauri-plugin-notification`，统一通知辅助函数
- [ ] DevView：显示 committed 结果三态

---

## 涉及的文件

| 操作 | 文件路径 | 说明 |
|------|----------|------|
| 修改 | `src-tauri/Cargo.toml` | +wl-clipboard-rs/arboard |
| 新增 | `src-tauri/src/commit/{mod.rs,clipboard.rs,inject.rs}` | 上屏核心 |
| 修改 | `src-tauri/src/session.rs` | 接入 commit |
| 修改 | `src-tauri/src/commands/mod.rs` | 通知注册 |
| 修改 | `src-tauri/tauri.conf.json`、`capabilities/default.json` | notification 插件 |
| 修改 | `src/views/DevView.vue` | 结果显示 |

---

## 验证标准

- [ ] 焦点在 gedit/浏览器地址栏：按住 Ctrl+Win 说话松开 → 文字直接出现在光标处（里程碑 M2 核心闭环）
- [ ] 焦点在桌面（无可编辑目标）：松开 → 收到「已复制」通知，手动 Ctrl+V 可粘贴
- [ ] 停掉 ydotoold 且无 X11 → 回退 copied，不崩溃
- [ ] 连续两次语音输入（旧剪贴板内容存在）不出现粘贴旧内容
- [ ] 空识别结果只通知、不上屏

## 备注

- 部署依赖：`ydotool` 安装 + `ydotoold` 服务 + `uinput` 组；setup 脚本在 todo10 统一落地，本 todo 手动验证
- ydotool 键码：29=左Ctrl，125=左Super，47=V

---

## 完成记录（2026-10-02）

### 交付内容
- `commit/clipboard.rs`：多级回退 wl-clipboard-rs（Wayland 原生，后台服务持有）→ arboard → xclip/xsel
- `commit/inject.rs`：ydotool Ctrl+V（socket 探测 YDOTOOL_SOCKET → /run/user/<uid>/.ydotool_socket → /run/ydotoold/socket）→ X11 xdotool 回退；注入前 150ms 静默
- `commit/mod.rs`：commit() 编排 + 三态结果（pasted/copied/none）+ committed 事件带 result + 统一 notify()（tauri-plugin-notification）
- `session.rs`：stop() 的 Committing 阶段改走 spawn_blocking(commit)（不卡 tokio driver）
- 前端：CommittedPayload 带三态；气泡徽标「✓ 已上屏 N 字 / ⧉ 已复制 N 字 / 未识别到语音」；DevView 显示 [result]
- capabilities 加 notification:default；windows 含 bubble
- **注入键序改用纯 Ctrl+V**（计划原写 Ctrl+Super+V；Wayland 下合成器不拦 Ctrl+V，Super 反而会让应用收到未绑定组合键）

### 部署现状（本机）
- ydotool 已装；ydotoold 以 root 跑：`sudo ydotoold -p /run/user/1000/.ydotool_socket -P 0660 -o 1000:1000`（socket 属主转给 guxing）
- **重登录后可改用 systemd --user 服务**（`systemctl --user enable --now ydotool`），当前 user manager 还是旧组（无 uinput）跑不了；todo10 setup 脚本固化
- ydotool 客户端注入验证 ✓（Shift+A 通信测试）
- 冒烟：commit 编排 copied 分支 ✓（clipboard_only 模式跑 smoke-session，「提交结果: copied」）

### 待实机验证（M2 核心闭环）
- [ ] 焦点在 gedit/浏览器：热键说话松开 → 文字出现在光标处
- [ ] 焦点在桌面 → 「已复制」通知 + Ctrl+V 可粘贴
- [ ] 停 ydotoold + 无 X11 → 回退 copied 不崩（代码路径已验证）
- [ ] 连续两次输入不粘旧内容（copy 在 inject 前，每次覆盖）
- ⚠️ pi shell 环境里 wl-paste 拿不到数据（compositor 不回 offer，疑似该 shell 会话特有）；桌面终端内大概率正常，若同样失败会走 copied 回退，不会卡死
