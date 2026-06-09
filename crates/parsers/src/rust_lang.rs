use crate::{ParseContext, PendingCall};
use anyhow::Result;
use core::{CodeGraph, EdgeKind, Node, NodeKind};
use tree_sitter::{Node as TsNode, Parser};

pub fn parse_file(source: &str, rel_path: &str, graph: &mut CodeGraph) -> Result<ParseContext> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|e| anyhow::anyhow!("Rust language error: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", rel_path))?;

    let module_id = path_to_module_id(rel_path);

    graph.add_node(Node {
        id: module_id.clone(),
        name: module_id.split("::").last().unwrap_or(&module_id).to_string(),
        kind: NodeKind::Module,
        file: rel_path.to_string(),
        line: 1,
        is_external: false,
        docstring: None,
    });

    let mut ctx = ParseContext {
        module_id: module_id.clone(),
        file: rel_path.to_string(),
        ..Default::default()
    };

    let root = tree.root_node();
    process_items(source, root, &module_id, rel_path, graph, &mut ctx, None);

    Ok(ctx)
}

fn path_to_module_id(rel_path: &str) -> String {
    let mut base = rel_path
        .trim_end_matches(".rs")
        .replace(['/', '\\'], "::")
        .trim_start_matches("::")
        .replace("src::", "")
        .to_string();
    // mod.rs and lib.rs define the parent crate/directory's module, not a sub-module named "mod"/"lib"
    if base.ends_with("::mod") || base.ends_with("::lib") {
        base.truncate(base.len() - 5);
    } else if base.ends_with("::main") {
        base.truncate(base.len() - 6);
    }
    base
}

fn node_text<'a>(node: TsNode<'a>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn get_doc_comment(node: TsNode, source: &str) -> Option<String> {
    // Look for preceding line_doc_comment or block_doc_comment
    // In tree-sitter-rust, doc comments are siblings before the node
    let mut result = Vec::new();
    if let Some(parent) = node.parent() {
        let mut cursor = parent.walk();
        let siblings: Vec<TsNode> = parent.children(&mut cursor).collect();
        for (i, sib) in siblings.iter().enumerate() {
            if sib.id() == node.id() {
                // Collect preceding doc comments
                let mut j = i;
                while j > 0 {
                    j -= 1;
                    if siblings[j].kind() == "line_comment" {
                        let text = node_text(siblings[j], source);
                        if text.starts_with("///") {
                            result.push(text.trim_start_matches("///").trim().to_string());
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                result.reverse();
                break;
            }
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(result.join("\n"))
    }
}

fn process_items(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
    impl_type: Option<&str>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "struct_item" => parse_struct(source, child, module_id, rel_path, graph, ctx),
            "enum_item" => parse_enum(source, child, module_id, rel_path, graph, ctx),
            "trait_item" => parse_trait(source, child, module_id, rel_path, graph, ctx),
            "impl_item" => parse_impl(source, child, module_id, rel_path, graph, ctx),
            "function_item" => {
                parse_function(source, child, module_id, rel_path, graph, ctx, impl_type);
            }
            "use_declaration" => parse_use(source, child, module_id, ctx),
            "mod_item" => {
                let mut c2 = child.walk();
                let mut mod_name = String::new();
                let mut inline_body: Option<TsNode> = None;
                for c in child.children(&mut c2) {
                    match c.kind() {
                        "identifier" => mod_name = node_text(c, source).to_string(),
                        "declaration_list" => inline_body = Some(c),
                        _ => {}
                    }
                }
                if mod_name.is_empty() { continue; }

                if let Some(body) = inline_body {
                    // mod foo { ... } — create a sub-module node in this same file
                    let sub_id = format!("{}::{}", module_id, mod_name);
                    let line = child.start_position().row + 1;
                    graph.add_node(Node {
                        id: sub_id.clone(),
                        name: mod_name.clone(),
                        kind: NodeKind::Module,
                        file: rel_path.to_string(),
                        line,
                        is_external: false,
                        docstring: None,
                    });
                    graph.add_edge(module_id, &sub_id, EdgeKind::Contains);
                    process_items(source, body, &sub_id, rel_path, graph, ctx, None);
                } else {
                    // mod foo; — external declaration, resolved in pass 2
                    ctx.pending_mods.push((module_id.to_string(), mod_name));
                }
            }
            _ => {}
        }
    }
}

fn parse_struct(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
) {
    let line = node.start_position().row + 1;
    let mut name = String::new();
    let mut field_types: Vec<String> = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" if name.is_empty() => {
                name = node_text(child, source).to_string();
            }
            "field_declaration_list" => {
                let mut c2 = child.walk();
                for field in child.children(&mut c2) {
                    if field.kind() == "field_declaration" {
                        let mut c3 = field.walk();
                        let mut field_name = String::new();
                        let mut field_type = String::new();
                        for fc in field.children(&mut c3) {
                            match fc.kind() {
                                "field_identifier" => {
                                    field_name = node_text(fc, source).to_string();
                                }
                                "type_identifier" | "scoped_type_identifier"
                                | "generic_type" => {
                                    if field_type.is_empty() {
                                        field_type = node_text(fc, source).to_string();
                                    }
                                }
                                _ => {}
                            }
                        }
                        if !field_name.is_empty() && !field_type.is_empty() {
                            field_types.push(field_type);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if name.is_empty() {
        return;
    }

    let struct_id = format!("{}::{}", module_id, name);
    let docstring = get_doc_comment(node, source);

    graph.add_node(Node {
        id: struct_id.clone(),
        name: name.clone(),
        kind: NodeKind::Class,
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring,
    });
    graph.add_edge(module_id, &struct_id, EdgeKind::Contains);

    for ft in field_types {
        ctx.pending_uses_type.push((struct_id.clone(), ft));
    }
}

fn parse_enum(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
) {
    let line = node.start_position().row + 1;
    let mut name = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" && name.is_empty() {
            name = node_text(child, source).to_string();
        }
    }

    if name.is_empty() {
        return;
    }

    let enum_id = format!("{}::{}", module_id, name);
    let docstring = get_doc_comment(node, source);

    graph.add_node(Node {
        id: enum_id.clone(),
        name: name.clone(),
        kind: NodeKind::Class,
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring,
    });
    graph.add_edge(module_id, &enum_id, EdgeKind::Contains);
    let _ = ctx;
}

fn parse_trait(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
) {
    let line = node.start_position().row + 1;
    let mut name = String::new();
    let mut body_node: Option<TsNode> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" if name.is_empty() => {
                name = node_text(child, source).to_string();
            }
            "declaration_list" => body_node = Some(child),
            _ => {}
        }
    }

    if name.is_empty() {
        return;
    }

    let trait_id = format!("{}::{}", module_id, name);
    let docstring = get_doc_comment(node, source);

    graph.add_node(Node {
        id: trait_id.clone(),
        name: name.clone(),
        kind: NodeKind::Class,
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring,
    });
    graph.add_edge(module_id, &trait_id, EdgeKind::Contains);

    if let Some(body) = body_node {
        process_items(source, body, module_id, rel_path, graph, ctx, Some(&trait_id));
    }
}

fn parse_impl(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
) {
    // impl [Trait for] Type { ... }
    let mut type_name = String::new();
    let mut trait_name: Option<String> = None;
    let mut body_node: Option<TsNode> = None;
    let mut has_for = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" => {
                if has_for {
                    type_name = node_text(child, source).to_string();
                } else if type_name.is_empty() {
                    // Could be trait name or type name — we handle 'for' keyword to distinguish
                    type_name = node_text(child, source).to_string();
                }
            }
            "scoped_type_identifier" => {
                if has_for {
                    type_name = node_text(child, source).to_string();
                } else if type_name.is_empty() {
                    type_name = node_text(child, source).to_string();
                }
            }
            "for" => {
                // The first type_identifier was the trait
                trait_name = Some(type_name.clone());
                type_name = String::new();
                has_for = true;
            }
            "declaration_list" => body_node = Some(child),
            _ => {}
        }
    }

    if type_name.is_empty() {
        return;
    }

    let struct_id = format!("{}::{}", module_id, type_name);

    if let Some(tname) = &trait_name {
        // impl Trait for Struct -> Inherits edge
        ctx.pending_inherits.push((struct_id.clone(), tname.clone()));
    }

    if let Some(body) = body_node {
        process_items(source, body, module_id, rel_path, graph, ctx, Some(&struct_id));
    }
}

fn parse_function(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
    parent_type: Option<&str>,
) {
    let line = node.start_position().row + 1;
    let mut name = String::new();
    let mut return_type: Option<String> = None;
    let mut body_node: Option<TsNode> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" if name.is_empty() => {
                name = node_text(child, source).to_string();
            }
            "type_identifier" | "scoped_type_identifier" | "generic_type"
                if return_type.is_none() =>
            {
                return_type = Some(node_text(child, source).to_string());
            }
            "block" => body_node = Some(child),
            _ => {}
        }
    }

    if name.is_empty() {
        return;
    }

    let func_id = if let Some(pt) = parent_type {
        format!("{}::{}", pt, name)
    } else {
        format!("{}::{}", module_id, name)
    };

    let kind = if parent_type.is_some() {
        NodeKind::Method
    } else {
        NodeKind::Function
    };

    let docstring = get_doc_comment(node, source);

    graph.add_node(Node {
        id: func_id.clone(),
        name: name.clone(),
        kind,
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring,
    });

    let parent_id = parent_type.unwrap_or(module_id);
    graph.add_edge(parent_id, &func_id, EdgeKind::Contains);

    if let Some(ret) = &return_type {
        ctx.pending_uses_type.push((func_id.clone(), ret.clone()));
    }

    if let Some(body) = body_node {
        parse_block(source, body, &func_id, parent_type, ctx);
    }
}

fn parse_block(
    source: &str,
    node: TsNode,
    caller_id: &str,
    in_type: Option<&str>,
    ctx: &mut ParseContext,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "call_expression" => {
                parse_call_expr(source, child, caller_id, in_type, ctx);
            }
            "method_call_expression" => {
                parse_method_call_expr(source, child, caller_id, in_type, ctx);
            }
            "expression_statement" | "let_declaration" | "return_expression"
            | "if_expression" | "match_expression" | "block" => {
                parse_block(source, child, caller_id, in_type, ctx);
            }
            _ => {}
        }
    }
}

fn is_valid_callee(name: &str) -> bool {
    !name.is_empty() && name.len() < 64 && !name.contains('(') && !name.contains(')')
}

fn last_component(path: &str) -> &str {
    path.split("::").last().unwrap_or(path)
}

fn parse_call_expr(
    source: &str,
    node: TsNode,
    caller_id: &str,
    in_type: Option<&str>,
    ctx: &mut ParseContext,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                let name = node_text(child, source).to_string();
                if is_valid_callee(&name) {
                    ctx.pending_calls.push(PendingCall {
                        caller_id: caller_id.to_string(),
                        callee_name: name,
                        object: None,
                        in_class: in_type.map(|s| s.to_string()),
                    });
                }
                return;
            }
            "scoped_identifier" => {
                // Take only the last :: component — avoids capturing full paths like
                // "std::time::SystemTime::now" as a single long callee name.
                let full = node_text(child, source);
                let name = last_component(full).to_string();
                if is_valid_callee(&name) {
                    ctx.pending_calls.push(PendingCall {
                        caller_id: caller_id.to_string(),
                        callee_name: name,
                        object: None,
                        in_class: in_type.map(|s| s.to_string()),
                    });
                }
                return;
            }
            // field_expression / path_expression: capture entire expression chains as text.
            // Skip them — method_call_expression handles obj.method() patterns correctly.
            _ => {}
        }
    }
}

fn parse_method_call_expr(
    source: &str,
    node: TsNode,
    caller_id: &str,
    in_type: Option<&str>,
    ctx: &mut ParseContext,
) {
    let mut cursor = node.walk();
    let mut receiver: Option<String> = None;
    let mut method: Option<String> = None;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "field_identifier" => {
                method = Some(node_text(child, source).to_string());
            }
            "identifier" if receiver.is_none() => {
                receiver = Some(node_text(child, source).to_string());
            }
            _ => {}
        }
    }

    if let Some(m) = method {
        ctx.pending_calls.push(PendingCall {
            caller_id: caller_id.to_string(),
            callee_name: m,
            object: receiver,
            in_class: in_type.map(|s| s.to_string()),
        });
    }
}

fn parse_use(source: &str, node: TsNode, module_id: &str, ctx: &mut ParseContext) {
    let text = node_text(node, source);
    // Strip "use " and ";"
    let raw_path = text
        .trim_start_matches("pub(crate) ")
        .trim_start_matches("pub ")
        .trim_start_matches("use ")
        .trim_end_matches(';')
        .trim()
        .to_string();

    // crate:: and self:: are internal — strip the prefix so paths match internal node IDs.
    let path = if raw_path.starts_with("crate::") {
        raw_path[7..].to_string()
    } else if raw_path.starts_with("self::") {
        raw_path[6..].to_string()
    } else {
        raw_path
    };

    if path.contains('{') {
        // use foo::{A, B}
        if let Some(prefix_end) = path.find('{') {
            let prefix = path[..prefix_end].trim_end_matches("::").to_string();
            let items = &path[prefix_end + 1..path.find('}').unwrap_or(path.len())];
            for item in items.split(',') {
                let item = item.trim();
                if !item.is_empty() && item != ".." {
                    let full = format!("{}::{}", prefix, item);
                    ctx.imports.insert(item.to_string(), full.clone());
                    ctx.imports.insert(full.clone(), full);
                }
            }
        }
    } else if path.ends_with("::*") {
        // Wildcard import
        let prefix = path.trim_end_matches("::*").to_string();
        ctx.imports.insert(prefix.clone(), prefix);
    } else {
        // Simple path
        let short = path.split("::").last().unwrap_or(&path).to_string();
        ctx.imports.insert(short, path.clone());
        ctx.imports.insert(path.clone(), path);
    }

    let _ = module_id;
}
