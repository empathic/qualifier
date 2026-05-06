use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    /// Topic name (e.g. `record`, `concepts`, `workflows`). Omit to print the orientation page.
    pub topic: Option<String>,
}

struct Page {
    name: &'static str,
    summary: &'static str,
    body: &'static str,
}

const OVERVIEW: &str = include_str!("pages/_overview.md");

const PAGES: &[Page] = &[
    Page {
        name: "concepts",
        summary: "Annotation model, kinds, supersession, IDs, .qual layout",
        body: include_str!("pages/concepts.md"),
    },
    Page {
        name: "workflows",
        summary: "Worked recipes for common tasks",
        body: include_str!("pages/workflows.md"),
    },
    Page {
        name: "pitfalls",
        summary: "Common mistakes agents make with qualifier",
        body: include_str!("pages/pitfalls.md"),
    },
    Page {
        name: "record",
        summary: "Record a new annotation",
        body: include_str!("pages/record.md"),
    },
    Page {
        name: "reply",
        summary: "Reply to an existing record",
        body: include_str!("pages/reply.md"),
    },
    Page {
        name: "resolve",
        summary: "Resolve (close) a record",
        body: include_str!("pages/resolve.md"),
    },
    Page {
        name: "emit",
        summary: "Emit a raw record of any type",
        body: include_str!("pages/emit.md"),
    },
    Page {
        name: "show",
        summary: "Show annotations for an artifact",
        body: include_str!("pages/show.md"),
    },
    Page {
        name: "ls",
        summary: "List artifacts by kind",
        body: include_str!("pages/ls.md"),
    },
    Page {
        name: "praise",
        summary: "Show who annotated an artifact and why",
        body: include_str!("pages/praise.md"),
    },
    Page {
        name: "review",
        summary: "Check freshness of annotations against current code",
        body: include_str!("pages/review.md"),
    },
    Page {
        name: "compact",
        summary: "Compact a .qual file",
        body: include_str!("pages/compact.md"),
    },
];

fn topic_names() -> String {
    PAGES.iter().map(|p| p.name).collect::<Vec<_>>().join(", ")
}

pub fn run(args: Args) -> crate::Result<()> {
    match args.topic.as_deref() {
        None => {
            print!("{}", render_overview());
            Ok(())
        }
        Some(name) => {
            if let Some(page) = PAGES.iter().find(|p| p.name == name) {
                print!("{}", page.body);
                Ok(())
            } else {
                eprintln!(
                    "qualifier agents: no such topic '{name}'. Available: {}",
                    topic_names()
                );
                std::process::exit(2);
            }
        }
    }
}

fn render_overview() -> String {
    // Replace the {{TOPICS}} sentinel with a `name — summary` listing.
    let topics_block: String = PAGES
        .iter()
        .map(|p| format!("- `{}` — {}", p.name, p.summary))
        .collect::<Vec<_>>()
        .join("\n");
    OVERVIEW.replace("{{TOPICS}}", &topics_block)
}

#[cfg(test)]
mod tests {
    #[test]
    fn overview_contains_topics_sentinel() {
        // If this fails, the orientation page lost the {{TOPICS}} substitution
        // anchor and the rendered overview will no longer list children.
        assert!(super::OVERVIEW.contains("{{TOPICS}}"));
    }
}
