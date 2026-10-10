mod app;
mod browser;
mod icons;
mod player;
mod render;
mod settings;
mod text;
mod thumbs;
mod util;

use app::{App, StartupOptions};
use player::{Player, PlayerOptions};
use raylib::prelude::*;
use settings::Settings;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set from SIGINT/SIGTERM so a terminal Ctrl+C still saves position and settings.
static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: i32) {
    QUIT_REQUESTED.store(true, Ordering::Relaxed);
}

fn install_signal_handlers() {
    unsafe extern "C" {
        fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
    }
    const SIGINT: i32 = 2;
    const SIGTERM: i32 = 15;
    unsafe {
        signal(SIGINT, on_signal);
        signal(SIGTERM, on_signal);
    }
}

const USAGE: &str = "\
Ghoori — a media player built on mpv and raylib

USAGE:
    ghoori [OPTIONS] [FILE|URL|DIR]...

OPTIONS:
    -f, --fullscreen        Start in fullscreen
        --volume <0-150>    Initial volume (default: last used)
        --mute              Start muted
        --no-thumbnails     Disable seek-bar preview thumbnails
        --mpv-<opt>=<val>   Pass an option straight to mpv (e.g. --mpv-hwdec=no)
    -v, --verbose           Print mpv log output to the terminal
    -h, --help              Show this help
    -V, --version           Show version

Files are added to the playlist in order; directories are expanded.
mpv settings are read from ~/.config/ghoori/mpv.conf.
Press F1 inside the player for keyboard shortcuts.
";

struct Cli {
    files: Vec<String>,
    fullscreen: bool,
    volume: Option<f64>,
    mute: bool,
    thumbnails: bool,
    verbose: bool,
    mpv: Vec<(String, String)>,
    snapshot: Option<(String, f64)>,
}

fn parse_args() -> Result<Cli, String> {
    let mut cli = Cli {
        files: Vec::new(),
        fullscreen: false,
        volume: None,
        mute: false,
        thumbnails: true,
        verbose: false,
        mpv: Vec::new(),
        snapshot: None,
    };
    let mut args = std::env::args().skip(1);
    let mut only_files = false;
    while let Some(a) = args.next() {
        if only_files || !a.starts_with('-') || a == "-" {
            cli.files.push(a);
            continue;
        }
        match a.as_str() {
            "--" => only_files = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("ghoori {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-f" | "--fullscreen" => cli.fullscreen = true,
            "--mute" => cli.mute = true,
            "--no-thumbnails" => cli.thumbnails = false,
            "-v" | "--verbose" => cli.verbose = true,
            "--volume" => {
                let v = args.next().ok_or("--volume needs a value")?;
                cli.volume = Some(v.parse().map_err(|_| format!("invalid volume: {v}"))?);
            }
            _ if a.starts_with("--volume=") => {
                let v = &a["--volume=".len()..];
                cli.volume = Some(v.parse().map_err(|_| format!("invalid volume: {v}"))?);
            }
            _ if a.starts_with("--mpv-") => {
                let rest = &a["--mpv-".len()..];
                let (k, v) = rest.split_once('=').unwrap_or((rest, "yes"));
                cli.mpv.push((k.to_string(), v.to_string()));
            }
            // Hidden: render for N seconds, save a screenshot of the window and exit.
            _ if a.starts_with("--snapshot=") => {
                let spec = &a["--snapshot=".len()..];
                let (path, secs) = spec.rsplit_once(':').ok_or("--snapshot=PATH:SECONDS")?;
                cli.snapshot = Some((path.to_string(), secs.parse().map_err(|_| "bad snapshot delay")?));
            }
            _ => return Err(format!("unknown option: {a}\n\n{USAGE}")),
        }
    }
    Ok(cli)
}

fn main() {
    let cli = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ghoori: {e}");
            std::process::exit(2);
        }
    };

    // libmpv requires the C numeric locale.
    unsafe { libc_setlocale_numeric_c() };

    let mut settings = Settings::load();
    if let Some(v) = cli.volume {
        settings.volume = v.clamp(0.0, 150.0);
    }
    if cli.mute {
        settings.muted = true;
    }

    // raylib's own warnings (e.g. Wayland refusing window positioning) are noise for users.
    let log_level = if cli.verbose { TraceLogLevel::LOG_INFO } else { TraceLogLevel::LOG_ERROR };
    let (mut rl, thread) = raylib::init()
        .log_level(log_level)
        .size(1280, 720)
        .title("Ghoori")
        .resizable()
        // Render at native resolution under fractional desktop scaling (e.g. KDE 115%):
        // layout stays in logical pixels while video and text use physical pixels.
        .highdpi()
        .vsync()
        .msaa_4x()
        .build();
    rl.set_window_min_size(480, 300);
    rl.set_exit_key(None);
    rl.set_target_fps(0);

    // Test snapshots must not touch the user's resume positions or settings.
    let mut mpv_opts = cli.mpv.clone();
    if cli.snapshot.is_some() {
        mpv_opts.push(("save-position-on-quit".into(), "no".into()));
        mpv_opts.push(("resume-playback".into(), "no".into()));
    }
    let player = match Player::new(PlayerOptions {
        volume: settings.volume,
        muted: settings.muted,
        verbose: cli.verbose,
        extra: &mpv_opts,
    }) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ghoori: failed to initialise mpv: {e}");
            std::process::exit(1);
        }
    };

    let opts = StartupOptions {
        files: cli.files,
        fullscreen: cli.fullscreen,
        thumbnails: cli.thumbnails,
        snapshot: cli.snapshot,
    };
    let mut app = match App::new(&mut rl, &thread, player, settings, opts) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("ghoori: failed to set up video output: {e}");
            std::process::exit(1);
        }
    };

    install_signal_handlers();
    while !app.should_quit() && !QUIT_REQUESTED.load(Ordering::Relaxed) {
        app.frame(&mut rl, &thread);
    }
    app.shutdown();
    // Render contexts and textures must go before the GL context does.
    drop(app);
}

unsafe fn libc_setlocale_numeric_c() {
    unsafe extern "C" {
        fn setlocale(category: i32, locale: *const std::ffi::c_char) -> *mut std::ffi::c_char;
    }
    const LC_NUMERIC: i32 = 1; // glibc / musl
    unsafe { setlocale(LC_NUMERIC, c"C".as_ptr()) };
}
