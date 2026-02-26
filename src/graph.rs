use std::collections::HashMap;
use std::path::Path;

use petgraph::algo;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::Deserialize;

/// A dependency graph over qualified artifact names.
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// The underlying directed graph. Edges point from dependent -> dependency.
    pub(crate) graph: DiGraph<String, ()>,
    /// Map from artifact name to node index for fast lookup.
    pub(crate) nodes: HashMap<String, NodeIndex>,
}

/// A single entry in the `qualifier.graph.jsonl` file.
#[derive(Debug, Deserialize)]
struct GraphEntry {
    subject: String,
    depends_on: Vec<String>,
}

impl DependencyGraph {
    /// Create an empty dependency graph.
    pub fn empty() -> Self {
        DependencyGraph {
            graph: DiGraph::new(),
            nodes: HashMap::new(),
        }
    }

    /// Get or create a node for the given artifact name.
    fn get_or_insert(&mut self, name: &str) -> NodeIndex {
        if let Some(&idx) = self.nodes.get(name) {
            idx
        } else {
            let idx = self.graph.add_node(name.to_string());
            self.nodes.insert(name.to_string(), idx);
            idx
        }
    }

    /// Return all artifact names in the graph.
    pub fn artifacts(&self) -> Vec<&str> {
        self.nodes.keys().map(|s| s.as_str()).collect()
    }

    /// Return the direct dependencies of an artifact.
    pub fn dependencies(&self, artifact: &str) -> Vec<&str> {
        match self.nodes.get(artifact) {
            Some(&idx) => self
                .graph
                .neighbors_directed(idx, petgraph::Direction::Outgoing)
                .map(|n| self.graph[n].as_str())
                .collect(),
            None => vec![],
        }
    }

    /// Check if the graph contains an artifact.
    pub fn contains(&self, artifact: &str) -> bool {
        self.nodes.contains_key(artifact)
    }

    /// Return the number of artifacts.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Return true if the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Return a topological ordering of artifact names (dependencies before dependents).
    /// Returns Err if a cycle is detected.
    pub fn toposort(&self) -> crate::Result<Vec<&str>> {
        match algo::toposort(&self.graph, None) {
            Ok(order) => {
                // petgraph toposort returns dependents before dependencies,
                // reverse to get dependencies first
                Ok(order
                    .into_iter()
                    .rev()
                    .map(|idx| self.graph[idx].as_str())
                    .collect())
            }
            Err(cycle) => Err(crate::Error::Cycle {
                context: "dependency graph".into(),
                detail: format!("cycle involving artifact '{}'", self.graph[cycle.node_id()]),
            }),
        }
    }

    /// Render the graph in DOT format for Graphviz.
    pub fn to_dot(&self) -> String {
        let mut out = String::from("digraph qualifier {\n");
        out.push_str("  rankdir=LR;\n");
        out.push_str("  node [shape=box];\n");

        for (name, &idx) in &self.nodes {
            out.push_str(&format!("  \"{}\";\n", name));
            for dep in self
                .graph
                .neighbors_directed(idx, petgraph::Direction::Outgoing)
            {
                out.push_str(&format!("  \"{}\" -> \"{}\";\n", name, self.graph[dep]));
            }
        }

        out.push_str("}\n");
        out
    }
}

/// Load a dependency graph from a `qualifier.graph.jsonl` file.
///
/// Each line is a JSON object: `{"subject": "...", "depends_on": ["...", ...]}`
/// Empty lines and lines starting with `//` are skipped.
pub fn load(path: &Path) -> crate::Result<DependencyGraph> {
    let content = std::fs::read_to_string(path)?;
    parse_graph(&content)
}

/// Parse a dependency graph from a JSONL string.
pub fn parse_graph(content: &str) -> crate::Result<DependencyGraph> {
    let mut dg = DependencyGraph::empty();

    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        let entry: GraphEntry = serde_json::from_str(trimmed)
            .map_err(|e| crate::Error::Validation(format!("graph line {}: {}", line_no + 1, e)))?;

        let from = dg.get_or_insert(&entry.subject);
        for dep in &entry.depends_on {
            let to = dg.get_or_insert(dep);
            dg.graph.add_edge(from, to, ());
        }
    }

    // Verify acyclicity
    dg.toposort()?;

    Ok(dg)
}

/// Serialize the graph to JSONL format.
pub fn to_jsonl(graph: &DependencyGraph) -> String {
    let mut out = String::new();
    let mut artifacts: Vec<&str> = graph.artifacts();
    artifacts.sort();

    for artifact in artifacts {
        let mut deps = graph
            .dependencies(artifact)
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        deps.sort();

        let entry = serde_json::json!({
            "subject": artifact,
            "depends_on": deps,
        });
        out.push_str(&serde_json::to_string(&entry).unwrap());
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let g = DependencyGraph::empty();
        assert!(g.is_empty());
        assert_eq!(g.len(), 0);
    }

    #[test]
    fn test_parse_simple_graph() {
        let input = r#"{"subject":"bin/server","depends_on":["lib/auth","lib/http"]}
{"subject":"lib/auth","depends_on":["lib/crypto"]}
{"subject":"lib/http","depends_on":[]}
{"subject":"lib/crypto","depends_on":[]}
"#;
        let g = parse_graph(input).unwrap();
        assert_eq!(g.len(), 4);
        assert!(g.contains("bin/server"));
        assert!(g.contains("lib/crypto"));

        let server_deps = g.dependencies("bin/server");
        assert!(server_deps.contains(&"lib/auth"));
        assert!(server_deps.contains(&"lib/http"));
        assert_eq!(server_deps.len(), 2);

        let crypto_deps = g.dependencies("lib/crypto");
        assert!(crypto_deps.is_empty());
    }

    #[test]
    fn test_parse_with_comments_and_blanks() {
        let input = r#"// dependency graph
{"subject":"a","depends_on":["b"]}

// b has no deps
{"subject":"b","depends_on":[]}
"#;
        let g = parse_graph(input).unwrap();
        assert_eq!(g.len(), 2);
    }

    #[test]
    fn test_cycle_detection() {
        let input = r#"{"subject":"a","depends_on":["b"]}
{"subject":"b","depends_on":["c"]}
{"subject":"c","depends_on":["a"]}
"#;
        let err = parse_graph(input).unwrap_err();
        assert!(
            matches!(err, crate::Error::Cycle { .. }),
            "expected cycle error, got: {err}"
        );
    }

    #[test]
    fn test_self_cycle() {
        let input = r#"{"subject":"a","depends_on":["a"]}"#;
        let err = parse_graph(input).unwrap_err();
        assert!(matches!(err, crate::Error::Cycle { .. }));
    }

    #[test]
    fn test_toposort_order() {
        let input = r#"{"subject":"app","depends_on":["lib"]}
{"subject":"lib","depends_on":["core"]}
{"subject":"core","depends_on":[]}
"#;
        let g = parse_graph(input).unwrap();
        let order = g.toposort().unwrap();

        // core should come before lib, lib before app
        let core_pos = order.iter().position(|&x| x == "core").unwrap();
        let lib_pos = order.iter().position(|&x| x == "lib").unwrap();
        let app_pos = order.iter().position(|&x| x == "app").unwrap();
        assert!(core_pos < lib_pos);
        assert!(lib_pos < app_pos);
    }

    #[test]
    fn test_to_dot() {
        let input = r#"{"subject":"a","depends_on":["b"]}
{"subject":"b","depends_on":[]}
"#;
        let g = parse_graph(input).unwrap();
        let dot = g.to_dot();
        assert!(dot.contains("digraph qualifier"));
        assert!(dot.contains("\"a\""));
        assert!(dot.contains("\"b\""));
        assert!(dot.contains("\"a\" -> \"b\""));
    }

    #[test]
    fn test_to_jsonl_roundtrip() {
        let input = r#"{"subject":"a","depends_on":["b"]}
{"subject":"b","depends_on":[]}
"#;
        let g = parse_graph(input).unwrap();
        let jsonl = to_jsonl(&g);
        let g2 = parse_graph(&jsonl).unwrap();
        assert_eq!(g2.len(), g.len());
        assert_eq!(
            g2.dependencies("a").into_iter().collect::<Vec<_>>(),
            g.dependencies("a").into_iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_load_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("qualifier.graph.jsonl");
        std::fs::write(
            &path,
            r#"{"subject":"x","depends_on":["y"]}
{"subject":"y","depends_on":[]}
"#,
        )
        .unwrap();

        let g = load(&path).unwrap();
        assert_eq!(g.len(), 2);
    }

    #[test]
    fn test_dependencies_unknown_artifact() {
        let g = DependencyGraph::empty();
        assert!(g.dependencies("nonexistent").is_empty());
    }
}
