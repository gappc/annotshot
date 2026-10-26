"""Generate a fake website screenshot to paste into Annotshot.

usage: mkshot.py out.png WIDTH HEIGHT
"""
import sys

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf, GLib  # noqa: E402

W, H = int(sys.argv[2]), int(sys.argv[3])
px = bytearray(b"\xff" * (W * H * 3))


def rect(x0, y0, x1, y1, color):
    for y in range(y0, min(y1, H)):
        for x in range(x0, min(x1, W)):
            i = (y * W + x) * 3
            px[i:i + 3] = bytes(color)


rect(0, 0, W, 40, (59, 74, 90))  # header bar
for x in range(20, 300, 7):  # title: glyph-like strokes, fine detail for pixelation
    rect(x, 62 + (x % 3) * 2, x + 3, 80 - (x % 5), (40, 40, 40))
for i in range(8):  # text lines
    rect(20, 100 + i * 22, W - 40 - (i % 3) * 60, 110 + i * 22, (190, 190, 190))
rect(20, 300, 160, 336, (34, 170, 119))  # button

pb = GdkPixbuf.Pixbuf.new_from_bytes(
    GLib.Bytes.new(bytes(px)), GdkPixbuf.Colorspace.RGB, False, 8, W, H, W * 3
)
pb.savev(sys.argv[1], "png", [], [])
