use crate::{ParseContext, PendingCall};
use anyhow::Result;
use core::{CodeGraph, EdgeKind, Node, NodeKind};
use tree_sitter::{Node as TsNode, Parser};

pub fn parse_file(source: &str, rel_path: &str, graph: &mut CodeGraph) -> Result<ParseContext> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_cpp::LANGUAGE.into())
        .map_err(|e| anyhow::anyhow!("C++ language error: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", rel_path))?;

    let module_id = path_to_module_id(rel_path);

    graph.add_node(Node {
        id: module_id.clone(),
        name: module_id.split('/').last().unwrap_or(&module_id).to_string(),
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
    process_translation_unit(source, root, &module_id, rel_path, graph, &mut ctx, None);

    Ok(ctx)
}

fn path_to_module_id(rel_path: &str) -> String {
    rel_path.replace(['\\'], "/").to_string()
}

fn node_text<'a>(node: TsNode<'a>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn process_translation_unit(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
    parent_class: Option<&str>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "class_specifier" => {
                parse_class(source, child, module_id, rel_path, graph, ctx);
            }
            "struct_specifier" => {
                parse_struct(source, child, module_id, rel_path, graph, ctx);
            }
            "function_definition" => {
                parse_function(source, child, module_id, rel_path, graph, ctx, parent_class);
            }
            "declaration" => {
                // Function declarations (prototypes)
                parse_declaration(source, child, module_id, rel_path, graph, ctx, parent_class);
            }
            "preproc_include" => {
                parse_include(source, child, module_id, ctx);
            }
            "namespace_definition" => {
                let mut c2 = child.walk();
                for c in child.children(&mut c2) {
                    if c.kind() == "declaration_list" {
                        process_translation_unit(
                            source, c, module_id, rel_path, graph, ctx, parent_class,
                        );
                    }
                }
            }
            "template_declaration" => {
                // Process inner declaration
                let mut c2 = child.walk();
                for c in child.children(&mut c2) {
                    match c.kind() {
                        "class_specifier" => {
                            parse_class(source, c, module_id, rel_path, graph, ctx);
                        }
                        "function_definition" => {
                            parse_function(
                                source, c, module_id, rel_path, graph, ctx, parent_class,
                            );
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn parse_class(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
) {
    let line = node.start_position().row + 1;
    let mut class_name = String::new();
    let mut base_classes: Vec<String> = Vec::new();
    let mut body_node: Option<TsNode> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" if class_name.is_empty() => {
                class_name = node_text(child, source).to_string();
            }
            "base_class_clause" => {
                let mut c2 = child.walk();
                for c in child.children(&mut c2) {
                    if c.kind() == "type_identifier" {
                        base_classes.push(node_text(c, source).to_string());
                    }
                }
            }
            "field_declaration_list" => body_node = Some(child),
            _ => {}
        }
    }

    if class_name.is_empty() {
        return;
    }

    let class_id = format!("{}/{}", module_id, class_name);

    graph.add_node(Node {
        id: class_id.clone(),
        name: class_name.clone(),
        kind: NodeKind::Class,
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring: None,
    });
    graph.add_edge(module_id, &class_id, EdgeKind::Contains);

    for base in &base_classes {
        ctx.pending_inherits.push((class_id.clone(), base.clone()));
    }

    if let Some(body) = body_node {
        parse_class_body(source, body, module_id, rel_path, graph, ctx, &class_id);
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
    let mut struct_name = String::new();
    let mut body_node: Option<TsNode> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" if struct_name.is_empty() => {
                struct_name = node_text(child, source).to_string();
            }
            "field_declaration_list" => body_node = Some(child),
            _ => {}
        }
    }

    if struct_name.is_empty() {
        return;
    }

    let struct_id = format!("{}/{}", module_id, struct_name);

    graph.add_node(Node {
        id: struct_id.clone(),
        name: struct_name.clone(),
        kind: NodeKind::Struct,
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring: None,
    });
    graph.add_edge(module_id, &struct_id, EdgeKind::Contains);

    if let Some(body) = body_node {
        parse_class_body(source, body, module_id, rel_path, graph, ctx, &struct_id);
    }
}

fn parse_class_body(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
    class_id: &str,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                parse_function(source, child, module_id, rel_path, graph, ctx, Some(class_id));
            }
            "declaration" => {
                parse_declaration(
                    source,
                    child,
                    module_id,
                    rel_path,
                    graph,
                    ctx,
                    Some(class_id),
                );
            }
            "access_specifier" => {} // public:, private:, etc.
            _ => {}
        }
    }
}

fn parse_function(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
    parent_class: Option<&str>,
) {
    let line = node.start_position().row + 1;
    let mut func_name = String::new();
    let mut body_node: Option<TsNode> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declarator" => {
                let mut c2 = child.walk();
                for c in child.children(&mut c2) {
                    match c.kind() {
                        "identifier" | "qualified_identifier" | "destructor_name" => {
                            if func_name.is_empty() {
                                func_name = node_text(c, source)
                                    .split("::")
                                    .last()
                                    .unwrap_or("")
                                    .to_string();
                            }
                        }
                        _ => {}
                    }
                }
            }
            "compound_statement" => body_node = Some(child),
            _ => {}
        }
    }

    if func_name.is_empty() {
        return;
    }

    let func_id = if let Some(cid) = parent_class {
        format!("{}::{}", cid, func_name)
    } else {
        format!("{}/{}", module_id, func_name)
    };

    let kind = if parent_class.is_some() {
        NodeKind::Method
    } else {
        NodeKind::Function
    };

    graph.add_node(Node {
        id: func_id.clone(),
        name: func_name.clone(),
        kind,
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring: None,
    });

    let parent_id = parent_class.unwrap_or(module_id);
    graph.add_edge(parent_id, &func_id, EdgeKind::Contains);

    if let Some(body) = body_node {
        parse_compound_statement(source, body, &func_id, parent_class, ctx);
    }
}

fn parse_declaration(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    _ctx: &mut ParseContext,
    parent_class: Option<&str>,
) {
    let line = node.start_position().row + 1;
    let mut func_name = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_declarator" {
            let mut c2 = child.walk();
            for c in child.children(&mut c2) {
                if c.kind() == "identifier" && func_name.is_empty() {
                    func_name = node_text(c, source).to_string();
                }
            }
        }
    }

    if func_name.is_empty() {
        return;
    }

    let func_id = if let Some(cid) = parent_class {
        format!("{}::{}", cid, func_name)
    } else {
        format!("{}/{}", module_id, func_name)
    };

    let kind = if parent_class.is_some() {
        NodeKind::Method
    } else {
        NodeKind::Function
    };

    if !graph.has_node(&func_id) {
        graph.add_node(Node {
            id: func_id.clone(),
            name: func_name.clone(),
            kind,
            file: rel_path.to_string(),
            line,
            is_external: false,
            docstring: None,
        });
        let parent_id = parent_class.unwrap_or(module_id);
        graph.add_edge(parent_id, &func_id, EdgeKind::Contains);
    }
}

fn parse_compound_statement(
    source: &str,
    node: TsNode,
    caller_id: &str,
    in_class: Option<&str>,
    ctx: &mut ParseContext,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "expression_statement" => {
                let mut c2 = child.walk();
                for c in child.children(&mut c2) {
                    if c.kind() == "call_expression" {
                        parse_call_expression(source, c, caller_id, in_class, ctx);
                    }
                }
            }
            "call_expression" => {
                parse_call_expression(source, child, caller_id, in_class, ctx);
            }
            "if_statement" | "for_statement" | "while_statement" | "compound_statement"
            | "return_statement" => {
                parse_compound_statement(source, child, caller_id, in_class, ctx);
            }
            _ => {}
        }
    }
}

fn parse_call_expression(
    source: &str,
    node: TsNode,
    caller_id: &str,
    in_class: Option<&str>,
    ctx: &mut ParseContext,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                let name = node_text(child, source).to_string();
                if !name.is_empty() {
                    ctx.pending_calls.push(PendingCall {
                        caller_id: caller_id.to_string(),
                        callee_name: name,
                        object: None,
                        in_class: in_class.map(|s| s.to_string()),
                    });
                }
                return;
            }
            "field_expression" => {
                let mut c2 = child.walk();
                let mut obj = String::new();
                let mut method = String::new();
                for c in child.children(&mut c2) {
                    match c.kind() {
                        "identifier" if obj.is_empty() => obj = node_text(c, source).to_string(),
                        "field_identifier" => method = node_text(c, source).to_string(),
                        _ => {}
                    }
                }
                if !method.is_empty() {
                    ctx.pending_calls.push(PendingCall {
                        caller_id: caller_id.to_string(),
                        callee_name: method,
                        object: Some(obj),
                        in_class: in_class.map(|s| s.to_string()),
                    });
                }
                return;
            }
            "scoped_identifier" | "qualified_identifier" => {
                let name = node_text(child, source).to_string();
                ctx.pending_calls.push(PendingCall {
                    caller_id: caller_id.to_string(),
                    callee_name: name,
                    object: None,
                    in_class: in_class.map(|s| s.to_string()),
                });
                return;
            }
            _ => {}
        }
    }
}

fn parse_include(source: &str, node: TsNode, module_id: &str, ctx: &mut ParseContext) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "system_lib_string" => {
                // <header> -> external
                let header = node_text(child, source)
                    .trim_start_matches('<')
                    .trim_end_matches('>')
                    .to_string();
                ctx.imports.insert(header.clone(), format!("__ext__.{}", header));
            }
            "string_literal" => {
                // "header" -> internal
                let header = node_text(child, source)
                    .trim_matches('"')
                    .to_string();
                let module_path = format!("{}/{}", module_id.rsplit('/').skip(1).collect::<Vec<_>>().iter().rev().cloned().collect::<Vec<_>>().join("/"), header);
                ctx.imports.insert(header.clone(), module_path);
            }
            _ => {}
        }
    }
}
