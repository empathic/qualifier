use std::path::Path;

/// A dependency graph over qualified artifact names.
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    // TODO: petgraph adjacency list
}

/// Load a dependency graph from a `qualifier.graph.jsonl` file.
pub fn load(_path: &Path) -> crate::Result<DependencyGraph> {
    todo!()
}
