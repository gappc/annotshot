# Select area: copy or crop — design

Date: 2026-11-17
Backlog: task 4 (`docs/superpowers/backlog.md`)
Status: approved 2026-11-17

## Why

The user had a screenshot of the whole screen (or another existing image) and
wanted to use only part of it. Both orders of work make sense to them:

- **crop first, then annotate**: cut the image down to the interesting part,
  annotate there, copy or save;
- **annotate first, then take a part**: annotate the full image and copy just
  a region of it, leaving the full image in place.

They asked for both in one feature.

## Goals

- One area tool: drag a rectangle on the image, adjust it, then either
  **copy the area** or **crop the image** to it.
- Cropping keeps the annotations **editable**: they shift with the crop.
- Cropping is **undoable** with Ctrl+Z (and redoable with Ctrl+Y), like any
  annotation change.

## Non-goals

- Undo for rotation. Rotation keeps clearing the history, crop steps included
  (it could use the new foundation later; not part of this task).
- Pasting an image as an object onto the current one. Ctrl+V keeps replacing
  the image, with the existing "Start a new image?" question.
- Fixed aspect ratios, typed-in sizes, a size readout, zoom.
- Saving only the area. Save always writes the whole (possibly cropped) image.

## Anti-goals

- A crop that silently loses work: everything a crop drops (pixels outside,
  annotations entirely outside) must come back with Ctrl+Z.
- Copy picking the area when the user did not mean it: copy uses the area only
  while one is visible on screen.

## Approaches

- A) Crop clears the undo history, as rotation does; annotations stay editable.
  Medium. Rejected: a crop is where an off-by-a-bit mistake is most likely, and
  there would be no way back except pasting the image again.
- **B) Crop with undo.** An undo step can also hold the previous base image,
  so Ctrl+Z brings back the full image with its annotations. **Chosen by the
  user.** This is the "shared foundation" from the backlog; it is smaller than
  feared because `History` is a plain list of snapshots.
- C) Two separate tools for copy-area and crop. Rejected: both drag the same
  rectangle, and the toolbar grows for nothing.

## Design

### The area tool

- New tool **Area**, shortcut **C**, a toggle button at the end of the tool
  column (after Redact). Tooltip "Select area (C)".
- Dragging on the image draws the area. It is not an annotation: it is not in
  `items`, not in the undo history, never rendered into exports.
- The area lives in image coordinates, is clamped to the image bounds and
  snapped to whole pixels. A drag below `MIN_SIZE` (a plain click) removes the
  area.
- While the Area tool is active and an area exists:
  - outside the area the image is dimmed; the area has a thin dashed border and
    the same corner/side handles as a selected box;
  - handles resize it, dragging inside moves it (clamped to the image), dragging
    outside starts a new area;
  - annotations cannot be selected or changed (the tool owns the mouse).
- **Escape** removes the area. Switching to another tool removes it too
  (an area only exists while the tool is active, so Copy is never surprising).
- Color and width buttons are hidden (or insensitive) while the Area tool is
  active; in their place the panel shows a **Crop** button, sensitive only when
  an area exists. Tooltip "Crop to area (Enter)".

### Copy area

- **Ctrl+C** and the header-bar Copy button copy only the area when one exists:
  `export::flatten` (image + annotations) cut to the area. Toast "Copied area".
  Without an area, copy works as today (whole image).
- Copying changes nothing in the document; the area stays, so several regions
  can be copied one after another.
- To continue with the region as a new image: Ctrl+V, as today.

### Crop

- **Enter** or the **Crop** button crops the image to the area:
  - the base image becomes a new surface of the area's size with those pixels;
  - every annotation shifts by minus the area's top-left corner
    (`geometry::translate`);
  - annotations that lie **entirely** outside the area (by `geometry::bounds`)
    are dropped; partly outside ones are kept and simply clipped when drawn and
    exported;
  - the area is removed; the tool switches back to the tool that was active
    before Area (so one can annotate right away);
  - the document becomes dirty.
- One crop = one undo step that restores the old image **and** the old item
  list (including the dropped annotations). Redo re-applies it.
- **Undo of a crop brings the area back** (decided by the user): the Area tool
  becomes active with the rectangle that was cropped to, so it can be nudged
  and cropped again. Redo ends as a crop does: area gone, previous tool active.
- An open text editor is committed first, as for every document action.

### Undo foundation

- A history snapshot holds the item list and, optionally, crop data: the base
  image, the area rectangle and the tool active before Area (`None` for
  ordinary annotation edits). Only crop steps carry it.
- Undo/redo of a step with an image swaps the base image as well: the current
  image goes onto the opposite stack, the stored one becomes current.
- Today the history lives in `Editor` and the image in `CanvasState`. Where the
  image swap happens is left to the builder (move the image into `Editor`, or
  have `Editor::undo/redo` hand back an image for the caller to install).
- Memory: each crop step keeps one full surface (a 4K screenshot is about
  33 MB). Accepted; `reset`, loading a new image and rotation clear it as they
  clear the history today.

### Effects on existing things

- Marker numbers come from list order: dropping markers renumbers the remaining
  ones. Accepted; undo restores them.
- Pixelate samples the base image; after a crop the blocks are taken from the
  cropped image, so block borders may shift slightly. Accepted.
- The pixel cache keys on the surface and rebuilds itself; the view re-fits on
  the next draw (as after rotation).
- Rotation is unchanged: it still asks merge/delete and clears the whole
  history, crop steps included.

## My calls (please check)

- Shortcut **C**, button at the end of the tool column, name "Select area".
- The area disappears when switching tools and after a crop; Escape clears it.
- After a crop the previous tool comes back (not Area).
- Partly-outside annotations are kept (clipped), only fully-outside ones drop.
- Copy uses the area automatically when one exists; no separate shortcut.
- Crop button in the side panel in place of colors/widths, plus Enter
  (and keypad Enter).
- An area covering the whole image does not crop: the Crop button is
  insensitive and Enter does nothing (no empty undo step, no dirty flag).
- Entering the Area tool drops the annotation selection. Undo/redo, paste and
  rotate remove an existing area (the image it referred to changed).

## Left to the builder

- Where the image swap for undo lives (see above), and the exact snapshot type.
- The icon: Adwaita has no crop icon installed here; pick one that exists in
  the GNOME/Flatpak runtime or bundle a symbolic SVG with the app.
- Dimming colour/alpha and dash pattern; cursor shapes for the area handles.
- Whether hiding or desensitising colors/widths looks better (check with the
  `tests/gui` Xvfb harness).
- Unit tests: area clamping/snapping, crop shifts and drops items, crop
  undo/redo restores image size and items, copy-area output size.
