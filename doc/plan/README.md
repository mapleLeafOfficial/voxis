# Voxis 开发计划总览

> PRD：[../PRD.md](../PRD.md) · 流程：模块化开发，每个 todo 完成后需用户确认再继续
> 当前状态：**todo3 已完成，待确认后开始 todo4**

## Todo 列表与进度

| # | 模块 | 对应 PRD | 优先级 | 依赖 | 状态 |
|---|------|----------|--------|------|------|
| 1 | 项目骨架（Tauri2+Vue3+配置+单实例+日志） | 开发约束 / M6部分 / M8部分 | P0 | — | ✅ |
| 2 | 录音引擎（cpal/PipeWire） | M2 | P0 | todo1 | ✅ |
| 3 | 流式 ASR 客户端（DashScope WS） | M3 | P0 | todo1 | ✅ |
| 4 | 会话状态机 + IPC 整合 | M2+M3 串联 | P0 | todo2, todo3 | ✅ |
| 5 | 气泡浮窗（实时预览 UI） | M4 | P0 | todo4 | ✅ |
| 6 | 全局热键引擎（evdev hold/lock） | M1 | P0 | todo4 | ✅ |
| 7 | 上屏引擎（剪贴板+注入粘贴） | M5 | P0 | todo4 | ✅ |
| 8 | 设置窗口 | M6 | P1 | todo6, todo7 | ⬜ |
| 9 | 整理文字（qwen-flash 后处理） | M7 | P1 | todo7 | ⬜ |
| 10 | 托盘/自启 + 打包安装 | M8+M9 | P1 | 全部 | ⬜ |

> 详细计划见 [todo1.md](todo1.md) ~ [todo10.md](todo10.md)

## 开发顺序

```
todo1 骨架 ─┬─→ todo2 录音 ──┬─→ todo4 会话状态机 ─┬─→ todo5 气泡
            └─→ todo3 ASR ───┘                     ├─→ todo6 热键 ──→ todo8 设置
                                                   ├─→ todo7 上屏 ──→ todo9 整理
                                                   └──────────────→ todo10 托盘/打包
```

## 里程碑

- **M1（可听）**：todo4 完成 —— 能用开发页按钮录音并实时出字
- **M2（可用）**：todo6+todo7 完成 —— 快捷键全流程闭环（微信式核心体验）
- **M3（可发布）**：todo10 完成 —— 安装脚本 + 打包产物

## 约定

- 每个 todo 完成后：更新对应 todo 文件勾选 + 本 README 状态列 + 提交 git
- 验证均在 Arch + GNOME + Wayland 实机进行
