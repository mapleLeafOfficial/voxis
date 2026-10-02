#!/usr/bin/env bash
# Voxis 环境安装脚本（Arch Linux / GNOME Wayland 为主，其他发行版需自行替换包管理命令）
# 用法：sudo ./setup.sh
set -uo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
ok()   { echo -e "${GREEN}✓${NC} $1"; }
warn() { echo -e "${YELLOW}!${NC} $1"; }
fail() { echo -e "${RED}✗${NC} $1"; }

REAL_USER="${SUDO_USER:-$USER}"
REAL_UID=$(id -u "$REAL_USER")

echo "=== Voxis 环境安装（用户: $REAL_USER）==="

# ---------- 1. 系统包 ----------
echo "-- 系统依赖 --"
MISSING=()
for pkg in ydotool wl-clipboard; do
  if pacman -Qi "$pkg" &>/dev/null; then ok "$pkg 已安装"; else MISSING+=("$pkg"); fi
done
if [ ${#MISSING[@]} -gt 0 ]; then
  echo "安装缺失包: ${MISSING[*]}"
  pacman -S --noconfirm --needed "${MISSING[@]}" || fail "包安装失败，请手动安装: ${MISSING[*]}"
fi
# GNOME 托盘需要 AppIndicator 扩展（GNOME 45+ 内置支持部分场景）
if command -v gnome-shell &>/dev/null; then
  if pacman -Qi gnome-shell-extension-appindicator &>/dev/null; then
    ok "gnome-shell-extension-appindicator 已安装"
  else
    warn "未安装 gnome-shell-extension-appindicator（托盘图标可能不显示）"
    echo "   安装: sudo pacman -S gnome-shell-extension-appindicator 然后注销重登"
  fi
fi

# ---------- 2. 用户组（input 读键盘 / uinput 写注入） ----------
echo "-- 用户组 --"
NEED_RELOGIN=0
for grp in input uinput; do
  if id -nG "$REAL_USER" | grep -qw "$grp"; then
    ok "已在 $grp 组"
  else
    groupadd -f "$grp" 2>/dev/null
    usermod -aG "$grp" "$REAL_USER" && { ok "已加入 $grp 组"; NEED_RELOGIN=1; }
  fi
done

# ---------- 3. ydotoold 守护（注入粘贴） ----------
echo "-- ydotoold 服务 --"
UNIT=/etc/systemd/system/ydotoold-voxis.service
if [ -f "$UNIT" ] && systemctl is-active --quiet ydotoold-voxis; then
  ok "ydotoold-voxis 已运行"
else
  RUNTIME_DIR="/run/user/$REAL_UID"
  cat > "$UNIT" <<EOF
[Unit]
Description=ydotoold for Voxis (socket: ${RUNTIME_DIR}/.ydotool_socket)
After=multi-user.target

[Service]
ExecStart=/usr/bin/ydotoold -p ${RUNTIME_DIR}/.ydotool_socket -P 0660 -o ${REAL_UID}:${REAL_UID}
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOF
  systemctl daemon-reload
  systemctl enable --now ydotoold-voxis && ok "ydotoold-voxis 已启用并启动"
fi
# socket 就绪检查（服务刚起时稍等）
sleep 1
if [ -S "/run/user/$REAL_UID/.ydotool_socket" ]; then
  ok "ydotoold socket 就绪"
else
  warn "socket 未就绪（/run/user/$REAL_UID/.ydotool_socket），重登录后再查"
fi

# ---------- 4. 自检汇总 ----------
echo ""
echo "=== 自检汇总 ==="
FAILS=0
id -nG "$REAL_USER" | grep -qw input   && ok "input 组"         || { fail "input 组"; FAILS=1; }
id -nG "$REAL_USER" | grep -qw uinput  && ok "uinput 组"        || { fail "uinput 组"; FAILS=1; }
systemctl is-active --quiet ydotoold-voxis && ok "ydotoold"     || { fail "ydotoold"; FAILS=1; }
ls /dev/input/event* >/dev/null 2>&1   && ok "/dev/input 可见"  || { fail "/dev/input"; FAILS=1; }

if [ "$NEED_RELOGIN" = 1 ]; then
  echo -e "${YELLOW}⚠ 组变更需要【注销并重新登录】才对桌面会话生效${NC}"
fi
echo "完成。重新登录后运行 voxis 即可（热键 + 托盘 + 粘贴全功能）。"
exit $FAILS
