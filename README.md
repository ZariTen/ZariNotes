# ZariNotes

<img src="assets/icon.svg" alt="" width="88">

ZariNotes is an offline Markdown writer. It is free software, written in Rust with [iced](https://iced.rs). Notes stay in a folder on your computer. The license is GPL-3.0-or-later.

Paste an image into a note with Ctrl+V. It is saved in a hidden `.images` folder next to that note, and live preview draws it at 480×320. On Wayland this needs `wl-paste` (`wl-clipboard`); on X11, `xclip`. A Nix install includes both.

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
