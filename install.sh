#!/bin/sh
# Install the binary, desktop entry, and icon.
# Default is a user install (~/.local, no root). System-wide: sudo PREFIX=/usr ./install.sh
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
if [ -n "${ROOT:-}" ]; then
  root=$ROOT
fi
prefix=${PREFIX:-${HOME}/.local}

if [ "${SKIP_BUILD:-}" != 1 ]; then
  cargo build --release --manifest-path "$root/Cargo.toml"
  install -Dm755 "$root/target/release/zarinotes" "$prefix/bin/zarinotes"
fi

install -Dm644 "$root/assets/zarinotes.desktop" "$prefix/share/applications/zarinotes.desktop"
install -Dm644 "$root/assets/icon.svg" "$prefix/share/icons/hicolor/scalable/apps/zarinotes.svg"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$prefix/share/applications" || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$prefix/share/icons/hicolor" || true
fi

echo "Installed ZariNotes to $prefix"
echo "The launcher entry is ZariNotes. $prefix/bin must be on PATH."
