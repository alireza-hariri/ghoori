#!/usr/bin/env bash
# Install script for Ghoori.
#
# Usage:
#   ./install.sh              build (if needed) and install to ~/.local
#   sudo ./install.sh         install system-wide to /usr/local
#   PREFIX=/opt/ghoori ./install.sh   custom prefix
#   ./install.sh --uninstall  remove the installed files
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
if [[ ${EUID:-$(id -u)} -eq 0 && $PREFIX == "$HOME/.local" ]]; then
    PREFIX="/usr/local"
fi

BINDIR="$PREFIX/bin"
APPDIR="$PREFIX/share/applications"
ICONDATADIR="$PREFIX/share/icons/hicolor"

# MIME types handled by ghoori (kept in sync with ghoori.desktop)
MIME_TYPES=(
    video/mp4 video/x-matroska video/webm video/quicktime video/x-msvideo
    video/mpeg video/ogg video/x-flv video/3gpp
    audio/mpeg audio/flac audio/ogg audio/x-wav audio/aac audio/mp4
    audio/opus audio/x-matroska application/x-mpegurl
    audio/x-mpegurl audio/x-scpls
)

# Write mime=ghoori.desktop into [Default Applications] in mimeapps.list.
# Done directly instead of via xdg-mime: on some distros (e.g. Arch) xdg-mime
# calls qtpaths for KDE detection and spams 'qtpaths: command not found'.
set_default_mime() {
    local mime="$1" desktop="ghoori.desktop"
    local conf="${XDG_CONFIG_HOME:-$HOME/.config}/mimeapps.list"
    mkdir -p "$(dirname "$conf")"
    touch "$conf"
    awk -v mime="$mime" -v desk="$desktop" '
        BEGIN { indef = 0; found = 0; printed = 0; hadsection = 0; hadcontent = 0 }
        /^[ \t]*\[/ {
            if (indef && !found) { print mime "=" desk; printed = 1 }
            indef = ($0 ~ /^[ \t]*\[Default Applications\]/)
            if (indef) { hadsection = 1; found = 0 }
            print
            next
        }
        indef && index($0, mime "=") == 1 {
            if (!found) { print mime "=" desk; found = 1; printed = 1 }
            next   # drop duplicate/old entries for this mime
        }
        /[^ \t]/ { hadcontent = 1 }
        { print }
        END {
            if (!printed) {
                if (indef) {
                    print mime "=" desk
                } else {
                    if (hadcontent && !hadsection) print ""
                    if (!hadsection) print "[Default Applications]"
                    print mime "=" desk
                }
            }
        }
    ' "$conf" > "$conf.tmp" && mv "$conf.tmp" "$conf"
}

uninstall() {
    local files=("$BINDIR/ghoori" "$APPDIR/ghoori.desktop")
    local sizes=("/scalable/apps/ghoori.svg" "/48x48/apps/ghoori.png" "/256x256/apps/ghoori.png")
    for s in "${sizes[@]}"; do
        files+=("$ICONDATADIR$s")
    done
    local removed=0
    for f in "${files[@]}"; do
        if [[ -e $f ]]; then
            rm -v "$f"
            removed=1
        fi
    done
    if [[ $removed -eq 1 ]]; then
        command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPDIR" 2>/dev/null || true
        command -v gtk-update-icon-cache  >/dev/null 2>&1 && gtk-update-icon-cache -qtf "$ICONDATADIR" 2>/dev/null || true
        echo "Ghoori uninstalled."
        echo "Note: MIME defaults in ~/.config/mimeapps.list were left untouched;"
        echo "remove the 'ghoori.desktop' lines there if you want them gone."
    else
        echo "Ghoori is not installed in $PREFIX."
    fi
    exit 0
}

[[ ${1:-} == "--uninstall" ]] && uninstall

cd "$(dirname "$0")"

# --- Build ---------------------------------------------------------------
if [[ ! -x target/release/ghoori ]]; then
    echo "==> Building (release)..."
    cargo build --release
fi

# --- Install binary -------------------------------------------------------
echo "==> Installing binary to $BINDIR"
install -d "$BINDIR"
install -m 755 target/release/ghoori "$BINDIR/ghoori"

# --- Install desktop entry ------------------------------------------------
# Exec must be an absolute path or a PATH command; use the installed binary
# path so the launcher works even if ~/.local/bin is not on PATH.
echo "==> Installing desktop entry to $APPDIR"
install -d "$APPDIR"
sed "s|^Exec=ghoori %U|Exec=$BINDIR/ghoori %U|" ghoori.desktop > "$APPDIR/ghoori.desktop"
chmod 644 "$APPDIR/ghoori.desktop"

# --- Install icon (optional) ----------------------------------------------
for icon in icons/ghoori.svg icons/ghoori.png icons/ghoori-256.png; do
    [[ -e $icon ]] || continue
    case "$icon" in
        *.svg) dir="$ICONDATADIR/scalable/apps" ;;
        *-256.png) dir="$ICONDATADIR/256x256/apps" ;;
        *.png) dir="$ICONDATADIR/48x48/apps" ;;
    esac
    echo "==> Installing icon to $dir"
    install -d "$dir"
    install -m 644 "$icon" "$dir/ghoori.${icon##*.}"
done

# --- Refresh caches --------------------------------------------------------
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPDIR" || true
command -v gtk-update-icon-cache  >/dev/null 2>&1 && gtk-update-icon-cache -qtf "$ICONDATADIR" 2>/dev/null || true

# --- Offer to make it the default player -----------------------------------
echo
read -rp "Set Ghoori as the default player for all supported media types? [y/N] " answer
if [[ ${answer,,} == y* ]]; then
    for m in "${MIME_TYPES[@]}"; do
        set_default_mime "$m"
    done
    echo "==> Defaults written to ~/.config/mimeapps.list"
else
    echo "==> Skipping. You can do it later with:"
    echo "    xdg-mime default ghoori.desktop video/mp4"
fi

echo
echo "Done. Run 'ghoori' or pick it from your application launcher."
