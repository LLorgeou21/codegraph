use anyhow::Result;
use analysis::{GraphAnalysis, GraphDiff};
use core::CodeGraph;
use std::path::Path;

// ─── HTML template ────────────────────────────────────────────────────────────

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
.orphan-badge{font-size:10px;color:#E67E22;font-style:italic}
#structural-wrap{position:absolute;inset:0;overflow:hidden;background:#0d1117;cursor:grab;display:none;z-index:1}
#struct-svg{display:block;position:absolute;top:0;left:0;width:100%;height:100%;user-select:none}
.struct-toolbar{position:absolute;top:8px;right:8px;display:flex;gap:5px;z-index:10}
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
    <h3>Filtre couplage min <span id="lbl-coupling">0</span>%</h3>
    <input type="range" id="coupling-min" min="0" max="100" value="0"
           oninput="document.getElementById('lbl-coupling').textContent=this.value;applyFilters()">
    <div style="font-size:10px;color:#555;margin-top:2px">Masque les nœuds sous ce seuil</div>
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
      <button id="btn-graph"  class="layout-btn is-active" onclick="toggleGraph()"      title="Layout force-dirigé">Graph</button>
      <button id="btn-hier"   class="layout-btn"           onclick="toggleHier()"       title="Layout hiérarchique">Hiéra.</button>
      <button id="btn-clust"  class="layout-btn"           onclick="toggleCluster()"    title="Grouper par module">Cluster</button>
      <button id="btn-comp"   class="layout-btn"           onclick="toggleCompMode()"   title="Composante par composante">Compos.</button>
      <button id="btn-struct" class="layout-btn"           onclick="toggleStructural()" title="Vue structurelle imbriquée (dossier › fichier › classe › fn)">Struct.</button>
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
      <button class="layout-btn" onclick="resetAll()"     title="Réinitialiser">Reset</button>
      <button class="layout-btn" onclick="resetFocus()"   title="Tout afficher">Tout voir</button>
      <button class="layout-btn" onclick="exportPNG()"    title="Exporter PNG">PNG</button>
      <button class="layout-btn" onclick="exportSVG()"    title="Exporter SVG vectoriel">SVG</button>
      <button class="layout-btn" onclick="rebuildGraph()" title="Reconstruire">↺</button>
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
    <h3>Nœuds orphelins</h3>
    <div id="orphan-box" style="font-size:11px;color:#E67E22">—</div>
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

  <!-- STRUCTURAL VIEW (nested boxes) -->
  <div id="structural-wrap">
    <svg id="struct-svg"></svg>
    <div class="struct-toolbar">
      <button id="btn-struct-reset"  class="layout-btn" title="Réinitialiser le zoom">Reset zoom</button>
      <button id="btn-struct-export" class="layout-btn" title="Exporter en SVG">Export SVG</button>
    </div>
  </div>

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
      <label>Force des liens <span id="lbl-spring">5</span></label>
      <input type="range" id="set-spring" min="1" max="50" value="5"
             oninput="document.getElementById('lbl-spring').textContent=this.value">
    </div>
    <div class="setting-row">
      <label>Taille des cellules (vue struct.) <span id="lbl-struct-w">1400</span></label>
      <input type="range" id="set-struct-w" min="300" max="4000" step="100" value="1400"
             oninput="document.getElementById('lbl-struct-w').textContent=this.value">
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

const APP_JS: &str = r##"
// ── Kind metadata ──────────────────────────────────────────────────────────
// Palette : 7 teintes réparties sur la roue chromatique (~51° d'écart moyen)
// pour que chaque type soit immédiatement reconnaissable, même en vue structurelle.
//  module   ~205° bleu         class    ~  5° rouge
//  struct   ~ 54° jaune        function ~145° vert
//  method   ~285° violet       property ~168° sarcelle
//  constant ~ 28° orange
const NODE_COLORS = {
  module:   '#3498DB',   // bleu
  class:    '#E74C3C',   // rouge
  struct:   '#F1C40F',   // jaune  (was teal — now clearly distinct from green)
  function: '#2ECC71',   // vert émeraude
  method:   '#9B59B6',   // violet améthyste  (was orange — now far from green)
  property: '#1ABC9C',   // sarcelle
  constant: '#E67E22',   // orange
};

const NODE_LABELS = {
  module:   'Module / Fichier',
  class:    'Classe · Interface · Trait',
  struct:   'Struct · Enum',
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

// ── Build filter UI ─────────────────────────────────────────────────────────
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
    const defaultOn = kind !== 'contains';
    const l = document.createElement('label');
    l.className = 'filter-row';
    l.innerHTML = `<input type="checkbox" class="ef" value="${kind}" ${defaultOn?'checked':''} onchange="applyFilters()">
      <span class="dot" style="background:${EDGE_COLORS[kind]||'#aaa'}"></span>${EDGE_LABELS[kind]}`;
    ef.appendChild(l);
  });
}

// ── Label helpers ───────────────────────────────────────────────────────────
function makeLabel(n) {
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
  const isTs   = ext === 'ts' || ext === 'tsx';
  switch (n.kind) {
    case 'class':    return isRust ? 'trait / enum' : isPy ? 'class' : isTs ? 'class / interface' : 'class';
    case 'struct':   return isRust ? 'struct / enum' : isTs ? 'enum' : 'struct';
    case 'function': return isRust ? 'fn (libre)' : isPy ? 'fonction' : 'fonction';
    case 'method':   return isRust ? 'fn (impl)' : isPy ? 'méthode' : 'méthode';
    case 'module':   return isRust ? 'module' : isPy ? 'module' : isTs ? 'module TS/JS' : 'fichier';
    default:         return n.kind;
  }
}

// ── Métriques par nœud (map complet envoyé par Rust) ───────────────────────
// METRICS_ALL : { node_id: { score, in_degree, out_degree, dit } }
const metricsMap = METRICS_ALL || {};

// ── Build vis datasets ──────────────────────────────────────────────────────
const cycleIds  = new Set((CYCLES || []).flat());
const diffAdded = new Set(DIFF ? DIFF.added_node_ids : []);
const diffRemov = new Set(DIFF ? DIFF.removed_node_ids : []);
const orphanIds = new Set(ORPHAN_NODES || []);

const degreeMap = {};
RAW_EDGES.forEach(e => {
  degreeMap[e.source] = (degreeMap[e.source] || 0) + 1;
  degreeMap[e.target] = (degreeMap[e.target] || 0) + 1;
});
const maxDegree = Object.values(degreeMap).reduce((a,b) => Math.max(a,b), 1);

const allNodes = RAW_NODES.map(n => {
  let kind = n.kind;
  if (kind === 'module' && !n.file) {
    const last = (n.name || '').trim();
    if (/^[A-Z_][A-Z0-9_]{1,}$/.test(last)) kind = 'constant';
    else if (/^[A-Z]/.test(last))            kind = 'class';
    else                                      kind = 'function';
  }

  let bg = NODE_COLORS[kind] || '#aaa';
  if (diffAdded.has(n.id))  bg = '#2ECC71';
  if (diffRemov.has(n.id))  bg = '#E74C3C';
  const isCycle  = cycleIds.has(n.id);
  const isOrphan = orphanIds.has(n.id);
  const deg = degreeMap[n.id] || 1;

  return {
    id:       n.id,
    label:    truncateLabel(makeLabel(n), 30),
    title:    `<b>${kindDisplay({...n, kind})}</b><br>${n.id}<br><small>${n.file || '(externe)'}:${n.line}</small>${isOrphan ? '<br><i style="color:#E67E22">nœud orphelin</i>' : ''}`,
    color: {
      background: bg,
      border:     isCycle ? '#ff4444' : darken(bg),
      highlight:  { background: bg, border: '#ffffff' },
    },
    borderWidth: isCycle ? 3 : isOrphan ? 2 : 1,
    borderDashes: isOrphan ? [4, 2] : false,
    shape: kind === 'module'   ? 'hexagon'
         : kind === 'class'    ? 'square'
         : kind === 'struct'   ? 'diamond'
         : kind === 'constant' ? 'triangleDown' : 'dot',
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

const nodesDS = new vis.DataSet(allNodes);
const edgesDS = new vis.DataSet(allEdges);

// ── Settings state ──────────────────────────────────────────────────────────
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
      forceAtlas2Based: {
        gravitationalConstant: -repulsionForce,
        springLength: 140,
        springConstant: springForce / 100,
        damping: 0.4,
      },
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

network.on('selectNode', params => {
  if (params.nodes.length) showDetail(params.nodes[0]);
});
network.on('deselectNode', () => {
  document.getElementById('detail-panel').innerHTML =
    '<p style="color:#555;font-size:12px">Sélectionnez un nœud</p>';
});
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
    <div>Modules: ${s.module_count} · Classes: ${s.class_count} · Structs: ${s.struct_count||0}</div>
    <div>Fn libres: ${s.function_count} · Méthodes: ${s.method_count}</div>
    <div>Externes: <span style="color:#E67E22">${s.external_count}</span></div>
    <div>${cycleLink} &nbsp; Composantes: ${(COMPONENTS||[]).length}</div>
  `;
})();

(function populateMetrics() {
  const box = document.getElementById('metrics-box');
  // METRICS_TOP : top-10 array from Rust
  if (!METRICS_TOP || !METRICS_TOP.length) { box.innerHTML='—'; return; }
  box.innerHTML = METRICS_TOP.map(m => {
    const short = m.id.split(/[.:\/\\]+/).pop() || m.id;
    return `<div class="metric-row">
      <span class="mn">${short}</span>
      <br>↓${m.in_degree} ↑${m.out_degree} DIT:${m.dit||0} — <b>${(m.score*100).toFixed(1)}%</b>
    </div>`;
  }).join('');
})();

(function populateOrphans() {
  const box = document.getElementById('orphan-box');
  const orphans = ORPHAN_NODES || [];
  if (!orphans.length) { box.innerHTML = '<span style="color:#2ECC71">Aucun nœud orphelin</span>'; return; }
  box.innerHTML = orphans.map(id => {
    const short = id.split(/[.:\/\\]+/).pop() || id;
    return `<span style="cursor:pointer;color:#E67E22" onclick="jumpTo('${id}')" title="${id}">${short}</span>`;
  }).join(', ');
})();

// ── showDetail ──────────────────────────────────────────────────────────────
function showDetail(id) {
  const n = RAW_NODES.find(x => x.id === id);
  if (!n) return;
  const inc = RAW_EDGES.filter(e => e.target === id);
  const out = RAW_EDGES.filter(e => e.source === id);
  const m   = metricsMap[id];

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

  const metricsHtml = m
    ? `<div style="font-size:11px;color:#a0c4ff;margin:6px 0 2px">Métriques</div>
       <div style="font-size:11px;background:#0f3460;border-radius:3px;padding:5px">
         ↓ ${m.in_degree} entrant(s) &nbsp; ↑ ${m.out_degree} sortant(s)<br>
         Couplage: <b>${(m.score*100).toFixed(1)}%</b> &nbsp; DIT: <b>${m.dit||0}</b>
         ${orphanIds.has(id) ? '<br><span class="orphan-badge">⚠ nœud orphelin</span>' : ''}
       </div>`
    : '';

  document.getElementById('detail-panel').innerHTML = `
    <h4>${makeLabel(n)}</h4>
    <span class="kind-tag" style="background:${NODE_COLORS[n.kind]||'#aaa'}22;color:${NODE_COLORS[n.kind]||'#aaa'};border:1px solid ${NODE_COLORS[n.kind]||'#aaa'}">
      ${kindDisplay(n)}
    </span>
    <div style="font-size:11px;color:#7f8c8d;margin-bottom:6px;word-break:break-all">${n.file}:${n.line}</div>
    ${n.docstring ? `<div style="font-size:11px;color:#aaa;margin-bottom:6px;font-style:italic">"${n.docstring}"</div>` : ''}
    ${metricsHtml}
    <div style="font-size:11px;color:#a0c4ff;margin-top:8px;margin-bottom:3px">↓ Entrants (${inc.length})</div>
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
  if (!CYCLES || !CYCLES.length) { setStatus('Aucun cycle détecté'); return; }
  const ids = [...new Set(CYCLES.flat())];
  network.selectNodes(ids);
  network.fit({ nodes: ids, animation: { duration: 600 } });
  setStatus(`${CYCLES.length} cycle(s) — ${ids.length} nœuds impliqués`);
}

// ── Filters ─────────────────────────────────────────────────────────────────
function applyFilters() {
  if (compMode) return;
  focusActive = false;

  const activeKinds  = new Set([...document.querySelectorAll('.nf:checked')].map(c => c.value));
  const activeEdgeK  = new Set([...document.querySelectorAll('.ef:checked')].map(c => c.value));
  const showExt      = document.getElementById('tog-ext').checked;
  const search       = document.getElementById('search-input').value.toLowerCase().trim();
  const minCoupling  = parseFloat(document.getElementById('coupling-min').value) / 100;

  nodesDS.update(allNodes.map(n => {
    let hidden = false;
    if (!activeKinds.has(n._kind))  hidden = true;
    if (!showExt && n._ext)         hidden = true;
    if (search && !n.label.toLowerCase().includes(search) && !n.id.toLowerCase().includes(search)) hidden = true;
    if (fsPathFilter !== null && (!n._file || !n._file.startsWith(fsPathFilter))) hidden = true;
    // Coupling filter (ignore nodes with no metrics data — e.g. external)
    if (minCoupling > 0 && metricsMap[n.id] !== undefined && metricsMap[n.id].score < minCoupling) hidden = true;
    return { id: n.id, hidden };
  }));

  const vis_set = new Set();
  nodesDS.forEach(n => { if (!n.hidden) vis_set.add(n.id); });

  edgesDS.update(allEdges.map(e => ({
    id: e.id,
    hidden: !activeEdgeK.has(e._kind) || !vis_set.has(e.from) || !vis_set.has(e.to),
  })));

  // Sync the structural view if it is currently visible
  if (structActive) { renderStructural(); }
}

// ── Layout & view state ──────────────────────────────────────────────────────
let physicsOn      = true;
let hierOn         = false;
let clusterOn      = false;
let compMode       = false;
let currentCompIdx = 0;
let structActive   = false;
let structMaxW     = 1400;   // max row-width inside containers (px, configurable)

function openSettings() {
  document.getElementById('set-min-size').value        = minNodeSize;
  document.getElementById('lbl-min-size').textContent  = minNodeSize;
  document.getElementById('set-ratio').value           = nodeRatio;
  document.getElementById('lbl-ratio').textContent     = nodeRatio;
  document.getElementById('set-repulsion').value       = repulsionForce;
  document.getElementById('lbl-repulsion').textContent = repulsionForce;
  document.getElementById('set-spring').value          = springForce;
  document.getElementById('lbl-spring').textContent    = springForce;
  document.getElementById('set-struct-w').value        = structMaxW;
  document.getElementById('lbl-struct-w').textContent  = structMaxW;
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
  const newStructW = parseInt(document.getElementById('set-struct-w').value);
  const maxNodeSize    = minNodeSize * nodeRatio;
  const springConstant = springForce / 100;
  network.setOptions({
    nodes:   { scaling: { min: minNodeSize, max: maxNodeSize, label: { enabled: true, min: 10, max: 32 } } },
    physics: { forceAtlas2Based: { gravitationalConstant: -repulsionForce, springConstant } },
  });
  // Re-render structural view if the width changed
  if (newStructW !== structMaxW) {
    structMaxW = newStructW;
    if (structActive) { renderStructural(); }
    else { structRendered = false; }   // will re-render next time Struct. is activated
  }
  closeSettings();
  setStatus(`Paramètres — taille ${minNodeSize}–${maxNodeSize}px · répulsion ${repulsionForce} · groupes struct. ${structMaxW}px`);
}

function rebuildGraph() {
  const visIds = new Set();
  nodesDS.forEach(n => { if (!n.hidden) visIds.add(n.id); });
  const subNodes = nodesDS.get({ filter: n => !n.hidden });
  const subEdges = edgesDS.get({ filter: e => !e.hidden && visIds.has(e.from) && visIds.has(e.to) });
  network.setData({ nodes: new vis.DataSet(subNodes), edges: new vis.DataSet(subEdges) });
  setStatus(`↺ Graphe rechargé — ${subNodes.length} nœuds, ${subEdges.length} liens`);
}

function resetAll() { location.reload(); }

function setSimSpeed(v) {
  document.getElementById('lbl-speed').textContent = v;
  const timestep = Math.min(v / 50, 1.0);
  network.setOptions({ physics: { timestep, adaptiveTimestep: false } });
  if (physicsOn && !hierOn) network.startSimulation();
}

function topLevel(id) {
  const sep = id.includes('::') ? '::' : '.';
  return id.split(sep)[0];
}

function updateLayoutBtns() {
  document.getElementById('btn-graph').classList.toggle('is-active', !hierOn && !compMode && !structActive);
  document.getElementById('btn-hier').classList.toggle('is-active', hierOn);
  document.getElementById('btn-clust').classList.toggle('is-active', clusterOn);
  document.getElementById('btn-comp').classList.toggle('is-active', compMode);
  document.getElementById('btn-struct').classList.toggle('is-active', structActive);
  const cb = document.getElementById('tog-force');
  if (cb) cb.checked = physicsOn;
}

function toggleGraph() {
  if (structActive) { exitStructural(); }
  if (compMode) { exitCompMode(); return; }
  if (hierOn) {
    hierOn = false; physicsOn = true;
    network.setOptions({
      layout: { hierarchical: { enabled: false } },
      physics: { solver: 'forceAtlas2Based', enabled: true },
    });
    updateLayoutBtns();
  }
}

function toggleForce() {
  if (compMode) { exitCompMode(); return; }
  const cb = document.getElementById('tog-force');
  physicsOn = cb ? cb.checked : !physicsOn;
  network.setOptions({ physics: { enabled: physicsOn } });
  updateLayoutBtns();
}

function toggleHier() {
  if (structActive) { exitStructural(); }
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

function toggleCluster() {
  if (structActive) { exitStructural(); }
  if (clusterOn) {
    clusterOn = false;
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
}

function toggleCompMode() {
  if (structActive) { exitStructural(); }
  if (compMode) { exitCompMode(); return; }
  const comps = COMPONENTS || [];
  if (!comps.length) { setStatus('Aucune composante connexe'); return; }
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
  network.setData({ nodes: nodesDS, edges: edgesDS });
  network.setOptions({ physics: { enabled: physicsOn && !hierOn } });
  applyFilters();
  updateLayoutBtns();
}

function showComponent(idx) {
  const comps = COMPONENTS || [];
  currentCompIdx = Math.max(0, Math.min(idx, comps.length - 1));
  const compNodeIds = new Set(comps[currentCompIdx]);
  const compNodes = allNodes.filter(n => compNodeIds.has(n.id));
  const compEdges = allEdges.filter(e => compNodeIds.has(e.from) && compNodeIds.has(e.to));
  network.setData({ nodes: new vis.DataSet(compNodes), edges: new vis.DataSet(compEdges) });
  setTimeout(() => network.fit({ animation: { duration: 400 } }), 50);
  updateCompNav(comps);
  setStatus(`Composante ${currentCompIdx+1} / ${comps.length} — ${comps[currentCompIdx].length} nœud(s)`);
}

function updateCompNav(comps) {
  comps = comps || COMPONENTS || [];
  document.getElementById('comp-label').textContent =
    `${currentCompIdx+1} / ${comps.length}  (${(comps[currentCompIdx]||[]).length} nœuds)`;
  document.getElementById('comp-prev').disabled = currentCompIdx <= 0;
  document.getElementById('comp-next').disabled = currentCompIdx >= comps.length - 1;
}

// ── Filesystem tree ──────────────────────────────────────────────────────────
let fsTree       = null;
let fsPathFilter = null;

function buildFsTree() {
  const root = { children: {}, count: 0 };
  RAW_NODES.forEach(n => {
    if (!n.file) return;
    const parts = n.file.split('/').filter(Boolean);
    let node = root;
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      if (!node.children[part]) {
        node.children[part] = { name: part, path: parts.slice(0,i+1).join('/'), isDir: i < parts.length-1, children: {}, count: 0 };
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
  if (currentPath) {
    const parts = currentPath.split('/').filter(Boolean);
    let bc = `<span class="fs-bc-link" onclick="navFs(null)">Tous</span>`;
    let acc = '';
    parts.forEach((p, i) => {
      acc = acc ? acc+'/'+p : p;
      const cap = acc;
      bc += ` › ${i === parts.length-1
        ? `<b style="color:#c0c0c0">${p}</b>`
        : `<span class="fs-bc-link" onclick="navFs('${cap}')">${p}</span>`}`;
    });
    breadEl.innerHTML = bc;
  } else {
    breadEl.innerHTML = '<b style="color:#4A90D9">Tous</b>';
  }
  const children = treeNode ? treeNode.children : {};
  const entries = Object.values(children).sort((a,b) => {
    if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
    return a.name.localeCompare(b.name);
  });
  if (!entries.length) { navEl.innerHTML = '<span style="font-size:10px;color:#555">—</span>'; return; }
  navEl.innerHTML = entries.map(e => {
    const active = fsPathFilter === e.path ? ' is-active' : '';
    const dirCls = e.isDir ? ' fs-dir' : '';
    const click  = e.isDir ? `navFs('${e.path}')` : `filterFs('${e.path}')`;
    return `<button class="fs-btn${dirCls}${active}" onclick="${click}" title="${e.path} (${e.count})">${e.isDir?'▶ ':''}${e.name}</button>`;
  }).join('');
}

function navFs(path) {
  let node = findFsNode(path), targetPath = path;
  while (node && node.children) {
    const kids = Object.values(node.children);
    if (kids.length === 1 && kids[0].isDir) { node = kids[0]; targetPath = kids[0].path; }
    else break;
  }
  fsPathFilter = targetPath || null;
  applyFilters();
  renderFsNav(node, targetPath || null);
}

function filterFs(path) {
  fsPathFilter = path;
  applyFilters();
  const parentPath = path.includes('/') ? path.substring(0, path.lastIndexOf('/')) : null;
  renderFsNav(findFsNode(parentPath), parentPath);
}

// ── Duplicate names debug ────────────────────────────────────────────────────
let dupPanelOpen = false;
function toggleDupPanel() {
  const box = document.getElementById('dup-box');
  dupPanelOpen = !dupPanelOpen;
  if (!dupPanelOpen) { box.style.display = 'none'; return; }
  const byName = {};
  RAW_NODES.forEach(n => {
    const label = n.name || n.id;
    if (!byName[label]) byName[label] = [];
    byName[label].push(n);
  });
  const dups = Object.entries(byName).filter(([,nodes]) => nodes.length > 1).sort((a,b) => b[1].length - a[1].length);
  if (!dups.length) {
    box.innerHTML = '<div style="color:#2ECC71">Aucun doublon</div>';
  } else {
    box.innerHTML = dups.map(([name, nodes]) => `
      <div style="border-bottom:1px solid #0f3460;padding:4px 0">
        <b style="color:#E67E22">${name}</b> (${nodes.length}×)
        ${nodes.map(n => `<div style="padding-left:8px;color:#888">
          <span style="color:${NODE_COLORS[n.kind]||'#aaa'}">${n.kind}</span>
          ${n.is_external ? '<span style="color:#E74C3C">[ext]</span>' : ''}
          — <span style="color:#a0c4ff;cursor:pointer" onclick="jumpTo('${n.id}')">${n.id}</span>
        </div>`).join('')}
      </div>`).join('');
  }
  box.style.display = 'block';
}

// ── PNG export ───────────────────────────────────────────────────────────────
function exportPNG() {
  // Dispatch vers le layout actif
  if (structActive) { exportStructPNG(); return; }

  const graphCanvas = container.querySelector('canvas');
  if (!graphCanvas) { alert('Canvas non disponible'); return; }

  let titleText;
  if (fsPathFilter) {
    titleText = fsPathFilter.split('/').filter(Boolean).pop() || fsPathFilter;
  } else {
    const rootParts = ROOT_DIR.split('/').filter(Boolean);
    titleText = rootParts.length > 0 ? rootParts[rootParts.length-1] : 'code_graph';
  }

  const visibleKinds = new Set();
  nodesDS.forEach(n => { if (!n.hidden) visibleKinds.add(n._kind); });

  // Use NODE_COLORS so the legend always matches the graph nodes.
  const kindMeta = {
    module:   { label: 'Module',    color: NODE_COLORS.module,   shape: 'hexagon' },
    class:    { label: 'Classe',    color: NODE_COLORS.class,    shape: 'square'  },
    struct:   { label: 'Struct',    color: NODE_COLORS.struct,   shape: 'diamond' },
    function: { label: 'Fonction',  color: NODE_COLORS.function, shape: 'dot'     },
    method:   { label: 'Méthode',   color: NODE_COLORS.method,   shape: 'dot'     },
    constant: { label: 'Constante', color: NODE_COLORS.constant, shape: 'triangle'},
  };
  const legendItems = Object.entries(kindMeta).filter(([k]) => visibleKinds.has(k));

  const dpr = window.devicePixelRatio || 1;
  const gw = Math.round(graphCanvas.width  / dpr);
  const gh = Math.round(graphCanvas.height / dpr);
  const LEG_FONT = 22;  // px — sera rendu à LEG_FONT*EXPORT_SCALE dans le PNG
  const PAD = 20, TITLE_H = 40, LEG_H = legendItems.length > 0 ? Math.round(LEG_FONT * 2.8) : 0;
  const totalW = gw, totalH = TITLE_H + gh + LEG_H + PAD * 3;

  // ×2 resolution: all drawing uses logical coords, canvas is 2× larger.
  const EXPORT_SCALE = 2;
  const out = document.createElement('canvas');
  out.width  = totalW * EXPORT_SCALE;
  out.height = totalH * EXPORT_SCALE;
  const ctx = out.getContext('2d');
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = 'high';
  ctx.scale(EXPORT_SCALE, EXPORT_SCALE);

  ctx.fillStyle = '#1a1a2e';
  ctx.fillRect(0, 0, totalW, totalH);
  ctx.fillStyle = '#ffffff';
  ctx.font = 'bold 22px sans-serif';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(titleText, totalW/2, PAD + TITLE_H/2);
  // Draw the vis-network canvas: source is DPR-adjusted, dest is logical size.
  ctx.drawImage(graphCanvas, 0, 0, graphCanvas.width, graphCanvas.height,
                             0, TITLE_H + PAD, gw, gh);

  if (legendItems.length > 0) {
    const iconR = Math.round(LEG_FONT * 0.55);   // proportionnel à la police
    const itemW = Math.floor(totalW / legendItems.length);
    const legY  = TITLE_H + PAD + gh + PAD/2 + LEG_H/2;
    const off   = Math.round(itemW * 0.30);       // décalage icône depuis cx
    legendItems.forEach(([, meta], i) => {
      const cx = itemW*i + itemW/2;
      ctx.fillStyle = meta.color;
      if (meta.shape === 'hexagon') {
        ctx.beginPath();
        for (let a = 0; a < 6; a++) {
          const ang = Math.PI/180*(60*a-30);
          const px = cx-off+iconR*Math.cos(ang), py = legY+iconR*Math.sin(ang);
          a===0 ? ctx.moveTo(px,py) : ctx.lineTo(px,py);
        }
        ctx.closePath(); ctx.fill();
      } else if (meta.shape === 'square') {
        ctx.fillRect(cx-off-iconR, legY-iconR, iconR*2, iconR*2);
      } else if (meta.shape === 'diamond') {
        ctx.beginPath();
        ctx.moveTo(cx-off, legY-iconR); ctx.lineTo(cx-off+iconR, legY);
        ctx.lineTo(cx-off, legY+iconR); ctx.lineTo(cx-off-iconR, legY);
        ctx.closePath(); ctx.fill();
      } else {
        ctx.beginPath(); ctx.arc(cx-off, legY, iconR, 0, Math.PI*2); ctx.fill();
      }
      ctx.fillStyle = '#eeeeee';
      ctx.font = 'bold ' + LEG_FONT + 'px sans-serif';
      ctx.textAlign = 'left';
      ctx.textBaseline = 'middle';
      ctx.fillText(meta.label, cx-off+iconR+6, legY);
    });
  }
  const a = document.createElement('a');
  a.href = out.toDataURL('image/png');
  a.download = titleText.toLowerCase().replace(/\s+/g,'_') + '_graph.png';
  a.click();
}

// ── SVG export ───────────────────────────────────────────────────────────────
function exportSVG() {
  const positions = network.getPositions();
  const visibleNodes = [];
  const visibleEdges = [];

  nodesDS.forEach(n => { if (!n.hidden && positions[n.id]) visibleNodes.push(n); });
  edgesDS.forEach(e => { if (!e.hidden && positions[e.from] && positions[e.to]) visibleEdges.push(e); });

  if (!visibleNodes.length) { setStatus('Aucun nœud visible à exporter'); return; }

  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  visibleNodes.forEach(n => {
    const p = positions[n.id];
    minX = Math.min(minX, p.x - 40); minY = Math.min(minY, p.y - 40);
    maxX = Math.max(maxX, p.x + 40); maxY = Math.max(maxY, p.y + 40);
  });

  const W = maxX - minX + 80, H = maxY - minY + 80;
  const ox = -minX + 40, oy = -minY + 40;

  const parts = [
    `<?xml version="1.0" encoding="UTF-8"?>`,
    `<svg xmlns="http://www.w3.org/2000/svg" width="${W.toFixed(0)}" height="${H.toFixed(0)}" viewBox="0 0 ${W.toFixed(0)} ${H.toFixed(0)}">`,
    `<rect width="${W.toFixed(0)}" height="${H.toFixed(0)}" fill="#0d1117"/>`,
    `<defs>`,
    `  <marker id="arr" markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto">`,
    `    <polygon points="0 0,8 3,0 6" fill="#666"/>`,
    `  </marker>`,
    `</defs>`,
  ];

  // Edges
  visibleEdges.forEach(e => {
    const sp = positions[e.from], tp = positions[e.to];
    const c  = EDGE_COLORS[e._kind] || '#555';
    parts.push(
      `<line x1="${(sp.x+ox).toFixed(1)}" y1="${(sp.y+oy).toFixed(1)}" ` +
      `x2="${(tp.x+ox).toFixed(1)}" y2="${(tp.y+oy).toFixed(1)}" ` +
      `stroke="${c}" stroke-width="1.2" stroke-opacity="0.65" marker-end="url(#arr)"/>`
    );
  });

  // Nodes
  const r = 12;
  visibleNodes.forEach(n => {
    const p  = positions[n.id];
    const cx = (p.x + ox).toFixed(1);
    const cy = (p.y + oy).toFixed(1);
    const c  = NODE_COLORS[n._kind] || '#aaa';
    const dc = darken(c);
    const lbl = (n.label || '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');

    if (n._kind === 'module') {
      // hexagon
      const pts = Array.from({length:6}, (_,i) => {
        const a = Math.PI/180*(60*i-30);
        return `${(p.x+ox+r*Math.cos(a)).toFixed(1)},${(p.y+oy+r*Math.sin(a)).toFixed(1)}`;
      }).join(' ');
      parts.push(`<polygon points="${pts}" fill="${c}" stroke="${dc}" stroke-width="1.2"/>`);
    } else if (n._kind === 'class') {
      parts.push(`<rect x="${(p.x+ox-r).toFixed(1)}" y="${(p.y+oy-r).toFixed(1)}" width="${2*r}" height="${2*r}" fill="${c}" stroke="${dc}" stroke-width="1.2" rx="2"/>`);
    } else if (n._kind === 'struct') {
      // diamond
      parts.push(
        `<polygon points="${cx},${(p.y+oy-r).toFixed(1)} ${(p.x+ox+r).toFixed(1)},${cy} ${cx},${(p.y+oy+r).toFixed(1)} ${(p.x+ox-r).toFixed(1)},${cy}" fill="${c}" stroke="${dc}" stroke-width="1.2"/>`
      );
    } else {
      parts.push(`<circle cx="${cx}" cy="${cy}" r="${r}" fill="${c}" stroke="${dc}" stroke-width="1.2"/>`);
    }
    parts.push(`<text x="${cx}" y="${(p.y+oy+r+11).toFixed(1)}" text-anchor="middle" fill="#cccccc" font-family="sans-serif" font-size="10">${lbl}</text>`);
  });

  parts.push(`</svg>`);

  const blob = new Blob([parts.join('\n')], { type: 'image/svg+xml' });
  const url  = URL.createObjectURL(blob);
  const a    = document.createElement('a');
  a.href     = url;
  a.download = 'code_graph.svg';
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  setStatus(`SVG exporté — ${visibleNodes.length} nœuds, ${visibleEdges.length} liens`);
}

// ── Right-click context menu ─────────────────────────────────────────────────
let ctxNode = null;

container.addEventListener('contextmenu', e => {
  e.preventDefault();
  hideCtx();
  const pos    = { x: e.offsetX, y: e.offsetY };
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
function hideCtx() { document.getElementById('ctx-menu').style.display = 'none'; }

document.getElementById('ctx-open').addEventListener('click', () => {
  if (!ctxNode) return;
  const full = (ROOT_DIR + '/' + ctxNode._file).replace(/\\/g,'/').replace(/\/+/g,'/');
  const dir  = full.substring(0, full.lastIndexOf('/'));
  window.open('file:///' + dir.replace(/^\//,''));
  hideCtx();
});
document.getElementById('ctx-focus-btn').addEventListener('click', () => {
  if (!ctxNode) return; focusNode(ctxNode.id); hideCtx();
});
document.getElementById('ctx-hide').addEventListener('click', () => {
  if (!ctxNode) return;
  nodesDS.update({ id: ctxNode.id, hidden: true });
  edgesDS.update(allEdges.filter(e => e.from === ctxNode.id || e.to === ctxNode.id).map(e => ({ id: e.id, hidden: true })));
  hideCtx();
  setStatus(`Masqué : ${ctxNode.id}`);
});
document.getElementById('ctx-copy').addEventListener('click', () => {
  if (!ctxNode) return;
  navigator.clipboard.writeText(ctxNode.id).catch(() => {});
  setStatus(`Copié : ${ctxNode.id}`);
  hideCtx();
});

// ── Status bar ───────────────────────────────────────────────────────────────
function setStatus(msg) { document.getElementById('status-bar').textContent = msg; }

// ── Helpers ──────────────────────────────────────────────────────────────────
function darken(hex) {
  try {
    const n = parseInt(hex.slice(1), 16);
    const r = Math.max(0, (n>>16)-40);
    const g = Math.max(0, ((n>>8)&0xff)-40);
    const b = Math.max(0, (n&0xff)-40);
    return '#' + [r,g,b].map(x=>x.toString(16).padStart(2,'0')).join('');
  } catch(_) { return '#333'; }
}

// ── Structural (nested-box) layout ───────────────────────────────────────────
// Shows the physical containment hierarchy: crate/folder > file > class > fn
// Each parent is a labelled rounded rect; children are laid out inside.
// Supports mouse-wheel zoom and drag pan.

let structRendered = false;
// ViewBox-based pan/zoom — no CSS transforms, no clientWidth dependency.
// _vbX/_vbY: viewBox origin in SVG-canvas coordinates
// _vbW/_vbH: viewBox dimensions (zoom level; start = full canvas)
// _canvasW/_canvasH: original treemap dimensions (set in renderStructural)
let _vbX = 0, _vbY = 0, _vbW = 1, _vbH = 1;
let _canvasW = 1, _canvasH = 1;

function toggleStructural() {
  structActive = !structActive;
  if (structActive) {
    document.getElementById('mynetwork').style.display = 'none';
    document.getElementById('structural-wrap').style.display = 'block';
    if (!structRendered) { renderStructural(); structRendered = true; }
  } else {
    exitStructural();
  }
  updateLayoutBtns();
}

function exitStructural() {
  structActive = false;
  document.getElementById('mynetwork').style.display = 'block';
  document.getElementById('structural-wrap').style.display = 'none';
  updateLayoutBtns();
}

// Recentre la vue structurelle en réinitialisant le viewBox au canvas entier.
// preserveAspectRatio="xMidYMid meet" assure le centrage automatique par le moteur SVG.
// Aucune dépendance aux dimensions CSS du wrap.
function fitStructural() {
  const svg = document.getElementById('struct-svg');
  if (!svg || _canvasW < 1) return;
  _vbX = 0; _vbY = 0; _vbW = _canvasW; _vbH = _canvasH;
  svg.setAttribute('viewBox', '0 0 ' + _canvasW + ' ' + _canvasH);
  svg.setAttribute('preserveAspectRatio', 'xMidYMid meet');
}

function renderStructural() {
  // ── 1. Build Contains tree from graph edges ─────────────────────────────────
  // Respect the "show external dependencies" filter
  const showExtStruct = document.getElementById('tog-ext').checked;

  const nodeMap = {};
  RAW_NODES.forEach(n => {
    if (!showExtStruct && n.is_external) return;
    nodeMap[n.id] = n;
  });

  const graphChildren = {};   // real graph parent_id -> [child_id, …]
  const hasGraphParent = new Set();
  RAW_EDGES.forEach(e => {
    // Only consider edges where both endpoints are visible
    if (e.kind === 'contains' && nodeMap[e.source] && nodeMap[e.target]) {
      if (!graphChildren[e.source]) graphChildren[e.source] = [];
      graphChildren[e.source].push(e.target);
      hasGraphParent.add(e.target);
    }
  });

  // Graph roots = real nodes with no Contains parent
  const graphRoots = RAW_NODES.filter(n => !hasGraphParent.has(n.id)).map(n => n.id);
  if (!graphRoots.length) { setStatus('Vue structurelle : aucun nœud racine trouvé'); return; }

  // ── 2. Synthesise virtual folder nodes from file paths ──────────────────────
  // Graph root nodes are usually file-level modules (kind=module, file=path/to/file.rs).
  // They carry path info but the graph has no folder nodes above them.
  // We create virtual "__vdir__/…" nodes for each directory level so that, e.g.,
  //   crates/parsers/src/lib.rs  and  crates/parsers/src/rust_lang.rs
  // both appear nested inside a "crates/parsers/src" virtual folder.

  const virtNodes    = {};   // vdir_id -> synthetic node descriptor
  const allChildren  = {};   // merged children map (graph + virtual)
  Object.entries(graphChildren).forEach(([k, v]) => { allChildren[k] = v.slice(); });

  function ensureDir(parts) {
    // Returns the virtual-node id for the folder represented by `parts`.
    const vid = '__vdir__/' + parts.join('/');
    if (!virtNodes[vid]) {
      virtNodes[vid] = { id: vid, name: parts[parts.length - 1], kind: 'folder', file: parts.join('/'), is_virtual: true };
      allChildren[vid] = [];
      if (parts.length > 1) {
        const parentVid = ensureDir(parts.slice(0, -1));
        if (!allChildren[parentVid].includes(vid)) allChildren[parentVid].push(vid);
      }
    }
    return vid;
  }

  // Assign each graph root with a file path to its folder parent
  const rootMovedToFolder = new Set();
  graphRoots.forEach(id => {
    const raw = nodeMap[id];
    if (!raw || !raw.file) return;
    const parts = raw.file.replace(/\\/g, '/').split('/').filter(Boolean);
    if (parts.length < 2) return;   // file already at repo root — keep as root
    const dirVid = ensureDir(parts.slice(0, -1));
    if (!allChildren[dirVid].includes(id)) allChildren[dirVid].push(id);
    rootMovedToFolder.add(id);
  });

  // ── 3. Collapse single-child virtual folder chains ─────────────────────────
  // "crates/" -> "parsers/" (single child) collapses into "crates/parsers/"
  const allNodeMap = Object.assign({}, nodeMap, virtNodes);

  function collapseChains(vid) {
    for (;;) {
      const kids = allChildren[vid] || [];
      if (kids.length === 1 && allNodeMap[kids[0]] && allNodeMap[kids[0]].is_virtual) {
        const child = kids[0];
        allNodeMap[vid].name += '/' + allNodeMap[child].name;
        allChildren[vid] = allChildren[child] || [];
        delete allNodeMap[child];
        delete allChildren[child];
      } else break;
    }
    (allChildren[vid] || []).forEach(k => {
      if (allNodeMap[k] && allNodeMap[k].is_virtual) collapseChains(k);
    });
  }

  // Determine top-level virtual dirs (those with no virtual parent)
  const virtHasParent = new Set();
  Object.values(allChildren).forEach(kids => kids.forEach(k => { if (allNodeMap[k] && allNodeMap[k].is_virtual) virtHasParent.add(k); }));
  const topVirtDirs = Object.keys(virtNodes).filter(vid => allNodeMap[vid] && !virtHasParent.has(vid));
  topVirtDirs.forEach(vid => collapseChains(vid));

  // ── 4. Build final root list ─────────────────────────────────────────────────
  // = graph roots NOT assigned to a folder  +  top-level virtual dirs (after collapse)
  const virtHasParent2 = new Set();
  Object.values(allChildren).forEach(kids => kids.forEach(k => { if (allNodeMap[k] && allNodeMap[k].is_virtual) virtHasParent2.add(k); }));

  const finalRoots = [];
  graphRoots.forEach(id => { if (!rootMovedToFolder.has(id)) finalRoots.push(id); });
  Object.keys(allNodeMap).forEach(vid => {
    if (allNodeMap[vid] && allNodeMap[vid].is_virtual && !virtHasParent2.has(vid)) finalRoots.push(vid);
  });

  if (!finalRoots.length) { setStatus('Vue structurelle : arbre vide après construction'); return; }

  // ── 5. Layout constants ──────────────────────────────────────────────────────
  const PAD    = 10;    // inner padding of containers
  const GAP    = 7;     // gap between siblings
  const HDR    = 24;    // header bar height for containers
  const LEAF_W = 118;   // leaf node width
  const LEAF_H = 32;    // leaf node height
  const MIN_W  = 170;   // minimum container width
  const FOLDER_COLOR  = '#607D8B';   // blue-grey for virtual dirs

  // ── 6. Treemap — compute subtree weights (bottom-up) ─────────────────────────
  // leaf = 1 unit.  container = sum of children.
  // structMaxW slider repurposed as "cell size" multiplier (1400 = default).
  // BASE_CELL: area allocated per unit of weight.
  // structMaxW slider acts as a global "cell size" multiplier.
  const BASE_CELL = 2000 * (structMaxW / 1400);

  // Minimum area in px² so the node's name is legible.
  //
  // Key insight: squarify produces roughly SQUARE cells (aspect ratio ≈ 1),
  // so the effective rendered width ≈ √(allocated_area).
  // To guarantee cell_width ≥ minW, we need area ≥ minW² (not minW × minH).
  //
  // Containers are different: their name goes in a fixed-height header band,
  // so their minimum area only needs to ensure header_width ≥ minW.
  function nameMinArea(id) {
    const raw  = allNodeMap[id];
    if (!raw) return BASE_CELL;
    const name = raw.name || id.split(/[\/::]+/).pop() || '';
    const kids = allChildren[id] || [];
    const isLeaf = kids.length === 0;
    // px per character: ~8 for leaves (font 11), ~9 for containers (font 12)
    const minW = Math.max(72, name.length * (isLeaf ? 8 : 9) + 24);
    // Leaf → minW² so √(area) ≈ minW
    // Container → minW × (HDR + 30): only the header band must be wide enough
    return isLeaf ? minW * minW : minW * (HDR + 30);
  }

  const weights = {};
  function computeWeight(id) {
    const kids     = allChildren[id] || [];
    const minUnits = nameMinArea(id) / BASE_CELL;
    if (!kids.length) {
      weights[id] = Math.max(1, minUnits);
      return;
    }
    kids.forEach(k => computeWeight(k));
    const childSum = kids.reduce((s, k) => s + weights[k], 0);
    weights[id] = Math.max(childSum, minUnits);
  }
  finalRoots.forEach(r => computeWeight(r));

  const totalWeight = finalRoots.reduce((s, r) => s + weights[r], 0);
  const totalArea   = totalWeight * BASE_CELL;
  // Canvas: 4:3 aspect ratio — wider than square, comfortable for code maps
  const canvasW = Math.round(Math.sqrt(totalArea * (4 / 3)));
  const canvasH = Math.round(totalArea / canvasW);

  // ── 7. Treemap — squarified layout (Bruls, Huizing, van Wijk 2000) ───────────
  // Each node occupies a rectangle proportional to its subtree weight.
  // The algorithm greedily builds strips, choosing orientation and item count to
  // minimise the worst aspect ratio across all cells in the strip.
  const pos   = {};   // id -> {x, y}
  const sizes = {};   // id -> {w, h}

  // Worst aspect ratio of items placed in a strip of shorter side L
  function worstAR(row, L) {
    if (!row.length || L <= 0) return Infinity;
    const s = row.reduce((a, i) => a + i.area, 0);
    if (s <= 0) return Infinity;
    let worst = 0;
    row.forEach(({ area }) => {
      const strip = s / L;
      const cell  = area / strip;
      const ar = strip > cell ? strip / cell : cell / strip;
      if (ar > worst) worst = ar;
    });
    return worst;
  }

  // Place one strip of items in rectangle [x,y,w,h], recurse into each node
  function placeStrip(row, x, y, w, h) {
    const rowArea = row.reduce((a, i) => a + i.area, 0);
    if (rowArea <= 0 || w < 1 || h < 1) return;
    let offset = 0;
    if (w >= h) {
      // Vertical strip on left — items stacked top-to-bottom
      const sw = rowArea / h;
      row.forEach(item => {
        const sh = item.area / sw;
        pos[item.id]   = { x,             y: y + offset };
        sizes[item.id] = { w: Math.max(2, sw), h: Math.max(2, sh) };
        placeChildren(item.id);
        offset += sh;
      });
    } else {
      // Horizontal strip on top — items placed left-to-right
      const sh = rowArea / w;
      row.forEach(item => {
        const sw = item.area / sh;
        pos[item.id]   = { x: x + offset, y };
        sizes[item.id] = { w: Math.max(2, sw), h: Math.max(2, sh) };
        placeChildren(item.id);
        offset += sw;
      });
    }
  }

  // Squarify: greedily add items to current strip while aspect ratio improves
  function squarifyInto(items, x, y, w, h) {
    if (!items.length || w < 2 || h < 2) return;
    let row = [], remaining = items.slice();
    let cx = x, cy = y, cw = w, ch = h;
    while (remaining.length) {
      const L = Math.min(cw, ch);
      if (!row.length || worstAR([...row, remaining[0]], L) <= worstAR(row, L)) {
        row.push(remaining.shift());
      } else {
        const rowArea = row.reduce((a, i) => a + i.area, 0);
        placeStrip(row, cx, cy, cw, ch);
        if (cw >= ch) { const sw = rowArea / ch; cx += sw; cw -= sw; }
        else          { const sh = rowArea / cw; cy += sh; ch -= sh; }
        row = [];
      }
    }
    if (row.length) placeStrip(row, cx, cy, cw, ch);
  }

  // Recursively fill a node's inner area with its children
  function placeChildren(id) {
    const kids = allChildren[id] || [];
    if (!kids.length || !pos[id] || !sizes[id]) return;
    const { x, y } = pos[id], { w, h } = sizes[id];
    const innerX = x + PAD, innerY = y + HDR;
    const innerW = w - 2 * PAD, innerH = h - HDR - PAD;
    if (innerW < 4 || innerH < 4) return;
    const totalW = kids.reduce((s, k) => s + weights[k], 0);
    if (!totalW) return;
    const innerArea = innerW * innerH;
    // Sort largest-first: squarify produces best ratios when areas are descending
    const items = kids
      .map(k => ({ id: k, area: (weights[k] / totalW) * innerArea }))
      .sort((a, b) => b.area - a.area);
    squarifyInto(items, innerX, innerY, innerW, innerH);
  }

  // Place all roots in the full canvas
  const rootItems = finalRoots
    .map(r => ({ id: r, area: (weights[r] / totalWeight) * (canvasW * canvasH) }))
    .sort((a, b) => b.area - a.area);
  squarifyInto(rootItems, 0, 0, canvasW, canvasH);

  // ── 8. Canvas dimensions — set by treemap ────────────────────────────────────
  const svgW = canvasW;
  const svgH = canvasH;

  // ── 9. Color helpers ─────────────────────────────────────────────────────────
  function hexAlpha(hex, a) {
    const n = parseInt((hex || '#888888').replace('#', ''), 16);
    return 'rgba(' + ((n>>16)&255) + ',' + ((n>>8)&255) + ',' + (n&255) + ',' + a + ')';
  }

  // ── 10. Build SVG DOM ────────────────────────────────────────────────────────
  // Store canvas dimensions for viewBox-based pan/zoom and PNG export.
  _canvasW = svgW; _canvasH = svgH;
  _vbX = 0; _vbY = 0; _vbW = svgW; _vbH = svgH;

  const NS  = 'http://www.w3.org/2000/svg';
  const svg = document.getElementById('struct-svg');
  svg.innerHTML = '';
  // Keep width/height attributes for the PNG export function (reads them directly).
  // The CSS width:100%;height:100% overrides them for display.
  svg.setAttribute('width',  svgW);
  svg.setAttribute('height', svgH);
  svg.setAttribute('viewBox', '0 0 ' + svgW + ' ' + svgH);
  // xMidYMid meet: SVG engine centres and scales the canvas to fill the element.
  svg.setAttribute('preserveAspectRatio', 'xMidYMid meet');

  const bgRect = document.createElementNS(NS, 'rect');
  bgRect.setAttribute('width', svgW); bgRect.setAttribute('height', svgH);
  bgRect.setAttribute('fill', '#0d1117');
  svg.appendChild(bgRect);

  let selectedG = null;

  function renderNode(id) {
    const raw = allNodeMap[id];
    if (!raw || !pos[id] || !sizes[id]) return;
    const { x, y } = pos[id];
    const { w, h } = sizes[id];
    if (w < 3 || h < 3) return;   // invisible — skip

    const kids    = allChildren[id] || [];
    const isLeaf  = kids.length === 0;
    const isVirt  = raw.is_virtual;
    const color   = isVirt ? FOLDER_COLOR : (NODE_COLORS[raw.kind] || '#888888');
    const isOrph  = !isVirt && orphanIds.has(id);

    // Adaptive metrics based on actual cell size
    const tiny      = w < 40  || h < 22;    // too small for any text
    const small     = w < 80  || h < 36;    // small: badge hidden, short label
    const hdrH      = !isLeaf ? Math.min(HDR, Math.max(14, Math.floor(h * 0.28))) : 0;
    const showHdr   = !isLeaf && hdrH >= 10 && h > hdrH + 6;
    const fontSize  = tiny ? 0 : small ? 9 : (isLeaf ? 11 : 12);
    const rx        = tiny ? 1 : (isLeaf ? 3 : 5);

    const g = document.createElementNS(NS, 'g');
    g.setAttribute('data-id', id);
    g.style.cursor = isVirt ? 'default' : 'pointer';

    // ── Main bounding rect ───────────────────────────────────────────────────
    const mainRect = document.createElementNS(NS, 'rect');
    mainRect.setAttribute('x', x); mainRect.setAttribute('y', y);
    mainRect.setAttribute('width', w); mainRect.setAttribute('height', h);
    mainRect.setAttribute('rx', rx);
    mainRect.setAttribute('fill',   hexAlpha(color, isLeaf ? 0.18 : (isVirt ? 0.04 : 0.07)));
    mainRect.setAttribute('stroke', hexAlpha(color, isLeaf ? 0.70 : (isVirt ? 0.50 : 0.40)));
    mainRect.setAttribute('stroke-width', '1');
    if (isVirt) mainRect.setAttribute('stroke-dasharray', '5 2');
    if (isOrph) mainRect.setAttribute('stroke-dasharray', '4 2');
    g.appendChild(mainRect);

    // ── Header band (containers only) ────────────────────────────────────────
    if (showHdr) {
      const hdr = document.createElementNS(NS, 'rect');
      hdr.setAttribute('x', x); hdr.setAttribute('y', y);
      hdr.setAttribute('width', w); hdr.setAttribute('height', hdrH);
      hdr.setAttribute('rx', rx);
      hdr.setAttribute('fill', hexAlpha(color, isVirt ? 0.22 : 0.32));
      g.appendChild(hdr);
      // Square off the bottom corners of the header band
      if (hdrH > rx) {
        const fix = document.createElementNS(NS, 'rect');
        fix.setAttribute('x', x); fix.setAttribute('y', y + hdrH - rx);
        fix.setAttribute('width', w); fix.setAttribute('height', rx);
        fix.setAttribute('fill', hexAlpha(color, isVirt ? 0.22 : 0.32));
        g.appendChild(fix);
      }
    }

    // ── Labels (skip for tiny cells) ─────────────────────────────────────────
    if (fontSize > 0) {
      // Kind badge — only when there is enough room
      if (!small && w > 26 && h > 18) {
        const badge = document.createElementNS(NS, 'text');
        badge.setAttribute('x', x + 4);
        badge.setAttribute('y', showHdr ? y + hdrH - 3 : y + h / 2 - fontSize * 0.6);
        badge.setAttribute('font-size', '7');
        badge.setAttribute('fill', hexAlpha(color, 0.88));
        badge.setAttribute('font-family', 'monospace');
        badge.setAttribute('font-weight', 'bold');
        badge.setAttribute('pointer-events', 'none');
        badge.textContent = isVirt ? 'DIR' : raw.kind.toUpperCase();
        g.appendChild(badge);
      }

      // Node name centred in its content area
      const maxChars = Math.max(2, Math.floor((w - 6) / (fontSize * 0.62)));
      const rawName  = raw.name || id.split(/[\/::]+/).pop() || id;
      const dispName = rawName.length > maxChars ? rawName.slice(0, maxChars - 1) + '…' : rawName;

      // Containers: name centred INSIDE the header band at the top.
      // Leaves: name centred in the whole cell.
      const labelY = showHdr
        ? y + hdrH / 2 + fontSize * 0.38   // centred in header band
        : y + h  / 2 + fontSize * 0.38;    // centred in cell (leaf)

      const lbl = document.createElementNS(NS, 'text');
      lbl.setAttribute('x', x + w / 2);
      lbl.setAttribute('y', labelY);
      lbl.setAttribute('text-anchor', 'middle');
      lbl.setAttribute('font-size', fontSize);
      lbl.setAttribute('fill', isVirt ? hexAlpha(color, 0.80) : (tiny ? hexAlpha(color, 0.9) : '#dde3ec'));
      lbl.setAttribute('font-family', 'sans-serif');
      lbl.setAttribute('font-weight', (!isLeaf && !tiny) ? 'bold' : 'normal');
      lbl.setAttribute('pointer-events', 'none');
      lbl.textContent = dispName;
      g.appendChild(lbl);
    }

    // ── Interaction ──────────────────────────────────────────────────────────
    if (!isVirt) {
      g.addEventListener('click', evt => {
        evt.stopPropagation();
        if (selectedG) {
          const pr = selectedG.querySelector('rect');
          if (pr) { pr.setAttribute('stroke', pr._origStroke || '#888'); pr.setAttribute('stroke-width', '1'); }
        }
        mainRect._origStroke = mainRect.getAttribute('stroke');
        mainRect.setAttribute('stroke', '#ffffff');
        mainRect.setAttribute('stroke-width', '2');
        selectedG = g;
        showDetail(id);
      });
    }

    svg.appendChild(g);
    kids.forEach(k => renderNode(k));
  }

  finalRoots.forEach(r => renderNode(r));

  const nFolders = Object.keys(virtNodes).filter(k => allNodeMap[k]).length;
  setStatus('Vue structurelle — ' + nFolders + ' dossier(s) · ' + RAW_NODES.length + ' nœuds — scroll = zoom · drag = pan');
  // preserveAspectRatio="xMidYMid meet" already centres the content — no RAF needed.
}

// ── Pan/zoom for structural view ─────────────────────────────────────────────
// ── ViewBox-based pan / zoom ──────────────────────────────────────────────────
// With preserveAspectRatio="xMidYMid meet" the SVG engine centres and letterboxes
// the canvas. We need to account for that letterbox offset when mapping screen ←→ SVG.
(function setupStructPanZoom() {
  const wrap = document.getElementById('structural-wrap');
  let dragging = false, lastMX = 0, lastMY = 0;

  // Returns effective px/SVGunit scale and letterbox offsets for the current viewBox.
  function viewInfo(r) {
    const eff = Math.min(r.width / _vbW, r.height / _vbH);
    return { eff, ox: (r.width - _vbW * eff) / 2, oy: (r.height - _vbH * eff) / 2 };
  }

  wrap.addEventListener('wheel', e => {
    e.preventDefault();
    const svg = document.getElementById('struct-svg');
    if (!svg || _vbW < 1) return;
    const r  = svg.getBoundingClientRect();
    const vi = viewInfo(r);
    // Mouse position in SVG canvas coordinates
    const mx = e.clientX - r.left - vi.ox;
    const my = e.clientY - r.top  - vi.oy;
    const svgMx = _vbX + mx / vi.eff;
    const svgMy = _vbY + my / vi.eff;
    // Zoom: shrink/grow viewBox (keep canvas aspect ratio)
    const f = e.deltaY < 0 ? 1 / 1.15 : 1.15;
    _vbW = Math.max(50, Math.min(_vbW * f, _canvasW * 30));
    _vbH = _vbW * (_canvasH / _canvasW);
    // Recompute info with new viewBox size, then anchor svgMx under cursor
    const vi2 = viewInfo(r);
    const mx2 = e.clientX - r.left - vi2.ox;
    const my2 = e.clientY - r.top  - vi2.oy;
    _vbX = svgMx - mx2 / vi2.eff;
    _vbY = svgMy - my2 / vi2.eff;
    svg.setAttribute('viewBox', _vbX + ' ' + _vbY + ' ' + _vbW + ' ' + _vbH);
  }, { passive: false });

  wrap.addEventListener('mousedown', e => {
    if (e.button !== 0) return;
    dragging = true; lastMX = e.clientX; lastMY = e.clientY;
    wrap.style.cursor = 'grabbing';
  });
  window.addEventListener('mousemove', e => {
    if (!dragging) return;
    const svg = document.getElementById('struct-svg');
    if (!svg || _vbW < 1) return;
    const r  = svg.getBoundingClientRect();
    const vi = viewInfo(r);
    // Convert pixel delta to SVG delta
    _vbX -= (e.clientX - lastMX) / vi.eff;
    _vbY -= (e.clientY - lastMY) / vi.eff;
    lastMX = e.clientX; lastMY = e.clientY;
    svg.setAttribute('viewBox', _vbX + ' ' + _vbY + ' ' + _vbW + ' ' + _vbH);
  });
  window.addEventListener('mouseup', () => {
    dragging = false;
    wrap.style.cursor = 'grab';
  });

  document.getElementById('btn-struct-reset').addEventListener('click', fitStructural);
  document.getElementById('btn-struct-export').addEventListener('click', exportStructSVG);
})();

function exportStructSVG() {
  const svg = document.getElementById('struct-svg');
  if (!svg || !svg.getAttribute('width')) { setStatus('Vue structurelle non encore rendue'); return; }
  const ser  = new XMLSerializer().serializeToString(svg);
  const blob = new Blob([ser], { type: 'image/svg+xml' });
  const url  = URL.createObjectURL(blob);
  const a    = document.createElement('a');
  a.href = url; a.download = 'code_structure.svg'; a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  setStatus('Structure SVG exportée');
}

// ── PNG export de la vue structurelle ────────────────────────────────────────
// Serialize le SVG → Image → Canvas 2D avec titre + légende
function exportStructPNG() {
  const svg = document.getElementById('struct-svg');
  if (!svg || !svg.getAttribute('width')) { setStatus('Vue structurelle non encore rendue'); return; }

  // ── Titre (même logique que l'export PNG du graph) ────────────────────────
  let titleText;
  if (fsPathFilter) {
    titleText = fsPathFilter.split('/').filter(Boolean).pop() || fsPathFilter;
  } else {
    const rootParts = ROOT_DIR.split('/').filter(Boolean);
    titleText = rootParts.length > 0 ? rootParts[rootParts.length - 1] : 'code_graph';
  }

  // ── Légende : dossiers virtuels + kinds présents ──────────────────────────
  const showExtStruct = document.getElementById('tog-ext').checked;
  const visibleKinds  = new Set();
  RAW_NODES.forEach(n => {
    if (!showExtStruct && n.is_external) return;
    if (n.kind) visibleKinds.add(n.kind);
  });
  const legItems = [{ label: 'Dossier', color: '#607D8B' }];
  Object.entries(NODE_COLORS).forEach(([k, c]) => {
    if (visibleKinds.has(k)) legItems.push({ label: NODE_LABELS[k] || k, color: c });
  });

  // ── Dimensions (logiques — canvas sortie = ×EXPORT_SCALE) ───────────────────
  // SVG est vectoriel : rendu source à taille raisonnable, ctx.scale() fait le reste.
  const EXPORT_SCALE = 3;
  const RAW_W  = parseInt(svg.getAttribute('width'))  || 800;
  const RAW_H  = parseInt(svg.getAttribute('height')) || 600;
  // Cap source : 4 500 px source × 3 = 13 500 px sortie (sous la limite canvas ~16 384)
  const MAX_SRC  = 4500;
  const srcScale = Math.min(1, MAX_SRC / Math.max(RAW_W, RAW_H));
  const svgW   = Math.round(RAW_W * srcScale);
  const svgH   = Math.round(RAW_H * srcScale);

  const PAD        = 24;
  const TITLE_H    = 50;
  const LEG_FONT   = 22;   // px logiques — rendu à LEG_FONT*EXPORT_SCALE dans le PNG
  const LEG_ITEM_H = Math.round(LEG_FONT * 2.6);   // hauteur de ligne proportionnelle
  const MIN_COL_W  = Math.round(LEG_FONT * 10);    // largeur min colonne pour le texte
  const LEG_COLS   = Math.max(1, Math.min(legItems.length, Math.floor(Math.max(svgW, 400) / MIN_COL_W)));
  const LEG_ROWS   = Math.ceil(legItems.length / LEG_COLS);
  const LEG_H      = LEG_ROWS * LEG_ITEM_H;

  const totalW = Math.max(svgW, 400);
  const totalH = PAD + TITLE_H + PAD + svgH + PAD + LEG_H + PAD;

  // ── Rendu asynchrone via <img> ────────────────────────────────────────────
  const ser    = new XMLSerializer().serializeToString(svg);
  const blob   = new Blob([ser], { type: 'image/svg+xml;charset=utf-8' });
  const svgUrl = URL.createObjectURL(blob);

  const img = new Image();
  img.onload = () => {
    // Canvas ×EXPORT_SCALE ; ctx.scale() préserve les coordonnées logiques.
    const out = document.createElement('canvas');
    out.width  = totalW * EXPORT_SCALE;
    out.height = totalH * EXPORT_SCALE;
    const c = out.getContext('2d');
    c.imageSmoothingEnabled = true;
    c.imageSmoothingQuality = 'high';
    c.scale(EXPORT_SCALE, EXPORT_SCALE);

    // Fond
    c.fillStyle = '#0d1117';
    c.fillRect(0, 0, totalW, totalH);

    // Titre
    c.fillStyle    = '#e6edf3';
    c.font         = 'bold 26px sans-serif';
    c.textAlign    = 'center';
    c.textBaseline = 'middle';
    c.fillText(titleText, totalW / 2, PAD + TITLE_H / 2);

    // Séparateur titre / contenu
    c.strokeStyle = '#30363d';
    c.lineWidth   = 1;
    c.beginPath();
    c.moveTo(PAD, PAD + TITLE_H);
    c.lineTo(totalW - PAD, PAD + TITLE_H);
    c.stroke();

    // SVG centré horizontalement
    const imgX = Math.round((totalW - svgW) / 2);
    c.drawImage(img, imgX, PAD + TITLE_H + PAD, svgW, svgH);
    URL.revokeObjectURL(svgUrl);

    // Séparateur contenu / légende
    const legTop = PAD + TITLE_H + PAD + svgH + PAD;
    c.strokeStyle = '#30363d';
    c.lineWidth   = 1;
    c.beginPath();
    c.moveTo(PAD, legTop - PAD / 2);
    c.lineTo(totalW - PAD, legTop - PAD / 2);
    c.stroke();

    // Légende
    const colW  = Math.floor(totalW / LEG_COLS);
    const iconR = Math.round(LEG_FONT * 0.55);
    const off   = Math.round(colW * 0.30);   // décalage icône depuis cx
    legItems.forEach((item, idx) => {
      const col = idx % LEG_COLS;
      const row = Math.floor(idx / LEG_COLS);
      const cx  = colW * col + colW / 2;
      const cy  = legTop + row * LEG_ITEM_H + LEG_ITEM_H / 2;
      // Carré de couleur
      c.fillStyle = item.color;
      c.fillRect(cx - off - iconR, cy - iconR, iconR * 2, iconR * 2);
      // Libellé
      c.fillStyle    = '#eeeeee';
      c.font         = 'bold ' + LEG_FONT + 'px sans-serif';
      c.textAlign    = 'left';
      c.textBaseline = 'middle';
      c.fillText(item.label, cx - off + iconR + 6, cy);
    });

    // Téléchargement
    const a = document.createElement('a');
    a.href     = out.toDataURL('image/png');
    a.download = titleText.toLowerCase().replace(/\s+/g, '_') + '_structure.png';
    a.click();
    setStatus('PNG exporté — ' + out.width + ' × ' + out.height + ' px');
  };
  img.onerror = () => {
    URL.revokeObjectURL(svgUrl);
    setStatus('Erreur rendu SVG → PNG');
  };
  img.src = svgUrl;
}

// ── Init ─────────────────────────────────────────────────────────────────────
buildFilters();
fsTree = buildFsTree();
renderFsNav(fsTree, null);
applyFilters();
"##;

// ─── Rust export function ─────────────────────────────────────────────────────

pub fn export_html(
    graph: &CodeGraph,
    out: &Path,
    analysis_opt: Option<&GraphAnalysis>,
    diff_opt: Option<&GraphDiff>,
    root_dir: Option<&Path>,
) -> Result<()> {
    let sg          = graph.to_serializable();
    let nodes_json  = serde_json::to_string(&sg.nodes)?;
    let edges_json  = serde_json::to_string(&sg.edges)?;
    let stats_json  = serde_json::to_string(&sg.stats)?;

    // ── Métriques : top-10 pour le panneau + map complet pour le filtre ───────
    let (metrics_top_json, metrics_all_json) = if let Some(a) = analysis_opt {
        let mut sorted: Vec<_> = a.metrics.iter()
            .map(|(id, m)| (id, m.coupling_score, m.in_degree, m.out_degree, m.depth_of_inheritance))
            .collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Top-10 pour le panneau latéral
        let top: Vec<serde_json::Value> = sorted.iter().take(10).map(|(id, score, ind, outd, dit)| {
            serde_json::json!({ "id": id, "score": score, "in_degree": ind, "out_degree": outd, "dit": dit })
        }).collect();

        // Map complet pour le filtre JS
        let all: serde_json::Value = a.metrics.iter()
            .map(|(id, m)| {
                (id.clone(), serde_json::json!({
                    "score": m.coupling_score,
                    "in_degree": m.in_degree,
                    "out_degree": m.out_degree,
                    "dit": m.depth_of_inheritance,
                }))
            })
            .collect::<serde_json::Map<_, _>>()
            .into();

        (serde_json::to_string(&top)?, serde_json::to_string(&all)?)
    } else {
        ("[]".into(), "{}".into())
    };

    let cycles_json = if let Some(a) = analysis_opt {
        serde_json::to_string(&a.cycles)?
    } else { "[]".into() };

    let components_json = if let Some(a) = analysis_opt {
        serde_json::to_string(&a.components)?
    } else { "[]".into() };

    let orphans_json = if let Some(a) = analysis_opt {
        serde_json::to_string(&a.orphan_nodes)?
    } else { "[]".into() };

    let diff_json = if let Some(d) = diff_opt {
        let added:   Vec<&str> = d.added_nodes.iter().map(|n| n.id.as_str()).collect();
        let removed: Vec<&str> = d.removed_nodes.iter().map(|n| n.id.as_str()).collect();
        serde_json::json!({ "added_node_ids": added, "removed_node_ids": removed }).to_string()
    } else { "null".into() };

    let root_dir_str = root_dir
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();

    let data = format!(
        "const RAW_NODES={nodes};\nconst RAW_EDGES={edges};\nconst STATS={stats};\n\
         const METRICS_TOP={metrics_top};\nconst METRICS_ALL={metrics_all};\n\
         const DIFF={diff};\nconst CYCLES={cycles};\n\
         const COMPONENTS={comps};\nconst ORPHAN_NODES={orphans};\nconst ROOT_DIR='{root}';",
        nodes       = nodes_json,
        edges       = edges_json,
        stats       = stats_json,
        metrics_top = metrics_top_json,
        metrics_all = metrics_all_json,
        diff        = diff_json,
        cycles      = cycles_json,
        comps       = components_json,
        orphans     = orphans_json,
        root        = root_dir_str,
    );

    let html = HTML_TEMPLATE
        .replace("__DATA__", &data)
        .replace("__APP__", APP_JS);

    std::fs::write(out, html)?;
    Ok(())
}
