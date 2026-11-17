# Rotate image — design

Date: 2026-11-17
Backlog: task 5 (`docs/superpowers/backlog.md`)
Status: implemented 2026-11-17

## Why

A photo copied from a phone to the desktop arrived sideways. Phone photos are
often upright only through their EXIF orientation tag, which is lost on the
clipboard, so Annotshot cannot fix this automatically. The user rotates
**before** annotating. Needed once so far: keep it small.

## Approaches

- **A) Rotate the image only; annotations are either merged into the image or
  deleted first.** Chosen: covers the real case, no coordinate transforms, no
  change to the undo history format. Merging reuses `export::flatten`.
- B) Rotate at any time, annotations stay editable and turn with it, full undo
  (image in the undo steps). This is the shared foundation with task 4;
  rejected because it covers a case the user does not have. Task 4 builds it
  if it ever needs it.
- C) Like A, plus Ctrl+Z undoes a rotation (undo steps also store the old
  image). Rejected for now by the user: not worth the extra complexity.

## Design

- Two header-bar buttons after Undo/Redo: **Rotate left** and **Rotate right**
  (`object-rotate-left-symbolic` / `object-rotate-right-symbolic`), tooltips
  with the shortcuts. 180° = press twice. Enabled whenever an image is open.
- Shortcuts: **Ctrl+R** right, **Ctrl+Shift+R** left (as in GNOME's image
  viewer; `R` without Ctrl stays Box).
- An open text editor is committed first (as every document action does);
  clicking a rotate button while typing turns the typed text into a label, so
  the dialog below appears. Ctrl+R does nothing while the editor is open (keys
  go to the text field).
- **Without annotations** the image rotates at once.
- **With annotations** a dialog asks first:
  - heading "Rotate image?", body saying annotations can either be merged into
    the image (no longer editable afterwards) or deleted;
  - responses: *Cancel* (close response), *Delete annotations* (destructive),
    *Merge into image* (suggested, default);
  - Merge: the flattened image (`export::flatten`, as for save/copy) becomes the
    new base image, then it is rotated. Delete: the items are dropped, then the
    image is rotated.
  - Like the other dialogs, keys and new dialogs are blocked while it shows
    (`busy` / `visible_dialog`).
- A rotation:
  - replaces `CanvasState.image` with a new surface of swapped width/height
    (90° turn of the pixels, no resampling);
  - leaves no annotations, and clears the undo/redo history and the selection,
    so Ctrl+Z cannot bring back annotations drawn on the unrotated image;
  - marks the document dirty (the image differs from what was saved).
  - The pixelate cache rebuilds itself (it keys on the surface); the view
    re-fits on the next draw.
- There is no undo for a rotation: the opposite button turns the image back;
  merged annotations stay merged, deleted ones stay deleted.

## Non-goals

- Undo/redo of a rotation (possible later: undo steps would store the old
  image; that is also what task 4's crop would need).
- Annotations that stay editable and turn with the image; free angles;
  flipping/mirroring.
- Reading EXIF orientation (not available from the clipboard).

## My calls (please check)

- Dialog wording and that *Merge into image* is the default.
- The rotate helper lives in `export.rs` next to `surface_from_texture`, with
  unit tests on a small asymmetric image; a new `Editor` method drops items,
  history and selection and sets dirty (unlike `reset`, which clears dirty).

## Implementation notes

- Built as designed. Headless check (tests/gui): rotate without annotations,
  Merge (box, pixelate and a just-typed label turn with the image and are no
  longer selectable), Delete, Cancel (box stays editable), and a click on the
  rotate button while typing commits the label and shows the dialog.
- Rotating after a save is covered by the unit test
  `clear_for_rotation_empties_and_dirties`; the harness cannot drive the
  portal save dialog.

## Left to the builder

- Exact names; headless check (tests/gui) that the buttons appear, the dialog
  shows only with annotations, both choices work, and a rotated portrait image
  fits the window.
