# Annotshot

A small GNOME app for annotating screenshots: paste an image from the clipboard, mark it up with boxes, arrows, text, numbered markers and redactions, then copy or save it.

## Prerequisites

    sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev

plus a Rust toolchain via [rustup](https://rustup.rs).

## Install

    ./install.sh              # builds and installs to ~/.local
    ./install.sh --uninstall

To install elsewhere, set `PREFIX`, e.g. `PREFIX=/usr/local ./install.sh` (default `~/.local`).

Annotshot then appears in the GNOME app launcher.

## Shortcuts

| Key | Action |
| --- | --- |
| V | Select |
| R | Box |
| A | Arrow |
| T | Text |
| X | Redact |
| N | Numbered marker |
| Ctrl+V | Paste |
| Ctrl+C | Copy |
| Ctrl+S | Save |
| Ctrl+Shift+S | Save as |
| Ctrl+Z / Ctrl+Y | Undo / redo |
| Delete / BackSpace | Delete selection |
| Escape | Finish text |
| Enter | New line in text |

## Notes

- The last drawn or clicked item stays selected. With any tool except Text, drag its corners, its sides (boxes and redactions) or an arrow's ends to resize it. The mouse cursor changes where that's possible. With Select, drag a selected box's inside to move it. A selected marker moves by dragging its inside, with any tool except Text.
- The file name's extension decides the format when saving: `.png` → PNG, `.jpg`/`.jpeg` → JPEG, no extension → `.jpg` is added. Choosing the PNG filter in the save dialog is not enough; change the name to end in `.png`.
- **Markers** count 1, 2, 3 … in the order you place them. Deleting one renumbers the later ones. The line width sets their size.
- Use **Redact** (a solid box) to hide sensitive data. Pixelating or blurring is not safe.
- A copied image stays available after you close the window: Annotshot keeps running in the background until something else is copied.

## Development

    cargo run                 # development build
    cargo test                # unit tests (model, editor, rendering, export, …)
    cargo clippy --all-targets -- -D warnings
    tests/gui/smoke.sh        # headless GUI run with screenshots, see tests/gui/README.md

GitHub Actions (`.github/workflows/ci.yml`) runs the tests, clippy and the
headless GUI run on every push, in an Ubuntu 26.04 container; the
screenshots are attached to each run.

After changes, run `./install.sh` again to update the launcher version. Design
and implementation plan: `docs/superpowers/`.

## License

Copyright (C) 2026 gappc

Annotshot is free software: you can redistribute it and/or modify it under the
terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version. See [LICENSE](LICENSE).
