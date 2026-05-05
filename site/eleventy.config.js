import markdownItAnchor from "markdown-it-anchor";
import markdownIt from "markdown-it";
import Prism from "prismjs";
import loadLanguages from "prismjs/components/index.js";
import { readFileSync } from "fs";

loadLanguages(["json", "bash", "rust", "toml"]);

const slugify = (s) =>
  s
    .toLowerCase()
    .replace(/[^\w\s-]/g, "")
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-")
    .trim();

function escapeHtml(s) {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function prismHighlight(code, lang) {
  // Treat jsonl as json for syntax highlighting purposes.
  const grammarLang = lang === "jsonl" ? "json" : lang;
  if (grammarLang && Prism.languages[grammarLang]) {
    const highlighted = Prism.highlight(
      code,
      Prism.languages[grammarLang],
      grammarLang,
    );
    return (
      '<pre class="language-' +
      lang +
      '"><code class="language-' +
      lang +
      '">' +
      highlighted +
      "</code></pre>"
    );
  }
  // Fallback: plain escaped text in a styled pre, so unknown languages
  // still render rather than silently vanishing.
  return (
    '<pre class="language-' +
    (lang || "text") +
    '"><code class="language-' +
    (lang || "text") +
    '">' +
    escapeHtml(code) +
    "</code></pre>"
  );
}

export default function (eleventyConfig) {
  eleventyConfig.addPassthroughCopy("css");
  eleventyConfig.addPassthroughCopy("js");
  eleventyConfig.addPassthroughCopy("wasm");
  eleventyConfig.addPassthroughCopy("examples");

  eleventyConfig.amendLibrary("md", (mdLib) => {
    mdLib.set({ highlight: prismHighlight });
    mdLib.use(markdownItAnchor, {
      permalink: markdownItAnchor.permalink.headerLink(),
      slugify,
    });
  });

  // Load SPEC.md from repo root, pre-render to HTML
  const specRaw = readFileSync("../SPEC.md", "utf-8");
  const specContent = specRaw.replace(/^# .+\n+(\*\*.+\n)*/m, "");
  const specVersionMatch = specRaw.match(/^\*\*Version:\*\*\s*(.+)$/m);
  if (!specVersionMatch) {
    throw new Error("Could not parse `**Version:**` line from SPEC.md");
  }
  const specVersion = specVersionMatch[1].trim();
  const specMd = markdownIt({
    html: true,
    linkify: true,
    highlight: prismHighlight,
  }).use(markdownItAnchor, {
    permalink: markdownItAnchor.permalink.headerLink(),
    slugify,
  });
  eleventyConfig.addGlobalData("specHtml", specMd.render(specContent));
  eleventyConfig.addGlobalData("specVersion", specVersion);

  // Wrap the body region of a Prism-highlighted JSON record in a span so
  // CSS can style it distinctly from the envelope. Walks the rendered
  // HTML, finds the `"body"` property, then counts brace depth in the
  // punctuation tokens to locate the matching close.
  function wrapRecordBody(html) {
    const bodyProp = /<span class="token property">"body"<\/span>/;
    const propMatch = html.match(bodyProp);
    if (!propMatch) return html;
    const lineStart = html.lastIndexOf("\n", propMatch.index) + 1;
    const openBraceRe = /<span class="token punctuation">{<\/span>/g;
    openBraceRe.lastIndex = propMatch.index;
    const open = openBraceRe.exec(html);
    if (!open) return html;
    const braceRe = /<span class="token punctuation">([{}])<\/span>/g;
    braceRe.lastIndex = open.index + open[0].length;
    let depth = 1;
    let closeEnd = -1;
    let m;
    while ((m = braceRe.exec(html)) !== null) {
      if (m[1] === "{") depth++;
      else if (--depth === 0) {
        closeEnd = m.index + m[0].length;
        break;
      }
    }
    if (closeEnd === -1) return html;
    return (
      html.slice(0, lineStart) +
      '<span class="record-body">' +
      html.slice(lineStart, closeEnd) +
      "</span>" +
      html.slice(closeEnd)
    );
  }

  // Vertically-stacked code comparison shortcode. Each record renders
  // at full content width with a small uppercase label above. Side-by-
  // side at the site's measure forced narrow panes that wrapped or
  // scrolled pretty-printed JSON; stacking trades vertical space for
  // legibility.
  // Usage: {% codecompare lang, labelA, codeA, labelB, codeB %}
  eleventyConfig.addShortcode(
    "codecompare",
    (lang, labelA, codeA, labelB, codeB) => {
      const paneA = wrapRecordBody(prismHighlight(codeA, lang));
      const paneB = wrapRecordBody(prismHighlight(codeB, lang));
      return `<div class="code-compare">
  <section class="cc-section">
    <div class="cc-label">${labelA}</div>
    <div class="cc-pane">${paneA}</div>
  </section>
  <section class="cc-section">
    <div class="cc-label">${labelB}</div>
    <div class="cc-pane">${paneB}</div>
  </section>
</div>`;
    },
  );

  // Load METABOX.md from repo root, pre-render to HTML
  const metaboxRaw = readFileSync("../METABOX.md", "utf-8");
  const metaboxContent = metaboxRaw.replace(/^# .+\n+(\*\*.+\n)*/m, "");
  eleventyConfig.addGlobalData("metaboxHtml", specMd.render(metaboxContent));

  // Load example files for the interactive playground
  eleventyConfig.addGlobalData("playgroundFiles", () => {
    const dir = "examples";
    const files = ["src-parser.rs.qual", "src-auth.rs.qual"];
    const result = {};
    for (const f of files) result[f] = readFileSync(`${dir}/${f}`, "utf-8");
    return result;
  });

  return {
    dir: {
      input: ".",
      output: "_site",
      includes: "_includes",
      data: "_data",
    },
    markdownTemplateEngine: "njk",
  };
}
