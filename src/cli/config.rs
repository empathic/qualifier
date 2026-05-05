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
    /// Default issuer for annotations.
    #[serde(default)]
    pub issuer: Option<String>,

    /// Default output format ("human" or "json").
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "human".into()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            issuer: None,
            format: default_format(),
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

    // Environment variables: QUALIFIER_ISSUER, QUALIFIER_FORMAT, etc.
    figment = figment.merge(Env::prefixed("QUALIFIER_"));

    figment.extract().unwrap_or_default()
}
