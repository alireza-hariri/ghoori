//! Resolution-independent icons drawn from raylib shape primitives.

use raylib::prelude::*;

fn v(x: f32, y: f32) -> Vector2 {
    Vector2::new(x, y)
}

/// Filled triangle with any winding (raylib needs counter-clockwise).
pub fn tri(d: &mut impl RaylibDraw, a: Vector2, b: Vector2, c: Vector2, col: Color) {
    let cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if cross < 0.0 { d.draw_triangle(a, b, c, col) } else { d.draw_triangle(a, c, b, col) }
}

fn rect(d: &mut impl RaylibDraw, x: f32, y: f32, w: f32, h: f32, col: Color) {
    d.draw_rectangle_rounded(Rectangle::new(x, y, w, h), 0.35, 4, col);
}

pub fn play(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color) {
    let h = s * 0.5;
    tri(d, v(c.x - h * 0.75, c.y - h), v(c.x - h * 0.75, c.y + h), v(c.x + h, c.y), col);
}

pub fn pause(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color) {
    let (w, h) = (s * 0.28, s * 0.9);
    rect(d, c.x - s * 0.38, c.y - h / 2.0, w, h, col);
    rect(d, c.x + s * 0.1, c.y - h / 2.0, w, h, col);
}

pub fn replay(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color) {
    let r = s * 0.42;
    d.draw_ring(c, r - s * 0.11, r, -60.0, 230.0, 24, col);
    let a = (-60.0f32).to_radians();
    let tip = v(c.x + r * a.cos(), c.y + r * a.sin());
    tri(
        d,
        v(tip.x - s * 0.22, tip.y - s * 0.02),
        v(tip.x + s * 0.12, tip.y - s * 0.22),
        v(tip.x + s * 0.12, tip.y + s * 0.16),
        col,
    );
}

pub fn skip(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color, forward: bool) {
    let h = s * 0.4;
    let dir = if forward { 1.0 } else { -1.0 };
    tri(d, v(c.x - dir * h * 0.8, c.y - h), v(c.x - dir * h * 0.8, c.y + h), v(c.x + dir * h * 0.6, c.y), col);
    let bx = if forward { c.x + h * 0.6 } else { c.x - h * 0.6 - s * 0.12 };
    rect(d, bx, c.y - h, s * 0.12, h * 2.0, col);
}

pub fn volume(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color, level: f64, muted: bool) {
    let x0 = c.x - s * 0.45;
    rect(d, x0, c.y - s * 0.14, s * 0.18, s * 0.28, col);
    tri(d, v(x0 + s * 0.1, c.y - s * 0.14), v(x0 + s * 0.38, c.y - s * 0.38), v(x0 + s * 0.1, c.y + s * 0.14), col);
    tri(d, v(x0 + s * 0.1, c.y + s * 0.14), v(x0 + s * 0.38, c.y - s * 0.38), v(x0 + s * 0.38, c.y + s * 0.38), col);
    let ax = x0 + s * 0.38;
    if muted {
        let (cx, cy, r) = (ax + s * 0.3, c.y, s * 0.15);
        d.draw_line_ex(v(cx - r, cy - r), v(cx + r, cy + r), s * 0.08, col);
        d.draw_line_ex(v(cx - r, cy + r), v(cx + r, cy - r), s * 0.08, col);
        return;
    }
    let ac = v(ax - s * 0.02, c.y);
    if level > 0.0 {
        d.draw_ring(ac, s * 0.14, s * 0.21, -50.0, 50.0, 12, col);
    }
    if level > 50.0 {
        d.draw_ring(ac, s * 0.3, s * 0.37, -50.0, 50.0, 16, col);
    }
    if level > 100.0 {
        d.draw_ring(ac, s * 0.46, s * 0.53, -45.0, 45.0, 16, col);
    }
}

pub fn fullscreen(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color, exit: bool) {
    let (h, l, t) = (s * 0.4, s * 0.28, s * 0.09);
    for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let corner = if exit { v(c.x + sx * h * 0.35, c.y + sy * h * 0.35) } else { v(c.x + sx * h, c.y + sy * h) };
        let dir = if exit { 1.0 } else { -1.0 };
        let hx = v(corner.x + sx * dir * l, corner.y);
        let vy = v(corner.x, corner.y + sy * dir * l);
        d.draw_line_ex(corner, hx, t, col);
        d.draw_line_ex(corner, vy, t, col);
        d.draw_circle_v(corner, t / 2.0, col);
    }
}

pub fn playlist(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color) {
    let t = s * 0.09;
    for (i, w) in [0.8f32, 0.8, 0.5].iter().enumerate() {
        let y = c.y - s * 0.3 + i as f32 * s * 0.3;
        d.draw_line_ex(v(c.x - s * 0.42, y), v(c.x - s * 0.42 + s * w, y), t, col);
    }
    tri(d, v(c.x + s * 0.18, c.y + s * 0.12), v(c.x + s * 0.18, c.y + s * 0.48), v(c.x + s * 0.46, c.y + s * 0.3), col);
}

pub fn subtitles(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color) {
    let r = Rectangle::new(c.x - s * 0.45, c.y - s * 0.32, s * 0.9, s * 0.64);
    d.draw_rectangle_rounded_lines_ex(r, 0.3, 6, s * 0.08, col);
    let t = s * 0.08;
    d.draw_line_ex(v(c.x - s * 0.28, c.y + s * 0.02), v(c.x + s * 0.02, c.y + s * 0.02), t, col);
    d.draw_line_ex(v(c.x + s * 0.12, c.y + s * 0.02), v(c.x + s * 0.28, c.y + s * 0.02), t, col);
    d.draw_line_ex(v(c.x - s * 0.28, c.y + s * 0.17), v(c.x - s * 0.1, c.y + s * 0.17), t, col);
    d.draw_line_ex(v(c.x, c.y + s * 0.17), v(c.x + s * 0.28, c.y + s * 0.17), t, col);
}

pub fn audio_tracks(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color) {
    let t = s * 0.1;
    for (i, h) in [0.3f32, 0.7, 0.5, 0.85, 0.4].iter().enumerate() {
        let x = c.x - s * 0.4 + i as f32 * s * 0.2;
        d.draw_line_ex(v(x, c.y - s * h / 2.0), v(x, c.y + s * h / 2.0), t, col);
    }
}

pub fn note(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color) {
    let t = s * 0.08;
    let (l, r) = (c.x - s * 0.2, c.x + s * 0.32);
    d.draw_ellipse((l - s * 0.1) as i32, (c.y + s * 0.3) as i32, s * 0.15, s * 0.11, col);
    d.draw_ellipse((r - s * 0.1) as i32, (c.y + s * 0.2) as i32, s * 0.15, s * 0.11, col);
    d.draw_line_ex(v(l + s * 0.03, c.y + s * 0.3), v(l + s * 0.03, c.y - s * 0.35), t, col);
    d.draw_line_ex(v(r + s * 0.03, c.y + s * 0.2), v(r + s * 0.03, c.y - s * 0.45), t, col);
    tri(d, v(l, c.y - s * 0.35), v(r + s * 0.07, c.y - s * 0.45), v(r + s * 0.07, c.y - s * 0.3), col);
    tri(d, v(l, c.y - s * 0.35), v(r + s * 0.07, c.y - s * 0.3), v(l, c.y - s * 0.2), col);
}

pub fn close(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color) {
    let r = s * 0.3;
    d.draw_line_ex(v(c.x - r, c.y - r), v(c.x + r, c.y + r), s * 0.1, col);
    d.draw_line_ex(v(c.x - r, c.y + r), v(c.x + r, c.y - r), s * 0.1, col);
}

pub fn folder(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color) {
    rect(d, c.x - s * 0.45, c.y - s * 0.32, s * 0.4, s * 0.2, col);
    rect(d, c.x - s * 0.45, c.y - s * 0.22, s * 0.9, s * 0.58, col);
}

pub fn file(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color) {
    let r = Rectangle::new(c.x - s * 0.32, c.y - s * 0.42, s * 0.64, s * 0.84);
    d.draw_rectangle_rounded_lines_ex(r, 0.2, 4, s * 0.08, col);
    play(d, Vector2::new(c.x + s * 0.03, c.y), s * 0.35, col);
}

pub fn up(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color) {
    let t = s * 0.1;
    d.draw_line_ex(v(c.x, c.y - s * 0.38), v(c.x, c.y + s * 0.38), t, col);
    d.draw_line_ex(v(c.x, c.y - s * 0.38), v(c.x - s * 0.3, c.y - s * 0.08), t, col);
    d.draw_line_ex(v(c.x, c.y - s * 0.38), v(c.x + s * 0.3, c.y - s * 0.08), t, col);
}

pub fn shuffle(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color) {
    let t = s * 0.08;
    let (l, r) = (c.x - s * 0.42, c.x + s * 0.3);
    d.draw_line_ex(v(l, c.y - s * 0.25), v(r, c.y + s * 0.25), t, col);
    d.draw_line_ex(v(l, c.y + s * 0.25), v(r, c.y - s * 0.25), t, col);
    for y in [-0.25f32, 0.25] {
        let y = c.y + s * y;
        tri(d, v(r, y - s * 0.14), v(r, y + s * 0.14), v(r + s * 0.18, y), col);
    }
}

pub fn repeat(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color, one: bool) {
    let r = Rectangle::new(c.x - s * 0.4, c.y - s * 0.25, s * 0.8, s * 0.5);
    d.draw_rectangle_rounded_lines_ex(r, 0.6, 8, s * 0.08, col);
    tri(d, v(c.x + s * 0.02, c.y - s * 0.4), v(c.x + s * 0.02, c.y - s * 0.1), v(c.x + s * 0.2, c.y - s * 0.25), col);
    if one {
        d.draw_circle_v(c, s * 0.13, col);
    }
}

pub fn trash(d: &mut impl RaylibDraw, c: Vector2, s: f32, col: Color) {
    rect(d, c.x - s * 0.35, c.y - s * 0.32, s * 0.7, s * 0.1, col);
    rect(d, c.x - s * 0.12, c.y - s * 0.42, s * 0.24, s * 0.1, col);
    let body = Rectangle::new(c.x - s * 0.27, c.y - s * 0.18, s * 0.54, s * 0.58);
    d.draw_rectangle_rounded_lines_ex(body, 0.2, 4, s * 0.08, col);
}

/// The ghoori (teapot) logo.
pub fn teapot(d: &mut impl RaylibDraw, c: Vector2, s: f32, body: Color, accent: Color) {
    // Spout and handle sit behind the body.
    let spout_base = v(c.x + s * 0.32, c.y + s * 0.08);
    d.draw_line_ex(spout_base, v(c.x + s * 0.62, c.y - s * 0.2), s * 0.11, body);
    d.draw_circle_v(v(c.x + s * 0.62, c.y - s * 0.2), s * 0.055, body);
    d.draw_ring(v(c.x - s * 0.42, c.y + s * 0.02), s * 0.13, s * 0.21, 90.0, 270.0, 24, body);
    d.draw_ellipse(c.x as i32, (c.y + s * 0.05) as i32, s * 0.45, s * 0.36, body);
    // Lid and knob.
    d.draw_circle_sector(v(c.x, c.y - s * 0.24), s * 0.22, 180.0, 360.0, 24, body);
    d.draw_circle_v(v(c.x, c.y - s * 0.48), s * 0.07, accent);
    // Band.
    d.draw_rectangle_rounded(Rectangle::new(c.x - s * 0.44, c.y - s * 0.02, s * 0.88, s * 0.1), 1.0, 8, accent);
    // Steam.
    for (i, x) in [-0.12f32, 0.06].iter().enumerate() {
        let x0 = c.x + s * x;
        let top = c.y - s * (0.72 + i as f32 * 0.08);
        d.draw_line_bezier(
            v(x0, c.y - s * 0.58),
            v(x0 + s * 0.04, top),
            s * 0.04,
            Color::new(body.r, body.g, body.b, body.a / 2),
        );
    }
}
