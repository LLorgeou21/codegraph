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
            .with_inner_size([680.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Code Graph",
        options,
        Box::new(|_cc| Ok(Box::new(AppState::default()))),
    )
}

// ── Worker messages ──────────────────────────────────────────────────────────

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

// ── App state machine ────────────────────────────────────────────────────────

#[derive(Default, PartialEq)]
enum AppStatus {
    #[default]
    Idle,
    Analyzing,
}

struct AppState {
    // UI inputs
    project_path: String,
    use_python: bool,
    use_rust: bool,
    use_cpp: bool,
    use_cache: bool,
    watch_mode: bool,
    focus_node: String,
    git_ref: String,

    // State
    status: AppStatus,
    tx: Option<Sender<WorkerMsg>>,
    rx: Option<Receiver<WorkerMsg>>,

    // Results
    last_result: Option<AnalysisResult>,
    last_error: Option<String>,

    // Watch
    _watcher: Option<Box<dyn notify::Watcher + Send>>,
    watch_tx: Option<Sender<()>>,
    watch_rx: Option<Receiver<()>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            project_path: String::new(),
            use_python: true,
            use_rust: true,
            use_cpp: false,
            use_cache: true,
            watch_mode: false,
            focus_node: String::new(),
            git_ref: "main".to_string(),
            status: AppStatus::Idle,
            tx: None,
            rx: None,
            last_result: None,
            last_error: None,
            _watcher: None,
            watch_tx: None,
            watch_rx: None,
        }
    }
}

impl AppState {
    fn selected_languages(&self) -> Vec<Language> {
        let mut langs = Vec::new();
        if self.use_python {
            langs.push(Language::Python);
        }
        if self.use_rust {
            langs.push(Language::Rust);
        }
        if self.use_cpp {
            langs.push(Language::Cpp);
        }
        langs
    }

    fn start_analysis(&mut self) {
        if self.project_path.is_empty() {
            self.last_error = Some("Chemin du projet vide.".to_string());
            return;
        }
        let root = PathBuf::from(&self.project_path);
        if !root.exists() {
            self.last_error = Some(format!("Chemin inexistant: {}", root.display()));
            return;
        }

        let langs = self.selected_languages();
        if langs.is_empty() {
            self.last_error = Some("Sélectionnez au moins un langage.".to_string());
            return;
        }

        let use_cache = self.use_cache;
        let focus_node = self.focus_node.trim().to_string();

        let (tx, rx) = mpsc::channel::<WorkerMsg>();
        let tx_clone = tx.clone();
        self.tx = Some(tx);
        self.rx = Some(rx);
        self.status = AppStatus::Analyzing;
        self.last_error = None;

        let root_clone = root.clone();
        std::thread::spawn(move || {
            let start = Instant::now();
            match run_analysis(&root_clone, &langs, use_cache, &focus_node) {
                Ok(result) => {
                    let _ = tx_clone.send(WorkerMsg::Done(result));
                }
                Err(e) => {
                    let _ = tx_clone.send(WorkerMsg::Error(e.to_string()));
                }
            }
            let _ = start;
        });
    }

    fn start_git_diff(&mut self) {
        if self.project_path.is_empty() || self.git_ref.is_empty() {
            return;
        }
        let root = PathBuf::from(&self.project_path);
        let git_ref = self.git_ref.clone();
        let langs = self.selected_languages();

        let (tx, rx) = mpsc::channel::<WorkerMsg>();
        let tx_clone = tx.clone();
        self.tx = Some(tx);
        self.rx = Some(rx);
        self.status = AppStatus::Analyzing;
        self.last_error = None;

        std::thread::spawn(move || {
            let start = Instant::now();
            match run_git_diff(&root, &git_ref, &langs) {
                Ok(result) => {
                    let _ = tx_clone.send(WorkerMsg::Done(result));
                }
                Err(e) => {
                    let _ = tx_clone.send(WorkerMsg::Error(e.to_string()));
                }
            }
            let _ = start;
        });
    }

    fn setup_watch(&mut self) {
        let root = PathBuf::from(&self.project_path);
        if !root.exists() {
            return;
        }

        let (w_tx, w_rx) = mpsc::channel::<()>();
        let w_tx_clone = w_tx.clone();

        let mut watcher = match notify::recommended_watcher(
            move |res: Result<Event, notify::Error>| {
                if res.is_ok() {
                    let _ = w_tx_clone.send(());
                }
            },
        ) {
            Ok(w) => w,
            Err(_) => return,
        };

        if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
            return;
        }

        self.watch_rx = Some(w_rx);
        self.watch_tx = Some(w_tx);
        self._watcher = Some(Box::new(watcher));
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
                        self.last_error = None;
                        self.status = AppStatus::Idle;
                    }
                    WorkerMsg::Error(e) => {
                        self.last_error = Some(e);
                        self.status = AppStatus::Idle;
                    }
                }
            }
        }

        // Watch mode
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

// ── Background worker functions ──────────────────────────────────────────────

fn run_analysis(
    root: &Path,
    langs: &[Language],
    use_cache: bool,
    focus_node: &str,
) -> anyhow::Result<AnalysisResult> {
    let start = Instant::now();

    let graph = parsers::analyze(root, langs, use_cache)?;

    let graph = if !focus_node.is_empty() && graph.has_node(focus_node) {
        analysis::focus(&graph, focus_node, 2)
    } else {
        graph
    };

    let anal = analysis::analyze(&graph);

    // Export HTML
    let html_path = root.join("graph.html");
    export::html::export_html(&graph, &html_path, Some(&anal), None, Some(root))?;

    // Export JSON
    let json_path = root.join("graph.json");
    export::export_json(&graph, &json_path)?;

    let elapsed = start.elapsed().as_secs_f64();

    Ok(AnalysisResult {
        graph,
        analysis: anal,
        elapsed_secs: elapsed,
        graph_html_path: Some(html_path),
        json_path: Some(json_path),
        diff_html_path: None,
    })
}

fn run_git_diff(
    root: &Path,
    git_ref: &str,
    langs: &[Language],
) -> anyhow::Result<AnalysisResult> {
    let start = Instant::now();

    let graph_diff = analysis::git_diff_analyze(root, git_ref, langs)?;

    // Build current graph for display
    let graph = parsers::analyze(root, langs, false)?;
    let anal = analysis::analyze(&graph);

    // Export diff HTML
    let diff_path = root.join("diff.html");
    export::html::export_html(&graph, &diff_path, Some(&anal), Some(&graph_diff), Some(root))?;

    let elapsed = start.elapsed().as_secs_f64();

    Ok(AnalysisResult {
        graph,
        analysis: anal,
        elapsed_secs: elapsed,
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
        let _ = std::process::Command::new("open")
            .arg(path)
            .spawn();
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(path)
            .spawn();
    }
}

// ── egui UI ─────────────────────────────────────────────────────────────────

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_results();

        // Request continuous repaints while analyzing
        if self.status == AppStatus::Analyzing {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Code Graph");
            ui.separator();

            // Project path
            ui.horizontal(|ui| {
                ui.label("Projet :");
                ui.text_edit_singleline(&mut self.project_path);
                if ui.button("Parcourir").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.project_path = folder.to_string_lossy().to_string();
                    }
                }
            });

            ui.add_space(6.0);

            // Languages
            ui.horizontal(|ui| {
                ui.label("Langages :");
                ui.checkbox(&mut self.use_python, "Python");
                ui.checkbox(&mut self.use_rust, "Rust");
                ui.checkbox(&mut self.use_cpp, "C++");
            });

            ui.add_space(4.0);

            // Options
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.use_cache, "Cache");
                let watch_label = if self.watch_mode {
                    "Watch (actif)"
                } else {
                    "Watch (auto-reload)"
                };
                if ui.checkbox(&mut self.watch_mode, watch_label).changed() {
                    if self.watch_mode {
                        self.setup_watch();
                    } else {
                        self.stop_watch();
                    }
                }
            });

            ui.add_space(4.0);

            // Focus
            ui.horizontal(|ui| {
                ui.label("Focus :");
                ui.text_edit_singleline(&mut self.focus_node)
                    .on_hover_text("ID de noeud optionnel (ex: mymodule.MyClass)");
            });

            ui.add_space(8.0);

            // Analyse button
            ui.vertical_centered(|ui| {
                let analyzing = self.status == AppStatus::Analyzing;
                let btn = egui::Button::new(if analyzing {
                    "⏳ Analyse en cours…"
                } else {
                    "Analyser"
                })
                .min_size(egui::vec2(200.0, 32.0));

                if ui.add_enabled(!analyzing, btn).clicked() {
                    self.start_analysis();
                }
            });

            ui.add_space(8.0);
            ui.separator();

            // Results
            ui.label(egui::RichText::new("─── Résultats ───").color(egui::Color32::GRAY));

            if let Some(ref err) = self.last_error.clone() {
                ui.colored_label(egui::Color32::RED, format!("Erreur: {}", err));
            }

            if let Some(ref res) = self.last_result {
                let stats = res.graph.stats();
                let anal = &res.analysis;

                ui.label(format!(
                    "{} noeuds | {} liens | {:.2}s",
                    stats.node_count, stats.edge_count, res.elapsed_secs
                ));
                ui.label(format!(
                    "module:{} class:{} function:{} method:{}",
                    stats.module_count, stats.class_count, stats.function_count, stats.method_count
                ));

                ui.add_space(4.0);

                ui.label(format!(
                    "Cycles: {}   Composantes: {}",
                    anal.cycles.len(),
                    anal.components.len()
                ));

                // Top coupled
                let mut sorted: Vec<(&String, f32)> = anal
                    .metrics
                    .iter()
                    .map(|(id, m)| (id, m.coupling_score))
                    .collect();
                sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                if let Some((top_id, top_score)) = sorted.first() {
                    ui.label(format!(
                        "Top couplé: {} (score: {:.2})",
                        top_id.split('.').last().unwrap_or(top_id),
                        top_score
                    ));
                }

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if let Some(ref html_path) = res.graph_html_path {
                        if ui.button("Ouvrir graph.html").clicked() {
                            open_file(html_path);
                        }
                    }
                    if let Some(ref json_path) = res.json_path {
                        if ui.button("Exporter JSON").clicked() {
                            open_file(json_path);
                        }
                    }
                });
            }

            ui.add_space(12.0);
            ui.separator();

            // Git diff section
            ui.label(egui::RichText::new("Git diff vs:").color(egui::Color32::GRAY));
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
                    if ui.button("Ouvrir diff.html").clicked() {
                        open_file(diff_path);
                    }
                }
            }
        });
    }
}
