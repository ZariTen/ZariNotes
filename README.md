# ZariNotes

A minimal Markdown notes app written in Rust with [iced](https://iced.rs).

## Features

- Pick a workspace folder (native dialog via the XDG portal); the last one is reopened on launch
- Sidebar tree of folders and `.md` files (recursive, hidden entries skipped), with a note filter and a workspace header
- Create notes by typing a name (`ideas`, `journal/2026-09-24`) and pressing Enter or Add
- **Live preview**: every line is rendered as Markdown except the one
  under the cursor, which shows the raw (syntax-highlighted) source.
  Code blocks, tables and front matter switch to source as a whole.
- Clickable task checkboxes, links open notes (relative `.md` paths) or the browser,
  Enter continues lists
- **Source mode** (`Ctrl+E` or the Live / Source switch): plain monospace editor with
  Markdown highlighting — use it for multi-line selections
- `Ctrl+S` to save; the footer shows Unsaved until you do. Unsaved changes are also
  auto-saved when switching notes or workspaces
- Footer shows the open note, word count, and cursor position. A `•` in the window title means unsaved changes

## Build & run

On NixOS / with Nix flakes:

```sh
nix develop
cargo run --release
```

Elsewhere: install a Rust toolchain (edition 2024, 1.88+) and run `cargo run --release`.