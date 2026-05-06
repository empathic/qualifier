+++
name = "compact"
summary = "Compact a .qual file"
sees_also = ["review"]
since = "0.5.0"
+++

# qualifier compact

## Purpose

Prune superseded records from a `.qual` file, or collapse the full history
to a single epoch record.

## When to use it

`.qual` files grow as annotations are added, superseded, and resolved.
`compact` is a maintenance command for trimming that growth. Run it
occasionally to keep `.qual` files readable and diff-friendly — not after
every annotation. The default prune mode removes records that have been
superseded by a later record in the same file. The `--snapshot` mode goes
further, collapsing all remaining records into a single epoch record that
preserves the final state as a summary. Snapshot is appropriate when you
want a clean break in the history (e.g., after a major release audit);
prune alone is the lighter-weight routine operation.

## Common invocations

```bash
# Prune superseded records for one artifact
qualifier compact src/auth.rs

# Preview what would be pruned, without writing
qualifier compact src/auth.rs --dry-run

# Snapshot one artifact's history to a single epoch record
qualifier compact src/auth.rs --snapshot

# Compact every .qual file in the project
qualifier compact --all

# Snapshot all files (use with care — removes detailed history)
qualifier compact --all --snapshot
```

## Flags worth knowing

**`--dry-run`** prints what would be removed or collapsed without modifying
any files. Always run this first when operating on a shared repo so you can
review the impact before committing.

**`--snapshot`** collapses all remaining (non-superseded) records into one
`epoch` record with `issuer: "urn:qualifier:compact"` and
`issuer_type: tool`. The original individual annotation IDs are no longer
present after snapshotting, so downstream tools cannot reference them. Use
`--dry-run` first and ensure no active ids are referenced by open
`--supersedes` or `--references` chains.

**`--all`** discovers and compacts every `.qual` file under the project root.
Combine with `--dry-run` to audit before committing the result.

## Gotchas

- The argument to `compact` is the **artifact name** (e.g., `src/auth.rs`),
  not the `.qual` file path. The command resolves the relevant `.qual` file
  from the artifact name.
- Compacting removes superseded records permanently from the file on disk.
  Ensure the `.qual` file is tracked in VCS before compacting so the history
  is recoverable if needed.
- `--snapshot` is destructive to annotation identity: individual record IDs
  disappear. Any external system that stored those IDs (e.g., a ticket
  linked to an annotation) will have broken references. Only snapshot when
  you are certain no one depends on the old IDs.
- If nothing is superseded, `compact` reports "nothing to compact" and makes
  no changes, so it is safe to run idempotently.
