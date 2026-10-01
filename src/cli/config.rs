use figment::Figment;
use figment::providers::{Format as _, Serialized, Toml};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::cli::output::Format;

/// Variable that sets the default issuer (see [`Config::issuer`]).
pub const ENV_ISSUER: &str = "QUALIFIER_ISSUER";
/// Variable that sets the default output format (see [`Config::format`]).
pub const ENV_FORMAT: &str = "QUALIFIER_FORMAT";

/// Qualifier configuration, merged from multiple sources via figment.
///
/// Precedence (highest wins):
/// 1. CLI flags (`--issuer`, `--format`), applied by the commands
/// 2. Environment variables (`QUALIFIER_ISSUER`, `QUALIFIER_FORMAT`)
/// 3. Project-level `.qualifier.toml`
/// 4. User-level `~/.config/qualifier/config.toml`
/// 5. Defaults (issuer from VCS identity; human output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default issuer for new records.
    #[serde(default)]
    pub issuer: Option<String>,

    /// Default `--format` for every command that has one.
    #[serde(default = "default_format")]
    pub format: Format,
}

fn default_format() -> Format {
    Format::Human
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

/// A trimmed, non-empty environment variable.
fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Load configuration by merging all sources.
///
/// Returns an error if any present config file is malformed or holds an
/// invalid value, or if `QUALIFIER_FORMAT` is not `human` or `json`.
/// Missing config files are not an error. Environment variables are read
/// as strings, so `QUALIFIER_ISSUER=12345` is the issuer `12345`; empty
/// variables count as unset.
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

    for (key, var) in [("issuer", ENV_ISSUER), ("format", ENV_FORMAT)] {
        if let Some(value) = env_nonempty(var) {
            figment = figment.merge(Serialized::default(key, value));
        }
    }

    figment
        .extract()
        .map_err(|e| crate::Error::Validation(format!("invalid configuration: {e}")))
}

static CURRENT: OnceLock<Config> = OnceLock::new();

/// Install the configuration loaded at startup. Later calls are ignored.
pub fn init(config: Config) {
    let _ = CURRENT.set(config);
}

/// The configuration installed by [`init`], or defaults when none was.
pub fn current() -> &'static Config {
    CURRENT.get_or_init(Config::default)
}
