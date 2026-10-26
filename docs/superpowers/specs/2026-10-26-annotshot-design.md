# Annotshot — design

Date: 2026-10-26
Status: implemented (v1, 2026-10-26) — see "Implementation notes" at the end for where the build differs from or adds to this design

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
| Select | V | Click an item to select it and show handles. Drag to move (a selected box moves by its inside); drag handles or sides to resize boxes and redaction boxes; for arrows, drag either end. Text labels can be moved but not resized (no resize handles). Double-click a text label to edit it. Delete/Backspace removes the selected item. |
| Box | R | Drag to draw a rectangle outline, no fill. |
| Arrow | A | Drag from start to end; arrowhead at the end point. |
| Text | T | Click to place a label and start typing; edits in place. Enter inserts a new line (multi-line labels, no automatic wrapping). Escape or clicking elsewhere finishes. Empty labels are discarded. *(Designer's call.)* |
| Redact | X *(builder may choose another letter)* | Drag to draw a solid, opaque filled box (uses the current color; black should be among the colors). |
| Marker | N | Click to place a numbered circle (1, 2, 3 …), centered on the click; drag before releasing to position it. *(Added 2026-10-26, see "Numbered markers".)* |
| Highlight | H | Drag to draw a highlighter-like tint. *(Added 2026-10-26, see "Highlight".)* |
| Pixelate | P | Drag to turn the screenshot inside into coarse blocks. Visual only, not for sensitive data. *(Added 2026-10-26, see "Pixelate".)* |

- After drawing an item, the current tool stays active so several items can be
  drawn in a row.
- Changing color or line width while an item is selected changes that item;
  otherwise it sets the style for the next item.
- While an item is selected, its corners, its sides (boxes and redactions) and
  an arrow's ends can be dragged with every tool except Text, without
  switching to Select. The mouse cursor shows where this is possible.
  *(Added after user feedback.)*

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
- The format follows the file name's extension (`.png` → PNG, `.jpg`/`.jpeg` → JPEG, none → `.jpg` added, asking before replacing an existing file of that name).
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

### Numbered markers (added 2026-10-26, agreed with the user)

- Look: a filled circle in the current color with a bold number in the
  middle: white, or black on yellow so it stays readable.
- Size follows the line width: radius 12 / 16 / 22 image pixels for thin /
  medium / thick; the font size equals the radius. Changing the width of a
  selected marker resizes it.
- Numbering: markers count 1, 2, 3 … in the order they were placed, with no
  gaps. Deleting marker 2 turns 3 into 2; undo brings the old numbers back.
  The number is never stored or typed; it comes from the order.
- A selected marker can be dragged by its inside with every tool except Text
  (move cursor). It has no resize handles.

### Highlight (added 2026-10-26, agreed with the user)

- A rectangle without outline, blended in "multiply" mode like a highlighter
  pen: dark text underneath stays dark, only the background takes the color.
  Ignores the line width. Resizes and moves like a redaction.
- Yellow is used as is; other colors are mixed halfway with white so the
  text stays readable (black gives a grey wash).
- Highlights are always drawn directly on the screenshot, under every other
  annotation, whatever order they were drawn in (canvas and export alike).
  Select therefore picks other items before a highlight at the same spot.
- Starts in yellow (user). *Builder's call:* the Highlight tool keeps its own
  current color. Switching to it shows the highlight color, switching away
  brings back the color the other tools had. A selection of the other kind
  is dropped on that switch, so the color buttons never show one color while
  changing an item of another.

### Pixelate (added 2026-10-26, agreed with the user)

- Drag a rectangle; the screenshot inside becomes blocks of its average
  colors. Block size follows the line width: 6 / 10 / 16 image pixels. Color
  is ignored. Resizes and moves like a redaction.
- Blocks sit on a grid anchored at the image's top-left corner, so they
  don't shift while the rectangle moves. Averages use only pixels inside the
  rectangle.
- Always made from the original screenshot and drawn under everything else
  (pixelations, then highlights, then the other annotations).
- Not safe for sensitive data (pixelated text can be reconstructed); the
  tooltip and README point to Redact. No blur (user's choice).

## Roadmap after the first version

In this order (user said these are sometimes required, so high priority):

1. ~~Numbered markers~~ — done, see "Numbered markers".
2. ~~Highlight~~ — done, see "Highlight".
3. ~~Pixelate~~ — done, see "Pixelate". Blur was dropped (user: pixelate
   alone is enough); it could come back as a second look of the same tool.

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

## Implementation notes (v1)

Where the build differs from or adds to the design above. Verified by unit
tests, a headless GUI harness (`tests/gui/`) and the user on their GNOME
Wayland desktop (clipboard after closing, save dialog, 125 % scaling).

- **Copied image after closing:** GNOME's behaviour couldn't be checked
  automatically, so Annotshot always stays in the background (window hidden)
  while it owns the clipboard and quits once something else is copied.
  Relaunching meanwhile shows a fresh empty window.
- **Typing in a label:** while the text editor is open, Ctrl+C/V/X/Z/Y and
  Delete act on the text. The Copy/Save buttons and Ctrl+S / Ctrl+Shift+S
  finish the label first, then act.
- **Selection sets the style:** selecting an item makes its color (and width,
  for boxes and arrows) the current style, so the tool column always shows
  what the next shape gets.
- **Drawing only starts on the image:** presses in the grey area around the
  image are ignored by drawing tools (except to grab a selected item's handle
  or side); a drag may still end outside.
- **Dialogs:** window shortcuts are ignored while a warning dialog is open.
- **PNG** is written via gdk-pixbuf (no alpha; screenshots are opaque).
- **App ID** `net.gappc.Annotshot`; Redact shortcut X; default tool Box;
  label border drawn inside the label box.
- **Markers:** the number is derived while drawing (`render::draw_items`
  counts markers in item order), so it can't get out of step with deletes,
  undo or redo. The marker being placed shows the next number.
- **Shape being drawn:** the canvas draws it as part of the item list, so a
  highlight or pixelation is layered like finished ones and a new marker shows
  the next number.
- **Pixelation speed:** the canvas keeps the block colors of the screenshot
  per block size (`render::PixelCache`, built on first use after a paste), so
  a redraw only sums the cells cut by the rectangle's edges. A full-screen 4K
  pixelation redraws in about 3 ms instead of 9 ms (release build; one-off
  build 4–7 ms). Export computes directly. Timing probe:
  `cargo test --release -- --ignored --nocapture pixelate_4k`.
