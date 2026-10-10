//! Font loading plus minimal Arabic-script shaping and bidi reordering so that
//! Persian/Arabic titles render legibly with raylib's glyph-by-glyph renderer.

use raylib::prelude::*;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

const FONT_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/vazirmatn/Vazirmatn-Medium.ttf",
    "/usr/share/fonts/vazirmatn/Vazirmatn-Regular.ttf",
    "/usr/share/fonts/TTF/Vazirmatn-Regular.ttf",
    "/usr/share/fonts/truetype/vazirmatn/Vazirmatn-Regular.ttf",
    "/usr/share/fonts/noto/NotoSans-Medium.ttf",
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/NotoSans-Regular.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/DejaVuSans.ttf",
    "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
];

/// Ask fontconfig for a sans font covering Persian, for layouts not in FONT_CANDIDATES.
/// raylib only loads plain .ttf/.otf files (no collections).
fn fontconfig_sans() -> Option<String> {
    let out = std::process::Command::new("fc-match").args(["-f", "%{file}", "sans-serif:lang=fa"]).output().ok()?;
    let path = String::from_utf8(out.stdout).ok()?;
    let ext = std::path::Path::new(&path).extension()?.to_ascii_lowercase();
    (ext == "ttf" || ext == "otf").then_some(path)
}

const BASE_SIZE: i32 = 64;
/// Text up to this many physical pixels gets its own atlas rasterized at exactly that size.
const MAX_EXACT_PX: i32 = 64;

pub struct Fonts {
    /// Large atlas with mipmaps, used for oversized text and as a fallback.
    font: Option<Font>,
    path: Option<std::ffi::CString>,
    /// Atlases rasterized at exact physical pixel sizes, so small UI text is drawn 1:1 instead of
    /// being minified from the large atlas (blurry, especially under fractional desktop scaling).
    exact: RefCell<HashMap<i32, Font>>,
    /// Physical pixels per layout unit (the HiDPI scale).
    scale: Cell<f32>,
}

fn codepoints() -> Vec<i32> {
    let ranges: &[(u32, u32)] = &[
        (0x20, 0x7E),
        (0xA0, 0x24F),
        (0x370, 0x3FF),
        (0x400, 0x4FF),
        (0x600, 0x6FF),
        (0x2000, 0x206F),
        (0x2190, 0x21FF),
        (0x2200, 0x22FF),
        (0xFB50, 0xFDFF),
        (0xFE70, 0xFEFF),
    ];
    ranges.iter().flat_map(|&(a, b)| a..=b).filter(|&c| char::from_u32(c).is_some()).map(|c| c as i32).collect()
}

fn load_raw(path: &std::ffi::CStr, px: i32) -> Option<raylib::ffi::Font> {
    let mut cps = codepoints();
    let f = unsafe { raylib::ffi::LoadFontEx(path.as_ptr(), px, cps.as_mut_ptr(), cps.len() as i32) };
    (f.texture.id != 0 && !f.glyphs.is_null()).then_some(f)
}

impl Fonts {
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, explicit: Option<&str>) -> Self {
        let _ = (rl, thread); // GL context must exist; raylib's loader is global.
        // fc-match is only spawned if none of the known paths load.
        let paths = (explicit.into_iter().map(String::from))
            .chain(FONT_CANDIDATES.iter().map(|p| p.to_string()))
            .chain(std::iter::once_with(fontconfig_sans).flatten());
        for p in paths {
            if !std::path::Path::new(&p).exists() {
                continue;
            }
            let Ok(cpath) = std::ffi::CString::new(p) else { continue };
            let Some(mut f) = load_raw(&cpath, BASE_SIZE) else { continue };
            unsafe {
                // Mipmaps keep large text smooth when it is drawn smaller than the atlas.
                raylib::ffi::GenTextureMipmaps(&mut f.texture);
                raylib::ffi::SetTextureFilter(f.texture, TextureFilter::TEXTURE_FILTER_TRILINEAR as i32);
                return Self {
                    font: Some(Font::from_raw(f)),
                    path: Some(cpath),
                    exact: RefCell::default(),
                    scale: Cell::new(1.0),
                };
            }
        }
        eprintln!("ghoori: no TrueType font found, falling back to raylib's bitmap font");
        Self { font: None, path: None, exact: RefCell::default(), scale: Cell::new(1.0) }
    }

    /// Set the HiDPI scale (physical pixels per layout unit) for subsequent text.
    pub fn set_scale(&self, scale: f32) {
        if scale.is_finite() && scale > 0.0 {
            self.scale.set(scale);
        }
    }

    /// The font and layout size to use for text of `size` layout units.
    fn pick(&self, size: f32) -> (raylib::ffi::Font, f32) {
        let scale = self.scale.get();
        let px = (size * scale).round() as i32;
        if let Some(path) = &self.path
            && (6..=MAX_EXACT_PX).contains(&px)
        {
            let mut exact = self.exact.borrow_mut();
            if !exact.contains_key(&px)
                && let Some(f) = load_raw(path, px)
            {
                unsafe { raylib::ffi::SetTextureFilter(f.texture, TextureFilter::TEXTURE_FILTER_BILINEAR as i32) };
                exact.insert(px, unsafe { Font::from_raw(f) });
            }
            if let Some(f) = exact.get(&px) {
                // Drawn at px / scale layout units, the screen transform maps it to exactly px pixels.
                return (*f.as_ref(), px as f32 / scale);
            }
        }
        (self.raw(), size)
    }

    fn raw(&self) -> raylib::ffi::Font {
        match &self.font {
            Some(f) => *f.as_ref(),
            None => unsafe { raylib::ffi::GetFontDefault() },
        }
    }

    pub fn measure(&self, s: &str, size: f32) -> f32 {
        let vis = visual(s);
        let c = std::ffi::CString::new(vis.replace('\0', "")).unwrap_or_default();
        let (font, size) = self.pick(size);
        unsafe { raylib::ffi::MeasureTextEx(font, c.as_ptr(), size, 0.0).x }
    }

    pub fn draw(&self, d: &mut impl RaylibDraw, s: &str, pos: Vector2, size: f32, color: Color) {
        let vis = visual(s);
        let (font, size) = self.pick(size);
        // Start glyphs on whole physical pixels so they are not resampled between texels.
        let scale = self.scale.get();
        let pos = Vector2::new((pos.x * scale).round() / scale, (pos.y * scale).round() / scale);
        d.draw_text_ex(unsafe { WeakFont::from_raw(font) }, &vis.replace('\0', ""), pos, size, 0.0, color);
    }

    /// Draw with a soft shadow for legibility over video.
    pub fn draw_shadow(&self, d: &mut impl RaylibDraw, s: &str, pos: Vector2, size: f32, color: Color) {
        let off = (size / 18.0).max(1.0);
        let shadow = Color::new(0, 0, 0, (color.a as f32 * 0.7) as u8);
        self.draw(d, s, Vector2::new(pos.x + off, pos.y + off), size, shadow);
        self.draw(d, s, pos, size, color);
    }

    /// Truncate with an ellipsis so the text fits in `max_w` pixels.
    pub fn fit<'a>(&self, s: &'a str, size: f32, max_w: f32) -> Cow<'a, str> {
        if self.measure(s, size) <= max_w {
            return Cow::Borrowed(s);
        }
        let chars: Vec<char> = s.chars().collect();
        let (mut lo, mut hi) = (0usize, chars.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let cand: String = chars[..mid].iter().collect::<String>() + "…";
            if self.measure(&cand, size) <= max_w { lo = mid } else { hi = mid - 1 }
        }
        Cow::Owned(chars[..lo].iter().collect::<String>().trim_end().to_string() + "…")
    }
}

// ---------------------------------------------------------------------------
// Arabic-script shaping

#[derive(Clone, Copy, PartialEq)]
enum Join {
    Dual,
    Right,
}

/// (isolated, final, initial, medial) presentation forms. Right-joining letters
/// have no initial/medial forms (0).
fn forms(c: char) -> Option<(Join, [u32; 4])> {
    use Join::*;
    let f = |i: u32, j: Join| {
        let v = match j {
            Dual => [i, i + 1, i + 2, i + 3],
            Right => [i, i + 1, 0, 0],
        };
        Some((j, v))
    };
    match c as u32 {
        0x0621 => Some((Right, [0xFE80, 0xFE80, 0, 0])),
        0x0622 => f(0xFE81, Right),
        0x0623 => f(0xFE83, Right),
        0x0624 => f(0xFE85, Right),
        0x0625 => f(0xFE87, Right),
        0x0626 => f(0xFE89, Dual),
        0x0627 => f(0xFE8D, Right),
        0x0628 => f(0xFE8F, Dual),
        0x0629 => f(0xFE93, Right),
        0x062A => f(0xFE95, Dual),
        0x062B => f(0xFE99, Dual),
        0x062C => f(0xFE9D, Dual),
        0x062D => f(0xFEA1, Dual),
        0x062E => f(0xFEA5, Dual),
        0x062F => f(0xFEA9, Right),
        0x0630 => f(0xFEAB, Right),
        0x0631 => f(0xFEAD, Right),
        0x0632 => f(0xFEAF, Right),
        0x0633 => f(0xFEB1, Dual),
        0x0634 => f(0xFEB5, Dual),
        0x0635 => f(0xFEB9, Dual),
        0x0636 => f(0xFEBD, Dual),
        0x0637 => f(0xFEC1, Dual),
        0x0638 => f(0xFEC5, Dual),
        0x0639 => f(0xFEC9, Dual),
        0x063A => f(0xFECD, Dual),
        0x0641 => f(0xFED1, Dual),
        0x0642 => f(0xFED5, Dual),
        0x0643 => f(0xFED9, Dual),
        0x0644 => f(0xFEDD, Dual),
        0x0645 => f(0xFEE1, Dual),
        0x0646 => f(0xFEE5, Dual),
        0x0647 => f(0xFEE9, Dual),
        0x0648 => f(0xFEED, Right),
        0x0649 => f(0xFEEF, Right),
        0x064A => f(0xFEF1, Dual),
        0x067E => f(0xFB56, Dual),  // پ
        0x0686 => f(0xFB7A, Dual),  // چ
        0x0698 => f(0xFB8A, Right), // ژ
        0x06A9 => f(0xFB8E, Dual),  // ک
        0x06AF => f(0xFB92, Dual),  // گ
        0x06CC => f(0xFBFC, Dual),  // ی
        _ => None,
    }
}

fn is_transparent(c: char) -> bool {
    matches!(c as u32, 0x064B..=0x065F | 0x0670 | 0x06D6..=0x06ED)
}

fn lam_alef(alef: char) -> Option<u32> {
    match alef as u32 {
        0x0622 => Some(0xFEF5),
        0x0623 => Some(0xFEF7),
        0x0625 => Some(0xFEF9),
        0x0627 => Some(0xFEFB),
        _ => None,
    }
}

pub fn shape(chars: &[char]) -> Vec<char> {
    let joins = |c: char| forms(c).map(|(j, _)| j);
    let neighbour = |i: usize, dir: isize| -> Option<char> {
        let mut k = i as isize + dir;
        while k >= 0 && (k as usize) < chars.len() {
            let c = chars[k as usize];
            if !is_transparent(c) {
                return Some(c);
            }
            k += dir;
        }
        None
    };
    let mut out = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let Some((join, f)) = forms(c) else {
            out.push(c);
            i += 1;
            continue;
        };
        let prev_connects = neighbour(i, -1).and_then(joins) == Some(Join::Dual);
        // Lam + Alef ligature.
        if c == '\u{0644}'
            && let Some(next) = chars.get(i + 1)
            && let Some(lig) = lam_alef(*next)
        {
            let code = if prev_connects { lig + 1 } else { lig };
            out.extend(char::from_u32(code));
            i += 2;
            continue;
        }
        let next_connects = join == Join::Dual && neighbour(i, 1).and_then(joins).is_some();
        let idx = match (prev_connects, next_connects) {
            (true, true) => 3,
            (true, false) => 1,
            (false, true) => 2,
            (false, false) => 0,
        };
        let code = if f[idx] == 0 { f[0] } else { f[idx] };
        out.push(char::from_u32(code).unwrap_or(c));
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Bidi (simplified): enough for titles mixing Persian with Latin and digits.

#[derive(Clone, Copy, PartialEq)]
enum Dir {
    L,
    R,
    N,
}

fn dir(c: char) -> Dir {
    let u = c as u32;
    if matches!(u, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
        && !matches!(u, 0x06F0..=0x06F9 | 0x0660..=0x0669)
    {
        Dir::R
    } else if c.is_alphanumeric() {
        Dir::L
    } else {
        Dir::N
    }
}

fn mirror(c: char) -> char {
    match c {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        _ => c,
    }
}

/// Reorder a logical run into visual order for the given base direction.
fn reorder(chars: &[char], rtl: bool) -> Vec<char> {
    let opposite = if rtl { Dir::L } else { Dir::R };
    // Split into segments: maximal spans that start and end with an opposite-direction
    // strong char (neutrals inside are absorbed); everything else is base direction.
    let mut segs: Vec<(bool, &[char])> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if dir(chars[i]) == opposite {
            let mut end = i;
            let mut j = i;
            while j < chars.len() && dir(chars[j]) != if rtl { Dir::R } else { Dir::L } {
                if dir(chars[j]) == opposite {
                    end = j;
                }
                j += 1;
            }
            segs.push((true, &chars[i..=end]));
            i = end + 1;
        } else {
            let start = i;
            while i < chars.len() && dir(chars[i]) != opposite {
                i += 1;
            }
            segs.push((false, &chars[start..i]));
        }
    }
    let mut out = Vec::with_capacity(chars.len());
    if rtl {
        for (embedded, seg) in segs.iter().rev() {
            if *embedded {
                out.extend_from_slice(seg);
            } else {
                out.extend(seg.iter().rev().map(|&c| mirror(c)));
            }
        }
    } else {
        for (embedded, seg) in &segs {
            if *embedded {
                out.extend(reorder(seg, true));
            } else {
                out.extend_from_slice(seg);
            }
        }
    }
    out
}

/// Convert a logical string to the glyph sequence to draw left-to-right.
pub fn visual(s: &str) -> Cow<'_, str> {
    if !s.chars().any(|c| dir(c) == Dir::R) {
        return Cow::Borrowed(s);
    }
    let chars: Vec<char> = s.chars().collect();
    let shaped = shape(&chars);
    let rtl = shaped.iter().map(|&c| dir(c)).find(|d| *d != Dir::N) == Some(Dir::R);
    Cow::Owned(reorder(&shaped, rtl).into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_is_untouched() {
        assert_eq!(visual("Hello (world) 123"), "Hello (world) 123");
    }

    #[test]
    fn persian_word_is_shaped_and_reversed() {
        // "سلام": seen initial, lam-alef ligature (final), meem isolated.
        let v: Vec<u32> = visual("سلام").chars().map(|c| c as u32).collect();
        assert_eq!(v, vec![0xFEE1, 0xFEFC, 0xFEB3]);
    }

    #[test]
    fn mixed_rtl_base_keeps_latin_runs() {
        let v = visual("فیلم HD 1080");
        assert!(v.starts_with("HD 1080") || v.starts_with("1080"), "{v}");
        assert!(v.contains("HD 1080"));
    }

    #[test]
    fn mixed_ltr_base_reverses_rtl_run() {
        let v: String = visual("Song - آب").into();
        assert!(v.starts_with("Song - "));
    }
}
