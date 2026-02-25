// Qualifier Scoring Page — table, graph, and cross-linking
// Three outputs: algorithm explanation (static HTML), score table, dependency graph

(function () {
  "use strict";

  var QC = window.QualifierCore;
  if (!QC) return;

  // --- DOM references ---
  var graphInput = document.getElementById("graph-input");
  var qualInput = document.getElementById("qual-input");
  var computeBtn = document.getElementById("scoring-compute-btn");
  var tableContainer = document.getElementById("score-table-container");
  var canvas = document.getElementById("scoring-canvas");
  if (!graphInput || !qualInput || !computeBtn || !tableContainer || !canvas) return;

  // --- State ---
  var currentScores = null;
  var currentAdj = null;
  var expandedArtifact = null;
  var svgEl = null;
  var gEl = null;
  var zoomBehavior = null;

  // --- Helpers ---
  function escapeHtml(s) {
    if (!s) return "";
    return s
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function escapeAttr(s) {
    if (!s) return "";
    return s
      .replace(/&/g, "&amp;")
      .replace(/"/g, "&quot;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  function statusBadgeClass(status) {
    if (status === "blocker" || status === "concern") return "negative";
    if (status === "ok" || status === "healthy") return "positive";
    return "neutral";
  }

  // ============================================================
  // COMPUTE — main entry point
  // ============================================================

  function compute() {
    var graphText = graphInput.value;
    var qualText = qualInput.value;

    var graphEntries = QC.parseGraph(graphText);
    var attestations = QC.parseQualFile(qualText);
    var scores = QC.effectiveScores(graphEntries, attestations);
    var adj = QC.buildAdjacency(graphEntries);

    currentScores = scores;
    currentAdj = adj;
    expandedArtifact = null;

    renderTable(scores);
    renderGraph(scores, adj);
  }

  // ============================================================
  // SCORE TABLE — the star of the page
  // ============================================================

  function renderTable(scores) {
    var entries = [];
    for (var art in scores) {
      var s = scores[art];
      entries.push({
        artifact: art,
        raw: s.raw,
        effective: s.effective,
        status: QC.scoreStatus(s.effective),
        limitingPath: s.limitingPath,
        attestations: s.attestations,
      });
    }

    // Sort: effective ascending, then raw ascending
    entries.sort(function (a, b) {
      if (a.effective !== b.effective) return a.effective - b.effective;
      return a.raw - b.raw;
    });

    var html = '<table class="score-table">';
    html += "<thead><tr>";
    html += "<th>Artifact</th>";
    html += '<th class="col-score">Raw</th>';
    html += '<th class="col-score">Eff</th>';
    html += "<th>Status</th>";
    html += '<th class="col-bar"></th>';
    html += "</tr></thead>";
    html += "<tbody>";

    for (var i = 0; i < entries.length; i++) {
      var e = entries[i];
      var rawClass = e.raw > 0 ? "positive" : e.raw < 0 ? "negative" : "zero";
      var effClass =
        e.effective > 0 ? "positive" : e.effective < 0 ? "negative" : "zero";
      var rawSign = e.raw > 0 ? "+" : "";
      var effSign = e.effective > 0 ? "+" : "";
      var badgeClass = statusBadgeClass(e.status);
      var barPct = Math.round(((e.effective + 100) / 200) * 100);
      var barClass = e.effective >= 0 ? "positive" : "negative";

      html +=
        '<tr class="st-row" data-artifact="' +
        escapeAttr(e.artifact) +
        '">';

      // Artifact name + limiting path hint
      html += '<td class="st-artifact">' + escapeHtml(e.artifact);
      if (e.limitingPath) {
        html +=
          '<div class="st-limited-by">limited by ' +
          escapeHtml(e.limitingPath[0]) +
          "</div>";
      }
      html += "</td>";

      // Raw score
      html +=
        '<td class="st-score ' + rawClass + '">' + rawSign + e.raw + "</td>";

      // Effective score
      html +=
        '<td class="st-score ' +
        effClass +
        '">' +
        effSign +
        e.effective +
        "</td>";

      // Status badge
      html +=
        '<td class="st-status"><span class="kind-badge ' +
        badgeClass +
        '">' +
        e.status +
        "</span></td>";

      // Bar
      html +=
        '<td class="st-bar"><span class="st-bar-track"><span class="st-bar-fill ' +
        barClass +
        '" style="width:' +
        barPct +
        '%"></span></span></td>';

      html += "</tr>";
    }

    html += "</tbody></table>";
    tableContainer.innerHTML = html;

    // Wire click listeners
    var rows = tableContainer.querySelectorAll("tr.st-row");
    for (var j = 0; j < rows.length; j++) {
      rows[j].addEventListener(
        "click",
        (function (row) {
          return function () {
            toggleExpansion(row.getAttribute("data-artifact"));
          };
        })(rows[j]),
      );
    }
  }

  // ============================================================
  // ROW EXPANSION — inline attestation detail
  // ============================================================

  function toggleExpansion(artifact) {
    // Remove any existing expansion row
    var existing = tableContainer.querySelector("tr.st-expansion-row");
    if (existing) existing.remove();

    // Remove highlight from all rows
    var allRows = tableContainer.querySelectorAll("tr.st-row");
    for (var i = 0; i < allRows.length; i++) {
      allRows[i].classList.remove("st-row-highlight");
    }

    // If clicking the same row, just collapse
    if (expandedArtifact === artifact) {
      expandedArtifact = null;
      return;
    }

    expandedArtifact = artifact;

    // Find the target row
    var targetRow = null;
    for (var k = 0; k < allRows.length; k++) {
      if (allRows[k].getAttribute("data-artifact") === artifact) {
        targetRow = allRows[k];
        break;
      }
    }
    if (!targetRow) return;
    targetRow.classList.add("st-row-highlight");

    // Build expansion content
    var s = currentScores[artifact];
    var html = '<div class="st-expansion">';
    html +=
      '<div class="st-expansion-title">Attestations for ' +
      escapeHtml(artifact) +
      "</div>";

    if (!s || s.attestations.length === 0) {
      html += '<p class="st-no-attestations">No attestations recorded.</p>';
    } else {
      for (var j = 0; j < s.attestations.length; j++) {
        var a = s.attestations[j];
        var scoreClass = a.score >= 0 ? "positive" : "negative";
        var sign = a.score >= 0 ? "+" : "";

        html += '<div class="st-attestation">';
        html +=
          '<span class="st-attestation-score ' +
          scoreClass +
          '">[' +
          sign +
          a.score +
          "]</span>";
        html +=
          '<span class="kind-badge ' +
          (a.score >= 0 ? "positive" : "negative") +
          '">' +
          escapeHtml(a.kind) +
          "</span>";
        html +=
          '<span class="st-attestation-summary">' +
          escapeHtml(a.summary) +
          "</span>";
        html +=
          '<span class="st-attestation-meta">' +
          escapeHtml(a.author || "") +
          " &middot; " +
          escapeHtml((a.created_at || "").substring(0, 10)) +
          "</span>";

        if (a.suggested_fix) {
          html +=
            '<div class="st-attestation-fix">Fix: ' +
            escapeHtml(a.suggested_fix) +
            "</div>";
        }
        html += "</div>";
      }
    }
    html += "</div>";

    // Create and insert expansion row
    var tr = document.createElement("tr");
    tr.className = "st-expansion-row";
    var td = document.createElement("td");
    td.setAttribute("colspan", "5");
    td.innerHTML = html;
    tr.appendChild(td);
    targetRow.parentNode.insertBefore(tr, targetRow.nextSibling);
  }

  // ============================================================
  // GRAPH → TABLE LINKING
  // ============================================================

  function scrollToAndHighlight(artifact) {
    toggleExpansion(artifact);

    var allRows = tableContainer.querySelectorAll("tr.st-row");
    for (var i = 0; i < allRows.length; i++) {
      if (allRows[i].getAttribute("data-artifact") === artifact) {
        allRows[i].scrollIntoView({ behavior: "smooth", block: "center" });
        break;
      }
    }
  }

  // ============================================================
  // DEPENDENCY GRAPH — dagre-d3 visualization
  // ============================================================

  function renderGraph(scores, adj) {
    // Clear existing canvas content (keep controls)
    var controlsEl = canvas.querySelector(".scoring-controls");
    canvas.innerHTML = "";
    if (controlsEl) canvas.appendChild(controlsEl);

    // Create SVG
    svgEl = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svgEl.setAttribute("width", "100%");
    svgEl.setAttribute("height", "100%");
    canvas.appendChild(svgEl);

    // Rebuild controls if they got removed
    if (!canvas.querySelector(".scoring-controls")) {
      var controls = document.createElement("div");
      controls.className = "scoring-controls";
      controls.innerHTML =
        '<button id="scoring-zoom-in" title="Zoom in">+</button>' +
        '<button id="scoring-zoom-out" title="Zoom out">&minus;</button>' +
        '<button id="scoring-fit" title="Fit to view">Fit</button>';
      canvas.insertBefore(controls, svgEl);
      wireControls();
    }

    // Build dagre graph
    var g = new dagreD3.graphlib.Graph().setGraph({
      rankdir: "TB",
      nodesep: 70,
      ranksep: 60,
      marginx: 20,
      marginy: 20,
    });

    // Add nodes
    for (var art in scores) {
      var s = scores[art];
      var status = QC.scoreStatus(s.effective);
      var color = QC.statusColor(status);
      g.setNode(art, {
        label: art,
        raw: s.raw,
        effective: s.effective,
        status: status,
        style:
          "fill: " +
          color.fill +
          "; stroke: " +
          color.stroke +
          "; stroke-width: " +
          (s.limitingPath ? "2" : "1.5") +
          ";",
        labelStyle:
          "font-family: JetBrains Mono, monospace; font-size: 11px; font-weight: 600; fill: #d0d5e3;",
        shape: "rect",
        width: 140,
        height: 52,
      });
    }

    // Add edges
    for (var node in adj) {
      var deps = adj[node];
      for (var i = 0; i < deps.length; i++) {
        var dep = deps[i];
        if (!scores[dep]) continue;
        var isLimiting =
          scores[node].limitingPath &&
          scores[node].limitingPath.indexOf(dep) !== -1;
        g.setEdge(node, dep, {
          style: isLimiting
            ? "stroke: #f87171; stroke-width: 2;"
            : "stroke: #6b7394; stroke-width: 1;",
          arrowheadStyle: isLimiting ? "fill: #f87171" : "fill: #6b7394",
        });
      }
    }

    // Render with dagre-d3
    var d3svg = d3.select(svgEl);
    gEl = d3svg.append("g");
    var renderer = new dagreD3.render();
    renderer(gEl, g);

    // Add score labels to nodes
    gEl.selectAll("g.node").each(function (v) {
      var nodeEl = d3.select(this);
      var sc = scores[v];
      if (!sc) return;
      var scoreColor = sc.effective >= 0 ? "#34d399" : "#f87171";

      // Score text below the label
      nodeEl
        .append("text")
        .attr("x", 0)
        .attr("y", 14)
        .attr("text-anchor", "middle")
        .attr("font-family", "JetBrains Mono, monospace")
        .attr("font-size", "9px")
        .attr("fill", scoreColor)
        .text("raw: " + sc.raw + "  eff: " + sc.effective);

      // Status label above
      nodeEl
        .append("text")
        .attr("x", 0)
        .attr("y", -14)
        .attr("text-anchor", "middle")
        .attr("font-family", "Instrument Sans, sans-serif")
        .attr("font-size", "8px")
        .attr("font-weight", "600")
        .attr("letter-spacing", "0.06em")
        .attr("fill", scoreColor)
        .text(QC.scoreStatus(sc.effective).toUpperCase());
    });

    // Click handler → scroll to table row
    gEl.selectAll("g.node").on("click", function (event, v) {
      scrollToAndHighlight(v);
    });

    // Zoom
    zoomBehavior = d3
      .zoom()
      .scaleExtent([0.2, 3])
      .on("zoom", function (event) {
        gEl.attr("transform", event.transform);
      });
    d3svg.call(zoomBehavior);

    // Fit to view
    fitToView();
  }

  function fitToView() {
    if (!svgEl || !gEl) return;
    var bbox = gEl.node().getBBox();
    var containerWidth = canvas.clientWidth;
    var containerHeight = canvas.clientHeight - 40;
    if (containerWidth <= 0 || containerHeight <= 0) return;

    var scale = Math.min(
      containerWidth / (bbox.width + 40),
      containerHeight / (bbox.height + 40),
      1.5,
    );
    var tx = (containerWidth - bbox.width * scale) / 2 - bbox.x * scale;
    var ty =
      (containerHeight - bbox.height * scale) / 2 - bbox.y * scale + 40;

    d3.select(svgEl)
      .transition()
      .duration(300)
      .call(
        zoomBehavior.transform,
        d3.zoomIdentity.translate(tx, ty).scale(scale),
      );
  }

  // ============================================================
  // CONTROLS
  // ============================================================

  function wireControls() {
    var zoomIn = document.getElementById("scoring-zoom-in");
    var zoomOut = document.getElementById("scoring-zoom-out");
    var fit = document.getElementById("scoring-fit");

    if (zoomIn)
      zoomIn.addEventListener("click", function () {
        if (zoomBehavior && svgEl)
          d3.select(svgEl)
            .transition()
            .duration(200)
            .call(zoomBehavior.scaleBy, 1.3);
      });
    if (zoomOut)
      zoomOut.addEventListener("click", function () {
        if (zoomBehavior && svgEl)
          d3.select(svgEl)
            .transition()
            .duration(200)
            .call(zoomBehavior.scaleBy, 0.7);
      });
    if (fit)
      fit.addEventListener("click", function () {
        fitToView();
      });
  }

  // ============================================================
  // INIT
  // ============================================================

  computeBtn.addEventListener("click", compute);
  wireControls();

  // Auto-compute on load with example data
  if (graphInput.value.trim() && qualInput.value.trim()) {
    setTimeout(compute, 100);
  }
})();
