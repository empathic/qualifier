use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    /// Topic name (e.g. `record`, `concepts`, `workflows`). Omit to print the orientation page.
    pub topic: Option<String>,
}

pub struct Page {
    pub name: &'static str,
    pub summary: &'static str,
    pub sees_also: &'static [&'static str],
    pub since: Option<&'static str>,
    pub body: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/agents_pages.rs"));

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
