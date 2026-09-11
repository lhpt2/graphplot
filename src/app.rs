use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use rand::SeedableRng;

use crate::expr::{self, EvalCtx, SetValue, StmtResult};
use crate::graph::{Graph, VertexId};
use crate::layout::Layout;

const PALETTE: [Color32; 8] = [
    Color32::from_rgb(255, 196, 0),
    Color32::from_rgb(255, 99, 132),
    Color32::from_rgb(54, 162, 235),
    Color32::from_rgb(75, 192, 192),
    Color32::from_rgb(153, 102, 255),
    Color32::from_rgb(255, 159, 64),
    Color32::from_rgb(199, 199, 60),
    Color32::from_rgb(120, 220, 120),
];

#[derive(Clone)]
struct NamedSetEntry {
    value: SetValue,
    color: Color32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditMode {
    AddVertex,
    AddEdge,
    Move,
    Delete,
}

impl EditMode {
    fn label(self) -> &'static str {
        match self {
            EditMode::AddVertex => "Knoten hinzufügen",
            EditMode::AddEdge => "Kante hinzufügen",
            EditMode::Move => "Verschieben",
            EditMode::Delete => "Löschen",
        }
    }
}

struct GraphTab {
    id: u64,
    name: String,
    graph: Graph,
    layout: Layout,
    mode: EditMode,
    random_n: usize,
    random_p: f32,
    random_seed: u64,
    pending_edge_from: Option<VertexId>,
    rename_target: Option<VertexId>,
    rename_buffer: String,
    status: Option<String>,
    highlighted: BTreeSet<String>,
}

impl GraphTab {
    fn new(id: u64, name: impl Into<String>, directed: bool) -> Self {
        Self {
            id,
            name: name.into(),
            graph: Graph::new(directed),
            layout: Layout::default(),
            mode: EditMode::AddVertex,
            random_n: 10,
            random_p: 0.25,
            random_seed: id.wrapping_mul(2654435761).wrapping_add(1),
            pending_edge_from: None,
            rename_target: None,
            rename_buffer: String::new(),
            status: None,
            highlighted: BTreeSet::new(),
        }
    }

    fn from_graph(id: u64, name: impl Into<String>, graph: Graph) -> Self {
        let mut tab = Self::new(id, name, graph.directed);
        tab.graph = graph;
        tab.layout.reset_circle(&tab.graph);
        tab
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpKind {
    Union,
    Intersection,
    Difference,
    EdgeDifference,
    SymmetricDifference,
    Complement,
    Induced,
    ConvertDirected,
}

impl OpKind {
    const ALL: [OpKind; 8] = [
        OpKind::Union,
        OpKind::Intersection,
        OpKind::Difference,
        OpKind::EdgeDifference,
        OpKind::SymmetricDifference,
        OpKind::Complement,
        OpKind::Induced,
        OpKind::ConvertDirected,
    ];

    fn label(self) -> &'static str {
        match self {
            OpKind::Union => "Vereinigung  A u B",
            OpKind::Intersection => "Durchschnitt  A n B",
            OpKind::Difference => "Differenz  A \\ B  (Knoten + Kanten)",
            OpKind::EdgeDifference => "Kantendifferenz  E(A) \\ E(B)",
            OpKind::SymmetricDifference => "Symmetrische Differenz  A xor B",
            OpKind::Complement => "Komplement von A",
            OpKind::Induced => "Induzierter Teilgraph von A",
            OpKind::ConvertDirected => "A umwandeln (gerichtet <-> ungerichtet)",
        }
    }

    fn is_binary(self) -> bool {
        matches!(
            self,
            OpKind::Union
                | OpKind::Intersection
                | OpKind::Difference
                | OpKind::EdgeDifference
                | OpKind::SymmetricDifference
        )
    }
}

pub struct GraphPlotApp {
    tabs: Vec<GraphTab>,
    selected: usize,
    next_id: u64,

    op_a: Option<u64>,
    op_b: Option<u64>,
    op_kind: OpKind,
    op_result_directed: bool,
    op_induced_selection: BTreeSet<VertexId>,
    op_error: Option<String>,

    named_sets: BTreeMap<String, NamedSetEntry>,
    expr_input: String,
    expr_result_directed: bool,
    expr_log: Vec<(bool, String)>,

    mis_source: Option<u64>,
    mis_name: String,

    lua_library: String,
    lua_filters: BTreeMap<String, String>,
    lua_editor_name: String,
    lua_editor_source: String,
    lua_apply_source: Option<u64>,
    lua_apply_filter: Option<String>,
    lua_result_name: String,
    lua_open_tab: bool,
    lua_log: Vec<(bool, String)>,
}

impl Default for GraphPlotApp {
    fn default() -> Self {
        let first = GraphTab::new(0, "G1", false);
        Self {
            tabs: vec![first],
            selected: 0,
            next_id: 1,
            op_a: Some(0),
            op_b: None,
            op_kind: OpKind::Union,
            op_result_directed: false,
            op_induced_selection: BTreeSet::new(),
            op_error: None,

            named_sets: BTreeMap::new(),
            expr_input: String::from(
                "# Beispiel: Knoten ohne Kante in G1\nA = { v : v in V(G1), forall u in V(G1) . not ((v,u) in E(G1) or (u,v) in E(G1)) }",
            ),
            expr_result_directed: false,
            expr_log: Vec::new(),

            mis_source: Some(0),
            mis_name: String::from("IS"),

            lua_library: String::from(
                "-- Wiederverwendbare Hilfsfunktionen für alle Filter-Skripte.\n\
                 function is_isolated(g, v)\n    \
                     return g:degree(v) == 0\n\
                 end",
            ),
            lua_filters: BTreeMap::new(),
            lua_editor_name: String::from("MeinFilter"),
            lua_editor_source: String::from(
                "-- g:vertices()  g:edges() (Liste {von, bis})  g:has_edge(a,b)  g:degree(v)\n\
                 -- g:neighbors(v)  g:directed()  g:add_vertex(v)  g:add_edge(a,b)\n\
                 -- g:remove_vertex(v)  g:remove_edge(a,b)\n\
                 -- new_graph(gerichtet)  complement(g)  induced_subgraph(g, {liste})\n\
                 -- greedy_independent_set(g)\n\
                 function filter(g)\n    \
                     local out = new_graph(g:directed())\n    \
                     for _, v in ipairs(g:vertices()) do\n        \
                         if is_isolated(g, v) then\n            \
                             out:add_vertex(v)\n        \
                         end\n    \
                     end\n    \
                     return out\n\
                 end",
            ),
            lua_apply_source: Some(0),
            lua_apply_filter: None,
            lua_result_name: String::new(),
            lua_open_tab: true,
            lua_log: Vec::new(),
        }
    }
}

impl GraphPlotApp {
    fn add_tab(&mut self, tab: GraphTab) {
        self.selected = self.tabs.len();
        self.tabs.push(tab);
    }

    fn tab_by_id(&self, id: u64) -> Option<&GraphTab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    fn compute_operation(&mut self) {
        self.op_error = None;
        let a_id = match self.op_a {
            Some(id) => id,
            None => {
                self.op_error = Some("Bitte Graph A auswählen.".into());
                return;
            }
        };
        let a = match self.tab_by_id(a_id) {
            Some(t) => t.graph.clone(),
            None => {
                self.op_error = Some("Graph A existiert nicht mehr.".into());
                return;
            }
        };

        let result = if self.op_kind.is_binary() {
            let b_id = match self.op_b {
                Some(id) => id,
                None => {
                    self.op_error = Some("Bitte Graph B auswählen.".into());
                    return;
                }
            };
            let b = match self.tab_by_id(b_id) {
                Some(t) => t.graph.clone(),
                None => {
                    self.op_error = Some("Graph B existiert nicht mehr.".into());
                    return;
                }
            };
            let d = self.op_result_directed;
            match self.op_kind {
                OpKind::Union => Graph::union(&a, &b, d),
                OpKind::Intersection => Graph::intersection(&a, &b, d),
                OpKind::Difference => Graph::difference(&a, &b, d),
                OpKind::EdgeDifference => Graph::edge_difference(&a, &b, d),
                OpKind::SymmetricDifference => Graph::symmetric_difference(&a, &b, d),
                _ => unreachable!(),
            }
        } else {
            match self.op_kind {
                OpKind::Complement => Graph::complement(&a),
                OpKind::ConvertDirected => Graph::with_directed(&a, self.op_result_directed),
                OpKind::Induced => {
                    if self.op_induced_selection.is_empty() {
                        self.op_error = Some(
                            "Bitte mindestens einen Knoten für den Teilgraphen auswählen.".into(),
                        );
                        return;
                    }
                    Graph::induced_subgraph(&a, &self.op_induced_selection)
                }
                _ => unreachable!(),
            }
        };

        let a_name = self
            .tab_by_id(a_id)
            .map(|t| t.name.clone())
            .unwrap_or_default();
        let name = match self.op_kind {
            OpKind::Complement => format!("not {a_name}"),
            OpKind::ConvertDirected => format!("{a_name}'"),
            OpKind::Induced => format!("{a_name}[S]"),
            _ => {
                let b_name = self
                    .op_b
                    .and_then(|id| self.tab_by_id(id))
                    .map(|t| t.name.clone())
                    .unwrap_or_default();
                format!("{a_name} {} {b_name}", op_symbol(self.op_kind))
            }
        };

        let id = self.next_id;
        self.next_id += 1;
        self.add_tab(GraphTab::from_graph(id, name, result));
    }

    fn tabs_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let mut to_close: Option<usize> = None;
            for (i, tab) in self.tabs.iter().enumerate() {
                let selected = i == self.selected;
                ui.horizontal(|ui| {
                    if ui.selectable_label(selected, &tab.name).clicked() {
                        self.selected = i;
                    }
                    if self.tabs.len() > 1 && ui.small_button("x").clicked() {
                        to_close = Some(i);
                    }
                });
            }
            if ui.button("+ Neuer Graph").clicked() {
                let id = self.next_id;
                self.next_id += 1;
                let name = format!("G{}", id + 1);
                self.add_tab(GraphTab::new(id, name, false));
            }
            if let Some(i) = to_close {
                self.tabs.remove(i);
                if self.selected >= self.tabs.len() {
                    self.selected = self.tabs.len().saturating_sub(1);
                }
            }
        });
    }

    fn operations_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mengenoperationen -> G'");
        ui.label("Erzeuge einen neuen Graphen G' = (V', E') aus bestehenden Graphen.");
        ui.add_space(8.0);

        egui::ComboBox::from_label("Graph A")
            .selected_text(
                self.op_a
                    .and_then(|id| self.tab_by_id(id))
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| "—".to_string()),
            )
            .show_ui(ui, |ui| {
                for tab in &self.tabs {
                    ui.selectable_value(&mut self.op_a, Some(tab.id), &tab.name);
                }
            });

        egui::ComboBox::from_label("Operation")
            .selected_text(self.op_kind.label())
            .show_ui(ui, |ui| {
                for kind in OpKind::ALL {
                    ui.selectable_value(&mut self.op_kind, kind, kind.label());
                }
            });

        if self.op_kind.is_binary() {
            egui::ComboBox::from_label("Graph B")
                .selected_text(
                    self.op_b
                        .and_then(|id| self.tab_by_id(id))
                        .map(|t| t.name.clone())
                        .unwrap_or_else(|| "—".to_string()),
                )
                .show_ui(ui, |ui| {
                    for tab in &self.tabs {
                        ui.selectable_value(&mut self.op_b, Some(tab.id), &tab.name);
                    }
                });
        }

        if self.op_kind.is_binary() || self.op_kind == OpKind::ConvertDirected {
            ui.checkbox(&mut self.op_result_directed, "Ergebnis gerichtet");
            ui.small(
                "Graphen mit abweichender Ausrichtung werden vor der Operation \
                 automatisch in diesen Modus überführt.",
            );
        }

        if self.op_kind == OpKind::Induced {
            ui.label("Knoten für den induzierten Teilgraphen:");
            let vertices: Option<Vec<VertexId>> = self
                .op_a
                .and_then(|id| self.tab_by_id(id))
                .map(|t| t.graph.vertices.iter().cloned().collect());
            if let Some(vertices) = vertices {
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for v in &vertices {
                            let mut checked = self.op_induced_selection.contains(v);
                            if ui.checkbox(&mut checked, v).changed() {
                                if checked {
                                    self.op_induced_selection.insert(v.clone());
                                } else {
                                    self.op_induced_selection.remove(v);
                                }
                            }
                        }
                    });
            }
        }

        ui.add_space(8.0);
        if ui.button("Berechnen -> neuer Tab").clicked() {
            self.compute_operation();
        }
        if let Some(err) = &self.op_error {
            ui.colored_label(Color32::from_rgb(220, 80, 80), err);
        }
    }

    fn run_expr_program(&mut self) {
        let input = self.expr_input.clone();
        let mut named_values: BTreeMap<String, SetValue> = self
            .named_sets
            .iter()
            .map(|(k, e)| (k.clone(), e.value.clone()))
            .collect();
        for raw_line in input.split(&['\n', ';'][..]) {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let stmt = match expr::parse_statement(line) {
                Ok(s) => s,
                Err(err) => {
                    self.push_log(false, format!("{line}\n  Fehler: {err}"));
                    break;
                }
            };

            let outcome = {
                let graphs: BTreeMap<&str, &Graph> = self
                    .tabs
                    .iter()
                    .map(|t| (t.name.as_str(), &t.graph))
                    .collect();
                let ctx = EvalCtx {
                    graphs,
                    named: &named_values,
                };
                expr::eval_statement(&stmt, &ctx)
            };

            match outcome {
                Ok(StmtResult::Set(name, value)) => {
                    let mut msg = format!(
                        "{name} = {} ({} Elemente): {}",
                        value.kind(),
                        value.len(),
                        truncate(&value.format(), 160)
                    );
                    if let expr::Stmt::DefineSet(
                        _,
                        expr::SetExpr::Comprehension {
                            output: expr::Output::Terms(names),
                            ..
                        },
                    ) = &stmt
                    {
                        if names.len() > 1 {
                            msg.push_str(&format!(
                                "\n  Hinweis: '{}' (ohne Klammern) vereinigt die Werte \
                                 dieser Variablen in EINE Menge, bildet aber keine Paare/Kanten. \
                                 Für Kanten '(x,y)' in Klammern schreiben; für \"nur Knoten ohne \
                                 Verbindung zu irgendeinem anderen\" eine Variable + forall nutzen.",
                                names.join(", ")
                            ));
                        }
                    }
                    named_values.insert(name.clone(), value.clone());
                    self.insert_named_set(name, value);
                    self.push_log(true, msg);
                }
                Ok(StmtResult::Graph(name, verts, edges)) => {
                    let mut g = Graph::new(self.expr_result_directed);
                    for v in &verts {
                        g.add_vertex(v.clone());
                    }
                    for (a, b) in &edges {
                        g.add_edge(a, b);
                    }
                    let msg = format!(
                        "Graph \"{name}\" erzeugt: |V| = {}, |E| = {}",
                        g.vertices.len(),
                        g.edges.len()
                    );
                    let id = self.next_id;
                    self.next_id += 1;
                    self.add_tab(GraphTab::from_graph(id, name, g));
                    self.push_log(true, msg);
                }
                Err(err) => {
                    self.push_log(false, format!("{line}\n  Fehler: {err}"));
                    break;
                }
            }
        }
    }

    fn push_log(&mut self, ok: bool, msg: String) {
        self.expr_log.push((ok, msg));
        if self.expr_log.len() > 50 {
            self.expr_log.remove(0);
        }
    }

    /// Inserts/overwrites a named set, keeping its existing highlight color
    /// if it already had one, or assigning the next palette color if new.
    fn insert_named_set(&mut self, name: String, value: SetValue) {
        let color = self
            .named_sets
            .get(&name)
            .map(|e| e.color)
            .unwrap_or_else(|| PALETTE[self.named_sets.len() % PALETTE.len()]);
        self.named_sets.insert(name, NamedSetEntry { value, color });
    }

    fn compute_greedy_independent_set(&mut self) {
        let Some(id) = self.mis_source else {
            self.push_log(false, "Bitte einen Graphen auswählen.".to_string());
            return;
        };
        let Some(tab) = self.tab_by_id(id) else {
            self.push_log(false, "Graph existiert nicht mehr.".to_string());
            return;
        };
        let name = self.mis_name.trim().to_string();
        if name.is_empty() {
            self.push_log(
                false,
                "Bitte einen Namen für die Menge angeben.".to_string(),
            );
            return;
        }
        let set = tab.graph.greedy_independent_set();
        let source_name = tab.name.clone();
        let value = SetValue::Vertices(set);
        let msg = format!(
            "{name} = unabhängige Menge (Greedy) von \"{source_name}\": {} ({} Elemente): {}",
            value.kind(),
            value.len(),
            truncate(&value.format(), 160)
        );
        self.insert_named_set(name, value);
        self.push_log(true, msg);
    }

    fn push_lua_log(&mut self, ok: bool, msg: String) {
        self.lua_log.push((ok, msg));
        if self.lua_log.len() > 50 {
            self.lua_log.remove(0);
        }
    }

    fn save_lua_filter(&mut self) {
        let name = self.lua_editor_name.trim().to_string();
        if name.is_empty() {
            self.push_lua_log(
                false,
                "Bitte einen Namen für den Filter angeben.".to_string(),
            );
            return;
        }
        self.lua_filters
            .insert(name.clone(), self.lua_editor_source.clone());
        self.lua_apply_filter = Some(name.clone());
        self.push_lua_log(true, format!("Filter \"{name}\" gespeichert."));
    }

    fn apply_lua_filter(&mut self) {
        let Some(tab_id) = self.lua_apply_source else {
            self.push_lua_log(false, "Bitte einen Graphen auswählen.".to_string());
            return;
        };
        let Some(filter_name) = self.lua_apply_filter.clone() else {
            self.push_lua_log(false, "Bitte einen Filter auswählen.".to_string());
            return;
        };
        let Some(source) = self.lua_filters.get(&filter_name).cloned() else {
            self.push_lua_log(false, "Filter existiert nicht mehr.".to_string());
            return;
        };
        let Some(tab) = self.tab_by_id(tab_id) else {
            self.push_lua_log(false, "Graph existiert nicht mehr.".to_string());
            return;
        };
        let input = tab.graph.clone();
        let source_graph_name = tab.name.clone();

        match crate::scripting::run_filter(&source, &self.lua_library, &input) {
            Ok(g) => {
                let name = if self.lua_result_name.trim().is_empty() {
                    format!("{filter_name}({source_graph_name})")
                } else {
                    self.lua_result_name.trim().to_string()
                };

                let vertices_name = format!("{name} (Knoten)");
                let edges_name = format!("{name} (Kanten)");
                self.insert_named_set(
                    vertices_name.clone(),
                    SetValue::Vertices(g.vertices.clone()),
                );
                self.insert_named_set(edges_name.clone(), SetValue::Edges(g.edges.clone()));

                let mut msg = format!(
                    "Filter \"{filter_name}\" auf \"{source_graph_name}\" angewendet: |V| = {}, |E| = {}. \
                     Als Mengen gespeichert: \"{vertices_name}\", \"{edges_name}\" (im Graphen markierbar).",
                    g.vertices.len(),
                    g.edges.len()
                );

                if self.lua_open_tab {
                    let id = self.next_id;
                    self.next_id += 1;
                    self.add_tab(GraphTab::from_graph(id, name.clone(), g));
                    msg.push_str(&format!(" Neuer Tab \"{name}\" geöffnet."));
                }

                self.push_lua_log(true, msg);
            }
            Err(err) => {
                self.push_lua_log(false, format!("Filter \"{filter_name}\":\n  {err}"));
            }
        }
    }

    fn lua_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Lua-Filter");
        ui.label(
            "Schreibe Filter-Skripte in Lua, die aus einem Graphen einen neuen Graphen bauen. \
             Eine gemeinsame Bibliothek mit eigenen Hilfsfunktionen steht allen Filtern zur \
             Verfügung.",
        );

        egui::CollapsingHeader::new("API-Referenz").show(ui, |ui| {
            ui.label(
                "g:vertices()   g:edges()  (Liste aus {von, bis})\n\
                 g:has_edge(a,b)   g:degree(v)   g:neighbors(v)   g:directed()\n\
                 g:add_vertex(v)   g:add_edge(a,b)\n\
                 g:remove_vertex(v)   g:remove_edge(a,b)\n\n\
                 new_graph(gerichtet)                       leerer neuer Graph\n\
                 complement(g)\n\
                 induced_subgraph(g, {liste})\n\
                 greedy_independent_set(g)                  liefert eine Liste von Knoten\n\n\
                 Ein Skript muss eine Funktion definieren:\n\
                 function filter(g) ... return g2 end\n\n\
                 Lua läuft mit der abgesicherten Standardbibliothek (kein Datei-/OS-Zugriff).",
            );
        });

        egui::CollapsingHeader::new("Bibliothek (eigene Hilfsfunktionen)").show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.lua_library)
                    .desired_rows(4)
                    .font(egui::TextStyle::Monospace),
            );
        });

        ui.label("Filter-Editor:");
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut self.lua_editor_name);
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.lua_editor_source)
                .desired_rows(10)
                .font(egui::TextStyle::Monospace),
        );
        if ui.button("Als Filter speichern").clicked() {
            self.save_lua_filter();
        }

        if !self.lua_filters.is_empty() {
            ui.add_space(6.0);
            ui.label("Gespeicherte Filter:");
            let names: Vec<String> = self.lua_filters.keys().cloned().collect();
            let mut to_remove: Option<String> = None;
            for name in &names {
                ui.horizontal(|ui| {
                    ui.label(name);
                    if ui.small_button("Bearbeiten").clicked() {
                        self.lua_editor_name = name.clone();
                        self.lua_editor_source = self.lua_filters[name].clone();
                    }
                    if ui.small_button("x").clicked() {
                        to_remove = Some(name.clone());
                    }
                });
            }
            if let Some(name) = to_remove {
                self.lua_filters.remove(&name);
                if self.lua_apply_filter.as_deref() == Some(name.as_str()) {
                    self.lua_apply_filter = None;
                }
            }

            ui.add_space(6.0);
            ui.label("Filter anwenden:");
            egui::ComboBox::from_label("Lua: Graph")
                .selected_text(
                    self.lua_apply_source
                        .and_then(|id| self.tab_by_id(id))
                        .map(|t| t.name.clone())
                        .unwrap_or_else(|| "—".to_string()),
                )
                .show_ui(ui, |ui| {
                    for tab in &self.tabs {
                        ui.selectable_value(&mut self.lua_apply_source, Some(tab.id), &tab.name);
                    }
                });
            egui::ComboBox::from_label("Lua: Filter")
                .selected_text(
                    self.lua_apply_filter
                        .clone()
                        .unwrap_or_else(|| "—".to_string()),
                )
                .show_ui(ui, |ui| {
                    for name in &names {
                        ui.selectable_value(&mut self.lua_apply_filter, Some(name.clone()), name);
                    }
                });
            ui.horizontal(|ui| {
                ui.label("Ergebnisname:");
                ui.text_edit_singleline(&mut self.lua_result_name);
            });
            ui.checkbox(&mut self.lua_open_tab, "Auch als neuer Tab öffnen");
            ui.label(
                "Das Ergebnis wird immer als Knoten- und Kantenmenge gespeichert und kann \
                 in jedem Graph-Tab unter \"Markierungen\" farbig hervorgehoben werden.",
            );
            if ui.button("Anwenden").clicked() {
                self.apply_lua_filter();
            }
        }

        if !self.lua_log.is_empty() {
            ui.add_space(6.0);
            ui.label("Verlauf:");
            egui::ScrollArea::vertical()
                .id_salt("lua_log_scroll")
                .max_height(160.0)
                .show(ui, |ui| {
                    for (ok, msg) in self.lua_log.iter().rev() {
                        let color = if *ok {
                            Color32::from_rgb(140, 190, 140)
                        } else {
                            Color32::from_rgb(220, 80, 80)
                        };
                        ui.colored_label(color, msg);
                    }
                });
        }
    }

    fn expr_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mengenausdrücke (Text)");
        ui.label(
            "Definiere benannte Knoten-/Kantenmengen und neue Graphen per Ausdruck. \
             Mehrere Zeilen (oder ';'-getrennt) werden der Reihe nach ausgeführt und \
             können aufeinander aufbauen.",
        );

        egui::CollapsingHeader::new("Syntax-Spickzettel").show(ui, |ui| {
            ui.label(
                "V(G) / E(G)   Knoten-/Kantenmenge von Tab \"G\"\n\
                 A | B   A & B   A - B   A ^ B    Vereinigung, Durchschnitt, Differenz, sym. Differenz\n\
                 {}  bzw. empty    leere Menge\n\
                 { v : v in S, Bedingung }              Mengenbildner über Knoten (oder Kanten, falls S eine Kantenmenge ist)\n\
                 { (u,v) : u in S1, v in S2, Bedingung }   Mengenbildner für Kanten aus zwei Knotenvariablen\n\
                 forall x in S . P     exists x in S . P     Quantoren\n\
                 x in S   x notin S   x = y   x != y   S <= T (Teilmenge)\n\
                 and / or / not  (auch: && || ! , bzw. ∧ ∨ ¬)\n\
                 Name = Ausdruck                zum Benennen einer Menge\n\
                 Name = (V-Ausdruck, E-Ausdruck)   erzeugt einen neuen Graphen G' = (V', E') als Tab\n\n\
                 Auch Unicode-Symbole werden akzeptiert: ∪ ∩ ∖ △ ∀ ∃ ∈ ∉ ⊆ ∧ ∨ ¬ ≠ ∅ \
                 (werden je nach Schriftart evtl. nicht angezeigt, funktionieren aber).",
            );
        });

        ui.add(
            egui::TextEdit::multiline(&mut self.expr_input)
                .desired_rows(4)
                .font(egui::TextStyle::Monospace),
        );
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.expr_result_directed, "Ergebnis-Graphen gerichtet");
        });
        if ui.button("Ausführen").clicked() {
            self.run_expr_program();
        }

        ui.add_space(10.0);
        egui::CollapsingHeader::new("Unabhängige Menge (Greedy)").show(ui, |ui| {
            ui.label(
                "Berechnet eine maximale unabhängige Menge (kein Knotenpaar darin ist durch \
                 eine Kante verbunden) per Greedy-Heuristik. Das ist NP-schwer exakt zu lösen; \
                 das Ergebnis ist maximal, aber nicht garantiert die größtmögliche Menge.",
            );
            egui::ComboBox::from_label("Graph")
                .selected_text(
                    self.mis_source
                        .and_then(|id| self.tab_by_id(id))
                        .map(|t| t.name.clone())
                        .unwrap_or_else(|| "—".to_string()),
                )
                .show_ui(ui, |ui| {
                    for tab in &self.tabs {
                        ui.selectable_value(&mut self.mis_source, Some(tab.id), &tab.name);
                    }
                });
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut self.mis_name);
            });
            if ui.button("Berechnen").clicked() {
                self.compute_greedy_independent_set();
            }
        });

        if !self.named_sets.is_empty() {
            ui.add_space(6.0);
            ui.label("Definierte Mengen (Farbe = Markierungsfarbe im Graphen):");
            let names: Vec<String> = self.named_sets.keys().cloned().collect();
            let mut to_remove: Option<String> = None;
            let mut to_open: Option<Graph> = None;
            let directed = self.expr_result_directed;
            egui::ScrollArea::vertical()
                .id_salt("named_sets_scroll")
                .max_height(160.0)
                .show(ui, |ui| {
                    for name in &names {
                        let Some(entry) = self.named_sets.get_mut(name) else {
                            continue;
                        };
                        ui.horizontal(|ui| {
                            ui.color_edit_button_srgba(&mut entry.color);
                            ui.label(format!(
                                "{name}: {} ({})",
                                entry.value.kind(),
                                entry.value.len()
                            ));
                            if ui.small_button("-> Tab").clicked() {
                                to_open = Some(set_value_to_graph(&entry.value, directed));
                            }
                            if ui.small_button("x").clicked() {
                                to_remove = Some(name.clone());
                            }
                        });
                    }
                });
            if let Some(name) = to_remove {
                self.named_sets.remove(&name);
            }
            if let Some(g) = to_open {
                let id = self.next_id;
                self.next_id += 1;
                let name = format!("Menge{}", id + 1);
                self.add_tab(GraphTab::from_graph(id, name, g));
            }
        }

        if !self.expr_log.is_empty() {
            ui.add_space(6.0);
            ui.label("Verlauf:");
            egui::ScrollArea::vertical()
                .id_salt("expr_log_scroll")
                .max_height(160.0)
                .show(ui, |ui| {
                    for (ok, msg) in self.expr_log.iter().rev() {
                        let color = if *ok {
                            Color32::from_rgb(140, 190, 140)
                        } else {
                            Color32::from_rgb(220, 80, 80)
                        };
                        ui.colored_label(color, msg);
                    }
                });
        }
    }
}

fn set_value_to_graph(value: &SetValue, directed: bool) -> Graph {
    let mut g = Graph::new(directed);
    match value {
        SetValue::Vertices(vs) => {
            for v in vs {
                g.add_vertex(v.clone());
            }
        }
        SetValue::Edges(es) => {
            for (a, b) in es {
                g.add_edge(a, b);
            }
        }
    }
    g
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let head: String = s.chars().take(max_chars).collect();
        format!("{head}...")
    }
}

fn op_symbol(kind: OpKind) -> &'static str {
    match kind {
        OpKind::Union => "u",
        OpKind::Intersection => "n",
        OpKind::Difference => "\\",
        OpKind::EdgeDifference => "\\E",
        OpKind::SymmetricDifference => "xor",
        _ => "",
    }
}

impl eframe::App for GraphPlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("tabs_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            self.tabs_bar(ui);
            ui.add_space(4.0);
        });

        egui::SidePanel::right("operations_panel")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.operations_panel(ui);
                    ui.separator();
                    self.expr_panel(ui);
                    ui.separator();
                    self.lua_panel(ui);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let named_sets = &self.named_sets;
            let mut still_settling = false;
            if let Some(tab) = self.tabs.get_mut(self.selected) {
                still_settling = draw_graph_tab(ui, tab, named_sets);
            }
            // Only keep repainting on a timer while the layout is actually
            // moving; once it settles, egui idles normally (still repaints
            // instantly on real input) instead of redrawing at full frame
            // rate forever, which is also what let tiny leftover forces
            // show up as a visible, endless jitter.
            if still_settling {
                ctx.request_repaint();
            }
        });
    }
}

fn draw_graph_tab(
    ui: &mut egui::Ui,
    tab: &mut GraphTab,
    named_sets: &BTreeMap<String, NamedSetEntry>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut tab.name);
        ui.separator();
        for mode in [
            EditMode::AddVertex,
            EditMode::AddEdge,
            EditMode::Move,
            EditMode::Delete,
        ] {
            if ui
                .selectable_label(tab.mode == mode, mode.label())
                .clicked()
            {
                tab.mode = mode;
                tab.pending_edge_from = None;
            }
        }
        ui.separator();
        let mut directed = tab.graph.directed;
        if ui.checkbox(&mut directed, "gerichtet").changed() {
            tab.graph = Graph::with_directed(&tab.graph, directed);
        }
        if ui.button("Graph leeren").clicked() {
            let directed = tab.graph.directed;
            tab.graph = Graph::new(directed);
            tab.layout = Layout::default();
            tab.pending_edge_from = None;
        }
    });

    if named_sets.is_empty() {
        ui.label("Markierungen: keine Mengen definiert.");
    } else {
        egui::CollapsingHeader::new("Markierungen")
            .default_open(true)
            .show(ui, |ui| {
                for (name, entry) in named_sets {
                    let mut checked = tab.highlighted.contains(name);
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut checked, name).changed() {
                            if checked {
                                tab.highlighted.insert(name.clone());
                            } else {
                                tab.highlighted.remove(name);
                            }
                        }
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, entry.color);
                        ui.label(format!("{} ({})", entry.value.kind(), entry.value.len()));
                    });
                }
            });
    }

    egui::CollapsingHeader::new("Zufallsgraph erzeugen").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Knoten (n):");
            ui.add(egui::Slider::new(&mut tab.random_n, 0..=60));
        });
        ui.horizontal(|ui| {
            ui.label("Kantenwahrscheinlichkeit (p):");
            ui.add(egui::Slider::new(&mut tab.random_p, 0.0..=1.0));
        });
        ui.horizontal(|ui| {
            ui.label("Seed:");
            ui.add(egui::DragValue::new(&mut tab.random_seed));
            if ui.button("Neuer Seed").clicked() {
                tab.random_seed = rand::random();
            }
        });
        if ui.button("Zufallsgraph erzeugen").clicked() {
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(tab.random_seed);
            tab.graph = Graph::random(
                tab.graph.directed,
                tab.random_n,
                tab.random_p as f64,
                &mut rng,
            );
            tab.layout.reset_circle(&tab.graph);
            tab.pending_edge_from = None;
        }
    });

    egui::CollapsingHeader::new("Knoten umbenennen").show(ui, |ui| {
        egui::ComboBox::from_label("Knoten")
            .selected_text(tab.rename_target.clone().unwrap_or_else(|| "—".to_string()))
            .show_ui(ui, |ui| {
                for v in &tab.graph.vertices {
                    ui.selectable_value(&mut tab.rename_target, Some(v.clone()), v);
                }
            });
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut tab.rename_buffer);
            let can_rename = tab
                .rename_target
                .as_ref()
                .map(|t| {
                    !tab.rename_buffer.is_empty()
                        && (tab.rename_buffer == *t
                            || !tab.graph.vertices.contains(&tab.rename_buffer))
                })
                .unwrap_or(false);
            if ui
                .add_enabled(can_rename, egui::Button::new("Umbenennen"))
                .clicked()
            {
                if let Some(old) = tab.rename_target.clone() {
                    rename_vertex(&mut tab.graph, &mut tab.layout, &old, &tab.rename_buffer);
                    tab.rename_target = Some(tab.rename_buffer.clone());
                }
            }
        });
    });

    if let Some(status) = &tab.status {
        ui.colored_label(Color32::from_rgb(150, 150, 150), status);
    }
    ui.label(format!(
        "|V| = {}    |E| = {}    Modus: {}",
        tab.graph.vertices.len(),
        tab.graph.edges.len(),
        if tab.graph.directed {
            "gerichtet"
        } else {
            "ungerichtet"
        }
    ));

    ui.separator();

    let available = ui.available_size_before_wrap();
    let size = Vec2::new(available.x.max(50.0), available.y.max(50.0));
    let (response, painter) = ui.allocate_painter(size, Sense::click_and_drag());
    let canvas_rect = response.rect;
    let center = canvas_rect.center();

    painter.rect_filled(canvas_rect, 4.0, ui.visuals().extreme_bg_color);

    tab.layout.sync(&tab.graph);
    let bounds = Vec2::new(
        (canvas_rect.width() / 2.0 - 20.0).max(10.0),
        (canvas_rect.height() / 2.0 - 20.0).max(10.0),
    );
    let still_settling = tab.layout.step(&tab.graph, bounds);

    let to_screen = |p: Pos2| center + p.to_vec2();
    let to_world = |p: Pos2| (p - center).to_pos2();

    let node_radius = 14.0;

    // Active highlights for this tab, in a fixed order so that a vertex/edge
    // belonging to more than one highlighted set consistently shows the
    // first matching set's color.
    let active_highlights: Vec<&NamedSetEntry> = tab
        .highlighted
        .iter()
        .filter_map(|name| named_sets.get(name))
        .collect();
    let vertex_highlight = |v: &VertexId| -> Option<Color32> {
        active_highlights
            .iter()
            .find_map(|entry| match &entry.value {
                SetValue::Vertices(set) if set.contains(v) => Some(entry.color),
                _ => None,
            })
    };
    let edge_highlight = |a: &str, b: &str| -> Option<Color32> {
        active_highlights
            .iter()
            .find_map(|entry| match &entry.value {
                SetValue::Edges(set) => {
                    let hit = set.contains(&(a.to_string(), b.to_string()))
                        || (!tab.graph.directed && set.contains(&(b.to_string(), a.to_string())));
                    hit.then_some(entry.color)
                }
                _ => None,
            })
    };

    // Draw edges first (under nodes).
    for (a, b) in &tab.graph.edges {
        let (Some(&pa), Some(&pb)) = (tab.layout.pos.get(a), tab.layout.pos.get(b)) else {
            continue;
        };
        let sa = to_screen(pa);
        let sb = to_screen(pb);
        let highlight = edge_highlight(a, b);
        let stroke = match highlight {
            Some(color) => Stroke::new(3.0, color),
            None => Stroke::new(1.6, ui.visuals().text_color().gamma_multiply(0.6)),
        };
        painter.line_segment([sa, sb], stroke);
        if tab.graph.directed {
            let arrow_color = highlight.unwrap_or_else(|| ui.visuals().text_color());
            draw_arrowhead(&painter, sa, sb, node_radius, arrow_color);
        }
    }

    // Pending edge preview.
    if let (EditMode::AddEdge, Some(from)) = (tab.mode, &tab.pending_edge_from) {
        if let Some(&p) = tab.layout.pos.get(from) {
            if let Some(pointer) = response.hover_pos() {
                painter.line_segment(
                    [to_screen(p), pointer],
                    Stroke::new(1.0, Color32::from_rgb(120, 170, 250)),
                );
            }
        }
    }

    // Draw nodes and collect interaction.
    let mut clicked_vertex: Option<VertexId> = None;
    let mut dragging_any = false;
    let ids: Vec<VertexId> = tab.graph.vertices.iter().cloned().collect();
    for v in &ids {
        let Some(&world_pos) = tab.layout.pos.get(v) else {
            continue;
        };
        let screen_pos = to_screen(world_pos);
        let rect = Rect::from_center_size(screen_pos, Vec2::splat(node_radius * 2.0));
        let node_id = response.id.with(("node", v));
        let node_resp = ui.interact(rect, node_id, Sense::click_and_drag());

        if node_resp.drag_started() {
            tab.layout.dragging = Some(v.clone());
        }
        if node_resp.dragged() {
            dragging_any = true;
            if let Some(p) = tab.layout.pos.get_mut(v) {
                *p += node_resp.drag_delta();
            }
        }
        if node_resp.drag_stopped() && tab.layout.dragging.as_deref() == Some(v.as_str()) {
            tab.layout.dragging = None;
        }
        if node_resp.clicked() {
            clicked_vertex = Some(v.clone());
        }

        let is_pending_from = tab.pending_edge_from.as_deref() == Some(v.as_str());
        let fill = if is_pending_from {
            Color32::from_rgb(120, 170, 250)
        } else if let Some(color) = vertex_highlight(v) {
            color
        } else if node_resp.hovered() {
            ui.visuals().widgets.hovered.bg_fill
        } else {
            ui.visuals().widgets.inactive.bg_fill
        };
        painter.circle(
            screen_pos,
            node_radius,
            fill,
            Stroke::new(1.5, ui.visuals().text_color()),
        );
        painter.text(
            screen_pos,
            egui::Align2::CENTER_CENTER,
            v,
            egui::FontId::proportional(11.0),
            ui.visuals().strong_text_color(),
        );
    }
    if !dragging_any && tab.layout.dragging.is_some() {
        tab.layout.dragging = None;
    }
    let still_settling = still_settling || dragging_any;

    tab.status = None;

    match tab.mode {
        EditMode::AddVertex => {
            if response.clicked() && clicked_vertex.is_none() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let label = tab.graph.fresh_label();
                    let world = to_world(pointer);
                    tab.graph.add_vertex(label.clone());
                    tab.layout.pos.insert(label, world);
                }
            }
        }
        EditMode::AddEdge => {
            if let Some(v) = clicked_vertex {
                match tab.pending_edge_from.take() {
                    Some(from) if from != v => {
                        tab.graph.add_edge(&from, &v);
                    }
                    Some(_) => {} // clicked same node again: cancel selection
                    None => tab.pending_edge_from = Some(v),
                }
            } else if response.clicked() {
                tab.pending_edge_from = None;
            }
        }
        EditMode::Move => {}
        EditMode::Delete => {
            if let Some(v) = clicked_vertex {
                tab.graph.remove_vertex(&v);
            } else if response.clicked() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let world = to_world(pointer);
                    if let Some(edge) = closest_edge(&tab.graph, &tab.layout, world, 8.0) {
                        tab.graph.remove_edge(&edge.0, &edge.1);
                    }
                }
            }
        }
    }

    still_settling
}

fn rename_vertex(graph: &mut Graph, layout: &mut Layout, old: &str, new: &str) {
    if old == new || new.is_empty() || graph.vertices.contains(new) {
        return;
    }
    let mut g = Graph::new(graph.directed);
    for v in &graph.vertices {
        g.add_vertex(if v == old { new.to_string() } else { v.clone() });
    }
    for (a, b) in &graph.edges {
        let a2 = if a == old { new } else { a.as_str() };
        let b2 = if b == old { new } else { b.as_str() };
        g.add_edge(a2, b2);
    }
    if let Some(p) = layout.pos.remove(old) {
        layout.pos.insert(new.to_string(), p);
    }
    *graph = g;
}

fn closest_edge(
    graph: &Graph,
    layout: &Layout,
    world: Pos2,
    max_dist: f32,
) -> Option<(VertexId, VertexId)> {
    let mut best: Option<(VertexId, VertexId, f32)> = None;
    for (a, b) in &graph.edges {
        let (Some(&pa), Some(&pb)) = (layout.pos.get(a), layout.pos.get(b)) else {
            continue;
        };
        let d = point_segment_distance(world, pa, pb);
        if best.as_ref().map(|(_, _, bd)| d < *bd).unwrap_or(true) {
            best = Some((a.clone(), b.clone(), d));
        }
    }
    best.filter(|(_, _, d)| *d <= max_dist)
        .map(|(a, b, _)| (a, b))
}

fn point_segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 1e-6 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    let proj = a + ab * t;
    (p - proj).length()
}

fn draw_arrowhead(painter: &egui::Painter, from: Pos2, to: Pos2, node_radius: f32, color: Color32) {
    let dir = (to - from).normalized();
    if !dir.x.is_finite() || !dir.y.is_finite() {
        return;
    }
    let tip = to - dir * (node_radius + 1.0);
    let back = tip - dir * 9.0;
    let normal = Vec2::new(-dir.y, dir.x);
    let left = back + normal * 4.0;
    let right = back - normal * 4.0;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, left, right],
        color,
        Stroke::NONE,
    ));
}
