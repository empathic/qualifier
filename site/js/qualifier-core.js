// Qualifier Core — shared data logic for playground and score explorer
// Parsing .qual files, computing scores, building graphs

(function () {
  "use strict";

  // --- .qual file parser ---
  // Parses JSONL with comment support (lines starting with //)
  function parseQualFile(text) {
    var attestations = [];
    var lines = text.split("\n");
    for (var i = 0; i < lines.length; i++) {
      var line = lines[i].trim();
      if (!line || line.indexOf("//") === 0) continue;
      try {
        attestations.push(JSON.parse(line));
      } catch (e) {
        // skip malformed lines
      }
    }
    return attestations;
  }

  // --- Graph parser ---
  // Parses qualifier.graph.jsonl
  function parseGraph(text) {
    var entries = [];
    var lines = text.split("\n");
    for (var i = 0; i < lines.length; i++) {
      var line = lines[i].trim();
      if (!line || line.indexOf("//") === 0) continue;
      try {
        entries.push(JSON.parse(line));
      } catch (e) {
        // skip malformed
      }
    }
    return entries;
  }

  // --- Supersession filtering ---
  // Returns only non-superseded records
  function filterSuperseded(records) {
    var superseded = {};
    for (var i = 0; i < records.length; i++) {
      var r = records[i];
      var sup = r.body && r.body.supersedes;
      if (sup) superseded[sup] = true;
    }
    var active = [];
    for (var j = 0; j < records.length; j++) {
      if (!superseded[records[j].id]) {
        active.push(records[j]);
      }
    }
    return active;
  }

  // --- Check if a record is scored (attestation or epoch) ---
  function isScored(record) {
    var t = record.type || "attestation";
    return t === "attestation" || t === "epoch";
  }

  // --- Raw score ---
  function rawScore(records) {
    var active = filterSuperseded(records);
    var sum = 0;
    for (var i = 0; i < active.length; i++) {
      if (isScored(active[i])) {
        var score = active[i].body ? active[i].body.score : 0;
        sum += score || 0;
      }
    }
    return Math.max(-100, Math.min(100, sum));
  }

  // --- Group records by subject ---
  function groupBySubject(records) {
    var groups = {};
    for (var i = 0; i < records.length; i++) {
      var r = records[i];
      var key = r.subject;
      if (!groups[key]) groups[key] = [];
      groups[key].push(r);
    }
    return groups;
  }

  // --- Build adjacency list from graph entries ---
  function buildAdjacency(graphEntries) {
    var adj = {};
    var allSubjects = {};
    for (var i = 0; i < graphEntries.length; i++) {
      var e = graphEntries[i];
      var subj = e.subject;
      var deps = e.body ? e.body.depends_on : e.depends_on;
      adj[subj] = deps || [];
      allSubjects[subj] = true;
      deps = deps || [];
      for (var j = 0; j < deps.length; j++) {
        allSubjects[deps[j]] = true;
      }
    }
    // Ensure all referenced subjects have entries
    for (var s in allSubjects) {
      if (!adj[s]) adj[s] = [];
    }
    return adj;
  }

  // --- Topological sort ---
  function toposort(adj) {
    var visited = {};
    var order = [];
    var temp = {};

    function visit(node) {
      if (temp[node]) return; // cycle — skip
      if (visited[node]) return;
      temp[node] = true;
      var deps = adj[node] || [];
      for (var i = 0; i < deps.length; i++) {
        visit(deps[i]);
      }
      temp[node] = false;
      visited[node] = true;
      order.push(node);
    }

    for (var node in adj) visit(node);
    return order;
  }

  // --- Effective scores ---
  // Returns { subject: { raw, effective, limitingPath } }
  function effectiveScores(graphEntries, records) {
    var adj = buildAdjacency(graphEntries);
    var grouped = groupBySubject(records);
    var order = toposort(adj);
    var scores = {};

    // Compute raw scores for all subjects
    for (var subj in adj) {
      scores[subj] = {
        raw: grouped[subj] ? rawScore(grouped[subj]) : 0,
        effective: 0,
        limitingPath: null,
        records: grouped[subj] ? filterSuperseded(grouped[subj]) : [],
      };
    }

    // Also include subjects only in records, not in graph
    for (var subjKey in grouped) {
      if (!scores[subjKey]) {
        scores[subjKey] = {
          raw: rawScore(grouped[subjKey]),
          effective: 0,
          limitingPath: null,
          records: filterSuperseded(grouped[subjKey]),
        };
      }
    }

    // Process in topological order (dependencies first)
    for (var i = 0; i < order.length; i++) {
      var node = order[i];
      var s = scores[node];
      s.effective = s.raw;
      s.limitingPath = null;

      var deps = adj[node] || [];
      for (var j = 0; j < deps.length; j++) {
        var dep = deps[j];
        if (scores[dep] && scores[dep].effective < s.effective) {
          s.effective = scores[dep].effective;
          s.limitingPath = [dep];
          if (scores[dep].limitingPath) {
            s.limitingPath = s.limitingPath.concat(scores[dep].limitingPath);
          }
        }
      }
    }

    return scores;
  }

  // --- Score status ---
  function scoreStatus(effective) {
    if (effective <= -30) return "blocker";
    if (effective < 0) return "concern";
    if (effective === 0) return "unqualified";
    if (effective < 50) return "ok";
    return "healthy";
  }

  // --- Score bar (text) ---
  function scoreBar(effective, width) {
    width = width || 10;
    var normalized = Math.round(((effective + 100) / 200) * width);
    normalized = Math.max(0, Math.min(width, normalized));
    var filled = "";
    var empty = "";
    for (var i = 0; i < normalized; i++) filled += "\u2588";
    for (var j = normalized; j < width; j++) empty += "\u2591";
    return filled + empty;
  }

  // --- Status color ---
  function statusColor(status) {
    switch (status) {
      case "healthy": return { fill: "#34d39920", stroke: "#34d399" };
      case "ok":      return { fill: "#34d39920", stroke: "#34d399" };
      case "concern": return { fill: "#f8717120", stroke: "#f87171" };
      case "blocker": return { fill: "#f8717120", stroke: "#f87171" };
      default:        return { fill: "#1b1f2a",   stroke: "#6b7394" };
    }
  }

  // --- Format detection ---
  function isMetabox(record) {
    return record.metabox === "1";
  }

  // --- Export ---
  window.QualifierCore = {
    parseQualFile: parseQualFile,
    parseGraph: parseGraph,
    filterSuperseded: filterSuperseded,
    rawScore: rawScore,
    groupBySubject: groupBySubject,
    buildAdjacency: buildAdjacency,
    toposort: toposort,
    effectiveScores: effectiveScores,
    scoreStatus: scoreStatus,
    scoreBar: scoreBar,
    statusColor: statusColor,
    isMetabox: isMetabox,
  };
})();
