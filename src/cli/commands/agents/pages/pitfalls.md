+++
name = "pitfalls"
summary = "Common mistakes agents make with qualifier"
since = "0.5.0"
+++

# qualifier — common pitfalls

- **Recording without `--span` when the concern is about specific lines.**
  A concern about a particular function or block is most actionable when it
  points at the exact lines. Without a span, the annotation attaches to the
  whole file and `qualifier review` cannot detect drift when the code changes.
  Do this instead: `qualifier record concern src/foo.rs:42:58 "..."`.

- **Conflating `reply` (add context) with `resolve` (close the issue).**
  `qualifier reply` writes a new `comment` that *references* the target — both
  records remain active. `qualifier resolve` writes a `resolve` record that
  *supersedes* the target, removing it from active views. Use `reply` when you
  have more information to add; use `resolve` when the concern is addressed.

- **Recording an annotation when a commit message would do.**
  Annotations are for observations that survive the current change and need to
  be visible to whoever touches the code next. If the information is only
  relevant to *this* commit (what changed and why), put it in the commit
  message instead. Annotations accumulate; don't add noise that expires
  immediately.

- **Editing `.qual` files by hand instead of using `record` or `emit`.**
  Record IDs are BLAKE3 hashes of the canonical form. Editing any field
  directly invalidates the `id`, and qualifier will reject the record on next
  read. Always use `qualifier record`, `qualifier emit`, `qualifier reply`,
  or `qualifier resolve` to write records.

- **Choosing `bug` or another custom kind when a built-in kind fits.**
  The built-in kinds (`concern`, `blocker`, `fail`, `pass`, `comment`,
  `praise`, `suggestion`, `waiver`, `resolve`) carry polarity semantics used
  by downstream tools for filtering and display. A custom kind like `bug` is
  valid but invisible to anything that filters by polarity or standard kind.
  Prefer `concern` for non-blocking bugs and `blocker` for must-fix issues;
  reserve custom kinds for genuinely domain-specific signals.

- **Using a non-URI issuer (must contain `:`).**
  Validation rejects any `issuer` value that does not contain a colon. A bare
  email address like `agent@example.com` will fail. Wrap it:
  `--issuer "mailto:agent@example.com"`. If your agent has an HTTP identity,
  use that directly: `--issuer "https://agents.example.com/review-bot"`.

<!-- Add new pitfalls here as we observe them. -->
