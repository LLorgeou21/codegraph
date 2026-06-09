/// Parser for TypeScript (.ts, .tsx) and JavaScript (.js, .jsx) files.
///
/// Handles:
///   - class / interface / abstract class    → NodeKind::Class
///   - type alias                            → NodeKind::Class
///   - enum                                  → NodeKind::Struct
///   - top-level function / arrow constant   → NodeKind::Function
///   - class methods                         → NodeKind::Method
///   - import declarations                   → ctx.imports
///   - call expressions                      → ctx.pending_calls
use crate::{ParseContext, PendingCall};
use anyhow::Result;
use core::{CodeGraph, EdgeKind, Node, NodeKind};
use tree_sitter::{Node as TsNode, Parser};

/// Whether this file is TypeScript (vs plain JavaScript).
#[derive(Clone, Copy, PartialEq)]
pub enum JsVariant {
    TypeScript,
    JavaScript,
}

pub fn parse_file(
    source: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    variant: JsVariant,
) -> Result<ParseContext> {
    let mut parser = Parser::new();

    let lang = if variant == JsVariant::TypeScript {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    } else {
        tree_sitter_javascript::LANGUAGE.into()
    };

    parser
        .set_language(&lang)
        .map_err(|e| anyhow::anyhow!("TS/JS language error: {}", e))?;

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
    process_statements(source, root, &module_id, rel_path, graph, &mut ctx, None);

    Ok(ctx)
}

fn path_to_module_id(rel_path: &str) -> String {
    rel_path
        .trim_end_matches(".tsx")
        .trim_end_matches(".ts")
        .trim_end_matches(".jsx")
        .trim_end_matches(".js")
        .replace(['\\'], "/")
        .trim_start_matches('/')
        .to_string()
}

fn node_text<'a>(node: TsNode<'a>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

// ── Top-level traversal ────────────────────────────────────────────────────────

fn process_statements(
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
            // --- classes & interfaces ---
            "class_declaration" | "abstract_class_declaration" => {
                parse_class(source, child, module_id, rel_path, graph, ctx);
            }
            "interface_declaration" => {
                parse_interface(source, child, module_id, rel_path, graph, ctx);
            }
            // --- type aliases ---
            "type_alias_declaration" => {
                parse_type_alias(source, child, module_id, rel_path, graph);
            }
            // --- enums ---
            "enum_declaration" => {
                parse_enum(source, child, module_id, rel_path, graph);
            }
            // --- free functions ---
            "function_declaration" | "generator_function_declaration" => {
                parse_function(source, child, module_id, rel_path, graph, ctx, parent_class);
            }
            // --- export { function () {} } ---
            "export_statement" => {
                process_statements(source, child, module_id, rel_path, graph, ctx, parent_class);
            }
            // --- const foo = () => {} ---
            "lexical_declaration" | "variable_declaration" => {
                parse_variable_decl(source, child, module_id, rel_path, graph, ctx);
            }
            // --- imports ---
            "import_statement" => {
                parse_import(source, child, module_id, ctx);
            }
            // --- expression statements (call sites at module level) ---
            "expression_statement" => {
                let mut c2 = child.walk();
                for c in child.children(&mut c2) {
                    if c.kind() == "call_expression" || c.kind() == "await_expression" {
                        collect_calls(source, c, module_id, None, ctx);
                    }
                }
            }
            _ => {}
        }
    }
}

// ── Class ─────────────────────────────────────────────────────────────────────

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
    let mut bases: Vec<String> = Vec::new();
    let mut body: Option<TsNode> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" | "identifier" if class_name.is_empty() => {
                class_name = node_text(child, source).to_string();
            }
            "class_heritage" => {
                let mut c2 = child.walk();
                for c in child.children(&mut c2) {
                    if c.kind() == "extends_clause" {
                        let mut c3 = c.walk();
                        for cc in c.children(&mut c3) {
                            if cc.kind() == "identifier" || cc.kind() == "type_identifier" {
                                bases.push(node_text(cc, source).to_string());
                            }
                        }
                    }
                }
            }
            "class_body" => body = Some(child),
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

    for base in &bases {
        ctx.pending_inherits.push((class_id.clone(), base.clone()));
    }

    if let Some(body_node) = body {
        parse_class_body(source, body_node, module_id, rel_path, graph, ctx, &class_id);
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
            "method_definition" | "public_field_definition" | "abstract_method_signature" => {
                parse_method(source, child, module_id, rel_path, graph, ctx, class_id);
            }
            _ => {}
        }
    }
}

fn parse_method(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
    class_id: &str,
) {
    let line = node.start_position().row + 1;
    let mut method_name = String::new();
    let mut body: Option<TsNode> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "property_identifier" | "identifier" if method_name.is_empty() => {
                method_name = node_text(child, source).to_string();
            }
            "statement_block" => body = Some(child),
            _ => {}
        }
    }

    if method_name.is_empty() || method_name == "constructor" {
        // still parse constructor body for calls
        if let Some(b) = body {
            collect_calls_block(source, b, class_id, Some(class_id), ctx);
        }
        return;
    }

    let method_id = format!("{}::{}", class_id, method_name);

    graph.add_node(Node {
        id: method_id.clone(),
        name: method_name.clone(),
        kind: NodeKind::Method,
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring: None,
    });
    graph.add_edge(class_id, &method_id, EdgeKind::Contains);

    let _ = module_id;
    if let Some(b) = body {
        collect_calls_block(source, b, &method_id, Some(class_id), ctx);
    }
}

// ── Interface ─────────────────────────────────────────────────────────────────

fn parse_interface(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
) {
    let line = node.start_position().row + 1;
    let mut name = String::new();
    let mut extends: Vec<String> = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" | "identifier" if name.is_empty() => {
                name = node_text(child, source).to_string();
            }
            "extends_type_clause" => {
                let mut c2 = child.walk();
                for c in child.children(&mut c2) {
                    if c.kind() == "type_identifier" || c.kind() == "identifier" {
                        extends.push(node_text(c, source).to_string());
                    }
                }
            }
            _ => {}
        }
    }

    if name.is_empty() {
        return;
    }

    let iface_id = format!("{}/{}", module_id, name);

    graph.add_node(Node {
        id: iface_id.clone(),
        name: name.clone(),
        kind: NodeKind::Class,   // interfaces are Class-kind
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring: None,
    });
    graph.add_edge(module_id, &iface_id, EdgeKind::Contains);

    for base in &extends {
        ctx.pending_inherits.push((iface_id.clone(), base.clone()));
    }
}

// ── Type alias ────────────────────────────────────────────────────────────────

fn parse_type_alias(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
) {
    let line = node.start_position().row + 1;
    let mut name = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if (child.kind() == "type_identifier" || child.kind() == "identifier") && name.is_empty() {
            name = node_text(child, source).to_string();
        }
    }

    if name.is_empty() {
        return;
    }

    let alias_id = format!("{}/{}", module_id, name);
    graph.add_node(Node {
        id: alias_id.clone(),
        name: name.clone(),
        kind: NodeKind::Class,
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring: None,
    });
    graph.add_edge(module_id, &alias_id, EdgeKind::Contains);
}

// ── Enum ──────────────────────────────────────────────────────────────────────

fn parse_enum(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
) {
    let line = node.start_position().row + 1;
    let mut name = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if (child.kind() == "identifier" || child.kind() == "type_identifier") && name.is_empty() {
            name = node_text(child, source).to_string();
        }
    }

    if name.is_empty() {
        return;
    }

    let enum_id = format!("{}/{}", module_id, name);
    graph.add_node(Node {
        id: enum_id.clone(),
        name: name.clone(),
        kind: NodeKind::Struct,
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring: None,
    });
    graph.add_edge(module_id, &enum_id, EdgeKind::Contains);
}

// ── Free functions ────────────────────────────────────────────────────────────

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
    let mut name = String::new();
    let mut body: Option<TsNode> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" if name.is_empty() => {
                name = node_text(child, source).to_string();
            }
            "statement_block" => body = Some(child),
            _ => {}
        }
    }

    if name.is_empty() {
        return;
    }

    let func_id = if let Some(cls) = parent_class {
        format!("{}::{}", cls, name)
    } else {
        format!("{}/{}", module_id, name)
    };

    graph.add_node(Node {
        id: func_id.clone(),
        name: name.clone(),
        kind: NodeKind::Function,
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring: None,
    });

    let parent_id = parent_class.unwrap_or(module_id);
    graph.add_edge(parent_id, &func_id, EdgeKind::Contains);

    if let Some(b) = body {
        collect_calls_block(source, b, &func_id, parent_class, ctx);
    }
}

/// `const foo = () => { ... }` or `const foo = function() { ... }`
fn parse_variable_decl(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            let mut name = String::new();
            let mut body: Option<TsNode> = None;
            let mut is_func = false;
            let line = child.start_position().row + 1;

            let mut c2 = child.walk();
            for c in child.children(&mut c2) {
                match c.kind() {
                    "identifier" if name.is_empty() => {
                        name = node_text(c, source).to_string();
                    }
                    "arrow_function" | "function" => {
                        is_func = true;
                        // look for statement_block inside
                        let mut c3 = c.walk();
                        for cc in c.children(&mut c3) {
                            if cc.kind() == "statement_block" {
                                body = Some(cc);
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }

            if !is_func || name.is_empty() {
                continue;
            }

            let func_id = format!("{}/{}", module_id, name);
            graph.add_node(Node {
                id: func_id.clone(),
                name: name.clone(),
                kind: NodeKind::Function,
                file: rel_path.to_string(),
                line,
                is_external: false,
                docstring: None,
            });
            graph.add_edge(module_id, &func_id, EdgeKind::Contains);

            if let Some(b) = body {
                collect_calls_block(source, b, &func_id, None, ctx);
            }
        }
    }
}

// ── Imports ───────────────────────────────────────────────────────────────────

fn parse_import(source: &str, node: TsNode, module_id: &str, ctx: &mut ParseContext) {
    // import { A, B } from 'mod'
    // import DefaultExport from 'mod'
    // import * as NS from 'mod'
    let mut source_mod = String::new();
    let mut names: Vec<(String, String)> = Vec::new(); // (local, original)

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string" => {
                source_mod = node_text(child, source)
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
            }
            "import_clause" => {
                let mut c2 = child.walk();
                for c in child.children(&mut c2) {
                    match c.kind() {
                        // import Default from ...
                        "identifier" => {
                            let n = node_text(c, source).to_string();
                            names.push((n.clone(), n));
                        }
                        // import { A, B as C }
                        "named_imports" => {
                            let mut c3 = c.walk();
                            for cc in c.children(&mut c3) {
                                if cc.kind() == "import_specifier" {
                                    let mut orig = String::new();
                                    let mut alias = String::new();
                                    let mut c4 = cc.walk();
                                    for item in cc.children(&mut c4) {
                                        if item.kind() == "identifier" {
                                            if orig.is_empty() {
                                                orig = node_text(item, source).to_string();
                                            } else {
                                                alias = node_text(item, source).to_string();
                                            }
                                        }
                                    }
                                    if !orig.is_empty() {
                                        let local = if alias.is_empty() { orig.clone() } else { alias };
                                        names.push((local, orig));
                                    }
                                }
                            }
                        }
                        // import * as NS
                        "namespace_import" => {
                            let mut c3 = c.walk();
                            for cc in c.children(&mut c3) {
                                if cc.kind() == "identifier" {
                                    let n = node_text(cc, source).to_string();
                                    names.push((n.clone(), "*".to_string()));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    if source_mod.is_empty() {
        return;
    }

    // Resolve relative paths
    let resolved_mod = if source_mod.starts_with('.') {
        let parent = module_id.rsplit('/').skip(1).collect::<Vec<_>>().iter().rev().cloned().collect::<Vec<_>>().join("/");
        if parent.is_empty() {
            source_mod.trim_start_matches("./").to_string()
        } else {
            format!("{}/{}", parent, source_mod.trim_start_matches("./"))
        }
    } else {
        source_mod.clone()
    };

    if names.is_empty() {
        // side-effect import: import 'mod'
        ctx.imports.insert(resolved_mod.clone(), resolved_mod);
    } else {
        for (local, orig) in names {
            let full = if orig == "*" {
                resolved_mod.clone()
            } else {
                format!("{}/{}", resolved_mod, orig)
            };
            ctx.imports.insert(local, full);
        }
    }

    let _ = module_id;
}

// ── Call collection ───────────────────────────────────────────────────────────

fn collect_calls_block(
    source: &str,
    node: TsNode,
    caller_id: &str,
    in_class: Option<&str>,
    ctx: &mut ParseContext,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "call_expression" | "await_expression" => {
                collect_calls(source, child, caller_id, in_class, ctx);
            }
            "expression_statement" | "return_statement" | "if_statement"
            | "for_statement" | "while_statement" | "try_statement"
            | "catch_clause" | "block" | "statement_block"
            | "arrow_function" | "lexical_declaration" | "variable_declarator" => {
                collect_calls_block(source, child, caller_id, in_class, ctx);
            }
            _ => {}
        }
    }
}

fn collect_calls(
    source: &str,
    node: TsNode,
    caller_id: &str,
    in_class: Option<&str>,
    ctx: &mut ParseContext,
) {
    // Recurse into await_expression first
    if node.kind() == "await_expression" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "call_expression" {
                collect_calls(source, child, caller_id, in_class, ctx);
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            // foo()  or  foo.bar()
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
            "member_expression" => {
                let mut c2 = child.walk();
                let mut obj = String::new();
                let mut prop = String::new();
                for c in child.children(&mut c2) {
                    match c.kind() {
                        "identifier" if obj.is_empty() => obj = node_text(c, source).to_string(),
                        "property_identifier" => prop = node_text(c, source).to_string(),
                        _ => {}
                    }
                }
                if !prop.is_empty() {
                    ctx.pending_calls.push(PendingCall {
                        caller_id: caller_id.to_string(),
                        callee_name: prop,
                        object: if obj.is_empty() { None } else { Some(obj) },
                        in_class: in_class.map(|s| s.to_string()),
                    });
                }
                return;
            }
            _ => {}
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use core::CodeGraph;

    fn parse_ts(source: &str) -> (CodeGraph, ParseContext) {
        let mut g = CodeGraph::new();
        let ctx = parse_file(source, "test.ts", &mut g, JsVariant::TypeScript).unwrap();
        (g, ctx)
    }

    fn parse_js(source: &str) -> (CodeGraph, ParseContext) {
        let mut g = CodeGraph::new();
        let ctx = parse_file(source, "test.js", &mut g, JsVariant::JavaScript).unwrap();
        (g, ctx)
    }

    #[test]
    fn test_ts_class() {
        let src = "class Animal { speak() {} }";
        let (g, _ctx) = parse_ts(src);
        assert!(g.has_node("test/Animal"), "Class node should exist");
        let n = g.get_node("test/Animal").unwrap();
        assert_eq!(n.kind, NodeKind::Class);
    }

    #[test]
    fn test_ts_class_with_method() {
        let src = "class Dog extends Animal { bark() { console.log('woof'); } }";
        let (g, ctx) = parse_ts(src);
        assert!(g.has_node("test/Dog"));
        assert!(g.has_node("test/Dog::bark"));
        assert!(ctx.pending_inherits.iter().any(|(_, b)| b == "Animal"),
            "Should have pending inherit from Animal");
    }

    #[test]
    fn test_ts_interface() {
        let src = "interface Shape { area(): number; }";
        let (g, _) = parse_ts(src);
        assert!(g.has_node("test/Shape"));
        let n = g.get_node("test/Shape").unwrap();
        assert_eq!(n.kind, NodeKind::Class);
    }

    #[test]
    fn test_ts_enum() {
        let src = "enum Direction { North, South, East, West }";
        let (g, _) = parse_ts(src);
        assert!(g.has_node("test/Direction"));
        let n = g.get_node("test/Direction").unwrap();
        assert_eq!(n.kind, NodeKind::Struct);
    }

    #[test]
    fn test_ts_function() {
        let src = "function greet(name: string): void { console.log(name); }";
        let (g, _) = parse_ts(src);
        assert!(g.has_node("test/greet"));
        let n = g.get_node("test/greet").unwrap();
        assert_eq!(n.kind, NodeKind::Function);
    }

    #[test]
    fn test_ts_arrow_const() {
        let src = "const add = (a: number, b: number) => { return a + b; };";
        let (g, _) = parse_ts(src);
        assert!(g.has_node("test/add"));
    }

    #[test]
    fn test_js_function() {
        let src = "function hello() { return 42; }";
        let (g, _) = parse_js(src);
        assert!(g.has_node("test/hello"));
    }

    #[test]
    fn test_ts_import_named() {
        let src = "import { useState, useEffect } from 'react';";
        let (_g, ctx) = parse_ts(src);
        assert!(ctx.imports.contains_key("useState"));
        assert!(ctx.imports.contains_key("useEffect"));
    }

    #[test]
    fn test_ts_import_default() {
        let src = "import React from 'react';";
        let (_g, ctx) = parse_ts(src);
        assert!(ctx.imports.contains_key("React"));
    }

    #[test]
    fn test_ts_type_alias() {
        let src = "type Point = { x: number; y: number; };";
        let (g, _) = parse_ts(src);
        assert!(g.has_node("test/Point"));
    }
}
