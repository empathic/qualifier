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

/// Resolve the user home directory across platforms.
///
/// Prefers `$HOME` (POSIX) and falls back to `$USERPROFILE` (Windows). Returns
/// `None` when neither is set so the user-level config merge is skipped rather
/// than silently failing.
fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Load configuration by merging all sources.
///
/// Returns an error if any present config file is malformed or any
/// `QUALIFIER_*` env var fails to deserialize. Missing config files are not
/// an error.
pub fn load(project_root: Option<&Path>) -> crate::Result<Config> {
    let mut figment = Figment::new().merge(Serialized::defaults(Config::default()));

    if let Some(home) = user_home_dir() {
        let user_config = home.join(".config").join("qualifier").join("config.toml");
        figment = figment.merge(Toml::file(user_config));
    }

    if let Some(root) = project_root {
        let project_config = root.join(".qualifier.toml");
        figment = figment.merge(Toml::file(project_config));
    }

    figment = figment.merge(Env::prefixed("QUALIFIER_"));

    figment
        .extract()
        .map_err(|e| crate::Error::Validation(format!("invalid configuration: {e}")))
}
