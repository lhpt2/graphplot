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
    vel: HashMap<VertexId, Vec2>,
    /// Vertex currently being dragged by the user, if any; it is excluded
    /// from the simulation so the pointer stays in full control of it.
    pub dragging: Option<VertexId>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            pos: HashMap::new(),
            vel: HashMap::new(),
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
        self.vel.retain(|k, _| graph.vertices.contains(k));

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
                v.clone(),
                Pos2::new(angle.cos() * radius + jitter, angle.sin() * radius + jitter),
            );
            self.vel.insert(v, Vec2::ZERO);
        }
    }

    /// Places every vertex fresh on a circle. Useful right after a full
    /// randomize/regenerate so the layout doesn't inherit stale positions.
    pub fn reset_circle(&mut self, graph: &Graph) {
        self.pos.clear();
        self.vel.clear();
        self.sync(graph);
    }

    /// Runs one relaxation step. `bounds` roughly limits how far vertices
    /// may drift so the graph stays visible in the canvas.
    pub fn step(&mut self, graph: &Graph, bounds: Vec2) {
        let ids: Vec<VertexId> = graph.vertices.iter().cloned().collect();
        let n = ids.len();
        if n == 0 {
            return;
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
            *disp.get_mut(id).unwrap() -= p.to_vec2() * 0.01;
        }

        let max_step = 8.0;
        for id in &ids {
            if self.dragging.as_deref() == Some(id.as_str()) {
                continue;
            }
            let d = disp[id];
            let len = d.length();
            let capped = if len > max_step {
                d * (max_step / len)
            } else {
                d
            };
            let vel = self.vel.entry(id.clone()).or_insert(Vec2::ZERO);
            *vel = (*vel + capped) * 0.5; // damping
            let p = self.pos.get_mut(id).unwrap();
            *p += *vel;
            p.x = p.x.clamp(-bounds.x, bounds.x);
            p.y = p.y.clamp(-bounds.y, bounds.y);
        }
    }
}
