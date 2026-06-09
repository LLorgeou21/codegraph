# codegraph

> Multi-language static analyzer that builds and visualizes dependency graphs across Python, Rust and C++ codebases.

![Graph of this codebase](docs/codegraph_self.png)

---

## What it does

**codegraph** parses your source tree, resolves cross-file and cross-language dependencies, and produces a single self-contained HTML file with an interactive graph of everything:

- **Nodes** — modules, classes, functions, methods, constants
- **Edges** — calls, imports, inheritance, type usage, containment

---

## Features

- Supports **Python**, **Rust** and **C++**
- Resolves imports, aliases, method calls and type annotations across files
- Interactive HTML visualization powered by [vis.js](https://visjs.org/)
  - Force-directed layout, hierarchical, cluster and component views
  - Node size proportional to connection count
  - File-tree navigation panel to drill into sub-folders
  - Right-click to hide a node and its edges
  - PNG export with title and legend
  - Settings panel (node size, repulsion, link attraction, simulation speed)
- Incremental cache — unchanged files are skipped on re-run
- Diff mode — highlights added / removed nodes between two runs

---

## Installation

Requires [Rust](https://rustup.rs/) (stable).

```bash
git clone https://github.com/you/codegraph
cd codegraph
cargo build --release
```

---

## Usage

```bash
# Analyze a directory (auto-detects Python / Rust / C++)
cargo run --release -- path/to/your/project

# Specify languages explicitly
cargo run --release -- path/to/your/project --lang rust,python

# Output to a custom file
cargo run --release -- path/to/your/project -o graph.html
```

Then open the generated `output.html` in your browser.

---

## Architecture

```
crates/
├── core/       — graph data model (nodes, edges, serialization)
├── parsers/    — language parsers + dependency resolver
│   ├── rust_lang.rs
│   ├── python.rs
│   └── cpp.rs
├── export/     — HTML template + vis.js rendering
└── app/        — CLI entry point
```

---

## License

MIT
