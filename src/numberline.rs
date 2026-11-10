//! The number line: an optional ruler along the real axis, drawn onto the picture itself,
//! so the explorer, saved images and video frames all show the same one. Its ticks follow
//! the zoom: labelled steps of 1, 2 or 5 times a power of ten, with as many decimals as
//! the step needs, spaced so that the labels never touch. When the real axis is out of
//! view (or nearly), the line runs along the edge nearest it, dashed.

use crate::engine::Region;
use ab_glyph::{Font, FontRef, GlyphId, PxScale, ScaleFont};
use std::sync::OnceLock;

/// A tick: its column in pixels (fractional), and its label if it's a major tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    pub x: f64,
    pub label: Option<String>,
}

/// The ticks for the real parts `re.0` (at column 0) to `re.1` (at column `w - 1`), for
/// labels `char_w` pixels a character. The labelled steps are at least a tenth of the
/// width apart, and far enough apart for the longest label and two characters' space.
pub fn ticks(re: (f64, f64), w: usize, char_w: f64) -> Vec<Tick> {
    let span = re.1 - re.0;
    if !(span > 0.0 && span.is_finite()) || w < 2 {
        return Vec::new();
    }
    let px = (w - 1) as f64 / span; // pixels per unit
    let x_of = |v: f64| (v - re.0) * px;
    let mut e = (span / 100.0).log10().floor() as i32;
    for m in (0..60).map(|i| i % 3) {
        let step = [1.0, 2.0, 5.0][m] * 10f64.powi(e);
        let decimals = (-e).max(0) as usize;
        let ns = (re.0 / step).ceil() as i64..=(re.1 / step).floor() as i64;
        let label = |n: i64| format!("{:.*}", decimals, n as f64 * step);
        let longest = ns.clone().map(|n| label(n).chars().count()).max().unwrap_or(0);
        if step * px >= ((longest + 2) as f64 * char_w).max(w as f64 / 10.0) {
            // minor ticks: quarters of a step of 2, otherwise fifths, if they're not too close
            let parts = if m == 1 { 4 } else { 5 };
            let minor = step / parts as f64;
            let mut out: Vec<Tick> = ns.map(|n| Tick { x: x_of(n as f64 * step), label: Some(label(n)) }).collect();
            if minor * px >= 8.0 {
                let ks = (re.0 / minor).ceil() as i64..=(re.1 / minor).floor() as i64;
                out.extend(ks.filter(|k| k.rem_euclid(parts) != 0).map(|k| Tick { x: x_of(k as f64 * minor), label: None }));
            }
            return out;
        }
        if m == 2 {
            e += 1;
        }
    }
    Vec::new()
}

/// The font: Hack, the monospaced font that comes with egui.
fn font() -> &'static FontRef<'static> {
    static FONT: OnceLock<FontRef<'static>> = OnceLock::new();
    FONT.get_or_init(|| FontRef::try_from_slice(epaint_default_fonts::HACK_REGULAR).expect("the Hack font"))
}

/// How much of a band of rows the line covers: the white (`fg`) and the dark outline
/// around it (`halo`), each 0 to 1 per pixel.
struct Band {
    top: i64,
    w: usize,
    rows: usize,
    fg: Vec<f32>,
}

impl Band {
    /// Covers the rectangle from (x0, y0) to (x1, y1), in pixels (pixel (c, r) spans
    /// c..c+1 and r..r+1), antialiased at its edges.
    fn rect(&mut self, x0: f64, x1: f64, y0: f64, y1: f64) {
        let rows = (y0.floor() as i64).max(self.top)..(y1.ceil() as i64).min(self.top + self.rows as i64);
        let cols = (x0.floor().max(0.0) as usize)..(x1.ceil().max(0.0) as usize).min(self.w);
        for r in rows {
            let cy = (y1.min(r as f64 + 1.0) - y0.max(r as f64)).max(0.0);
            for c in cols.clone() {
                let cx = (x1.min(c as f64 + 1.0) - x0.max(c as f64)).max(0.0);
                let i = (r - self.top) as usize * self.w + c;
                self.fg[i] = self.fg[i].max((cx * cy) as f32);
            }
        }
    }

    /// Writes `text` with its left end at `x` and its baseline at `y`.
    fn text(&mut self, text: &str, x: f64, y: f64, size: f32) {
        let f = font();
        let scaled = f.as_scaled(PxScale::from(size));
        let mut pen = x as f32;
        for ch in text.chars() {
            // a proper minus sign, if the font has one
            let id = match ch {
                '-' if f.glyph_id('−') != GlyphId(0) => f.glyph_id('−'),
                _ => f.glyph_id(ch),
            };
            let glyph = id.with_scale_and_position(size, ab_glyph::point(pen, y as f32));
            pen += scaled.h_advance(id);
            let Some(outline) = f.outline_glyph(glyph) else { continue };
            let b = outline.px_bounds();
            outline.draw(|gx, gy, cov| {
                let (c, r) = (b.min.x as i64 + gx as i64, b.min.y as i64 + gy as i64);
                if c >= 0 && (c as usize) < self.w && r >= self.top && r < self.top + self.rows as i64 {
                    let i = (r - self.top) as usize * self.w + c as usize;
                    self.fg[i] = self.fg[i].max(cov.min(1.0));
                }
            });
        }
    }

    /// The outline: the coverage spread `r` pixels each way.
    fn halo(&self, r: usize) -> Vec<f32> {
        let (w, h) = (self.w, self.rows);
        let mut across = vec![0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let row = &self.fg[y * w + x.saturating_sub(r)..y * w + (x + r + 1).min(w)];
                across[y * w + x] = row.iter().copied().fold(0.0, f32::max);
            }
        }
        let mut out = vec![0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                out[y * w + x] = (y.saturating_sub(r)..(y + r + 1).min(h)).map(|yy| across[yy * w + x]).fold(0.0, f32::max);
            }
        }
        out
    }
}

/// Lays out the number line on a `w` x `h` picture of `region`, and calls `f` with each
/// pixel it touches: its index, and how much of the dark outline and of the white line
/// cover it.
fn cover(w: usize, h: usize, region: &Region, mut f: impl FnMut(usize, f32, f32)) {
    if w < 16 || h < 16 {
        return;
    }
    let size = (w.min(h) as f64 / 42.0).max(11.0);
    let thick = (size / 10.0).max(1.0);
    let halo = (size / 8.0).round().max(1.0) as usize;
    let (major, minor) = (size * 0.45, size * 0.22); // half-lengths
    let gap = size * 0.25;
    let f0 = font().as_scaled(PxScale::from(size as f32));
    let char_w = f0.h_advance(font().glyph_id('0')) as f64;
    let digit_h = font().outline_glyph(font().glyph_id('0').with_scale(size as f32)).map_or(size * 0.73, |o| o.px_bounds().height() as f64);

    // the row of the real axis, kept in from the edges so that the ticks fit
    let (im0, im1) = region.im;
    let axis = if im1 > im0 { im1 / (im1 - im0) * (h - 1) as f64 } else { f64::NAN };
    let pad = major + thick;
    let y = if axis.is_finite() { axis.clamp(pad, (h - 1) as f64 - pad) } else { (h - 1) as f64 - pad };
    let dashed = !((axis - y).abs() <= 0.5);
    let label_h = gap + digit_h;
    let below = y + major + label_h + halo as f64 <= (h - 1) as f64;

    let top = ((y - pad - label_h - halo as f64 * 2.0).floor() as i64).max(0);
    let bottom = ((y + pad + label_h + halo as f64 * 2.0).ceil() as i64 + 1).min(h as i64);
    let mut band = Band { top, w, rows: (bottom - top).max(0) as usize, fg: vec![0.0; w * (bottom - top).max(0) as usize] };
    // pixel centres are at whole numbers, rectangles' edges at halves
    let (yc, ht) = (y + 0.5, thick / 2.0);
    if dashed {
        let (dash, space) = (size * 0.5, size * 0.35);
        let mut x = 0.0;
        while x < w as f64 {
            band.rect(x, x + dash, yc - ht, yc + ht);
            x += dash + space;
        }
    } else {
        band.rect(0.0, w as f64, yc - ht, yc + ht);
    }
    for t in ticks(region.re, w, char_w) {
        let xc = t.x + 0.5;
        let len = if t.label.is_some() { major } else { minor };
        band.rect(xc - ht, xc + ht, yc - len, yc + len);
        if let Some(label) = t.label {
            let lw = label.chars().count() as f64 * char_w;
            let x0 = xc - lw / 2.0;
            if x0 >= halo as f64 && x0 + lw <= (w - halo) as f64 {
                let base = if below { yc + major + gap + digit_h } else { yc - major - gap };
                band.text(&label, x0, base, size as f32);
            }
        }
    }
    let outline = band.halo(halo);
    for (i, (&fg, &dark)) in band.fg.iter().zip(&outline).enumerate() {
        if dark > 0.0 {
            f(band.top as usize * w + i, dark, fg);
        }
    }
}

/// The colour of a pixel under the number line.
fn blend([r, g, b]: [u8; 3], dark: f32, fg: f32) -> [u8; 3] {
    let mix = |c: u8| {
        let c = c as f32 * (1.0 - 0.65 * dark);
        (c + (255.0 - c) * fg).round().clamp(0.0, 255.0) as u8
    };
    [mix(r), mix(g), mix(b)]
}

/// Draws the number line onto a `w` x `h` picture (packed RGB) of `region`.
pub fn draw(rgb: &mut [u8], w: usize, h: usize, region: &Region) {
    cover(w, h, region, |i, dark, fg| {
        let p = &mut rgb[3 * i..3 * i + 3];
        p.copy_from_slice(&blend([p[0], p[1], p[2]], dark, fg));
    });
}

/// Draws the number line onto a `w` x `h` picture of `region` in some other pixel format,
/// read with `get` and written with `make`.
pub fn draw_pixels<P: Copy>(pixels: &mut [P], w: usize, h: usize, region: &Region, get: impl Fn(P) -> [u8; 3], make: impl Fn([u8; 3]) -> P) {
    cover(w, h, region, |i, dark, fg| pixels[i] = make(blend(get(pixels[i]), dark, fg)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(re: (f64, f64), w: usize) -> Vec<String> {
        ticks(re, w, 10.0).into_iter().filter_map(|t| t.label).collect()
    }

    #[test]
    fn ticks_follow_the_zoom() {
        assert_eq!(labels((-2.0, 2.0), 800), ["-2.0", "-1.5", "-1.0", "-0.5", "0.0", "0.5", "1.0", "1.5", "2.0"]);
        // around e, a thousand times closer: more decimals, and still several labels
        let e = std::f64::consts::E;
        let near_e = labels((e - 0.002, e + 0.002), 800);
        assert!(near_e.len() >= 4 && near_e.iter().all(|l| l.starts_with("2.7") && l.len() == 6), "{near_e:?}");
        // labels never touch: at least the longest label and two spaces apart
        for hw in [1.0, 1e-3, 1e-6, 1e-9, 1e-12] {
            let ts: Vec<Tick> = ticks((e - hw, e + hw), 640, 10.0).into_iter().filter(|t| t.label.is_some()).collect();
            assert!(ts.len() >= 2, "hw {hw}: {ts:?}");
            let longest = ts.iter().map(|t| t.label.as_ref().unwrap().len()).max().unwrap();
            assert!(ts.windows(2).all(|p| p[1].x - p[0].x >= (longest + 2) as f64 * 10.0), "hw {hw}");
        }
    }

    #[test]
    fn ticks_sit_on_their_values() {
        for t in ticks((0.95, 1.35), 401, 8.0) {
            if let Some(l) = t.label {
                let v: f64 = l.parse().unwrap();
                assert!((t.x - (v - 0.95) * 1000.0).abs() < 1e-6, "{l} at {}", t.x);
            }
        }
    }

    #[test]
    fn draws_on_the_axis_or_along_the_edge() {
        let (w, h) = (320, 180);
        let lit_rows = |im: (f64, f64)| {
            let mut rgb = vec![0u8; w * h * 3];
            draw(&mut rgb, w, h, &Region { re: (-1.0, 1.0), im, width: w });
            (0..h).filter(|&r| (0..w).filter(|&c| rgb[3 * (r * w + c)] > 128).count() > w / 3).collect::<Vec<_>>()
        };
        let mid = (h - 1) as f64 / 2.0;
        assert!(lit_rows((-0.5, 0.5)).iter().all(|&r| (r as f64 - mid).abs() <= 2.0), "the line is across the middle");
        assert!(lit_rows((1.0, 2.0)).iter().all(|&r| r > h * 3 / 4), "the axis is below: the line runs along the bottom");
        assert!(lit_rows((-2.0, -1.0)).iter().all(|&r| r < h / 4), "the axis is above: along the top");
    }
}
