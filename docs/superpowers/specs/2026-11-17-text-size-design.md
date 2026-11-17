# Text size — design

Date: 2026-11-17
Backlog: task 2 (`docs/superpowers/backlog.md`)
Status: approved 2026-11-17, not yet built

## Why

Text labels have a fixed size of 16 px. On large screenshots (4K, 200 %
scaling) that is too small; on small screenshots it is too large.

## Approaches

- **A) Reuse `Width` (Thin/Medium/Thick) with three widely spaced font sizes.**
  The existing buttons start to apply to text. Chosen: the smallest change, no new
  UI, and it works like color, width and the other tools.
- B) A separate text size with 4–5 steps and its own buttons. Finer steps, but
  the toolbar changes per tool and needs more code. Rejected for now.
- C) Like A, but scaled by the image size. Fits the problem best, but is less
  predictable and conflicts with cropping (task 4). Rejected for now.

B or C can be added later without rework.

## Design

- `Shape::Text` gets a `width: Width` field. New labels take `editor.style.width`,
  just as they take the color.
- Font size per step: **Thin 12 px, Medium 16 px (as today), Thick 28 px**,
  via `Width::font_px()`. `TEXT_FONT_PX` goes away.
- Padding scales with the font: `TEXT_PADDING` (6 px at 16 px) becomes
  `font_px * 6/16`. The border stays 2 px. (My call: a fixed padding looks
  cramped at 28 px.)
- `TextMeasure::text_size(text, font_px)` takes the size; `bounds`, `hit_test`,
  render and the two test stubs pass it through.
- `set_width` changes a selected label (one undo step), just as `set_color`
  does. `adopt_selected_style` takes the width from a selected label.
- An open text editor follows a width click immediately: font size in the CSS,
  padding and field size are restyled (like `TextEditor::set_color`), and focus
  goes back to the field. When an existing label is being edited, it is selected,
  so it is resized as well.
- Color and width stay shared across all tools: whoever drew a thick box gets
  thick text next. This is how it already works for color.

## Non-goals

- More than three steps, a free size, or automatic scaling by image size.
- Saved projects: annotations are not saved, so no migration is needed.

## Left to the builder

- Exact names; whether padding goes through a helper `text_padding(font_px)`.
- Check headless (tests/gui) that typing, committing and re-editing look the same
  at all three sizes, also at view scale ≠ 1.
