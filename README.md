# ZariNotes

<img src="assets/icon.svg" alt="" width="88">

A minimal Markdown notes app written in Rust with [iced](https://iced.rs).

## Features

- Pick a workspace folder (native dialog via the XDG portal); the last one is reopened on launch
- Sidebar tree of folders and `.md` files (recursive, hidden entries skipped), with a search bar and a workspace header
- Right-click the sidebar. A small menu opens at the cursor with New Note or New Folder, then asks for a name. A right-click on a folder creates inside it; on a note, beside it; otherwise at the workspace root
- **Live preview**: every line is rendered as Markdown except the one
  under the cursor, which shows the raw (syntax-highlighted) source.
  Code blocks, tables and front matter switch to source as a whole.
- Clickable task checkboxes, links open notes (relative `.md` paths) or the browser,
  Enter continues lists
- Select and copy across lines in live preview (drag, shift-arrows, Ctrl+A). The selected lines show as source, so the copy is the Markdown
- **Source mode** (`Ctrl+E` or the Live / Source switch): plain monospace editor with
  Markdown highlighting for the whole note
- `Ctrl+S` to save; the footer shows Unsaved until you do. Unsaved changes are also
  auto-saved when switching notes or workspaces
- `Ctrl+Z` undoes the last change (`Ctrl+Shift+Z` or `Ctrl+Y` redoes). A run of typing is one step, in Live and in Source
- Footer shows the open note, word count, and cursor position. A `•` in the window title means unsaved changes
- Light / Dark switch in the footer (remembered). Light is Retro Classic (vintage beige and slate); dark is Dolch Noir

## Install

Linux. `./install.sh` builds the release binary and installs it, the desktop entry, and the icon under `~/.local` (no root). `~/.local/bin` must be on `PATH`. System-wide: `sudo PREFIX=/usr ./install.sh`. Log out and back in if the launcher does not show the new entry.

Nix installs that same desktop entry. Add the flake as an input:

```nix
{
  inputs.zarinotes.url = "github:ZariTen/ZariNotes";

  # NixOS
  environment.systemPackages = [
    inputs.zarinotes.packages.x86_64-linux.default
  ];

  # Home Manager
  # home.packages = [
  #   inputs.zarinotes.packages.${pkgs.stdenv.hostPlatform.system}.default
  # ];
}
```

Or, without a configuration: `nix profile install github:ZariTen/ZariNotes`. Log out and back in if the launcher does not pick up the new entry. `nix run github:ZariTen/ZariNotes` starts it without installing.

## Build & run

On NixOS / with Nix flakes:

```sh
nix develop
cargo run --release
```

Elsewhere: install a Rust toolchain (edition 2024, 1.88+) and run `cargo run --release`.