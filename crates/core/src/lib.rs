use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Module,
    Class,
    Struct,   // distinct from Class (Rust structs, C++ structs)
    Function,
    Method,
    Property,
    Constant,
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeKind::Module   => write!(f, "module"),
            NodeKind::Class    => write!(f, "class"),
            NodeKind::Struct   => write!(f, "struct"),
            NodeKind::Function => write!(f, "function"),
            NodeKind::Method   => write!(f, "method"),
            NodeKind::Property => write!(f, "property"),
            NodeKind::Constant => write!(f, "constant"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Imports,
    Inherits,
    Calls,
    UsesType,
    ExternalDep,
    FieldType,
}

impl std::fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeKind::Contains    => write!(f, "contains"),
            EdgeKind::Imports     => write!(f, "imports"),
            EdgeKind::Inherits    => write!(f, "inherits"),
            EdgeKind::Calls       => write!(f, "calls"),
            EdgeKind::UsesType    => write!(f, "uses_type"),
            EdgeKind::ExternalDep => write!(f, "external_dep"),
            EdgeKind::FieldType   => write!(f, "field_type"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    pub file: String,
    pub line: usize,
    pub is_external: bool,
    pub docstring: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub module_count: usize,
    pub class_count: usize,
    pub struct_count: usize,
    pub function_count: usize,
    pub method_count: usize,
    pub property_count: usize,
    pub constant_count: usize,
    pub external_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CodeGraph {
    #[serde(skip)]
    pub graph: StableDiGraph<Node, EdgeKind>,
    pub index_map: HashMap<String, NodeIndex>,
}

impl Default for CodeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeGraph {
    pub fn new() -> Self {
        CodeGraph {
            graph: StableDiGraph::new(),
            index_map: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: Node) -> NodeIndex {
        if let Some(&idx) = self.index_map.get(&node.id) {
            return idx;
        }
        let id = node.id.clone();
        let idx = self.graph.add_node(node);
        self.index_map.insert(id, idx);
        idx
    }

    pub fn add_edge(&mut self, source: &str, target: &str, kind: EdgeKind) {
        let src_idx = match self.index_map.get(source) {
            Some(&idx) => idx,
            None => return,
        };
        let tgt_idx = match self.index_map.get(target) {
            Some(&idx) => idx,
            None => return,
        };
        // Avoid duplicate edges
        if !self
            .graph
            .edges_connecting(src_idx, tgt_idx)
            .any(|e| *e.weight() == kind)
        {
            self.graph.add_edge(src_idx, tgt_idx, kind);
        }
    }

    pub fn has_node(&self, id: &str) -> bool {
        self.index_map.contains_key(id)
    }

    pub fn get_node(&self, id: &str) -> Option<&Node> {
        self.index_map
            .get(id)
            .and_then(|&idx| self.graph.node_weight(idx))
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.graph.node_weights()
    }

    pub fn edges(&self) -> impl Iterator<Item = (&Node, &Node, &EdgeKind)> {
        self.graph.edge_indices().filter_map(move |e| {
            let (s, t) = self.graph.edge_endpoints(e)?;
            let src = self.graph.node_weight(s)?;
            let tgt = self.graph.node_weight(t)?;
            let kind = self.graph.edge_weight(e)?;
            Some((src, tgt, kind))
        })
    }

    pub fn stats(&self) -> GraphStats {
        let mut stats = GraphStats {
            node_count: 0,
            edge_count: self.graph.edge_count(),
            module_count: 0,
            class_count: 0,
            struct_count: 0,
            function_count: 0,
            method_count: 0,
            property_count: 0,
            constant_count: 0,
            external_count: 0,
        };
        for node in self.nodes() {
            stats.node_count += 1;
            if node.is_external {
                stats.external_count += 1;
            }
            match node.kind {
                NodeKind::Module   => stats.module_count += 1,
                NodeKind::Class    => stats.class_count += 1,
                NodeKind::Struct   => stats.struct_count += 1,
                NodeKind::Function => stats.function_count += 1,
                NodeKind::Method   => stats.method_count += 1,
                NodeKind::Property => stats.property_count += 1,
                NodeKind::Constant => stats.constant_count += 1,
            }
        }
        stats
    }

    pub fn to_serializable(&self) -> SerializableGraph {
        let nodes: Vec<Node> = self.graph.node_weights().cloned().collect();
        let edges: Vec<Edge> = self
            .graph
            .edge_indices()
            .filter_map(|e| {
                let (s, t) = self.graph.edge_endpoints(e)?;
                let src = self.graph.node_weight(s)?;
                let tgt = self.graph.node_weight(t)?;
                let kind = self.graph.edge_weight(e)?.clone();
                Some(Edge {
                    source: src.id.clone(),
                    target: tgt.id.clone(),
                    kind,
                })
            })
            .collect();
        SerializableGraph {
            stats: self.stats(),
            nodes,
            edges,
        }
    }

    pub fn from_serializable(sg: SerializableGraph) -> Self {
        let mut graph = CodeGraph::new();
        for node in sg.nodes {
            graph.add_node(node);
        }
        for edge in sg.edges {
            graph.add_edge(&edge.source, &edge.target, edge.kind);
        }
        graph
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableGraph {
    pub stats: GraphStats,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: &str, kind: NodeKind) -> Node {
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

    #[test]
    fn test_add_node_deduplication() {
        let mut g = CodeGraph::new();
        let idx1 = g.add_node(make_node("a", NodeKind::Function));
        let idx2 = g.add_node(make_node("a", NodeKind::Function));
        assert_eq!(idx1, idx2, "Same node inserted twice should return same index");
        assert_eq!(g.graph.node_count(), 1);
    }

    #[test]
    fn test_add_edge_deduplication() {
        let mut g = CodeGraph::new();
        g.add_node(make_node("a", NodeKind::Module));
        g.add_node(make_node("b", NodeKind::Function));
        g.add_edge("a", "b", EdgeKind::Contains);
        g.add_edge("a", "b", EdgeKind::Contains); // duplicate
        assert_eq!(g.graph.edge_count(), 1, "Duplicate edges should be deduplicated");
    }

    #[test]
    fn test_different_edge_kinds_allowed() {
        let mut g = CodeGraph::new();
        g.add_node(make_node("a", NodeKind::Module));
        g.add_node(make_node("b", NodeKind::Function));
        g.add_edge("a", "b", EdgeKind::Contains);
        g.add_edge("a", "b", EdgeKind::Calls);
        assert_eq!(g.graph.edge_count(), 2);
    }

    #[test]
    fn test_stats_counts() {
        let mut g = CodeGraph::new();
        g.add_node(make_node("m", NodeKind::Module));
        g.add_node(make_node("c", NodeKind::Class));
        g.add_node(make_node("s", NodeKind::Struct));
        g.add_node(make_node("f", NodeKind::Function));
        g.add_node(make_node("me", NodeKind::Method));
        let stats = g.stats();
        assert_eq!(stats.module_count, 1);
        assert_eq!(stats.class_count, 1);
        assert_eq!(stats.struct_count, 1);
        assert_eq!(stats.function_count, 1);
        assert_eq!(stats.method_count, 1);
        assert_eq!(stats.node_count, 5);
    }

    #[test]
    fn test_roundtrip_serialization() {
        let mut g = CodeGraph::new();
        g.add_node(make_node("mod::A", NodeKind::Class));
        g.add_node(make_node("mod::B", NodeKind::Function));
        g.add_edge("mod::A", "mod::B", EdgeKind::Calls);
        let sg = g.to_serializable();
        let g2 = CodeGraph::from_serializable(sg);
        assert_eq!(g2.graph.node_count(), 2);
        assert_eq!(g2.graph.edge_count(), 1);
    }
}
