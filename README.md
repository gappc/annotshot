# Annotshot

A small GNOME app for annotating screenshots: paste an image from the clipboard, mark it up with boxes, arrows, text and redactions, then copy or save it.

## Prerequisites

    sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev

plus a Rust toolchain via [rustup](https://rustup.rs).

## Install

    ./install.sh              # builds and installs to ~/.local
    ./install.sh --uninstall

Annotshot then appears in the GNOME app launcher.

## Shortcuts

| Key | Action |
| --- | --- |
| V | Select |
| R | Box |
| A | Arrow |
| T | Text |
| X | Redact |
| Ctrl+V | Paste |
| Ctrl+C | Copy |
| Ctrl+S | Save |
| Ctrl+Shift+S | Save as |
| Ctrl+Z / Ctrl+Y | Undo / redo |
| Delete / BackSpace | Delete selection |
| Escape | Finish text |
| Enter | New line in text |

## Notes

- Use **Redact** (a solid box) to hide sensitive data. Pixelating or blurring is not safe.
- A copied image stays available after you close the window: Annotshot keeps running in the background until something else is copied.
