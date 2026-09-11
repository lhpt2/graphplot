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

## Lua-Filter

Zusätzlich zur Mengen-Methode gibt es einen Bereich **"Lua-Filter"**, mit
dem sich aus einem Graphen per Lua-Skript ein neuer Graph bauen lässt –
für Transformationen, die sich nicht sinnvoll als Mengenausdruck
formulieren lassen (z. B. iterative/algorithmische Logik). Läuft über
[mlua](https://github.com/mlua-rs/mlua) mit gebündeltem Lua 5.4 (kein
System-Lua nötig) und der abgesicherten Standardbibliothek (kein Datei-/
Prozess-/OS-Zugriff aus den Skripten heraus).

Ein Skript muss eine Funktion `filter(g)` definieren, die den
Eingabegraphen erhält und einen (neuen oder veränderten) Graphen
zurückgibt:

```lua
function filter(g)
    local out = new_graph(g:directed())
    for _, v in ipairs(g:vertices()) do
        if g:degree(v) == 0 then
            out:add_vertex(v)
        end
    end
    return out
end
```

API auf einem Graphen `g`:

| Methode | Bedeutung |
|---|---|
| `g:vertices()` | Liste aller Knoten |
| `g:edges()` | Liste von `{von, bis}`-Paaren |
| `g:has_edge(a, b)` | Kante vorhanden? (bei ungerichteten Graphen symmetrisch) |
| `g:degree(v)` | Anzahl anliegender Kanten |
| `g:neighbors(v)` | Liste benachbarter Knoten |
| `g:directed()` | gerichtet? |
| `g:add_vertex(v)` / `g:add_edge(a, b)` | Knoten/Kante hinzufügen |
| `g:remove_vertex(v)` / `g:remove_edge(a, b)` | Knoten/Kante entfernen |

Globale Hilfsfunktionen: `new_graph(gerichtet)` (leerer neuer Graph),
`complement(g)`, `induced_subgraph(g, {liste})`,
`greedy_independent_set(g)` (liefert eine Knotenliste) – letztere drei
rufen direkt die entsprechenden, bereits in Rust implementierten
Graph-Operationen auf.

Eine gemeinsame **Bibliothek** (eigener, persistenter Lua-Quelltext) wird
vor jedem Filter-Skript geladen, sodass eigene Hilfsfunktionen
(`function is_isolated(g, v) ... end`) über mehrere Filter hinweg
wiederverwendet werden können. Fertige Filter lassen sich unter einem
Namen speichern, bearbeiten, löschen und über eine Auswahl (Graph +
Filter) auf einen bestehenden Graphen anwenden.

Ein Filter-Ergebnis wird dabei genauso wie ein Mengenausdruck-Ergebnis
behandelt: Knoten- und Kantenmenge des Ergebnisgraphen landen automatisch
als zwei benannte Mengen ("Name (Knoten)" / "Name (Kanten)") in der
Liste "Definierte Mengen" – mit eigener Farbe und direkt über
"Markierungen" in jedem Graph-Tab farbig hervorhebbar, genau wie jede
andere über einen Mengenausdruck erzeugte Menge. Zusätzlich kann
optional (Checkbox "Auch als neuer Tab öffnen", standardmäßig an) der
komplette Ergebnisgraph als neuer, geplotteter Tab geöffnet werden.

## Unabhängige Menge (Independent Set)

Eine unabhängige Menge (kein Knotenpaar darin durch eine Kante verbunden)
ist *kein* per-Knoten-Filter, sondern eine Eigenschaft der gesamten
gewählten Teilmenge – das lässt sich nicht als einfacher Mengenbildner
ausdrücken (die *größte* unabhängige Menge exakt zu finden ist NP-schwer).
Dafür gibt es im Bereich "Mengenausdrücke" einen eigenen Abschnitt
**"Unabhängige Menge (Greedy)"**: Graph auswählen, Name vergeben,
"Berechnen" klicken – erzeugt per Greedy-Heuristik (Knoten in aufsteigender
Grad-Reihenfolge aufnehmen, sofern keine Kante zu einem schon gewählten
Knoten besteht) eine *maximale* (nicht notwendig größtmögliche)
unabhängige Menge als benannte Knotenmenge, direkt nutzbar für die
Markierung im Graphen.

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
| `{ (u,v) : u in S1, v in S2, Bedingung }` | Mengenbildner für **Kanten** aus zwei Knotenvariablen | |
| `{ u, v : u in S1, v in S2, Bedingung }` | wie oben, aber **ohne** Klammern: vereinigt die Werte von `u` **und** `v` in *eine* Knotenmenge (keine Kanten!) | |

⚠️ Die letzte Zeile ist eine häufige Falle: `{ v, w : v in V(G1), w in V(G1),
v != w, (v,w) notin E(G1) }` liefert (fast) **alle** Knoten, nicht die
"unverbundenen" – weil praktisch jeder Knoten irgendeinen Nicht-Nachbarn hat
und dadurch über mindestens eine bestehende Kombination in die Vereinigung
gelangt. Für "Knoten ohne Kante zu irgendeinem anderen" die
Ein-Variablen-`forall`-Form (siehe `Isoliert` unten) verwenden; für die
tatsächliche Menge der Nicht-Kanten (als Kantenmenge) die Klammerform
`{ (u,v) : ... }` (siehe `Komplement` unten).

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

### Mengen im vorhandenen Graphen einfärben

Jede definierte Menge bekommt automatisch eine eigene Farbe aus einer
Palette (in der Liste "Definierte Mengen" per Klick auf das Farbfeld
jederzeit änderbar – ein normaler Farbwähler). Jeder Graph-Tab hat oben
einen **"Markierungen"**-Bereich mit einer Checkbox pro definierter Menge:
Aktivierte Mengen werden mit ihrer zugehörigen Farbe direkt im bestehenden
Graphen hervorgehoben (Knoten oder Kanten, je nach Mengentyp) – auch mehrere
gleichzeitig, jede in ihrer eigenen Farbe. Das funktioniert für jede
beliebige Menge, unabhängig davon, aus welchem Graphen sie berechnet wurde;
es werden nur die tatsächlich im aktuellen Tab vorhandenen Knoten/Kanten
markiert.

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
