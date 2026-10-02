# Todo10：托盘/自启 + 打包安装

## 基本信息

- **对应 PRD 模块**：M8 + M9
- **优先级**：P1
- **预估工作量**：~12 个文件
- **依赖**：全部前置 todo
- **状态**：✅ 已完成（v0.1.0 打包发布）

---

## 目标描述

完成托盘常驻（两态图标、点击说话、右键菜单）、开机自启与最小化启动，产出 setup.sh 安装脚本与打包产物（deb/AppImage），编写 README。交付可发布版本。

---

## 任务清单

### 托盘（Rust）

- [ ] 引入 `tray-icon`（Tauri 2 内置 tray API）
- [ ] 两态图标：空闲（麦克风）/ 录音中（红色高亮），SVG 转 PNG 资源放 `src-tauri/icons/tray/`
- [ ] **左键点击 = 点击麦克风入口**：Idle → `start_session(lock=true)`；Recording → `stop_session()`
- [ ] 右键菜单：打开设置 / 开始·停止（状态文案切换）/ 检查权限（跑一遍 get_permission_status，缺失弹通知）/ 退出（含清理：会话中止、日志 flush）
- [ ] 会话状态变更 → 图标与菜单文案同步

### 自启与启动行为

- [ ] CLI 参数：`--minimized`（不显示主窗口，仅托盘）——autostart desktop 使用
- [ ] autostart desktop 模板安装/更新（与 todo8 的开关打通，Exec 路径用打包后的实际路径）
- [ ] 启动时权限自检：缺失 → 通知 + 不注册热键（避免假死感）

### 打包与安装

- [ ] `scripts/setup.sh`：
  - 检测/安装系统依赖：`ydotool`、`wl-clipboard`、`libappindicator`（托盘需要，GNOME 需 AppIndicator 扩展，检测并提示 `gnome-shell-extension-appindicator`）
  - `usermod -aG input,uinput $USER`（已在组则跳过）+ 提示需重新登录
  - `systemctl enable --now ydotoold`（含 udev uinput 权限说明）
  - 全流程自检输出 ✓/✗
- [ ] `scripts/build.sh`：`bun install && bun run tauri build` → 产出 `.deb` / AppImage / 裸二进制，版本号从 Cargo.toml 读取
- [ ] README.md：项目简介、微信功能对照表、安装（setup.sh + 包两种）、快捷键用法（hold/lock/托盘）、配置说明、故障排查（权限、托盘不显示、粘贴失败、Wayland 定位）、隐私声明（音频不落盘）
- [ ] git 提交整理 + 打 tag `v0.1.0`

---

## 涉及的文件

| 操作 | 文件路径 | 说明 |
|------|----------|------|
| 修改 | `src-tauri/src/tray.rs`（新增） | 托盘逻辑 |
| 新增 | `src-tauri/icons/tray/{idle.png,recording.png}` | 托盘图标 |
| 修改 | `src-tauri/src/lib.rs`、`main.rs` | --minimized、托盘注册 |
| 新增 | `src-tauri/assets/voxis.desktop` | autostart 模板 |
| 新增 | `scripts/{setup.sh,build.sh}` | 安装/构建 |
| 重写 | `README.md` | 完整文档 |
| 修改 | `doc/plan/README.md` | 进度收尾 |

---

## 验证标准

- [ ] 托盘常驻显示，录音中图标变红；左键点击开始/结束会话全程可用（微信第三种触发方式）
- [ ] `voxis --minimized` 开机进入仅托盘模式，快捷键照常工作
- [ ] 在干净依赖清单下跑 `setup.sh` 后，重登录，全功能可用（热键+粘贴+托盘）
- [ ] `build.sh` 产出 deb/AppImage，安装后 `voxis` 可启动、菜单/图标正常
- [ ] 退出菜单能真正退出（进程消失、ydotoold 不受影响）
- [ ] README 可让新用户 15 分钟内从零跑通

## 备注

GNOME 托盘依赖 AppIndicator 扩展（此前已装 `gnome-shell-extension-appindicator`）；AppImage 在 Wayland 下托盘可能有兼容问题，README 注明 deb 为推荐安装方式。

---

## 完成记录（2026-10-02）

### 交付内容
- `tray.rs`：Tauri 2 tray-icon（features: tray-icon + image-png）；两态图标（ImageMagick 绘制麦克风 idle 灰/录音红，
  include_bytes 嵌入）；左键点击 toggle 会话（Idle→start(lock=true)，进行中→stop）；右键菜单
  （显示主窗口/设置/开关（文案随状态）/检查权限（通知汇总）/退出（录音中先走 commit 管道再退出））；
  SessionManager::emit_state → tray::update_state 同步图标与菜单文案
- `--minimized`：debug 构建不弹主窗（release 本就 visible:false）；托盘/快捷键照常
- `autostart.rs`：Exec 改 current_exe() 实际路径 + --minimized
- 设置页主题/路由：`ui://navigate` 事件 → 托盘「设置…」跳转 /settings（main.ts 监听）
- `scripts/setup.sh`：包安装（ydotool/wl-clipboard/appindicator 检测）+ input/uinput 组 + 
  ydotoold-voxis systemd 服务（模板按实际 uid 生成，socket 落 /run/user/<uid>/.ydotool_socket）+ 自检 ✓/✗
- `scripts/build.sh`：bun install + tauri build，产物清单输出
- README 重写：功能对照/安装/快捷键/配置/故障排查/隐私/开发

### 验证
- cargo/vue-tsc/vite 全零警告零错误；运行时冒烟托盘无错误日志
- release `--minimized`：无窗口、仅托盘、热键引擎正常监听 ✓
- 打包产物：deb 5.99MiB / rpm / AppImage 102MiB / 裸二进制 15MB ✓
- 待实机：托盘菜单点击各项、开机自启 GNOME 生效、deb 安装后的新用户完整流程
