#!/bin/bash
# Headless GUI harness: runs Annotshot on a private Xvfb screen (GTK's X11
# backend) and drives it with xdotool. Your own desktop session is not touched.
#
#   tests/gui/gui.sh start              start Xvfb, a session bus and the app
#   tests/gui/gui.sh paste FILE.png     put an image on the clipboard and press Ctrl+V
#   tests/gui/gui.sh key KEYS...        xdotool key names, e.g. t ctrl+z Escape
#   tests/gui/gui.sh type TEXT          type text
#   tests/gui/gui.sh click X Y          left click at screen coordinates
#   tests/gui/gui.sh drag X1 Y1 X2 Y2   left-button drag
#   tests/gui/gui.sh shot NAME [x,y,w,h]  screenshot to out/NAME.png, optionally cropped
#   tests/gui/gui.sh stop               stop everything
#
# The window is placed at 0,0 (1100x750) on a 1280x800 screen.
# Needs: Xvfb, xdotool, xclip, xwd (x11-apps), python3-gi. Build first: cargo build.
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
OUT="$HERE/out"
APP=${ANNOTSHOT_BIN:-$ROOT/target/debug/annotshot}
export DISPLAY=${ANNOTSHOT_TEST_DISPLAY:-:99}
mkdir -p "$OUT"

settle() { sleep "${1:-0.3}"; }

case "${1:-}" in
  start)
    [ -x "$APP" ] || { echo "missing $APP (run cargo build)" >&2; exit 1; }
    Xvfb "$DISPLAY" -screen 0 1280x800x24 -nolisten tcp >"$OUT/xvfb.log" 2>&1 &
    echo $! >"$OUT/xvfb.pid"
    settle 1
    dbus-daemon --session --fork --print-address=1 --print-pid=1 >"$OUT/dbus.info"
    DBUS_SESSION_BUS_ADDRESS=$(head -1 "$OUT/dbus.info") GDK_BACKEND=x11 GSK_RENDERER=cairo \
      "$APP" >"$OUT/app.log" 2>&1 &
    echo $! >"$OUT/app.pid"
    xdotool search --sync --name '^Annotshot' >/dev/null
    settle 1.5 ;;
  paste)
    xclip -selection clipboard -t image/png -i "$2" &
    echo $! >>"$OUT/xclip.pids"
    settle 0.5
    xdotool key ctrl+v
    settle 0.8 ;;
  key)
    shift; xdotool key "$@"; settle ;;
  type)
    xdotool type --delay 30 "$2"; settle ;;
  click)
    xdotool mousemove "$2" "$3" click 1; settle ;;
  drag)
    xdotool mousemove "$2" "$3" mousedown 1; settle 0.15
    xdotool mousemove $(( ($2 + $4) / 2 )) $(( ($3 + $5) / 2 )); settle 0.1
    xdotool mousemove "$4" "$5"; settle 0.15
    xdotool mouseup 1; settle ;;
  shot)
    xwd -root -silent >"$OUT/screen.xwd"
    python3 -I "$HERE/xwd2png.py" "$OUT/screen.xwd" "$OUT/$2.png" "${3:-}"
    echo "$OUT/$2.png" ;;
  stop)
    for f in app xvfb; do
      [ -f "$OUT/$f.pid" ] && kill "$(cat "$OUT/$f.pid")" 2>/dev/null || true
      rm -f "$OUT/$f.pid"
    done
    [ -f "$OUT/dbus.info" ] && kill "$(sed -n 2p "$OUT/dbus.info")" 2>/dev/null || true
    rm -f "$OUT/dbus.info"
    [ -f "$OUT/xclip.pids" ] && xargs -r kill <"$OUT/xclip.pids" 2>/dev/null || true
    rm -f "$OUT/xclip.pids" ;;
  *)
    sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 1 ;;
esac
