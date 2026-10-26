#!/bin/bash
# Example run: paste a sample screenshot, type a two-line label, draw a box and
# resize it by a corner and a side, draw an arrow and move its start point.
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

echo "screenshots in $HERE/out/"
