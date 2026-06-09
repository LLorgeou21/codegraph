pub mod html;

use anyhow::Result;
use core::{CodeGraph, EdgeKind, NodeKind};
use std::path::Path;

pub fn export_json(graph: &CodeGraph, out: &Path) -> Result<()> {
    let sg = graph.to_serializable();
    let json = serde_json::to_string_pretty(&sg)?;
    std::fs::write(out, json)?;
    Ok(())
}

pub fn export_dot(graph: &CodeGraph, out: &Path) -> Result<()> {
    let mut dot = String::new();
    dot.push_str("digraph codegraph {\n");
    dot.push_str("    rankdir=LR;\n");
    dot.push_str("    node [shape=box, fontname=\"Helvetica\"];\n");
    dot.push_str("    edge [fontname=\"Helvetica\", fontsize=10];\n\n");

    for node in graph.nodes() {
        let color = node_color(&node.kind);
        let label = format!("{} [{}]", node.name, node.kind);
        let safe_id = safe_dot_id(&node.id);
        dot.push_str(&format!(
            "    {} [label=\"{}\", color=\"{}\", style=filled, fillcolor=\"{}\"];\n",
            safe_id,
            escape_dot(&label),
            color,
            lighten(color),
        ));
    }

    dot.push('\n');

    for (src, tgt, kind) in graph.edges() {
        let src_id = safe_dot_id(&src.id);
        let tgt_id = safe_dot_id(&tgt.id);
        let edge_color = edge_color(kind);
        let label = kind.to_string();
        dot.push_str(&format!(
            "    {} -> {} [label=\"{}\", color=\"{}\"];\n",
            src_id, tgt_id, label, edge_color,
        ));
    }

    dot.push_str("}\n");
    std::fs::write(out, dot)?;
    Ok(())
}

fn safe_dot_id(id: &str) -> String {
    format!(
        "n_{}",
        id.chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>()
    )
}

fn escape_dot(s: &str) -> String {
    s.replace('"', "\\\"")
}

pub fn node_color(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Module   => "#4A90D9",
        NodeKind::Class    => "#E74C3C",
        NodeKind::Struct   => "#1ABC9C",
        NodeKind::Function => "#2ECC71",
        NodeKind::Method   => "#F39C12",
        NodeKind::Property => "#9B59B6",
        NodeKind::Constant => "#E67E22",
    }
}

pub fn lighten(color: &str) -> String {
    match color {
        "#4A90D9" => "#AED0F0".to_string(),
        "#E74C3C" => "#F7B7B2".to_string(),
        "#1ABC9C" => "#A2DDD6".to_string(),
        "#2ECC71" => "#A3E9C5".to_string(),
        "#F39C12" => "#FAD7A0".to_string(),
        "#9B59B6" => "#D2B4DE".to_string(),
        "#E67E22" => "#FAD3A2".to_string(),
        _ => "#EEEEEE".to_string(),
    }
}

pub fn edge_color(kind: &EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Contains    => "#BDC3C7",
        EdgeKind::Imports     => "#7F8C8D",
        EdgeKind::Inherits    => "#9B59B6",
        EdgeKind::Calls       => "#3498DB",
        EdgeKind::UsesType    => "#1ABC9C",
        EdgeKind::ExternalDep => "#E67E22",
        EdgeKind::FieldType   => "#F39C12",
    }
}
