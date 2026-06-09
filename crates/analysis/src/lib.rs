use anyhow::Result;
use core::{CodeGraph, Edge, Node};
use parsers::Language;
use petgraph::algo::kosaraju_scc;
use petgraph::stable_graph::NodeIndex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub in_degree: usize,
    pub out_degree: usize,
    pub coupling_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphAnalysis {
    pub cycles: Vec<Vec<String>>,
    pub metrics: HashMap<String, NodeMetrics>,
    pub components: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDiff {
    pub added_nodes: Vec<Node>,
    pub removed_nodes: Vec<Node>,
    pub added_edges: Vec<Edge>,
    pub removed_edges: Vec<Edge>,
}

pub fn analyze(graph: &CodeGraph) -> GraphAnalysis {
    // Compute in/out degrees
    let total_nodes = graph.graph.node_count();
    let max_degree = if total_nodes > 1 { (total_nodes - 1) * 2 } else { 1 };

    let mut metrics: HashMap<String, NodeMetrics> = HashMap::new();

    for node in graph.nodes() {
        let idx = match graph.index_map.get(&node.id) {
            Some(&i) => i,
            None => continue,
        };
        let in_d = graph.graph.neighbors_directed(idx, petgraph::Incoming).count();
        let out_d = graph.graph.neighbors_directed(idx, petgraph::Outgoing).count();
        let coupling = (in_d + out_d) as f32 / max_degree as f32;
        metrics.insert(
            node.id.clone(),
            NodeMetrics {
                in_degree: in_d,
                out_degree: out_d,
                coupling_score: coupling,
            },
        );
    }

    // Find cycles using Kosaraju SCC
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

    // Weakly connected components
    let undirected: petgraph::graph::UnGraph<(), ()> = petgraph::graph::UnGraph::from_edges(
        graph
            .graph
            .edge_indices()
            .filter_map(|e| {
                let (s, t) = graph.graph.edge_endpoints(e)?;
                Some((s.index() as u32, t.index() as u32))
            })
            .collect::<Vec<_>>(),
    );

    // Use a simple BFS to find connected components on the stable graph
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
        // BFS
        let mut component = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(idx);
        visited.insert(idx);

        while let Some(curr) = queue.pop_front() {
            if let Some(n) = graph.graph.node_weight(curr) {
                component.push(n.id.clone());
            }
            // Both directions for undirected component
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

    let _ = undirected;

    GraphAnalysis {
        cycles,
        metrics,
        components,
    }
}

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

    // Collect edges as (source_id, target_id, kind) sets
    let old_edges: HashSet<(String, String, String)> = old
        .edges()
        .map(|(s, t, k)| (s.id.clone(), t.id.clone(), format!("{:?}", k)))
        .collect();
    let new_edges: HashSet<(String, String, String)> = new
        .edges()
        .map(|(s, t, k)| (s.id.clone(), t.id.clone(), format!("{:?}", k)))
        .collect();

    let added_edges: Vec<Edge> = new
        .edges()
        .filter(|(s, t, k)| !old_edges.contains(&(s.id.clone(), t.id.clone(), format!("{:?}", k))))
        .map(|(s, t, k)| Edge {
            source: s.id.clone(),
            target: t.id.clone(),
            kind: k.clone(),
        })
        .collect();

    let removed_edges: Vec<Edge> = old
        .edges()
        .filter(|(s, t, k)| !new_edges.contains(&(s.id.clone(), t.id.clone(), format!("{:?}", k))))
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

pub fn focus(graph: &CodeGraph, node_id: &str, depth: usize) -> CodeGraph {
    let mut result = CodeGraph::new();

    let start_idx = match graph.index_map.get(node_id) {
        Some(&idx) => idx,
        None => return result,
    };

    // BFS up to `depth` levels
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

    // Add edges between collected nodes
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

pub fn git_diff_analyze(
    root: &Path,
    git_ref: &str,
    languages: &[Language],
) -> Result<GraphDiff> {
    use std::process::Command;

    // Get list of tracked files at this ref
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
            Some("py") if languages.contains(&Language::Python) => Language::Python,
            Some("rs") if languages.contains(&Language::Rust) => Language::Rust,
            Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") | Some("h")
                if languages.contains(&Language::Cpp) =>
            {
                Language::Cpp
            }
            _ => continue,
        };

        // Get file content at git ref
        let content_output = Command::new("git")
            .args(["show", &format!("{}:{}", git_ref, file_path)])
            .current_dir(root)
            .output()
            .map_err(|e| anyhow::anyhow!("git show failed: {}", e))?;

        if !content_output.status.success() {
            continue;
        }

        let source = String::from_utf8_lossy(&content_output.stdout).to_string();
        let rel_path = file_path.replace('\\', "/");

        let ctx = match lang {
            Language::Python => parsers::python::parse_file(&source, &rel_path, &mut old_graph),
            Language::Rust => parsers::rust_lang::parse_file(&source, &rel_path, &mut old_graph),
            Language::Cpp => parsers::cpp::parse_file(&source, &rel_path, &mut old_graph),
        };

        if let Ok(c) = ctx {
            contexts.push(c);
        }
    }

    parsers::Resolver::resolve_all(contexts, &mut old_graph);

    // Build current graph
    let new_graph = parsers::analyze(root, languages, false)?;

    Ok(diff(&old_graph, &new_graph))
}
