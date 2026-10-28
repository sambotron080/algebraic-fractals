//! Shared by the explorer and the video maker, without the window: views, saved
//! locations, number parsing and formatting, and the auto order / max coefficient.
//! (Carried over from the old programs' src/bin/common/mod.rs, minus its egui widgets;
//! saved places became locations in groups.)

use crate::colour::{ColourBy, Colormap, Style};
use crate::density::{dot, pixels, poly_counts, Predictor};
use crate::engine::{CoefSet, Coefs, Plan, Region};
use std::path::PathBuf;

/// (sparse sets like the powers of 2 need large values to have many members)
pub const MAX_COEF: i32 = 1_000_000;

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

/// A saved location: a view with a name. Only the view is kept: going to a location
/// keeps the search settings and colouring you have.
#[derive(Clone, PartialEq, Debug)]
pub struct Location {
    pub name: String,
    pub view: View,
}

/// Locations filed together, in order. A group is usually the waypoints of one zoom
/// video.
#[derive(Clone, PartialEq, Debug)]
pub struct Group {
    pub name: String,
    pub locations: Vec<Location>,
}

/// The group new locations go into unless another is chosen. It always exists.
pub const UNSORTED: &str = "Unsorted";
/// The group the old programs' saved places are brought into.
pub const OLD_PLACES_GROUP: &str = "Old places";

/// Where saved locations, video projects and settings live: %APPDATA%\AlgebraicFractals
/// on Windows, ~/Library/Application Support/AlgebraicFractals on macOS, and
/// ~/.config/AlgebraicFractals elsewhere.
pub fn settings_dir() -> PathBuf {
    let var = |v: &str| std::env::var_os(v).map(PathBuf::from);
    let base = if cfg!(windows) {
        var("APPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|h| h.join(".config")))
    };
    base.map(|d| d.join("AlgebraicFractals")).unwrap_or_else(|| PathBuf::from("."))
}

pub fn locations_path() -> PathBuf {
    settings_dir().join("locations.tsv")
}

/// The old programs' saved places.
pub fn old_places_path() -> PathBuf {
    settings_dir().join("places.tsv")
}

/// Tabs and other control characters would break the line format.
fn one_line(s: &str) -> String {
    s.chars().map(|ch| if ch.is_control() { ' ' } else { ch }).collect()
}

impl Location {
    /// Tab-separated: group, name, centre re, im, half-width.
    pub fn to_line(&self, group: &str) -> String {
        let v = self.view;
        format!("{}\t{}\t{}\t{}\t{}", one_line(group), one_line(&self.name), v.cr, v.ci, v.hw)
    }

    /// The group's name and the location, from a line written by `to_line`.
    pub fn from_line(line: &str) -> Option<(String, Location)> {
        let f: Vec<&str> = line.split('\t').collect();
        let num = |i: usize| f.get(i)?.trim().parse::<f64>().ok().filter(|x| x.is_finite());
        let view = View { cr: num(2)?, ci: num(3)?, hw: num(4)?.abs() };
        (view.hw > 0.0).then(|| (f[0].to_string(), Location { name: f[1].to_string(), view }))
    }
}

/// The groups from the lines of locations.tsv: a group's locations in the file's order,
/// the groups in the order they first appear. A line holding only a group's name keeps
/// an empty group.
pub fn groups_from_text(text: &str) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let (name, location) = match Location::from_line(line) {
            Some((g, l)) => (g, Some(l)),
            None if !line.contains('\t') => (line.to_string(), None),
            None => continue,
        };
        let i = match groups.iter().position(|g| g.name == name) {
            Some(i) => i,
            None => {
                groups.push(Group { name, locations: Vec::new() });
                groups.len() - 1
            }
        };
        groups[i].locations.extend(location);
    }
    groups
}

pub fn groups_to_text(groups: &[Group]) -> String {
    let mut s = String::from("# Algebraic Fractals saved locations: group, name, re, im, half-width\n");
    for g in groups {
        if g.locations.is_empty() {
            s += &one_line(&g.name);
            s.push('\n');
        }
        for l in &g.locations {
            s += &l.to_line(&g.name);
            s.push('\n');
        }
    }
    s
}

/// The saved locations, by group, with Unsorted first. Until there's a locations.tsv,
/// the old programs' saved places are listed too, in the group `OLD_PLACES_GROUP`, last
/// (their order, coefficients and colouring are left behind, and so are their saved
/// renders).
pub fn load_groups() -> Vec<Group> {
    let read = |path: PathBuf| std::fs::read_to_string(path).ok();
    let mut groups = match read(locations_path()) {
        Some(s) => groups_from_text(&s),
        None => old_places_group(&read(old_places_path()).unwrap_or_default()).into_iter().collect(),
    };
    match groups.iter().position(|g| g.name == UNSORTED) {
        Some(i) => {
            let g = groups.remove(i);
            groups.insert(0, g);
        }
        None => groups.insert(0, Group { name: UNSORTED.into(), locations: Vec::new() }),
    }
    groups
}

/// The old programs' saved places (the text of their places.tsv) as the group
/// `OLD_PLACES_GROUP`, if there are any.
pub fn old_places_group(text: &str) -> Option<Group> {
    let locations: Vec<Location> =
        text.lines().filter(|l| !l.starts_with('#')).filter_map(OldPlace::from_line).map(|p| Location { name: p.name, view: p.view }).collect();
    (!locations.is_empty()).then(|| Group { name: OLD_PLACES_GROUP.into(), locations })
}

pub fn store_groups(groups: &[Group]) -> std::io::Result<()> {
    std::fs::create_dir_all(settings_dir())?;
    std::fs::write(locations_path(), groups_to_text(groups))
}

/// A line of the old programs' places.tsv, or a waypoint line of their saved video
/// paths (.path files): a view with an order, coefficients and colouring. Only read, to
/// bring old places and paths in.
pub struct OldPlace {
    pub name: String,
    pub view: View,
    pub order: usize,
    pub coefs: Coefs,
    pub style: Style,
}

impl OldPlace {
    /// Tab-separated: name, centre re, im, half-width, order, set, (unused: once a
    /// minimum |c|, always 1), max, zero,
    /// then the display style: colour by, n, max %, colormap, invert, shade; then zero_root;
    /// then min %, gamma, cycles, shift, brightness, contrast, saturation; then log scale
    /// for length/height; then smooth stars. (Columns were only ever added at the end, and
    /// missing ones take their defaults.)
    pub fn from_line(line: &str) -> Option<OldPlace> {
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
        Some(OldPlace { name: f[0].to_string(), view, order: int(4)?.clamp(1, 16) as usize, coefs, style })
    }
}

/// Multi-line summary of a view for hover text.
pub fn view_info(v: &View) -> String {
    format!("centre {}\nhalf-width {:.4e}", fmt_complex(v.cr, v.ci), v.hw)
}

/// A name with the characters that file names can't contain replaced by _.
pub fn file_safe(name: &str) -> String {
    name.chars().map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect()
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

/// "Algebraic Fractals" in the user's folder for `kind` (Pictures, Videos, or Movies on
/// macOS).
fn default_dir(kind: &str) -> String {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    home.map_or_else(|| PathBuf::from("."), |h| h.join(kind).join("Algebraic Fractals")).display().to_string()
}

/// Where images are saved unless another folder is chosen.
pub fn default_images_dir() -> String {
    default_dir("Pictures")
}

/// Where videos are saved unless another folder is chosen.
pub fn default_videos_dir() -> String {
    default_dir(if cfg!(target_os = "macos") { "Movies" } else { "Videos" })
}

/// The first of `stem`, `stem_2`, `stem_3`, ... for which `taken` is false, so that
/// nothing is ever overwritten.
pub fn unused_stem(stem: &str, taken: impl Fn(&str) -> bool) -> String {
    (1..).map(|i| if i == 1 { stem.to_string() } else { format!("{stem}_{i}") }).find(|s| !taken(s)).unwrap()
}

/// "3x² - 2x + 1", for a quadratic's coefficients [a, b, c] (a x² + b x + c).
pub fn quadratic_text([a, b, c]: [i64; 3]) -> String {
    let term = |k: i64, x: &str, first: bool| match (k, first) {
        (0, _) => String::new(),
        (k, true) => format!("{}{x}", if k == 1 && !x.is_empty() { String::new() } else { k.to_string() }),
        (k, false) => format!(" {} {}{x}", if k < 0 { '-' } else { '+' }, if k.abs() == 1 && !x.is_empty() { String::new() } else { k.abs().to_string() }),
    };
    format!("{}{}{}", term(a, "x²", true), term(b, "x", false), term(c, "", false))
}

/// "3x^4 - 2x^2 + 1": highest power first, coefficients of ±1 left out (except the constant).
pub fn poly_string(c: &[i32]) -> String {
    let mut s = String::new();
    for (i, &a) in c.iter().enumerate().rev() {
        if a == 0 {
            continue;
        }
        if s.is_empty() {
            if a < 0 {
                s.push('-');
            }
        } else {
            s += if a < 0 { " - " } else { " + " };
        }
        let m = a.unsigned_abs();
        if i == 0 || m != 1 {
            s += &m.to_string();
        }
        if i >= 1 {
            s.push('x');
        }
        if i >= 2 {
            s += &format!("^{i}");
        }
    }
    if s.is_empty() {
        "0".into()
    } else {
        s
    }
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
    }

    #[test]
    fn location_lines() {
        let v = View { cr: -0.1, ci: 1.0 / 3.0, hw: 2.5e-9 };
        let l = Location { name: "spiral\tarm".into(), view: v };
        let back = Location::from_line(&l.to_line("deep\tzoom")).unwrap();
        assert_eq!(back, ("deep zoom".to_string(), Location { name: "spiral arm".into(), view: v }));
        assert_eq!(Location::from_line("g\tn\t0\t0\t0"), None);
        assert_eq!(Location::from_line("g\tn\t0\tx\t1"), None);
        // groups keep their order and their locations' order, and empty ones survive
        let at = |hw: f64| View { cr: 0.25, ci: -0.5, hw };
        let groups = vec![
            Group { name: UNSORTED.into(), locations: vec![Location { name: "a".into(), view: at(1.0) }] },
            Group { name: "empty".into(), locations: vec![] },
            Group { name: "dive".into(), locations: vec![Location { name: "b".into(), view: at(0.1) }, Location { name: "c".into(), view: at(1e-12) }] },
        ];
        assert_eq!(groups_from_text(&groups_to_text(&groups)), groups);
        // a group's lines needn't be together
        let mixed = "g\ta\t0\t0\t1\nh\tb\t0\t0\t1\ng\tc\t0\t0\t1\nbad\tline\n";
        let g = groups_from_text(mixed);
        assert_eq!(g.iter().map(|g| (g.name.as_str(), g.locations.len())).collect::<Vec<_>>(), [("g", 2), ("h", 1)]);
    }

    /// A places.tsv written by the old programs comes in as the group Old places.
    #[test]
    fn old_places_come_in() {
        let g = old_places_group(include_str!("../tests/data/places.tsv")).unwrap();
        assert_eq!(g.name, OLD_PLACES_GROUP);
        let names: Vec<&str> = g.locations.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["spiral", "lace", "deep flower"]);
        assert_eq!(g.locations[2].view, View { cr: 1.0 / 3.0, ci: 0.4714045207910317, hw: 2.4e-8 });
        assert_eq!(old_places_group("# nothing\n"), None);
    }

    /// Auto lands near the target root density, and zooming in never lowers it.
    #[test]
    fn auto_density() {
        let mut p = Predictor::new();
        let coefs = Coefs { zero_root: true, ..Coefs::ints(12) };
        let mut last = (0, 0);
        for hw in [2.0, 0.5, 0.1, 0.02, 0.004] {
            let region = Region::square(0.4, 0.85, hw, 512);
            let (_, m, note_m) = choose_auto(&mut p, 4, coefs, &region, 4.0, false, f64::INFINITY);
            let (o, _, _) = choose_auto(&mut p, 4, coefs, &region, 4.0, true, f64::INFINITY);
            let dm = p.per_pixel(4, &m, &region) * 100.0;
            // with a free max coefficient, the target is hit closely
            if note_m.is_empty() && m.max > 1 {
                let below = p.per_pixel(4, &Coefs { max: m.max - 1, ..m }, &region) * 100.0;
                assert!((4.0 * 0.6..=4.0 / 0.6).contains(&dm) || below < 4.0, "hw {hw}: {dm}");
            }
            assert!(m.max >= last.0 && o >= last.1, "zooming in never lowers the auto setting");
            last = (m.max, o);
        }
    }

    /// Lines written by the old programs: the oldest (9 columns) and the newest (25).
    #[test]
    fn old_place_lines() {
        let p = OldPlace::from_line("spiral\t1.9\t0.19\t0.16\t4\tints\t1\t32\t1").unwrap();
        assert_eq!((p.name.as_str(), p.view, p.order), ("spiral", View { cr: 1.9, ci: 0.19, hw: 0.16 }, 4));
        assert_eq!(p.coefs, Coefs { set: CoefSet::Ints, max: 32, zero: true, zero_root: false });
        assert_eq!(p.style, Style::default());
        let line = "lace\t-0.5\t0.25\t0.001\t5\tpow2\t1\t64\t0\tlog\t2\t99\tinferno\t1\t0\t1\t5\t1.5\t2\t0.25\t0.1\t1.2\t0.8\t1\t0";
        let p = OldPlace::from_line(line).unwrap();
        assert_eq!(p.coefs, Coefs { set: CoefSet::Powers2, max: 64, zero: false, zero_root: true });
        let s = p.style;
        assert_eq!((s.by, s.n, s.high, s.cmap, s.invert, s.shade), (ColourBy::Log, 2.0, 99.0, Colormap::Inferno, true, false));
        assert_eq!((s.low, s.gamma, s.cycles, s.shift, s.brightness, s.contrast, s.saturation), (5.0, 1.5, 2.0, 0.25, 0.1, 1.2, 0.8));
        assert_eq!((s.log_poly, s.smooth), (true, false));
    }
}
