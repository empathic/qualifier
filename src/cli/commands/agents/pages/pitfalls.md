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

- **Resolving annotations the user has not directed you to close.**
  `qualifier resolve` *closes* a record — the original concern is hidden
  from `qualifier show`, `qualifier ls`, and other active views. An agent
  resolving an annotation it does not fully understand silently buries a
  concern the user may still want to act on. Resolution is a user
  decision: surface the annotation (e.g., with `qualifier show`) and let
  the user decide whether it is addressed.

- **Adding positive annotations the user did not ask for.**
  An agent volunteering `praise`, `pass`, or other positive-polarity
  annotations without explicit user direction creates review noise the
  user has to triage. Annotations exist to surface things the user needs
  to act on; reflexively recording approval of code you happen to be
  reading does the opposite. Record a positive annotation only when the
  user explicitly asks for it, or when documenting an intentional
  non-obvious design choice that future readers would otherwise mistake
  for a bug.

- **Using a non-URI issuer (must contain `:`).**
  Validation rejects any `issuer` value that does not contain a colon. A bare
  email address like `agent@example.com` will fail. Wrap it:
  `--issuer "mailto:agent@example.com"`. If your agent has an HTTP identity,
  use that directly: `--issuer "https://agents.example.com/review-bot"`.

<!-- Add new pitfalls here as we observe them. -->
