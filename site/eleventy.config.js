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

function prismHighlight(code, lang) {
  if (lang && Prism.languages[lang]) {
    var highlighted = Prism.highlight(code, Prism.languages[lang], lang);
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
  return "";
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
  const specMd = markdownIt({
    html: true,
    linkify: true,
    highlight: prismHighlight,
  }).use(markdownItAnchor, {
    permalink: markdownItAnchor.permalink.headerLink(),
    slugify,
  });
  eleventyConfig.addGlobalData("specHtml", specMd.render(specContent));

  // Load example files for the interactive playground
  eleventyConfig.addGlobalData("playgroundFiles", () => {
    const dir = "examples";
    const files = [
      "src-parser.rs.qual",
      "src-auth.rs.qual",
      "qualifier.graph.jsonl",
    ];
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
