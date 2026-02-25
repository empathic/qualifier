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
  // Returns only non-superseded attestations
  function filterSuperseded(attestations) {
    var superseded = {};
    for (var i = 0; i < attestations.length; i++) {
      var a = attestations[i];
      if (a.supersedes) superseded[a.supersedes] = true;
    }
    var active = [];
    for (var j = 0; j < attestations.length; j++) {
      if (!superseded[attestations[j].id]) {
        active.push(attestations[j]);
      }
    }
    return active;
  }

  // --- Raw score ---
  function rawScore(attestations) {
    var active = filterSuperseded(attestations);
    var sum = 0;
    for (var i = 0; i < active.length; i++) {
      sum += active[i].score || 0;
    }
    return Math.max(-100, Math.min(100, sum));
  }

  // --- Group attestations by artifact ---
  function groupByArtifact(attestations) {
    var groups = {};
    for (var i = 0; i < attestations.length; i++) {
      var a = attestations[i];
      var key = a.artifact;
      if (!groups[key]) groups[key] = [];
      groups[key].push(a);
    }
    return groups;
  }

  // --- Build adjacency list from graph entries ---
  function buildAdjacency(graphEntries) {
    var adj = {};
    var allArtifacts = {};
    for (var i = 0; i < graphEntries.length; i++) {
      var e = graphEntries[i];
      adj[e.artifact] = e.depends_on || [];
      allArtifacts[e.artifact] = true;
      var deps = e.depends_on || [];
      for (var j = 0; j < deps.length; j++) {
        allArtifacts[deps[j]] = true;
      }
    }
    // Ensure all referenced artifacts have entries
    for (var art in allArtifacts) {
      if (!adj[art]) adj[art] = [];
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
  // Returns { artifact: { raw, effective, limitingPath } }
  function effectiveScores(graphEntries, attestations) {
    var adj = buildAdjacency(graphEntries);
    var grouped = groupByArtifact(attestations);
    var order = toposort(adj);
    var scores = {};

    // Compute raw scores for all artifacts
    for (var art in adj) {
      scores[art] = {
        raw: grouped[art] ? rawScore(grouped[art]) : 0,
        effective: 0,
        limitingPath: null,
        attestations: grouped[art] ? filterSuperseded(grouped[art]) : [],
      };
    }

    // Also include artifacts only in attestations, not in graph
    for (var artKey in grouped) {
      if (!scores[artKey]) {
        scores[artKey] = {
          raw: rawScore(grouped[artKey]),
          effective: 0,
          limitingPath: null,
          attestations: filterSuperseded(grouped[artKey]),
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

  // --- Export ---
  window.QualifierCore = {
    parseQualFile: parseQualFile,
    parseGraph: parseGraph,
    filterSuperseded: filterSuperseded,
    rawScore: rawScore,
    groupByArtifact: groupByArtifact,
    buildAdjacency: buildAdjacency,
    toposort: toposort,
    effectiveScores: effectiveScores,
    scoreStatus: scoreStatus,
    scoreBar: scoreBar,
  };
})();
