# Todo5：气泡浮窗

## 基本信息

- **对应 PRD 模块**：M4
- **优先级**：P0
- **预估工作量**：~8 个文件
- **依赖**：todo4
- **状态**：⬜ 未开始

---

## 目标描述

实现录音气泡浮窗：置顶、无边框、透明、不抢焦点，实时展示 partial 文字与音量动画；松开后显示上屏/复制结果并淡出。这是微信体验的「跟手感」所在。

---

## 任务清单

### Tauri 窗口

- [ ] `tauri.conf.json` 新增 `bubble` 窗口：`decorations:false, transparent:true, always_on_top:true, skip_taskbar:true, focusable:false, visible:false, 520×88`
- [ ] `src-tauri/src/bubble.rs`：`show_bubble(pos)`（按 `ui.bubble_pos` 计算坐标：bottom_center 默认，屏幕工作区上方 120px；支持四角）/ `hide_bubble()` / 状态文本更新（用 emit 事件驱动，不直接操作 DOM）

### 前端

- [ ] `src/views/BubbleView.vue` + `src/components/bubble/`：
  - 状态点：Idle 隐藏 / Recording 呼吸红点 / Committing 黄点 / Done 绿点 / Error 红叉
  - 音量条：订阅 `session://volume`，3~5 根跳动条动画（CSS，rAF 节流）
  - 文本区：partial 文本实时替换滚动到底部；SentenceEnd 后保留累积
  - 结果态：`committed` 事件 → 「✓ 已上屏 N 字」/「⧉ 已复制」→ 1.5s 淡出（css transition）→ hide
  - 错误态：显示简短摘要，2s 后淡出
  - 点击气泡 = `stop_session()`（`focusable:false` 下点击仍可命中，验证之；若收不到点击则改用 `accepts_first_mouse`/透明度方案兜底并在备注记录）
- [ ] 主题：`ui.theme` light/dark/system（`prefers-color-scheme`）
- [ ] DevView 会话触发改为驱动气泡（DevView 保留触发按钮，展示移到气泡）

### Rust 接线

- [ ] SessionManager：start → show_bubble，结束+淡出动画时长后 → hide_bubble；events 已就绪

---

## 涉及的文件

| 操作 | 文件路径 | 说明 |
|------|----------|------|
| 修改 | `src-tauri/tauri.conf.json` | +bubble 窗口 |
| 新增 | `src-tauri/src/bubble.rs` | 气泡窗口管理 |
| 修改 | `src-tauri/src/session.rs` | 气泡联动 |
| 修改 | `src-tauri/src/lib.rs` | 注册 |
| 新增 | `src/views/BubbleView.vue` | 气泡根组件 |
| 新增 | `src/components/bubble/{VolumeBars.vue,StatusDot.vue,ResultBadge.vue}` | 子组件 |
| 修改 | `src/router.ts`、`src/views/DevView.vue` | 路由 + 触发改造 |

---

## 验证标准

- [ ] 会话中气泡出现在屏幕底部居中，partial 文字陆续出现
- [ ] **关键**：气泡出现/更新全程，正在打字的窗口焦点不被打断（可先开 gedit 连续打字验证）
- [ ] 松开后 2s 内显示结果徽标并淡出，气泡隐藏
- [ ] 点击气泡能结束会话
- [ ] GNOME 深色/浅色切换主题跟随
- [ ] 气泡位置改四角后重启生效

## 备注

Wayland 下窗口定位：Tauri `set_position` 用逻辑像素，多显示器先只保证主屏。
