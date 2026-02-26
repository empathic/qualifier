// Qualifier Playground — interactive terminal for the qualifier site
// Adapted from the toolpath playground for the qualifier project.
// Mock WASM layer with real score computation via QualifierCore.

(function () {
  "use strict";

  // ---------------------------------------------------------------
  // 1. VirtualFS
  // ---------------------------------------------------------------

  function VirtualFS(fileMap) {
    this._files = {};
    if (fileMap) {
      for (var k in fileMap) {
        if (fileMap.hasOwnProperty(k)) {
          this._files[k] = fileMap[k];
        }
      }
    }
  }

  VirtualFS.prototype.list = function () {
    var names = [];
    for (var k in this._files) {
      if (this._files.hasOwnProperty(k)) names.push(k);
    }
    names.sort();
    return names;
  };

  VirtualFS.prototype.get = function (name) {
    return this._files.hasOwnProperty(name) ? this._files[name] : null;
  };

  VirtualFS.prototype.has = function (name) {
    return this._files.hasOwnProperty(name);
  };

  VirtualFS.prototype.size = function (name) {
    var content = this.get(name);
    if (content === null) return 0;
    return new Blob([content]).size;
  };

  VirtualFS.prototype.formatSize = function (bytes) {
    if (bytes < 1024) return bytes + " B";
    if (bytes < 1048576) return (bytes / 1024).toFixed(1) + " KB";
    return (bytes / 1048576).toFixed(1) + " MB";
  };

  // ---------------------------------------------------------------
  // 2. Command Parser
  // ---------------------------------------------------------------

  function parseCommand(line) {
    var args = [];
    var current = "";
    var inSingle = false;
    var inDouble = false;
    var escape = false;

    for (var i = 0; i < line.length; i++) {
      var ch = line[i];

      if (escape) {
        current += ch;
        escape = false;
        continue;
      }

      if (ch === "\\") {
        escape = true;
        continue;
      }

      if (ch === "'" && !inDouble) {
        inSingle = !inSingle;
        continue;
      }

      if (ch === '"' && !inSingle) {
        inDouble = !inDouble;
        continue;
      }

      if ((ch === " " || ch === "\t") && !inSingle && !inDouble) {
        if (current.length > 0) {
          args.push(current);
          current = "";
        }
        continue;
      }

      current += ch;
    }

    if (current.length > 0) args.push(current);
    return args;
  }

  // ---------------------------------------------------------------
  // 3. ANSI helpers
  // ---------------------------------------------------------------

  var ANSI = {
    reset: "\x1b[0m",
    bold: "\x1b[1m",
    dim: "\x1b[2m",
    accent: "\x1b[34m",
    red: "\x1b[31m",
    green: "\x1b[32m",
    flint: "\x1b[90m",
    white: "\x1b[37m",
    cyan: "\x1b[36m",
    magenta: "\x1b[35m",
  };

  function copperBold(s) {
    return ANSI.accent + ANSI.bold + s + ANSI.reset;
  }

  function red(s) {
    return ANSI.red + s + ANSI.reset;
  }

  function green(s) {
    return ANSI.green + s + ANSI.reset;
  }

  function dim(s) {
    return ANSI.dim + s + ANSI.reset;
  }

  function pencil(s) {
    return ANSI.flint + s + ANSI.reset;
  }

  function cyan(s) {
    return ANSI.cyan + s + ANSI.reset;
  }

  function bold(s) {
    return ANSI.bold + s + ANSI.reset;
  }

  // ---------------------------------------------------------------
  // 4. Mock WASM layer — uses QualifierCore for real computation
  // ---------------------------------------------------------------

  function loadAllAttestations(fs) {
    var QC = window.QualifierCore;
    var all = [];
    var files = fs.list();
    for (var i = 0; i < files.length; i++) {
      var f = files[i];
      if (f.indexOf(".qual") !== -1) {
        var content = fs.get(f);
        if (content && QC) {
          var parsed = QC.parseQualFile(content);
          all = all.concat(parsed);
        }
      }
    }
    return all;
  }

  function loadGraph(fs) {
    var QC = window.QualifierCore;
    var files = fs.list();
    for (var i = 0; i < files.length; i++) {
      var f = files[i];
      if (f.indexOf(".graph.jsonl") !== -1) {
        var content = fs.get(f);
        if (content && QC) {
          return QC.parseGraph(content);
        }
      }
    }
    return [];
  }

  function computeScores(fs) {
    var QC = window.QualifierCore;
    if (!QC) return null;
    var attestations = loadAllAttestations(fs);
    var graphEntries = loadGraph(fs);
    return QC.effectiveScores(graphEntries, attestations);
  }

  function pad(s, width, right) {
    s = String(s);
    while (s.length < width) {
      if (right) {
        s = s + " ";
      } else {
        s = " " + s;
      }
    }
    return s;
  }

  function statusColor(status, text) {
    if (status === "blocker" || status === "concern") return red(text);
    if (status === "healthy") return green(text);
    return text;
  }

  function barColor(effective, bar) {
    if (effective <= -30) return red(bar);
    if (effective < 0) return red(bar);
    if (effective >= 50) return green(bar);
    return ANSI.accent + bar + ANSI.reset;
  }

  // qualifier score
  function cmdScore(fs) {
    var QC = window.QualifierCore;
    if (!QC) return { output: red("error: QualifierCore not loaded") };

    var scores = computeScores(fs);
    if (!scores) return { output: red("error: no data found") };

    var lines = [];
    lines.push(
      "  " +
        dim("ARTIFACT") +
        pad("", 14) +
        dim(pad("RAW", 6)) +
        dim(pad("EFF", 6)) +
        "   " +
        dim("STATUS")
    );

    // Sort artifacts: blockers first (lowest effective), then by name
    var arts = [];
    for (var art in scores) {
      if (scores.hasOwnProperty(art)) arts.push(art);
    }
    arts.sort(function (a, b) {
      var ea = scores[a].effective;
      var eb = scores[b].effective;
      if (ea !== eb) return ea - eb;
      return a < b ? -1 : a > b ? 1 : 0;
    });

    for (var i = 0; i < arts.length; i++) {
      var name = arts[i];
      var s = scores[name];
      var status = QC.scoreStatus(s.effective);
      var bar = QC.scoreBar(s.effective, 10);

      var line =
        "  " +
        pad(name, 22, true) +
        pad(String(s.raw), 6) +
        pad(String(s.effective), 6) +
        "   " +
        barColor(s.effective, bar) +
        "  " +
        statusColor(status, status);

      lines.push(line);
    }

    return { output: lines.join("\r\n") };
  }

  // qualifier show <artifact>
  function cmdShow(args, fs) {
    var QC = window.QualifierCore;
    if (!QC) return { output: red("error: QualifierCore not loaded") };

    if (args.length < 1) {
      return { output: red("error: ") + "usage: qualifier show <artifact>" };
    }

    var artifact = args[0];
    var scores = computeScores(fs);
    if (!scores || !scores[artifact]) {
      return { output: red("error: ") + "unknown artifact: " + artifact };
    }

    var s = scores[artifact];
    var status = QC.scoreStatus(s.effective);
    var lines = [];

    lines.push("");
    lines.push("  " + copperBold(artifact));
    lines.push(
      "  Raw score:       " +
        (s.raw >= 0 ? bold(String(s.raw)) : red(String(s.raw)))
    );
    lines.push(
      "  Effective score: " +
        (s.effective >= 0
          ? bold(String(s.effective))
          : red(String(s.effective)))
    );

    if (s.limitingPath && s.limitingPath.length > 0) {
      lines.push(
        "  " + red("Limited by: ") + dim(s.limitingPath.join(" -> "))
      );
    }

    lines.push("");

    var atts = s.attestations || [];
    if (atts.length === 0) {
      lines.push("  " + dim("No attestations."));
    } else {
      lines.push("  Attestations (" + atts.length + "):");
      for (var i = 0; i < atts.length; i++) {
        var a = atts[i];
        var sign = a.score >= 0 ? "+" : "";
        var scoreStr = "[" + sign + a.score + "]";
        scoreStr = a.score >= 0 ? green(pad(scoreStr, 5)) : red(pad(scoreStr, 5));

        var kindStr = pad(a.kind, 11, true);
        var summaryStr = '"' + a.summary + '"';
        var authorStr = (a.author || "").replace(/@.*/, "");
        var dateStr = (a.created_at || "").substring(0, 10);

        lines.push(
          "    " +
            scoreStr +
            " " +
            kindStr +
            " " +
            dim(summaryStr) +
            "  " +
            pencil(pad(authorStr, 6, true)) +
            " " +
            pencil(dateStr)
        );
      }
    }

    lines.push("");
    return { output: lines.join("\r\n") };
  }

  // qualifier check [--min-score N]
  function cmdCheck(args, fs) {
    var QC = window.QualifierCore;
    if (!QC) return { output: red("error: QualifierCore not loaded") };

    var threshold = 0;
    for (var i = 0; i < args.length; i++) {
      if (args[i] === "--min-score" && i + 1 < args.length) {
        threshold = parseInt(args[i + 1], 10);
        if (isNaN(threshold)) threshold = 0;
      }
    }

    var scores = computeScores(fs);
    if (!scores) return { output: red("error: no data found") };

    var failures = [];
    var passes = [];
    for (var art in scores) {
      if (!scores.hasOwnProperty(art)) continue;
      if (scores[art].effective < threshold) {
        failures.push(art);
      } else {
        passes.push(art);
      }
    }

    var lines = [];
    failures.sort();
    passes.sort();

    for (var f = 0; f < failures.length; f++) {
      var s = scores[failures[f]];
      lines.push(
        "  " +
          red("FAIL") +
          "  " +
          pad(failures[f], 22, true) +
          "eff: " +
          red(String(s.effective)) +
          "  " +
          dim("(threshold: " + threshold + ")")
      );
    }

    for (var p = 0; p < passes.length; p++) {
      var sp = scores[passes[p]];
      lines.push(
        "  " +
          green("PASS") +
          "  " +
          pad(passes[p], 22, true) +
          "eff: " +
          green(String(sp.effective)) +
          "  " +
          dim("(threshold: " + threshold + ")")
      );
    }

    lines.push("");
    if (failures.length > 0) {
      lines.push(
        red("check failed: ") +
          failures.length +
          " artifact" +
          (failures.length !== 1 ? "s" : "") +
          " below threshold " +
          threshold
      );
    } else {
      lines.push(
        green("check passed: ") +
          "all artifacts meet threshold " +
          threshold
      );
    }

    return { output: lines.join("\r\n") };
  }

  // qualifier ls [--below N]
  function cmdLs(args, fs) {
    var QC = window.QualifierCore;
    if (!QC) return { output: red("error: QualifierCore not loaded") };

    var below = null;
    for (var i = 0; i < args.length; i++) {
      if (args[i] === "--below" && i + 1 < args.length) {
        below = parseInt(args[i + 1], 10);
        if (isNaN(below)) below = null;
      }
    }

    var scores = computeScores(fs);
    if (!scores) return { output: red("error: no data found") };

    var arts = [];
    for (var art in scores) {
      if (!scores.hasOwnProperty(art)) continue;
      if (below !== null && scores[art].effective >= below) continue;
      arts.push(art);
    }

    arts.sort(function (a, b) {
      var ea = scores[a].effective;
      var eb = scores[b].effective;
      if (ea !== eb) return ea - eb;
      return a < b ? -1 : a > b ? 1 : 0;
    });

    var lines = [];
    for (var j = 0; j < arts.length; j++) {
      var name = arts[j];
      var s = scores[name];
      var status = QC.scoreStatus(s.effective);
      lines.push(
        "  " +
          pad(name, 22, true) +
          pad(String(s.effective), 6) +
          "  " +
          statusColor(status, status)
      );
    }

    if (lines.length === 0) {
      lines.push(dim("  (no matching artifacts)"));
    }

    return { output: lines.join("\r\n") };
  }

  // qualifier attest
  function cmdAttest() {
    return {
      output:
        dim("qualifier attest <artifact> [options]") +
        "\r\n\r\n" +
        "  Records a quality attestation for an artifact.\r\n" +
        "  " +
        pencil("(playground is read-only -- cannot write .qual files)") +
        "\r\n\r\n" +
        "  Options:\r\n" +
        "    --kind <kind>          blocker|concern|praise|pass|fail|suggestion|waiver\r\n" +
        "    --score <n>            Score delta (-100..100)\r\n" +
        "    --summary <text>       One-line description\r\n" +
        "    --suggested-fix <text> Actionable suggestion\r\n" +
        "    --tag <tag>            Classification tag (repeatable)\r\n" +
        "    --author <email>       Attestation author",
    };
  }

  // qualifier init
  function cmdInit() {
    return {
      output:
        "\r\n" +
        "  Created " +
        copperBold("qualifier.graph.jsonl") +
        " " +
        dim("(empty -- populate with your dependency graph)") +
        "\r\n" +
        "  Detected VCS: " +
        bold("git") +
        "\r\n" +
        "  Added " +
        copperBold("*.qual merge=union") +
        " to .gitattributes\r\n",
    };
  }

  // qualifier --help
  function cmdHelp() {
    var lines = [];
    lines.push("");
    lines.push(copperBold("qualifier") + " -- quality attestation toolkit");
    lines.push("");
    lines.push(bold("USAGE:"));
    lines.push("    qualifier <COMMAND> [OPTIONS]");
    lines.push("");
    lines.push(bold("COMMANDS:"));
    lines.push(
      "    " +
        copperBold(pad("score", 12, true)) +
        "Compute and display scores for all artifacts"
    );
    lines.push(
      "    " +
        copperBold(pad("show", 12, true)) +
        "Show attestations and scores for an artifact"
    );
    lines.push(
      "    " +
        copperBold(pad("check", 12, true)) +
        "CI gate: exit non-zero if any score below threshold"
    );
    lines.push(
      "    " +
        copperBold(pad("ls", 12, true)) +
        "List artifacts by score"
    );
    lines.push(
      "    " +
        copperBold(pad("attest", 12, true)) +
        "Add an attestation to an artifact"
    );
    lines.push(
      "    " +
        copperBold(pad("compact", 12, true)) +
        "Compact a .qual file (prune superseded)"
    );
    lines.push(
      "    " +
        copperBold(pad("graph", 12, true)) +
        "Visualize the dependency graph"
    );
    lines.push(
      "    " +
        copperBold(pad("init", 12, true)) +
        "Initialize qualifier in a repo"
    );
    lines.push(
      "    " +
        copperBold(pad("blame", 12, true)) +
        "Per-line VCS attribution"
    );
    lines.push("");
    lines.push(bold("OPTIONS:"));
    lines.push("    -h, --help       Print help");
    lines.push("    -V, --version    Print version");
    lines.push("    --format <FMT>   Output format (text, json)");
    lines.push("");
    return { output: lines.join("\r\n") };
  }

  // Route qualifier subcommands
  function dispatchQualifier(args, fs) {
    if (args.length === 0) return cmdHelp();

    var sub = args[0];
    var rest = args.slice(1);

    switch (sub) {
      case "score":
        return cmdScore(fs);
      case "show":
        return cmdShow(rest, fs);
      case "check":
        return cmdCheck(rest, fs);
      case "ls":
        return cmdLs(rest, fs);
      case "attest":
        return cmdAttest();
      case "init":
        return cmdInit();
      case "--help":
      case "-h":
        return cmdHelp();
      case "--version":
      case "-V":
        return { output: "qualifier 0.1.1" };
      default:
        return {
          output:
            red("error: ") +
            "unrecognized command '" +
            sub +
            "'\r\n" +
            dim("Run 'qualifier --help' for usage."),
        };
    }
  }

  // ---------------------------------------------------------------
  // 5. Shell builtins
  // ---------------------------------------------------------------

  function builtinLs(args, fs) {
    var files = fs.list();
    var lines = [];
    for (var i = 0; i < files.length; i++) {
      var sz = fs.formatSize(fs.size(files[i]));
      lines.push("  " + pad(sz, 8) + "  " + files[i]);
    }
    return { output: lines.join("\r\n") };
  }

  function builtinCat(args, fs) {
    if (args.length < 1) {
      return { output: red("usage: ") + "cat <file>" };
    }
    var name = args[0];
    if (!fs.has(name)) {
      return { output: red("cat: ") + name + ": No such file" };
    }
    var content = fs.get(name);
    // Replace \n with \r\n for terminal display
    return { output: content.replace(/\n/g, "\r\n") };
  }

  function builtinHelp(fs) {
    var files = fs.list();
    var fileList = files.join(", ");

    var lines = [];
    lines.push("");
    lines.push(
      copperBold("QUALIFIER PLAYGROUND") +
        " " +
        dim("-- mock CLI (wasm build pending)")
    );
    lines.push("");
    lines.push(bold("Shell builtins:"));
    lines.push(
      "  " + copperBold(pad("ls", 38, true)) + "List files"
    );
    lines.push(
      "  " +
        copperBold(pad("cat <file>", 38, true)) +
        "Display file contents"
    );
    lines.push(
      "  " + copperBold(pad("clear", 38, true)) + "Clear terminal"
    );
    lines.push(
      "  " + copperBold(pad("help", 38, true)) + "Show this message"
    );
    lines.push("");
    lines.push(bold("CLI commands:"));
    lines.push(
      "  " +
        copperBold(pad("qualifier score", 38, true)) +
        "Compute and display all scores"
    );
    lines.push(
      "  " +
        copperBold(pad("qualifier show <artifact>", 38, true)) +
        "Show attestations for an artifact"
    );
    lines.push(
      "  " +
        copperBold(pad("qualifier check [--min-score N]", 38, true)) +
        "CI gate check"
    );
    lines.push(
      "  " +
        copperBold(pad("qualifier ls [--below N]", 38, true)) +
        "List artifacts by score"
    );
    lines.push(
      "  " +
        copperBold(pad("qualifier init", 38, true)) +
        "Initialize qualifier in a repo"
    );
    lines.push(
      "  " +
        copperBold(pad("qualifier --help", 38, true)) +
        "Full CLI usage"
    );
    lines.push("");
    lines.push(dim("Files: ") + fileList);
    lines.push("");
    return { output: lines.join("\r\n") };
  }

  // ---------------------------------------------------------------
  // 6. Command dispatcher
  // ---------------------------------------------------------------

  function dispatch(line, fs) {
    var trimmed = line.trim();
    if (!trimmed) return { output: "" };

    var args = parseCommand(trimmed);
    if (args.length === 0) return { output: "" };

    var cmd = args[0];

    // Shell builtins
    switch (cmd) {
      case "ls":
        return builtinLs(args.slice(1), fs);
      case "cat":
        return builtinCat(args.slice(1), fs);
      case "clear":
        return { clear: true };
      case "help":
        return builtinHelp(fs);
    }

    // qualifier commands
    if (cmd === "qualifier") {
      return dispatchQualifier(args.slice(1), fs);
    }

    // cargo install qualifier -- special case for boot
    if (cmd === "cargo" && args[1] === "install" && args[2] === "qualifier") {
      return {
        output:
          dim("  qualifier is already installed.") +
          "\r\n" +
          "  " +
          pencil("(mock environment -- wasm build pending)"),
      };
    }

    return {
      output:
        red("command not found: ") +
        cmd +
        "\r\n" +
        dim("Type 'help' for available commands."),
    };
  }

  // ---------------------------------------------------------------
  // 7. Word boundary helpers
  // ---------------------------------------------------------------

  function wordBoundaryRight(buf, pos) {
    var i = pos;
    // skip non-word chars
    while (i < buf.length && /\s/.test(buf[i])) i++;
    // skip word chars
    while (i < buf.length && !/\s/.test(buf[i])) i++;
    return i;
  }

  function wordBoundaryLeft(buf, pos) {
    var i = pos;
    // skip spaces to the left
    while (i > 0 && /\s/.test(buf[i - 1])) i--;
    // skip word chars to the left
    while (i > 0 && !/\s/.test(buf[i - 1])) i--;
    return i;
  }

  function wordEndRight(buf, pos) {
    var i = pos;
    if (i < buf.length) i++;
    // skip spaces
    while (i < buf.length && /\s/.test(buf[i])) i++;
    // skip word chars
    while (i < buf.length && !/\s/.test(buf[i])) i++;
    if (i > pos && i <= buf.length) i--;
    return i;
  }

  function WORDBoundaryRight(buf, pos) {
    var i = pos;
    while (i < buf.length && buf[i] !== " ") i++;
    while (i < buf.length && buf[i] === " ") i++;
    return i;
  }

  function WORDBoundaryLeft(buf, pos) {
    var i = pos;
    while (i > 0 && buf[i - 1] === " ") i--;
    while (i > 0 && buf[i - 1] !== " ") i--;
    return i;
  }

  function WORDEndRight(buf, pos) {
    var i = pos;
    if (i < buf.length) i++;
    while (i < buf.length && buf[i] === " ") i++;
    while (i < buf.length && buf[i] !== " ") i++;
    if (i > pos && i <= buf.length) i--;
    return i;
  }

  // ---------------------------------------------------------------
  // 8. TermShell class
  // ---------------------------------------------------------------

  function TermShell(container, fs) {
    this.fs = fs;
    this.history = [];
    this.historyIndex = -1;
    this.savedLine = "";
    this.buffer = "";
    this.cursor = 0;
    this.mode = "vi"; // default: vi
    this.viMode = "insert"; // insert or command
    this.killRing = "";
    this.viOperator = null;
    this.pinnedText = "";

    var self = this;

    // --- xterm terminal ---
    this.term = new Terminal({
      cursorBlink: true,
      cursorStyle: "block",
      fontSize: 14,
      fontFamily: '"JetBrains Mono", monospace',
      rows: 20,
      theme: {
        background: "#141720",
        foreground: "#d0d5e3",
        cursor: "#818cf8",
        cursorAccent: "#141720",
        selectionBackground: "#818cf844",
        selectionForeground: "#eef0f6",
        black: "#0c0e11",
        red: "#f87171",
        green: "#34d399",
        yellow: "#fbbf24",
        blue: "#60a5fa",
        magenta: "#c084fc",
        cyan: "#22d3ee",
        white: "#d0d5e3",
        brightBlack: "#6b7394",
        brightRed: "#fca5a5",
        brightGreen: "#6ee7b7",
        brightYellow: "#fcd34d",
        brightBlue: "#93c5fd",
        brightMagenta: "#d8b4fe",
        brightCyan: "#67e8f9",
        brightWhite: "#eef0f6",
      },
      allowTransparency: false,
      scrollback: 1000,
    });

    // --- FitAddon ---
    this.fitAddon = new FitAddon.FitAddon();
    this.term.loadAddon(this.fitAddon);

    this.term.open(container);
    this.fitAddon.fit();

    // Refit on resize
    this._resizeHandler = function () {
      self.fitAddon.fit();
    };
    window.addEventListener("resize", this._resizeHandler);

    // --- Mode toggle button ---
    this.modeBtn = document.createElement("button");
    this.modeBtn.className = "playground-mode-toggle vi";
    this.modeBtn.textContent = "VI";
    this.modeBtn.addEventListener("click", function () {
      self.setMode(self.mode === "vi" ? "emacs" : "vi");
    });
    container.appendChild(this.modeBtn);

    // --- Pinned command header ---
    this.pinnedEl = document.createElement("div");
    this.pinnedEl.className = "playground-pinned-cmd";
    this.pinnedEl.style.display = "none";
    this.pinnedEl.innerHTML =
      '<span class="pinned-prompt">qual</span>' +
      '<span class="pinned-dollar"> $ </span>' +
      '<span class="pinned-text"></span>';
    container.appendChild(this.pinnedEl);

    // --- Key handler ---
    this.term.onKey(function (e) {
      self.handleKey(e.key, e.domEvent);
    });

    // --- Paste handler ---
    this.term.onData(function (data) {
      // onData fires for paste events (multi-char data) and
      // other input sources. We handle paste only when length > 1
      // to avoid double-handling single keypresses.
      if (data.length > 1 && data.indexOf("\x1b") === -1) {
        // Switch to insert mode if in vi command mode
        if (self.mode === "vi" && self.viMode === "command") {
          self.viEnterInsert();
        }
        for (var i = 0; i < data.length; i++) {
          var ch = data[i];
          if (ch === "\r" || ch === "\n") {
            self.submit();
            return;
          }
          self.insertChar(ch);
        }
      }
    });
  }

  TermShell.prototype.setMode = function (mode) {
    this.mode = mode;
    if (mode === "vi") {
      this.modeBtn.textContent = "VI";
      this.modeBtn.classList.add("vi");
      this.viMode = "insert";
    } else {
      this.modeBtn.textContent = "EMACS";
      this.modeBtn.classList.remove("vi");
    }
  };

  TermShell.prototype.viEnterInsert = function () {
    this.viMode = "insert";
    this.viOperator = null;
    this.term.options.cursorStyle = "block";
  };

  TermShell.prototype.viEnterCommand = function () {
    this.viMode = "command";
    this.viOperator = null;
    this.term.options.cursorStyle = "underline";
    // In vi, entering command mode moves cursor back one if possible
    if (this.cursor > 0 && this.cursor >= this.buffer.length) {
      this.cursor = this.buffer.length - 1;
      this.refreshLine();
    }
  };

  TermShell.prototype.killRange = function (from, to) {
    if (from === to) return;
    var start = Math.min(from, to);
    var end = Math.max(from, to);
    this.killRing = this.buffer.substring(start, end);
    this.buffer = this.buffer.substring(0, start) + this.buffer.substring(end);
    this.cursor = start;
    this.refreshLine();
  };

  TermShell.prototype.prompt = function () {
    this.buffer = "";
    this.cursor = 0;
    this.historyIndex = -1;
    this.savedLine = "";
    if (this.mode === "vi") {
      this.viMode = "insert";
      this.viOperator = null;
      this.term.options.cursorStyle = "block";
    }
    this.term.write(copperBold("qual") + " " + pencil("$") + " ");
  };

  TermShell.prototype.refreshLine = function () {
    // Clear current line after prompt and rewrite
    var promptLen = 6; // "qual $ " = 6 visible chars
    this.term.write("\r");
    this.term.write(copperBold("qual") + " " + pencil("$") + " ");
    this.term.write(this.buffer);
    // Clear from cursor to end of line
    this.term.write("\x1b[K");
    // Move cursor to correct position
    var cursorOffset = this.buffer.length - this.cursor;
    if (cursorOffset > 0) {
      this.term.write("\x1b[" + cursorOffset + "D");
    }
  };

  TermShell.prototype.insertChar = function (ch) {
    this.buffer =
      this.buffer.substring(0, this.cursor) +
      ch +
      this.buffer.substring(this.cursor);
    this.cursor++;
    this.refreshLine();
  };

  TermShell.prototype.pinCommand = function (cmd) {
    this.pinnedText = cmd;
    this.updatePinned();
  };

  TermShell.prototype.updatePinned = function () {
    if (!this.pinnedText) {
      this.pinnedEl.style.display = "none";
      return;
    }

    var viewport = this.term.element
      ? this.term.element.querySelector(".xterm-viewport")
      : null;

    // Show pinned header if scrolled past the original command
    if (viewport && viewport.scrollTop > 30) {
      this.pinnedEl.style.display = "";
      this.pinnedEl.querySelector(".pinned-text").textContent =
        this.pinnedText;
    } else {
      this.pinnedEl.style.display = "none";
    }
  };

  TermShell.prototype.submit = function () {
    var line = this.buffer;
    this.term.write("\r\n");

    if (line.trim()) {
      this.history.unshift(line);
      if (this.history.length > 200) this.history.pop();
    }

    this.historyIndex = -1;
    this.savedLine = "";

    // Pin command
    this.pinCommand(line.trim());

    var result = dispatch(line, this.fs);

    if (result.clear) {
      this.term.clear();
      this.pinnedText = "";
      this.updatePinned();
      this.prompt();
      return;
    }

    if (result.output) {
      this.term.write(result.output);
      this.term.write("\r\n");
    }

    this.prompt();

    // Set up scroll watcher for pinned header
    var self = this;
    var viewport = this.term.element
      ? this.term.element.querySelector(".xterm-viewport")
      : null;
    if (viewport) {
      viewport.addEventListener("scroll", function () {
        self.updatePinned();
      });
    }
  };

  TermShell.prototype.historyPrev = function () {
    if (this.history.length === 0) return;
    if (this.historyIndex === -1) {
      this.savedLine = this.buffer;
    }
    if (this.historyIndex < this.history.length - 1) {
      this.historyIndex++;
      this.buffer = this.history[this.historyIndex];
      this.cursor = this.buffer.length;
      this.refreshLine();
    }
  };

  TermShell.prototype.historyNext = function () {
    if (this.historyIndex === -1) return;
    this.historyIndex--;
    if (this.historyIndex === -1) {
      this.buffer = this.savedLine;
    } else {
      this.buffer = this.history[this.historyIndex];
    }
    this.cursor = this.buffer.length;
    this.refreshLine();
  };

  // --- Key dispatch ---
  TermShell.prototype.handleKey = function (key, domEvent) {
    if (this.mode === "emacs") {
      this.handleEmacs(key, domEvent, domEvent.code);
    } else {
      this.handleVi(key, domEvent, domEvent.code);
    }
  };

  // --- Emacs bindings ---
  TermShell.prototype.handleEmacs = function (key, domEvent, code) {
    var ctrl = domEvent.ctrlKey;
    var alt = domEvent.altKey || domEvent.metaKey;

    // Enter
    if (key === "\r") {
      this.submit();
      return;
    }

    // Ctrl+C
    if (ctrl && (code === "KeyC" || key === "\x03")) {
      this.term.write("^C\r\n");
      this.prompt();
      return;
    }

    // Ctrl+A — beginning of line
    if (ctrl && code === "KeyA") {
      this.cursor = 0;
      this.refreshLine();
      return;
    }

    // Ctrl+E — end of line
    if (ctrl && code === "KeyE") {
      this.cursor = this.buffer.length;
      this.refreshLine();
      return;
    }

    // Ctrl+B — back one char
    if (ctrl && code === "KeyB") {
      if (this.cursor > 0) {
        this.cursor--;
        this.refreshLine();
      }
      return;
    }

    // Ctrl+F — forward one char
    if (ctrl && code === "KeyF") {
      if (this.cursor < this.buffer.length) {
        this.cursor++;
        this.refreshLine();
      }
      return;
    }

    // Ctrl+D — delete char under cursor (or EOF if empty)
    if (ctrl && code === "KeyD") {
      if (this.buffer.length === 0) return;
      if (this.cursor < this.buffer.length) {
        this.buffer =
          this.buffer.substring(0, this.cursor) +
          this.buffer.substring(this.cursor + 1);
        this.refreshLine();
      }
      return;
    }

    // Ctrl+H — backspace
    if (ctrl && code === "KeyH") {
      if (this.cursor > 0) {
        this.buffer =
          this.buffer.substring(0, this.cursor - 1) +
          this.buffer.substring(this.cursor);
        this.cursor--;
        this.refreshLine();
      }
      return;
    }

    // Ctrl+K — kill to end of line
    if (ctrl && code === "KeyK") {
      this.killRing = this.buffer.substring(this.cursor);
      this.buffer = this.buffer.substring(0, this.cursor);
      this.refreshLine();
      return;
    }

    // Ctrl+U — kill to beginning of line
    if (ctrl && code === "KeyU") {
      this.killRing = this.buffer.substring(0, this.cursor);
      this.buffer = this.buffer.substring(this.cursor);
      this.cursor = 0;
      this.refreshLine();
      return;
    }

    // Ctrl+W — kill word backward
    if (ctrl && code === "KeyW") {
      var wbLeft = wordBoundaryLeft(this.buffer, this.cursor);
      this.killRange(wbLeft, this.cursor);
      return;
    }

    // Ctrl+Y — yank (paste from kill ring)
    if (ctrl && code === "KeyY") {
      if (this.killRing) {
        this.buffer =
          this.buffer.substring(0, this.cursor) +
          this.killRing +
          this.buffer.substring(this.cursor);
        this.cursor += this.killRing.length;
        this.refreshLine();
      }
      return;
    }

    // Ctrl+T — transpose characters
    if (ctrl && code === "KeyT") {
      if (this.cursor > 0 && this.buffer.length >= 2) {
        var pos = this.cursor;
        if (pos === this.buffer.length) pos--;
        if (pos > 0) {
          var chars = this.buffer.split("");
          var tmp = chars[pos - 1];
          chars[pos - 1] = chars[pos];
          chars[pos] = tmp;
          this.buffer = chars.join("");
          this.cursor = pos + 1;
          this.refreshLine();
        }
      }
      return;
    }

    // Ctrl+P — history prev
    if (ctrl && code === "KeyP") {
      this.historyPrev();
      return;
    }

    // Ctrl+N — history next
    if (ctrl && code === "KeyN") {
      this.historyNext();
      return;
    }

    // Alt+B — word backward
    if (alt && code === "KeyB") {
      this.cursor = wordBoundaryLeft(this.buffer, this.cursor);
      this.refreshLine();
      return;
    }

    // Alt+F — word forward
    if (alt && code === "KeyF") {
      this.cursor = wordBoundaryRight(this.buffer, this.cursor);
      this.refreshLine();
      return;
    }

    // Alt+D — kill word forward
    if (alt && code === "KeyD") {
      var wbRight = wordBoundaryRight(this.buffer, this.cursor);
      this.killRange(this.cursor, wbRight);
      return;
    }

    // Arrow keys
    if (code === "ArrowLeft") {
      if (this.cursor > 0) {
        this.cursor--;
        this.refreshLine();
      }
      return;
    }
    if (code === "ArrowRight") {
      if (this.cursor < this.buffer.length) {
        this.cursor++;
        this.refreshLine();
      }
      return;
    }
    if (code === "ArrowUp") {
      this.historyPrev();
      return;
    }
    if (code === "ArrowDown") {
      this.historyNext();
      return;
    }

    // Home / End
    if (code === "Home") {
      this.cursor = 0;
      this.refreshLine();
      return;
    }
    if (code === "End") {
      this.cursor = this.buffer.length;
      this.refreshLine();
      return;
    }

    // Backspace
    if (key === "\x7f" || code === "Backspace") {
      if (this.cursor > 0) {
        this.buffer =
          this.buffer.substring(0, this.cursor - 1) +
          this.buffer.substring(this.cursor);
        this.cursor--;
        this.refreshLine();
      }
      return;
    }

    // Delete
    if (code === "Delete") {
      if (this.cursor < this.buffer.length) {
        this.buffer =
          this.buffer.substring(0, this.cursor) +
          this.buffer.substring(this.cursor + 1);
        this.refreshLine();
      }
      return;
    }

    // Tab — ignore
    if (key === "\t") return;

    // Escape — ignore in emacs mode
    if (key === "\x1b") return;

    // Regular printable characters
    if (key.length === 1 && !ctrl && !alt && key >= " ") {
      this.insertChar(key);
    }
  };

  // --- Vi dispatch ---
  TermShell.prototype.handleVi = function (key, domEvent, code) {
    if (this.viMode === "insert") {
      this.handleViInsert(key, domEvent, code);
    } else {
      this.handleViCommand(key, domEvent, code);
    }
  };

  // --- Vi insert mode ---
  TermShell.prototype.handleViInsert = function (key, domEvent, code) {
    var ctrl = domEvent.ctrlKey;

    // Enter
    if (key === "\r") {
      this.submit();
      return;
    }

    // Escape — enter command mode
    if (key === "\x1b" || code === "Escape") {
      this.viEnterCommand();
      return;
    }

    // Ctrl+C
    if (ctrl && (code === "KeyC" || key === "\x03")) {
      this.term.write("^C\r\n");
      this.prompt();
      return;
    }

    // Ctrl+U — kill line
    if (ctrl && code === "KeyU") {
      this.killRing = this.buffer.substring(0, this.cursor);
      this.buffer = this.buffer.substring(this.cursor);
      this.cursor = 0;
      this.refreshLine();
      return;
    }

    // Ctrl+W — kill word backward
    if (ctrl && code === "KeyW") {
      var wb = wordBoundaryLeft(this.buffer, this.cursor);
      this.killRange(wb, this.cursor);
      return;
    }

    // Ctrl+H — backspace
    if (ctrl && code === "KeyH") {
      if (this.cursor > 0) {
        this.buffer =
          this.buffer.substring(0, this.cursor - 1) +
          this.buffer.substring(this.cursor);
        this.cursor--;
        this.refreshLine();
      }
      return;
    }

    // Arrow keys
    if (code === "ArrowLeft") {
      if (this.cursor > 0) {
        this.cursor--;
        this.refreshLine();
      }
      return;
    }
    if (code === "ArrowRight") {
      if (this.cursor < this.buffer.length) {
        this.cursor++;
        this.refreshLine();
      }
      return;
    }
    if (code === "ArrowUp") {
      this.historyPrev();
      return;
    }
    if (code === "ArrowDown") {
      this.historyNext();
      return;
    }

    // Backspace
    if (key === "\x7f" || code === "Backspace") {
      if (this.cursor > 0) {
        this.buffer =
          this.buffer.substring(0, this.cursor - 1) +
          this.buffer.substring(this.cursor);
        this.cursor--;
        this.refreshLine();
      }
      return;
    }

    // Delete
    if (code === "Delete") {
      if (this.cursor < this.buffer.length) {
        this.buffer =
          this.buffer.substring(0, this.cursor) +
          this.buffer.substring(this.cursor + 1);
        this.refreshLine();
      }
      return;
    }

    // Tab — ignore
    if (key === "\t") return;

    // Regular printable chars
    if (key.length === 1 && !ctrl && key >= " ") {
      this.insertChar(key);
    }
  };

  // --- Vi command mode ---
  TermShell.prototype.handleViCommand = function (key, domEvent, code) {
    var ctrl = domEvent.ctrlKey;

    // If there's a pending operator (d or c), handle the motion
    if (this.viOperator) {
      var op = this.viOperator;
      this.viOperator = null;
      var from = this.cursor;
      var to = from;

      switch (key) {
        case "w":
          to = wordBoundaryRight(this.buffer, from);
          break;
        case "b":
          to = wordBoundaryLeft(this.buffer, from);
          break;
        case "e":
          to = wordEndRight(this.buffer, from);
          if (op === "d" || op === "c") to++; // delete through end
          break;
        case "W":
          to = WORDBoundaryRight(this.buffer, from);
          break;
        case "B":
          to = WORDBoundaryLeft(this.buffer, from);
          break;
        case "E":
          to = WORDEndRight(this.buffer, from);
          if (op === "d" || op === "c") to++;
          break;
        case "0":
          to = 0;
          break;
        case "$":
          to = this.buffer.length;
          break;
        case "^":
          to = 0;
          while (to < this.buffer.length && this.buffer[to] === " ") to++;
          break;
        case "h":
          to = Math.max(0, from - 1);
          break;
        case "l":
          to = Math.min(this.buffer.length, from + 1);
          break;
        case "d":
          // dd — kill whole line
          if (op === "d") {
            this.killRing = this.buffer;
            this.buffer = "";
            this.cursor = 0;
            this.refreshLine();
            return;
          }
          break;
        case "c":
          // cc — change whole line
          if (op === "c") {
            this.killRing = this.buffer;
            this.buffer = "";
            this.cursor = 0;
            this.refreshLine();
            this.viEnterInsert();
            return;
          }
          break;
        default:
          // Unknown motion — cancel
          return;
      }

      this.killRange(Math.min(from, to), Math.max(from, to));
      if (op === "c") {
        this.viEnterInsert();
      }
      return;
    }

    // Ctrl+C
    if (ctrl && (code === "KeyC" || key === "\x03")) {
      this.term.write("^C\r\n");
      this.prompt();
      return;
    }

    // Motions
    switch (key) {
      // h — left
      case "h":
        if (this.cursor > 0) {
          this.cursor--;
          this.refreshLine();
        }
        return;

      // l — right
      case "l":
        if (this.cursor < this.buffer.length - 1) {
          this.cursor++;
          this.refreshLine();
        }
        return;

      // w — word forward
      case "w":
        this.cursor = wordBoundaryRight(this.buffer, this.cursor);
        if (this.cursor > this.buffer.length - 1 && this.buffer.length > 0)
          this.cursor = this.buffer.length - 1;
        this.refreshLine();
        return;

      // b — word backward
      case "b":
        this.cursor = wordBoundaryLeft(this.buffer, this.cursor);
        this.refreshLine();
        return;

      // e — word end
      case "e":
        this.cursor = wordEndRight(this.buffer, this.cursor);
        if (this.cursor > this.buffer.length - 1 && this.buffer.length > 0)
          this.cursor = this.buffer.length - 1;
        this.refreshLine();
        return;

      // W — WORD forward
      case "W":
        this.cursor = WORDBoundaryRight(this.buffer, this.cursor);
        if (this.cursor > this.buffer.length - 1 && this.buffer.length > 0)
          this.cursor = this.buffer.length - 1;
        this.refreshLine();
        return;

      // B — WORD backward
      case "B":
        this.cursor = WORDBoundaryLeft(this.buffer, this.cursor);
        this.refreshLine();
        return;

      // E — WORD end
      case "E":
        this.cursor = WORDEndRight(this.buffer, this.cursor);
        if (this.cursor > this.buffer.length - 1 && this.buffer.length > 0)
          this.cursor = this.buffer.length - 1;
        this.refreshLine();
        return;

      // 0 — beginning of line
      case "0":
        this.cursor = 0;
        this.refreshLine();
        return;

      // ^ — first non-space
      case "^":
        this.cursor = 0;
        while (
          this.cursor < this.buffer.length &&
          this.buffer[this.cursor] === " "
        )
          this.cursor++;
        this.refreshLine();
        return;

      // $ — end of line
      case "$":
        this.cursor = Math.max(0, this.buffer.length - 1);
        this.refreshLine();
        return;

      // i — insert at cursor
      case "i":
        this.viEnterInsert();
        return;

      // a — insert after cursor
      case "a":
        if (this.cursor < this.buffer.length) this.cursor++;
        this.viEnterInsert();
        this.refreshLine();
        return;

      // I — insert at beginning
      case "I":
        this.cursor = 0;
        this.viEnterInsert();
        this.refreshLine();
        return;

      // A — insert at end
      case "A":
        this.cursor = this.buffer.length;
        this.viEnterInsert();
        this.refreshLine();
        return;

      // s — substitute character
      case "s":
        if (this.cursor < this.buffer.length) {
          this.killRing = this.buffer[this.cursor];
          this.buffer =
            this.buffer.substring(0, this.cursor) +
            this.buffer.substring(this.cursor + 1);
          this.refreshLine();
        }
        this.viEnterInsert();
        return;

      // S — substitute whole line
      case "S":
        this.killRing = this.buffer;
        this.buffer = "";
        this.cursor = 0;
        this.refreshLine();
        this.viEnterInsert();
        return;

      // x — delete char under cursor
      case "x":
        if (this.cursor < this.buffer.length) {
          this.killRing = this.buffer[this.cursor];
          this.buffer =
            this.buffer.substring(0, this.cursor) +
            this.buffer.substring(this.cursor + 1);
          if (
            this.cursor >= this.buffer.length &&
            this.buffer.length > 0
          )
            this.cursor = this.buffer.length - 1;
          this.refreshLine();
        }
        return;

      // X — delete char before cursor
      case "X":
        if (this.cursor > 0) {
          this.killRing = this.buffer[this.cursor - 1];
          this.buffer =
            this.buffer.substring(0, this.cursor - 1) +
            this.buffer.substring(this.cursor);
          this.cursor--;
          this.refreshLine();
        }
        return;

      // D — delete to end of line
      case "D":
        this.killRing = this.buffer.substring(this.cursor);
        this.buffer = this.buffer.substring(0, this.cursor);
        if (this.cursor > 0 && this.cursor >= this.buffer.length)
          this.cursor = this.buffer.length - 1;
        this.refreshLine();
        return;

      // C — change to end of line
      case "C":
        this.killRing = this.buffer.substring(this.cursor);
        this.buffer = this.buffer.substring(0, this.cursor);
        this.refreshLine();
        this.viEnterInsert();
        return;

      // d — delete operator (waits for motion)
      case "d":
        this.viOperator = "d";
        return;

      // c — change operator (waits for motion)
      case "c":
        this.viOperator = "c";
        return;

      // p — paste after cursor
      case "p":
        if (this.killRing) {
          var pos = Math.min(this.cursor + 1, this.buffer.length);
          this.buffer =
            this.buffer.substring(0, pos) +
            this.killRing +
            this.buffer.substring(pos);
          this.cursor = pos + this.killRing.length - 1;
          this.refreshLine();
        }
        return;

      // P — paste before cursor
      case "P":
        if (this.killRing) {
          this.buffer =
            this.buffer.substring(0, this.cursor) +
            this.killRing +
            this.buffer.substring(this.cursor);
          this.cursor += this.killRing.length - 1;
          this.refreshLine();
        }
        return;

      // k — history prev
      case "k":
        this.historyPrev();
        return;

      // j — history next
      case "j":
        this.historyNext();
        return;

      // Enter — submit
      case "\r":
        this.submit();
        return;
    }

    // Arrow keys in command mode
    if (code === "ArrowLeft") {
      if (this.cursor > 0) {
        this.cursor--;
        this.refreshLine();
      }
      return;
    }
    if (code === "ArrowRight") {
      if (this.cursor < this.buffer.length - 1) {
        this.cursor++;
        this.refreshLine();
      }
      return;
    }
    if (code === "ArrowUp") {
      this.historyPrev();
      return;
    }
    if (code === "ArrowDown") {
      this.historyNext();
      return;
    }
  };

  // --- Programmatic execution ---
  TermShell.prototype.exec = function (line, callback) {
    this.buffer = line;
    this.cursor = line.length;
    this.refreshLine();
    var self = this;
    setTimeout(function () {
      self.submit();
      if (callback) callback();
    }, 50);
  };

  // --- Character-by-character typing animation ---
  TermShell.prototype.autoType = function (line, callback) {
    var self = this;
    var i = 0;
    var speed = 35;
    var variance = 25;

    function typeNext() {
      if (i < line.length) {
        self.insertChar(line[i]);
        i++;
        var delay = speed + Math.floor(Math.random() * variance);
        setTimeout(typeNext, delay);
      } else {
        setTimeout(function () {
          self.submit();
          if (callback) callback();
        }, 300);
      }
    }

    typeNext();
  };

  // ---------------------------------------------------------------
  // 9. Boot sequence
  // ---------------------------------------------------------------

  function boot() {
    var container = document.getElementById("playground-terminal");
    if (!container) return;

    var fileMap = window.__PLAYGROUND_FILES__ || {};
    var fs = new VirtualFS(fileMap);
    var shell = new TermShell(container, fs);

    // Auto-type boot sequence
    shell.prompt();
    shell.autoType("cargo install qualifier", function () {
      shell.autoType("qualifier score", function () {
        // Done — user has control
      });
    });
  }

  // ---------------------------------------------------------------
  // 10. Init — wire up "Try in browser" button
  // ---------------------------------------------------------------

  function init() {
    var btn = document.getElementById("try-it-btn");
    var section = document.getElementById("playground-section");
    if (!btn || !section) return;

    btn.addEventListener("click", function () {
      section.hidden = false;
      section.scrollIntoView({ behavior: "smooth" });
      setTimeout(boot, 80);
      // Disable button so boot only runs once
      btn.disabled = true;
      btn.style.opacity = "0.5";
      btn.style.cursor = "default";
    });
  }

  // Run init when DOM is ready
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
