use anyhow::Result;
use analysis::{GraphAnalysis, GraphDiff};
use core::CodeGraph;
use std::path::Path;

// ─── HTML template ────────────────────────────────────────────────────────────
// __DATA__ and __APP__ are replaced at runtime; no Rust format! brace-escaping
// needed inside APP_JS or HTML_TEMPLATE.

const HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="fr">
<head>
<meta charset="UTF-8">
<title>Code Graph</title>
<script src="https://unpkg.com/vis-network/standalone/umd/vis-network.min.js"></script>
<style>
*{box-sizing:border-box;margin:0;padding:0}
body{font-family:'Segoe UI',sans-serif;background:#1a1a2e;color:#e0e0e0;display:flex;height:100vh;overflow:hidden}
#sl{width:260px;min-width:210px;background:#16213e;padding:12px;display:flex;flex-direction:column;gap:8px;overflow-y:auto;border-right:1px solid #0f3460;flex-shrink:0}
#net-wrap{flex:1;position:relative;overflow:hidden}
#mynetwork{width:100%;height:100%;background:#0d1117}
#sr{width:270px;min-width:210px;background:#16213e;padding:12px;display:flex;flex-direction:column;gap:6px;overflow-y:auto;border-left:1px solid #0f3460;flex-shrink:0}
#status-bar{position:absolute;bottom:0;left:0;right:0;background:rgba(0,0,0,.7);color:#a0c4ff;font-size:11px;padding:4px 10px;pointer-events:none}
h2{font-size:15px;color:#4A90D9;margin-bottom:2px}
h3{font-size:11px;color:#a0c4ff;text-transform:uppercase;letter-spacing:.08em;margin-bottom:4px}
.sec{border-top:1px solid #0f3460;padding-top:8px}
.filter-row{display:flex;align-items:center;gap:5px;font-size:12px;cursor:pointer;padding:1px 0}
.filter-row input[type=checkbox]{cursor:pointer;accent-color:#4A90D9}
.dot{width:10px;height:10px;border-radius:50%;flex-shrink:0}
input[type=text],input[type=search]{width:100%;padding:5px 8px;background:#0f3460;border:1px solid #1a5276;border-radius:4px;color:#e0e0e0;font-size:12px;outline:none}
.btn-row{display:flex;gap:5px;flex-wrap:wrap}
.layout-btn{background:#0f3460;border:1px solid #1a5276;color:#c0c0c0;padding:5px 9px;border-radius:4px;cursor:pointer;font-size:11px;transition:background .15s,color .15s,border-color .15s}
.layout-btn:hover{background:#1a5276;color:#fff}
.layout-btn.is-active{background:#163a6e;border-color:#4A90D9;color:#4A90D9;font-weight:600;box-shadow:0 0 0 2px #4A90D933}
.nav-arrow{background:#0f3460;border:1px solid #1a5276;color:#c0c0c0;padding:4px 10px;border-radius:4px;cursor:pointer;font-size:14px;transition:background .15s,color .15s}
.nav-arrow:hover:not(:disabled){background:#1a5276;color:#fff}
.nav-arrow:disabled{opacity:0.3;cursor:default}
.fs-btn{background:#0f3460;border:1px solid #1a5276;color:#8fa8c0;padding:3px 7px;border-radius:3px;cursor:pointer;font-size:10px;transition:background .15s,color .15s;max-width:100%;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.fs-btn:hover{background:#1a5276;color:#fff}
.fs-btn.is-active{background:#163a6e;border-color:#4A90D9;color:#4A90D9;font-weight:600}
.fs-btn.fs-dir{color:#a0c4ff;border-color:#1a3a5c}
.fs-bc-link{cursor:pointer;color:#4A90D9;text-decoration:underline}
.fs-bc-link:hover{color:#7ab5f0}
.stat-box{background:#0f3460;border-radius:4px;padding:7px;font-size:12px;line-height:1.7}
.metric-row{font-size:11px;padding:3px 0;border-bottom:1px solid #0f3460;line-height:1.5}
.metric-row .mn{color:#a0c4ff}
#detail-panel{font-size:12px;flex:1;overflow-y:auto}
#detail-panel h4{color:#4A90D9;margin-bottom:4px;font-size:13px}
.edge-item{padding:2px 0;font-size:11px;cursor:pointer;border-radius:2px;padding:2px 4px}
.edge-item:hover{background:#0f3460}
.kind-tag{font-size:10px;padding:1px 5px;border-radius:3px;margin-bottom:6px;display:inline-block}
input[type=range]{width:100%;accent-color:#4A90D9;cursor:pointer}
#ctx-menu{display:none;position:fixed;background:#16213e;border:1px solid #0f3460;border-radius:6px;z-index:9999;overflow:hidden;box-shadow:0 4px 16px rgba(0,0,0,.6);min-width:190px}
.ctx-item{padding:8px 16px;cursor:pointer;font-size:12px;display:flex;align-items:center;gap:8px}
.ctx-item:hover{background:#0f3460;color:#4A90D9}
.cycle-link{cursor:pointer;color:#E74C3C;text-decoration:underline;font-size:12px}
.cycle-link:hover{color:#ff6b6b}
.gear-btn{background:none;border:none;color:#4A90D9;font-size:18px;cursor:pointer;padding:0 4px;opacity:.8;line-height:1}
.gear-btn:hover{opacity:1}
.modal-overlay{display:none;position:fixed;inset:0;background:rgba(0,0,0,.75);z-index:10000;align-items:center;justify-content:center}
.modal-overlay.open{display:flex}
.modal-box{background:#16213e;border:1px solid #0f3460;border-radius:8px;padding:20px;min-width:300px;max-width:380px;box-shadow:0 8px 32px rgba(0,0,0,.8)}
.modal-box h3{color:#4A90D9;margin-bottom:14px;font-size:14px}
.setting-row{display:flex;flex-direction:column;gap:4px;margin-bottom:14px}
.setting-row label{font-size:12px;color:#a0c4ff;display:flex;justify-content:space-between}
.modal-footer{display:flex;gap:8px;justify-content:flex-end;margin-top:6px}
</style>
</head>
<body>

<!-- LEFT SIDEBAR -->
<div id="sl">
  <h2 style="display:flex;justify-content:space-between;align-items:center">Code Graph <button class="gear-btn" onclick="openSettings()" title="Paramètres">⚙</button></h2>

  <div class="sec">
    <h3>Recherche</h3>
    <input type="search" id="search-input" placeholder="Nom du nœud…" oninput="applyFilters()">
  </div>

  <div class="sec">
    <h3>Types de nœuds</h3>
    <div id="node-filters"></div>
  </div>

  <div class="sec">
    <h3>Types de liens</h3>
    <div id="edge-filters"></div>
  </div>

  <div class="sec">
    <h3>Options</h3>
    <label class="filter-row"><input type="checkbox" id="tog-ext"   checked onchange="applyFilters()"> Afficher dépendances externes</label>
    <label class="filter-row"><input type="checkbox" id="tog-force" checked onchange="toggleForce()"> Simulation de forces</label>
    <div style="margin-top:4px">
      <label style="font-size:11px;color:#a0c4ff;display:flex;justify-content:space-between">Vitesse <span id="lbl-speed">5</span></label>
      <input type="range" id="sim-speed" min="1" max="50" value="5"
             oninput="setSimSpeed(this.value)">
    </div>
  </div>

  <div class="sec">
    <h3>Layout</h3>
    <div class="btn-row">
      <button id="btn-graph" class="layout-btn is-active" onclick="toggleGraph()"     title="Layout force-dirigé (placement libre)">Graph</button>
      <button id="btn-hier"  class="layout-btn"           onclick="toggleHier()"      title="Layout hiérarchique (arbre dirigé)">Hiéra.</button>
      <button id="btn-clust" class="layout-btn"           onclick="toggleCluster()"   title="Grouper les nœuds par module">Cluster</button>
      <button id="btn-comp"  class="layout-btn"           onclick="toggleCompMode()"  title="Naviguer composante par composante connexe">Compos.</button>
    </div>
    <div id="comp-nav" style="display:none;align-items:center;gap:8px;margin-top:8px">
      <button id="comp-prev" class="nav-arrow" onclick="showComponent(currentCompIdx-1)">&#8592;</button>
      <span id="comp-label" style="font-size:12px;color:#c0c0c0;min-width:120px;text-align:center">—</span>
      <button id="comp-next" class="nav-arrow" onclick="showComponent(currentCompIdx+1)">&#8594;</button>
    </div>
  </div>

  <div class="sec">
    <h3>Vue</h3>
    <div class="btn-row">
      <button class="layout-btn" onclick="resetAll()"      title="Réinitialiser complètement la page">Reset</button>
      <button class="layout-btn" onclick="resetFocus()"    title="Réafficher tous les nœuds">Tout voir</button>
      <button class="layout-btn" onclick="exportPNG()"     title="Exporter en PNG">PNG</button>
      <button class="layout-btn" onclick="rebuildGraph()"  title="Reconstruire avec les éléments visibles">↺ Recharger</button>
    </div>
  </div>

  <div class="sec">
    <h3>Crate / Module</h3>
    <div id="fs-breadcrumb" style="font-size:10px;color:#6a8a9a;margin-bottom:4px;word-break:break-all">Tous</div>
    <div id="fs-nav-box" style="display:flex;flex-wrap:wrap;gap:4px"></div>
  </div>

  <div class="sec">
    <h3>Focus (double-clic) — profondeur <span id="depth-val">1</span></h3>
    <input type="range" id="focus-depth" min="1" max="5" value="1"
           oninput="document.getElementById('depth-val').textContent=this.value">
  </div>

  <div class="sec">
    <h3>Statistiques</h3>
    <div class="stat-box" id="stats-box">Chargement…</div>
  </div>

  <div class="sec">
    <h3>Top couplage</h3>
    <div id="metrics-box">—</div>
  </div>

  <div class="sec">
    <h3>Debug — noms dupliqués</h3>
    <button class="layout-btn" style="width:100%;margin-bottom:4px" onclick="toggleDupPanel()">Afficher les doublons</button>
    <div id="dup-box" style="display:none;font-size:10px;max-height:200px;overflow-y:auto"></div>
  </div>
</div>

<!-- NETWORK -->
<div id="net-wrap">
  <div id="mynetwork"></div>
  <div id="status-bar">Prêt — double-clic pour focus, clic droit pour options</div>
</div>

<!-- RIGHT SIDEBAR -->
<div id="sr">
  <h3>Détail du nœud</h3>
  <div id="detail-panel"><p style="color:#555;font-size:12px">Sélectionnez un nœud</p></div>
</div>

<!-- CONTEXT MENU -->
<div id="ctx-menu">
  <div class="ctx-item" id="ctx-open">📂 Ouvrir le dossier</div>
  <div class="ctx-item" id="ctx-focus-btn">🎯 Mettre en focus</div>
  <div class="ctx-item" id="ctx-hide">👁 Masquer ce nœud</div>
  <div class="ctx-item" id="ctx-copy">📋 Copier l'identifiant</div>
</div>

<!-- SETTINGS MODAL -->
<div class="modal-overlay" id="settings-modal" onclick="if(event.target===this)closeSettings()">
  <div class="modal-box">
    <h3>⚙ Paramètres du graphe</h3>
    <div class="setting-row">
      <label>Taille minimum <span id="lbl-min-size">15</span> px</label>
      <input type="range" id="set-min-size" min="5" max="60" value="15"
             oninput="document.getElementById('lbl-min-size').textContent=this.value">
    </div>
    <div class="setting-row">
      <label>Ratio max / min <span id="lbl-ratio">13</span> ×</label>
      <input type="range" id="set-ratio" min="2" max="30" value="13"
             oninput="document.getElementById('lbl-ratio').textContent=this.value">
    </div>
    <div class="setting-row">
      <label>Force de répulsion <span id="lbl-repulsion">50</span></label>
      <input type="range" id="set-repulsion" min="5" max="500" value="50"
             oninput="document.getElementById('lbl-repulsion').textContent=this.value">
    </div>
    <div class="setting-row">
      <label>Force des liens (attraction) <span id="lbl-spring">5</span></label>
      <input type="range" id="set-spring" min="1" max="50" value="5"
             oninput="document.getElementById('lbl-spring').textContent=this.value">
    </div>
    <div class="modal-footer">
      <button class="layout-btn" onclick="closeSettings()">Annuler</button>
      <button class="layout-btn is-active" onclick="applySettings()">Appliquer</button>
    </div>
  </div>
</div>

<script>
__DATA__
</script>
<script>
__APP__
</script>
</body>
</html>"#;

// ─── Application JavaScript ───────────────────────────────────────────────────
// Plain raw string — braces are literal, no Rust format! escaping needed.

const APP_JS: &str = r#"
// ── Kind metadata ──────────────────────────────────────────────────────────
const NODE_COLORS = {
  module:   '#4A90D9',
  class:    '#E74C3C',
  function: '#2ECC71',
  method:   '#F39C12',
  property: '#9B59B6',
  constant: '#E67E22',
};

const NODE_LABELS = {
  module:   'Module / Fichier',
  class:    'Struct · Classe · Enum',
  function: 'Fn libre',
  method:   'Fn impl · Méthode',
  property: 'Propriété',
  constant: 'Constante',
};

const EDGE_COLORS = {
  contains:    '#BDC3C7',
  imports:     '#7F8C8D',
  inherits:    '#9B59B6',
  calls:       '#3498DB',
  uses_type:   '#1ABC9C',
  external_dep:'#E67E22',
  field_type:  '#F39C12',
};

const EDGE_LABELS = {
  contains:    'Contient',
  imports:     'Importe',
  inherits:    'Hérite',
  calls:       'Appelle',
  uses_type:   'Utilise type',
  external_dep:'Dép. externe',
  field_type:  'Type de champ',
};

// ── Build filter UI dynamically ─────────────────────────────────────────────
function buildFilters() {
  const nf = document.getElementById('node-filters');
  Object.entries(NODE_LABELS).forEach(([kind, label]) => {
    const l = document.createElement('label');
    l.className = 'filter-row';
    l.innerHTML = `<input type="checkbox" class="nf" value="${kind}" checked onchange="applyFilters()">
      <span class="dot" style="background:${NODE_COLORS[kind]||'#aaa'}"></span>${label}`;
    nf.appendChild(l);
  });

  const ef = document.getElementById('edge-filters');
  Object.entries(EDGE_LABELS).forEach(([kind, label]) => {
    const defaultOn = kind !== 'contains'; // contains hidden by default
    const l = document.createElement('label');
    l.className = 'filter-row';
    l.innerHTML = `<input type="checkbox" class="ef" value="${kind}" ${defaultOn?'checked':''} onchange="applyFilters()">
      <span class="dot" style="background:${EDGE_COLORS[kind]||'#aaa'}"></span>${EDGE_LABELS[kind]}`;
    ef.appendChild(l);
  });
}

// ── Label helpers ───────────────────────────────────────────────────────────
function makeLabel(n) {
  // Methods show Parent::name for clarity
  if (n.kind === 'method') {
    const sep = n.id.includes('::') ? '::' : '.';
    const parts = n.id.split(sep);
    if (parts.length >= 2) return parts.slice(-2).join('::');
  }
  return n.name;
}

function truncateLabel(text, max) {
  if (!text) return text;
  return text.length > max ? text.substring(0, max - 1) + '…' : text;
}

function kindDisplay(n) {
  const ext = (n.file || '').split('.').pop().toLowerCase();
  const isRust = ext === 'rs';
  const isPy   = ext === 'py';
  switch (n.kind) {
    case 'class':    return isRust ? 'struct / enum' : isPy ? 'class' : 'class / struct';
    case 'function': return isRust ? 'fn (libre)' : isPy ? 'fonction' : 'fonction';
    case 'method':   return isRust ? 'fn (impl)' : isPy ? 'méthode' : 'méthode';
    case 'module':   return isRust ? 'module' : isPy ? 'module' : 'fichier';
    default:         return n.kind;
  }
}

// ── Build vis datasets ──────────────────────────────────────────────────────
const cycleIds  = new Set((CYCLES || []).flat());
const diffAdded = new Set(DIFF ? DIFF.added_node_ids : []);
const diffRemov = new Set(DIFF ? DIFF.removed_node_ids : []);

const degreeMap = {};
RAW_EDGES.forEach(e => {
  degreeMap[e.source] = (degreeMap[e.source] || 0) + 1;
  degreeMap[e.target] = (degreeMap[e.target] || 0) + 1;
});
const maxDegree = Object.values(degreeMap).reduce((a,b) => Math.max(a,b), 1);

const allNodes = RAW_NODES.map(n => {
  // A node is truly a Module/File only when it has an actual source file.
  // External "modules" without a file are really types, functions, etc.
  let kind = n.kind;
  if (kind === 'module' && !n.file) {
    const last = (n.name || '').trim();
    if (/^[A-Z_][A-Z0-9_]{1,}$/.test(last)) kind = 'constant';        // ALL_CAPS
    else if (/^[A-Z]/.test(last))            kind = 'class';           // PascalCase
    else                                      kind = 'function';        // lowercase
  }

  let bg = NODE_COLORS[kind] || '#aaa';
  if (diffAdded.has(n.id))  bg = '#2ECC71';
  if (diffRemov.has(n.id))  bg = '#E74C3C';
  const isCycle = cycleIds.has(n.id);
  const deg = degreeMap[n.id] || 1;
  return {
    id:       n.id,
    label:    truncateLabel(makeLabel(n), 30),
    title:    `<b>${kindDisplay({...n, kind})}</b><br>${n.id}<br><small>${n.file || '(externe)'}:${n.line}</small>`,
    color: {
      background: bg,
      border:     isCycle ? '#ff4444' : darken(bg),
      highlight:  { background: bg, border: '#ffffff' },
    },
    borderWidth: isCycle ? 3 : 1,
    shape: kind === 'module'   ? 'hexagon'
         : kind === 'class'    ? 'square'
         : kind === 'constant' ? 'diamond' : 'dot',
    font:   { color: '#ffffff', size: 13 },
    value:  Math.sqrt(deg),
    mass:   1 + 4 * Math.sqrt(deg / maxDegree),
    _kind:  kind,
    _ext:   n.is_external,
    _file:  n.file,
    _line:  n.line,
    _docstr: n.docstring,
  };
});

const allEdges = RAW_EDGES.map((e, i) => ({
  id:    i,
  from:  e.source,
  to:    e.target,
  label: e.kind,
  color: { color: EDGE_COLORS[e.kind] || '#aaa', highlight: '#fff', opacity: .8 },
  font:  { color: '#888', size: 9, align: 'middle' },
  arrows:'to',
  _kind: e.kind,
}));

// Persistent DataSets — filters and focus only toggle the `hidden` flag, never replace data.
// Component mode is the only case that calls network.setData() with a lightweight subset.
const nodesDS = new vis.DataSet(allNodes);
const edgesDS = new vis.DataSet(allEdges);

// ── Settings state (declared here so network init can use them) ───────────────
let minNodeSize    = 15;
let nodeRatio      = 13;
let repulsionForce = 50;
let springForce    = 5;

// ── Network init ────────────────────────────────────────────────────────────
const container = document.getElementById('mynetwork');
const network = new vis.Network(container,
  { nodes: nodesDS, edges: edgesDS },
  {
    nodes: {
      scaling: { min: 15, max: 200, label: { enabled: true, min: 10, max: 32 } },
    },
    physics: {
      solver: 'forceAtlas2Based',
      forceAtlas2Based: { gravitationalConstant: -repulsionForce, springLength: 140, springConstant: springForce / 100, damping: 0.4 },
      stabilization: { iterations: 150 },
    },
    interaction: { hover: true, multiselect: false },
    layout: { improvedLayout: false },
  }
);

network.on('stabilizationProgress', p =>
  setStatus(`Stabilisation… ${Math.round(p.iterations/p.total*100)}%`));
network.on('stabilizationIterationsDone', () =>
  setStatus('Prêt — double-clic pour focus · clic droit pour options'));

// ── Selection → detail panel ────────────────────────────────────────────────
network.on('selectNode', params => {
  if (params.nodes.length) showDetail(params.nodes[0]);
});
network.on('deselectNode', () => {
  document.getElementById('detail-panel').innerHTML =
    '<p style="color:#555;font-size:12px">Sélectionnez un nœud</p>';
});

// ── Double-click → focus ────────────────────────────────────────────────────
network.on('doubleClick', params => {
  if (!params.nodes.length) { resetFocus(); return; }
  focusNode(params.nodes[0]);
});

// ── Stats box ───────────────────────────────────────────────────────────────
(function populateStats() {
  const s = STATS;
  const cycleCount = (CYCLES || []).length;
  const cycleLink  = cycleCount > 0
    ? `<span class="cycle-link" onclick="highlightCycles()">${cycleCount} cycle(s) ⚠</span>`
    : `<span style="color:#2ECC71">0 cycle</span>`;
  document.getElementById('stats-box').innerHTML = `
    <div>Nœuds: <b>${s.node_count}</b> &nbsp; Liens: <b>${s.edge_count}</b></div>
    <div>Modules: ${s.module_count} · Struct/Classes: ${s.class_count}</div>
    <div>Fn libres: ${s.function_count} · Méthodes: ${s.method_count}</div>
    <div>Externes: <span style="color:#E67E22">${s.external_count}</span></div>
    <div>${cycleLink} &nbsp; Composantes: ${(COMPONENTS||[]).length}</div>
  `;
})();

(function populateMetrics() {
  const box = document.getElementById('metrics-box');
  if (!METRICS || !METRICS.length) { box.innerHTML='—'; return; }
  box.innerHTML = METRICS.map(m => {
    const short = m.id.split(/[.:\/\\]+/).pop() || m.id;
    return `<div class="metric-row">
      <span class="mn">${short}</span>
      <br>↓${m.in_degree} ↑${m.out_degree} — couplage: <b>${(m.score*100).toFixed(1)}%</b>
    </div>`;
  }).join('');
})();

// ── showDetail ──────────────────────────────────────────────────────────────
function showDetail(id) {
  const n = RAW_NODES.find(x => x.id === id);
  if (!n) return;
  const inc = RAW_EDGES.filter(e => e.target === id);
  const out = RAW_EDGES.filter(e => e.source === id);

  const inHtml = inc.map(e =>
    `<div class="edge-item" onclick="jumpTo('${e.source}')">
      <span style="color:${EDGE_COLORS[e.kind]||'#aaa'}">[${e.kind}]</span>
      ${e.source.split(/[.:\/\\]+/).pop()}
    </div>`).join('') || '<div style="color:#555;font-size:11px">aucun</div>';

  const outHtml = out.map(e =>
    `<div class="edge-item" onclick="jumpTo('${e.target}')">
      <span style="color:${EDGE_COLORS[e.kind]||'#aaa'}">[${e.kind}]</span>
      ${e.target.split(/[.:\/\\]+/).pop()}
    </div>`).join('') || '<div style="color:#555;font-size:11px">aucun</div>';

  document.getElementById('detail-panel').innerHTML = `
    <h4>${makeLabel(n)}</h4>
    <span class="kind-tag" style="background:${NODE_COLORS[n.kind]||'#aaa'}22;color:${NODE_COLORS[n.kind]||'#aaa'};border:1px solid ${NODE_COLORS[n.kind]||'#aaa'}">
      ${kindDisplay(n)}
    </span>
    <div style="font-size:11px;color:#7f8c8d;margin-bottom:6px;word-break:break-all">${n.file}:${n.line}</div>
    ${n.docstring ? `<div style="font-size:11px;color:#aaa;margin-bottom:6px;font-style:italic">"${n.docstring}"</div>` : ''}
    <div style="font-size:11px;color:#a0c4ff;margin-bottom:3px">↓ Entrants (${inc.length})</div>
    ${inHtml}
    <div style="font-size:11px;color:#a0c4ff;margin:8px 0 3px">↑ Sortants (${out.length})</div>
    ${outHtml}
  `;
}

function jumpTo(id) {
  const node = nodesDS.get(id);
  if (node && node.hidden) nodesDS.update({ id, hidden: false });
  network.selectNodes([id]);
  network.fit({ nodes: [id], animation: { duration: 400 } });
  showDetail(id);
}

// ── Focus with BFS depth ────────────────────────────────────────────────────
let focusActive = false;

function focusNode(id) {
  if (compMode) return;
  const depth = parseInt(document.getElementById('focus-depth').value) || 1;
  const visited = new Set([id]);
  let frontier = [id];

  for (let d = 0; d < depth; d++) {
    const next = [];
    for (const nid of frontier) {
      RAW_EDGES.forEach(e => {
        if (e.source === nid && !visited.has(e.target)) { visited.add(e.target); next.push(e.target); }
        if (e.target === nid && !visited.has(e.source)) { visited.add(e.source); next.push(e.source); }
      });
    }
    frontier = next;
    if (!frontier.length) break;
  }

  nodesDS.update(allNodes.map(n => ({ id: n.id, hidden: !visited.has(n.id) })));
  edgesDS.update(allEdges.map(e => ({ id: e.id, hidden: !visited.has(e.from) || !visited.has(e.to) })));
  focusActive = true;
  setStatus(`Focus: ${id.split(/[.:\/\\]+/).pop()} (prof. ${depth}) — double-clic vide pour tout afficher`);
}

function resetFocus() {
  focusActive = false;
  if (compMode) { exitCompMode(); return; }
  applyFilters();
}

// ── Cycle highlighting ──────────────────────────────────────────────────────
function highlightCycles() {
  if (!CYCLES || !CYCLES.length) {
    setStatus('Aucun cycle détecté');
    return;
  }
  const ids = [...new Set(CYCLES.flat())];
  network.selectNodes(ids);
  network.fit({ nodes: ids, animation: { duration: 600 } });
  setStatus(`${CYCLES.length} cycle(s) — ${ids.length} nœuds impliqués (en rouge dans le graphe)`);
}

// ── Filters ─────────────────────────────────────────────────────────────────
function applyFilters() {
  if (compMode) return;  // comp mode manages its own view
  focusActive = false;

  const activeKinds = new Set([...document.querySelectorAll('.nf:checked')].map(c => c.value));
  const activeEdgeK = new Set([...document.querySelectorAll('.ef:checked')].map(c => c.value));
  const showExt     = document.getElementById('tog-ext').checked;
  const search      = document.getElementById('search-input').value.toLowerCase().trim();

  nodesDS.update(allNodes.map(n => {
    let hidden = false;
    if (!activeKinds.has(n._kind))  hidden = true;
    if (!showExt && n._ext)         hidden = true;
    if (search && !n.label.toLowerCase().includes(search) && !n.id.toLowerCase().includes(search)) hidden = true;
    if (fsPathFilter !== null && (!n._file || !n._file.startsWith(fsPathFilter))) hidden = true;
    return { id: n.id, hidden };
  }));

  const vis_set = new Set();
  nodesDS.forEach(n => { if (!n.hidden) vis_set.add(n.id); });

  edgesDS.update(allEdges.map(e => ({
    id: e.id,
    hidden: !activeEdgeK.has(e._kind) || !vis_set.has(e.from) || !vis_set.has(e.to),
  })));
}

// ── Layout & view state ───────────────────────────────────────────────────────
let physicsOn      = true;
let hierOn         = false;
let clusterOn      = false;
let compMode       = false;
let currentCompIdx = 0;

function openSettings() {
  document.getElementById('set-min-size').value        = minNodeSize;
  document.getElementById('lbl-min-size').textContent  = minNodeSize;
  document.getElementById('set-ratio').value           = nodeRatio;
  document.getElementById('lbl-ratio').textContent     = nodeRatio;
  document.getElementById('set-repulsion').value       = repulsionForce;
  document.getElementById('lbl-repulsion').textContent = repulsionForce;
  document.getElementById('set-spring').value          = springForce;
  document.getElementById('lbl-spring').textContent    = springForce;
  document.getElementById('settings-modal').classList.add('open');
}
function closeSettings() {
  document.getElementById('settings-modal').classList.remove('open');
}
function applySettings() {
  minNodeSize    = parseInt(document.getElementById('set-min-size').value);
  nodeRatio      = parseInt(document.getElementById('set-ratio').value);
  repulsionForce = parseInt(document.getElementById('set-repulsion').value);
  springForce    = parseInt(document.getElementById('set-spring').value);
  const maxNodeSize   = minNodeSize * nodeRatio;
  const springConstant = springForce / 100;   // 0.01 … 0.50
  network.setOptions({
    nodes:   { scaling: { min: minNodeSize, max: maxNodeSize, label: { enabled: true, min: 10, max: 32 } } },
    physics: { forceAtlas2Based: { gravitationalConstant: -repulsionForce, springConstant } },
  });
  closeSettings();
  setStatus(`Paramètres — taille ${minNodeSize}–${maxNodeSize}px · répulsion ${repulsionForce} · liens ${springForce}`);
}

// ── Rebuild graph from visible elements ────────────────────────────────────────
function rebuildGraph() {
  const visIds = new Set();
  nodesDS.forEach(n => { if (!n.hidden) visIds.add(n.id); });
  const subNodes = nodesDS.get({ filter: n => !n.hidden });
  const subEdges = edgesDS.get({ filter: e => !e.hidden && visIds.has(e.from) && visIds.has(e.to) });
  network.setData({ nodes: new vis.DataSet(subNodes), edges: new vis.DataSet(subEdges) });
  setStatus(`↺ Graphe rechargé — ${subNodes.length} nœuds, ${subEdges.length} liens`);
}

// ── Reset all ────────────────────────────────────────────────────────────────
function resetAll() { location.reload(); }

// ── Simulation speed ──────────────────────────────────────────────────────────
function setSimSpeed(v) {
  document.getElementById('lbl-speed').textContent = v;
  const timestep = Math.min(v / 50, 1.0);   // 0.02 … 1.0
  network.setOptions({ physics: { timestep, adaptiveTimestep: false } });
  if (physicsOn && !hierOn) network.startSimulation();
}

function topLevel(id) {
  const sep = id.includes('::') ? '::' : '.';
  return id.split(sep)[0];
}

function updateLayoutBtns() {
  document.getElementById('btn-graph').classList.toggle('is-active', !hierOn && !compMode);
  document.getElementById('btn-hier').classList.toggle('is-active', hierOn);
  document.getElementById('btn-clust').classList.toggle('is-active', clusterOn);
  document.getElementById('btn-comp').classList.toggle('is-active', compMode);
  const cb = document.getElementById('tog-force');
  if (cb) cb.checked = physicsOn;
}

// ── Graph layout (free force-directed) ───────────────────────────────────────
function toggleGraph() {
  if (compMode) { exitCompMode(); return; }
  if (hierOn) {
    hierOn = false;
    physicsOn = true;
    network.setOptions({
      layout: { hierarchical: { enabled: false } },
      physics: { solver: 'forceAtlas2Based', enabled: true },
    });
    updateLayoutBtns();
  }
  // already in graph mode → nothing to do
}

// ── Force toggle (physics on/off checkbox) ────────────────────────────────────
function toggleForce() {
  if (compMode) { exitCompMode(); return; }
  const cb = document.getElementById('tog-force');
  physicsOn = cb ? cb.checked : !physicsOn;
  network.setOptions({ physics: { enabled: physicsOn } });
  updateLayoutBtns();
}

// ── Hierarchical layout toggle ────────────────────────────────────────────────
function toggleHier() {
  if (compMode) { exitCompMode(); return; }
  hierOn = !hierOn;
  if (hierOn) {
    physicsOn = false;
    network.setOptions({
      layout: { hierarchical: { enabled: true, direction: 'UD', sortMethod: 'directed' } },
      physics: { enabled: false },
    });
  } else {
    physicsOn = true;
    network.setOptions({
      layout: { hierarchical: { enabled: false } },
      physics: { solver: 'forceAtlas2Based', enabled: true },
    });
  }
  updateLayoutBtns();
}

// ── Cluster toggle ────────────────────────────────────────────────────────────
function toggleCluster() {
  if (clusterOn) {
    clusterOn = false;
    // Restore full persistent datasets then re-apply current filters
    network.setData({ nodes: nodesDS, edges: edgesDS });
    applyFilters();
    updateLayoutBtns();
    return;
  }
  const modules = {};
  RAW_NODES.forEach(n => {
    const mod = topLevel(n.id);
    if (!modules[mod]) modules[mod] = [];
    modules[mod].push(n.id);
  });
  Object.entries(modules).forEach(([mod, ids]) => {
    if (ids.length < 2) return;
    network.cluster({
      joinCondition: node => ids.includes(node.id),
      clusterNodeProperties: {
        id:    'cluster_' + mod,
        label: `[ ${mod} (${ids.length}) ]`,
        color: { background: '#0f3460', border: '#4A90D9' },
        font:  { color: '#4A90D9', size: 13 },
        shape: 'box',
      },
    });
  });
  clusterOn = true;
  updateLayoutBtns();
  setStatus('Clustérisé par module — re-cliquer "Cluster" pour déplier · clic sur un cluster pour l\'ouvrir');
}

// ── Component navigator mode ──────────────────────────────────────────────────
// Compos. is a VIEW MODE: it replaces the base graph entirely.
// The only way to exit is via Force, Hiéra., or Tout voir.
function toggleCompMode() {
  if (compMode) { exitCompMode(); return; }
  const comps = COMPONENTS || [];
  if (!comps.length) { setStatus('Aucune composante connexe détectée'); return; }
  compMode = true;
  currentCompIdx = 0;
  document.getElementById('comp-nav').style.display = 'flex';
  network.setOptions({ physics: { enabled: false } });
  showComponent(0);
  updateLayoutBtns();
}

function exitCompMode() {
  compMode = false;
  document.getElementById('comp-nav').style.display = 'none';
  // Restore persistent datasets, then re-apply filters + physics state
  network.setData({ nodes: nodesDS, edges: edgesDS });
  network.setOptions({ physics: { enabled: physicsOn && !hierOn } });
  applyFilters();
  updateLayoutBtns();
}

function showComponent(idx) {
  const comps = COMPONENTS || [];
  currentCompIdx = Math.max(0, Math.min(idx, comps.length - 1));
  const compNodeIds = new Set(comps[currentCompIdx]);
  // Build lightweight subset — only component nodes rendered by vis.js
  const compNodes = allNodes.filter(n => compNodeIds.has(n.id));
  const compEdges = allEdges.filter(e => compNodeIds.has(e.from) && compNodeIds.has(e.to));
  network.setData({ nodes: new vis.DataSet(compNodes), edges: new vis.DataSet(compEdges) });
  setTimeout(() => network.fit({ animation: { duration: 400 } }), 50);
  updateCompNav(comps);
  setStatus(`Composante ${currentCompIdx + 1} / ${comps.length} — ${comps[currentCompIdx].length} nœud(s)`);
}

function updateCompNav(comps) {
  comps = comps || COMPONENTS || [];
  document.getElementById('comp-label').textContent =
    `${currentCompIdx + 1} / ${comps.length}  (${(comps[currentCompIdx] || []).length} nœuds)`;
  document.getElementById('comp-prev').disabled = currentCompIdx <= 0;
  document.getElementById('comp-next').disabled = currentCompIdx >= comps.length - 1;
}

// ── Filesystem tree navigator ─────────────────────────────────────────────────
// Builds a virtual tree from the _file paths of real (non-external) nodes.
// A node is Module/File only when it has an actual _file in the tree.
// Clicking a directory drills into it; clicking a leaf filters to that path.

let fsTree    = null;   // root tree node
let fsPathFilter = null; // null = all, string = filter nodes whose _file starts with this

function buildFsTree() {
  const root = { children: {}, count: 0 };
  RAW_NODES.forEach(n => {
    if (!n.file) return;  // external nodes have no file (RAW_NODES uses .file, not ._file)
    const parts = n.file.split('/').filter(Boolean);
    let node = root;
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      if (!node.children[part]) {
        node.children[part] = {
          name: part,
          path: parts.slice(0, i + 1).join('/'),
          isDir: i < parts.length - 1,
          children: {},
          count: 0,
        };
      }
      node.children[part].count++;
      node = node.children[part];
    }
  });
  return root;
}

function findFsNode(path) {
  if (!path) return fsTree;
  let node = fsTree;
  for (const part of path.split('/').filter(Boolean)) {
    node = node.children[part];
    if (!node) return null;
  }
  return node;
}

function renderFsNav(treeNode, currentPath) {
  const breadEl = document.getElementById('fs-breadcrumb');
  const navEl   = document.getElementById('fs-nav-box');

  // Build breadcrumb
  if (currentPath) {
    const parts = currentPath.split('/').filter(Boolean);
    let bc = `<span class="fs-bc-link" onclick="navFs(null)">Tous</span>`;
    let acc = '';
    parts.forEach((p, i) => {
      acc = acc ? acc + '/' + p : p;
      const cap = acc;
      bc += ` › ${i === parts.length - 1
        ? `<b style="color:#c0c0c0">${p}</b>`
        : `<span class="fs-bc-link" onclick="navFs('${cap}')">${p}</span>`}`;
    });
    breadEl.innerHTML = bc;
  } else {
    breadEl.innerHTML = '<b style="color:#4A90D9">Tous</b>';
  }

  const children = treeNode ? treeNode.children : {};
  const entries = Object.values(children).sort((a, b) => {
    if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
    return a.name.localeCompare(b.name);
  });

  if (!entries.length) { navEl.innerHTML = '<span style="font-size:10px;color:#555">—</span>'; return; }

  navEl.innerHTML = entries.map(e => {
    const active = fsPathFilter === e.path ? ' is-active' : '';
    const dirCls = e.isDir ? ' fs-dir' : '';
    const label  = (e.isDir ? '▶ ' : '') + e.name;
    const click  = e.isDir
      ? `navFs('${e.path}')`
      : `filterFs('${e.path}')`;
    return `<button class="fs-btn${dirCls}${active}" onclick="${click}" title="${e.path} (${e.count} nœuds)">${label}</button>`;
  }).join('');
}

function navFs(path) {
  const node = findFsNode(path);
  // When entering a dir that has only one child dir, skip to it automatically
  let target = node;
  let targetPath = path;
  while (target && target.children) {
    const kids = Object.values(target.children);
    if (kids.length === 1 && kids[0].isDir && !fsPathFilter?.startsWith(kids[0].path)) {
      target = kids[0];
      targetPath = kids[0].path;
    } else break;
  }
  fsPathFilter = targetPath || null;
  applyFilters();
  renderFsNav(target, targetPath || null);
}

function filterFs(path) {
  fsPathFilter = path;
  applyFilters();
  // Keep showing the parent dir level
  const parentPath = path.includes('/') ? path.substring(0, path.lastIndexOf('/')) : null;
  renderFsNav(findFsNode(parentPath), parentPath);
}

// ── Duplicate names debug ────────────────────────────────────────────────────
let dupPanelOpen = false;
function toggleDupPanel() {
  const box = document.getElementById('dup-box');
  dupPanelOpen = !dupPanelOpen;
  if (!dupPanelOpen) { box.style.display = 'none'; return; }

  // Group RAW_NODES by display name
  const byName = {};
  RAW_NODES.forEach(n => {
    const label = n.name || n.id;
    if (!byName[label]) byName[label] = [];
    byName[label].push(n);
  });

  const dups = Object.entries(byName)
    .filter(([, nodes]) => nodes.length > 1)
    .sort((a, b) => b[1].length - a[1].length);

  if (!dups.length) {
    box.innerHTML = '<div style="color:#2ECC71">Aucun doublon de nom détecté</div>';
  } else {
    box.innerHTML = dups.map(([name, nodes]) => `
      <div style="border-bottom:1px solid #0f3460;padding:4px 0">
        <b style="color:#E67E22">${name}</b> (${nodes.length}×)
        ${nodes.map(n => `
          <div style="padding-left:8px;color:#888">
            <span style="color:${NODE_COLORS[n.kind]||'#aaa'}">${n.kind}</span>
            ${n.is_external ? '<span style="color:#E74C3C">[ext]</span>' : ''}
            — <span style="color:#a0c4ff;cursor:pointer" onclick="jumpTo('${n.id}')">${n.id}</span>
          </div>`).join('')}
      </div>`).join('');
  }
  box.style.display = 'block';
}

// ── PNG export ──────────────────────────────────────────────────────────────
function exportPNG() {
  const graphCanvas = container.querySelector('canvas');
  if (!graphCanvas) { alert('Canvas non disponible'); return; }

  // ── Title ────────────────────────────────────────────────────────────────
  // Use active folder filter, fallback to last segment of ROOT_DIR
  let titleText;
  if (fsPathFilter) {
    titleText = fsPathFilter.split('/').filter(Boolean).pop() || fsPathFilter;
  } else {
    const rootParts = ROOT_DIR.split('/').filter(Boolean);
    titleText = rootParts.length > 0 ? rootParts[rootParts.length - 1] : 'code_graph';
  }

  // ── Legend items — only visible node kinds ───────────────────────────────
  const visibleKinds = new Set();
  nodesDS.forEach(n => { if (!n.hidden) visibleKinds.add(n._kind); });

  const kindMeta = {
    module:   { label: 'Module',    color: '#4A90D9', shape: 'hexagon' },
    class:    { label: 'Classe',    color: '#E67E22', shape: 'square'  },
    function: { label: 'Fonction',  color: '#2ECC71', shape: 'dot'     },
    method:   { label: 'Méthode',   color: '#1ABC9C', shape: 'dot'     },
    constant: { label: 'Constante', color: '#9B59B6', shape: 'diamond' },
    external: { label: 'Externe',   color: '#888888', shape: 'dot'     },
  };
  const legendItems = Object.entries(kindMeta).filter(([k]) => visibleKinds.has(k));

  // ── Composite canvas ─────────────────────────────────────────────────────
  // Use CSS pixels (not raw canvas pixels) to avoid DPR bleed on HiDPI screens
  const dpr   = window.devicePixelRatio || 1;
  const gw    = Math.round(graphCanvas.width  / dpr);
  const gh    = Math.round(graphCanvas.height / dpr);

  const PAD       = 20;
  const TITLE_H   = 40;
  const LEG_H     = legendItems.length > 0 ? 36 : 0;
  const totalW    = gw;
  const totalH    = TITLE_H + gh + LEG_H + PAD * 3;

  const out = document.createElement('canvas');
  out.width  = totalW;
  out.height = totalH;
  const ctx = out.getContext('2d');
  ctx.imageSmoothingEnabled = false;

  // background
  ctx.fillStyle = '#1a1a2e';
  ctx.fillRect(0, 0, totalW, totalH);

  // title
  ctx.fillStyle = '#ffffff';
  ctx.font = 'bold 22px sans-serif';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(titleText, totalW / 2, PAD + TITLE_H / 2);

  // graph — draw at CSS size to neutralise the DPR scaling
  ctx.drawImage(graphCanvas, 0, TITLE_H + PAD, gw, gh);

  // legend
  if (legendItems.length > 0) {
    const iconR  = 7;
    const itemW  = Math.floor(totalW / legendItems.length);
    const legY   = TITLE_H + PAD + gh + PAD / 2 + LEG_H / 2;

    legendItems.forEach(([, meta], i) => {
      const cx = itemW * i + itemW / 2;
      ctx.fillStyle = meta.color;

      if (meta.shape === 'hexagon') {
        // draw hexagon
        ctx.beginPath();
        for (let a = 0; a < 6; a++) {
          const ang = Math.PI / 180 * (60 * a - 30);
          const px = cx - 28 + iconR * Math.cos(ang);
          const py = legY + iconR * Math.sin(ang);
          a === 0 ? ctx.moveTo(px, py) : ctx.lineTo(px, py);
        }
        ctx.closePath();
        ctx.fill();
      } else if (meta.shape === 'square') {
        ctx.fillRect(cx - 28 - iconR, legY - iconR, iconR * 2, iconR * 2);
      } else if (meta.shape === 'diamond') {
        ctx.beginPath();
        ctx.moveTo(cx - 28, legY - iconR);
        ctx.lineTo(cx - 28 + iconR, legY);
        ctx.lineTo(cx - 28, legY + iconR);
        ctx.lineTo(cx - 28 - iconR, legY);
        ctx.closePath();
        ctx.fill();
      } else {
        ctx.beginPath();
        ctx.arc(cx - 28, legY, iconR, 0, Math.PI * 2);
        ctx.fill();
      }

      ctx.fillStyle = '#dddddd';
      ctx.font = '13px sans-serif';
      ctx.textAlign = 'left';
      ctx.textBaseline = 'middle';
      ctx.fillText(meta.label, cx - 28 + iconR + 5, legY);
    });
  }

  // download
  const a = document.createElement('a');
  a.href = out.toDataURL('image/png');
  a.download = titleText.toLowerCase().replace(/\s+/g, '_') + '_graph.png';
  a.click();
}

// ── Right-click context menu ────────────────────────────────────────────────
let ctxNode = null;

container.addEventListener('contextmenu', e => {
  e.preventDefault();
  hideCtx();
  const pos = { x: e.offsetX, y: e.offsetY };
  const nodeId = network.getNodeAt(pos);
  if (!nodeId) return;
  ctxNode = RAW_NODES.find(n => n.id === nodeId);
  if (!ctxNode) return;
  const menu = document.getElementById('ctx-menu');
  menu.style.display = 'block';
  menu.style.left = e.clientX + 'px';
  menu.style.top  = e.clientY + 'px';
});

document.addEventListener('click', hideCtx);

function hideCtx() {
  document.getElementById('ctx-menu').style.display = 'none';
}

document.getElementById('ctx-open').addEventListener('click', () => {
  if (!ctxNode) return;
  // Build file:/// URL pointing to the containing directory
  const full = (ROOT_DIR + '/' + ctxNode._file).replace(/\\/g, '/').replace(/\/+/g, '/');
  const dir  = full.substring(0, full.lastIndexOf('/'));
  const url  = 'file:///' + dir.replace(/^\//, '');
  window.open(url);
  hideCtx();
});

document.getElementById('ctx-focus-btn').addEventListener('click', () => {
  if (!ctxNode) return;
  focusNode(ctxNode.id);
  hideCtx();
});

document.getElementById('ctx-hide').addEventListener('click', () => {
  if (!ctxNode) return;
  nodesDS.update({ id: ctxNode.id, hidden: true });
  edgesDS.update(
    allEdges.filter(e => e.from === ctxNode.id || e.to === ctxNode.id)
            .map(e => ({ id: e.id, hidden: true }))
  );
  hideCtx();
  setStatus(`Masqué : ${ctxNode.id}`);
});

document.getElementById('ctx-copy').addEventListener('click', () => {
  if (!ctxNode) return;
  navigator.clipboard.writeText(ctxNode.id).catch(() => {});
  setStatus(`Copié : ${ctxNode.id}`);
  hideCtx();
});

// ── Status bar ──────────────────────────────────────────────────────────────
function setStatus(msg) {
  document.getElementById('status-bar').textContent = msg;
}

// ── Helpers ─────────────────────────────────────────────────────────────────
function darken(hex) {
  try {
    const n = parseInt(hex.slice(1), 16);
    const r = Math.max(0, (n>>16) - 40);
    const g = Math.max(0, ((n>>8)&0xff) - 40);
    const b = Math.max(0, (n&0xff) - 40);
    return '#' + [r,g,b].map(x=>x.toString(16).padStart(2,'0')).join('');
  } catch(_) { return '#333'; }
}

// ── Init ────────────────────────────────────────────────────────────────────
buildFilters();
fsTree = buildFsTree();
renderFsNav(fsTree, null);
applyFilters();
"#;

// ─── Rust export function ─────────────────────────────────────────────────────

pub fn export_html(
    graph: &CodeGraph,
    out: &Path,
    analysis_opt: Option<&GraphAnalysis>,
    diff_opt: Option<&GraphDiff>,
    root_dir: Option<&Path>,
) -> Result<()> {
    let sg = graph.to_serializable();
    let nodes_json   = serde_json::to_string(&sg.nodes)?;
    let edges_json   = serde_json::to_string(&sg.edges)?;
    let stats_json   = serde_json::to_string(&sg.stats)?;

    let metrics_json = if let Some(a) = analysis_opt {
        let mut sorted: Vec<_> = a.metrics.iter()
            .map(|(id, m)| (id, m.coupling_score, m.in_degree, m.out_degree))
            .collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top: Vec<serde_json::Value> = sorted.iter().take(5).map(|(id, score, ind, outd)| {
            serde_json::json!({ "id": id, "score": score, "in_degree": ind, "out_degree": outd })
        }).collect();
        serde_json::to_string(&top)?
    } else { "[]".into() };

    let cycles_json = if let Some(a) = analysis_opt {
        serde_json::to_string(&a.cycles)?
    } else { "[]".into() };

    let components_json = if let Some(a) = analysis_opt {
        serde_json::to_string(&a.components)?
    } else { "[]".into() };

    let diff_json = if let Some(d) = diff_opt {
        let added:   Vec<&str> = d.added_nodes.iter().map(|n| n.id.as_str()).collect();
        let removed: Vec<&str> = d.removed_nodes.iter().map(|n| n.id.as_str()).collect();
        serde_json::json!({ "added_node_ids": added, "removed_node_ids": removed }).to_string()
    } else { "null".into() };

    let root_dir_str = root_dir
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();

    // Data injection — only this section uses format! with named params
    let data = format!(
        "const RAW_NODES={nodes};\nconst RAW_EDGES={edges};\nconst STATS={stats};\n\
         const METRICS={metrics};\nconst DIFF={diff};\nconst CYCLES={cycles};\n\
         const COMPONENTS={comps};\nconst ROOT_DIR='{root}';",
        nodes   = nodes_json,
        edges   = edges_json,
        stats   = stats_json,
        metrics = metrics_json,
        diff    = diff_json,
        cycles  = cycles_json,
        comps   = components_json,
        root    = root_dir_str,
    );

    let html = HTML_TEMPLATE
        .replace("__DATA__", &data)
        .replace("__APP__", APP_JS);

    std::fs::write(out, html)?;
    Ok(())
}
