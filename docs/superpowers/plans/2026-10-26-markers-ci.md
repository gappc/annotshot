# Numbered markers + CI Implementation Plan

> Status: executed 2026-10-26 on branch feature/markers-ci (inline). Rulings: Tasks 1+2 share a commit (the new enum variant needs the editor matches to compile); `marker_number` became a test helper because `draw_items` derives the numbers itself.

**Goal:** A Marker tool (N) that places numbered circles, plus a GitHub Actions workflow.

**Architecture:** New `Shape::Marker { center, style }`. The number is never stored:
it is 1 + the count of markers before it in `items`, so deletes close gaps and
undo restores the old numbering for free. Rendering gets the number from
`draw_items`; the provisional marker gets `Editor::next_marker_number()`.

**Spec:** `docs/superpowers/specs/2026-10-26-annotshot-design.md` (Roadmap item 1)
plus the agreed details below.

## Agreed details

- Filled circle in the current color, bold number centered: white text, black on Yellow.
- Radius from width: Thin 12, Medium 16, Thick 22 image px; font size = radius.
- Placed by click (press shows it, drag positions it, release commits); centered on the point.
- Numbers = placement order among existing markers, no gaps, nothing stored.
- Shortcut `N`, toolbar button "Marker" after Redact.
- Selected marker: dragged inside its circle with any tool except Text moves it (move cursor). No resize handles; size follows width (changing width on a selected marker resizes it).

## Review Focus

1. Clicking with the Marker tool on the selected marker must move it, not stack a new one.
2. Deleting marker 2 of 3 renumbers 3 → 2; undo restores 2 and 3.
3. Numbers ≥ 10 must still fit the circle (two digits).
4. Export (copy/save) must show the same numbers as the canvas.
5. Width/color buttons on a selected marker change it and record one undo step.

## Tasks

### Task 1: Model + geometry
- [x] Tests: `marker_radius_per_width`, `marker_bounds_and_hit` (inside circle + tol hits, outside misses), `marker_grab_is_center_inside_circle` (`grab_at` → `Handle::Center`), `translate` moves the center, no corner handles.
- [x] Add `Shape::Marker { center: Point, style: Style }`, `Width::marker_radius()`, `Handle::Center`; update `bounds`, `hit_test`, `grab_at`, `translate`, `drag_handle` (Center: no-op), `handles` (empty).

### Task 2: Editor
- [x] Tests: `marker_tool_places_on_click`, `numbers_follow_order_and_close_gaps` (`marker_number(&items, id)`), `undo_restores_numbering`, `marker_tool_on_selected_marker_moves_it`, `width_resizes_selected_marker`, `next_marker_number`.
- [x] `Tool::Marker`; press starts `Drag::Draw` with a Marker whose drag moves the center; release always commits a marker; `grab_selected` returning `Center` starts `Drag::Move`; style adoption/set_color/set_width cover Marker.
- [x] `pub fn marker_number(items: &[Item], id: ItemId) -> Option<u32>` and `Editor::next_marker_number()`.

### Task 3: Render + canvas + app
- [x] Tests: `marker_fills_circle_with_number` (center pixel area contains white text pixels, edge pixel is the color), `yellow_marker_uses_dark_number`, `draw_items_numbers_markers_in_order`, cursor for `Handle::Center` is "move", `N` key → Marker.
- [x] `draw_shape(cr, shape, number, m)`; `draw_items` counts markers; canvas passes `next_marker_number()` for the provisional shape; `cursor_for` crosshair for Marker; toolbar button + `N` shortcut.
- [x] Extend `tests/gui/smoke.sh` with markers (place 3, delete #2, undo); check screenshots.
- [x] README shortcuts/notes and design "Implementation notes" updated. Commit.

### Task 4: CI
- [x] `.github/workflows/ci.yml`: on push/PR, container `ubuntu:26.04` (distro GTK ≥ 4.16 and libadwaita ≥ 1.6 that the crate features need; ubuntu-latest is too old), apt deps + Xvfb tools, rustup stable with clippy, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo build`, `tests/gui/smoke.sh`, upload `tests/gui/out/*.png` as an artifact.
- [x] Validate the workflow YAML locally; README Development section mentions CI. Commit.
