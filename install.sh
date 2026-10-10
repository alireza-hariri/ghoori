#!/usr/bin/env bash
# Install script for Ghoori. Works both from a source checkout (builds with cargo) and from an extracted
# release archive (installs the prebuilt binary next to this script).
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: ./install.sh [OPTIONS]

Installs the ghoori binary, its launcher entry and icons.

Options:
  --prefix DIR           Install under DIR (default: ~/.local, or /usr/local when run as root)
  --default-player       Make Ghoori the default player for audio/video files (no prompt)
  --no-default-player    Don't ask about the default player
  --uninstall            Remove an installed Ghoori
  -h, --help             Show this help

Examples:
  ./install.sh                     install for the current user
  sudo ./install.sh                install system-wide (from a source checkout, build first:
                                   cargo build --release)
  ./install.sh --uninstall         uninstall
EOF
}

PREFIX="${PREFIX:-}"
DEFAULT_PLAYER=ask
ACTION=install
while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix) PREFIX="${2:?--prefix needs a directory}"; shift ;;
        --prefix=*) PREFIX="${1#*=}" ;;
        --default-player) DEFAULT_PLAYER=yes ;;
        --no-default-player) DEFAULT_PLAYER=no ;;
        --uninstall) ACTION=uninstall ;;
        -h|--help) usage; exit 0 ;;
        *) echo "install.sh: unknown option: $1" >&2; echo >&2; usage >&2; exit 2 ;;
    esac
    shift
done

IS_ROOT=0
[[ ${EUID:-$(id -u)} -eq 0 ]] && IS_ROOT=1
SELFDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -z $PREFIX ]]; then
    if [[ $SELFDIR == */share/ghoori ]]; then
        PREFIX="${SELFDIR%/share/ghoori}"    # the copy installed for uninstalling
    elif [[ $IS_ROOT -eq 1 ]]; then PREFIX=/usr/local; else PREFIX="$HOME/.local"; fi
fi

BINDIR="$PREFIX/bin"
APPDIR="$PREFIX/share/applications"
ICONDATADIR="$PREFIX/share/icons/hicolor"
DATADIR="$PREFIX/share/ghoori"

if [[ -t 1 ]]; then
    BOLD=$'\e[1m' RED=$'\e[31m' YELLOW=$'\e[33m' RESET=$'\e[0m'
else
    BOLD='' RED='' YELLOW='' RESET=''
fi
step() { echo "${BOLD}==>${RESET} $*"; }
warn() { echo "${YELLOW}warning:${RESET} $*" >&2; }
die() { echo "${RED}error:${RESET} $*" >&2; exit 1; }

# MIME types handled by ghoori (kept in sync with ghoori.desktop)
MIME_TYPES=(
    video/mp4 video/x-matroska video/webm video/quicktime video/x-msvideo
    video/mpeg video/ogg video/x-flv video/3gpp
    audio/mpeg audio/flac audio/ogg audio/x-wav audio/aac audio/mp4
    audio/opus audio/x-matroska application/x-mpegurl
    audio/x-mpegurl audio/x-scpls
)

# --- Distro-specific package hints ------------------------------------------
distro_family() {
    local id="" like=""
    if [[ -r /etc/os-release ]]; then
        # shellcheck source=/dev/null
        id=$(. /etc/os-release && echo "${ID:-}")
        # shellcheck source=/dev/null
        like=$(. /etc/os-release && echo "${ID_LIKE:-}")
    fi
    case " $id $like " in
        *" arch "*) echo arch ;;
        *" debian "*|*" ubuntu "*) echo debian ;;
        *" fedora "*|*" rhel "*) echo fedora ;;
        *" suse "*|*" opensuse "*) echo suse ;;
        *) echo unknown ;;
    esac
}

runtime_hint() {
    case "$(distro_family)" in
        arch) echo "    sudo pacman -S mpv" ;;
        debian) echo "    sudo apt install libmpv2" ;;
        fedora) echo "    sudo dnf install mpv-libs" ;;
        suse) echo "    sudo zypper install libmpv2" ;;
        *) echo "    Install libmpv (mpv 0.38 or newer) with your package manager." ;;
    esac
}

build_hint() {
    case "$(distro_family)" in
        arch) echo "    sudo pacman -S --needed rust cmake clang mpv wayland libxkbcommon libx11 libxrandr libxinerama libxcursor libxi" ;;
        debian) echo "    sudo apt install build-essential cmake clang pkg-config libmpv-dev libwayland-dev libxkbcommon-dev"
                echo "                     wayland-protocols libx11-dev libxrandr-dev libxinerama-dev libxcursor-dev libxi-dev libgl-dev" ;;
        fedora) echo "    sudo dnf install gcc cmake clang mpv-devel wayland-devel libxkbcommon-devel libX11-devel"
                echo "                     libXrandr-devel libXinerama-devel libXcursor-devel libXi-devel mesa-libGL-devel" ;;
        suse) echo "    sudo zypper install gcc cmake clang mpv-devel wayland-devel libxkbcommon-devel libX11-devel"
              echo "                        libXrandr-devel libXinerama-devel libXcursor-devel libXi-devel Mesa-libGL-devel" ;;
        *) echo "    A C toolchain, CMake, clang, libmpv headers and the Wayland/X11 development libraries." ;;
    esac
}

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

refresh_caches() {
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q "$APPDIR" 2>/dev/null || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -qtf "$ICONDATADIR" 2>/dev/null || true
    fi
}

uninstall() {
    local files=(
        "$BINDIR/ghoori" "$APPDIR/ghoori.desktop" "$DATADIR/install.sh"
        "$ICONDATADIR/scalable/apps/ghoori.svg" "$ICONDATADIR/48x48/apps/ghoori.png"
        "$ICONDATADIR/256x256/apps/ghoori.png"
    )
    local removed=0
    for f in "${files[@]}"; do
        if [[ -e $f ]]; then
            rm -v "$f"
            removed=1
        fi
    done
    rmdir "$DATADIR" 2>/dev/null || true
    if [[ $removed -eq 0 ]]; then
        echo "Ghoori is not installed in $PREFIX."
        [[ $IS_ROOT -eq 0 ]] && echo "If you installed it system-wide, run: sudo $0 --uninstall"
        exit 0
    fi
    refresh_caches
    echo "Ghoori uninstalled."
    echo "Your settings and resume positions are kept in ~/.config/ghoori and ~/.local/state/ghoori."
    echo "Default-player entries in ~/.config/mimeapps.list were left untouched;"
    echo "remove the 'ghoori.desktop' lines there if you want them gone."
    exit 0
}

[[ $ACTION == uninstall ]] && uninstall

SRCDIR="$SELFDIR"
cd "$SRCDIR"

set_default_player() {
    for m in "${MIME_TYPES[@]}"; do
        set_default_mime "$m"
    done
    step "Ghoori is now the default player (~/.config/mimeapps.list)"
}

# The copy kept in $DATADIR can't reinstall, but it can still set the default player per user.
if [[ ! -f Cargo.toml && ! -x ghoori && $DEFAULT_PLAYER == yes && $IS_ROOT -eq 0 ]]; then
    set_default_player
    exit 0
fi

# --- Locate or build the binary ---------------------------------------------
if [[ -f ghoori && -x ghoori && ! -f Cargo.toml ]]; then
    BIN="$SRCDIR/ghoori"    # release archive
elif [[ -f Cargo.toml ]]; then
    BIN="$SRCDIR/target/release/ghoori"
    if [[ $IS_ROOT -eq 1 ]]; then
        # Never build as root: it leaves root-owned files in target/ and root rarely has a Rust toolchain.
        [[ -x $BIN ]] || die "build first as your normal user: cargo build --release, then re-run: sudo $0"
        if [[ -n $(find src Cargo.toml Cargo.lock build.rs -newer "$BIN" 2>/dev/null | head -1) ]]; then
            warn "the sources are newer than $BIN; run 'cargo build --release' as your user to install the latest version"
        fi
    else
        if ! command -v cargo >/dev/null 2>&1; then
            echo "${RED}error:${RESET} cargo (Rust) was not found. Install it from https://rustup.rs:" >&2
            echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" >&2
            echo "and make sure these build dependencies are installed:" >&2
            build_hint >&2
            echo "Or download a prebuilt release: https://github.com/alireza-hariri/ghoori/releases/latest" >&2
            exit 1
        fi
        step "Building (release)..."
        if ! cargo build --release --locked; then
            echo >&2
            echo "${RED}error:${RESET} the build failed. Make sure the build dependencies are installed:" >&2
            build_hint >&2
            exit 1
        fi
    fi
else
    die "no ghoori binary or Cargo.toml next to this script; run it from the source tree or the extracted release"
fi

# --- Check runtime libraries -------------------------------------------------
if command -v ldd >/dev/null 2>&1; then
    missing=$(ldd "$BIN" 2>/dev/null | awk '/not found/ { print $1 }' || true)
    if [[ -n $missing ]]; then
        echo "${RED}error:${RESET} missing libraries: ${missing//$'\n'/ }" >&2
        echo "Install libmpv:" >&2
        runtime_hint >&2
        exit 1
    fi
fi

# --- Install ------------------------------------------------------------------
step "Installing binary to $BINDIR"
install -Dm755 "$BIN" "$BINDIR/ghoori"

# Exec must be an absolute path or a PATH command; use the installed binary
# path so the launcher works even if ~/.local/bin is not on PATH.
step "Installing launcher entry to $APPDIR"
install -d "$APPDIR"
sed "s|^Exec=ghoori %U|Exec=$BINDIR/ghoori %U|" ghoori.desktop > "$APPDIR/ghoori.desktop"
chmod 644 "$APPDIR/ghoori.desktop"

step "Installing icons to $ICONDATADIR"
install -Dm644 icons/ghoori.svg "$ICONDATADIR/scalable/apps/ghoori.svg"
install -Dm644 icons/ghoori.png "$ICONDATADIR/48x48/apps/ghoori.png"
install -Dm644 icons/ghoori-256.png "$ICONDATADIR/256x256/apps/ghoori.png"

# Keep a copy of this script so uninstalling doesn't need the source tree or archive.
install -Dm755 "$SRCDIR/install.sh" "$DATADIR/install.sh"

refresh_caches

# --- Default player -------------------------------------------------------------
if [[ $IS_ROOT -eq 1 ]]; then
    [[ $DEFAULT_PLAYER == yes ]] && warn "--default-player is per user; run it without sudo: $DATADIR/install.sh --default-player"
    DEFAULT_PLAYER=no
elif [[ $DEFAULT_PLAYER == ask ]]; then
    DEFAULT_PLAYER=no
    if [[ -t 0 ]]; then
        echo
        read -rp "Make Ghoori the default player for audio and video files? [y/N] " answer
        [[ ${answer,,} == y* ]] && DEFAULT_PLAYER=yes
    fi
fi
[[ $DEFAULT_PLAYER == yes ]] && set_default_player

echo
echo "${BOLD}Ghoori $("$BINDIR/ghoori" --version 2>/dev/null | awk '{ print $2 }') installed.${RESET}"
echo "Start it from your application menu, or run: ghoori [FILE|URL|DIR]..."
case ":$PATH:" in
    *":$BINDIR:"*) ;;
    *) echo
       warn "$BINDIR is not on your PATH, so the 'ghoori' command won't be found in a terminal."
       echo "Add this line to your ~/.bashrc or ~/.zshrc:"
       echo "    export PATH=\"$BINDIR:\$PATH\"" ;;
esac
echo "To uninstall: $DATADIR/install.sh --uninstall"
