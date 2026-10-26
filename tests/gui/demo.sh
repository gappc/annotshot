#!/bin/bash
# Records the README demo GIF (docs/demo.gif) on the headless screen: paste a
# made-up settings page, mark up a bug report, redact a key, then show that
# the annotations stay editable. Needs ffmpeg plus the gui.sh tools.
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
OUT="$HERE/out"
g() { "$HERE/gui.sh" "$@"; }
export DISPLAY=${ANNOTSHOT_TEST_DISPLAY:-:99}

# The pasted 860x540 page shows 1:1 with its top-left corner here.
OX=175 OY=128

pause() { sleep "$1"; }

# Move the mouse smoothly to image coordinates.
glide() {
  local x=$(($1 + OX)) y=$(($2 + OY)) steps=${3:-18} cx cy i
  eval "$(xdotool getmouselocation --shell | grep -E '^[XY]=')"
  cx=$X cy=$Y
  for ((i = 1; i <= steps; i++)); do
    xdotool mousemove $((cx + (x - cx) * i / steps)) $((cy + (y - cy) * i / steps))
    sleep 0.016
  done
}
click() { glide "$1" "$2"; pause 0.15; xdotool click 1; pause 0.3; }
drag() {
  glide "$1" "$2"; pause 0.15
  xdotool mousedown 1; pause 0.1
  glide "$3" "$4" "${5:-24}"; pause 0.1
  xdotool mouseup 1; pause 0.4
}
key() { xdotool key "$@"; pause 0.5; }
# Click a toolbar color swatch (screen coordinates).
swatch() { xdotool mousemove "$1" "$2" click 1; pause 0.3; }

mkdir -p "$OUT" "$ROOT/docs"
python3 -I "$HERE/mkdemo.py" "$OUT/demo-page.png"
trap 'g stop' EXIT
g start
xdotool mousemove 600 300

ffmpeg -y -loglevel error -f x11grab -framerate 25 -video_size 1100x750 -draw_mouse 1 \
  -i "$DISPLAY+0,0" -c:v libx264rgb -preset ultrafast -qp 0 "$OUT/demo.mkv" &
FF=$!
pause 1.2

g paste "$OUT/demo-page.png"
pause 0.8

# Highlight part of the error message (extended later to show editing).
key h
drag 44 144 330 168

# Numbered steps: 1 = the URL that was entered, 2 = the button.
key n
click 596 353
click 752 468                        # lands on the button text; fixed below

# Arrow and label pointing at the error.
key a
drag 690 236 640 178
key t
click 586 238
xdotool type --delay 55 "Since Monday's release"
pause 0.3
key Escape

# Hide the API key for real, and pixelate the user name.
key v
click 400 515                        # deselect the label, so black only sets the next color
key x
swatch 59 520                        # black
drag 56 406 300 432
key p
drag 694 8 820 40

pause 0.8

# Everything stays editable: widen the highlight, move marker 2 off the button.
key v
click 200 156
drag 330 156 538 156 30
click 752 468
drag 752 468 742 434 20
click 400 515                        # deselect
pause 2.5

kill -INT "$FF"
wait "$FF" || true

ffmpeg -y -loglevel error -i "$OUT/demo.mkv" \
  -vf "fps=15,scale=800:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
  "$ROOT/docs/demo.gif"
ls -l "$ROOT/docs/demo.gif"
