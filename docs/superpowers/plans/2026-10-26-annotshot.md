# Annotshot Implementation Plan

> **Status:** executed 2026-10-26 (all 10 tasks plus a final review fix wave; merged to `main`). Later changes are in the git history and the design's "Implementation notes".

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A native GNOME app (Rust + GTK4 + libadwaita) that takes a screenshot pasted from the clipboard, lets the user add boxes, arrows, text labels and redaction boxes as editable objects, and copies or saves the flattened result as JPEG/PNG.

**Architecture:** All annotation logic (item model, geometry, hit-testing, undo history, pointer state machine) lives in pure Rust modules with no GTK dependency and is unit-tested. Rendering and export use Cairo + Pango and are tested headlessly against `cairo::ImageSurface` pixels. A thin relm4 UI layer wires GTK events, clipboard and file dialogs to that core; it is verified with manual checklists.

**Tech Stack:** Rust (stable), gtk4 0.11 (feature `v4_16`), libadwaita 0.9 (feature `v1_6`), relm4 0.11 (feature `libadwaita`), kurbo 0.13, pangocairo 0.22 (same gtk-rs-core release as gtk4's cairo/pango). Cairo, Pango, gdk-pixbuf via `gtk::cairo`, `gtk::pango`, `gtk::gdk_pixbuf`.

**Spec:** `docs/superpowers/specs/2026-10-26-annotshot-design.md`

## Global Constraints

- Target: Ubuntu 26.04 LTS, GNOME on Wayland, GTK 4.22, libadwaita 1.9.
- Dependencies limited to: `gtk4`, `libadwaita`, `relm4`, `kurbo`, `pangocairo`. **No** `undo`, `image`, `imageproc`. No other crates without asking the user.
- App ID: `net.gappc.Annotshot` (planner's call). App name shown to users: `Annotshot`.
- Annotation coordinates are stored in **image pixels** (f64). Export is always at the pasted image's original pixel size.
- Colors (fixed palette, default Red): Red `#e01b24`, Yellow `#f6d32d`, Green `#33d17a`, Blue `#3584e4`, Black `#000000`.
- Line widths (image px): Thin 2, Medium 4 (default), Thick 8.
- Text label: font `Sans Bold`, absolute size 16 px; padding 6 px; border 2 px in the item color; white fill; text in the item color. Multi-line, no wrapping.
- Arrowhead: length `max(12, 4 × width)`, half-angle 30°, filled, tip at the end point.
- Redact: opaque fill in the current color, no transparency.
- Shortcuts: V Select, R Box, A Arrow, T Text, X Redact, Ctrl+V paste, Ctrl+C copy, Ctrl+S save, Ctrl+Shift+S save as, Ctrl+Z undo, **Ctrl+Y redo**, Delete/BackSpace delete selected.
- Hit tolerance 6 screen px and handle size 8 screen px; both divided by the current view scale before use in image coordinates.
- JPEG quality 90. Save dialog pre-selects JPEG; default file name `screenshot-YYYY-MM-DD-HHMMSS.jpg`.
- Copying does not count as saving.
- Commit messages: plain, no AI/Claude mention, no co-author trailers.

## Review Focus

1. **Typing letters in a text label** (e.g. "Remove the arrow") must not switch tools or delete items; single-key shortcuts and Delete apply only while no text editor is open, and Ctrl+C / Ctrl+Z inside the editor act on the text. → manual check in Task 7.
2. **Large or HiDPI screenshots** (e.g. 3840×2160, or GNOME scale 200 %) must export at exactly the pasted pixel size and stay clickable with the same on-screen tolerance at any view scale. → tests in Task 5 (export size) and Task 6 (view transform).
3. **Pasting when the clipboard holds no image** (text, empty) shows a toast and leaves the current image and annotations untouched. → manual check in Task 6.
4. **Save failure or cancel** (dialog cancelled, read-only folder): nothing changes, unsaved state stays, an error toast appears on failure, and the remembered path is not set. → test in Task 5 (error returned) and manual check in Task 8.
5. **Zero-size drags and clicks** with Box/Arrow/Redact (click without moving, or a 1-px drag) must not create invisible items that later catch clicks or count as changes. → test in Task 3.

---

## File Structure

```
Cargo.toml
install.sh                         # build + install into ~/.local (Task 10)
data/net.gappc.Annotshot.desktop   # launcher entry (Task 10)
data/net.gappc.Annotshot.svg       # app icon (Task 10)
src/main.rs        # starts the relm4 app (Task 1)
src/model.rs       # Color, Width, Style, Shape, Item, constants, TextMeasure trait (Task 2)
src/geometry.rs    # bounds, hit-testing, handles, move/resize (Task 2)
src/history.rs     # snapshot undo/redo stack (Task 3)
src/editor.rs      # Editor: tools, selection, pointer state machine, dirty flag (Task 3)
src/render.rs      # Cairo/Pango drawing of items + selection; PangoMeasure (Task 4)
src/export.rs      # flatten, PNG/JPEG save, format from path, texture<->surface (Task 5)
src/view.rs        # ViewTransform: fit image into widget, map coordinates (Task 6)
src/app.rs         # relm4 AppModel: window, tool column, canvas, dialogs (Tasks 1, 6–9)
src/text_edit.rs   # in-place text editing overlay (Task 7)
```

---

### Task 1: Prerequisites and empty window

**Files:**
- Create: `Cargo.toml`, `src/main.rs`, `src/app.rs`

**Interfaces:**
- Produces: `app::AppModel` (relm4 `Component`), `app::AppMsg` enum (extended in later tasks). Window layout: `adw::ApplicationWindow` → `adw::ToolbarView` with `adw::HeaderBar` (start: Undo `edit-undo-symbolic`, Redo `edit-redo-symbolic`; end: Save… suggested-action, Copy `edit-copy-symbolic`) → `adw::ToastOverlay` → horizontal `gtk::Box`: left vertical tool column, right `gtk::Overlay` containing a `gtk::DrawingArea` (hexpand, vexpand) and an `adw::StatusPage` hint "Press Ctrl+V to paste a screenshot" shown while no image is loaded.
- Tool column (top to bottom): toggle buttons Select, Box, Arrow, Text, Redact (grouped, one active), separator, five color toggle buttons (round swatches), separator, three width toggle buttons (Thin/Medium/Thick). Tooltips include the shortcut letter.

- [ ] **Step 1: Ask the user to install prerequisites (manual, needs sudo)**

Ask the user to run:
```bash
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # if rustup not installed
```
Verify: `cargo --version && pkg-config --modversion gtk4 libadwaita-1` prints a cargo version, `4.22.x` and `1.9.x`.

- [ ] **Step 2: Create the crate**

`cargo init --name annotshot`, then `cargo add gtk4@0.11 --rename gtk --features v4_16`, `cargo add libadwaita@0.9 --rename adw --features v1_6`, `cargo add relm4@0.11 --features libadwaita`, `cargo add kurbo@0.13`, `cargo add pangocairo` (must resolve to the same gtk-rs-core minor as gtk4's cairo, i.e. 0.22; check `cargo tree -d` shows no duplicate glib/cairo).

- [ ] **Step 3: Implement `main.rs` and the static window in `app.rs`**

`main` runs `relm4::RelmApp::new("net.gappc.Annotshot").run::<AppModel>(())`. Window default size 1100×750, title "Annotshot". No behaviour yet beyond showing the layout above.

- [ ] **Step 4: Verify**

Run: `cargo build && cargo run`
Expected: window opens with header bar, tool column, and the paste hint; no warnings in the terminal. `cargo clippy` clean.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src
git commit -m "Scaffold Annotshot window"
```

---

### Task 2: Item model and geometry

**Files:**
- Create: `src/model.rs`, `src/geometry.rs` (tests in `#[cfg(test)] mod tests` inside each file)

**Interfaces:**
- Produces (model.rs):
  - `enum Color { Red, Yellow, Green, Blue, Black }` with `fn rgb(self) -> (f64, f64, f64)` (values from Global Constraints), `Color::ALL: [Color; 5]`, `Default = Red`.
  - `enum Width { Thin, Medium, Thick }` with `fn px(self) -> f64` (2/4/8), `Default = Medium`.
  - `struct Style { color: Color, width: Width }` (Copy, Default).
  - `type ItemId = u64;`
  - `enum Shape { Rect { rect: kurbo::Rect, style: Style }, Arrow { start: Point, end: Point, style: Style }, Text { origin: Point, text: String, color: Color }, Redact { rect: kurbo::Rect, color: Color } }` (Clone, PartialEq, Debug). `Rect` values are always normalized (x0<x1, y0<y1).
  - `struct Item { id: ItemId, shape: Shape }`.
  - consts `TEXT_FONT_PX = 16.0`, `TEXT_PADDING = 6.0`, `TEXT_BORDER = 2.0`, `MIN_SIZE = 3.0`.
  - `trait TextMeasure { fn text_size(&self, text: &str) -> kurbo::Size; }` — size of the laid-out text without padding.
- Produces (geometry.rs):
  - `fn bounds(shape: &Shape, m: &dyn TextMeasure) -> kurbo::Rect` — Text: `origin` to `origin + text_size + 2*TEXT_PADDING`.
  - `fn hit_test(shape: &Shape, p: Point, tol: f64, m: &dyn TextMeasure) -> bool` — Rect: within `tol + width/2` of any edge (outline only, interior is not a hit); Arrow: distance to segment ≤ `tol + width/2` (use `kurbo::Line` + `ParamCurveNearest`); Text and Redact: inside bounds inflated by `tol`.
  - `enum Handle { TopLeft, TopRight, BottomLeft, BottomRight, Start, End }`
  - `fn handles(shape: &Shape) -> Vec<(Handle, Point)>` — Rect/Redact: four corners; Arrow: Start, End; Text: none.
  - `fn handle_at(shape: &Shape, p: Point, size: f64) -> Option<Handle>` — `p` within `size/2` of a handle point.
  - `fn translate(shape: &mut Shape, d: kurbo::Vec2)`
  - `fn drag_handle(shape: &mut Shape, h: Handle, p: Point)` — moves that corner/end to `p`; Rect/Redact re-normalize afterwards.

- [ ] **Step 1: Write failing tests** (use a `FixedMeasure(Size)` test double)

```rust
#[test] fn rect_outline_hit_but_not_interior() {
    let s = Shape::Rect { rect: Rect::new(10.,10.,110.,60.), style: Style::default() };
    assert!(hit_test(&s, Point::new(10., 30.), 6., &M));   // on left edge
    assert!(hit_test(&s, Point::new(60., 13.), 6., &M));   // near top edge
    assert!(!hit_test(&s, Point::new(60., 35.), 6., &M));  // centre
}
#[test] fn arrow_hit_near_segment_only() {
    let s = Shape::Arrow { start: Point::new(0.,0.), end: Point::new(100.,0.), style: Style::default() };
    assert!(hit_test(&s, Point::new(50., 7.), 6., &M));    // 6 + 4/2 = 8
    assert!(!hit_test(&s, Point::new(50., 9.), 6., &M));
    assert!(!hit_test(&s, Point::new(120., 0.), 6., &M));
}
#[test] fn text_bounds_include_padding() {
    let s = Shape::Text { origin: Point::new(5.,5.), text: "hi".into(), color: Color::Red };
    assert_eq!(bounds(&s, &FixedMeasure(Size::new(20.,16.))), Rect::new(5.,5.,37.,33.));
}
#[test] fn redact_interior_is_hit() { /* point at centre of Redact rect → true */ }
#[test] fn drag_corner_past_opposite_renormalizes() {
    let mut s = Shape::Rect { rect: Rect::new(10.,10.,50.,50.), style: Style::default() };
    drag_handle(&mut s, Handle::TopLeft, Point::new(80., 70.));
    assert_eq!(s, Shape::Rect { rect: Rect::new(50.,50.,80.,70.), style: Style::default() });
}
#[test] fn arrow_end_handle_moves_end() { /* drag End to (5,5) → end == (5,5), start unchanged */ }
#[test] fn text_has_no_handles() { assert!(handles(&text_shape()).is_empty()); }
#[test] fn translate_moves_all_points() { /* Arrow start/end both shifted by (3,4) */ }
```

- [ ] **Step 2: Run** `cargo test` — Expected: compile errors / FAIL.
- [ ] **Step 3: Implement** the interfaces above.
- [ ] **Step 4: Run** `cargo test` — Expected: all PASS.
- [ ] **Step 5: Commit** — `git commit -am "Add annotation model and geometry"` (add new files first).

---

### Task 3: History and editor state machine

**Files:**
- Create: `src/history.rs`, `src/editor.rs` (tests inline)

**Interfaces:**
- Consumes: everything from Task 2.
- Produces (history.rs): `struct History` with `fn record(&mut self, before: Vec<Item>)` (push to undo, clear redo), `fn undo(&mut self, current: Vec<Item>) -> Option<Vec<Item>>`, `fn redo(&mut self, current: Vec<Item>) -> Option<Vec<Item>>`, `fn can_undo/can_redo(&self) -> bool`, `fn clear(&mut self)`.
- Produces (editor.rs):
  - `enum Tool { Select, Rect, Arrow, Text, Redact }`, default `Rect` so a freshly pasted image is immediately drawable (planner's call).
  - `enum PressOutcome { Nothing, StartText(Point), EditText(ItemId) }`
  - `struct Editor { pub items: Vec<Item>, pub selected: Option<ItemId>, pub tool: Tool, pub style: Style, /* private: history, dirty, drag state, next_id */ }`
  - `fn new() -> Self`; `fn reset(&mut self)` — clears items, selection, history, dirty=false (tool and style kept).
  - `fn press(&mut self, p: Point, tol: f64, m: &dyn TextMeasure) -> PressOutcome`
    - Select: handle of the selected item under `p` (size = `tol * 8/6`) → start handle drag; else topmost item hit (last in `items`) → select it and start move; else deselect.
    - Rect/Arrow/Redact: start a new provisional item at `p` (not yet in history).
    - Text: if an existing Text item is hit → `EditText(id)`; else `StartText(p)`.
  - `fn drag_to(&mut self, p: Point)` — updates provisional item / moves / drags handle.
  - `fn release(&mut self, p: Point)` — commits: new item only if its size ≥ `MIN_SIZE` in width or height (arrow: length ≥ `MIN_SIZE`), then it is appended, selected, recorded in history and marks dirty; a move/handle drag that changed the item records **one** history step. Tool stays the same.
  - `fn double_click(&mut self, p: Point, tol: f64, m: &dyn TextMeasure) -> Option<ItemId>` — Text item hit → its id.
  - `fn add_text(&mut self, origin: Point, text: &str)` — ignored if `text.trim()` empty; else append with current color, select, record, dirty.
  - `fn set_text(&mut self, id: ItemId, text: &str)` — empty → remove item; unchanged → no history step.
  - `fn text_of(&self, id: ItemId) -> Option<(Point, String, Color)>`
  - `fn delete_selected(&mut self) -> bool`
  - `fn set_color(&mut self, c: Color)`, `fn set_width(&mut self, w: Width)` — always update `style`; if an item is selected, also change it (one history step, Text/Redact ignore width).
  - `fn undo(&mut self) -> bool`, `fn redo(&mut self) -> bool` (clear selection if the selected id no longer exists), `fn can_undo/can_redo`.
  - `fn is_dirty(&self) -> bool`, `fn mark_saved(&mut self)`.
  - `fn provisional(&self) -> Option<&Shape>` — the item being drawn, for rendering.

- [ ] **Step 1: Write failing tests**

```rust
#[test] fn drawing_a_box_adds_selects_and_dirties() {
    let mut e = Editor::new(); e.tool = Tool::Rect;
    e.press(pt(10.,10.), 6., &M); e.drag_to(pt(60.,40.)); e.release(pt(60.,40.));
    assert_eq!(e.items.len(), 1); assert!(e.is_dirty()); assert_eq!(e.selected, Some(e.items[0].id));
    assert_eq!(e.tool, Tool::Rect);
}
#[test] fn click_without_drag_creates_nothing() {        // Review Focus 5
    for t in [Tool::Rect, Tool::Arrow, Tool::Redact] {
        let mut e = Editor::new(); e.tool = t;
        e.press(pt(10.,10.), 6., &M); e.drag_to(pt(11.,11.)); e.release(pt(11.,11.));
        assert!(e.items.is_empty()); assert!(!e.is_dirty()); assert!(!e.can_undo());
    }
}
#[test] fn move_is_one_undo_step() {
    // draw box, switch to Select, press on its edge, drag in 3 steps, release
    // → undo once restores original rect; undo again removes the box
}
#[test] fn select_picks_topmost_item() { /* two overlapping redacts; press in overlap selects the later one */ }
#[test] fn redo_after_undo_and_new_change_clears_redo() { /* undo, draw new → can_redo() == false */ }
#[test] fn set_color_changes_selected_item_and_style() { /* select box, set_color(Blue) → item style Blue, e.style.color Blue, one undo step */ }
#[test] fn set_text_empty_removes_item() { /* add_text then set_text(id, "  ") → items empty */ }
#[test] fn add_text_blank_is_ignored() { /* add_text(origin, "") → no item, not dirty */ }
#[test] fn text_tool_on_existing_label_edits_it() { /* press returns EditText(id) */ }
#[test] fn mark_saved_clears_dirty_and_next_change_sets_it() {}
#[test] fn reset_clears_everything_but_keeps_tool_and_style() {}
```

- [ ] **Step 2: Run** `cargo test` — Expected: FAIL.
- [ ] **Step 3: Implement** `History` (snapshot of the full `Vec<Item>` per step) and `Editor` as specified.
- [ ] **Step 4: Run** `cargo test` — Expected: all PASS.
- [ ] **Step 5: Commit** — `"Add editor state and undo history"`.

---

### Task 4: Rendering

**Files:**
- Create: `src/render.rs` (tests inline)

**Interfaces:**
- Consumes: Task 2 model/geometry, `Editor::provisional`.
- Produces:
  - `struct PangoMeasure` implementing `TextMeasure` (lays out with `pangocairo` on a 1×1 `ImageSurface`; font `Sans Bold` absolute size `TEXT_FONT_PX`). `fn new() -> Self`.
  - `fn draw_shape(cr: &cairo::Context, shape: &Shape, m: &PangoMeasure)` — strokes with round joins/caps; arrow = line from start to the base of the head + filled triangle head (Global Constraints sizes); text = white rect + `TEXT_BORDER` border in color + text at `origin + TEXT_PADDING`; redact = filled rect.
  - `fn draw_items(cr: &cairo::Context, items: &[Item], skip: Option<ItemId>, m: &PangoMeasure)` — in order; `skip` hides the item currently being text-edited.
  - `fn draw_selection(cr: &cairo::Context, shape: &Shape, scale: f64, m: &PangoMeasure)` — dashed bounding rect (1 screen px, i.e. `1/scale`) plus white squares with dark border at each handle, `8/scale` image px wide.

- [ ] **Step 1: Write failing tests** (render onto a white 200×200 `ImageSurface::create(Format::ARgb32, …)`, read pixels with a `pixel(surface, x, y) -> (u8,u8,u8)` helper)

```rust
#[test] fn redact_fills_opaquely() { /* Redact Black 20..80 → pixel(50,50) == (0,0,0) */ }
#[test] fn rect_is_outline_only() { /* Red Rect 20..180, Medium → pixel(20,100) is red, pixel(100,100) is white */ }
#[test] fn arrow_head_at_end() { /* Arrow (20,100)->(180,100) Thick → pixel(175,100) red; pixel(180-12-1, 100+8) red (inside head), pixel(25,108) white */ }
#[test] fn text_label_has_white_box_and_colored_border() {
    /* Text at (10,10) "Hi", Blue → pixel at border (10+1, 10+1) blue, pixel just inside padding (10+4,10+4) white */
}
#[test] fn skip_hides_item() { /* draw_items with skip=Some(id) → surface stays white */ }
#[test] fn pango_measure_grows_with_text() {
    let m = PangoMeasure::new();
    assert!(m.text_size("Hello world").width > m.text_size("Hi").width);
    assert!(m.text_size("a\nb").height > m.text_size("a").height);
}
```

- [ ] **Step 2: Run** `cargo test render` — Expected: FAIL.
- [ ] **Step 3: Implement** as specified.
- [ ] **Step 4: Run** `cargo test` — Expected: all PASS.
- [ ] **Step 5: Commit** — `"Add Cairo rendering of annotations"`.

---

### Task 5: Export (flatten, PNG/JPEG, texture conversion)

**Files:**
- Create: `src/export.rs` (tests inline, temp files under `std::env::temp_dir()`)

**Interfaces:**
- Consumes: `render::draw_items`, `PangoMeasure`.
- Produces:
  - `enum ImageFormat { Jpeg, Png }`
  - `fn format_for_path(path: &Path) -> (PathBuf, ImageFormat)` — `.png` (any case) → Png; `.jpg`/`.jpeg` → Jpeg; anything else → Jpeg with `.jpg` appended.
  - `fn flatten(base: &cairo::ImageSurface, items: &[Item], m: &PangoMeasure) -> cairo::ImageSurface` — new surface of the base's exact size, base painted, items drawn, no selection.
  - `fn save(surface: &cairo::ImageSurface, path: &Path, fmt: ImageFormat) -> Result<(), String>` — PNG via `surface.write_to_png`; JPEG via `gdk_pixbuf::Pixbuf::from_bytes` (RGB, no alpha, from `argb32_to_rgb`) and `savev(path, "jpeg", &[("quality", "90")])`. Errors become a short human-readable message.
  - `fn argb32_to_rgb(data: &[u8], width: usize, height: usize, stride: usize) -> Vec<u8>` — little-endian Cairo ARGB32 (bytes B,G,R,A) to packed RGB; alpha ignored (screenshots are opaque).
  - `fn surface_from_texture(t: &gdk::Texture) -> cairo::ImageSurface` — via `gdk::TextureDownloader` with `MemoryFormat::B8g8r8a8Premultiplied`, keeping `t.width()×t.height()`.
  - `fn texture_from_surface(s: &cairo::ImageSurface) -> gdk::Texture` — `gdk::MemoryTexture` with the same format/stride.

- [ ] **Step 1: Write failing tests**

```rust
#[test] fn format_for_path_rules() {
    assert_eq!(format_for_path(Path::new("/x/a.PNG")).1, ImageFormat::Png);
    assert_eq!(format_for_path(Path::new("/x/a.jpeg")), (PathBuf::from("/x/a.jpeg"), ImageFormat::Jpeg));
    assert_eq!(format_for_path(Path::new("/x/shot")), (PathBuf::from("/x/shot.jpg"), ImageFormat::Jpeg));
}
#[test] fn argb_to_rgb_swaps_channels_and_respects_stride() {
    let data = [0x10,0x20,0x30,0xff, 0,0,0,0 /*padding*/];
    assert_eq!(argb32_to_rgb(&data, 1, 1, 8), vec![0x30,0x20,0x10]);
}
#[test] fn flatten_keeps_original_size() {           // Review Focus 2
    let base = ImageSurface::create(Format::ARgb32, 3840, 2160).unwrap();
    let out = flatten(&base, &[], &PangoMeasure::new());
    assert_eq!((out.width(), out.height()), (3840, 2160));
}
#[test] fn saved_jpeg_and_png_reload_with_same_size_and_colors() {
    // 64×48 white base + Black Redact 0..32 x 0..48; save both formats; reload with Pixbuf::from_file;
    // size == 64×48; pixel (10,10) ≈ black (JPEG tolerance ±8); pixel (50,10) ≈ white
}
#[test] fn save_into_missing_dir_returns_err() {      // Review Focus 4
    assert!(save(&surface, Path::new("/nonexistent/dir/a.jpg"), ImageFormat::Jpeg).is_err());
}
```

- [ ] **Step 2: Run** `cargo test export` — Expected: FAIL.
- [ ] **Step 3: Implement** as specified (texture functions are exercised manually in Tasks 6 and 8).
- [ ] **Step 4: Run** `cargo test` — Expected: all PASS.
- [ ] **Step 5: Commit** — `"Add flattening and PNG/JPEG export"`.

---

### Task 6: Canvas — paste, draw, select, tools, undo

**Files:**
- Create: `src/view.rs` (tests inline)
- Modify: `src/app.rs`

**Interfaces:**
- Consumes: `Editor`, `render::*`, `export::surface_from_texture`.
- Produces (view.rs): `struct ViewTransform { scale: f64, offset: Vec2 }` with `fn fit(image: Size, widget: Size) -> Self` (scale = `min(1, w_ratio, h_ratio)`, image centred), `fn to_image(&self, p: Point) -> Point`, `fn to_widget(&self, p: Point) -> Point`.
- Produces (app.rs): shared canvas state `Rc<RefCell<CanvasState { editor: Editor, image: Option<cairo::ImageSurface>, measure: PangoMeasure, editing: Option<ItemId> }>>` used by the draw func; `AppMsg` variants `Paste`, `Pasted(Option<gdk::Texture>)`, `SetTool(Tool)`, `SetColor(Color)`, `SetWidth(Width)`, `Undo`, `Redo`, `DeleteSelected`, `Redraw`.

- [ ] **Step 1: Write failing tests for `ViewTransform`**

```rust
#[test] fn small_image_is_not_upscaled_and_is_centred() {
    let v = ViewTransform::fit(Size::new(400.,300.), Size::new(800.,600.));
    assert_eq!(v.scale, 1.0); assert_eq!(v.to_widget(Point::ZERO), Point::new(200.,150.));
}
#[test] fn large_image_scales_down_and_round_trips() {   // Review Focus 2
    let v = ViewTransform::fit(Size::new(3840.,2160.), Size::new(960.,600.));
    assert_eq!(v.scale, 0.25);
    let p = Point::new(1234.,567.); let q = v.to_image(v.to_widget(p));
    assert!((p - q).hypot() < 1e-9);
}
```

- [ ] **Step 2: Run** `cargo test view` — FAIL; **Step 3: implement** `view.rs`; **Step 4: run** — PASS.

- [ ] **Step 5: Wire the canvas in `app.rs`**
  - Draw func: grey background; if an image exists apply `ViewTransform::fit`, paint image, `draw_items(skip = editing)`, provisional shape, `draw_selection` for the selected item.
  - `GestureDrag` (button 1) → `editor.press/drag_to/release` with points converted by `to_image` and `tol = 6 / scale`; `GestureClick` with `n_press == 2` → `double_click`. Queue redraw after each.
  - Ctrl+V → `display.clipboard().read_texture_future()`; `Some(texture)` → `editor.reset()`, store `surface_from_texture`, hide hint; `None`/error → toast "Clipboard does not contain an image" and change nothing (Review Focus 3). (The "Start a new image?" confirmation is added in Task 8.)
  - Tool/color/width buttons and shortcuts V/R/A/T/X, Ctrl+Z, Ctrl+Y, Delete/BackSpace via an `EventControllerKey` on the window that returns `Propagation::Proceed` while `editing.is_some()` (Review Focus 1). Buttons reflect `editor.tool/style`; Undo/Redo buttons sensitive per `can_undo/can_redo`. Changing color/width while an item is selected changes it.
  - Selecting an item updates the color/width buttons to that item's style (planner's call).
  - `StartText`/`EditText` outcomes are ignored for now (Task 7).

- [ ] **Step 6: Manual verification** — `cargo run`; copy a screenshot with GNOME's tool (Print → select area → it lands on the clipboard):
  1. Ctrl+V shows it; a 4K screenshot is scaled down to fit.
  2. Draw boxes, arrows, redactions; several in a row with the same tool.
  3. V, click an edge → handles; move, resize, drag arrow ends; Delete removes it.
  4. Change color/width with an item selected → item changes.
  5. Ctrl+Z / Ctrl+Y walk through every step; a drag is one step.
  6. Copy some plain text in another app, press Ctrl+V → toast, image unchanged.

- [ ] **Step 7: Commit** — `"Wire canvas: paste, draw, select, undo"`.

---

### Task 7: In-place text editing

**Files:**
- Create: `src/text_edit.rs`
- Modify: `src/app.rs`

**Interfaces:**
- Consumes: `PressOutcome::{StartText, EditText}`, `Editor::{double_click, add_text, set_text, text_of}`, `CanvasState.editing`, `ViewTransform`.
- Produces: `struct TextEditor` wrapping a `gtk::TextView` placed in the canvas `gtk::Overlay` (halign/valign Start, margins from `to_widget(origin + TEXT_PADDING)`), styled via a CSS provider to match the label (Sans Bold, `TEXT_FONT_PX × scale` px, item color, white background, colored border). Methods: `fn open(&self, origin_widget: Point, scale: f64, color: Color, initial: &str)`, `fn close(&self) -> String`, `fn is_open(&self) -> bool`. Signal to the app: `AppMsg::TextCommit`.

- [ ] **Step 1: Implement** `TextEditor`. Behaviour:
  - Text tool click on empty area → open empty editor at that point (`editing = None`, remember origin); commit → `add_text`.
  - Text tool click on a label, or double-click with Select → `editing = Some(id)`, label hidden while editing, editor prefilled; commit → `set_text`.
  - Enter inserts a newline; Escape commits; pressing on the canvas outside the editor commits (and then the press is handled normally); switching tool, paste, copy, save and close all commit first.
  - Empty text discards (new) or deletes (existing).
  - While open: window key controller lets all keys through (Task 6), so letters, Delete, Ctrl+C, Ctrl+Z act inside the TextView.

- [ ] **Step 2: Manual verification**
  1. T, click, type "Remove the arrow, then delete X" with Enter for a second line → no tool switches, nothing deleted (Review Focus 1); Escape → label appears in boxed style at the same place and size it was typed.
  2. Double-click with Select → edit, change text, click outside → updated; Ctrl+Z restores old text.
  3. Clear all text and commit → label removed.
  4. Ctrl+C while typing copies the selected text, not the image.
  5. Labels can be moved with Select and show no resize handles.

- [ ] **Step 3: Commit** — `"Add in-place text editing"`.

---

### Task 8: Copy, save, save as, unsaved-changes dialogs

**Files:**
- Modify: `src/app.rs`

**Interfaces:**
- Consumes: `export::{flatten, save, format_for_path, texture_from_surface}`, `Editor::{is_dirty, mark_saved, reset}`.
- Produces: `AppModel.file: Option<PathBuf>`; `AppMsg` variants `Copy`, `Save`, `SaveAs`, `SaveChosen(Option<PathBuf>)`, `CloseRequested`, `ConfirmNew`. Title shows `Annotshot`, then `• <file name>` once saved, prefixed with `*` when dirty (planner's call).

- [ ] **Step 1: Implement**
  - **Ctrl+C / Copy button:** commit text editing, `flatten`, `clipboard.set_texture(texture_from_surface)`, toast "Copied". Dirty unchanged.
  - **Ctrl+S / Save…:** no `file` → same as Save As; else `save` to `file`, `mark_saved`, toast "Saved".
  - **Ctrl+Shift+S:** `gtk::FileDialog` with filters "JPEG image" (`*.jpg`, `*.jpeg`, default) and "PNG image" (`*.png`), initial name `screenshot-YYYY-MM-DD-HHMMSS.jpg` (local time via `glib::DateTime::now_local`), initial folder = folder of `file` if any. Result → `format_for_path` → `save`; on success set `file` to the adjusted path and `mark_saved`; on error toast "Could not save: <message>" and leave `file`/dirty unchanged; cancel → nothing (Review Focus 4).
  - Save/Copy disabled while no image.
  - **Ctrl+V with an image open:** `adw::AlertDialog` "Start a new image?" — if dirty, body "Your changes to the current image have not been saved." with responses Cancel / Discard (destructive) / Save; otherwise Cancel / New Image. Save → save (via dialog if needed), then paste; Discard/New Image → paste. A new image clears `file`.
  - **Close:** `close-request` while dirty → same dialog wording "Save changes before closing?" with Cancel / Discard / Save; Save that gets cancelled or fails keeps the window open.

- [ ] **Step 2: Manual verification**
  1. Ctrl+S on a new image → dialog with JPEG selected and a timestamped name; save; Ctrl+S again → no dialog, file updated (check mtime).
  2. Ctrl+Shift+S → choose `x.png` → PNG written (`file x.png` says PNG); next Ctrl+S writes the PNG.
  3. Type a name without extension → `.jpg` added.
  4. Save into a read-only folder → error toast, title still shows `*`.
  5. Cancel the dialog → nothing changes.
  6. Copy → paste into another app (e.g. a chat or LibreOffice) at full resolution with annotations.
  7. Draw, Ctrl+V → warning with three choices; each behaves as described. No changes → "Start a new image?" with New Image.
  8. Draw, close → warning; Cancel keeps it; Save saves then closes; Copy-only then close → warning still appears.

- [ ] **Step 3: Commit** — `"Add copy, save and unsaved-changes dialogs"`.

---

### Task 9: Copied image survives closing the app

**Files:**
- Modify: `src/app.rs`, `src/main.rs`

**Interfaces:**
- Consumes: Copy from Task 8, `gdk::Clipboard::is_local` / `notify::local`.

- [ ] **Step 1: Verify GNOME's behaviour (manual)**
Run the app, paste, draw, Ctrl+C, close (Discard). Then paste into another app.
- Pastes correctly → GNOME keeps the image. Record this in the commit message, skip Step 2, go to Step 3.
- Nothing pastes → do Step 2.

- [ ] **Step 2 (only if needed): Keep the process alive while it owns the clipboard**
On window close (after the unsaved-changes flow), if `clipboard.is_local()` is true: hide the window, `app.hold()` and keep the guard; connect `notify::local` and drop the guard (quit) once it becomes false. If the app is activated again while hidden, reset the model (`editor.reset()`, no image, `file = None`) and present the window. Exit immediately when not owning the clipboard.

- [ ] **Step 3: Manual verification**
  1. Copy, close, paste into another app → works.
  2. After that, copy something else elsewhere → `pgrep annotshot` shows no process within a second (only relevant with Step 2).
  3. Copy, close, launch again → fresh empty window; the earlier copy still pastes.
  4. Close without copying → process exits right away.

- [ ] **Step 4: Commit** — `"Keep copied image available after closing"` (or `"Document clipboard persistence on GNOME"` if Step 2 was not needed, with the finding in the message).

---

### Task 10: Launcher entry, icon, install script

**Files:**
- Create: `data/net.gappc.Annotshot.desktop`, `data/net.gappc.Annotshot.svg`, `install.sh`, `README.md`

**Interfaces:**
- Consumes: release binary `target/release/annotshot`.

- [ ] **Step 1: Write the desktop file**

```ini
[Desktop Entry]
Type=Application
Name=Annotshot
Comment=Annotate screenshots from the clipboard
Exec=@BINDIR@/annotshot
Icon=net.gappc.Annotshot
Terminal=false
Categories=Graphics;Utility;
Keywords=screenshot;annotate;arrow;markup;
StartupNotify=true
```

- [ ] **Step 2: Create a simple scalable SVG icon** (e.g. a rounded rectangle "screenshot" with a red arrow and box), 128×128 viewBox, flat colors from the palette.

- [ ] **Step 3: Write `install.sh`** (`set -euo pipefail`): `cargo build --release`; install binary to `~/.local/bin/annotshot`; desktop file to `~/.local/share/applications/` with `@BINDIR@` replaced by the absolute `~/.local/bin` path; icon to `~/.local/share/icons/hicolor/scalable/apps/`; run `update-desktop-database ~/.local/share/applications` and `gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor` if available (ignore failures). Support `./install.sh --uninstall` removing those three files. Also set the window's icon name to `net.gappc.Annotshot` in `app.rs`.

- [ ] **Step 4: Write `README.md`**: what it is, prerequisites (Task 1 Step 1 command), `./install.sh`, shortcut list from Global Constraints, note that Redact is the safe way to hide sensitive data (pixelate/blur is not).

- [ ] **Step 5: Verify**
  1. `./install.sh` succeeds; "Annotshot" appears in GNOME's app search with the icon and launches.
  2. Launching it a second time while open brings the existing window to the front.
  3. `./install.sh --uninstall` removes it from the launcher.
  4. `cargo test && cargo clippy -- -D warnings` clean.

- [ ] **Step 6: Commit** — `"Add launcher entry, icon and install script"`.
