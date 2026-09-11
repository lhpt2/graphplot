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

## Mengenausdrücke (Text)

Neben dem einfachen Dropdown-Panel gibt es ein Textfeld für frei
formulierte Mengenausdrücke in mathematischer Schreibweise. Damit lassen
sich beliebig viele Operationen hintereinander ausführen und benannte
Zwischenmengen (Knoten- *oder* Kantenmengen) definieren, die spätere
Ausdrücke wieder referenzieren können.

Eine Eingabe besteht aus einer oder mehreren Zeilen (oder `;`-getrennten
Anweisungen) der Form:

```
Name = Mengenausdruck
Name = (Knotenausdruck, Kantenausdruck)     # erzeugt einen neuen Graphen G' = (V', E') als Tab
```

Grundbausteine:

| Syntax | Bedeutung | Unicode-Alternative |
|---|---|---|
| `V(G)` / `E(G)` | Knoten-/Kantenmenge des Tabs `G` | |
| `A \| B`, `A & B`, `A - B`, `A ^ B` | Vereinigung, Durchschnitt, Differenz, symm. Differenz | `∪ ∩ ∖ △` |
| `{}` / `empty` | leere Menge | `∅` |
| `x in S`, `x notin S` | Element-/Nichtelement-Beziehung | `∈ ∉` |
| `x = y`, `x != y` | Gleichheit/Ungleichheit | `≠` |
| `S <= T` | Teilmenge | `⊆` |
| `and`, `or`, `not` | Aussagenlogik | `∧ ∨ ¬` |
| `forall x in S . P`, `exists x in S . P` | Quantoren | `∀ ∃` |
| `{ v : v in S, Bedingung }` | Mengenbildner über Knoten (oder Kanten, wenn `S` eine Kantenmenge ist) | |
| `{ (u,v) : u in S1, v in S2, Bedingung }` | Mengenbildner für Kanten aus zwei Knotenvariablen | |

Beispiele:

```
# Knoten ohne jede Kante in G1
Isoliert = { v : v in V(G1), forall u in V(G1) . not ((v,u) in E(G1) or (u,v) in E(G1)) }

# Alle "fehlenden" Kanten (Komplement) über einer Knotenmenge
Komplement = { (u,v) : u in V(G1), v in V(G1), u != v, (u,v) notin E(G1) }

# mehrere Schritte hintereinander, letzter Schritt erzeugt einen neuen Graphen-Tab
Kern = V(G1) - Isoliert
G' = (Kern, E(G1))
```

Da egui's Standardschrift nicht jedes mathematische Sonderzeichen
darstellen kann, ist die ASCII-Schreibweise (`|`, `&`, `-`, `^`, `in`,
`forall`, `exists`, ...) die primäre, garantiert lesbare Syntax; die
Unicode-Symbole werden vom Parser zusätzlich als Synonyme akzeptiert.

Definierte Mengen erscheinen in einer Liste mit Größe/Typ und lassen sich
per Klick auch direkt als eigener Graph-Tab öffnen (Knoten ohne Kanten
bzw. Kanten mit ihren Endpunkten).

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
