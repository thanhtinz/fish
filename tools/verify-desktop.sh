#!/usr/bin/env bash
# Proof that the desktop application actually starts.
#
# An application that compiles and then fails to open a window is a whole class of failure the
# unit tests cannot see: a missing system library, a bad Tauri config, assets that were not
# embedded. This boots the real release binary against a virtual display and checks that its
# window appears and that the interface rendered rather than showing a connection error.
#
# Needs: Xvfb, xdotool, xwd (x11-apps), and the webkit2gtk development packages.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
display=":${TJ_DISPLAY:-98}"
work="$(mktemp -d)"

cleanup() {
    [ -n "${app_pid:-}" ] && kill "$app_pid" 2>/dev/null || true
    [ -n "${xvfb_pid:-}" ] && kill "$xvfb_pid" 2>/dev/null || true
    rm -rf "$work"
}
trap cleanup EXIT

npm --prefix "$root/desktop" run build
cargo build --release -p tjlocalizer-desktop

Xvfb "$display" -screen 0 1360x900x24 -nolisten tcp > "$work/xvfb.log" 2>&1 &
xvfb_pid=$!
sleep 2

DISPLAY="$display" WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    GDK_BACKEND=x11 "$root/target/release/tjlocalizer-desktop" > "$work/app.log" 2>&1 &
app_pid=$!

# The webview takes a few seconds to come up; poll rather than guess a sleep.
window=""
for _ in $(seq 1 30); do
    window="$(DISPLAY="$display" xdotool search --name '^TJLocalizer$' 2>/dev/null | tail -1 || true)"
    [ -n "$window" ] && break
    sleep 1
done
if [ -z "$window" ]; then
    echo "the application never opened a window" >&2
    cat "$work/app.log" >&2
    exit 1
fi
sleep 4

DISPLAY="$display" xwd -id "$window" -silent > "$work/shot.xwd"
convert "$work/shot.xwd" "$work/shot.png"

# A window is not enough: when the frontend assets are missing, the webview happily shows a page
# saying it could not reach the dev server. That page is mostly white, and the interface is not.
mean="$(convert "$work/shot.png" -colorspace Gray -format "%[fx:mean]" info:)"
if awk "BEGIN { exit !($mean > 0.5) }"; then
    echo "the window is blank or showing an error page (mean brightness $mean)" >&2
    exit 1
fi

echo "ok: the desktop application starts and renders (mean brightness $mean)"
