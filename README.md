<div align="center">
<img src="icons/ghoori.svg" width="128" alt="Ghoori logo">

# Ghoori

A media player for Linux, written in Rust.
</div>

Ghoori plays anything mpv plays — local files, playlists, network streams — with a clean, fast interface on top.
Under the hood, [libmpv2-rs](https://crates.io/crates/libmpv2 decodes and renders)
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

## Requirements

- libmpv (mpv ≥ 0.38, client API 2.x)
- A C toolchain, CMake and clang (raylib is compiled from source by `raylib-sys`)
- Wayland and/or X11 development libraries (GLFW is built with both and picks the session's backend)

On Arch Linux: `pacman -S mpv cmake clang wayland libxkbcommon libx11 libxrandr libxinerama libxcursor libxi`

## Build & run

```sh
cargo build --release
./target/release/ghoori [OPTIONS] [FILE|URL|DIR]...
```

| Option | Meaning |
| --- | --- |
| `-f`, `--fullscreen` | Start fullscreen |
| `--volume <0-150>` | Initial volume (default: last used) |
| `--mute` | Start muted |
| `--no-thumbnails` | Disable seek previews |
| `--mpv-<opt>=<val>` | Pass any option to mpv, e.g. `--mpv-hwdec=no` |
| `-v`, `--verbose` | Show mpv and raylib logs |

To install the binary, launcher entry and icon into `~/.local` (or `/usr/local` with `sudo`), run `./install.sh`.
Use `./install.sh --uninstall` to remove them. The icon source is `icons/ghoori.svg`. The PNGs are rendered from it
with `rsvg-convert -w N -h N icons/ghoori.svg`.

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
