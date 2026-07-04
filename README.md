# Notypo

Notypo is a macOS Markdown editor built around a Typora-style live preview experience. It embeds the TypeMark editor UI in a native Rust/AppKit shell, so writing, previewing, opening, and saving Markdown files happen in one focused desktop window.

## Product Features

- Live Preview Markdown editing: Markdown syntax is rendered in place while you write, keeping the document readable without switching between edit and preview panes.
- Standard document workflow: create new documents, open existing Markdown files, save, and save as from the native macOS menu.
- Native macOS window and menu integration: common shortcuts for New, Open, Save, Undo, Redo, Cut, Copy, Paste, and Select All are wired through the app menu, and the seamless titlebar can be dragged to move the window.
- Sidebar table of contents: the sidebar can show a document outline generated from headings, and `View -> Toggle Outline` toggles it quickly.
- Sidebar file navigation: the sidebar also includes file list / file tree views for navigating nearby files. The top-left sidebar button switches between TOC and file navigation.
- Search and replace UI: document search supports normal search controls plus case-sensitive, whole-word, and regular-expression options.
- File search UI: the file sidebar includes search controls for finding files and matches in the current folder context.
- Markdown extras: the bundled editor assets include support for tables, task lists, code fences, math, images, and diagrams such as Mermaid.
- Theme support: Notypo uses the bundled TypeMark themes and follows macOS appearance, using a light theme in normal mode and a dark theme in Dark Mode.
- Local-first editing: documents are loaded from and saved to local files; no account or cloud service is required.

## Current Scope

Notypo currently targets macOS. The app is intentionally small: it focuses on Markdown editing, local files, the document outline, and the file sidebar rather than publishing or collaboration features.

## Build and Run

```bash
cargo run
```

The editor assets are bundled under `assets/TypeMark`, and the native host is implemented in `src/main.rs`.
