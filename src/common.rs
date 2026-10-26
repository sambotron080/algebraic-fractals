//! Shared by the explorer and the video maker, without the window: views, saved
//! places, number parsing and formatting, and the auto order / max coefficient.
//! (Carried over from the old programs' src/bin/common/mod.rs, minus its egui widgets.)

use crate::colour::{ColourBy, Colormap, Style};
use crate::density::{dot, pixels, poly_counts, Predictor};
use crate::engine::{CoefSet, Coefs, Plan, Region};
use std::path::PathBuf;

/// (sparse sets like the powers of 2 need large values to have many members)
pub const MAX_COEF: i32 = 1_000_000;
/// The explorer's auto settings never choose more work per pixel than this (about
/// 7 s at 256 x 256 and 2 minutes at 1024 x 1024 on 12 cores).
pub const AUTO_MAX_STEPS: f64 = 2e4;

/// A square region of the complex plane.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct View {
    pub cr: f64,
    pub ci: f64,
    pub hw: f64, // half-width
}

impl View {
    pub const HOME: View = View { cr: 0.0, ci: 0.0, hw: 2.0 };

    /// Whether `r` is a render of this view (a place's saved render could be left over
    /// from an older place of the same name).
    pub fn shown_by(&self, r: &crate::engine::Render) -> bool {
        let (re, im) = (&r.re_axis, &r.im_axis);
        let near = |a: f64, b: f64| (a - b).abs() <= 1e-6 * self.hw;
        r.width >= 2
            && r.height >= 2
            && near((re[0] + re[r.width - 1]) / 2.0, self.cr)
            && near((im[0] + im[r.height - 1]) / 2.0, self.ci)
            && near((re[r.width - 1] - re[0]) / 2.0, self.hw)
    }
}

/// A named, saved view. Going to a place uses only its centre and half-width (and its
/// saved render, if it has one: see `place_render_path`); the order, coefficients and
/// colouring are kept with it because saved video paths store their waypoints in the
/// same format, and use them.
pub struct Place {
    pub name: String,
    pub view: View,
    pub order: usize,
    pub coefs: Coefs,
    pub style: Style,
}

/// Saved places live in places.tsv, one per line, in %APPDATA%\AlgebraicFractals on
/// Windows, ~/Library/Application Support/AlgebraicFractals on macOS, and
/// ~/.config/AlgebraicFractals elsewhere.
pub fn places_path() -> PathBuf {
    let var = |v: &str| std::env::var_os(v).map(PathBuf::from);
    let base = if cfg!(windows) {
        var("APPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|h| h.join(".config")))
    };
    base.map(|d| d.join("AlgebraicFractals")).unwrap_or_else(|| PathBuf::from(".")).join("places.tsv")
}

impl Place {
    /// Tab-separated: name, centre re, im, half-width, order, set, (unused: once a
    /// minimum |c|, now always 1), max, zero,
    /// then the display style: colour by, n, max %, colormap, invert, shade; then zero_root;
    /// then min %, gamma, cycles, shift, brightness, contrast, saturation; then log scale
    /// for length/height; then smooth stars. (Columns were only ever added at the end, so
    /// older files still load.)
    pub fn to_line(&self) -> String {
        let (v, c, s) = (self.view, self.coefs, self.style);
        let name: String = self.name.chars().map(|ch| if ch.is_control() { ' ' } else { ch }).collect();
        format!(
            "{name}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            v.cr, v.ci, v.hw, self.order, c.set.name(), 1, c.max, c.zero as u8,
            s.by.short(), s.n, s.high, s.cmap.name(), s.invert as u8, s.shade as u8, c.zero_root as u8,
            s.low, s.gamma, s.cycles, s.shift, s.brightness, s.contrast, s.saturation, s.log_poly as u8, s.smooth as u8
        )
    }

    pub fn from_line(line: &str) -> Option<Place> {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 9 {
            return None;
        }
        let num = |i: usize| f.get(i)?.trim().parse::<f64>().ok().filter(|x| x.is_finite());
        let int = |i: usize| f[i].trim().parse::<i32>().ok();
        let flag = |i: usize| f.get(i).map(|s| s.trim() == "1");
        let coefs = Coefs {
            set: CoefSet::ALL.into_iter().find(|s| s.name() == f[5].trim())?,
            max: int(7)?.clamp(1, MAX_COEF),
            zero: flag(8)?,
            zero_root: flag(15).unwrap_or(false), // added later; older files have no column 15
        };
        let d = Style::default();
        let style = Style {
            by: f.get(9).and_then(|t| ColourBy::ALL.into_iter().find(|b| b.short() == t.trim())).unwrap_or(d.by),
            n: num(10).unwrap_or(d.n),
            high: num(11).unwrap_or(d.high),
            cmap: f.get(12).and_then(|t| Colormap::ALL.into_iter().find(|c| c.name() == t.trim())).unwrap_or(d.cmap),
            invert: flag(13).unwrap_or(d.invert),
            shade: flag(14).unwrap_or(d.shade),
            log_poly: flag(23).unwrap_or(d.log_poly),
            smooth: flag(24).unwrap_or(d.smooth),
            low: num(16).unwrap_or(d.low),
            gamma: num(17).unwrap_or(d.gamma),
            cycles: num(18).unwrap_or(d.cycles),
            shift: num(19).unwrap_or(d.shift),
            brightness: num(20).unwrap_or(d.brightness),
            contrast: num(21).unwrap_or(d.contrast),
            saturation: num(22).unwrap_or(d.saturation),
        };
        let view = View { cr: num(1)?, ci: num(2)?, hw: num(3)?.abs() };
        Some(Place { name: f[0].to_string(), view, order: int(4)?.clamp(1, 16) as usize, coefs, style })
    }

    /// Multi-line summary for hover text (it reads the saved render's header, if any).
    pub fn info(&self) -> String {
        let mut s = format!("centre {}\nhalf-width {:.4e}", fmt_complex(self.view.cr, self.view.ci), self.view.hw);
        if let Ok(h) = crate::algf::read_header(place_render_path(&self.name)) {
            s += &format!("\nsaved render: {} x {}, order {}, {}", h.width, h.height, h.order, h.coefs.describe());
        }
        s
    }
}

/// Where a saved place's render is kept: a "renders" folder next to the saved places,
/// one .algf file per place (see crate::algf).
pub fn place_render_path(name: &str) -> PathBuf {
    places_path().with_file_name("renders").join(format!("{}.algf", file_safe(name)))
}

/// A name with the characters that file names can't contain replaced by _.
pub fn file_safe(name: &str) -> String {
    name.chars().map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect()
}

pub fn load_places() -> Vec<Place> {
    std::fs::read_to_string(places_path())
        .map(|s| s.lines().filter(|l| !l.starts_with('#')).filter_map(Place::from_line).collect())
        .unwrap_or_default()
}

pub fn store_places(places: &[Place]) -> std::io::Result<()> {
    let path = places_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut s = String::from("# Algebraic Fractals saved places: name, re, im, half-width, order, set, (unused), max, zero, colour by, n, max %, colormap, invert, shade, c0 = 0 allowed, min %, gamma, cycles, shift, brightness, contrast, saturation, log length/height\n");
    for p in places {
        s += &p.to_line();
        s.push('\n');
    }
    std::fs::write(path, s)
}

/// "0.5+0.8i", "-1.2 - 3e-4i", "2i", "-i", "0.3" or "0.5, 0.8" -> (re, im)
pub fn parse_complex(s: &str) -> Option<(f64, f64)> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().replace('j', "i").replace("*i", "i");
    let fin = |t: &str| t.parse::<f64>().ok().filter(|x| x.is_finite());
    if let Some((a, b)) = s.split_once(',') {
        return Some((fin(a)?, fin(b)?));
    }
    let Some(body) = s.strip_suffix('i') else { return fin(&s).map(|r| (r, 0.0)) };
    // the imaginary part starts at the last sign that isn't the leading one or an exponent's
    let b = body.as_bytes();
    let k = (1..b.len()).rev().find(|&k| (b[k] == b'+' || b[k] == b'-') && !matches!(b[k - 1], b'e' | b'E'));
    let (re, im) = match k {
        Some(k) => (&body[..k], &body[k..]),
        None => ("", body),
    };
    let im = match im {
        "" | "+" => 1.0,
        "-" => -1.0,
        t => fin(t)?,
    };
    Some((if re.is_empty() { 0.0 } else { fin(re)? }, im))
}

/// Shortest exact form, e.g. "0.5-0.25i", for the centre text boxes.
pub fn complex_text(re: f64, im: f64) -> String {
    format!("{re}{}{}i", if im.is_sign_negative() { '-' } else { '+' }, im.abs())
}

/// Fixed-width form for display, e.g. "+0.500000000000 + 0.250000000000i".
pub fn fmt_complex(re: f64, im: f64) -> String {
    format!("{re:+.12} {} {:.12}i", if im < 0.0 { '-' } else { '+' }, im.abs())
}

/// A positive number to 2 significant figures, e.g. 0.034, 4.1, 230
pub fn sig(x: f64) -> String {
    if !(x > 0.0) {
        return "0".into();
    }
    let decimals = (1.0 - x.log10().floor()).max(0.0) as usize;
    format!("{x:.decimals$}")
}

/// Looks for the project's Results folder above the executable, else uses the current
/// folder (or the home folder, for an app started from the macOS Finder, which starts in /).
pub fn default_save_dir() -> String {
    let mut dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()));
    while let Some(d) = dir {
        if d.join("Results").is_dir() {
            return d.join("Results").display().to_string();
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
    let cwd = std::env::current_dir().ok().filter(|d| d.parent().is_some());
    cwd.or_else(|| std::env::var_os("HOME").map(PathBuf::from)).map_or_else(|| ".".into(), |d| d.display().to_string())
}

/// Chooses the order (`auto_order`) or else the max coefficient whose predicted density
/// of algebraic numbers in `region` is closest to `target` per 100 square pixels, keeping
/// the other settings and never needing more than `max_steps` per pixel. Returns the
/// order, the coefficients, and why the target couldn't be reached, if it couldn't.
pub fn choose_auto(
    p: &mut Predictor,
    order: usize,
    coefs: Coefs,
    region: &Region,
    target: f64,
    auto_order: bool,
    max_steps: f64,
) -> (usize, Coefs, String) {
    let target = target / 100.0;
    let fits = |order: usize, c: &Coefs| Plan::new(order, c, false).is_ok_and(|p| p.steps() <= max_steps);
    let miss = |dens: f64| (dens.max(1e-300) / target).ln().abs();
    let mut note = String::new();

    if auto_order {
        let mut best: Option<(usize, f64)> = None;
        for o in 1..=16 {
            if !fits(o, &coefs) {
                if best.is_some_and(|(_, d)| d < target) {
                    note = format!("order {o} would get closer, but is too slow for auto");
                }
                break;
            }
            let dens = p.per_pixel(o, &coefs, region);
            if best.map_or(true, |(_, d)| miss(dens) < miss(d)) {
                best = Some((o, dens));
            }
            if dens >= target {
                break; // higher orders only get denser
            }
        }
        return (best.map_or(order, |(o, _)| o), coefs, note);
    }

    let with = |m: i32| Coefs { max: m, ..coefs };
    let lo = 1;
    if !fits(order, &with(lo)) {
        return (order, coefs, "too slow for auto even with the smallest max coefficient".into());
    }
    // the largest max coefficient that isn't too slow
    let mut cap = MAX_COEF;
    if !fits(order, &with(cap)) {
        let (mut a, mut b) = (lo, cap); // fits(a), !fits(b)
        while b - a > 1 {
            let mid = (a + b) / 2;
            if fits(order, &with(mid)) { a = mid } else { b = mid }
        }
        cap = a;
    }
    // The density's shape is sampled with one max coefficient but barely depends on it,
    // so search with the current one, then refine with the result.
    let mut m = coefs.max.clamp(lo, cap);
    let px = pixels(region);
    for _ in 0..3 {
        let shape = p.shape(order, &with(m), region);
        let dens = |m: i32| dot(&shape, &poly_counts(order, &with(m))) / px;
        note.clear();
        let next = if dens(cap) < target {
            if cap < MAX_COEF {
                let why = if max_steps.is_finite() { "is too slow for auto" } else { "has too many polynomials to search" };
                note = format!("a max coefficient above {cap} would get closer, but {why}");
            }
            cap
        } else if dens(lo) >= target {
            lo
        } else {
            let (mut a, mut b) = (lo, cap); // dens(a) < target <= dens(b)
            while b - a > 1 {
                let mid = (a + b) / 2;
                if dens(mid) < target { a = mid } else { b = mid }
            }
            if miss(dens(a)) < miss(dens(b)) { a } else { b }
        };
        if next == m {
            break;
        }
        m = next;
    }
    (order, with(m), note)
}

/// "0, ±1, ±2, ±3, ±5, ±8" (shortened when long)
pub fn allowed_text(c: &Coefs) -> String {
    let mags: Vec<i32> = c.values().into_iter().filter(|&v| v > 0).collect();
    let shown: Vec<String> = if mags.len() > 10 {
        let mut v: Vec<String> = mags[..6].iter().map(|m| format!("±{m}")).collect();
        v.push(format!("... ±{} ({} values)", mags[mags.len() - 1], mags.len()));
        v
    } else {
        mags.iter().map(|m| format!("±{m}")).collect()
    };
    let mut s = if c.zero { String::from("0, ") } else { String::new() };
    s += &shown.join(", ");
    if mags.is_empty() {
        s = "nothing".into();
    }
    s + if c.zero && !c.zero_root { "  (c0 is never 0)" } else { "" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complex_parsing() {
        assert_eq!(parse_complex("0.5+0.8i"), Some((0.5, 0.8)));
        assert_eq!(parse_complex(" -1.2 - 3e-4i "), Some((-1.2, -3e-4)));
        assert_eq!(parse_complex("1e-5i"), Some((0.0, 1e-5)));
        assert_eq!(parse_complex("2i"), Some((0.0, 2.0)));
        assert_eq!(parse_complex("-i"), Some((0.0, -1.0)));
        assert_eq!(parse_complex("1-i"), Some((1.0, -1.0)));
        assert_eq!(parse_complex("0.3"), Some((0.3, 0.0)));
        assert_eq!(parse_complex("0.5, 0.8"), Some((0.5, 0.8)));
        assert_eq!(parse_complex("1.5E+2-2j"), Some((150.0, -2.0)));
        assert_eq!(parse_complex("abc"), None);
        let v = View { cr: -0.1, ci: 1.0 / 3.0, hw: 2.0 };
        assert_eq!(parse_complex(&complex_text(v.cr, v.ci)), Some((v.cr, v.ci)));
        let p = Place { name: "spiral".into(), view: v, order: 5, coefs: Coefs { set: CoefSet::Powers2, max: 64, zero: false, zero_root: true }, style: Style::default() };
        let q = Place::from_line(&p.to_line()).unwrap();
        assert_eq!((q.name, q.view, q.order, q.coefs, q.style), (p.name, p.view, p.order, p.coefs, p.style));
    }
}
