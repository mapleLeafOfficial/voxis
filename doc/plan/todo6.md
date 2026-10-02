# Todo6：全局热键引擎

## 基本信息

- **对应 PRD 模块**：M1
- **优先级**：P0
- **预估工作量**：~7 个文件
- **依赖**：todo4
- **状态**：✅ 已完成（uinput 注入全链路验证通过，见完成记录）

---

## 目标描述

evdev 直读 `/dev/input` 实现系统级组合键监听：hold 模式（按住 Ctrl+Win 说话、松开结束）与 lock 模式（Ctrl+Win+Shift toggle），驱动会话状态机。含权限自检与缺失指引。

---

## 任务清单

### Rust

- [ ] **依赖**：`evdev` crate
- [ ] **设备监听** `src-tauri/src/hotkey/evdev.rs`：
  - 枚举 `/dev/input/event*`，过滤具备键盘能力（`EV_KEY` + `KEY_A` 等基本键）的设备
  - 每设备一个阻塞读线程 → `mpsc` 汇聚 `KeyEvent{keycode, pressed}`（**只读不吞事件**，evdev 天然不阻断）
  - 设备热插拔：简化 v1 启动时枚举一次 + 通知提示（监听 udev 变更列为 P2）
- [ ] **键码映射** `src-tauri/src/hotkey/keys.rs`：evdev `Key` ↔ 名字（"Ctrl","Super","Shift","Alt","A".."Z","0".."9","Space"…），供配置与 UI 录制复用
- [ ] **状态机** `src-tauri/src/hotkey/state.rs`：
  - 维护当前按住集合 `pressed: HashSet<Key>`
  - `hold` 组合：按下集合 ⊇ hold 键集 → 触发 `start(hold)`（去抖：会话非 Idle 忽略）；任一 hold 键释放且会话因本组合而起 → `stop()`（记录 `session_source=hotkey_hold`，与托盘/点击来源区分）
  - `lock` 组合：按下瞬间集合 ⊇ lock 键集 → toggle start/stop（只在新按下边沿触发，不重复）
  - 冲突：hold 触发后用户又补按 Shift 成 lock 集合 → 仍按 hold 的松开语义处理（简单优先，备注说明）
- [ ] **权限自检** `src-tauri/src/hotkey/permission.rs` + 命令 `get_permission_status`：
  - `input` 组（可读 /dev/input/event*）、`uinput` 组（/dev/uinput 存在且可写）、`ydotoold` 存活（`/run/user/<uid>/.ydotool_socket` 或进程检测，todo7 前先报状态不阻塞）
  - 任一缺失 → 桌面通知 + `hotkey://permission` 事件（todo8 设置页消费）
- [ ] **接线**：`state.rs` 触发 SessionManager；配置热键变更 → 重建监听（todo8 接 UI，本 todo 读配置启动即可）
- [ ] DevView：显示当前按住键集合 + 触发日志（调试用）

---

## 涉及的文件

| 操作 | 文件路径 | 说明 |
|------|----------|------|
| 修改 | `src-tauri/Cargo.toml` | +evdev |
| 新增 | `src-tauri/src/hotkey/mod.rs` | 声明 |
| 新增 | `src-tauri/src/hotkey/{evdev.rs,keys.rs,state.rs,permission.rs}` | 核心 |
| 修改 | `src-tauri/src/session.rs` | 记录会话来源 source |
| 修改 | `src-tauri/src/commands/mod.rs` | +get_permission_status |
| 修改 | `src/views/DevView.vue` | 调试显示 |

---

## 验证标准

- [ ] 权限满足时：任意应用获得焦点下，**按住 Ctrl+Win 说话 → 松开** → 会话完成（气泡/日志可见）
- [ ] `Ctrl+Win+Shift` 一次开始、再一次结束
- [ ] 触发热键期间，打字、单击 Win 打开 Activities 等系统行为不受影响
- [ ] `get_permission_status` 在本机返回全 ✓（先跑 setup 把用户加入 input/uinput 组）
- [ ] 权限缺失模拟（临时移出组）→ 启动收到通知，应用不崩溃
- [ ] 空闲 CPU 为 0（阻塞读，非轮询）

## 备注

- 部署依赖：`sudo usermod -aG input,uinput guxing` + 重登录生效（**本机已执行**，桌面会话需重登录）；ydotool 完整接入在 todo7
- 笔记本多键盘设备（内建+外接）需多设备汇聚，组合键跨设备按下属边缘 case，v1 不保证

---

## 完成记录（2026-10-02）

### 交付内容
- `hotkey/keys.rs`：evdev KeyCode ↔ 规范名（左右修饰键归并 Ctrl/Super/Shift/Alt；A-Z/0-9/Space/…）
- `hotkey/evdev.rs`：枚举 /dev/input/event*（is_keyboard = KEY_A+KEY_SPACE+KEY_LEFTCTRL 能力位），每设备阻塞读线程 → mpsc 汇聚；drop 即停
- `hotkey/state.rs`：状态机（lock 优先 + hold 150ms 延迟窗口）：
  - lock 全集达成 → 立即 toggle（lock ⊇ hold，必须先判，否则 lock 必被 hold 抢触发——首版踩坑已修）
  - 仅 hold 达成 → 延迟 150ms 触发，窗口内补按出 lock 全集则取消 hold 改触发 lock
  - hold 会话开始后补按 Shift → 仍按 hold 语义（todo6 备注「简单优先」）
  - start/stop 由 spawn 到 tokio，状态机线程只做阻塞 recv_timeout(50ms)，空闲 CPU 0
- `hotkey/permission.rs`：input（试开 event*）/uinput（试写）/ydotoold（socket 存在）三项自检 + problems 指引
- `hotkey/mod.rs` start()：自检 → 读配置（hold/lock 键名）→ 监听 → 状态机；无 input 权限只报错不崩溃
- `commands::get_permission_status` + `hotkey://debug`/`hotkey://permission` 事件；DevView 热键调试面板（权限徽章 + 按住集合 + 最近触发）
- 部署：已 `usermod -aG input,uinput guxing`（重登录后桌面会话生效）

### uinput 注入全链路验证（无头，真键盘事件路径）
- hold：Ctrl↓→Super↓（150ms 窗口）→ 会话 Recording → 3s → Super↑ → stop → 会话结束 ✓
- lock：干扰 tap Ctrl×2 无误触 → Ctrl+Super+Shift 三连 → Lock 开始 → 再按一次 → 停止 ✓
- 无权限：无 input 组时引擎不启动、有 problems 指引、应用不崩 ✓
- 待实机验证：真键盘（非 uinput）下焦点应用不被干扰（evdev 只读不吞，理论无影响）、气泡随热键弹出
