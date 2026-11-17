# Backlog

Feedback after v0.1.0 (2026-11-17). Each task is handled in its own session
to keep the context clean. Status: `offen` → `brainstorming` → `geplant` →
`in Arbeit` → `erledigt`.

| # | Task                              | Status | Größe | Berührt                                    |
|---|-----------------------------------|--------|-------|--------------------------------------------|
| 1 | Weiß in der Farbpalette           | erledigt | S     | `model.rs` (Color), `app.rs` (Swatches)    |
| 2 | Textgröße einstellbar             | offen  | S–M   | `model.rs` (Shape::Text), `text_edit.rs`, `render.rs`, `app.rs` |
| 3 | Zoom mit Strg + Mausrad           | offen  | M     | `view.rs`, `canvas.rs`, `text_edit.rs`     |
| 4 | Bereich auswählen und weiterverwenden | offen | M–L | neues Werkzeug, `history.rs`, `export.rs`, `files.rs` |
| 5 | Bild drehen                       | offen  | M     | `history.rs`, `render.rs`, `geometry.rs`   |

Empfohlene Reihenfolge: 1 → 2 → 3 → 5 → 4. Die ersten beiden sind klein und
unabhängig. 4 und 5 ändern beide das Grundbild; wer zuerst kommt, baut die
gemeinsame Grundlage (siehe unten), der zweite nutzt sie.

## 1. Weiß in der Farbpalette

Weiße Flächen mit Weiß überdecken (Redact/Box auf weißem Hintergrund).

- `Color::ALL` hat 5 Farben, die Swatches in `app.rs` sind einzeln
  ausgeschrieben; ein sechster Swatch kommt dazu.
- Zu prüfen: Lesbarkeit, wo Weiß auf Weiß landet: Marker-Nummer (heute
  vermutlich weiß auf Farbe), Text-Editor-Rahmen, Swatch selbst braucht einen
  sichtbaren Rand. Highlight in Weiß ist wohl wirkungslos – erlauben oder nicht?
- Kurzer Shortcut-/Reihenfolge-Check: wo in der Palette steht Weiß (neben Schwarz)?

## 2. Textgröße einstellbar

Analog zu Breite bei Box/Arrow (Thin/Medium/Thick).

- `Shape::Text` hat heute nur `color`; feste Größe `TEXT_FONT_PX = 16`.
- Naheliegend: `Width` wiederverwenden (z. B. 12/16/24 px) und die
  Breiten-Buttons auch beim Text-Werkzeug zeigen.
- Offene Fragen: drei Stufen reichen, oder mehr? Soll ein Klick auf eine Stufe
  die Größe eines ausgewählten/offenen Texts ändern (wie die Farbe heute)?

## 3. Zoom mit Strg + Mausrad

- `view.rs` kennt nur „einpassen, nie vergrößern“. Dazu kommen Zoomfaktor und
  Verschiebung (Pan).
- Offene Fragen: Zoom zur Mausposition hin? Wie wird verschoben – Mausrad
  ohne Strg (vertikal), Shift+Rad (horizontal), mittlere Maustaste ziehen,
  Leertaste ziehen? Shortcuts Strg+0 (einpassen) / Strg+1 (100 %)? Anzeige des
  Zoomgrads? Kantenschärfe bei Vergrößerung (nearest neighbour)?
- Der Text-Editor-Overlay rechnet schon mit `view.scale`, sollte also mitgehen –
  testen.

## 4. Bereich auswählen und weiterverwenden

Aus einem Vollbild-Screenshot nur einen Teil weiterverwenden.

- Mehrere mögliche Formen, im Brainstorming festlegen:
  a) **Zuschneiden** (Crop-Werkzeug): Bild wird auf den Bereich reduziert,
     Annotationen außerhalb fallen weg/werden verschoben. Undo-fähig.
  b) **Bereich kopieren**: Auswahl → Zwischenablage (mit oder ohne
     Annotationen?).
  c) **Bereich als neues Bild öffnen** / aus der Zwischenablage einfügen
     (Strg+V als neues Bild oder als Ebene?).
- „Neu einfügen“ klärt sich im Brainstorming: ersetzt das eingefügte Bild das
  aktuelle, oder wird es als Objekt auf das aktuelle gelegt?

## 5. Bild drehen

- 90° links/rechts (180° als Folge)? Freie Winkel vermutlich nicht nötig.
- Annotationen müssen mitgedreht werden (Koordinaten transformieren); Text
  bleibt dabei lesbar waagerecht oder dreht sich mit?
- Shortcut-Vorschlag: Strg+R / Strg+Shift+R – prüfen, dass nichts kollidiert
  (R ist Box).

## Gemeinsame Grundlage für 4 und 5

`History` speichert nur die Item-Liste, nicht das Grundbild. Zuschneiden und
Drehen verändern das Grundbild, also muss ein Undo-Schritt künftig auch das
Bild (oder die Transformation) enthalten. Das ist die eigentliche
Architekturänderung; sie wird beim ersten der beiden Tasks gebaut.
