use crate::{ParseContext, PendingCall};
use anyhow::Result;
use core::{CodeGraph, EdgeKind, Node, NodeKind};
use tree_sitter::{Node as TsNode, Parser};

pub fn parse_file(source: &str, rel_path: &str, graph: &mut CodeGraph) -> Result<ParseContext> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|e| anyhow::anyhow!("Python language error: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", rel_path))?;

    let module_id = path_to_module_id(rel_path);

    // Add module node
    graph.add_node(Node {
        id: module_id.clone(),
        name: module_id.split('.').last().unwrap_or(&module_id).to_string(),
        kind: NodeKind::Module,
        file: rel_path.to_string(),
        line: 1,
        is_external: false,
        docstring: extract_module_docstring(source, tree.root_node()),
    });

    let mut ctx = ParseContext {
        module_id: module_id.clone(),
        file: rel_path.to_string(),
        ..Default::default()
    };

    let root = tree.root_node();
    process_module(source, root, &module_id, rel_path, graph, &mut ctx, None, None);

    Ok(ctx)
}

fn path_to_module_id(rel_path: &str) -> String {
    rel_path
        .trim_end_matches(".py")
        .replace(['/', '\\'], ".")
        .trim_start_matches('.')
        .to_string()
}

fn node_text<'a>(node: TsNode<'a>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn extract_module_docstring(source: &str, root: TsNode) -> Option<String> {
    let first = root.child(0)?;
    if first.kind() == "expression_statement" {
        let child = first.child(0)?;
        if child.kind() == "string" {
            let text = node_text(child, source);
            return Some(clean_docstring(text));
        }
    }
    None
}

fn clean_docstring(s: &str) -> String {
    s.trim_matches('"')
        .trim_matches('\'')
        .trim_start_matches("\"\"\"")
        .trim_end_matches("\"\"\"")
        .trim_start_matches("'''")
        .trim_end_matches("'''")
        .trim()
        .to_string()
}

fn get_decorators(node: TsNode, source: &str) -> Vec<String> {
    let mut decorators = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "decorator" {
            let text = node_text(child, source).to_string();
            decorators.push(text.trim_start_matches('@').to_string());
        }
    }
    decorators
}

fn get_first_string_child(node: TsNode, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "expression_statement" {
            let mut c2 = child.walk();
            for c in child.children(&mut c2) {
                if c.kind() == "string" {
                    return Some(clean_docstring(node_text(c, source)));
                }
            }
        }
    }
    None
}

fn process_module(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
    parent_class: Option<&str>,
    parent_func: Option<&str>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "import_statement" => parse_import(source, child, ctx),
            "import_from_statement" => parse_import_from(source, child, ctx, module_id),
            "class_definition" => {
                parse_class(source, child, module_id, rel_path, graph, ctx);
            }
            "function_definition" => {
                parse_function(
                    source,
                    child,
                    module_id,
                    rel_path,
                    graph,
                    ctx,
                    parent_class,
                    parent_func,
                );
            }
            "expression_statement" => {
                // Check for module-level constants
                if parent_class.is_none() && parent_func.is_none() {
                    parse_constant(source, child, module_id, rel_path, graph);
                }
            }
            _ => {}
        }
    }
}

fn parse_import(source: &str, node: TsNode, ctx: &mut ParseContext) {
    // import X, import X as Y
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "dotted_name" => {
                let name = node_text(child, source).to_string();
                ctx.imports.insert(name.clone(), name);
            }
            "aliased_import" => {
                let mut c2 = child.walk();
                let mut orig = String::new();
                let mut alias = String::new();
                for c in child.children(&mut c2) {
                    match c.kind() {
                        "dotted_name" if orig.is_empty() => orig = node_text(c, source).to_string(),
                        "identifier" => alias = node_text(c, source).to_string(),
                        _ => {}
                    }
                }
                if !alias.is_empty() {
                    ctx.alias_map.insert(alias.clone(), orig.clone());
                    ctx.imports.insert(alias, orig);
                }
            }
            _ => {}
        }
    }
}

fn parse_import_from(source: &str, node: TsNode, ctx: &mut ParseContext, module_id: &str) {
    // from X import Y, Z
    // from . import X
    let mut cursor = node.walk();
    let mut module_name = String::new();
    let mut dots = 0usize;
    let mut names: Vec<(String, Option<String>)> = Vec::new();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "relative_import" => {
                // Count dots
            }
            "." => dots += 1,
            "dotted_name" if module_name.is_empty() => {
                module_name = node_text(child, source).to_string();
            }
            "import_from_as_names" | "import_prefix" => {
                let mut c2 = child.walk();
                for c in child.children(&mut c2) {
                    if c.kind() == "dotted_name" {
                        module_name = node_text(c, source).to_string();
                    }
                }
            }
            "wildcard_import" => {}
            "aliased_import" => {
                let mut c2 = child.walk();
                let mut orig = String::new();
                let mut alias = String::new();
                for c in child.children(&mut c2) {
                    match c.kind() {
                        "identifier" if orig.is_empty() => orig = node_text(c, source).to_string(),
                        "identifier" => alias = node_text(c, source).to_string(),
                        _ => {}
                    }
                }
                if !orig.is_empty() {
                    names.push((orig, if alias.is_empty() { None } else { Some(alias) }));
                }
            }
            "identifier" => {
                names.push((node_text(child, source).to_string(), None));
            }
            _ => {}
        }
    }

    // Resolve relative import
    let full_module = if dots > 0 {
        let parts: Vec<&str> = module_id.split('.').collect();
        let keep = parts.len().saturating_sub(dots);
        let base = parts[..keep].join(".");
        if module_name.is_empty() {
            base
        } else {
            format!("{}.{}", base, module_name)
        }
    } else {
        module_name.clone()
    };

    if names.is_empty() {
        // from . import => whole module
        ctx.imports.insert(full_module.clone(), full_module);
    } else {
        for (name, alias) in names {
            let full_path = format!("{}.{}", full_module, name);
            let key = alias.unwrap_or_else(|| name.clone());
            ctx.imports.insert(key, full_path);
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
    let decorators = get_decorators(node, source);
    let is_dataclass = decorators.iter().any(|d| d == "dataclass");

    // Get class name
    let mut class_name = String::new();
    let mut bases: Vec<String> = Vec::new();
    let mut body_node: Option<TsNode> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" if class_name.is_empty() => {
                class_name = node_text(child, source).to_string();
            }
            "argument_list" => {
                let mut c2 = child.walk();
                for c in child.children(&mut c2) {
                    if c.kind() == "identifier" || c.kind() == "attribute" {
                        bases.push(node_text(c, source).to_string());
                    }
                }
            }
            "block" => body_node = Some(child),
            _ => {}
        }
    }

    if class_name.is_empty() {
        return;
    }

    let class_id = format!("{}.{}", module_id, class_name);
    let docstring = body_node.and_then(|b| get_first_string_child(b, source));

    graph.add_node(Node {
        id: class_id.clone(),
        name: class_name.clone(),
        kind: NodeKind::Class,
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring,
    });
    graph.add_edge(module_id, &class_id, EdgeKind::Contains);

    // Inheritance
    for base in &bases {
        ctx.pending_inherits.push((class_id.clone(), base.clone()));
    }

    // Parse body
    if let Some(body) = body_node {
        let mut cursor2 = body.walk();
        for child in body.children(&mut cursor2) {
            match child.kind() {
                "function_definition" => {
                    parse_function(
                        source,
                        child,
                        module_id,
                        rel_path,
                        graph,
                        ctx,
                        Some(&class_id),
                        None,
                    );
                }
                "expression_statement" if is_dataclass => {
                    // Dataclass fields
                    parse_dataclass_field(source, child, &class_id, ctx);
                }
                "annotated_assignment" if is_dataclass => {
                    parse_dataclass_annotated_field(source, child, &class_id, ctx);
                }
                _ => {}
            }
        }
    }
}

fn parse_dataclass_field(source: &str, node: TsNode, class_id: &str, ctx: &mut ParseContext) {
    // e.g.  name: Type = default
    let text = node_text(node, source);
    if text.contains(':') {
        let parts: Vec<&str> = text.splitn(2, ':').collect();
        if parts.len() == 2 {
            let field_name = parts[0].trim().to_string();
            let type_part = parts[1].split('=').next().unwrap_or("").trim().to_string();
            if !type_part.is_empty() {
                ctx.class_fields
                    .entry(class_id.to_string())
                    .or_default()
                    .insert(field_name, type_part.clone());
                ctx.pending_uses_type
                    .push((class_id.to_string(), type_part));
            }
        }
    }
}

fn parse_dataclass_annotated_field(
    source: &str,
    node: TsNode,
    class_id: &str,
    ctx: &mut ParseContext,
) {
    let mut cursor = node.walk();
    let mut field_name = String::new();
    let mut type_name = String::new();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" if field_name.is_empty() => {
                field_name = node_text(child, source).to_string();
            }
            "type" | "identifier" | "attribute" if type_name.is_empty() && !field_name.is_empty() => {
                type_name = node_text(child, source).to_string();
            }
            _ => {}
        }
    }
    if !field_name.is_empty() && !type_name.is_empty() {
        ctx.class_fields
            .entry(class_id.to_string())
            .or_default()
            .insert(field_name, type_name.clone());
        ctx.pending_uses_type
            .push((class_id.to_string(), type_name));
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
    parent_func: Option<&str>,
) {
    let line = node.start_position().row + 1;
    let decorators = get_decorators(node, source);
    let is_property = decorators.iter().any(|d| d == "property" || d.ends_with(".getter") || d.ends_with(".setter"));

    let mut func_name = String::new();
    let mut return_type: Option<String> = None;
    let mut params_node: Option<TsNode> = None;
    let mut body_node: Option<TsNode> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" if func_name.is_empty() => {
                func_name = node_text(child, source).to_string();
            }
            "parameters" => params_node = Some(child),
            "type" => return_type = Some(node_text(child, source).to_string()),
            "block" => body_node = Some(child),
            _ => {}
        }
    }

    if func_name.is_empty() {
        return;
    }

    let func_id = if let Some(class_id) = parent_class {
        format!("{}.{}", class_id, func_name)
    } else if let Some(pf_id) = parent_func {
        format!("{}.{}", pf_id, func_name)
    } else {
        format!("{}.{}", module_id, func_name)
    };

    let kind = if is_property {
        NodeKind::Property
    } else if parent_class.is_some() {
        NodeKind::Method
    } else {
        NodeKind::Function
    };

    let docstring = body_node.and_then(|b| get_first_string_child(b, source));

    graph.add_node(Node {
        id: func_id.clone(),
        name: func_name.clone(),
        kind,
        file: rel_path.to_string(),
        line,
        is_external: false,
        docstring,
    });

    let parent_id = parent_class.or(parent_func).unwrap_or(module_id);
    graph.add_edge(parent_id, &func_id, EdgeKind::Contains);

    // Return type annotation
    if let Some(ret) = &return_type {
        let type_str = ret.trim_start_matches("->").trim().to_string();
        if type_str != "None" && !type_str.is_empty() {
            ctx.pending_uses_type.push((func_id.clone(), type_str));
        }
    }

    // Parse parameters for type annotations
    if let Some(params) = params_node {
        parse_params(source, params, &func_id, ctx);
    }

    // Parse body for assignments, calls, nested functions
    if let Some(body) = body_node {
        parse_function_body(source, body, module_id, rel_path, graph, ctx, parent_class, &func_id);
    }
}

fn parse_params(source: &str, params_node: TsNode, func_id: &str, ctx: &mut ParseContext) {
    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        match child.kind() {
            "typed_parameter" | "typed_default_parameter" => {
                let mut c2 = child.walk();
                let mut pname = String::new();
                let mut ptype = String::new();
                for c in child.children(&mut c2) {
                    match c.kind() {
                        "identifier" if pname.is_empty() => pname = node_text(c, source).to_string(),
                        "type" if ptype.is_empty() => ptype = node_text(c, source).to_string(),
                        "identifier" if !pname.is_empty() && ptype.is_empty() => {
                            ptype = node_text(c, source).to_string();
                        }
                        _ => {}
                    }
                }
                if !pname.is_empty() && !ptype.is_empty() && pname != "self" && pname != "cls" {
                    ctx.local_var_types
                        .entry(func_id.to_string())
                        .or_default()
                        .insert(pname, ptype.clone());
                    ctx.pending_uses_type.push((func_id.to_string(), ptype));
                }
            }
            _ => {}
        }
    }
}

fn parse_function_body(
    source: &str,
    body: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
    ctx: &mut ParseContext,
    parent_class: Option<&str>,
    func_id: &str,
) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "expression_statement" => {
                let mut c2 = child.walk();
                for c in child.children(&mut c2) {
                    match c.kind() {
                        "call" => {
                            parse_call(source, c, func_id, parent_class, ctx);
                        }
                        "assignment" => {
                            parse_assignment_in_func(source, c, func_id, parent_class, ctx);
                        }
                        _ => {}
                    }
                }
            }
            "assignment" => {
                parse_assignment_in_func(source, child, func_id, parent_class, ctx);
            }
            "annotated_assignment" => {
                parse_annotated_assignment(source, child, func_id, parent_class, ctx);
            }
            "return_statement" => {
                // Could track but skip for now
            }
            "function_definition" => {
                // Nested function
                parse_function(
                    source,
                    child,
                    module_id,
                    rel_path,
                    graph,
                    ctx,
                    parent_class,
                    Some(func_id),
                );
            }
            "if_statement" | "for_statement" | "while_statement" | "try_statement"
            | "with_statement" => {
                parse_function_body(source, child, module_id, rel_path, graph, ctx, parent_class, func_id);
            }
            _ => {}
        }
    }
}

fn parse_call(
    source: &str,
    node: TsNode,
    caller_id: &str,
    in_class: Option<&str>,
    ctx: &mut ParseContext,
) {
    let mut cursor = node.walk();
    let mut func_part: Option<TsNode> = None;
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute" || child.kind() == "identifier" {
            func_part = Some(child);
            break;
        }
    }

    if let Some(fp) = func_part {
        if fp.kind() == "attribute" {
            let mut c2 = fp.walk();
            let mut obj = String::new();
            let mut method = String::new();
            for c in fp.children(&mut c2) {
                match c.kind() {
                    "identifier" if obj.is_empty() => obj = node_text(c, source).to_string(),
                    "identifier" => method = node_text(c, source).to_string(),
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
        } else {
            let name = node_text(fp, source).to_string();
            if !name.is_empty() {
                ctx.pending_calls.push(PendingCall {
                    caller_id: caller_id.to_string(),
                    callee_name: name,
                    object: None,
                    in_class: in_class.map(|s| s.to_string()),
                });
            }
        }
    }
}

fn parse_assignment_in_func(
    source: &str,
    node: TsNode,
    func_id: &str,
    in_class: Option<&str>,
    ctx: &mut ParseContext,
) {
    // self.x = ClassName(...) or x = ClassName(...)
    let mut cursor = node.walk();
    let children: Vec<TsNode> = node.children(&mut cursor).collect();

    if children.len() < 3 {
        return;
    }

    let lhs = &children[0];
    let rhs = children.last().unwrap();

    // Determine variable name
    let var_name = if lhs.kind() == "attribute" {
        let mut c2 = lhs.walk();
        let parts: Vec<&str> = lhs
            .children(&mut c2)
            .filter(|c| c.kind() == "identifier")
            .map(|c| node_text(c, source))
            .collect();
        if parts.len() >= 2 && parts[0] == "self" {
            Some(("self.".to_string() + parts[1], true))
        } else {
            None
        }
    } else if lhs.kind() == "identifier" {
        Some((node_text(*lhs, source).to_string(), false))
    } else {
        None
    };

    // Determine type from RHS call
    let type_name = if rhs.kind() == "call" {
        let mut c2 = rhs.walk();
        let mut func_name = String::new();
        for c in rhs.children(&mut c2) {
            if c.kind() == "identifier" || c.kind() == "attribute" {
                func_name = node_text(c, source).to_string();
                break;
            }
        }
        if !func_name.is_empty() {
            Some(func_name)
        } else {
            None
        }
    } else {
        None
    };

    if let (Some((var, is_self)), Some(tp)) = (var_name, type_name) {
        if is_self {
            if let Some(class_id) = in_class {
                let field_name = var.trim_start_matches("self.").to_string();
                ctx.class_fields
                    .entry(class_id.to_string())
                    .or_default()
                    .insert(field_name, tp.clone());
                ctx.pending_uses_type.push((class_id.to_string(), tp));
            }
        } else {
            ctx.local_var_types
                .entry(func_id.to_string())
                .or_default()
                .insert(var, tp.clone());
            ctx.pending_uses_type.push((func_id.to_string(), tp));
        }
    }
}

fn parse_annotated_assignment(
    source: &str,
    node: TsNode,
    func_id: &str,
    in_class: Option<&str>,
    ctx: &mut ParseContext,
) {
    // self.x: ClassName = ... or x: ClassName = ...
    let mut cursor = node.walk();
    let mut var_name: Option<String> = None;
    let mut is_self = false;
    let mut type_name: Option<String> = None;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "attribute" if var_name.is_none() => {
                let mut c2 = child.walk();
                let parts: Vec<&str> = child
                    .children(&mut c2)
                    .filter(|c| c.kind() == "identifier")
                    .map(|c| node_text(c, source))
                    .collect();
                if parts.len() >= 2 && parts[0] == "self" {
                    var_name = Some(parts[1].to_string());
                    is_self = true;
                }
            }
            "identifier" if var_name.is_none() => {
                var_name = Some(node_text(child, source).to_string());
            }
            "type" if type_name.is_none() => {
                type_name = Some(node_text(child, source).to_string());
            }
            "identifier" if var_name.is_some() && type_name.is_none() => {
                type_name = Some(node_text(child, source).to_string());
            }
            _ => {}
        }
    }

    if let (Some(var), Some(tp)) = (var_name, type_name) {
        if is_self {
            if let Some(class_id) = in_class {
                ctx.class_fields
                    .entry(class_id.to_string())
                    .or_default()
                    .insert(var, tp.clone());
                ctx.pending_uses_type.push((class_id.to_string(), tp));
            }
        } else {
            ctx.local_var_types
                .entry(func_id.to_string())
                .or_default()
                .insert(var, tp.clone());
            ctx.pending_uses_type.push((func_id.to_string(), tp));
        }
    }
}

fn parse_constant(
    source: &str,
    node: TsNode,
    module_id: &str,
    rel_path: &str,
    graph: &mut CodeGraph,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "assignment" {
            let mut c2 = child.walk();
            let children: Vec<TsNode> = child.children(&mut c2).collect();
            if let Some(lhs) = children.first() {
                if lhs.kind() == "identifier" {
                    let name = node_text(*lhs, source).to_string();
                    if name.chars().all(|c| c.is_uppercase() || c == '_') && name.len() > 1 {
                        let line = node.start_position().row + 1;
                        let const_id = format!("{}.{}", module_id, name);
                        graph.add_node(Node {
                            id: const_id.clone(),
                            name: name.clone(),
                            kind: NodeKind::Constant,
                            file: rel_path.to_string(),
                            line,
                            is_external: false,
                            docstring: None,
                        });
                        graph.add_edge(module_id, &const_id, EdgeKind::Contains);
                    }
                }
            }
        }
    }
}
