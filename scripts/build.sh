#!/usr/bin/env bash
# Voxis 构建脚本：前端 + tauri bundle（deb 为主，AppImage 视网络而定）
# 用法：./build.sh
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' src-tauri/Cargo.toml | sed 's/.*"\(.*\)"/\1/')
echo "=== 构建 Voxis v$VERSION ==="

echo "-- 前端依赖与构建 --"
bun install
bun run build

echo "-- Tauri bundle（release 编译较慢，首次约 5~10 分钟）--"
bun run tauri build

echo ""
echo "=== 产物 ==="
ls -la src-tauri/target/release/voxis 2>/dev/null && echo "↑ 裸二进制（可直接拷贝执行，需自行完成 setup.sh 环境步骤）"
ls src-tauri/target/release/bundle/deb/*.deb 2>/dev/null || echo "（deb 未产出）"
ls src-tauri/target/release/bundle/appimage/*.AppImage 2>/dev/null || echo "（AppImage 未产出，Wayland 托盘兼容性一般，推荐 deb）"
