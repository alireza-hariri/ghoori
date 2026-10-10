fn main() {
    // raylib resets GLFW's window hints inside InitWindow, so the Wayland app-id / X11 class can't be set
    // beforehand. Route raylib's glfwCreateWindow call through a wrapper (src/render.rs) that sets them;
    // the desktop uses them to match the window with ghoori.desktop and its icon.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg=-Wl,--wrap=glfwCreateWindow");
    }
}
