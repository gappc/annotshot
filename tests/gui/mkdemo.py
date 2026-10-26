"""Generate a realistic-looking (made-up) settings page for the demo GIF.

usage: mkdemo.py out.png
"""
import sys

import cairo
import gi

gi.require_version("Pango", "1.0")
gi.require_version("PangoCairo", "1.0")
from gi.repository import Pango, PangoCairo  # noqa: E402

W, H = 860, 540
surface = cairo.ImageSurface(cairo.FORMAT_RGB24, W, H)
cr = cairo.Context(surface)


def rgb(h):
    return tuple(int(h[i:i + 2], 16) / 255 for i in (0, 2, 4))


def fill(x, y, w, h, color, radius=0):
    cr.set_source_rgb(*rgb(color))
    if radius:
        cr.new_sub_path()
        cr.arc(x + w - radius, y + radius, radius, -1.5708, 0)
        cr.arc(x + w - radius, y + h - radius, radius, 0, 1.5708)
        cr.arc(x + radius, y + h - radius, radius, 1.5708, 3.1416)
        cr.arc(x + radius, y + radius, radius, 3.1416, 4.7124)
        cr.close_path()
    else:
        cr.rectangle(x, y, w, h)
    cr.fill()


def stroke_box(x, y, w, h, color, radius=4):
    cr.set_source_rgb(*rgb(color))
    cr.set_line_width(1)
    cr.new_sub_path()
    cr.arc(x + w - radius, y + radius, radius, -1.5708, 0)
    cr.arc(x + w - radius, y + h - radius, radius, 0, 1.5708)
    cr.arc(x + radius, y + h - radius, radius, 1.5708, 3.1416)
    cr.arc(x + radius, y + radius, radius, 3.1416, 4.7124)
    cr.close_path()
    cr.stroke()


def text(x, y, s, size=13, color="2e3436", weight="", family="Adwaita Sans"):
    layout = PangoCairo.create_layout(cr)
    layout.set_font_description(Pango.FontDescription.from_string(f"{family} {weight} {size}px"))
    layout.set_text(s, -1)
    cr.set_source_rgb(*rgb(color))
    cr.move_to(x, y)
    PangoCairo.show_layout(cr, layout)
    return layout.get_pixel_size()


fill(0, 0, W, H, "f6f7f9")

# Top bar
fill(0, 0, W, 48, "1f2a37")
text(20, 14, "◆ Acme Cloud", 16, "ffffff", "Bold")
x = 180
for item in ["Dashboard", "Projects", "Settings", "Billing"]:
    w, _ = text(x, 16, item, 13, "ffffff" if item == "Settings" else "9aa5b1")
    x += w + 28
fill(W - 160, 12, 24, 24, "6c8ebf", 12)
text(W - 157, 16, "JD", 11, "ffffff", "Bold")
text(W - 128, 16, "Jane Doe", 13, "d0d6dd")

# Title
text(32, 72, "Project settings", 22, "1f2a37", "Bold")
text(32, 104, "storefront-web · production", 13, "6b7785")

# Error banner
fill(32, 136, W - 64, 40, "fdecea", 6)
fill(32, 136, 4, 40, "c62828")
text(48, 147, "Couldn't save changes: the webhook URL did not respond (timeout after 10 s).", 13, "8e1c1c")

# Card
fill(32, 196, W - 64, 300, "ffffff", 8)
stroke_box(32, 196, W - 64, 300, "e1e5ea", 8)
text(52, 214, "Deployment", 15, "1f2a37", "Bold")


def field(y, label, value, mono=False):
    text(52, y, label, 12, "6b7785")
    fill(52, y + 20, 520, 34, "ffffff", 4)
    stroke_box(52, y + 20, 520, 34, "c9d1da")
    text(62, y + 28, value, 13, "1f2a37", family="DejaVu Sans Mono" if mono else "Adwaita Sans")


field(248, "Branch", "main")
field(316, "Webhook URL", "https://hooks.acme-internal.example/deploy/storefront")
field(384, "API key", "ak_demo_NOT_A_REAL_KEY_1234", mono=True)
text(590, 412, "Rotate key", 13, "2f6fd0")

# Buttons
fill(W - 290, 452, 100, 32, "ffffff", 6)
stroke_box(W - 290, 452, 100, 32, "c9d1da", 6)
text(W - 266, 459, "Cancel", 13, "1f2a37")
fill(W - 178, 452, 126, 32, "2f6fd0", 6)
text(W - 162, 459, "Save changes", 13, "ffffff", "Bold")

surface.write_to_png(sys.argv[1])
