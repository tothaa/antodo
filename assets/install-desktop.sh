#!/bin/sh
# Install the launcher and icon needed by Linux/Wayland taskbars.
set -eu
asset_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
data_dir=${XDG_DATA_HOME:-"$HOME/.local/share"}
install -Dm644 "$asset_dir/antodo.desktop" "$data_dir/applications/antodo.desktop"
install -Dm644 "$asset_dir/antodo.svg" "$data_dir/icons/hicolor/scalable/apps/antodo.svg"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$data_dir/applications"
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$data_dir/icons/hicolor" >/dev/null 2>&1 || :
fi
