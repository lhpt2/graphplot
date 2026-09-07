# graphplot

Ein Rust/eframe(egui)-Programm zum Plotten, manuellen Bearbeiten und
Kombinieren von Graphen.

## Funktionen

- **Zufallsgraphen erzeugen** (Erdős–Rényi-Modell `G(n, p)`), gerichtet oder
  ungerichtet, mit einstellbarem Seed für Reproduzierbarkeit.
- **Manuelles Erstellen/Bearbeiten** direkt auf der Zeichenfläche:
  - *Knoten hinzufügen*: Klick auf freie Fläche
  - *Kante hinzufügen*: zwei Knoten nacheinander anklicken
  - *Verschieben*: Knoten per Drag&Drop bewegen (überschreibt kurzzeitig das
    automatische Kräfte-basierte Layout)
  - *Löschen*: Klick auf einen Knoten löscht ihn (inkl. Kanten); Klick nahe
    einer Kante löscht diese
  - Knoten lassen sich umbenennen
- Jeder Graph lebt in einem eigenen **Tab** und wird live mit einem
  einfachen Fruchterman-Reingold-artigen Feder-Layout animiert.
- **Mengenoperationen** über das rechte Panel erzeugen aus ein oder zwei
  bestehenden Graphen einen neuen Graphen `G' = (V', E')`, der automatisch
  in einem neuen Tab angelegt und geplottet wird:
  - Vereinigung `A ∪ B`
  - Durchschnitt `A ∩ B`
  - Differenz `A \ B` (Knoten- und Kantenmenge)
  - Kantendifferenz `E(A) \ E(B)` (Knotenmenge bleibt erhalten)
  - Symmetrische Differenz `A △ B`
  - Komplement von `A`
  - Induzierter Teilgraph von `A` auf einer wählbaren Knotenteilmenge
  - Umwandlung von `A` zwischen gerichtet und ungerichtet

## Knotenidentität bei Mengenoperationen

Knoten werden über ihr Label (z. B. `v0`, `v3`, ...) identifiziert. Zwei
Graphen, die einen Knoten mit demselben Label besitzen, referenzieren bei
Mengenoperationen denselben Knoten in `V' ` – das entspricht der
mathematischen Mengen-Definition `G = (V, E)`. Sollen zwei Graphen als
vollständig disjunkt behandelt werden, empfiehlt es sich, ihre Knoten vorher
über "Knoten umbenennen" eindeutig zu benennen.

Werden zwei Graphen unterschiedlicher Ausrichtung (gerichtet/ungerichtet)
kombiniert, wird vor der Operation automatisch in den gewählten
Ziel-Modus konvertiert (ungerichtete Kante `{a,b}` ⇄ gerichtete Kantenpaare
`(a,b)` und `(b,a)`).

## Bauen & Starten

```bash
cargo run --release
```

## Tests

Die Graph-Mengenoperationen sind mit Unit-Tests abgedeckt:

```bash
cargo test
```
