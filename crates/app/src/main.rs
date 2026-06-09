use analysis::GraphAnalysis;
use core::CodeGraph;
use eframe::egui;
use notify::{Event, RecursiveMode, Watcher};
use parsers::Language;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Code Graph")
            .with_inner_size([720.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Code Graph",
        options,
        Box::new(|_cc| Ok(Box::new(AppState::default()))),
    )
}

// ── Worker messages ───────────────────────────────────────────────────────────

enum WorkerMsg {
    Done(AnalysisResult),
    Error(String),
}

struct AnalysisResult {
    graph: CodeGraph,
    analysis: GraphAnalysis,
    elapsed_secs: f64,
    graph_html_path: Option<PathBuf>,
    json_path: Option<PathBuf>,
    diff_html_path: Option<PathBuf>,
}

// ── App state ─────────────────────────────────────────────────────────────────

#[derive(Default, PartialEq)]
enum AppStatus {
    #[default]
    Idle,
    Analyzing,
}

struct AppState {
    // UI inputs
    project_path:  String,
    output_path:   String, // dossier de sortie ; vide = même que project_path
    use_python:     bool,
    use_rust:       bool,
    use_cpp:        bool,
    use_typescript: bool,
    use_javascript: bool,
    use_cache:      bool,
    watch_mode:     bool,
    focus_node:     String,
    focus_depth:    usize,
    git_ref:        String,

    // State
    status: AppStatus,
    tx: Option<Sender<WorkerMsg>>,
    rx: Option<Receiver<WorkerMsg>>,

    // Results
    last_result: Option<AnalysisResult>,
    last_error:  Option<String>,
    clean_msg:   Option<String>, // retour de l'opération « Nettoyer »

    // Watch
    _watcher:  Option<Box<dyn notify::Watcher + Send>>,
    watch_tx:  Option<Sender<()>>,
    watch_rx:  Option<Receiver<()>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            project_path:   String::new(),
            output_path:    String::new(),
            use_python:     true,
            use_rust:       true,
            use_cpp:        false,
            use_typescript: false,
            use_javascript: false,
            use_cache:      true,
            watch_mode:     false,
            focus_node:     String::new(),
            focus_depth:    2,
            git_ref:        "main".to_string(),
            status:         AppStatus::Idle,
            tx:             None,
            rx:             None,
            last_result:    None,
            last_error:     None,
            clean_msg:      None,
            _watcher:       None,
            watch_tx:       None,
            watch_rx:       None,
        }
    }
}

impl AppState {
    // Dossier de sortie effectif : output_path si renseigné, sinon project_path.
    fn effective_output_dir(&self) -> PathBuf {
        let p = self.output_path.trim();
        if p.is_empty() {
            PathBuf::from(&self.project_path)
        } else {
            PathBuf::from(p)
        }
    }

    fn selected_languages(&self) -> Vec<Language> {
        let mut langs = Vec::new();
        if self.use_python     { langs.push(Language::Python);     }
        if self.use_rust       { langs.push(Language::Rust);       }
        if self.use_cpp        { langs.push(Language::Cpp);        }
        if self.use_typescript { langs.push(Language::TypeScript); }
        if self.use_javascript { langs.push(Language::JavaScript); }
        langs
    }

    fn start_analysis(&mut self) {
        if self.project_path.is_empty() {
            self.last_error = Some("Chemin du projet vide.".to_string());
            return;
        }
        let root = PathBuf::from(&self.project_path);
        if !root.exists() {
            self.last_error = Some(format!("Chemin inexistant : {}", root.display()));
            return;
        }
        let langs = self.selected_languages();
        if langs.is_empty() {
            self.last_error = Some("Sélectionnez au moins un langage.".to_string());
            return;
        }

        let use_cache   = self.use_cache;
        let focus_node  = self.focus_node.trim().to_string();
        let focus_depth = self.focus_depth;
        let output_dir  = self.effective_output_dir();

        let (tx, rx) = mpsc::channel::<WorkerMsg>();
        let tx_clone = tx.clone();
        self.tx = Some(tx);
        self.rx = Some(rx);
        self.status     = AppStatus::Analyzing;
        self.last_error = None;
        self.clean_msg  = None;

        std::thread::spawn(move || {
            let start = Instant::now();
            let result = run_analysis(&root, &output_dir, &langs, use_cache, &focus_node, focus_depth);
            let elapsed = start.elapsed().as_secs_f64();
            match result {
                Ok(mut r) => {
                    r.elapsed_secs = elapsed;
                    let _ = tx_clone.send(WorkerMsg::Done(r));
                }
                Err(e) => {
                    let _ = tx_clone.send(WorkerMsg::Error(e.to_string()));
                }
            }
        });
    }

    fn start_git_diff(&mut self) {
        if self.project_path.is_empty() || self.git_ref.is_empty() {
            return;
        }
        let root       = PathBuf::from(&self.project_path);
        let git_ref    = self.git_ref.clone();
        let langs      = self.selected_languages();
        let output_dir = self.effective_output_dir();

        let (tx, rx) = mpsc::channel::<WorkerMsg>();
        let tx_clone = tx.clone();
        self.tx = Some(tx);
        self.rx = Some(rx);
        self.status     = AppStatus::Analyzing;
        self.last_error = None;
        self.clean_msg  = None;

        std::thread::spawn(move || {
            let start = Instant::now();
            let result = run_git_diff(&root, &output_dir, &git_ref, &langs);
            let elapsed = start.elapsed().as_secs_f64();
            match result {
                Ok(mut r) => {
                    r.elapsed_secs = elapsed;
                    let _ = tx_clone.send(WorkerMsg::Done(r));
                }
                Err(e) => {
                    let _ = tx_clone.send(WorkerMsg::Error(e.to_string()));
                }
            }
        });
    }

    /// Supprime graph.html, graph.json et diff.html du dossier de sortie.
    fn clean_output_files(&mut self) {
        let out_dir = self.effective_output_dir();
        if !out_dir.exists() {
            self.clean_msg = Some("Dossier de sortie introuvable.".to_string());
            return;
        }

        let candidates = ["graph.html", "graph.json", "diff.html"];
        let mut deleted = 0usize;
        let mut errors: Vec<String> = Vec::new();

        for name in &candidates {
            let path = out_dir.join(name);
            if path.exists() {
                match std::fs::remove_file(&path) {
                    Ok(_) => deleted += 1,
                    Err(e) => errors.push(format!("{name}: {e}")),
                }
            }
        }

        self.clean_msg = Some(if !errors.is_empty() {
            format!("Erreur(s) : {}", errors.join(", "))
        } else if deleted == 0 {
            "Aucun fichier généré trouvé.".to_string()
        } else {
            format!("{deleted} fichier(s) supprimé(s).")
        });

        // Si les fichiers supprimés correspondent aux résultats affichés, on les efface.
        if let Some(ref mut res) = self.last_result {
            if deleted > 0 {
                res.graph_html_path = None;
                res.json_path       = None;
                res.diff_html_path  = None;
            }
        }
    }

    fn setup_watch(&mut self) {
        let root = PathBuf::from(&self.project_path);
        if !root.exists() {
            return;
        }

        let (w_tx, w_rx) = mpsc::channel::<()>();
        let w_tx_clone   = w_tx.clone();

        // On filtre les fichiers générés pour éviter une boucle infinie.
        let generated = [
            "graph.html",
            "graph.json",
            "diff.html",
            ".codegraph_cache.json",
        ];

        let mut watcher = match notify::recommended_watcher(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let is_generated = event.paths.iter().any(|p| {
                        p.file_name()
                            .map(|n| generated.iter().any(|g| n == *g))
                            .unwrap_or(false)
                    });
                    if !is_generated {
                        let _ = w_tx_clone.send(());
                    }
                }
            },
        ) {
            Ok(w) => w,
            Err(_) => return,
        };

        if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
            return;
        }

        self.watch_rx  = Some(w_rx);
        self.watch_tx  = Some(w_tx);
        self._watcher  = Some(Box::new(watcher));
    }

    fn stop_watch(&mut self) {
        self._watcher = None;
        self.watch_rx = None;
        self.watch_tx = None;
    }

    fn poll_results(&mut self) {
        if let Some(rx) = &self.rx {
            if let Ok(msg) = rx.try_recv() {
                match msg {
                    WorkerMsg::Done(result) => {
                        self.last_result = Some(result);
                        self.last_error  = None;
                        self.status      = AppStatus::Idle;
                    }
                    WorkerMsg::Error(e) => {
                        self.last_error = Some(e);
                        self.status     = AppStatus::Idle;
                    }
                }
            }
        }

        if self.watch_mode && self.status == AppStatus::Idle {
            let triggered = self
                .watch_rx
                .as_ref()
                .and_then(|rx| rx.try_recv().ok())
                .is_some();
            if triggered {
                self.start_analysis();
            }
        }
    }
}

// ── Worker functions ──────────────────────────────────────────────────────────

fn run_analysis(
    root: &Path,
    output_dir: &Path,
    langs: &[Language],
    use_cache: bool,
    focus_node: &str,
    focus_depth: usize,
) -> anyhow::Result<AnalysisResult> {
    // Crée le dossier de sortie si nécessaire.
    std::fs::create_dir_all(output_dir)?;

    let graph = parsers::analyze(root, langs, use_cache)?;

    let graph = if !focus_node.is_empty() && graph.has_node(focus_node) {
        analysis::focus(&graph, focus_node, focus_depth)
    } else {
        graph
    };

    let anal = analysis::analyze(&graph);

    let html_path = output_dir.join("graph.html");
    export::html::export_html(&graph, &html_path, Some(&anal), None, Some(root))?;

    let json_path = output_dir.join("graph.json");
    export::export_json(&graph, &json_path)?;

    Ok(AnalysisResult {
        graph,
        analysis: anal,
        elapsed_secs: 0.0,
        graph_html_path: Some(html_path),
        json_path: Some(json_path),
        diff_html_path: None,
    })
}

fn run_git_diff(
    root: &Path,
    output_dir: &Path,
    git_ref: &str,
    langs: &[Language],
) -> anyhow::Result<AnalysisResult> {
    std::fs::create_dir_all(output_dir)?;

    let graph_diff = analysis::git_diff_analyze(root, git_ref, langs)?;

    let graph = parsers::analyze(root, langs, false)?;
    let anal  = analysis::analyze(&graph);

    let diff_path = output_dir.join("diff.html");
    export::html::export_html(&graph, &diff_path, Some(&anal), Some(&graph_diff), Some(root))?;

    Ok(AnalysisResult {
        graph,
        analysis: anal,
        elapsed_secs: 0.0,
        graph_html_path: None,
        json_path: None,
        diff_html_path: Some(diff_path),
    })
}

fn open_file(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &path.to_string_lossy()])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

// ── egui UI ───────────────────────────────────────────────────────────────────

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_results();

        if self.status == AppStatus::Analyzing {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Code Graph");
            ui.separator();

            // ── Projet ────────────────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.label("Projet :");
                ui.text_edit_singleline(&mut self.project_path);
                if ui.button("Parcourir").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.project_path = folder.to_string_lossy().to_string();
                    }
                }
            });

            // ── Dossier de sortie ─────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.label("Sortie :");
                let hint = if self.output_path.trim().is_empty() {
                    "= dossier du projet"
                } else {
                    ""
                };
                ui.add(
                    egui::TextEdit::singleline(&mut self.output_path)
                        .hint_text(hint),
                );
                if ui.button("Parcourir").clicked() {
                    let start = if self.output_path.trim().is_empty() && !self.project_path.is_empty() {
                        Some(PathBuf::from(&self.project_path))
                    } else {
                        None
                    };
                    let mut dlg = rfd::FileDialog::new();
                    if let Some(p) = start {
                        dlg = dlg.set_directory(p);
                    }
                    if let Some(folder) = dlg.pick_folder() {
                        self.output_path = folder.to_string_lossy().to_string();
                    }
                }
                // Bouton Nettoyer
                let can_clean = !self.project_path.is_empty()
                    && self.status == AppStatus::Idle;
                if ui.add_enabled(can_clean, egui::Button::new("🗑 Nettoyer")).clicked() {
                    self.clean_output_files();
                }
            });

            // Retour de l'opération Nettoyer
            if let Some(ref msg) = self.clean_msg.clone() {
                let color = if msg.starts_with("Erreur") || msg.starts_with("Dossier") {
                    egui::Color32::RED
                } else {
                    egui::Color32::from_rgb(80, 200, 120)
                };
                ui.colored_label(color, msg);
            }

            ui.add_space(6.0);

            // ── Langages ──────────────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.label("Langages :");
                ui.checkbox(&mut self.use_python,     "Python");
                ui.checkbox(&mut self.use_rust,       "Rust");
                ui.checkbox(&mut self.use_cpp,        "C++");
                ui.checkbox(&mut self.use_typescript, "TypeScript");
                ui.checkbox(&mut self.use_javascript, "JavaScript");
            });

            ui.add_space(4.0);

            // ── Options ───────────────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.use_cache, "Cache");
                let watch_label = if self.watch_mode { "Watch (actif)" } else { "Watch (auto-reload)" };
                if ui.checkbox(&mut self.watch_mode, watch_label).changed() {
                    if self.watch_mode { self.setup_watch(); } else { self.stop_watch(); }
                }
            });

            ui.add_space(4.0);

            // ── Focus ─────────────────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.label("Focus :");
                ui.text_edit_singleline(&mut self.focus_node)
                    .on_hover_text("ID du nœud (ex: mymodule.MyClass) — laisser vide pour tout afficher");
            });

            ui.horizontal(|ui| {
                ui.label("Profondeur focus :");
                ui.add(egui::Slider::new(&mut self.focus_depth, 1..=5).text("niveaux"));
            });

            ui.add_space(8.0);

            // ── Bouton Analyser ───────────────────────────────────────────────
            ui.vertical_centered(|ui| {
                let analyzing = self.status == AppStatus::Analyzing;
                let btn = egui::Button::new(if analyzing { "⏳ Analyse en cours…" } else { "▶ Analyser" })
                    .min_size(egui::vec2(220.0, 34.0));
                if ui.add_enabled(!analyzing, btn).clicked() {
                    self.start_analysis();
                }
            });

            ui.add_space(8.0);
            ui.separator();

            // ── Résultats ─────────────────────────────────────────────────────
            ui.label(egui::RichText::new("─── Résultats ───").color(egui::Color32::GRAY));

            if let Some(ref err) = self.last_error.clone() {
                ui.colored_label(egui::Color32::RED, format!("Erreur : {}", err));
            }

            if let Some(ref res) = self.last_result {
                let stats = res.graph.stats();
                let anal  = &res.analysis;

                ui.label(format!(
                    "{} nœuds | {} liens | {:.2}s",
                    stats.node_count, stats.edge_count, res.elapsed_secs
                ));
                ui.label(format!(
                    "module:{} class:{} struct:{} fn:{} method:{}",
                    stats.module_count, stats.class_count, stats.struct_count,
                    stats.function_count, stats.method_count
                ));

                ui.add_space(4.0);

                let orphan_count = anal.orphan_nodes.len();
                ui.label(format!(
                    "Cycles: {} · Composantes: {} · Orphelins: {}",
                    anal.cycles.len(), anal.components.len(), orphan_count
                ));

                if let Some((top_id, top_m)) = anal
                    .metrics
                    .iter()
                    .max_by(|a, b| a.1.coupling_score.partial_cmp(&b.1.coupling_score).unwrap_or(std::cmp::Ordering::Equal))
                {
                    ui.label(format!(
                        "Top couplé : {} (↓{} ↑{} DIT:{} — {:.1}%)",
                        top_id.split('.').last().unwrap_or(top_id),
                        top_m.in_degree, top_m.out_degree, top_m.depth_of_inheritance,
                        top_m.coupling_score * 100.0,
                    ));
                }

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if let Some(ref html_path) = res.graph_html_path {
                        if ui.button("🌐 Ouvrir graph.html").clicked() {
                            open_file(html_path);
                        }
                    }
                    if let Some(ref json_path) = res.json_path {
                        if ui.button("📄 Exporter JSON").clicked() {
                            open_file(json_path);
                        }
                    }
                });
            }

            ui.add_space(12.0);
            ui.separator();

            // ── Git diff ──────────────────────────────────────────────────────
            ui.label(egui::RichText::new("Git diff vs :").color(egui::Color32::GRAY));
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.git_ref);
                let analyzing = self.status == AppStatus::Analyzing;
                if ui.add_enabled(!analyzing, egui::Button::new("Diff")).clicked() {
                    self.start_git_diff();
                }
            });

            if let Some(ref res) = self.last_result {
                if let Some(ref diff_path) = res.diff_html_path {
                    ui.add_space(4.0);
                    if ui.button("🌐 Ouvrir diff.html").clicked() {
                        open_file(diff_path);
                    }
                }
            }
        });
    }
}
