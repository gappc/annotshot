"""Convert an XWD screen dump (from `xwd -root`) to PNG, optionally cropped.

usage: xwd2png.py in.xwd out.png [x,y,w,h]
"""
import struct
import sys

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf, GLib  # noqa: E402

data = open(sys.argv[1], "rb").read()
h = struct.unpack(">25I", data[:100])
header_size, width, height = h[0], h[4], h[5]
byte_order, bits_per_pixel, bytes_per_line, ncolors = h[7], h[11], h[12], h[19]
offset = header_size + ncolors * 12  # pixels follow the header and colormap
n = bits_per_pixel // 8

rgb = bytearray()
for y in range(height):
    row = data[offset + y * bytes_per_line: offset + y * bytes_per_line + width * n]
    for x in range(width):
        px = row[n * x: n * x + n]
        if byte_order == 1:  # MSB first
            px = px[::-1]
        rgb += bytes((px[2], px[1], px[0]))  # stored B, G, R(, pad)

pb = GdkPixbuf.Pixbuf.new_from_bytes(
    GLib.Bytes.new(bytes(rgb)), GdkPixbuf.Colorspace.RGB, False, 8, width, height, width * 3
)
if len(sys.argv) > 3 and sys.argv[3]:
    x, y, w, ht = map(int, sys.argv[3].split(","))
    pb = pb.new_subpixbuf(x, y, w, ht)
pb.savev(sys.argv[2], "png", [], [])
