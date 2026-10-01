# Todo8：设置窗口

## 基本信息

- **对应 PRD 模块**：M6
- **优先级**：P1
- **预估工作量**：~14 个文件
- **依赖**：todo6、todo7（配置项生效依赖对应模块）
- **状态**：⬜ 未开始

---

## 目标描述

完整设置主窗口：账户 / 快捷键 / 识别 / 上屏 / 外观 / 关于 分组页签，所有 PRD §3.1 配置项可视化编辑并即时生效；含权限自检面板与 API Key 连通性测试。

---

## 任务清单

### 前端

- [ ] `src/views/SettingsView.vue`：左侧页签导航 + 右侧表单（或上下分组，选简洁做法）
- [ ] **账户页**：API Key 输入（脱敏 `••••` + 眼睛切换 + 「测试」按钮）、说明环境变量优先级
- [ ] **快捷键页**：
  - hold / lock / polish(manual) 三组组合键录制组件 `HotkeyRecorder.vue`：点击「录制」→ 按下组合 → 显示 `Ctrl+Super` 等 → 保存
  - 键名来自 `keys.rs` 映射（新增命令 `list_key_names`）
- [ ] **识别页**：模型（下拉+自定义）、断句静音 ms（滑条 500~3000）、语义断句开关、语言（自动/zh/en）、最长录音秒数
- [ ] **上屏页**：自动粘贴/仅复制单选、输入设备下拉（`list_input_devices`）
- [ ] **外观页**：气泡位置（bottom_center/四角）、主题三选、开机自启开关
- [ ] **权限面板**（快捷键页内嵌或独立卡片）：消费 `get_permission_status`，input/uinput/ydotoold 三项 ✓/✗，✗ 项展示修复命令（可复制）
- [ ] 全部变更走 `set_config` 即时保存；重启后回显

### Rust

- [ ] `set_config` 增强：diff 检测 → 热键变更重载 hotkey 引擎、音频设备变更标记下次会话生效、主题变更发事件
- [ ] 命令：`test_api_key()`（起一次 1s 假会话或直接 WS 握手验证）、`list_key_names()`、`show_main_window()`
- [ ] 开机自启：写/删 `~/.config/autostart/voxis.desktop`（`Exec=/usr/bin/voxis --minimized` 占位，todo10 打包后校正路径）

---

## 涉及的文件

| 操作 | 文件路径 | 说明 |
|------|----------|------|
| 新增 | `src/views/SettingsView.vue` | 主设置页 |
| 新增 | `src/components/settings/{AccountTab.vue,HotkeyTab.vue,AsrTab.vue,CommitTab.vue,AppearanceTab.vue,PermissionPanel.vue,HotkeyRecorder.vue}` | 页签组件 |
| 修改 | `src/router.ts` | /settings 路由 |
| 修改 | `src-tauri/src/commands/mod.rs` | 新命令 |
| 修改 | `src-tauri/src/hotkey/state.rs` | 配置热重载 |
| 新增 | `src-tauri/src/autostart.rs` | 自启管理 |

---

## 验证标准

- [ ] 每个配置项改完立即持久化（查看 config.json），重启应用回显正确
- [ ] 改 hold 快捷键为 `Alt+S` → 立即按新组合可触发（旧组合失效）
- [ ] 「测试」按钮对正确 Key 返回成功、错误 Key 给出失败原因
- [ ] 权限面板与实际状态一致；复制修复命令可执行
- [ ] 开机自启开关生成/删除 desktop 文件，GNOME「 tweaks/启动应用」可见
- [ ] 输入设备切换后下一会话生效

## 备注

UI 风格 Tailwind 简洁卡片式，跟随系统深浅色；不做自定义主题色。
