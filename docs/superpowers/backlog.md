# Backlog

Feedback after v0.1.0 (2026-11-17). Each task is handled in its own session
to keep the context clean. Status: `offen` → `brainstorming` → `geplant` →
`in Arbeit` → `erledigt` (oder `zurückgestellt`).

| # | Task                              | Status | Größe | Berührt                                    |
|---|-----------------------------------|--------|-------|--------------------------------------------|
| 1 | Weiß in der Farbpalette           | erledigt | S     | `model.rs` (Color), `app.rs` (Swatches)    |
| 2 | Textgröße einstellbar             | erledigt | S–M   | `model.rs` (Shape::Text), `text_edit.rs`, `render.rs`, `app.rs` |
| 3 | Zoom mit Strg + Mausrad           | zurückgestellt | M     | `view.rs`, `canvas.rs`, `text_edit.rs`     |
| 4 | Bereich auswählen und weiterverwenden | brainstorming | M–L | neues Werkzeug, `history.rs`, `export.rs`, `files.rs` |
| 5 | Bild drehen                       | erledigt | M     | `history.rs`, `render.rs`, `geometry.rs`   |

Empfohlene Reihenfolge: 1 → 2 → 5 → 4 (3 zurückgestellt). Die ersten beiden sind klein und
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

**Zurückgestellt (2026-11-17):** nur eine Idee, bisher kein konkreter Bedarf.
Ohne Anlass wären Zoombereich, Kantenschärfe und Verschieben geraten. Wieder
aufnehmen, wenn ein echter Fall auftaucht; mit Task 4 (Zuschneiden) kann der
Bedarf auch kleiner werden.

Befund aus dem Brainstorming:
- Die Ansicht ist zustandslos: `CanvasState::view()` rechnet sie bei jedem
  Zeichnen/Klick per `ViewTransform::fit` aus der Widget-Größe neu. Zoom braucht
  erstmals gespeicherten Zustand (Faktor + Verschiebung) in `CanvasState`.
- Die Leinwand ist eine nackte `DrawingArea` in einem `Overlay`, kein
  `ScrolledWindow` – keine Scrollbalken, keine Scroll-Events bisher.
- Geht schon mit: Klicktoleranz und Auswahl-Griffe rechnen in Bildschirmpixeln
  (`/ view.scale`), Drags merken sich die Ansicht vom Start.
- Text-Editor: wird beim Öffnen platziert; bei Größenänderung wird heute
  committet. Neu platzieren ginge, `placement()` braucht nur Ursprung + Skalierung.
- Bild wird mit `cairo::Filter::Good` gezeichnet; bei starker Vergrößerung
  weich, für pixelgenaues Arbeiten wäre `Nearest` nötig.

Ursprüngliche Notizen:

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

**Vorüberlegung (2026-11-17, beim Brainstorming zu Task 5):**
- **b) Bereich kopieren** ist klein: neues Werkzeug zum Aufziehen eines
  Rechtecks, dann nur diesen Bereich kopieren (`export::flatten` + Zuschnitt,
  Copy gibt es schon). Das Grundbild bleibt unverändert, also kein Umbau der
  Undo-Liste. Mit dem vorhandenen Strg+V wird der Ausschnitt danach zum neuen
  Bild (Rückfrage „Start a new image?“ wie heute).
- **c) als Ersetzen** ist praktisch b + Strg+V; ein Knopf „Auf Auswahl
  zuschneiden“ würde das nur zusammenfassen. Annotationen landen dabei fest im
  Bild; zurück zum Vollbild nur über eine vorher gespeicherte Datei.
  **c) als Objekt** auf dem Bild wäre groß (neue Annotationsart mit Bild).
- Erst **a) Zuschneiden mit bearbeitbaren Annotationen und Undo** braucht die
  gemeinsame Grundlage (unten). Task 5 hat sie **nicht** gebaut: Drehen
  übernimmt Annotationen ins Bild oder löscht sie und leert die Undo-Liste
  (`Editor::clear_for_rotation`); dieses Muster ginge auch fürs Zuschneiden.

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
(Stand 2026-11-17: Task 5 ist ohne sie ausgekommen, siehe Vorüberlegung bei 4.)
