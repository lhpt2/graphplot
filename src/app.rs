use std::collections::BTreeSet;

use eframe::egui;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use rand::SeedableRng;

use crate::graph::{Graph, VertexId};
use crate::layout::Layout;

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
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(tab) = self.tabs.get_mut(self.selected) {
                draw_graph_tab(ui, tab);
            }
        });

        // Keep animating the layout of the active tab.
        ctx.request_repaint();
    }
}

fn draw_graph_tab(ui: &mut egui::Ui, tab: &mut GraphTab) {
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
    tab.layout.step(&tab.graph, bounds);

    let to_screen = |p: Pos2| center + p.to_vec2();
    let to_world = |p: Pos2| (p - center).to_pos2();

    let node_radius = 14.0;

    // Draw edges first (under nodes).
    for (a, b) in &tab.graph.edges {
        let (Some(&pa), Some(&pb)) = (tab.layout.pos.get(a), tab.layout.pos.get(b)) else {
            continue;
        };
        let sa = to_screen(pa);
        let sb = to_screen(pb);
        painter.line_segment(
            [sa, sb],
            Stroke::new(1.6, ui.visuals().text_color().gamma_multiply(0.6)),
        );
        if tab.graph.directed {
            draw_arrowhead(&painter, sa, sb, node_radius, ui.visuals().text_color());
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
