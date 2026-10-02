// Windows 热键源：SetWindowsHookExW(WH_KEYBOARD_LL) 低级键盘钩子。
// 专用线程装钩子 + 消息循环（LL 钩子回调要求线程有消息泵）；事件转规范名 RawKeyEvent，
// 与 Linux evdev 源同构 → HotkeyEngine 状态机零改动。
// 细节：
// - 回调内只做映射与入队（系统对 LL 回调有限时，超时会绕过钩子），状态机在独立线程跑；
// - 过滤 LLKHF_INJECTED（SendInput 合成键，防注入的 Ctrl+V 回环进状态机）；
// - 停止：主线程 PostThreadMessageW(WM_QUIT) 让 GetMessageW 返回 → 卸钩子。
#![allow(clippy::upper_case_acronyms)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, OnceLock};

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, KBDLLHOOKSTRUCT, KBDLLHOOKSTRUCT_FLAGS, MSG, WH_KEYBOARD_LL,
    WM_QUIT,
};

use super::monitor::MonitorHandle;
use super::RawKeyEvent;

/// 钩子回调 → 事件通道（LL 回调是纯函数指针，无用户数据，用全局单例传）
static TX: OnceLock<Sender<RawKeyEvent>> = OnceLock::new();
/// 钩子线程 id（停引擎用）
static HOOK_THREAD_ID: OnceLock<u32> = OnceLock::new();
/// 停止标志（quitter 调用方置位后再发 WM_QUIT，防竞态重复装钩）
static STOPPED: AtomicBool = AtomicBool::new(false);

/// 启动钩子线程，返回句柄与"设备数"（恒 1，接口与 evdev 对齐）
pub fn spawn_monitors(tx: Sender<RawKeyEvent>) -> (MonitorHandle, usize) {
    let _ = TX.set(tx);
    STOPPED.store(false, Ordering::Relaxed);

    let stop = Arc::new(AtomicBool::new(false));
    let handle = std::thread::Builder::new()
        .name("voxis-win-hook".into())
        .spawn(move || unsafe { hook_thread() })
        .expect("创建钩子线程失败");

    let stop2 = stop.clone();
    let quitter = Box::new(move || {
        STOPPED.store(true, Ordering::Relaxed);
        // stop2 随闭包消费（FnOnce），引用计数归零
        let _ = stop2;
        if let Some(tid) = HOOK_THREAD_ID.get() {
            unsafe {
                let _ = PostThreadMessageW(*tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    });
    (MonitorHandle::with_quitter(stop, vec![handle], quitter), 1)
}

/// 钩子线程主体：装钩 → 消息循环 → 卸钩
unsafe fn hook_thread() {
    let _ = HOOK_THREAD_ID.set(GetCurrentThreadId());
    let module = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
    let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), Some(module), 0) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("[hotkey] SetWindowsHookExW 失败: {e}");
            return;
        }
    };
    tracing::info!("[hotkey] WH_KEYBOARD_LL 已安装");

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        DispatchMessageW(&msg);
    }

    let _ = UnhookWindowsHookEx(hook);
    tracing::info!("[hotkey] WH_KEYBOARD_LL 已卸载");
}

/// LL 键盘回调：映射 vkCode → 规范名入队。只做轻量工作（回调限时约束）。
unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        // 合成键（含本应用 SendInput 的 Ctrl+V）不进状态机
        if info.flags & LLKHF_INJECTED == KBDLLHOOKSTRUCT_FLAGS(0) {
            let down = wparam.0 as u32 == WM_KEYDOWN || wparam.0 as u32 == WM_SYSKEYDOWN;
            if let Some(name) = vk_to_name(info.vkCode as u16) {
                if let Some(tx) = TX.get() {
                    let _ = tx.send(RawKeyEvent { name: name.into(), pressed: down });
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

const WM_KEYDOWN: u32 = 0x0100;
const WM_SYSKEYDOWN: u32 = 0x0104;
const LLKHF_INJECTED: KBDLLHOOKSTRUCT_FLAGS = KBDLLHOOKSTRUCT_FLAGS(0x10);

/// vkCode → 规范名（对齐 keys.rs 语义：修饰键左右归并，A..Z/0..9）
fn vk_to_name(vk: u16) -> Option<&'static str> {
    Some(match vk {
        v if v == VK_LCONTROL.0 || v == VK_RCONTROL.0 => "Ctrl",
        v if v == VK_LWIN.0 || v == VK_RWIN.0 => "Super",
        v if v == VK_LSHIFT.0 || v == VK_RSHIFT.0 => "Shift",
        v if v == VK_LMENU.0 || v == VK_RMENU.0 => "Alt",
        v if v == VK_SPACE.0 => "Space",
        v if v == VK_RETURN.0 => "Enter",
        v if v == VK_TAB.0 => "Tab",
        v if v == VK_ESCAPE.0 => "Esc",
        // VK_A..VK_Z = 0x41..0x5A，VK_0..VK_9 = 0x30..0x39
        v @ 0x41..=0x5A => ALPHA[(v - 0x41) as usize],
        v @ 0x30..=0x39 => {
            let d = if v == 0x30 { 0 } else { v - 0x30 };
            DIGITS[d as usize]
        }
        _ => return None,
    })
}

// w! 宏生成的静态字符串未用到时避免 unused 警告（保留 import 供后续消息窗口扩展）

const ALPHA: [&str; 26] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];
const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
