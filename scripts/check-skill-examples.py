#!/usr/bin/env python3
"""Run every qualifier example in the plugin's skills against a real binary.

Scans plugins/claude-code/skills/**/*.md (SKILL.md files and the subagent
briefs) for:

  - JSONL record lines: every line of a ```json fence, and every inline
    backticked object with a "kind", "location", "references", or
    "supersedes" key (so a line that lost its kind still fails). Each
    file's lines run as one `record --stdin --dry-run` batch in a copy of
    the fixture. Their keys must also be ones the binary's `record --help`
    lists for an overrides line, since the CLI ignores unknown keys rather
    than rejecting them.
  - qualifier invocations: `qualifier <sub> ...` or `{QUALIFIER} <sub> ...`
    at the start of a line in a shell fence, or as a whole inline code span.
    Every subcommand and flag must be one the binary's `--help` lists;
    anything else fails before it runs. Read commands run in the fixture;
    every other command runs in a throwaway copy of it.

Each example must exit 0. Two markers, each alone on the line before an
example (or before the fence that holds it), change that:

  <!-- example: skip — <reason> -->   don't run it; counted and printed
  <!-- example: expect-fail -->       it must exit non-zero

Placeholders are filled from placeholder_table() below. A `<...>` or `…`
left in free text (a command's message positional, a --detail or
--suggested-fix value, a record line's message/detail/suggested_fix) becomes
sample text; one left anywhere else fails the check, quoted or not.

Binary: $QUALIFIER_BIN, else target/debug/qualifier, else
target/release/qualifier. With none, prints a skip line and exits 2.
`-v` prints every command and record line as it is run.
Exit 0: all passed. Exit 1: a failure (each names file:line).
"""

import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SKILLS = os.path.join(REPO, "plugins", "claude-code", "skills")

# Subcommands that only read; they run in the fixture itself (which is
# checked for changes afterwards). Anything else runs in a copy.
READ_COMMANDS = {"threads", "show", "ls", "review", "diff", "praise", "agents", "haiku", "help"}

SAMPLE_TEXT = "sample text"
SESSION = "fixture"


def placeholder_table(fx, thread):
    """The placeholder -> value table. `thread` is the fixture thread an
    example targets: shell examples always use the first open thread; the
    Nth record line of a file uses open thread N (mod the count), so one
    batch can close one thread and reply on another."""
    other = fx["open"][(fx["open"].index(thread) + 1) % len(fx["open"])]
    start, end = thread["span"]
    return [
        # Controller-filled brief placeholders.
        ("{QUALIFIER}", fx["bin"]),                      # the binary under test
        ("{SCRATCH}", fx["scratch"]),                    # a directory outside the repo
        ("{TAG}", "review:fixture"),                     # the review-scoped tag
        ("{IDS or SCOPE}", thread["root"]),              # the threads a triager reads
        # IDs: the targeted thread's live root (full 64-hex ID).
        ("<root.id>", thread["root"]),
        ("<B's root.id>", thread["root"]),
        ("<id>", thread["root"]),
        ("<reply.id>", thread["reply"] or thread["root"]),  # a live reply in that thread
        ("<A's origin>", other["origin"]),               # another thread's origin
        ("<A prefix>", other["root"][:8]),
        # Subjects and spans: the targeted thread's file and new lines in it.
        ("<root.subject>", thread["subject"]),
        ("<B's root.subject>", thread["subject"]),
        ("<location>", f"{thread['subject']}:{start + 1}:{end + 1}"),
        ("<path>", thread["subject"]),
        ("<file>", thread["subject"]),                   # a redirect target becomes the batch file
        ("<start>", str(start + 1)),
        ("<end>", str(end + 1)),
        # Everything else structural.
        ("<kind>", thread["kind"]),                      # same kind: supersession keeps it
        ("<email>", "decider@example.com"),
        ("<sha>", fx["sha"]),
        ("<tag>", "fixture"),
        ("<value>", f"claude-code:{SESSION}"),           # this session's tag value
        ("<scratch>", fx["scratch"]),
    ]


def substitute(text, table):
    for key, value in table:
        text = text.replace(key, value)
    # `<a|b|c>` names a choice; take the first.
    return re.sub(r"<([a-z]+)(?:\|[a-z]+)+>", r"\1", text)


PLACEHOLDER_RE = re.compile(r"<[^<>\s][^<>]*>|\{[A-Z][A-Za-z_ ]*\}")

# --- extraction ----------------------------------------------------------------

MARKER_RE = re.compile(r"<!--\s*example:\s*(?:(skip)\s*(?:—|--?)\s*(\S.*?)|(expect-fail))\s*-->")
INVOCATION_RE = re.compile(r"^(?:qualifier|\{QUALIFIER\})\s+[a-z]")
LOOSE_INVOCATION_RE = re.compile(r"(?<![\w./-])(?:qualifier|\{QUALIFIER\})\s+[a-z]")
SHELL_LANGS = {"", "bash", "sh", "shell", "console", "zsh"}


class Example:
    def __init__(self, path, line, kind, text, mode, reason):
        self.path, self.line, self.kind, self.text = path, line, kind, text
        self.mode, self.reason = mode, reason

    @property
    def where(self):
        return f"{os.path.relpath(self.path, REPO)}:{self.line}"


def extract(path, errors):
    """Returns the file's examples; appends extraction problems to errors."""
    lines = open(path, encoding="utf-8").read().split("\n")
    rel = os.path.relpath(path, REPO)
    # Blank the frontmatter (allowed-tools names the wrapper, not an example).
    if lines and lines[0] == "---":
        for i in range(1, len(lines)):
            if lines[i] == "---":
                lines[: i + 1] = [""] * (i + 1)
                break

    markers = {}  # 1-based line a marker applies to -> [mode, reason, used]
    masked = list(lines)  # prose only: fences and markers blanked
    examples = []
    fence = None  # (lang, marker applying to the whole fence)
    pending = ""  # a shell line continued with `\`
    pending_line = 0
    for n, line in enumerate(lines, 1):
        stripped = line.strip()
        m = MARKER_RE.fullmatch(stripped)
        if m:
            mode = "skip" if m.group(1) else "expect-fail"
            markers[n + 1] = [mode, m.group(2), False]
            masked[n - 1] = ""
            continue
        if "<!--" in stripped and "example:" in stripped:
            errors.append(f"{rel}:{n}: malformed example marker: {stripped}")
        if stripped.startswith("```"):
            masked[n - 1] = ""
            if fence is None:
                fence = (stripped[3:].strip().lower(), markers.get(n))
            else:
                fence = None
            continue
        if fence is None:
            continue
        masked[n - 1] = ""
        lang, fence_marker = fence
        marker = markers.get(n) or fence_marker
        mode, reason = (marker[0], marker[1]) if marker else (None, None)
        if lang == "json":
            if stripped:
                examples.append(Example(path, n, "json", stripped, mode, reason))
                if marker:
                    marker[2] = True
        elif lang in SHELL_LANGS:
            if pending:
                text, start = pending + " " + stripped, pending_line
            else:
                text, start = stripped, n
            if text.endswith("\\"):
                pending, pending_line = text[:-1].rstrip(), start
                continue
            pending = ""
            if INVOCATION_RE.match(text) or (LOOSE_INVOCATION_RE.search(text) and mode == "skip"):
                examples.append(Example(path, start, "cmd", text, mode, reason))
                if marker:
                    marker[2] = True
            elif LOOSE_INVOCATION_RE.search(text):
                errors.append(f"{rel}:{start}: qualifier call not at the start of the line; "
                              "rewrite it or mark it `<!-- example: skip — <reason> -->`")

    # Inline code spans in prose; a span may wrap lines but not a blank line.
    prose = "\n".join(masked)
    for m in re.finditer(r"`((?:[^`\n]|\n(?![ \t]*\n))+)`", prose):
        n = prose.count("\n", 0, m.start()) + 1
        content = " ".join(m.group(1).split())
        marker = markers.get(n)
        mode, reason = (marker[0], marker[1]) if marker else (None, None)
        if re.match(r'\{\s*"', content):
            examples.append(Example(path, n, "inline-json", content, mode, reason))
        elif INVOCATION_RE.match(content) or (LOOSE_INVOCATION_RE.search(content) and mode == "skip"):
            examples.append(Example(path, n, "cmd", content, mode, reason))
        elif LOOSE_INVOCATION_RE.search(content):
            errors.append(f"{rel}:{n}: qualifier call inside `{content}` is not at its start; "
                          "rewrite it or mark it `<!-- example: skip — <reason> -->`")
            continue
        else:
            continue
        if marker:
            marker[2] = True

    for n, (mode, _reason, used) in sorted(markers.items()):
        if not used:
            errors.append(f"{rel}:{n - 1}: `example: {mode}` marker is not followed by an example")
    return examples


# --- fixture ------------------------------------------------------------------


def child_env(home):
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("QUALIFIER_", "CLAUDE", "GIT_"))}
    env.update({
        "HOME": home,
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
        # What the plugin's sessions look like to qualifier.
        "CLAUDECODE": "1",
        "CLAUDE_CODE_SESSION_ID": SESSION,
    })
    return env


def run(argv, cwd, env, stdin=None):
    with open(stdin) if stdin else open(os.devnull) as fh:
        return subprocess.run(argv, cwd=cwd, env=env, stdin=fh, capture_output=True,
                              text=True, timeout=30)


def build_fixture(qbin, root, env):
    """A git repo with source files and open, replied-to, and resolved
    threads, all written by the binary under test."""
    repo = os.path.join(root, "fixture")
    os.makedirs(repo)

    def sh(*argv):
        r = run(list(argv), repo, env)
        if r.returncode != 0:
            raise SystemExit(f"fixture: {' '.join(argv)} failed:\n{r.stdout}{r.stderr}")
        return r.stdout

    files = {"src/net/tcp.rs": 120, "src/pool.rs": 120, "docs/specs/cache.md": 90}
    for rel, count in files.items():
        os.makedirs(os.path.dirname(os.path.join(repo, rel)), exist_ok=True)
        with open(os.path.join(repo, rel), "w") as fh:
            fh.writelines(f"line {i} of {rel}\n" for i in range(1, count + 1))
    sh("git", "init", "-q", "-b", "main")
    sh("git", "config", "user.email", "dev@example.com")
    sh("git", "config", "user.name", "Dev")
    sh("git", "config", "commit.gpgsign", "false")
    sh("git", "add", "-A")
    sh("git", "commit", "-q", "-m", "fixture")

    def record(*args):
        return json.loads(sh(qbin, "record", *args, "--format", "json"))["id"]

    a = record("concern", "src/net/tcp.rs:40:52", "Retry loop has no budget",
               "--detail", "Retries forever on ECONNREFUSED", "--suggested-fix", "Cap retries at 5",
               "--tag", "review", "--tag", "review:fixture")
    sh(qbin, "reply", a, "confirmed: loop at src/net/tcp.rs:44 never exits", "--tag", "verified:confirmed")
    record("blocker", "src/net/tcp.rs:60:64", "Timeout ignored",
           "--detail", "Failure: connect(addr, 5s) blocks forever", "--tag", "review", "--tag", "review:fixture")
    c = record("suggestion", "src/pool.rs:88:90", "Take the lock inside ConnPool::get")
    sh(qbin, "reply", c, "Needs a decision: move the lock?", "--tag", "status:needs-decision")
    d = record("concern", "src/pool.rs:10:12", "Pool size is hard-coded")
    sh(qbin, "resolve", d, "Configurable since fixture commit", "--reason", "fixed")
    record("alternative", "docs/specs/cache.md:40:52", "Per-tenant cache processes",
           "--tag", "revisit:tenants exceed 50")
    sh("git", "add", "-A")
    sh("git", "commit", "-q", "-m", "fixture records")

    opened = []
    for t in json.loads(sh(qbin, "threads", "--format", "json")):
        span = t["root"]["body"].get("span") or {"start": {"line": 1}}
        start = span["start"]["line"]
        replies = [r["record"]["id"] for r in t["replies"] if r["active"]]
        opened.append({
            "root": t["root"]["id"], "origin": t["origin"], "subject": t["root"]["subject"],
            "reply": replies[0] if replies else None,
            "kind": t["root"]["body"]["kind"],
            "span": (start, span.get("end", span["start"])["line"]),
        })
    opened.sort(key=lambda t: t["root"] != a)  # the replied-to thread first
    assert len(opened) == 4 and opened[0]["root"] == a and opened[0]["reply"], opened
    return repo, opened, sh("git", "rev-parse", "--short", "HEAD").strip()


def qual_digest(repo):
    h = hashlib.sha256()
    for dirpath, dirnames, filenames in sorted(os.walk(repo)):
        dirnames[:] = sorted(d for d in dirnames if d != ".git")
        for f in sorted(filenames):
            if f.endswith(".qual"):
                h.update(f.encode())
                h.update(open(os.path.join(dirpath, f), "rb").read())
    return h.hexdigest()


def batch_keys(qbin, env):
    """Keys an overrides line may carry, from the binary's own help."""
    text = run([qbin, "record", "--help"], REPO, env).stdout
    m = re.search(r"An overrides object:\s*`(\{.*?\})`", text, re.S)
    keys = set(re.findall(r'"([a-z_]+)"\s*:', m.group(1))) if m else set()
    if not {"kind", "location", "message"} <= keys:
        raise SystemExit(f"could not read the overrides line keys from `record --help` (got {keys})")
    return keys


# --- checks ----------------------------------------------------------------------


# Free text in a command: the MESSAGE positional (found in each
# subcommand's usage line) and the values of these flags. `--help` doesn't
# mark a value flag as free text, so this list is kept by hand;
# command_grammar() checks that every flag in it still takes a value in that
# subcommand's `--help`.
FREE_TEXT_FLAGS = {
    "record": {"--detail", "--suggested-fix"},
    "reply": {"--detail", "--suggested-fix"},
}
OPTION_RE = re.compile(r"^ {2,6}(?:(-[A-Za-z0-9]), )?(--[a-z][a-z0-9-]*)?(?: <[^>]+>)?")


def command_grammar(qbin, env):
    """{subcommand: {"flags": {flag: takes_value}, "message": index or None}}
    parsed from `qualifier --help` and each subcommand's `--help`: an option
    line without a `<VALUE>` is a boolean flag. Fails loudly on help it
    can't parse."""
    top = run([qbin, "--help"], REPO, env).stdout
    # `help` is clap's own and has no --help of its own.
    subs = [s for s in re.findall(r"^  ([a-z][a-z-]*)\s{2,}\S", top, re.M) if s != "help"]
    if not {"record", "reply", "resolve", "threads"} <= set(subs):
        raise SystemExit(f"could not read the subcommands from `qualifier --help` (got {subs})")
    grammar = {}
    for sub in subs:
        text = run([qbin, sub, "--help"], REPO, env).stdout
        usage = re.search(rf"^Usage: qualifier {sub}\b(.*)$", text, re.M)
        options = text.partition("\nOptions:\n")[2]
        if not usage or not options:
            raise SystemExit(f"could not parse `qualifier {sub} --help` (no usage line or Options)")
        flags = {}
        for line in options.splitlines():
            m = OPTION_RE.match(line)
            if not m or not (m.group(1) or m.group(2)):
                continue
            takes_value = bool(re.match(r"^\s*(?:-\w, )?--[\w-]+ <", line))
            for name in (m.group(1), m.group(2)):
                if name:
                    flags[name] = takes_value
        if flags.get("--help") is not False:
            raise SystemExit(f"could not parse the options of `qualifier {sub} --help`")
        positionals = [re.sub(r"[\[\]<>.]", "", w) for w in usage.group(1).split()
                       if w != "[OPTIONS]"]
        grammar[sub] = {"flags": flags,
                        "message": positionals.index("MESSAGE") if "MESSAGE" in positionals else None}
    for sub, names in FREE_TEXT_FLAGS.items():
        for name in sorted(names):
            if grammar.get(sub, {}).get("flags", {}).get(name) is not True:
                raise SystemExit(f"FREE_TEXT_FLAGS names {sub} {name}, which `qualifier {sub} "
                                 "--help` does not list as a flag taking a value")
    return grammar


ELLIPSIS_RE = re.compile(r"…|\.\.\.")


def free_text(tok):
    return ELLIPSIS_RE.sub(SAMPLE_TEXT, PLACEHOLDER_RE.sub(SAMPLE_TEXT, tok))


def leftover(tok):
    """The first unsubstituted placeholder or ellipsis in a token, if any."""
    m = PLACEHOLDER_RE.search(tok) or ELLIPSIS_RE.search(tok)
    if m:
        return m.group(0)
    if "<" in tok or ">" in tok:
        return tok  # a placeholder with spaces split across tokens
    return None


def prepare_command(ex, fx):
    """-> (argv, subcommand, uses_stdin) or raises ValueError."""
    text = substitute(ex.text, placeholder_table(fx, fx["open"][0]))
    try:
        tokens = shlex.split(text, comments=True)
    except ValueError as e:
        raise ValueError(f"cannot parse as a shell command ({e})")
    words, stdin, i = [], False, 0
    while i < len(tokens):
        tok = tokens[i]
        if tok == "<":
            if i + 1 >= len(tokens):
                raise ValueError("`<` without a file")
            stdin, i = True, i + 2
            continue
        if tok in ("|", "||", "&&", ";", ">", ">>", "2>", "&"):
            raise ValueError(f"unsupported shell syntax {tok!r}; mark it skip")
        words.append(tok)
        i += 1
    sub = words[1] if len(words) > 1 else ""
    # An unknown subcommand or flag fails here, before anything runs, so an
    # expect-fail example can't pass on a typo instead of the failure it
    # documents.
    if sub not in fx["grammar"]:
        raise ValueError(f"unknown subcommand {sub!r}: `qualifier --help` does not list it")
    grammar = fx["grammar"][sub]
    flags = grammar["flags"]
    argv, positional, i, options_done = [fx["bin"], sub], 0, 2, False
    while i < len(words):
        tok, value = words[i], None
        flag = tok.partition("=")[0]
        if tok == "--":
            options_done = True
        elif tok.startswith("-") and len(tok) > 1 and not options_done and flag not in flags:
            raise ValueError(f"unknown flag {flag} for qualifier {sub}: "
                             f"`qualifier {sub} --help` does not list it")
        if tok.startswith("-") and not options_done and flags.get(flag, False):
            flag, eq, inline = tok.partition("=")
            if eq:
                tok, value = flag, inline
            elif i + 1 < len(words):
                value, i = words[i + 1], i + 1
            free = flag in FREE_TEXT_FLAGS.get(sub, set())
            parts = [tok] if value is None else [tok, value]
        else:
            free = not tok.startswith("-") and grammar.get("message") == positional
            if not tok.startswith("-"):
                positional += 1
            parts, value = [tok], tok
        for part in parts:
            if free and part is value:
                part = free_text(part)
            bad = leftover(part)
            if bad:
                raise ValueError(
                    f"unsubstituted placeholder or elision {bad!r} in {part!r}: only the message "
                    "and --detail/--suggested-fix values are free text; add the placeholder to "
                    "the table in scripts/check-skill-examples.py or make the example concrete")
            argv.append(part)
        i += 1
    return argv, sub, stdin


def prepare_json(ex, fx, index, keys):
    """-> (parsed object, record line or None) or raises ValueError."""
    thread = fx["open"][index % len(fx["open"])]
    text = substitute(ex.text, placeholder_table(fx, thread))
    try:
        obj = json.loads(text)
    except json.JSONDecodeError as e:
        raise ValueError(f"not valid JSON ({e.msg} at column {e.colno})")
    if not isinstance(obj, dict):
        raise ValueError("not a JSON object")
    if ex.kind == "inline-json" and not {"kind", "location", "references", "supersedes"} & set(obj):
        return obj, None  # e.g. a subagent's final-message shape
    unknown = sorted(set(obj) - keys)
    if unknown:
        raise ValueError(f"keys {unknown} are not overrides-line keys per `record --help` "
                         f"(the CLI would silently ignore them); known: {sorted(keys)}")
    for key, value in obj.items():
        values = value if isinstance(value, list) else [value]
        for v in values:
            if not isinstance(v, str):
                continue
            left = PLACEHOLDER_RE.search(v)
            if left and key in ("message", "detail", "suggested_fix"):
                continue
            if left or "…" in v:
                raise ValueError(f"unsubstituted placeholder in {key!r}: {v!r}")
    for key in ("message", "detail", "suggested_fix"):
        if isinstance(obj.get(key), str):
            obj[key] = PLACEHOLDER_RE.sub(SAMPLE_TEXT, obj[key])
    return obj, json.dumps(obj)


def main():
    verbose = "-v" in sys.argv[1:]
    qbin = os.environ.get("QUALIFIER_BIN", "")
    if qbin:
        qbin = os.path.abspath(qbin)
        if not (os.path.isfile(qbin) and os.access(qbin, os.X_OK)):
            print(f"FAIL: QUALIFIER_BIN={qbin} is not an executable file", file=sys.stderr)
            return 1
    else:
        for cand in ("target/debug/qualifier", "target/release/qualifier"):
            path = os.path.join(REPO, cand)
            if os.path.isfile(path) and os.access(path, os.X_OK):
                qbin = path
                break
    if not qbin:
        print("skip: no qualifier binary for skill examples "
              "(set QUALIFIER_BIN or run `cargo build --bin qualifier`)")
        return 2

    errors = []
    paths = sorted(
        os.path.join(d, f) for d, _, fs in os.walk(SKILLS) for f in fs if f.endswith(".md")
    )
    examples = [ex for p in paths for ex in extract(p, errors)]

    root = tempfile.mkdtemp(prefix="skill-examples.")
    try:
        home = os.path.join(root, "home")
        scratch = os.path.join(root, "scratch")
        os.makedirs(home)
        os.makedirs(scratch)
        env = child_env(home)
        version = run([qbin, "--version"], REPO, env).stdout.strip()
        keys = batch_keys(qbin, env)
        grammar = command_grammar(qbin, env)
        repo, opened, sha = build_fixture(qbin, root, env)
        fx = {"bin": qbin, "scratch": scratch, "open": opened, "sha": sha, "grammar": grammar}
        before = qual_digest(repo)
        copies = [0]

        def copy():
            copies[0] += 1
            dest = os.path.join(root, f"copy-{copies[0]}")
            shutil.copytree(repo, dest, symlinks=True)
            return dest

        def outcome(ex, r, argv):
            failed = r.returncode != 0
            if failed == (ex.mode == "expect-fail"):
                return True
            want = "a non-zero exit (marked expect-fail)" if ex.mode == "expect-fail" else "exit 0"
            shown = " ".join(shlex.quote(a) for a in argv)
            out = (r.stderr.strip() or r.stdout.strip())[-800:]
            errors.append(f"{ex.where}: expected {want}, got {r.returncode}\n"
                          f"    example: {ex.text}\n    ran:     {shown}\n    output:  {out}")
            return False

        skipped = [ex for ex in examples if ex.mode == "skip"]
        live = [ex for ex in examples if ex.mode != "skip"]

        # Record lines, one batch per file.
        batches, record_count, other_json = {}, 0, 0
        for path in paths:
            lines = []  # (example, line)
            index = 0
            for ex in (e for e in live if e.path == path and e.kind != "cmd"):
                try:
                    _obj, line = prepare_json(ex, fx, index, keys)
                except ValueError as e:
                    errors.append(f"{ex.where}: {e}\n    example: {ex.text}")
                    continue
                if line is None:
                    other_json += 1
                    continue
                index += 1
                lines.append((ex, line))
            batch = os.path.join(scratch, f"batch-{len(batches)}.jsonl")
            good = [line for ex, line in lines if ex.mode != "expect-fail"]
            if not good:
                # A redirect in a file without record lines gets one reply.
                t = fx["open"][0]
                good = [json.dumps({"kind": "comment", "location": t["subject"],
                                    "references": t["root"], "message": SAMPLE_TEXT})]
            with open(batch, "w") as fh:
                fh.write("\n".join(good) + "\n")
            batches[path] = batch
            real = [(ex, line) for ex, line in lines if ex.mode != "expect-fail"]
            if real:
                record_count += len(real)
                if verbose:
                    for ex, line in real:
                        print(f"line: {ex.where}: {line}")
                argv = [qbin, "record", "--stdin", "--dry-run"]
                r = run(argv, copy(), env, stdin=batch)
                if r.returncode != 0:
                    bad = [int(n) for n in re.findall(r"stdin line (\d+):", r.stderr)]
                    named = [real[n - 1][0].where for n in bad if 0 < n <= len(real)]
                    errors.append(
                        f"{', '.join(named) or os.path.relpath(path, REPO)}: record lines rejected "
                        f"by `record --stdin --dry-run`:\n    {r.stderr.strip()[-1200:]}")
            for ex, line in lines:
                if ex.mode == "expect-fail":
                    one = os.path.join(scratch, f"expect-fail-{ex.line}.jsonl")
                    with open(one, "w") as fh:
                        fh.write(line + "\n")
                    record_count += 1
                    outcome(ex, run([qbin, "record", "--stdin", "--dry-run"], copy(), env, stdin=one),
                            ["record", "--stdin", "--dry-run", "<", one])

        # Commands.
        cmd_count = 0
        for ex in (e for e in live if e.kind == "cmd"):
            try:
                argv, sub, uses_stdin = prepare_command(ex, fx)
            except ValueError as e:
                errors.append(f"{ex.where}: {e}\n    example: {ex.text}")
                continue
            cwd = repo if sub in READ_COMMANDS else copy()
            cmd_count += 1
            if verbose:
                print(f"run: {ex.where}: {' '.join(shlex.quote(a) for a in argv[1:])}"
                      + (" < batch" if uses_stdin else ""))
            outcome(ex, run(argv, cwd, env, stdin=batches[ex.path] if uses_stdin else None), argv)

        if qual_digest(repo) != before:
            errors.append("a read command changed the fixture's .qual files")

        for ex in skipped:
            print(f"skip: {ex.where}: {ex.reason}")
        if errors:
            for e in errors:
                print(f"FAIL: {e}", file=sys.stderr)
            return 1
        print(f"skill examples: {cmd_count} commands and {record_count} record lines "
              f"({other_json} other JSON objects parsed) across {len(paths)} files pass "
              f"against {version}; {len(skipped)} skipped")
        return 0
    finally:
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
