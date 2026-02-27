// Qualifier Playground — interactive terminal for the qualifier site
// Runs the real qualifier CLI compiled to wasm via Emscripten.

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

  function bold(s) {
    return ANSI.bold + s + ANSI.reset;
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

  // ---------------------------------------------------------------
  // 4. WASM layer — real qualifier CLI via Emscripten
  // ---------------------------------------------------------------

  var compiledWasm = null;
  var wasmFiles = null;
  var wasmReady = false;
  var wasmError = null;

  function loadWasm(files) {
    wasmFiles = files;
    if (typeof createQualifierModule !== "function") {
      wasmError = "Wasm module not loaded";
      return Promise.reject(new Error(wasmError));
    }
    return fetch("/wasm/qualifier.wasm")
      .then(function (resp) {
        return WebAssembly.compileStreaming(resp);
      })
      .then(function (mod) {
        compiledWasm = mod;
        wasmReady = true;
      })
      .catch(function (err) {
        wasmError = err.message || String(err);
        throw err;
      });
  }

  // Fresh Emscripten instance per call — exit() kills the instance, not us
  function runQualifier(args) {
    var stdout = "";
    var stderr = "";
    return createQualifierModule({
      noInitialRun: true,
      instantiateWasm: function (imports, callback) {
        WebAssembly.instantiate(compiledWasm, imports).then(function (
          instance,
        ) {
          callback(instance);
        });
        return {};
      },
      print: function (text) {
        stdout += text + "\n";
      },
      printErr: function (text) {
        stderr += text + "\n";
      },
    }).then(function (mod) {
      for (var name in wasmFiles) {
        if (wasmFiles.hasOwnProperty(name)) {
          mod.FS.writeFile("/" + name, wasmFiles[name]);
        }
      }
      try {
        mod.callMain(args);
      } catch (e) {
        // exit() throws to unwind — expected
      }
      return { stdout: stdout.trimEnd(), stderr: stderr.trimEnd() };
    });
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
    lines.push(copperBold("QUALIFIER PLAYGROUND"));
    lines.push("");
    lines.push(bold("Shell builtins:"));
    lines.push(
      "  " + copperBold(pad("ls", 38, true)) + "List files",
    );
    lines.push(
      "  " +
        copperBold(pad("cat <file>", 38, true)) +
        "Display file contents",
    );
    lines.push(
      "  " + copperBold(pad("clear", 38, true)) + "Clear terminal",
    );
    lines.push(
      "  " + copperBold(pad("help", 38, true)) + "Show this message",
    );
    lines.push("");
    lines.push(bold("CLI commands:"));
    lines.push(
      "  " +
        copperBold(pad("qualifier score", 38, true)) +
        "Compute and display all scores",
    );
    lines.push(
      "  " +
        copperBold(pad("qualifier show <artifact>", 38, true)) +
        "Show attestations for an artifact",
    );
    lines.push(
      "  " +
        copperBold(pad("qualifier check [--min-score N]", 38, true)) +
        "CI gate check",
    );
    lines.push(
      "  " +
        copperBold(pad("qualifier ls [--below N]", 38, true)) +
        "List artifacts by score",
    );
    lines.push(
      "  " +
        copperBold(pad("qualifier --help", 38, true)) +
        "Full CLI usage",
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

    // qualifier commands — delegate to real wasm binary
    if (cmd === "qualifier") {
      if (!wasmReady) {
        if (wasmError) {
          return { output: red("Wasm failed to load: " + wasmError) };
        }
        return { output: dim("Loading CLI... try again in a moment.") };
      }
      var qualArgs = args.slice(1);
      return runQualifier(qualArgs).then(function (result) {
        var output = "";
        if (result.stdout) output += result.stdout.replace(/\n/g, "\r\n");
        if (result.stderr) {
          if (output) output += "\r\n";
          output += result.stderr.replace(/\n/g, "\r\n");
        }
        return { output: output };
      });
    }

    // cargo install qualifier — special case for boot
    if (cmd === "cargo" && args[1] === "install" && args[2] === "qualifier") {
      return {
        output:
          dim("  qualifier is already installed."),
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
    this.busy = false;

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
      if (self.busy) return;
      self.handleKey(e.key, e.domEvent);
    });

    // --- Paste handler ---
    this.term.onData(function (data) {
      if (self.busy) return;
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

    var self = this;
    var result = dispatch(line, this.fs);

    if (result && typeof result.then === "function") {
      // Async (wasm command) — disable input until done
      this.busy = true;
      result.then(function (r) {
        if (r && r.output) {
          self.term.write(r.output);
          self.term.write("\r\n");
        }
        self.busy = false;
        self.prompt();
        self._attachScrollWatcher();
      });
    } else {
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
      this._attachScrollWatcher();
    }
  };

  TermShell.prototype._attachScrollWatcher = function () {
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

  // --- Programmatic execution (async-aware) ---
  TermShell.prototype.exec = function (line, callback) {
    this.buffer = line;
    this.cursor = line.length;
    this.refreshLine();
    var self = this;
    setTimeout(function () {
      self.term.write("\r\n");
      if (line.trim()) {
        self.history.unshift(line);
      }
      self.pinCommand(line.trim());
      var result = dispatch(line, self.fs);
      if (result && typeof result.then === "function") {
        result.then(function (r) {
          if (r && r.output) {
            self.term.write(r.output);
            self.term.write("\r\n");
          }
          if (callback) callback();
        });
      } else {
        if (result) {
          if (result.clear) {
            self.term.clear();
          } else if (result.output) {
            self.term.write(result.output);
            self.term.write("\r\n");
          }
        }
        if (callback) callback();
      }
    }, 50);
  };

  // --- Character-by-character typing animation (async-aware) ---
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
          self.term.write("\r\n");
          if (line.trim()) {
            self.history.unshift(line);
          }
          self.pinCommand(line.trim());
          var result = dispatch(line, self.fs);
          if (result && typeof result.then === "function") {
            result.then(function (r) {
              if (r && r.output) {
                self.term.write(r.output);
                self.term.write("\r\n");
              }
              if (callback) callback();
            });
          } else {
            if (result && result.output) {
              self.term.write(result.output);
              self.term.write("\r\n");
            }
            if (callback) callback();
          }
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

    shell.term.write(dim("  Loading CLI...") + "\r\n\r\n");

    loadWasm(fileMap)
      .then(function () {
        shell.prompt();
        shell.autoType("qualifier score", function () {
          shell.prompt();
        });
      })
      .catch(function () {
        shell.term.write(red("  Failed to load wasm binary.") + "\r\n");
        shell.term.write(
          dim("  Shell builtins (ls, cat, help) are still available.") +
            "\r\n\r\n",
        );
        shell.prompt();
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
