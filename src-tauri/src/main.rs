// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // NVIDIA + WebKitGTK 在 Wayland 下会触发 Wayland 协议错误（Error 71），
    // 关闭 webkit 合成模式规避；已设置的环墨变量优先
    if std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
        // SAFETY: 必须在任何线程启动前设置
        unsafe { std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1") };
    }
    voxis_lib::run()
}
