+++
name = "ls"
summary = "List artifacts by kind"
sees_also = ["show"]
since = "0.5.0"
+++

# qualifier ls

## Purpose

List all artifacts that have annotations, with an optional filter by
annotation kind.

## When to use it

`ls` gives a project-wide inventory of annotated artifacts — useful for an
initial orientation or for finding every artifact with a given kind of
annotation. It does not show the content of the annotations; it only shows
subject names and counts. For the annotations themselves, follow up with
`qualifier show <artifact>`. Compare with `show`, which requires a specific
artifact name and shows full annotation text, and `praise`, which focuses
on authorship for one artifact.

## Common invocations

```bash
# List all annotated artifacts in the project
qualifier ls

# Filter to artifacts that have at least one "concern" annotation
qualifier ls --kind concern

# Machine-readable JSON output
qualifier ls --format json
```

## Flags worth knowing

**`--kind <KIND>`** filters the output to artifacts that have at least one
annotation of the given kind (e.g., `concern`, `pass`, `blocker`). Only
standard and custom kind strings stored in `body.kind` are matched; envelope
`type` filtering is not available here (for that, use `qualifier show --type`
per artifact).

**`--format json`** emits a JSON array where each element has `subject`,
`annotation_count`, and `kinds` (an array of all kind strings recorded for
that artifact, including duplicates). This is useful for scripts that need
to walk every annotated artifact or count by kind.

**`--no-ignore`** bypasses `.gitignore` and `.qualignore` filtering when
discovering `.qual` files. Use this if you suspect an artifact is being
hidden by an ignore rule.

## Gotchas

- `ls` operates on `.qual` files discovered from the project root. It does
  not scan source files for artifacts that lack annotations — the
  `--unqualified` flag exists as a placeholder but is not yet implemented.
- Counts include all records in the `.qual` file, not just active ones.
  An artifact that was annotated and then fully resolved will still appear
  in `ls` output with a non-zero count.
- The output is sorted alphabetically by subject. There is no sort-by-count
  option; pipe through your shell's `sort` if needed.
