# Highlight tool Implementation Plan

> Status: executed 2026-10-26 on branch feature/highlight (inline). Addition: a highlight being dragged is also drawn under the other items.

**Goal:** A Highlight tool (H): drag a semi-transparent, highlighter-like rectangle.

**Architecture:** New `Shape::Highlight { rect, color }`, handled like `Redact`
in geometry (corners/sides, interior hit). Drawn with Cairo's multiply operator
so dark text stays dark. `draw_items` draws all highlights first, so they sit
under every other annotation regardless of order.

**Spec:** `docs/superpowers/specs/2026-10-26-annotshot-design.md` (Roadmap item 2)
plus the details agreed on 2026-10-26 below.

## Agreed details

- Multiply-blended fill, no outline; ignores line width.
- Tint: Yellow as is; other colors mixed 50 % with white (black becomes grey).
- `H`, toolbar button "Highlight" after Marker.
- Always under other annotations, in canvas and export.
- Defaults to yellow (user). *Builder's call:* the Highlight tool remembers its
  own color: entering it sets the current color to the highlight color (yellow
  at first), leaving it restores the color the other tools had. Switching into
  Highlight clears a selection that isn't a highlight, switching out clears a
  selected highlight, so the toolbar never shows one color while recoloring
  another item.
- Select picks other annotations before highlights at the same point (they are drawn on top).

## Review Focus

1. Box drawn before a highlight must not be tinted by it.
2. Select-click on an arrow lying over a highlight picks the arrow.
3. Red-box → H → draw: highlight is yellow, and back on R the box color is red again.
4. Export equals canvas (draw_items order).
5. Undo/redo of highlight color/resize like other items.

## Tasks

### Task 1: Model, geometry, editor
- [x] Tests: highlight grab/hit like redact; `highlight_tool_draws_yellow_and_restores_color`; `set_tool_clears_mismatched_selection`; `select_prefers_items_over_highlights`; set_color on selected highlight + undo.
- [x] `Shape::Highlight`, `Tool::Highlight`, `Editor::set_tool(Tool)` with `highlight_color`/`other_color`; geometry arms shared with Redact; app uses `set_tool`.

### Task 2: Render, app, GUI check, docs
- [x] Tests: `highlight_multiplies` (white → tint, black stays black), `non_yellow_highlight_is_lighter`, `highlights_draw_under_other_items`, `H` key, crosshair cursor.
- [x] `highlight_rgb(Color)`, draw arm with `Operator::Multiply`, two-pass `draw_items`; toolbar button + `H`.
- [x] smoke.sh: highlight over text lines and under the box; check screenshots.
- [x] README + design notes; commit.
