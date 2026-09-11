use std::collections::HashMap;

use egui::{Pos2, Vec2};
use rand::Rng;

use crate::graph::{Graph, VertexId};

/// Simple Fruchterman-Reingold-style force-directed layout, relaxed a bit
/// every frame so newly added/removed vertices settle smoothly instead of
/// jumping around. Positions are stored in "world" coordinates local to the
/// canvas (roughly centered around the origin).
pub struct Layout {
    pub pos: HashMap<VertexId, Pos2>,
    /// Vertex currently being dragged by the user, if any; it is excluded
    /// from the simulation so the pointer stays in full control of it.
    pub dragging: Option<VertexId>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            pos: HashMap::new(),
            dragging: None,
        }
    }
}

impl Layout {
    /// Makes sure every vertex in `graph` has a position, placing new
    /// vertices on a circle around the existing layout (or the origin for
    /// the very first vertices). Removes stale entries for deleted vertices.
    pub fn sync(&mut self, graph: &Graph) {
        self.pos.retain(|k, _| graph.vertices.contains(k));

        let missing: Vec<VertexId> = graph
            .vertices
            .iter()
            .filter(|v| !self.pos.contains_key(*v))
            .cloned()
            .collect();
        if missing.is_empty() {
            return;
        }
        let mut rng = rand::thread_rng();
        let radius = 150.0;
        let n = missing.len();
        for (i, v) in missing.into_iter().enumerate() {
            let angle = (i as f32) / (n.max(1) as f32) * std::f32::consts::TAU;
            let jitter = rng.gen_range(-10.0..10.0);
            self.pos.insert(
                v,
                Pos2::new(angle.cos() * radius + jitter, angle.sin() * radius + jitter),
            );
        }
    }

    /// Places every vertex fresh on a circle. Useful right after a full
    /// randomize/regenerate so the layout doesn't inherit stale positions.
    pub fn reset_circle(&mut self, graph: &Graph) {
        self.pos.clear();
        self.sync(graph);
    }

    /// Runs one relaxation step. `bounds` roughly limits how far vertices
    /// may drift so the graph stays visible in the canvas. Returns `true`
    /// while the layout is still settling (so the caller knows whether it's
    /// worth scheduling another repaint) and `false` once every vertex has
    /// come to rest.
    pub fn step(&mut self, graph: &Graph, bounds: Vec2) -> bool {
        let ids: Vec<VertexId> = graph.vertices.iter().cloned().collect();
        let n = ids.len();
        if n == 0 {
            return false;
        }

        let area = (bounds.x * bounds.y).max(1.0);
        let k = (area / n as f32).sqrt() * 0.9;

        let mut disp: HashMap<VertexId, Vec2> =
            ids.iter().map(|i| (i.clone(), Vec2::ZERO)).collect();

        // Repulsion between all pairs.
        for i in 0..n {
            for j in (i + 1)..n {
                let a = &ids[i];
                let b = &ids[j];
                let pa = self.pos[a];
                let pb = self.pos[b];
                let delta = pa - pb;
                let dist = delta.length().max(0.01);
                let force = (k * k) / dist;
                let dir = delta / dist;
                *disp.get_mut(a).unwrap() += dir * force;
                *disp.get_mut(b).unwrap() -= dir * force;
            }
        }

        // Attraction along edges.
        for (a, b) in &graph.edges {
            if let (Some(&pa), Some(&pb)) = (self.pos.get(a), self.pos.get(b)) {
                let delta = pa - pb;
                let dist = delta.length().max(0.01);
                let force = (dist * dist) / k;
                let dir = delta / dist;
                *disp.get_mut(a).unwrap() -= dir * force;
                *disp.get_mut(b).unwrap() += dir * force;
            }
        }

        // Mild pull toward center to keep disconnected components from
        // drifting away forever.
        for id in &ids {
            let p = self.pos[id];
            *disp.get_mut(id).unwrap() -= p.to_vec2() * 0.03;
        }

        // Below this force a vertex is considered at rest; residual forces
        // this small are mostly two repulsion/attraction terms that don't
        // *quite* cancel, and only cause endless tiny jitter instead of
        // ever settling, so they're treated as zero. There is deliberately
        // no velocity/momentum carried between frames: each step moves a
        // vertex by a *damped, capped fraction of the current net force*
        // only, so the step size shrinks smoothly to zero as the layout
        // approaches equilibrium instead of overshooting past it and
        // oscillating forever (the classic failure mode of explicit-Euler
        // spring simulations without a cooling schedule).
        const REST_EPSILON: f32 = 0.05;
        // Generous safety cap only for extreme cases (e.g. freshly
        // randomized, near-overlapping vertices); STEP_DAMPING alone
        // governs the step size everywhere near equilibrium. A tighter cap
        // here would force the same fixed-size jump every frame even once
        // close to rest, which is exactly what caused a stable two-step
        // oscillation (jump past equilibrium, jump back, forever) instead
        // of ever truly settling.
        const MAX_STEP: f32 = 20.0;
        const STEP_DAMPING: f32 = 0.08;

        let mut moving = false;
        for id in &ids {
            if self.dragging.as_deref() == Some(id.as_str()) {
                continue;
            }
            let d = disp[id];
            let len = d.length();
            if len < 1e-6 {
                continue;
            }

            let step_len = (len * STEP_DAMPING).min(MAX_STEP);
            let movement = d * (step_len / len);

            let p = self.pos.get_mut(id).unwrap();
            let before = *p;
            let mut new_p = before + movement;
            new_p.x = new_p.x.clamp(-bounds.x, bounds.x);
            new_p.y = new_p.y.clamp(-bounds.y, bounds.y);

            // Judge "still moving" by how far the vertex would actually end
            // up moving after damping and clamping, not by the raw force
            // that was acting on it - and only commit the move at all when
            // it clears that same bar. Two different thresholds here (raw
            // force vs. actual, damped movement) is what let positions
            // creep by tiny sub-threshold amounts forever without ever
            // tripping "still moving": a vertex pinned against the canvas
            // edge (e.g. an isolated vertex repelled outward with nothing
            // to balance it) can feel a persistent, never-vanishing raw
            // force every frame that nonetheless produces a movement well
            // under the rest threshold once damped and clamped.
            if (new_p - before).length() >= REST_EPSILON {
                *p = new_p;
                moving = true;
            }
        }
        moving
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> Graph {
        let mut g = Graph::new(false);
        g.add_edge("a", "b");
        g.add_edge("b", "c");
        g.add_edge("c", "a");
        g
    }

    #[test]
    fn settles_to_rest_within_a_bounded_number_of_steps() {
        let g = triangle();
        let mut layout = Layout::default();
        layout.reset_circle(&g);
        let bounds = Vec2::new(300.0, 300.0);

        let mut settled_at = None;
        for i in 0..2000 {
            if !layout.step(&g, bounds) {
                settled_at = Some(i);
                break;
            }
        }
        assert!(
            settled_at.is_some(),
            "layout never reported settling within 2000 steps"
        );
    }

    #[test]
    fn stays_at_rest_once_settled() {
        let g = triangle();
        let mut layout = Layout::default();
        layout.reset_circle(&g);
        let bounds = Vec2::new(300.0, 300.0);

        let mut settled = false;
        for _ in 0..2000 {
            if !layout.step(&g, bounds) {
                settled = true;
                break;
            }
        }
        assert!(settled, "layout never settled within 2000 steps");
        let positions_before: HashMap<VertexId, Pos2> = layout.pos.clone();

        // A handful of further steps must not move anything (no perpetual
        // jitter) and must keep reporting "not moving".
        for _ in 0..10 {
            let moving = layout.step(&g, bounds);
            assert!(!moving, "layout resumed moving after settling");
        }
        for (v, p) in &layout.pos {
            assert_eq!(*p, positions_before[v], "vertex {v} drifted after settling");
        }
    }

    #[test]
    fn single_vertex_settles_at_the_origin() {
        let mut g = Graph::new(false);
        g.add_vertex("solo");
        let mut layout = Layout::default();
        layout.reset_circle(&g);
        let bounds = Vec2::new(300.0, 300.0);

        // A lone vertex is only pulled by the mild centering force, so it
        // must settle at the origin and then report "at rest".
        let mut settled = false;
        for _ in 0..20_000 {
            if !layout.step(&g, bounds) {
                settled = true;
                break;
            }
        }
        assert!(settled, "lone vertex never settled");
        let p = layout.pos["solo"];
        // With only the weak centering force acting on it, the vertex stops
        // once its damped step would be smaller than REST_EPSILON - which
        // happens at roughly REST_EPSILON / STEP_DAMPING / centering_coeff
        // from the origin (currently ~21px), not exactly at the origin. That
        // is the intended trade-off: settling a hair off-center beats never
        // truly coming to rest.
        assert!(
            p.distance(Pos2::ZERO) < 25.0,
            "settled far from origin: {p:?}"
        );

        // And once settled, a lone vertex already sitting at rest must stay
        // exactly there rather than drifting off due to residual jitter.
        for _ in 0..10 {
            assert!(!layout.step(&g, bounds));
        }
    }

    #[test]
    fn mixed_graph_with_isolated_vertex_settles_without_oscillating() {
        // A small connected component plus one isolated vertex - the shape
        // that originally showed visible jitter in the app.
        let mut g = Graph::new(false);
        for (a, b) in [
            ("v0", "v1"),
            ("v1", "v2"),
            ("v2", "v3"),
            ("v3", "v0"),
            ("v0", "v2"),
        ] {
            g.add_edge(a, b);
        }
        g.add_vertex("iso");

        let mut layout = Layout::default();
        layout.reset_circle(&g);
        let bounds = Vec2::new(400.0, 400.0);

        let mut settled = false;
        for _ in 0..20_000 {
            if !layout.step(&g, bounds) {
                settled = true;
                break;
            }
        }
        assert!(settled, "mixed graph never settled");

        let positions_before: HashMap<VertexId, Pos2> = layout.pos.clone();
        for _ in 0..20 {
            assert!(
                !layout.step(&g, bounds),
                "layout resumed moving after settling"
            );
        }
        for (v, p) in &layout.pos {
            assert_eq!(*p, positions_before[v], "vertex {v} drifted after settling");
        }
    }
}
