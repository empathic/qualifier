#[cfg(target_os = "emscripten")]
pub fn run() -> crate::Result<()> {
    Err(crate::Error::Validation(
        "init is not available in the browser".into(),
    ))
}

#[cfg(not(target_os = "emscripten"))]
pub fn run() -> crate::Result<()> {
    use std::fs;
    use std::path::Path;

    use crate::qual_file::{detect_vcs, find_project_root};

    let root = find_project_root(Path::new(".")).unwrap_or_else(|| ".".into());

    // Create qualifier.graph.jsonl if it doesn't exist
    let graph_path = root.join("qualifier.graph.jsonl");
    if graph_path.exists() {
        println!("  qualifier.graph.jsonl already exists");
    } else {
        fs::write(&graph_path, "")?;
        println!("  Created qualifier.graph.jsonl (empty — populate with your dependency graph)");
    }

    // VCS-specific setup
    match detect_vcs(&root) {
        Some("git") => {
            let gitattributes = root.join(".gitattributes");
            let content = if gitattributes.exists() {
                fs::read_to_string(&gitattributes)?
            } else {
                String::new()
            };

            if content.contains("*.qual") {
                println!("  .gitattributes already contains *.qual rule");
            } else {
                let mut file = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&gitattributes)?;
                use std::io::Write;
                if !content.is_empty() && !content.ends_with('\n') {
                    writeln!(file)?;
                }
                writeln!(file, "*.qual merge=union")?;
                println!("  Detected VCS: git");
                println!("  Added *.qual merge=union to .gitattributes");
            }
        }
        Some("hg") => {
            println!("  Detected VCS: hg");
            println!("  Add `**.qual = union` to your .hgrc merge patterns");
        }
        Some(vcs) => {
            println!("  Detected VCS: {vcs}");
            println!("  Configure your VCS to use union merge for *.qual files");
            println!("  (see SPEC.md section 7 for details)");
        }
        None => {
            println!("  No VCS detected — skipping merge configuration (see SPEC.md section 7)");
        }
    }

    Ok(())
}
