#!/bin/bash
# Example run: paste a sample screenshot, type a two-line label, draw a box and
# resize it by a corner and a side, draw an arrow and move its start point,
# place numbered markers, move one, delete one and undo, then highlight.
# Screenshots land in tests/gui/out/; look at them to judge the result.
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
g() { "$HERE/gui.sh" "$@"; }
trap 'g stop' EXIT

mkdir -p "$HERE/out"
python3 -I "$HERE/mkshot.py" "$HERE/out/sample.png" 700 400

g start
g paste "$HERE/out/sample.png"     # 700x400 image appears at about (251,198)
g shot 01-pasted 0,0,1100,750

g key t
g click 500 450
g type "What about PNG?"
g key Return
g type "second line"
g shot 02-text-typing 300,350,600,250
g key Escape
g shot 03-text-done 300,350,600,250

g key r
g drag 400 300 600 400               # new box
g drag 600 400 680 470               # bottom-right corner, Box tool still active
g drag 400 350 330 350               # left side
g shot 04-box-resized 230,180,760,450

g key a
g drag 300 560 520 520               # new arrow
g drag 300 560 350 600               # move its start point
g shot 05-arrow 230,180,760,450

g key n
g click 760 250                      # markers 1, 2, 3
g click 820 300
g click 880 250
g drag 880 250 900 330               # drag the selected marker 3 down (Marker tool)
g shot 06-markers 230,180,760,450
g key v
g click 820 300                      # select marker 2 and delete it: 3 becomes 2
g key Delete
g shot 07-marker-deleted 230,180,760,450
g key ctrl+z                         # back to 1, 2, 3
g shot 08-marker-undone 230,180,760,450

g key h
g drag 270 290 680 315               # highlight two text lines, under the box
g shot 09-highlight 230,180,760,450
g key r
g drag 300 370 360 400               # back on Box: red again, not yellow
g shot 10-box-after-highlight 230,180,760,450

# Fail if the app crashed along the way (screenshots alone wouldn't show it).
kill -0 "$(cat "$HERE/out/app.pid")" || { echo "annotshot is no longer running" >&2; exit 1; }
echo "screenshots in $HERE/out/"
