//! Text measurement and rasterization for the bill overlays.
//!
//! Ports the web generator's canvas text semantics: `fillText(text, x, y)`
//! draws with the BASELINE at y, `_fitFontToBox` walks the size down from a
//! start value using measured width and an approximated height of size×1.2.
//! Fonts: DejaVu Sans Mono stands in for Courier New, DejaVu Sans Condensed
//! for Arial Narrow (both embeddable under the Bitstream Vera license).

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgb, RgbImage};
use std::sync::OnceLock;

static MONO_BYTES: &[u8] = include_bytes!("../assets/DejaVuSansMono.ttf");
static CONDENSED_BYTES: &[u8] = include_bytes!("../assets/DejaVuSansCondensed.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillFont {
    /// Courier New role: WIF, segwit address, timestamp.
    Mono,
    /// Arial Narrow / sans role: taproot address, labels, banner.
    Condensed,
}

fn font_ref(which: BillFont) -> &'static FontRef<'static> {
    static MONO: OnceLock<FontRef<'static>> = OnceLock::new();
    static CONDENSED: OnceLock<FontRef<'static>> = OnceLock::new();
    match which {
        BillFont::Mono => {
            MONO.get_or_init(|| FontRef::try_from_slice(MONO_BYTES).expect("embedded font parses"))
        }
        BillFont::Condensed => CONDENSED.get_or_init(|| {
            FontRef::try_from_slice(CONDENSED_BYTES).expect("embedded font parses")
        }),
    }
}

/// Advance width of `text` at `px`, kerning included (ctx.measureText twin).
pub fn measure(which: BillFont, px: f32, text: &str) -> f32 {
    let font = font_ref(which).as_scaled(PxScale::from(px));
    let mut width = 0.0;
    let mut prev = None;
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        if let Some(p) = prev {
            width += font.kern(p, id);
        }
        width += font.h_advance(id);
        prev = Some(id);
    }
    width
}

/// `_fitFontToBox` twin: largest integer size (descending from `start`, floor
/// 6) whose measured width fits `max_w` and whose size×1.2 fits `max_h`.
/// Returns (size, measured_width).
pub fn fit_to_box(which: BillFont, text: &str, max_w: f32, max_h: f32, start: u32) -> (f32, f32) {
    for size in (6..=start).rev() {
        let size = size as f32;
        let tw = measure(which, size, text);
        if tw <= max_w && size * 1.2 <= max_h {
            return (size, tw);
        }
    }
    (6.0, measure(which, 6.0, text))
}

fn blend(img: &mut RgbImage, x: i64, y: i64, color: Rgb<u8>, coverage: f32) {
    if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 {
        return;
    }
    let c = coverage.clamp(0.0, 1.0);
    let px = img.get_pixel_mut(x as u32, y as u32);
    for i in 0..3 {
        px.0[i] = (px.0[i] as f32 * (1.0 - c) + color.0[i] as f32 * c).round() as u8;
    }
}

/// fillText twin: draw `text` with its baseline at (`x`, `baseline_y`).
pub fn draw_text(
    img: &mut RgbImage,
    which: BillFont,
    px: f32,
    x: f32,
    baseline_y: f32,
    color: Rgb<u8>,
    text: &str,
) {
    let font = font_ref(which);
    let scaled = font.as_scaled(PxScale::from(px));
    let mut caret = x;
    let mut prev = None;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(p) = prev {
            caret += scaled.kern(p, id);
        }
        let glyph = id.with_scale_and_position(PxScale::from(px), ab_glyph::point(caret, baseline_y));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, cov| {
                blend(
                    img,
                    bounds.min.x as i64 + gx as i64,
                    bounds.min.y as i64 + gy as i64,
                    color,
                    cov,
                );
            });
        }
        caret += scaled.h_advance(id);
        prev = Some(id);
    }
}

/// Canvas `translate(px,py); rotate(-90deg); fillText(text, 0, baseline)`
/// twin: vertical text reading bottom-to-top. Local glyph coords (lx, ly)
/// land on the page at (px + ly, py - lx).
pub fn draw_text_rotated_ccw(
    img: &mut RgbImage,
    which: BillFont,
    px_size: f32,
    px: f32,
    py: f32,
    baseline: f32,
    color: Rgb<u8>,
    text: &str,
) {
    let font = font_ref(which);
    let scaled = font.as_scaled(PxScale::from(px_size));
    let mut caret = 0.0f32;
    let mut prev = None;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(p) = prev {
            caret += scaled.kern(p, id);
        }
        let glyph = id.with_scale_and_position(PxScale::from(px_size), ab_glyph::point(caret, baseline));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, cov| {
                let lx = bounds.min.x + gx as f32;
                let ly = bounds.min.y + gy as f32;
                blend(
                    img,
                    (px + ly).round() as i64,
                    (py - lx).round() as i64,
                    color,
                    cov,
                );
            });
        }
        caret += scaled.h_advance(id);
        prev = Some(id);
    }
}
