pub mod cpp;
pub mod python;
pub mod rust_lang;

use anyhow::Result;
use core::{CodeGraph, EdgeKind, Node, NodeKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Python,
    Rust,
    Cpp,
}

#[derive(Debug, Clone)]
pub struct PendingCall {
    pub caller_id: String,
    pub callee_name: String,
    pub object: Option<String>,
    pub in_class: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ParseContext {
    pub module_id: String,
    pub file: String,
    pub imports: HashMap<String, String>,
    pub alias_map: HashMap<String, String>,
    pub pending_calls: Vec<PendingCall>,
    pub pending_inherits: Vec<(String, String)>,
    pub pending_uses_type: Vec<(String, String)>,
    pub local_var_types: HashMap<String, HashMap<String, String>>,
    pub class_fields: HashMap<String, HashMap<String, String>>,
    /// Rust `mod foo;` external declarations: (parent_module_id, mod_name)
    pub pending_mods: Vec<(String, String)>,
}

/// Infer the NodeKind for an external symbol from its short name.
/// A node is Module only when it corresponds to an actual source file —
/// external symbols should be Class / Constant / Function, never Module.
fn infer_external_kind(name: &str) -> NodeKind {
    // Take only the last component (after :: or .)
    let last = name.split("::").last()
        .or_else(|| name.split('.').last())
        .unwrap_or(name)
        .trim();

    if last.is_empty() { return NodeKind::Module; }

    let first = last.chars().next().unwrap();

    // ALL_CAPS → constant
    if last.len() > 1
        && last.chars().all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit())
    {
        return NodeKind::Constant;
    }

    // PascalCase (first char uppercase) → struct / enum / class
    if first.is_uppercase() {
        return NodeKind::Class;
    }

    // Lowercase name that contains '::' or '.' separators looks like a path → Module
    // Otherwise treat as a free function
    if name.contains("::") || name.contains('.') {
        NodeKind::Module
    } else {
        NodeKind::Function
    }
}

pub struct Resolver {
    name_registry: HashMap<String, Vec<String>>,
}

impl Resolver {
    pub fn build(graph: &CodeGraph) -> Self {
        let mut name_registry: HashMap<String, Vec<String>> = HashMap::new();
        for node in graph.nodes() {
            let short = node.name.clone();
            name_registry
                .entry(short)
                .or_default()
                .push(node.id.clone());
            // Also index last segment of id
            if let Some(seg) = node.id.split('.').last() {
                if seg != node.name {
                    name_registry
                        .entry(seg.to_string())
                        .or_default()
                        .push(node.id.clone());
                }
            }
        }
        Resolver { name_registry }
    }

    fn resolve_name(
        &self,
        name: &str,
        ctx: &ParseContext,
        graph: &mut CodeGraph,
    ) -> Option<String> {
        // 1. Check local imports
        if let Some(full_path) = ctx.imports.get(name) {
            if graph.has_node(full_path) {
                return Some(full_path.clone());
            }
            // Try alias map
        }
        // Resolve alias
        let resolved_name = if let Some(real) = ctx.alias_map.get(name) {
            real.as_str()
        } else {
            name
        };

        // 2. Check name_registry
        if let Some(candidates) = self.name_registry.get(resolved_name) {
            if candidates.len() == 1 {
                return Some(candidates[0].clone());
            }
            // Prefer same module
            for c in candidates {
                if c.starts_with(&ctx.module_id) {
                    return Some(c.clone());
                }
            }
            if !candidates.is_empty() {
                return Some(candidates[0].clone());
            }
        }

        // 3. If the name contains path separators, try looking up just the last segment
        // before falling back to external. Handles cases like `crate::foo::Bar` where
        // the full path was not stripped at call site but "Bar" is a known internal node.
        let short_name = resolved_name
            .split("::")
            .last()
            .or_else(|| resolved_name.split('.').last())
            .unwrap_or(resolved_name);
        if short_name != resolved_name {
            if let Some(candidates) = self.name_registry.get(short_name) {
                if let Some(id) = candidates.iter().find(|id| !id.starts_with("__ext__.")) {
                    return Some(id.clone());
                }
            }
        }

        // 4. Genuinely external — keyed by short name to avoid duplicates
        let ext_id = format!("__ext__.{}", short_name);
        if !graph.has_node(&ext_id) {
            graph.add_node(Node {
                id: ext_id.clone(),
                name: short_name.to_string(),
                kind: infer_external_kind(resolved_name),
                file: String::new(),
                line: 0,
                is_external: true,
                docstring: None,
            });
        }
        Some(ext_id)
    }

    pub fn resolve_all(ctxs: Vec<ParseContext>, graph: &mut CodeGraph) {
        use std::collections::HashSet;

        // Build registry and collect internal module node IDs before any resolution
        let resolver = Resolver::build(graph);
        let internal_module_ids: HashSet<String> = graph
            .nodes()
            .filter(|n| n.kind == NodeKind::Module && !n.is_external)
            .map(|n| n.id.clone())
            .collect();

        for ctx in ctxs {
            // ── mod foo; → Contains edge to the child module file ────────────
            for (parent_id, mod_name) in &ctx.pending_mods {
                if let Some(candidates) = resolver.name_registry.get(mod_name) {
                    // Prefer an internal Module node whose id ends with ::mod_name
                    let target = candidates
                        .iter()
                        .find(|id| internal_module_ids.contains(*id))
                        .or_else(|| candidates.first());
                    if let Some(t) = target {
                        graph.add_edge(parent_id, t, EdgeKind::Contains);
                    }
                }
            }

            // ── Inherits ─────────────────────────────────────────────────────
            for (child_id, parent_name) in &ctx.pending_inherits {
                if let Some(target_id) = resolver.resolve_name(parent_name, &ctx, graph) {
                    graph.add_edge(child_id, &target_id, EdgeKind::Inherits);
                }
            }

            // ── UsesType ─────────────────────────────────────────────────────
            for (user_id, type_name) in &ctx.pending_uses_type {
                if let Some(target_id) = resolver.resolve_name(type_name, &ctx, graph) {
                    graph.add_edge(user_id, &target_id, EdgeKind::UsesType);
                }
            }

            // ── Calls ────────────────────────────────────────────────────────
            for pc in &ctx.pending_calls {
                let callee = if let Some(obj) = &pc.object {
                    let type_name = ctx
                        .class_fields
                        .get(pc.in_class.as_deref().unwrap_or(""))
                        .and_then(|f| f.get(obj.as_str()))
                        .or_else(|| {
                            ctx.local_var_types
                                .get(&pc.caller_id)
                                .and_then(|m| m.get(obj.as_str()))
                        })
                        .cloned();
                    if let Some(tp) = type_name {
                        format!("{}.{}", tp, pc.callee_name)
                    } else {
                        pc.callee_name.clone()
                    }
                } else {
                    pc.callee_name.clone()
                };

                if let Some(target_id) = resolver.resolve_name(&callee, &ctx, graph) {
                    graph.add_edge(&pc.caller_id, &target_id, EdgeKind::Calls);
                }
            }

            // ── Imports ───────────────────────────────────────────────────────
            // Each (local_alias, full_path) pair. full_path may appear twice (once as key
            // and once as value) so we deduplicate by full_path.
            let mut seen_paths: HashSet<&str> = HashSet::new();
            for (_local, full_path) in &ctx.imports {
                if !seen_paths.insert(full_path.as_str()) {
                    continue;
                }

                if graph.has_node(full_path) {
                    // Already an internal node with exactly this ID
                    graph.add_edge(&ctx.module_id, full_path, EdgeKind::Imports);
                    continue;
                }

                // Try to resolve to an existing internal node by short name
                let short = full_path
                    .split("::")
                    .last()
                    .or_else(|| full_path.split('.').last())
                    .unwrap_or(full_path.as_str());

                let internal_hit = resolver
                    .name_registry
                    .get(short)
                    .and_then(|candidates| {
                        // Prefer internal nodes (no __ext__ prefix)
                        candidates
                            .iter()
                            .find(|id| !id.starts_with("__ext__."))
                            .or_else(|| candidates.first())
                    })
                    .cloned();

                if let Some(ref target_id) = internal_hit {
                    if !target_id.starts_with("__ext__.") {
                        // Resolved to an internal node — link with Imports, no new node
                        graph.add_edge(&ctx.module_id, target_id, EdgeKind::Imports);
                        continue;
                    }
                }

                // Genuinely external — key by short name to avoid duplicates
                let ext_id = format!("__ext__.{}", short);
                if !graph.has_node(&ext_id) {
                    graph.add_node(Node {
                        id: ext_id.clone(),
                        name: short.to_string(),
                        kind: infer_external_kind(full_path),
                        file: String::new(),
                        line: 0,
                        is_external: true,
                        docstring: None,
                    });
                }
                graph.add_edge(&ctx.module_id, &ext_id, EdgeKind::ExternalDep);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cache {
    pub hashes: HashMap<String, String>,
    pub graph_json: Option<String>,
}

impl Cache {
    pub fn load(dir: &Path) -> Option<Self> {
        let path = dir.join(".codegraph_cache.json");
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn save(&self, dir: &Path) {
        let path = dir.join(".codegraph_cache.json");
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, data);
        }
    }

    pub fn is_unchanged(&self, file: &Path) -> bool {
        let key = file.to_string_lossy().to_string();
        if let Some(cached_hash) = self.hashes.get(&key) {
            if let Ok(data) = std::fs::read(file) {
                let hash = blake3::hash(&data).to_hex().to_string();
                return *cached_hash == hash;
            }
        }
        false
    }
}

fn collect_files(root: &Path, languages: &[Language]) -> Vec<(PathBuf, Language)> {
    let mut result = Vec::new();
    collect_files_recursive(root, root, languages, &mut result);
    result
}

fn collect_files_recursive(
    root: &Path,
    dir: &Path,
    languages: &[Language],
    result: &mut Vec<(PathBuf, Language)>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            collect_files_recursive(root, &path, languages, result);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let lang = match ext {
                "py" if languages.contains(&Language::Python) => Some(Language::Python),
                "rs" if languages.contains(&Language::Rust) => Some(Language::Rust),
                "cpp" | "cc" | "cxx" | "h" | "hpp"
                    if languages.contains(&Language::Cpp) =>
                {
                    Some(Language::Cpp)
                }
                _ => None,
            };
            if let Some(l) = lang {
                result.push((path, l));
            }
        }
    }
}

pub fn analyze(root: &Path, languages: &[Language], use_cache: bool) -> Result<CodeGraph> {
    let mut cache_opt = if use_cache { Cache::load(root) } else { None };

    let files = collect_files(root, languages);

    // Check if all files are unchanged
    if let Some(ref cache) = cache_opt {
        if let Some(ref json) = cache.graph_json {
            let all_unchanged = files.iter().all(|(p, _)| cache.is_unchanged(p));
            if all_unchanged && !files.is_empty() {
                let sg: core::SerializableGraph = serde_json::from_str(json)?;
                return Ok(CodeGraph::from_serializable(sg));
            }
        }
    }

    let mut graph = CodeGraph::new();
    let mut contexts: Vec<ParseContext> = Vec::new();
    let mut new_hashes: HashMap<String, String> = HashMap::new();

    for (path, lang) in &files {
        let data = std::fs::read(path)?;
        let hash = blake3::hash(&data).to_hex().to_string();
        new_hashes.insert(path.to_string_lossy().to_string(), hash);

        let source = String::from_utf8_lossy(&data).to_string();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let ctx = match lang {
            Language::Python => python::parse_file(&source, &rel, &mut graph)?,
            Language::Rust => rust_lang::parse_file(&source, &rel, &mut graph)?,
            Language::Cpp => cpp::parse_file(&source, &rel, &mut graph)?,
        };
        contexts.push(ctx);
    }

    Resolver::resolve_all(contexts, &mut graph);

    // Save cache
    if use_cache {
        let sg = graph.to_serializable();
        let json = serde_json::to_string(&sg)?;
        let new_cache = Cache {
            hashes: new_hashes,
            graph_json: Some(json),
        };
        new_cache.save(root);
        cache_opt = Some(new_cache);
    }
    let _ = cache_opt;

    Ok(graph)
}
