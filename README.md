<div align="center">
<img src="icons/ghoori-256.png" width="128" alt="Ghoori logo">

# Ghoori

A media player for Linux, written in Rust.
</div>

Ghoori plays anything mpv plays — local files, playlists, network streams — with a clean, fast interface on top.
Under the hood, mpv through [libmpv2-rs](https://crates.io/crates/libmpv2) decodes and renders
video through its OpenGL render API, while [raylib](https://github.com/raylib-rs/raylib-rs) owns the window and
draws the entire interface.

Vibe-coded: most of it was written with an LLM, then reviewed and hammered on by hand until it worked properly.

## Features

- Plays anything mpv plays: local files, folders, playlists (`m3u`, `pls`, `cue`), network streams and URLs
- Hardware decoding with zero-copy VAAPI on Wayland (EGL). NVIDIA (NVDEC/CUDA) is used only when the dGPU is
  already powered on, so hybrid laptops never wake or stall on a sleeping GPU
- Auto-hiding controls: seek bar with buffered range, chapter marks, A-B loop range and hover time/chapter tooltip
- Seek-bar preview thumbnails rendered by a second, muted mpv instance
- Playlist panel: click to play, drag to reorder, remove, shuffle, repeat, clear; drop files onto it to append
- Subtitle and audio track menus, external subtitle loading, subtitle delay, subtitles lifted above the controls
- Playback speed menu, frame stepping, chapter navigation, A-B loop, repeat file
- Built-in file browser (type to filter, natural sort, quick places, "play folder"), URL/path dialog, clipboard paste
- Drag & drop of files, folders and subtitle files
- Resumes where you left off; remembers volume, mute and recent files (shown on the start screen)
- Audio-only view with title/artist/album; cover art is shown when the file has it
- Statistics overlay (codec, resolution, fps, decoder, dropped frames, bitrates, cache)
- Persian/Arabic titles shaped and ordered correctly
- Right-click context menu and full keyboard control (press **F1** in the player)

## Install

Ghoori needs **libmpv** (mpv 0.35 or newer). It is usually already installed if you have mpv. The installer checks for it
and tells you the exact command for your distro if it is missing.

| Distro | Install libmpv |
| --- | --- |
| Arch, Manjaro, EndeavourOS | `sudo pacman -S mpv` |
| Debian 12+, Ubuntu 23.04+, Mint 22+ | `sudo apt install libmpv2` |
| Fedora | `sudo dnf install mpv-libs` (enable [RPM Fusion](https://rpmfusion.org) for all codecs) |
| openSUSE Tumbleweed | `sudo zypper install libmpv2` |

Ubuntu 22.04 and Debian 11 ship an older libmpv that is not supported.

### Option 1: prebuilt binary (any distro, x86_64)

```sh
curl -fsSL https://github.com/alireza-hariri/ghoori/releases/latest/download/ghoori-x86_64-linux.tar.gz | tar xz
./ghoori-x86_64-linux/install.sh
```

This installs Ghoori for your user into `~/.local` and adds it to your application menu. It asks whether Ghoori should
become your default video and audio player. Use `sudo ./ghoori-x86_64-linux/install.sh` to install for all users into
`/usr/local` instead. Afterwards the `ghoori-x86_64-linux` folder can be deleted.

### Option 2: Arch Linux package

```sh
git clone https://github.com/alireza-hariri/ghoori.git
cd ghoori/packaging/arch
makepkg -si
```

This builds the latest code from GitHub into a normal pacman package (`ghoori-git`), so `pacman -R ghoori-git`
removes it. Re-run `makepkg -si` in the same folder to update.

### Option 3: build from source

You need Rust 1.88 or newer. Most distros ship an older `rustc`, so get it from [rustup.rs](https://rustup.rs). You also
need these build dependencies:

| Distro | Build dependencies |
| --- | --- |
| Arch | `sudo pacman -S --needed rust cmake clang mpv wayland libxkbcommon libx11 libxrandr libxinerama libxcursor libxi` |
| Debian, Ubuntu | `sudo apt install build-essential cmake clang pkg-config libmpv-dev libwayland-dev libxkbcommon-dev wayland-protocols libx11-dev libxrandr-dev libxinerama-dev libxcursor-dev libxi-dev libgl-dev` |
| Fedora | `sudo dnf install gcc cmake clang mpv-devel wayland-devel libxkbcommon-devel libX11-devel libXrandr-devel libXinerama-devel libXcursor-devel libXi-devel mesa-libGL-devel` |

Then:

```sh
git clone https://github.com/alireza-hariri/ghoori.git
cd ghoori
./install.sh
```

`install.sh` builds a release binary and installs it like the prebuilt one. To install system-wide, build as your
normal user first and then install with sudo: `cargo build --release && sudo ./install.sh`. Run
`./install.sh --help` for all options.

To just try it without installing: `cargo run --release -- video.mkv`.

### Updating

- **Prebuilt binary:** run the two commands from option 1 again.
- **Arch package:** run `makepkg -si` again in `ghoori/packaging/arch`.
- **From source:** run `git pull && ./install.sh` in the `ghoori` folder.

Settings and resume positions are kept.

### Uninstalling

```sh
~/.local/share/ghoori/install.sh --uninstall            # user install
sudo /usr/local/share/ghoori/install.sh --uninstall     # system-wide install
```

Your settings in `~/.config/ghoori` are left in place. Delete that folder too if you want them gone.

## Usage

Open Ghoori from your application menu, open files with it from your file manager, or run it from a terminal:

```sh
ghoori [OPTIONS] [FILE|URL|DIR]...
```

| Option | Meaning |
| --- | --- |
| `-f`, `--fullscreen` | Start fullscreen |
| `--volume <0-150>` | Initial volume (default: last used) |
| `--mute` | Start muted |
| `--no-thumbnails` | Disable seek previews |
| `--mpv-<opt>=<val>` | Pass any option to mpv, e.g. `--mpv-hwdec=no` |
| `-v`, `--verbose` | Show mpv and raylib logs |
| `-V`, `--version` | Show the version |

YouTube and other web video URLs work when [yt-dlp](https://github.com/yt-dlp/yt-dlp) is installed.

## Troubleshooting

- **`ghoori: command not found` after installing:** `~/.local/bin` is not on your `PATH`. The installer prints the
  line to add to `~/.bashrc` or `~/.zshrc`. The application-menu entry works either way.
- **`error while loading shared libraries: libmpv.so.2`:** install libmpv (see the table under [Install](#install)).
- **Black video, stutter or a crash on start:** try software decoding with `ghoori --mpv-hwdec=no video.mkv`. If that
  helps, put `hwdec=no` in `~/.config/ghoori/mpv.conf` to make it permanent, and please open an issue. Run with
  `--verbose` to see mpv's log.
- **Persian/Arabic titles show as boxes:** install the Vazirmatn font (`ttf-vazirmatn` on Arch, `fonts-vazirmatn` on
  Debian/Ubuntu), or point `GHOORI_FONT` at any `.ttf` file that covers the script.
- **Ghoori is not offered in "Open with":** log out and back in once after installing so your desktop picks up the new
  launcher entry.

When reporting a bug, please include the output of `ghoori --version` and of `ghoori --verbose FILE`.

## Keyboard

| Key | Action |
| --- | --- |
| Space / K | Play / pause |
| Left / Right | Seek 5 s (Shift: 1 s) |
| J / L | Seek 10 s |
| Shift + Up / Down | Seek 1 min |
| 0 – 9 | Jump to 0% – 90% |
| PgUp / PgDn | Previous / next chapter |
| , / . | Frame back / step |
| Up / Down, wheel | Volume |
| M, middle click | Mute |
| [ / ], Backspace | Speed −/+, reset |
| N / P, Ctrl+Right / Left | Next / previous file |
| F, F11, double click | Fullscreen |
| Tab | Playlist |
| C / A | Cycle subtitles / audio |
| V | Show / hide subtitles |
| Z / X | Subtitle delay −/+ 100 ms |
| R / Shift+R | Repeat file / A-B loop |
| S | Screenshot (saved to `~/Pictures/Ghoori`) |
| I | Statistics |
| Ctrl+O / Ctrl+U / Ctrl+V | Open file browser / URL dialog / play clipboard |
| Ctrl+S / Ctrl+T | Shuffle playlist / toggle seek previews |
| Q, Ctrl+Q | Quit (remembers position) |

## Files

| Path | Contents |
| --- | --- |
| `~/.config/ghoori/settings.conf` | Volume, mute, recent files, last browser folder |
| `~/.config/ghoori/mpv.conf` | Your own mpv options (optional) |
| `~/.local/state/ghoori/watch_later/` | Resume positions |

## Architecture

| Module | Role |
| --- | --- |
| `main.rs` | CLI, window creation, main loop, signal handling |
| `player.rs` | mpv setup, observed-property state, event pump, hwdec selection |
| `render.rs` | mpv OpenGL render context (raw `libmpv2-sys`), GL state hand-off with raylib |
| `thumbs.rs` | Seek-preview thumbnailer (second mpv core rendering into a small texture) |
| `app.rs` | Input, actions and the immediate-mode UI (bar, panels, menus, dialogs) |
| `browser.rs` | File browser model |
| `text.rs` | Font loading, Arabic-script shaping and bidi reordering |
| `icons.rs` | Vector icons drawn from raylib primitives |
| `settings.rs`, `util.rs` | Persistence and helpers |

Notes on the libmpv/raylib integration:

- The render context is driven through `libmpv2-sys` directly. The safe wrapper's FBO struct lacks the
  `internal_format` field mpv reads.
- mpv expects default GL state. raylib enables back-face culling and blending, so both are turned off before every
  mpv render and restored afterwards. Otherwise mpv's quad gets culled and the video is black.
- The native Wayland/X11 display is passed to mpv so VAAPI interop works.
- raylib resets GLFW's window hints inside `InitWindow`. `build.rs` therefore links raylib's `glfwCreateWindow` call
  through a wrapper that sets the Wayland app-id and the X11 class to `ghoori`. Desktops use these to match the
  window to `ghoori.desktop` and its icon.
- The window uses raylib's HiDPI mode. Under fractional desktop scaling (e.g. KDE at 115%) the UI is laid out in
  logical pixels while the framebuffer, video and text render at native physical resolution. Without it, GLFW
  (built with both X11 and Wayland) scales the framebuffer behind raylib's back and viewport and mouse coordinates
  drift apart. raylib also rescales the mouse as if cursor positions were in pixels, but Wayland reports them in
  logical units, so the app sets the mouse scale itself every frame from GLFW's window size.
- Text is rasterized into a separate atlas for each physical pixel size it is drawn at, and glyph origins are snapped
  to physical pixels. This keeps small UI text sharp at fractional scales instead of minifying a large atlas.

## Icons

The icon source is `icons/ghoori.svg`. The PNGs are rendered from it with `rsvg-convert -w N -h N icons/ghoori.svg`.

## License

Ghoori is released under the [MIT License](LICENSE). Release binaries link to libmpv, which is licensed separately
(LGPL or GPL, depending on how your distro builds it).
