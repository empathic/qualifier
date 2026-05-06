//! Build-time generator for the `qualifier agents` page registry.
//!
//! Walks `src/cli/commands/agents/pages/*.md`, parses TOML frontmatter
//! delimited by `+++` lines, and emits `$OUT_DIR/agents_pages.rs`
//! containing:
//!
//! - `pub const OVERVIEW: &str = "...";`
//! - `pub const PAGES: &[Page] = &[...];`
//!
//! `Page` is defined in `src/cli/commands/agents/mod.rs`. The generated
//! file is `include!`d there.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

const PAGES_DIR: &str = "src/cli/commands/agents/pages";

#[derive(Deserialize)]
struct PageMeta {
    name: String,
    summary: Option<String>,
    #[serde(default)]
    sees_also: Vec<String>,
    since: Option<String>,
}

fn main() {
    println!("cargo:rerun-if-changed={PAGES_DIR}");
    println!("cargo:rerun-if-changed=build.rs");

    let mut entries: Vec<_> = fs::read_dir(PAGES_DIR)
        .unwrap_or_else(|e| panic!("read {PAGES_DIR}: {e}"))
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();
    entries.sort_by_key(|e| e.path());

    let mut overview_body: Option<String> = None;
    let mut topic_entries: Vec<String> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for entry in entries {
        let path = entry.path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| panic!("non-utf8 filename: {}", path.display()))
            .to_string();
        let raw =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

        let after_open = raw.strip_prefix("+++\n").unwrap_or_else(|| {
            panic!(
                "{}: file must begin with '+++' frontmatter delimiter on its first line",
                path.display()
            )
        });
        let close_offset = after_open.find("\n+++\n").unwrap_or_else(|| {
            panic!(
                "{}: missing closing '+++' frontmatter delimiter",
                path.display()
            )
        });
        let frontmatter = &after_open[..close_offset];
        let body = after_open[close_offset + "\n+++\n".len()..].trim_start_matches('\n');

        let meta: PageMeta = toml::from_str(frontmatter)
            .unwrap_or_else(|e| panic!("{}: invalid TOML frontmatter: {e}", path.display()));

        if meta.name != stem {
            panic!(
                "{}: frontmatter name '{}' does not match filename stem '{}'",
                path.display(),
                meta.name,
                stem
            );
        }
        if !seen_names.insert(meta.name.clone()) {
            panic!("duplicate page name '{}'", meta.name);
        }

        if stem.starts_with('_') {
            if stem == "_overview" {
                overview_body = Some(body.to_string());
            } else {
                panic!(
                    "{}: unsupported internal page; only '_overview' is recognized",
                    path.display()
                );
            }
        } else {
            let summary = meta.summary.unwrap_or_else(|| {
                panic!(
                    "{}: topic page is missing required 'summary' field",
                    path.display()
                )
            });
            let sees_also_lit = meta
                .sees_also
                .iter()
                .map(|s| format!("{s:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            let since_lit = match meta.since {
                Some(v) => format!("Some({v:?})"),
                None => "None".into(),
            };
            topic_entries.push(format!(
                "    Page {{ name: {:?}, summary: {:?}, sees_also: &[{sees_also_lit}], since: {since_lit}, body: {:?} }},",
                meta.name, summary, body,
            ));
        }
    }

    let overview = overview_body.unwrap_or_else(|| {
        panic!("{PAGES_DIR}/_overview.md is required but not found");
    });

    let generated = format!(
        "const OVERVIEW: &str = {overview:?};\n\nconst PAGES: &[Page] = &[\n{}\n];\n",
        topic_entries.join("\n"),
    );

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("agents_pages.rs");
    fs::write(&out_path, generated)
        .unwrap_or_else(|e| panic!("write {}: {e}", out_path.display(),));
}
