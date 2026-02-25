// Qualifier Score Explorer — interactive dependency graph visualization
// Uses D3.js + dagre-d3 for graph layout and rendering

(function () {
  "use strict";

  var QC = window.QualifierCore;
  if (!QC) return;

  var canvas = document.getElementById("scoring-canvas");

  var graphInput = document.getElementById("graph-input");
  var qualInput = document.getElementById("qual-input");
  var computeBtn = document.getElementById("scoring-compute-btn");
  if (!canvas || !graphInput || !qualInput || !computeBtn) return;

  var currentScores = null;
  var currentAdj = null;
  var svgEl = null;
  var gEl = null;
  var zoomBehavior = null;
  var detailPanel = null;

  // --- Colors ---
  var COLORS = {
    healthy: { fill: "#34d39920", stroke: "#34d399" },
    ok: { fill: "#34d39920", stroke: "#34d399" },
    concern: { fill: "#f8717120", stroke: "#f87171" },
    blocker: { fill: "#f8717120", stroke: "#f87171" },
    unqualified: { fill: "#1b1f2a", stroke: "#6b7394" },
  };

  function getColor(status) {
    return COLORS[status] || COLORS.unqualified;
  }

  // --- Score bar as small rectangles ---
  function miniBar(score, width, height) {
    width = width || 60;
    height = height || 6;
    var normalized = Math.round(((score + 100) / 200) * width);
    normalized = Math.max(0, Math.min(width, normalized));
    var svg = "";
    svg +=
      '<rect x="0" y="0" width="' +
      width +
      '" height="' +
      height +
      '" fill="#1b1f2a" stroke="#252a3866" stroke-width="0.5"/>';
    if (normalized > 0) {
      var barColor = score >= 0 ? "#34d399" : "#f87171";
      svg +=
        '<rect x="0" y="0" width="' +
        normalized +
        '" height="' +
        height +
        '" fill="' +
        barColor +
        '" opacity="0.6"/>';
    }
    return svg;
  }

  // --- Render graph ---
  function render() {
    var graphText = graphInput.value;
    var qualText = qualInput.value;

    var graphEntries = QC.parseGraph(graphText);
    var attestations = QC.parseQualFile(qualText);
    var scores = QC.effectiveScores(graphEntries, attestations);
    var adj = QC.buildAdjacency(graphEntries);

    currentScores = scores;
    currentAdj = adj;

    // Clear existing
    var controlsEl = canvas.querySelector(".scoring-controls");
    canvas.innerHTML = "";
    if (controlsEl) canvas.appendChild(controlsEl);
    if (detailPanel) {
      detailPanel.remove();
      detailPanel = null;
    }

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
      var color = getColor(status);
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

    // Add score labels and mini bars to nodes
    gEl.selectAll("g.node").each(function (v) {
      var node = d3.select(this);
      var s = scores[v];
      if (!s) return;
      var status = QC.scoreStatus(s.effective);
      var scoreColor = s.effective >= 0 ? "#34d399" : "#f87171";

      // Score text below the label
      node
        .append("text")
        .attr("x", 0)
        .attr("y", 14)
        .attr("text-anchor", "middle")
        .attr("font-family", "JetBrains Mono, monospace")
        .attr("font-size", "9px")
        .attr("fill", scoreColor)
        .text(
          "raw: " + s.raw + "  eff: " + s.effective,
        );

      // Status label
      node
        .append("text")
        .attr("x", 0)
        .attr("y", -14)
        .attr("text-anchor", "middle")
        .attr("font-family", "Instrument Sans, sans-serif")
        .attr("font-size", "8px")
        .attr("font-weight", "600")
        .attr("letter-spacing", "0.06em")
        .attr("text-transform", "uppercase")
        .attr("fill", scoreColor)
        .text(status.toUpperCase());
    });

    // Click handler for detail panel
    gEl.selectAll("g.node").on("click", function (event, v) {
      showDetail(v);
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
    var containerHeight = canvas.clientHeight - 40; // controls bar
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
      .call(zoomBehavior.transform, d3.zoomIdentity.translate(tx, ty).scale(scale));
  }

  // --- Detail panel ---
  function showDetail(artifact) {
    if (!currentScores || !currentScores[artifact]) return;
    var s = currentScores[artifact];

    if (detailPanel) detailPanel.remove();
    detailPanel = document.createElement("div");
    detailPanel.className = "scoring-detail";

    var html = '<h3>' + escapeHtml(artifact) + "</h3>";
    html += '<div class="score-summary">';
    html += "Raw: " + s.raw + " &middot; Effective: " + s.effective;
    if (s.limitingPath) {
      html +=
        '<br><span style="color: var(--fail); font-size: 0.78rem;">Limited by ' +
        escapeHtml(s.limitingPath[0]) +
        "</span>";
    }
    html += "</div>";

    if (s.attestations.length === 0) {
      html += '<p style="color: var(--flint); font-size: 0.88rem;">No attestations.</p>';
    } else {
      for (var i = 0; i < s.attestations.length; i++) {
        var a = s.attestations[i];
        var scoreClass = a.score >= 0 ? "positive" : "negative";
        var sign = a.score >= 0 ? "+" : "";
        html += '<div class="attestation-item">';
        html +=
          '<span class="attestation-score ' +
          scoreClass +
          '">[' +
          sign +
          a.score +
          "]</span> ";
        html +=
          '<span class="kind-badge ' +
          (a.score >= 0 ? "positive" : "negative") +
          '">' +
          escapeHtml(a.kind) +
          "</span>";
        html +=
          '<div class="attestation-summary">' +
          escapeHtml(a.summary) +
          "</div>";
        html +=
          '<div class="attestation-meta">' +
          escapeHtml(a.author || "") +
          " &middot; " +
          escapeHtml((a.created_at || "").substring(0, 10)) +
          "</div>";
        if (a.suggested_fix) {
          html +=
            '<div class="attestation-meta" style="color: var(--accent); margin-top: 0.2rem;">Fix: ' +
            escapeHtml(a.suggested_fix) +
            "</div>";
        }
        html += "</div>";
      }
    }

    // Close button
    html +=
      '<button style="position:absolute;top:0.5rem;right:0.75rem;background:none;border:none;font-size:1.2rem;cursor:pointer;color:var(--flint);" onclick="this.parentElement.remove()">&times;</button>';

    detailPanel.innerHTML = html;
    canvas.appendChild(detailPanel);
  }

  function escapeHtml(s) {
    if (!s) return "";
    return s
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  // --- Wire controls ---
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

  // --- Init ---
  computeBtn.addEventListener("click", render);
  wireControls();

  // Auto-render on load with default data
  if (graphInput.value.trim() && qualInput.value.trim()) {
    setTimeout(render, 100);
  }
})();
