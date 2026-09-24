# ZariNotes

A minimal Markdown notes app written in Rust with [iced](https://iced.rs).

## Features

- Pick a workspace folder (native dialog via the XDG portal); the last one is reopened on launch
- Sidebar lists every `.md` file in the workspace (recursive, hidden entries skipped)
- Create notes by typing a name (`ideas`, `journal/2026-09-24`) and pressing Enter
- **Live preview**: every line is rendered as Markdown except the one
  under the cursor, which shows the raw (syntax-highlighted) source.
  Code blocks, tables and front matter switch to source as a whole.
- Clickable task checkboxes, links open notes (relative `.md` paths) or the browser,
  Enter continues lists
- **Source mode** (`Ctrl+E` or the status-bar button): plain monospace editor with
  Markdown highlighting — use it for multi-line selections
- `Ctrl+S` to save; unsaved changes are auto-saved when switching notes/workspaces
- Dirty marker (`•`) in the window title, cursor position in the status bar

## Build & run

On NixOS / with Nix flakes:

```sh
nix develop
cargo run --release
```

Elsewhere: install a Rust toolchain (edition 2024, 1.88+) and run `cargo run --release`.