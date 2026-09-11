use std::collections::BTreeSet;

/// A vertex identity. Vertices are compared and combined by this label, so
/// set operations between two graphs are well-defined regardless of how
/// each graph's vertices were laid out on screen.
pub type VertexId = String;

/// An edge is a pair of vertex labels. For undirected graphs the pair is
/// always stored normalized (smaller label first) so that `{a, b}` and
/// `{b, a}` are the same set element.
pub type Edge = (VertexId, VertexId);

#[derive(Clone, Debug)]
pub struct Graph {
    pub directed: bool,
    pub vertices: BTreeSet<VertexId>,
    pub edges: BTreeSet<Edge>,
}

impl Graph {
    pub fn new(directed: bool) -> Self {
        Self {
            directed,
            vertices: BTreeSet::new(),
            edges: BTreeSet::new(),
        }
    }

    /// Normalizes an edge for storage: undirected edges are sorted so both
    /// insertion orders map to the same set element; directed edges keep
    /// their (from, to) order.
    fn normalize(&self, from: &str, to: &str) -> Edge {
        if self.directed || from <= to {
            (from.to_string(), to.to_string())
        } else {
            (to.to_string(), from.to_string())
        }
    }

    pub fn add_vertex(&mut self, id: impl Into<VertexId>) {
        self.vertices.insert(id.into());
    }

    pub fn remove_vertex(&mut self, id: &str) {
        self.vertices.remove(id);
        self.edges.retain(|(a, b)| a != id && b != id);
    }

    pub fn add_edge(&mut self, from: &str, to: &str) {
        if from == to {
            return; // no self loops
        }
        self.vertices.insert(from.to_string());
        self.vertices.insert(to.to_string());
        self.edges.insert(self.normalize(from, to));
    }

    pub fn remove_edge(&mut self, from: &str, to: &str) {
        let e = self.normalize(from, to);
        self.edges.remove(&e);
    }

    pub fn has_edge(&self, from: &str, to: &str) -> bool {
        self.edges.contains(&self.normalize(from, to))
    }

    /// Generates a fresh vertex label not currently used, e.g. "v0", "v1", ...
    pub fn fresh_label(&self) -> VertexId {
        let mut i = self.vertices.len();
        loop {
            let candidate = format!("v{i}");
            if !self.vertices.contains(&candidate) {
                return candidate;
            }
            i += 1;
        }
    }

    /// Returns this graph's edge set expressed as directed pairs. Undirected
    /// edges expand into both (a,b) and (b,a).
    pub fn edges_as_directed(&self) -> BTreeSet<Edge> {
        if self.directed {
            return self.edges.clone();
        }
        let mut out = BTreeSet::new();
        for (a, b) in &self.edges {
            out.insert((a.clone(), b.clone()));
            out.insert((b.clone(), a.clone()));
        }
        out
    }

    /// Returns this graph's edge set as undirected pairs (normalized,
    /// smaller label first). A directed a->b and b->a collapse to one
    /// undirected edge {a,b}.
    pub fn edges_as_undirected(&self) -> BTreeSet<Edge> {
        if !self.directed {
            return self.edges.clone();
        }
        let mut out = BTreeSet::new();
        for (a, b) in &self.edges {
            if a <= b {
                out.insert((a.clone(), b.clone()));
            } else {
                out.insert((b.clone(), a.clone()));
            }
        }
        out
    }

    /// Edge set expressed in the requested mode (used by set operations so
    /// two graphs of differing directedness can still be combined).
    fn edges_as(&self, directed: bool) -> BTreeSet<Edge> {
        if directed {
            self.edges_as_directed()
        } else {
            self.edges_as_undirected()
        }
    }

    /// Builds a graph from a vertex set and an edge set that are already in
    /// the target `directed` mode.
    fn from_sets(directed: bool, vertices: BTreeSet<VertexId>, edges: BTreeSet<Edge>) -> Self {
        Self {
            directed,
            vertices,
            edges,
        }
    }

    pub fn union(a: &Graph, b: &Graph, directed: bool) -> Graph {
        let vertices: BTreeSet<_> = a.vertices.union(&b.vertices).cloned().collect();
        let edges: BTreeSet<_> = a
            .edges_as(directed)
            .union(&b.edges_as(directed))
            .cloned()
            .collect();
        Graph::from_sets(directed, vertices, edges)
    }

    pub fn intersection(a: &Graph, b: &Graph, directed: bool) -> Graph {
        let vertices: BTreeSet<_> = a.vertices.intersection(&b.vertices).cloned().collect();
        let edges: BTreeSet<_> = a
            .edges_as(directed)
            .intersection(&b.edges_as(directed))
            .cloned()
            .collect();
        Graph::from_sets(directed, vertices, edges)
    }

    /// V' = V(a) \ V(b), E' = edges of a (restricted to the mode) with both
    /// endpoints surviving in V'.
    pub fn difference(a: &Graph, b: &Graph, directed: bool) -> Graph {
        let vertices: BTreeSet<_> = a.vertices.difference(&b.vertices).cloned().collect();
        let edges: BTreeSet<_> = a
            .edges_as(directed)
            .into_iter()
            .filter(|(u, v)| vertices.contains(u) && vertices.contains(v))
            .collect();
        Graph::from_sets(directed, vertices, edges)
    }

    /// Edge-only difference: keeps V(a), removes edges that also appear in b.
    pub fn edge_difference(a: &Graph, b: &Graph, directed: bool) -> Graph {
        let vertices = a.vertices.clone();
        let edges: BTreeSet<_> = a
            .edges_as(directed)
            .difference(&b.edges_as(directed))
            .cloned()
            .collect();
        Graph::from_sets(directed, vertices, edges)
    }

    pub fn symmetric_difference(a: &Graph, b: &Graph, directed: bool) -> Graph {
        let vertices: BTreeSet<_> = a
            .vertices
            .symmetric_difference(&b.vertices)
            .cloned()
            .collect();
        let edges: BTreeSet<_> = a
            .edges_as(directed)
            .symmetric_difference(&b.edges_as(directed))
            .cloned()
            .collect();
        Graph::from_sets(directed, vertices, edges)
    }

    /// Induced subgraph on the given vertex subset.
    pub fn induced_subgraph(a: &Graph, subset: &BTreeSet<VertexId>) -> Graph {
        let vertices: BTreeSet<_> = a.vertices.intersection(subset).cloned().collect();
        let edges: BTreeSet<_> = a
            .edges
            .iter()
            .filter(|(u, v)| vertices.contains(u) && vertices.contains(v))
            .cloned()
            .collect();
        Graph::from_sets(a.directed, vertices, edges)
    }

    /// Complement: same vertex set, edge set is "all possible edges minus
    /// the existing ones" in the graph's own directedness.
    pub fn complement(a: &Graph) -> Graph {
        let mut edges = BTreeSet::new();
        if a.directed {
            for u in &a.vertices {
                for v in &a.vertices {
                    if u != v && !a.edges.contains(&(u.clone(), v.clone())) {
                        edges.insert((u.clone(), v.clone()));
                    }
                }
            }
        } else {
            let verts: Vec<_> = a.vertices.iter().cloned().collect();
            for i in 0..verts.len() {
                for j in (i + 1)..verts.len() {
                    let e = (verts[i].clone(), verts[j].clone());
                    if !a.edges.contains(&e) {
                        edges.insert(e);
                    }
                }
            }
        }
        Graph::from_sets(a.directed, a.vertices.clone(), edges)
    }

    /// Reinterprets this graph as directed/undirected, converting the edge
    /// set accordingly (collapsing or duplicating as needed).
    pub fn with_directed(a: &Graph, directed: bool) -> Graph {
        let edges = a.edges_as(directed);
        Graph::from_sets(directed, a.vertices.clone(), edges)
    }

    pub fn random(directed: bool, n: usize, p: f64, rng: &mut impl rand::Rng) -> Graph {
        let mut g = Graph::new(directed);
        let labels: Vec<VertexId> = (0..n).map(|i| format!("v{i}")).collect();
        for l in &labels {
            g.add_vertex(l.clone());
        }
        for i in 0..n {
            let j_range: Vec<usize> = if directed {
                (0..n).filter(|&j| j != i).collect()
            } else {
                ((i + 1)..n).collect()
            };
            for j in j_range {
                if rng.gen_bool(p) {
                    g.add_edge(&labels[i], &labels[j]);
                }
            }
        }
        g
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(directed: bool, verts: &[&str], edges: &[(&str, &str)]) -> Graph {
        let mut g = Graph::new(directed);
        for v in verts {
            g.add_vertex(*v);
        }
        for (a, b) in edges {
            g.add_edge(a, b);
        }
        g
    }

    #[test]
    fn undirected_edge_normalization() {
        let mut graph = Graph::new(false);
        graph.add_edge("b", "a");
        assert!(graph.has_edge("a", "b"));
        assert!(graph.has_edge("b", "a"));
        assert_eq!(graph.edges.len(), 1);
    }

    #[test]
    fn no_self_loops() {
        let mut graph = Graph::new(true);
        graph.add_edge("a", "a");
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn union_combines_vertices_and_edges() {
        let a = g(false, &["a", "b"], &[("a", "b")]);
        let b = g(false, &["b", "c"], &[("b", "c")]);
        let u = Graph::union(&a, &b, false);
        assert_eq!(
            u.vertices,
            ["a", "b", "c"].iter().map(|s| s.to_string()).collect()
        );
        assert!(u.has_edge("a", "b"));
        assert!(u.has_edge("b", "c"));
        assert_eq!(u.edges.len(), 2);
    }

    #[test]
    fn intersection_keeps_only_shared_edges() {
        let a = g(false, &["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let b = g(false, &["a", "b", "c"], &[("a", "b")]);
        let i = Graph::intersection(&a, &b, false);
        assert!(i.has_edge("a", "b"));
        assert!(!i.has_edge("b", "c"));
    }

    #[test]
    fn vertex_difference_drops_dangling_edges() {
        let a = g(false, &["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let b = g(false, &["c"], &[]);
        let d = Graph::difference(&a, &b, false);
        assert_eq!(
            d.vertices,
            ["a", "b"].iter().map(|s| s.to_string()).collect()
        );
        assert!(d.has_edge("a", "b"));
        assert!(!d.has_edge("b", "c")); // c no longer exists in the result
    }

    #[test]
    fn symmetric_difference_is_self_inverse() {
        let a = g(false, &["a", "b"], &[("a", "b")]);
        let b = g(false, &["b", "c"], &[("b", "c")]);
        let sd1 = Graph::symmetric_difference(&a, &b, false);
        let sd2 = Graph::symmetric_difference(&b, &a, false);
        assert_eq!(sd1.vertices, sd2.vertices);
        assert_eq!(sd1.edges, sd2.edges);
        assert!(sd1.vertices.contains("a"));
        assert!(sd1.vertices.contains("c"));
        assert!(!sd1.vertices.contains("b")); // b is in both -> symmetric diff removes it
    }

    #[test]
    fn complement_undirected() {
        let a = g(false, &["a", "b", "c"], &[("a", "b")]);
        let c = Graph::complement(&a);
        assert!(!c.has_edge("a", "b"));
        assert!(c.has_edge("a", "c"));
        assert!(c.has_edge("b", "c"));
        assert_eq!(c.edges.len(), 2);
    }

    #[test]
    fn induced_subgraph_restricts_edges() {
        let a = g(
            false,
            &["a", "b", "c"],
            &[("a", "b"), ("b", "c"), ("a", "c")],
        );
        let subset: BTreeSet<VertexId> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        let sub = Graph::induced_subgraph(&a, &subset);
        assert_eq!(sub.vertices.len(), 2);
        assert!(sub.has_edge("a", "b"));
        assert!(!sub.has_edge("b", "c"));
        assert!(!sub.has_edge("a", "c"));
    }

    #[test]
    fn directed_undirected_roundtrip_collapses_bidirectional_edges() {
        let mut directed = Graph::new(true);
        directed.add_edge("a", "b");
        directed.add_edge("b", "a");
        let undirected = Graph::with_directed(&directed, false);
        assert_eq!(undirected.edges.len(), 1);
        assert!(undirected.has_edge("a", "b"));
    }

    #[test]
    fn random_graph_respects_vertex_count_and_no_self_loops() {
        let mut rng = rand::thread_rng();
        let graph = Graph::random(true, 12, 0.3, &mut rng);
        assert_eq!(graph.vertices.len(), 12);
        for (a, b) in &graph.edges {
            assert_ne!(a, b);
        }
    }
}
