use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Qualifier configuration, merged from multiple sources via figment.
///
/// Precedence (highest wins):
/// 1. CLI flags (passed via `Serialized`)
/// 2. Environment variables (`QUALIFIER_*`)
/// 3. Project-level `.qualifier.toml`
/// 4. User-level `~/.config/qualifier/config.toml`
/// 5. Defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Path to the dependency graph file.
    #[serde(default = "default_graph_path")]
    pub graph: PathBuf,

    /// Default author for attestations.
    #[serde(default)]
    pub author: Option<String>,

    /// Default output format ("human" or "json").
    #[serde(default = "default_format")]
    pub format: String,

    /// Minimum score threshold for `qualifier check`.
    #[serde(default)]
    pub min_score: i32,
}

fn default_graph_path() -> PathBuf {
    PathBuf::from("qualifier.graph.jsonl")
}

fn default_format() -> String {
    "human".into()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            graph: default_graph_path(),
            author: None,
            format: default_format(),
            min_score: 0,
        }
    }
}

/// Load configuration by merging all sources.
pub fn load(project_root: Option<&Path>) -> Config {
    let mut figment = Figment::new().merge(Serialized::defaults(Config::default()));

    // User-level config: ~/.config/qualifier/config.toml
    if let Ok(home) = std::env::var("HOME") {
        let user_config = PathBuf::from(home)
            .join(".config")
            .join("qualifier")
            .join("config.toml");
        figment = figment.merge(Toml::file(user_config));
    }

    // Project-level config: <root>/.qualifier.toml
    if let Some(root) = project_root {
        let project_config = root.join(".qualifier.toml");
        figment = figment.merge(Toml::file(project_config));
    }

    // Environment variables: QUALIFIER_GRAPH, QUALIFIER_AUTHOR, etc.
    figment = figment.merge(Env::prefixed("QUALIFIER_"));

    figment.extract().unwrap_or_default()
}

/// Load the dependency graph, falling back to an empty graph.
///
/// If `explicit_path` is set, loads from that path.
/// Otherwise looks for `qualifier.graph.jsonl` under `root`.
pub fn load_graph(
    explicit_path: Option<&str>,
    root: Option<&Path>,
) -> crate::graph::DependencyGraph {
    if let Some(path) = explicit_path {
        crate::graph::load(Path::new(path))
            .unwrap_or_else(|_| crate::graph::DependencyGraph::empty())
    } else if let Some(root) = root {
        let default = root.join("qualifier.graph.jsonl");
        if default.exists() {
            crate::graph::load(&default).unwrap_or_else(|_| crate::graph::DependencyGraph::empty())
        } else {
            crate::graph::DependencyGraph::empty()
        }
    } else {
        crate::graph::DependencyGraph::empty()
    }
}
