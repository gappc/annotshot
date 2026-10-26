# Annotshot — design

Date: 2026-10-26
Status: draft, awaiting user approval

## Intent

A small, fast native GNOME app for annotating screenshots, in the spirit of
Greenshot's image editor but trimmed down.

The trigger: during a meeting the user took a screenshot of a website, needed
to add text (and would have liked arrows), and had to open GIMP to do it. That
is far too heavy. Annotshot should be quick enough to use live in a meeting:
open it from the app launcher, paste, mark up, save or copy, done.

Capturing stays with GNOME's built-in screenshot tool, which puts the image on
the clipboard. Annotshot never captures anything itself.

## Goals

- Start instantly from the GNOME app launcher.
- Paste a screenshot from the clipboard (Ctrl+V) into an empty window.
- Annotate with boxes, arrows, text labels and redaction boxes.
- Every annotation stays an editable object (select, move, resize, restyle,
  edit text, delete) for as long as the image is open.
- Save as JPEG (priority) or PNG through the standard GNOME save dialog, and
  copy the result to the clipboard.
- Feel native on GNOME (GTK4 + libadwaita) and work correctly on Wayland.

## Non-goals

- Taking screenshots.
- Reopening a saved image with editable annotations (no project file format).
  Saving flattens everything; the user explicitly accepts this.
- A free color picker — a handful of fixed colors is enough.
- General image editing (cropping, resizing, filters) — not requested.

## Anti-goals (would be a failure even if it technically works)

- Slow startup or a heavy, GIMP-like feel.
- Annotations that can't be adjusted after drawing (the Satty/Swappy
  limitation the user specifically wants to avoid).
- A "redact" that can be reversed. Redaction must replace pixels with an
  opaque fill.
- Saved images at a lower resolution than the pasted screenshot.

## Platform and constraints

- Target: the user's machine, Ubuntu 26.04 LTS, GNOME on Wayland.
  Runtime libraries present: GTK 4.22, libadwaita 1.9.
- Not installed yet (needed to build): Rust toolchain (rustup), and
  `libgtk-4-dev`, `libadwaita-1-dev` (pulls in Cairo/Pango/gdk-pixbuf dev files).
  Installing these needs `sudo`, so the plan starts with a manual step for the
  user.
- Language and toolkit (decided with the user): **Rust + GTK4**.
- Dependencies (checked for license, activity and version compatibility on
  2026-10-26; all MIT and/or Apache-2.0, all actively maintained):
  - `gtk4` 0.11 (with Cairo, Pango, gdk-pixbuf, glib 0.22 from gtk-rs-core)
  - `libadwaita` 0.9
  - `relm4` 0.11 (app structure; requires gtk4 ^0.11, libadwaita ^0.9)
  - `kurbo` 0.13 (2D geometry: hit-testing, bounding boxes, nearest point on
    a segment)
  - **Not** `undo` (stale, unnecessary). **Not** `image`/`imageproc` in the
    first version (only needed later for smooth blur).
- Satty (`Satty-org/Satty`, MPL-2.0, Rust + GTK4) may be used as a reference
  for ideas. Copying whole files would keep those files under MPL-2.0; avoid
  that unless deliberate.

## The app

### Window (layout "B", chosen by the user)

- libadwaita window with a header bar:
  - left: Undo, Redo
  - centre: title (app name, plus file name once saved; unsaved marker)
  - right: Copy, Save… (suggested-action style)
- Left: a vertical tool column:
  Select, Box, Arrow, Text, Redact — then the current color and line width.
  Later tools (markers, highlight, pixelate/blur) are added to this column.
- Rest of the window: the image canvas.

### Start state

- The window opens empty, showing a hint such as
  "Press Ctrl+V to paste a screenshot".
- Starting the app while it is already open brings the existing window to the
  front (single instance; GTK application default). *(Designer's call.)*

### Pasting

- Ctrl+V reads an image from the clipboard and shows it on the canvas.
- If an image is already open, Ctrl+V asks "Start a new image?" before
  replacing it. If there are unsaved changes, that dialog says so (same
  warning as on close).
- If the clipboard holds no image, show a short non-blocking notice (toast).
- The canvas scales the image down to fit the window when needed (zoom is not
  required). Export always uses the image's original pixel size.

### Tools

| Tool | Shortcut | Behaviour |
|---|---|---|
| Select | V | Click an item to select it and show handles. Drag to move; drag handles to resize boxes and redaction boxes; for arrows, drag either end. Text labels can be moved but not resized (no resize handles). Double-click a text label to edit it. Delete/Backspace removes the selected item. |
| Box | R | Drag to draw a rectangle outline, no fill. |
| Arrow | A | Drag from start to end; arrowhead at the end point. |
| Text | T | Click to place a label and start typing; edits in place. Enter inserts a new line (multi-line labels, no automatic wrapping). Escape or clicking elsewhere finishes. Empty labels are discarded. *(Designer's call.)* |
| Redact | X *(builder may choose another letter)* | Drag to draw a solid, opaque filled box (uses the current color; black should be among the colors). |

- After drawing an item, the current tool stays active so several items can be
  drawn in a row.
- Changing color or line width while an item is selected changes that item;
  otherwise it sets the style for the next item.

### Look of annotations

- **Text labels (chosen by the user):** "boxed" style, as in Greenshot — bold
  text in the current color, on a white box with a border in the current
  color. Font size: a sensible default for screenshots; adjustable font size is
  not required in the first version.
- Boxes and arrows: stroked in the current color with the current width.
- Colors: a fixed palette of about five — red (default), yellow, green, blue,
  black.
- Line widths: a few presets (e.g. thin / medium / thick).

### Undo / redo

- Ctrl+Z undo, **Ctrl+Y redo** (user's choice). Header bar buttons do the same.
- Covers every change to annotations: add, move, resize, restyle, text edit,
  delete. A drag counts as one step.
- Pasting a new image starts a fresh history.

### Copy and save

- **Ctrl+C:** copies the flattened image (screenshot + annotations, original
  resolution) to the clipboard. The image stays open. Copying does **not**
  count as saving — closing after only copying still shows the unsaved-changes
  warning (confirmed by the user).
- **Copied image must survive closing the app.** On Wayland the copying app
  owns the clipboard. The builder must verify whether GNOME (mutter) keeps a
  copied image after Annotshot exits. If it does not, Annotshot keeps running
  in the background after its window closes (window hidden, application held)
  until another app takes over the clipboard, then exits. Launching Annotshot
  again during that time shows a fresh empty window. *(Designer's call.)*
- **Ctrl+S:** the first time for an image, opens the GNOME save dialog
  (portal-based `GtkFileDialog`) with JPEG pre-selected; PNG also offered.
  After that, Ctrl+S overwrites the same file silently.
- **Ctrl+Shift+S:** Save As — always opens the dialog; later Ctrl+S saves go to
  the new file.
- The format follows the chosen file type / extension.
- After saving, the image stays open and editable.
- JPEG encoding via gdk-pixbuf (part of the GTK stack); PNG via gdk-pixbuf or
  Cairo. No extra image crate.

### Unsaved changes

- An image counts as having unsaved changes if anything was drawn or changed
  since the last save (or since paste, if never saved).
- Closing the window with unsaved changes shows a warning dialog
  (save / discard / cancel).
- Ctrl+V over an image with unsaved changes warns the same way.

### Installation and launch

- Launched from the GNOME app launcher (user's choice).
- Ships with a `.desktop` file and an app icon.
- An install script builds a release binary and installs the binary, desktop
  file and icon into the user's home (`~/.local/bin`,
  `~/.local/share/applications`, `~/.local/share/icons`). No root needed.

## Roadmap after the first version

In this order (user said these are sometimes required, so high priority):

1. Numbered markers (circles with 1, 2, 3 … that auto-increment).
2. Highlight (semi-transparent marker over an area, e.g. yellow).
3. Pixelate / blur as a *visual* effect. Pixelate needs no extra crates; smooth
   blur would add `image`/`imageproc`. Must be clearly separate from Redact,
   which remains the safe option for sensitive data (pixelated and blurred
   text can be reconstructed by tools like Depix and Unredacter).

## Left to the builder

- Internal architecture: item model, how relm4 components are split, how
  rendering is organised (Cairo in a `DrawingArea` is the expected approach).
- Undo implementation (snapshotting the item list per change is fine).
- How in-place text editing is done (e.g. an overlay text widget while
  editing).
- Exact defaults: font size, line-width presets, arrowhead size, handle size,
  JPEG quality (around 90 is reasonable).
- Icon design, exact hint wording, tooltips.
- Shortcut for Redact if X feels wrong.
- Testing approach.
