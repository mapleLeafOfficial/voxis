# Voxis Windows 版打包指南（v0.2）

> 在 **Windows 10/11 x64** 机器上完成构建。Linux 侧已完成全部平台代码（todo1~4），
> Windows 上只需装工具链 → 拉代码 → 一条命令出安装包。

## 一、安装工具链

### 1. Rust（MSVC 工具链）

1. 装 **Visual Studio Build Tools**（只装 C++ 构建，无需完整 VS）：
   https://visualstudio.microsoft.com/visual-cpp-build-tools/
   安装器里勾选 **「使用 C++ 的桌面开发」**（含 MSVC + Windows SDK）
2. 装 Rust（默认 MSVC host）：
   https://rustup.rs/ → 下载 `rustup-init.exe` 一路回车

验证（PowerShell）：

```powershell
cargo --version          # 1.8x+
rustc --version
```

### 2. Node / bun（任选其一，推荐 bun）

```powershell
# bun（推荐）
powershell -c "irm bun.sh/install.ps1 | iex"
# 或 Node.js LTS: https://nodejs.org/
```

### 3. WebView2 Runtime

- **Win11 自带**，无需操作
- Win10 缺失时装 Evergreen Runtime：https://developer.microsoft.com/microsoft-edge/webview2/

## 二、拿代码并构建

```powershell
git clone git@github.com:mapleLeafOfficial/voxis.git
cd voxis

bun install              # 或 npm install
bun run tauri build      # 或 npm run tauri build
```

首次构建会自动下载 WiX（msi 用）与 NSIS 工具链，耐心等。

## 三、产物位置

```
src-tauri\target\release\
├── voxis.exe                                   # 裸二进制（可直接运行）
└── bundle\
    ├── nsis\Voxis_0.2.0_x64-setup.exe          # NSIS 安装器（推荐分发这个）
    └── msi\Voxis_0.2.0_x64_en-US.msi           # MSI 安装包
```

安装器装完开始菜单出现 **Voxis**；设置页可开「开机自启」（写注册表 `HKCU\...\Run`）。

## 四、开发模式调试（可选）

```powershell
bun run tauri dev
```

> 需要环境变量 `QWEN_API_KEY`（与 Linux 相同，读自 `%APPDATA%\voxis\config.json`
> 的 `asr.api_key`——首次启动设置页里填也行）。

## 五、首次运行须知

- **SmartScreen 提示**：安装包未做代码签名，双击安装时 Windows 可能弹
  「已保护你的电脑」→ 点 **更多信息 → 仍要运行**（仅首次）
- **热键默认值（Windows 与 Linux 不同）**：
  - 按住说话：`Ctrl+Alt`
  - 常听（lock）：`Ctrl+Alt+Shift`
  - 整理剪贴板（manual polish）：`Ctrl+Alt+O`
  - 均可在设置页改。刻意避开 `Win` 键（按住/松开涉及开始菜单等系统行为）

## 六、已知边界（README 故障排查对应）

| 现象 | 原因 |
|---|---|
| 管理员权限窗口（任务管理器等）里热键/粘贴无效 | UIPI 隔离，普通进程无法注入提权窗口——以管理员身份运行 voxis 可解 |
| 钩子被安全软件拦截 | 个别软（如某些输入法管理/键盘工具）会干扰 LL 钩子，加白名单 |
| 全屏游戏/视频里气泡不显示 | 置顶窗口可被独占全屏覆盖，属预期 |
| 安装了中文输入法时粘贴 | Ctrl+V 在常规窗口直接粘贴文本，与输入法不冲突 |

## 七、版本发布

改 `src-tauri/Cargo.toml` 与 `package.json` 的版本号 → 重新构建 → 打 tag：

```powershell
git tag v0.2.0
git push origin v0.2.0
```

把 `bundle\nsis\*-setup.exe` 上传 GitHub Release 即完成分发。
