//! Lua scripting backend: lets the user write filter scripts that turn one
//! graph into a new graph, plus a shared "library" of Lua helper functions
//! loaded before every filter runs.
//!
//! A filter script must define a global function `filter(g)` that receives
//! the input graph and returns a new graph (built via `new_graph(directed)`
//! and its `add_vertex`/`add_edge` methods, or via one of the built-in
//! helper functions below).
//!
//! Lua state is created fresh per run with `Lua::new()`, which only loads
//! the "safe" standard library subset (no file/process/OS access).

use std::collections::BTreeSet;

use mlua::{Lua, UserData, UserDataMethods, Value};

use crate::graph::Graph;

impl UserData for Graph {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("directed", |_, this, ()| Ok(this.directed));

        methods.add_method("vertices", |_, this, ()| {
            Ok(this.vertices.iter().cloned().collect::<Vec<_>>())
        });

        methods.add_method("edges", |lua, this, ()| {
            let arr = lua.create_table()?;
            for (i, (a, b)) in this.edges.iter().enumerate() {
                let t = lua.create_table()?;
                t.set(1, a.clone())?;
                t.set(2, b.clone())?;
                arr.set(i + 1, t)?;
            }
            Ok(arr)
        });

        methods.add_method("has_edge", |_, this, (a, b): (String, String)| {
            Ok(this.has_edge(&a, &b))
        });

        methods.add_method("degree", |_, this, v: String| {
            Ok(this
                .edges
                .iter()
                .filter(|(a, b)| *a == v || *b == v)
                .count())
        });

        methods.add_method("neighbors", |_, this, v: String| {
            let mut out: BTreeSet<String> = BTreeSet::new();
            for (a, b) in &this.edges {
                if *a == v {
                    out.insert(b.clone());
                } else if *b == v {
                    out.insert(a.clone());
                }
            }
            Ok(out.into_iter().collect::<Vec<_>>())
        });

        methods.add_method_mut("add_vertex", |_, this, v: String| {
            this.add_vertex(v);
            Ok(())
        });

        methods.add_method_mut("add_edge", |_, this, (a, b): (String, String)| {
            this.add_edge(&a, &b);
            Ok(())
        });

        methods.add_method_mut("remove_vertex", |_, this, v: String| {
            this.remove_vertex(&v);
            Ok(())
        });

        methods.add_method_mut("remove_edge", |_, this, (a, b): (String, String)| {
            this.remove_edge(&a, &b);
            Ok(())
        });
    }
}

impl mlua::FromLua for Graph {
    fn from_lua(value: Value, _lua: &Lua) -> mlua::Result<Self> {
        match value {
            Value::UserData(ud) => Ok(ud.borrow::<Graph>()?.clone()),
            other => Err(mlua::Error::FromLuaConversionError {
                from: other.type_name(),
                to: "Graph".to_string(),
                message: Some("Erwarte einen Graphen (userdata)".to_string()),
            }),
        }
    }
}

fn register_api(lua: &Lua) -> mlua::Result<()> {
    let globals = lua.globals();

    globals.set(
        "new_graph",
        lua.create_function(|_, directed: bool| Ok(Graph::new(directed)))?,
    )?;

    globals.set(
        "complement",
        lua.create_function(|_, g: Graph| Ok(Graph::complement(&g)))?,
    )?;

    globals.set(
        "induced_subgraph",
        lua.create_function(|_, (g, subset): (Graph, BTreeSet<String>)| {
            Ok(Graph::induced_subgraph(&g, &subset))
        })?,
    )?;

    globals.set(
        "greedy_independent_set",
        lua.create_function(|_, g: Graph| {
            Ok(g.greedy_independent_set().into_iter().collect::<Vec<_>>())
        })?,
    )?;

    Ok(())
}

/// Runs `library` (shared helper definitions) followed by `source` (the
/// filter script itself) in a fresh Lua state, then calls the script's
/// `filter(g)` function with `input` and returns its result.
pub fn run_filter(source: &str, library: &str, input: &Graph) -> Result<Graph, String> {
    let lua = Lua::new();
    register_api(&lua).map_err(|e| format!("Interner Fehler bei API-Registrierung: {e}"))?;

    if !library.trim().is_empty() {
        lua.load(library)
            .set_name("bibliothek")
            .exec()
            .map_err(|e| format!("Fehler in der Bibliothek: {e}"))?;
    }

    lua.load(source)
        .set_name("filter")
        .exec()
        .map_err(|e| format!("Fehler im Skript: {e}"))?;

    let filter_fn: mlua::Function = lua.globals().get("filter").map_err(|_| {
        "Das Skript muss eine Funktion 'function filter(g) ... return g2 end' definieren."
            .to_string()
    })?;

    filter_fn
        .call::<Graph>(input.clone())
        .map_err(|e| format!("Fehler beim Ausführen von filter(g): {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Graph {
        let mut g = Graph::new(false);
        g.add_edge("a", "b");
        g.add_edge("b", "c");
        g.add_vertex("d");
        g
    }

    #[test]
    fn identity_filter_roundtrips() {
        let g = sample();
        let out = run_filter("function filter(g) return g end", "", &g).unwrap();
        assert_eq!(out.vertices, g.vertices);
        assert_eq!(out.edges, g.edges);
    }

    #[test]
    fn builds_new_graph_from_scratch() {
        let g = sample();
        let src = r#"
            function filter(g)
                local out = new_graph(g:directed())
                for _, v in ipairs(g:vertices()) do
                    if g:degree(v) == 0 then
                        out:add_vertex(v)
                    end
                end
                return out
            end
        "#;
        let out = run_filter(src, "", &g).unwrap();
        assert_eq!(out.vertices.len(), 1);
        assert!(out.vertices.contains("d"));
        assert!(out.edges.is_empty());
    }

    #[test]
    fn can_call_native_complement() {
        let g = sample();
        let src = "function filter(g) return complement(g) end";
        let out = run_filter(src, "", &g).unwrap();
        assert!(!out.has_edge("a", "b"));
        assert!(out.has_edge("a", "c"));
    }

    #[test]
    fn can_call_native_greedy_independent_set() {
        let g = sample();
        let src = r#"
            function filter(g)
                local out = new_graph(false)
                for _, v in ipairs(greedy_independent_set(g)) do
                    out:add_vertex(v)
                end
                return out
            end
        "#;
        let out = run_filter(src, "", &g).unwrap();
        assert_eq!(out.vertices, g.greedy_independent_set());
    }

    #[test]
    fn library_functions_are_available_in_filter() {
        let g = sample();
        let library = r#"
            function is_isolated(g, v)
                return g:degree(v) == 0
            end
        "#;
        let src = r#"
            function filter(g)
                local out = new_graph(g:directed())
                for _, v in ipairs(g:vertices()) do
                    if is_isolated(g, v) then
                        out:add_vertex(v)
                    end
                end
                return out
            end
        "#;
        let out = run_filter(src, library, &g).unwrap();
        assert_eq!(out.vertices.len(), 1);
        assert!(out.vertices.contains("d"));
    }

    #[test]
    fn missing_filter_function_errors_clearly() {
        let g = sample();
        let err = run_filter("local x = 1", "", &g).unwrap_err();
        assert!(err.contains("filter(g)"));
    }

    #[test]
    fn lua_syntax_error_is_reported() {
        let g = sample();
        let err = run_filter("function filter(g", "", &g).unwrap_err();
        assert!(err.contains("Fehler im Skript"));
    }

    #[test]
    fn add_and_remove_methods_mutate_output_graph() {
        let g = sample();
        let src = r#"
            function filter(g)
                g:add_vertex("e")
                g:add_edge("d", "e")
                g:remove_edge("a", "b")
                return g
            end
        "#;
        let out = run_filter(src, "", &g).unwrap();
        assert!(out.vertices.contains("e"));
        assert!(out.has_edge("d", "e"));
        assert!(!out.has_edge("a", "b"));
    }
}
