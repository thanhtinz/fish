#!/usr/bin/env bash
# Proof that the desktop application starts *and that its screens render*.
#
# An application that compiles and then fails to open a window is a whole class of failure the unit
# tests cannot see: a missing system library, a bad Tauri config, assets that were not embedded.
#
# So is a card that throws. React unmounts the whole tree when a component throws during render, so
# one bad prop anywhere turns the entire window into the page background - which is dark, not white,
# and so passes a brightness check comfortably. That is not hypothetical: every card added to a tab
# is mounted the moment that tab renders, and nothing else here would notice.
#
# Hence two measures rather than one. Brightness catches the webview showing an error page instead
# of the interface. Standard deviation catches the interface having rendered nothing at all: a real
# screen of this application measures 0.064 upwards, and a page holding only its own background
# measures zero.
#
# Needs: Xvfb, xdotool, xwd (x11-apps), ImageMagick, and the webkit2gtk development packages.
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
cargo build --release -p tjlocalizer-desktop -p tjlocalizer-cli
tj="$root/target/release/tjlocalizer"

# A project with something in every tab, so opening one is a real exercise rather than an empty
# shell: text extracted, a build recorded, and a few journal entries to draw.
"$tj" import "$root/crates/tjlocalizer-core/tests/data/sample-game.jar" \
    --into "$work/demo" --name sample-game --source-language en > /dev/null
"$tj" extract "$work/demo" > /dev/null
"$tj" note "$work/demo" "left off waiting on a screenshot of the shop menu" > /dev/null
"$tj" build "$work/demo" > /dev/null

# A home of its own: it seeds the recent-projects list the sidebar reads, and it keeps the
# emulator search away from whatever the machine running this happens to have installed.
export HOME="$work/home"
config="$HOME/.config/com.thanhtinz.tjlocalizer"
mkdir -p "$config"
printf '{"paths":["%s"]}\n' "$work/demo" > "$config/recent-projects.json"

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
DISPLAY="$display" xdotool windowactivate "$window" 2>/dev/null || true
sleep 4

# Asserts one screen rendered. `what` is what gets printed when it did not, so it is a sentence
# about the application rather than a file name.
check() {
    local what="$1"
    DISPLAY="$display" xwd -id "$window" -silent > "$work/shot.xwd"
    convert "$work/shot.xwd" "$work/shot.png"

    local mean deviation
    mean="$(convert "$work/shot.png" -colorspace Gray -format "%[fx:mean]" info:)"
    deviation="$(convert "$work/shot.png" -colorspace Gray -format "%[fx:standard_deviation]" info:)"

    # When the frontend assets are missing the webview shows a page saying it could not reach the
    # dev server. That page is mostly white, and the interface is not.
    if awk "BEGIN { exit !($mean > 0.5) }"; then
        echo "$what: showing an error page (mean brightness $mean)" >&2
        cp "$work/shot.png" "$work/failed.png"
        exit 1
    fi
    # A page holding nothing but its own background has no contrast in it. That is what is left
    # when a component threw during render.
    if awk "BEGIN { exit !($deviation < 0.03) }"; then
        echo "$what: nothing rendered - a component probably threw (deviation $deviation)" >&2
        cat "$work/app.log" >&2
        exit 1
    fi
    echo "  $what (brightness $mean, contrast $deviation)"
}

click() {
    DISPLAY="$display" xdotool mousemove --window "$window" "$1" "$2" click 1
    sleep "${3:-3}"
}

check "the window with no project open"

# Opening a project mounts every card on the Overview tab, the journal among them.
click 120 110 4
check "a project open, on Tổng quan"

# And each tab mounts its own. The emulator card lives on Đóng gói and is rendered whether or not
# anybody has scrolled to it, which is what makes this worth doing.
click 413 66 && check "Văn bản"
click 492 66 && check "Font"
click 546 66 && check "Ảnh"
click 627 66 && check "Đóng gói"

echo "ok: the desktop application starts, opens a project, and every tab renders"
