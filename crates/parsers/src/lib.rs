pub mod cpp;
pub mod python;
pub mod rust_lang;
pub mod typescript;

use anyhow::Result;
use core::{CodeGraph, EdgeKind, Node, NodeKind};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// ── Language enum ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Python,
    Rust,
    Cpp,
    TypeScript,
    JavaScript,
}

// ── ParseContext & PendingCall — serialisable pour le cache incrémental ───────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCall {
    pub caller_id: String,
    pub callee_name: String,
    pub object: Option<String>,
    pub in_class: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// Rust `mod foo;` external declarations
    pub pending_mods: Vec<(String, String)>,
}

// ── External kind inference ───────────────────────────────────────────────────

/// Infer the NodeKind for an external symbol from its short name.
fn infer_external_kind(name: &str) -> NodeKind {
    let last = name
        .split("::")
        .last()
        .or_else(|| name.split('.').last())
        .unwrap_or(name)
        .trim();

    if last.is_empty() {
        return NodeKind::Module;
    }

    let first = last.chars().next().unwrap();

    if last.len() > 1 && last.chars().all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit()) {
        return NodeKind::Constant;
    }

    if first.is_uppercase() {
        return NodeKind::Class;
    }

    if name.contains("::") || name.contains('.') {
        NodeKind::Module
    } else {
        NodeKind::Function
    }
}

// ── Resolver ──────────────────────────────────────────────────────────────────

pub struct Resolver {
    name_registry: HashMap<String, Vec<String>>,
}

impl Resolver {
    pub fn build(graph: &CodeGraph) -> Self {
        let mut name_registry: HashMap<String, Vec<String>> = HashMap::new();
        for node in graph.nodes() {
            let short = node.name.clone();
            name_registry.entry(short).or_default().push(node.id.clone());
            if let Some(seg) = node.id.split('.').last() {
                if seg != node.name {
                    name_registry.entry(seg.to_string()).or_default().push(node.id.clone());
                }
            }
        }
        Resolver { name_registry }
    }

    fn resolve_name(&self, name: &str, ctx: &ParseContext, graph: &mut CodeGraph) -> Option<String> {
        if let Some(full_path) = ctx.imports.get(name) {
            if graph.has_node(full_path) {
                return Some(full_path.clone());
            }
        }

        let resolved_name = if let Some(real) = ctx.alias_map.get(name) {
            real.as_str()
        } else {
            name
        };

        if let Some(candidates) = self.name_registry.get(resolved_name) {
            if candidates.len() == 1 {
                return Some(candidates[0].clone());
            }
            for c in candidates {
                if c.starts_with(&ctx.module_id) {
                    return Some(c.clone());
                }
            }
            if !candidates.is_empty() {
                return Some(candidates[0].clone());
            }
        }

        let short_name = resolved_name
            .split("::")
            .last()
            .or_else(|| resolved_name.split('.').last())
            .or_else(|| resolved_name.split('/').last())
            .unwrap_or(resolved_name);
        if short_name != resolved_name {
            if let Some(candidates) = self.name_registry.get(short_name) {
                if let Some(id) = candidates.iter().find(|id| !id.starts_with("__ext__.")) {
                    return Some(id.clone());
                }
            }
        }

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
        let resolver = Resolver::build(graph);
        let internal_module_ids: HashSet<String> = graph
            .nodes()
            .filter(|n| n.kind == NodeKind::Module && !n.is_external)
            .map(|n| n.id.clone())
            .collect();

        for ctx in ctxs {
            // mod foo; → Contains edge to child module
            for (parent_id, mod_name) in &ctx.pending_mods {
                if let Some(candidates) = resolver.name_registry.get(mod_name) {
                    let target = candidates
                        .iter()
                        .find(|id| internal_module_ids.contains(*id))
                        .or_else(|| candidates.first());
                    if let Some(t) = target {
                        graph.add_edge(parent_id, t, EdgeKind::Contains);
                    }
                }
            }

            for (child_id, parent_name) in &ctx.pending_inherits {
                if let Some(target_id) = resolver.resolve_name(parent_name, &ctx, graph) {
                    graph.add_edge(child_id, &target_id, EdgeKind::Inherits);
                }
            }

            for (user_id, type_name) in &ctx.pending_uses_type {
                if let Some(target_id) = resolver.resolve_name(type_name, &ctx, graph) {
                    graph.add_edge(user_id, &target_id, EdgeKind::UsesType);
                }
            }

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

            let mut seen_paths: HashSet<&str> = HashSet::new();
            for (_local, full_path) in &ctx.imports {
                if !seen_paths.insert(full_path.as_str()) {
                    continue;
                }

                if graph.has_node(full_path) {
                    graph.add_edge(&ctx.module_id, full_path, EdgeKind::Imports);
                    continue;
                }

                let short = full_path
                    .split("::")
                    .last()
                    .or_else(|| full_path.split('.').last())
                    .or_else(|| full_path.split('/').last())
                    .unwrap_or(full_path.as_str());

                let internal_hit = resolver
                    .name_registry
                    .get(short)
                    .and_then(|candidates| {
                        candidates
                            .iter()
                            .find(|id| !id.starts_with("__ext__."))
                            .or_else(|| candidates.first())
                    })
                    .cloned();

                if let Some(ref target_id) = internal_hit {
                    if !target_id.starts_with("__ext__.") {
                        graph.add_edge(&ctx.module_id, target_id, EdgeKind::Imports);
                        continue;
                    }
                }

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

// ── Cache incrémental ─────────────────────────────────────────────────────────
//
// Le cache stocke :
//   - hashes  : hash blake3 de chaque fichier (clé = chemin absolu)
//   - graph_json   : graphe sérialisé (nœuds + arêtes)
//   - contexts_json : Vec<ParseContext> pour tous les fichiers analysés
//
// Lors d'un rechargement :
//   1. Les fichiers INCHANGÉS → leurs nœuds sont restaurés depuis graph_json
//      ET leur ParseContext est restauré depuis contexts_json.
//   2. Les fichiers MODIFIÉS → re-parsés depuis zéro (nouveaux nœuds + contexte).
//   3. Le Resolver tourne sur (old_contexts + new_contexts) → résolution correcte.
//
// Limite connue : si un fichier inchangé référençait un symbole renommé dans un
// fichier modifié, l'arête sera manquante jusqu'au prochain cache-clear.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cache {
    pub hashes: HashMap<String, String>,
    pub graph_json: Option<String>,
    pub contexts_json: Option<String>,
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

    pub fn hash_for(&self, key: &str) -> Option<&String> {
        self.hashes.get(key)
    }
}

// ── File collection ───────────────────────────────────────────────────────────

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
            // Skip hidden directories, build artifacts, and dependency folders
            if name.starts_with('.')
                || name == "target"
                || name == "node_modules"
                || name == "__pycache__"
                || name == "dist"
                || name == "build"
            {
                continue;
            }
            collect_files_recursive(root, &path, languages, result);
        } else {
            // Skip codegraph-generated output files to avoid watch-mode loops
            let fname = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if fname == "graph.html"
                || fname == "graph.json"
                || fname == "diff.html"
                || fname == ".codegraph_cache.json"
            {
                continue;
            }

            let lang = match path.extension().and_then(|e| e.to_str()) {
                Some("py") if languages.contains(&Language::Python) => Some(Language::Python),
                Some("rs") if languages.contains(&Language::Rust) => Some(Language::Rust),
                Some("cpp") | Some("cc") | Some("cxx") | Some("h") | Some("hpp")
                    if languages.contains(&Language::Cpp) =>
                {
                    Some(Language::Cpp)
                }
                Some("ts") | Some("tsx") if languages.contains(&Language::TypeScript) => {
                    Some(Language::TypeScript)
                }
                Some("js") | Some("jsx") | Some("mjs") | Some("cjs")
                    if languages.contains(&Language::JavaScript) =>
                {
                    Some(Language::JavaScript)
                }
                _ => None,
            };
            if let Some(l) = lang {
                result.push((path, l));
            }
        }
    }
}

// ── Analyse principale ────────────────────────────────────────────────────────

pub fn analyze(root: &Path, languages: &[Language], use_cache: bool) -> Result<CodeGraph> {
    let cache_opt = if use_cache { Cache::load(root) } else { None };

    let files = collect_files(root, languages);

    // ── Étape 1 : lecture parallèle + hash (rayon) ────────────────────────────
    let file_data: Vec<(PathBuf, Language, Vec<u8>, String, String)> = files
        .par_iter()
        .filter_map(|(path, lang)| {
            let data = std::fs::read(path).ok()?;
            let hash = blake3::hash(&data).to_hex().to_string();
            let _key = path.to_string_lossy().to_string();
            let rel = path
                .strip_prefix(root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            Some((path.clone(), lang.clone(), data, hash, rel))
        })
        .collect();

    // ── Étape 2 : déterminer les fichiers modifiés vs inchangés ──────────────
    let mut new_hashes: HashMap<String, String> = HashMap::new();
    let mut unchanged_rel: HashSet<String> = HashSet::new();

    for (path, _lang, _data, hash, rel) in &file_data {
        let key = path.to_string_lossy().to_string();
        let is_unchanged = cache_opt
            .as_ref()
            .and_then(|c| c.hash_for(&key))
            .map(|h| h == hash)
            .unwrap_or(false);
        new_hashes.insert(key, hash.clone());
        if is_unchanged {
            unchanged_rel.insert(rel.clone());
        }
    }

    // ── Étape 3 : restaurer les nœuds/arêtes inchangés depuis le cache ────────
    let mut graph = CodeGraph::new();
    let mut cached_contexts: Vec<ParseContext> = Vec::new();

    if let Some(ref cache) = cache_opt {
        // Restaurer les nœuds
        if let Some(ref json) = cache.graph_json {
            if let Ok(sg) = serde_json::from_str::<core::SerializableGraph>(json) {
                // Restaurer les nœuds des fichiers inchangés + nœuds externes
                for node in &sg.nodes {
                    if unchanged_rel.contains(node.file.as_str()) || node.is_external {
                        graph.add_node(node.clone());
                    }
                }
                // Arêtes où les deux extrémités sont présentes dans le graphe restauré
                for edge in &sg.edges {
                    if graph.has_node(&edge.source) && graph.has_node(&edge.target) {
                        graph.add_edge(&edge.source, &edge.target, edge.kind.clone());
                    }
                }
            }
        }

        // Restaurer les ParseContexts des fichiers inchangés
        if let Some(ref ctx_json) = cache.contexts_json {
            if let Ok(ctxs) = serde_json::from_str::<Vec<ParseContext>>(ctx_json) {
                for ctx in ctxs {
                    if unchanged_rel.contains(ctx.file.as_str()) {
                        cached_contexts.push(ctx);
                    }
                }
            }
        }
    }

    // ── Étape 4 : parser uniquement les fichiers modifiés ────────────────────
    let mut new_contexts: Vec<ParseContext> = Vec::new();

    for (path, lang, data, _hash, rel) in &file_data {
        if unchanged_rel.contains(rel.as_str()) {
            continue; // déjà restauré depuis le cache
        }

        let source = String::from_utf8_lossy(data).to_string();

        let ctx = match lang {
            Language::Python => python::parse_file(&source, rel, &mut graph)?,
            Language::Rust => rust_lang::parse_file(&source, rel, &mut graph)?,
            Language::Cpp => cpp::parse_file(&source, rel, &mut graph)?,
            Language::TypeScript => {
                typescript::parse_file(&source, rel, &mut graph, typescript::JsVariant::TypeScript)?
            }
            Language::JavaScript => {
                typescript::parse_file(&source, rel, &mut graph, typescript::JsVariant::JavaScript)?
            }
        };
        new_contexts.push(ctx);
        let _ = path;
    }

    // ── Étape 5 : résolution (contextes inchangés + nouveaux) ─────────────────
    let all_contexts: Vec<ParseContext> = cached_contexts
        .into_iter()
        .chain(new_contexts.into_iter())
        .collect();

    Resolver::resolve_all(all_contexts.clone(), &mut graph);

    // ── Étape 6 : mise à jour du cache ───────────────────────────────────────
    if use_cache {
        let sg = graph.to_serializable();
        let graph_json = serde_json::to_string(&sg)?;
        let contexts_json = serde_json::to_string(&all_contexts)?;
        Cache {
            hashes: new_hashes,
            graph_json: Some(graph_json),
            contexts_json: Some(contexts_json),
        }
        .save(root);
    }

    Ok(graph)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use tempfile::TempDir;

    fn write_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_collect_files_excludes_generated() {
        let dir = TempDir::new().unwrap();
        write_file(&dir, "main.py", "");
        write_file(&dir, "graph.html", "");
        write_file(&dir, "graph.json", "");
        write_file(&dir, ".codegraph_cache.json", "");

        let files = collect_files(dir.path(), &[Language::Python]);
        assert_eq!(files.len(), 1, "Generated files should be excluded");
        assert!(files[0].0.file_name().unwrap() == "main.py");
    }

    #[test]
    fn test_analyze_python_simple() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir,
            "app.py",
            "class Dog:\n    def bark(self):\n        pass\n",
        );

        let graph = analyze(dir.path(), &[Language::Python], false).unwrap();
        assert!(graph.has_node("app"), "Module node should exist");
        assert!(graph.has_node("app.Dog"), "Class node should exist");
        assert!(graph.has_node("app.Dog.bark"), "Method node should exist");
    }

    #[test]
    fn test_analyze_typescript_simple() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir,
            "index.ts",
            "class Cat { meow() {} }\nfunction greet() {}\n",
        );

        let graph = analyze(dir.path(), &[Language::TypeScript], false).unwrap();
        assert!(graph.has_node("index"), "Module should exist");
        assert!(graph.has_node("index/Cat"));
        assert!(graph.has_node("index/greet"));
    }

    #[test]
    fn test_cache_incremental_no_reparse_unchanged() {
        let dir = TempDir::new().unwrap();
        write_file(&dir, "mod.py", "def hello(): pass\n");

        // Premier passage — crée le cache
        let g1 = analyze(dir.path(), &[Language::Python], true).unwrap();
        assert!(g1.has_node("mod.hello"));

        // Deuxième passage — tout inchangé, doit charger depuis le cache
        let g2 = analyze(dir.path(), &[Language::Python], true).unwrap();
        assert!(g2.has_node("mod.hello"), "Node should be restored from cache");
    }

    #[test]
    fn test_infer_external_kind() {
        assert_eq!(infer_external_kind("SOME_CONSTANT"), NodeKind::Constant);
        assert_eq!(infer_external_kind("MyClass"), NodeKind::Class);
        assert_eq!(infer_external_kind("some_function"), NodeKind::Function);
    }
}
