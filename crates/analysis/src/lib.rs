use anyhow::Result;
use core::{CodeGraph, Edge, EdgeKind, Node};
use parsers::Language;
use petgraph::algo::kosaraju_scc;
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

// ── Metrics types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub in_degree: usize,
    pub out_degree: usize,
    /// Normalised coupling score ∈ [0.0, 1.0]
    pub coupling_score: f32,
    /// Depth of inheritance tree (0 = no parent, 1 = one level of inheritance…)
    pub depth_of_inheritance: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphAnalysis {
    pub cycles: Vec<Vec<String>>,
    pub metrics: HashMap<String, NodeMetrics>,
    pub components: Vec<Vec<String>>,
    /// IDs of nodes that have zero incoming **and** zero outgoing edges
    pub orphan_nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDiff {
    pub added_nodes: Vec<Node>,
    pub removed_nodes: Vec<Node>,
    pub added_edges: Vec<Edge>,
    pub removed_edges: Vec<Edge>,
}

// ── Analysis ──────────────────────────────────────────────────────────────────

pub fn analyze(graph: &CodeGraph) -> GraphAnalysis {
    let total_nodes = graph.graph.node_count();
    let max_degree = if total_nodes > 1 { (total_nodes - 1) * 2 } else { 1 };

    // ── Depth of Inheritance Tree ─────────────────────────────────────────────
    let dit_map = compute_dit(graph);

    // ── Per-node metrics ──────────────────────────────────────────────────────
    let mut metrics: HashMap<String, NodeMetrics> = HashMap::new();
    let mut orphan_nodes: Vec<String> = Vec::new();

    for node in graph.nodes() {
        let idx = match graph.index_map.get(&node.id) {
            Some(&i) => i,
            None => continue,
        };
        let in_d  = graph.graph.neighbors_directed(idx, petgraph::Incoming).count();
        let out_d = graph.graph.neighbors_directed(idx, petgraph::Outgoing).count();
        let coupling = (in_d + out_d) as f32 / max_degree as f32;
        let dit = *dit_map.get(&node.id).unwrap_or(&0);

        if in_d == 0 && out_d == 0 {
            orphan_nodes.push(node.id.clone());
        }

        metrics.insert(
            node.id.clone(),
            NodeMetrics {
                in_degree: in_d,
                out_degree: out_d,
                coupling_score: coupling,
                depth_of_inheritance: dit,
            },
        );
    }

    // ── Cycles (Kosaraju SCC) ─────────────────────────────────────────────────
    let sccs = kosaraju_scc(&graph.graph);
    let mut cycles: Vec<Vec<String>> = Vec::new();
    for scc in &sccs {
        if scc.len() > 1 {
            let ids: Vec<String> = scc
                .iter()
                .filter_map(|&idx| graph.graph.node_weight(idx))
                .map(|n| n.id.clone())
                .collect();
            cycles.push(ids);
        }
    }

    // ── Connected components (BFS sur graphe non-orienté) ────────────────────
    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut components: Vec<Vec<String>> = Vec::new();

    for node in graph.nodes() {
        let idx = match graph.index_map.get(&node.id) {
            Some(&i) => i,
            None => continue,
        };
        if visited.contains(&idx) {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(idx);
        visited.insert(idx);

        while let Some(curr) = queue.pop_front() {
            if let Some(n) = graph.graph.node_weight(curr) {
                component.push(n.id.clone());
            }
            for nb in graph
                .graph
                .neighbors_directed(curr, petgraph::Outgoing)
                .chain(graph.graph.neighbors_directed(curr, petgraph::Incoming))
            {
                if !visited.contains(&nb) {
                    visited.insert(nb);
                    queue.push_back(nb);
                }
            }
        }
        components.push(component);
    }

    GraphAnalysis {
        cycles,
        metrics,
        components,
        orphan_nodes,
    }
}

// ── DIT computation ───────────────────────────────────────────────────────────

/// Compute the depth of inheritance tree for every node.
///
/// DIT(node) = 0 if node has no Inherits-out edges,
///           = 1 + max(DIT(parents)) otherwise.
fn compute_dit(graph: &CodeGraph) -> HashMap<String, usize> {
    let mut dit: HashMap<String, usize> = HashMap::new();
    let node_ids: Vec<String> = graph.nodes().map(|n| n.id.clone()).collect();

    for id in &node_ids {
        if !dit.contains_key(id) {
            let mut visited = HashSet::new();
            compute_dit_rec(graph, id, &mut dit, &mut visited);
        }
    }

    dit
}

fn compute_dit_rec(
    graph: &CodeGraph,
    node_id: &str,
    dit: &mut HashMap<String, usize>,
    visited: &mut HashSet<String>,
) -> usize {
    if let Some(&cached) = dit.get(node_id) {
        return cached;
    }
    // Cycle guard
    if !visited.insert(node_id.to_string()) {
        return 0;
    }

    let idx = match graph.index_map.get(node_id) {
        Some(&i) => i,
        None => {
            dit.insert(node_id.to_string(), 0);
            return 0;
        }
    };

    // Traverse Inherits edges going outward (A inherits B → B is the parent)
    let max_parent: usize = graph
        .graph
        .edges_directed(idx, petgraph::Outgoing)
        .filter(|e| *e.weight() == EdgeKind::Inherits)
        .map(|e| {
            if let Some(parent) = graph.graph.node_weight(e.target()) {
                compute_dit_rec(graph, &parent.id, dit, visited)
            } else {
                0
            }
        })
        .max()
        .unwrap_or(0);

    let depth = if max_parent == 0 &&
        graph
            .graph
            .edges_directed(idx, petgraph::Outgoing)
            .filter(|e| *e.weight() == EdgeKind::Inherits)
            .count()
            == 0
    {
        0 // no parents → DIT = 0
    } else {
        max_parent + 1
    };

    dit.insert(node_id.to_string(), depth);
    visited.remove(node_id);
    depth
}

// ── Diff ──────────────────────────────────────────────────────────────────────

pub fn diff(old: &CodeGraph, new: &CodeGraph) -> GraphDiff {
    let old_ids: HashSet<&str> = old.nodes().map(|n| n.id.as_str()).collect();
    let new_ids: HashSet<&str> = new.nodes().map(|n| n.id.as_str()).collect();

    let added_nodes: Vec<Node> = new
        .nodes()
        .filter(|n| !old_ids.contains(n.id.as_str()))
        .cloned()
        .collect();

    let removed_nodes: Vec<Node> = old
        .nodes()
        .filter(|n| !new_ids.contains(n.id.as_str()))
        .cloned()
        .collect();

    let old_edges: HashSet<(String, String, String)> = old
        .edges()
        .map(|(s, t, k)| (s.id.clone(), t.id.clone(), k.to_string()))
        .collect();
    let new_edges: HashSet<(String, String, String)> = new
        .edges()
        .map(|(s, t, k)| (s.id.clone(), t.id.clone(), k.to_string()))
        .collect();

    let added_edges: Vec<Edge> = new
        .edges()
        .filter(|(s, t, k)| {
            !old_edges.contains(&(s.id.clone(), t.id.clone(), k.to_string()))
        })
        .map(|(s, t, k)| Edge {
            source: s.id.clone(),
            target: t.id.clone(),
            kind: k.clone(),
        })
        .collect();

    let removed_edges: Vec<Edge> = old
        .edges()
        .filter(|(s, t, k)| {
            !new_edges.contains(&(s.id.clone(), t.id.clone(), k.to_string()))
        })
        .map(|(s, t, k)| Edge {
            source: s.id.clone(),
            target: t.id.clone(),
            kind: k.clone(),
        })
        .collect();

    GraphDiff {
        added_nodes,
        removed_nodes,
        added_edges,
        removed_edges,
    }
}

// ── Focus ─────────────────────────────────────────────────────────────────────

pub fn focus(graph: &CodeGraph, node_id: &str, depth: usize) -> CodeGraph {
    let mut result = CodeGraph::new();

    let start_idx = match graph.index_map.get(node_id) {
        Some(&idx) => idx,
        None => return result,
    };

    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::new();
    queue.push_back((start_idx, 0));
    visited.insert(start_idx);

    while let Some((idx, level)) = queue.pop_front() {
        if let Some(node) = graph.graph.node_weight(idx) {
            result.add_node(node.clone());
        }
        if level < depth {
            for nb in graph
                .graph
                .neighbors_directed(idx, petgraph::Outgoing)
                .chain(graph.graph.neighbors_directed(idx, petgraph::Incoming))
            {
                if !visited.contains(&nb) {
                    visited.insert(nb);
                    queue.push_back((nb, level + 1));
                }
            }
        }
    }

    for e in graph.graph.edge_indices() {
        if let Some((s, t)) = graph.graph.edge_endpoints(e) {
            if visited.contains(&s) && visited.contains(&t) {
                if let (Some(sn), Some(tn), Some(ek)) = (
                    graph.graph.node_weight(s),
                    graph.graph.node_weight(t),
                    graph.graph.edge_weight(e),
                ) {
                    result.add_edge(&sn.id, &tn.id, ek.clone());
                }
            }
        }
    }

    result
}

// ── Git diff analysis ─────────────────────────────────────────────────────────

pub fn git_diff_analyze(
    root: &Path,
    git_ref: &str,
    languages: &[Language],
) -> Result<GraphDiff> {
    use std::process::Command;

    let output = Command::new("git")
        .args(["ls-tree", "-r", "--name-only", git_ref])
        .current_dir(root)
        .output()
        .map_err(|e| anyhow::anyhow!("git ls-tree failed: {}", e))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "git ls-tree error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let file_list = String::from_utf8_lossy(&output.stdout);
    let mut old_graph = CodeGraph::new();
    let mut contexts = Vec::new();

    for file_path in file_list.lines() {
        let lang = match file_path.split('.').last() {
            Some("py")  if languages.contains(&Language::Python)     => Language::Python,
            Some("rs")  if languages.contains(&Language::Rust)       => Language::Rust,
            Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") | Some("h")
                if languages.contains(&Language::Cpp) => Language::Cpp,
            Some("ts") | Some("tsx") if languages.contains(&Language::TypeScript) => Language::TypeScript,
            Some("js") | Some("jsx") if languages.contains(&Language::JavaScript) => Language::JavaScript,
            _ => continue,
        };

        let content_output = Command::new("git")
            .args(["show", &format!("{}:{}", git_ref, file_path)])
            .current_dir(root)
            .output()
            .map_err(|e| anyhow::anyhow!("git show failed: {}", e))?;

        if !content_output.status.success() {
            continue;
        }

        let source  = String::from_utf8_lossy(&content_output.stdout).to_string();
        let rel_path = file_path.replace('\\', "/");

        let ctx = match lang {
            Language::Python     => parsers::python::parse_file(&source, &rel_path, &mut old_graph),
            Language::Rust       => parsers::rust_lang::parse_file(&source, &rel_path, &mut old_graph),
            Language::Cpp        => parsers::cpp::parse_file(&source, &rel_path, &mut old_graph),
            Language::TypeScript => parsers::typescript::parse_file(&source, &rel_path, &mut old_graph, parsers::typescript::JsVariant::TypeScript),
            Language::JavaScript => parsers::typescript::parse_file(&source, &rel_path, &mut old_graph, parsers::typescript::JsVariant::JavaScript),
        };

        if let Ok(c) = ctx {
            contexts.push(c);
        }
    }

    parsers::Resolver::resolve_all(contexts, &mut old_graph);

    let new_graph = parsers::analyze(root, languages, false)?;

    Ok(diff(&old_graph, &new_graph))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use core::{CodeGraph, EdgeKind, Node, NodeKind};

    fn node(id: &str, kind: NodeKind) -> Node {
        Node {
            id: id.to_string(),
            name: id.to_string(),
            kind,
            file: "test.rs".to_string(),
            line: 1,
            is_external: false,
            docstring: None,
        }
    }

    fn make_graph_with_cycle() -> CodeGraph {
        let mut g = CodeGraph::new();
        g.add_node(node("A", NodeKind::Class));
        g.add_node(node("B", NodeKind::Class));
        g.add_node(node("C", NodeKind::Function));
        // A → B → A (cycle), C is orphan
        g.add_edge("A", "B", EdgeKind::Calls);
        g.add_edge("B", "A", EdgeKind::Calls);
        g
    }

    #[test]
    fn test_cycle_detection() {
        let g = make_graph_with_cycle();
        let a = analyze(&g);
        assert!(!a.cycles.is_empty(), "Should detect at least one cycle");
        let all_cycle_nodes: HashSet<&str> = a.cycles.iter().flatten().map(|s| s.as_str()).collect();
        assert!(all_cycle_nodes.contains("A"));
        assert!(all_cycle_nodes.contains("B"));
    }

    #[test]
    fn test_orphan_detection() {
        let g = make_graph_with_cycle();
        let a = analyze(&g);
        assert!(a.orphan_nodes.contains(&"C".to_string()), "C should be an orphan");
        assert!(!a.orphan_nodes.contains(&"A".to_string()));
    }

    #[test]
    fn test_dit_no_inheritance() {
        let mut g = CodeGraph::new();
        g.add_node(node("Base", NodeKind::Class));
        let a = analyze(&g);
        assert_eq!(a.metrics["Base"].depth_of_inheritance, 0);
    }

    #[test]
    fn test_dit_one_level() {
        let mut g = CodeGraph::new();
        g.add_node(node("Base", NodeKind::Class));
        g.add_node(node("Child", NodeKind::Class));
        g.add_edge("Child", "Base", EdgeKind::Inherits);
        let a = analyze(&g);
        assert_eq!(a.metrics["Child"].depth_of_inheritance, 1);
        assert_eq!(a.metrics["Base"].depth_of_inheritance, 0);
    }

    #[test]
    fn test_dit_two_levels() {
        let mut g = CodeGraph::new();
        g.add_node(node("Base", NodeKind::Class));
        g.add_node(node("Mid", NodeKind::Class));
        g.add_node(node("Leaf", NodeKind::Class));
        g.add_edge("Mid", "Base", EdgeKind::Inherits);
        g.add_edge("Leaf", "Mid", EdgeKind::Inherits);
        let a = analyze(&g);
        assert_eq!(a.metrics["Leaf"].depth_of_inheritance, 2);
        assert_eq!(a.metrics["Mid"].depth_of_inheritance, 1);
    }

    #[test]
    fn test_components() {
        let mut g = CodeGraph::new();
        g.add_node(node("X", NodeKind::Function));
        g.add_node(node("Y", NodeKind::Function));
        g.add_node(node("Z", NodeKind::Function));
        g.add_edge("X", "Y", EdgeKind::Calls);
        // Z is disconnected
        let a = analyze(&g);
        assert_eq!(a.components.len(), 2);
    }

    #[test]
    fn test_coupling_score_range() {
        let g = make_graph_with_cycle();
        let a = analyze(&g);
        for (_, m) in &a.metrics {
            assert!(m.coupling_score >= 0.0);
            assert!(m.coupling_score <= 1.0);
        }
    }

    #[test]
    fn test_diff_added_removed() {
        let mut old = CodeGraph::new();
        old.add_node(node("common", NodeKind::Function));
        old.add_node(node("removed", NodeKind::Function));

        let mut new_g = CodeGraph::new();
        new_g.add_node(node("common", NodeKind::Function));
        new_g.add_node(node("added", NodeKind::Function));

        let d = diff(&old, &new_g);
        assert!(d.added_nodes.iter().any(|n| n.id == "added"));
        assert!(d.removed_nodes.iter().any(|n| n.id == "removed"));
    }
}
