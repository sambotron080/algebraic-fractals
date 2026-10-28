//! What a video's frames show (`Frames`), how they're coloured (`Colouring`), the
//! automatic max coefficients at the waypoints (`auto_maxes`), and the time each frame
//! takes to render (`Frames::frame_secs`).

use super::reuse::{self, rung_between, Geom, PatchPlan};
use super::{cross_fade, downsample, fade_between, frame_region, frame_width, held_scale, max_at, ZoomPath};
use crate::colour::{colourise_with, ColourBy, Levels, Scale, Stats, Style};
use crate::common::{choose_auto, View, MAX_COEF};
use crate::density::Predictor;
use crate::engine::{work_per_pixel, Coefs, Plan, Region, Render};
use std::collections::HashMap;

/// Frame heights offered; the width follows from the shape.
pub const HEIGHTS: [usize; 6] = [360, 480, 720, 1080, 1440, 2160];
pub const FPS: [u32; 5] = [24, 25, 30, 50, 60];
/// (name, H.264 quantiser, rough bits per pixel per frame for star fields, measured)
pub const QUALITY: [(&str, u8, f64); 4] = [("good", 26, 1.5), ("high", 20, 2.3), ("very high", 16, 2.9), ("near lossless", 10, 3.7)];
/// Smoothing renders this many times more finely in each direction, then averages.
pub const SMOOTHING: [(usize, &str); 3] = [(1, "none"), (2, "2 x 2 (4x slower)"), (3, "3 x 3 (9x slower)")];
/// Renders above this many pixels (counting smoothing) would need too much memory.
pub const MAX_RENDER_PIXELS: usize = 30_000_000;
/// When reusing renders, a patch has at most this many times a frame's rendering pixels.
pub const PATCH_FRAMES: usize = 6;
/// When reusing renders, the steps the max coefficient moves in: (name, ln of how many
/// times as many polynomials each step allows), so 13%, 28% or 65% more.
pub const COEF_STEPS: [(&str, f64); 3] = [("fine", 0.125), ("medium", 0.25), ("coarse", 0.5)];

/// One render a frame needs: max coefficient, sample points (real axis, imaginary axis,
/// top first), and weight in the frame's cross-fade.
pub type Job = (i32, Vec<f64>, Vec<f64>, f64);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Wide,
    Square,
}

impl Shape {
    pub const ALL: [Shape; 2] = [Shape::Wide, Shape::Square];

    pub fn ratio(self) -> f64 {
        match self {
            Shape::Wide => 16.0 / 9.0,
            Shape::Square => 1.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Shape::Wide => "16:9 (widescreen)",
            Shape::Square => "square",
        }
    }

    /// As saved in a project file.
    pub fn short(self) -> &'static str {
        match self {
            Shape::Wide => "16:9",
            Shape::Square => "1:1",
        }
    }
}

/// Everything that decides what the frames show (but not how they're coloured).
#[derive(Clone, PartialEq, Debug)]
pub struct Frames {
    pub path: ZoomPath,
    /// the max coefficient at each waypoint (parallel to path.stops); the frames between
    /// follow a smooth curve through them
    pub maxes: Vec<i32>,
    pub order: usize,
    /// the coefficient set (its `max` is unused: see `maxes`)
    pub coefs: Coefs,
    /// cross-fade between the max coefficients either side of each frame's, so that new
    /// algebraic numbers fade in instead of appearing all at once
    pub fade: bool,
    /// render the plane in patches shared by consecutive frames (see reuse.rs), with the
    /// max coefficient moving up a ladder of rungs, instead of every frame separately
    pub reuse: bool,
    /// with reuse: the size of the max coefficient's steps (index into COEF_STEPS)
    pub step: usize,
    pub height: usize,
    pub shape: Shape,
    /// smoothing: render aa x aa samples per video pixel
    pub aa: usize,
}

/// Each render of a waypoint's frame's levels (for a colour scale measured at the
/// waypoints), with its weight.
pub type WaypointLevels = Vec<(Levels, f64)>;

/// What a waypoint's levels are measured from: the order, coefficients and its frame's jobs.
pub type LevelsKey = (usize, Coefs, Vec<Job>);

impl Frames {
    pub fn count(&self) -> usize {
        self.path.frames()
    }

    pub fn width(&self) -> usize {
        frame_width(self.height, self.shape.ratio())
    }

    /// The region frame `k` shows, at the video's resolution.
    pub fn region(&self, k: usize) -> Region {
        frame_region(self.path.view(k), self.height, self.shape.ratio(), 1)
    }

    /// The same region at the rendering resolution (finer, with smoothing).
    pub fn render_region(&self, k: usize) -> Region {
        frame_region(self.path.view(k), self.height, self.shape.ratio(), self.aa)
    }

    /// Frame `k`'s max coefficient (fractional between the waypoints).
    pub fn max(&self, k: usize) -> f64 {
        max_at(&self.path, &self.maxes, k)
    }

    pub fn coefs_for(&self, max: i32) -> Coefs {
        Coefs { max, ..self.coefs }
    }

    /// The max coefficients worth rendering: with reuse, the ladder of rungs (see
    /// reuse.rs), plus the waypoints' own values, so that a waypoint's frame, or a
    /// stretch at a constant max coefficient, needs only one render; otherwise every whole
    /// number (and this is empty).
    pub fn ladder(&self) -> Vec<i32> {
        if !self.reuse {
            return Vec::new();
        }
        let top = self.maxes.iter().copied().max().unwrap_or(1);
        let mut l = reuse::ladder(self.coefs.set, self.order, top, COEF_STEPS[self.step].1);
        l.extend(self.maxes.iter().map(|&m| m.max(1)));
        l.sort_unstable();
        l.dedup();
        l
    }

    /// The max coefficients to render frame `k` at, with their weights: one, or with
    /// fading, the two either side of its max coefficient (rungs of `ladder` with reuse,
    /// otherwise the coefficient set's members).
    pub fn needs(&self, ladder: &[i32], k: usize) -> Vec<(i32, f64)> {
        let m = self.max(k);
        let (lo, up, t) = if self.reuse { rung_between(ladder, m) } else { fade_between(self.coefs.set, m) };
        if self.fade && up > lo && t > 1e-6 {
            vec![(lo, 1.0 - t), (up, t)]
        } else {
            vec![(if t < 0.5 { lo } else { up }, 1.0)]
        }
    }

    /// Frame `k`'s pixels (at the video's resolution).
    pub fn geom(&self, k: usize) -> Geom {
        Geom::of(&self.region(k))
    }

    /// The patches for the whole video (with reuse).
    pub fn patch_plan(&self, ladder: &[i32]) -> PatchPlan {
        let cap = MAX_RENDER_PIXELS.min(PATCH_FRAMES * self.width() * self.height * self.aa * self.aa);
        let need = |k: usize| {
            let g = self.geom(k);
            (g, g.dx().min(g.dy()) / self.aa as f64, self.needs(ladder, k))
        };
        reuse::plan(self.count(), need, cap)
    }

    /// What to render for frame `k`. With reuse (`pp`), these are the windows of its
    /// patches that cover it, sampled exactly as the whole patches are.
    pub fn jobs(&self, ladder: &[i32], pp: Option<&PatchPlan>, k: usize) -> Vec<Job> {
        match pp {
            Some(pp) => pp.uses[k]
                .iter()
                .map(|&(id, weight)| {
                    let p = &pp.patches[id];
                    let (re, im) = p.axes(p.window(&self.geom(k)));
                    (p.max, re, im, weight)
                })
                .collect(),
            None => {
                let (re, im) = self.render_region(k).axes();
                self.needs(ladder, k).into_iter().map(|(m, weight)| (m, re.clone(), im.clone(), weight)).collect()
            }
        }
    }

    /// The percentiles of a render (from a histogram with reuse, where there's a bigger
    /// render for every frame).
    pub fn stats(&self, r: &Render) -> Stats {
        if self.reuse { Stats::binned(r) } else { Stats::new(r) }
    }

    /// Frame `k`'s pixels, from its renders (as `jobs` describes them, each with its
    /// percentiles and weight), each on the scale `scale` gives it.
    pub fn finish(&self, k: usize, renders: &[(Render, Stats, f64)], style: &Style, scale: &dyn Fn(&Stats) -> Scale) -> Vec<u8> {
        if self.reuse {
            // each render averaged down in linear light, then cross-faded in display brightness
            let g = self.geom(k);
            let images = renders
                .iter()
                .map(|(r, st, weight)| {
                    let mut lin = vec![0f32; g.w * g.h * 3];
                    reuse::add_frame(r, st, &g, style, &scale(st), 1.0, &mut lin);
                    (reuse::to_rgb(&lin), *weight)
                })
                .collect();
            cross_fade(images)
        } else {
            let images = renders.iter().map(|(r, st, w)| (colourise_with(r, st, style, &scale(st)), *w)).collect();
            downsample(&cross_fade(images), self.width(), self.height, self.aa)
        }
    }

    /// What waypoint `i`'s levels are measured from: its frame rendered on its own (not
    /// from patches, even with reuse), so it only changes when that waypoint does.
    pub fn levels_key(&self, ladder: &[i32], i: usize) -> LevelsKey {
        (self.order, self.coefs, self.jobs(ladder, None, self.path.stop_frame(i)))
    }

    /// The rough time each frame takes, in seconds, given the time per unit of search
    /// work (pixels x steps per pixel) measured by a preview. With reuse, a frame's time
    /// includes the patches first needed by it, which are rendered then. Each frame also
    /// gets the time for the rest of the work: the percentiles, colouring, averaging down
    /// and encoding, which measured about 40 ns per pixel per render (12 cores, 720p).
    /// Deep views are cheaper than their steps suggest, because tiles share the search
    /// (engine::work_per_pixel); that is measured on up to SAMPLES renders and
    /// interpolated between them. With `waypoint_levels`, the waypoints' frames are
    /// rendered first. It runs searches, so call it off the UI thread.
    pub fn frame_secs(&self, secs_per_work: f64, waypoint_levels: bool) -> Vec<f64> {
        const SAMPLES: usize = 24;
        let ladder = self.ladder();
        let mut plans: HashMap<i32, Option<Plan>> = HashMap::new();
        let px = (self.width() * self.height * self.aa * self.aa) as f64;
        let needs: Vec<Vec<(i32, f64)>> = (0..self.count()).map(|k| self.needs(&ladder, k)).collect();
        let mut secs: Vec<f64> = needs.iter().map(|n| 40e-9 * px * n.len() as f64).collect();
        // the renders: (frame that needs it first, max coefficient, pixels, its sample points)
        type Axes<'a> = Box<dyn Fn() -> (Vec<f64>, Vec<f64>) + 'a>;
        let renders: Vec<(usize, i32, f64, Axes)> = if self.reuse {
            self.patch_plan(&ladder)
                .patches
                .into_iter()
                .map(|p| (p.first, p.max, p.pixels() as f64, Box::new(move || p.axes(p.all())) as Axes))
                .collect()
        } else {
            needs
                .iter()
                .enumerate()
                .flat_map(|(k, n)| n.iter().map(move |&(m, _)| (k, m, px, Box::new(move || self.render_region(k).axes()) as Axes)))
                .collect()
        };
        for &(_, m, _, _) in &renders {
            plans.entry(m).or_insert_with(|| Plan::new(self.order, &self.coefs_for(m), false).ok());
        }
        // work per pixel relative to steps, at evenly spaced renders
        let n = renders.len();
        let picks: Vec<usize> = if n <= SAMPLES { (0..n).collect() } else { (0..SAMPLES).map(|i| i * (n - 1) / (SAMPLES - 1)).collect() };
        let mut ratio = Vec::new();
        for &i in &picks {
            let (_, m, _, axes) = &renders[i];
            if let Some(p) = &plans[m] {
                let (re, im) = axes();
                ratio.push((i, work_per_pixel(p, &re, &im) / p.steps().max(1e-9)));
            }
        }
        let ratio_at = |i: usize| match ratio.partition_point(|r| r.0 < i) {
            0 => ratio.first().map_or(1.0, |r| r.1),
            j if j == ratio.len() => ratio[j - 1].1,
            j if ratio[j].0 == i => ratio[j].1,
            j => {
                let ((i0, r0), (i1, r1)) = (ratio[j - 1], ratio[j]);
                r0 + (r1 - r0) * (i - i0) as f64 / (i1 - i0) as f64
            }
        };
        for (i, (k, m, pixels, _)) in renders.iter().enumerate() {
            let steps = plans[m].as_ref().map_or(0.0, |p| p.steps());
            secs[*k] += pixels * steps * ratio_at(i) * secs_per_work;
        }
        if waypoint_levels {
            // the waypoints' frames, rendered first (roughly: at the mean ratio)
            let mean = ratio.iter().map(|r| r.1).sum::<f64>() / ratio.len().max(1) as f64;
            for i in 0..self.path.stops.len() {
                for &(m, _) in &needs[self.path.stop_frame(i)] {
                    let plan = plans.entry(m).or_insert_with(|| Plan::new(self.order, &self.coefs_for(m), false).ok());
                    secs[0] += px * plan.as_ref().map_or(0.0, |p| p.steps()) * mean * secs_per_work;
                }
            }
        }
        secs
    }
}

/// How the lengths and heights are spread over the colormap, when colouring by them.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LhScale {
    /// each frame from its own lowest to its highest
    Each,
    /// one range for the whole video, from the lowest at any waypoint to the highest
    Waypoints,
    /// one range set by hand
    Fixed(f64, f64),
}

/// How every frame of a video is coloured: one style for the whole video, and the colour
/// scale it's used on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Colouring {
    pub style: Style,
    /// the residual brightness on percentiles measured at the waypoints and moved steadily
    /// between them (see `held_scale`), instead of each frame's own
    pub steady: bool,
    pub lh: LhScale,
}

impl Default for Colouring {
    fn default() -> Colouring {
        Colouring { style: Style::default(), steady: false, lh: LhScale::Waypoints }
    }
}

impl Colouring {
    fn by_poly(&self) -> bool {
        matches!(self.style.by, ColourBy::Length | ColourBy::Height)
    }

    /// Whether the residual brightness is coloured at all (not for length/height unshaded).
    fn uses_residual(&self) -> bool {
        !self.by_poly() || self.style.shade
    }

    /// Whether the waypoints' levels have to be measured first.
    pub fn needs_levels(&self) -> bool {
        (self.steady && self.uses_residual()) || (self.by_poly() && self.lh == LhScale::Waypoints)
    }

    /// The scale a render (with percentiles `st`) of frame `k` is coloured on, given the
    /// waypoints' levels (empty unless `needs_levels`).
    pub fn scale(&self, path: &ZoomPath, levels: &[WaypointLevels], k: usize, st: &Stats) -> Scale {
        let own = st.scale(&self.style);
        let held = (!levels.is_empty() && levels.iter().all(|l| !l.is_empty())).then(|| held_scale(path, &self.style, levels, k));
        let mut s = match &held {
            Some(h) if self.steady => Scale { length: own.length, height: own.height, ..*h },
            _ => own,
        };
        match (self.lh, &held) {
            (LhScale::Waypoints, Some(h)) => (s.length, s.height) = (h.length, h.height),
            (LhScale::Fixed(lo, hi), _) => (s.length, s.height) = ((lo, hi), (lo, hi)),
            _ => {}
        }
        s
    }
}

/// What the automatic max coefficients depend on.
#[derive(Clone, PartialEq, Debug)]
pub struct AutoInputs {
    pub stops: Vec<View>,
    pub auto: Vec<bool>,
    /// the max coefficients set by hand (0 where auto)
    pub manual: Vec<i32>,
    /// the segments that keep the flower size (see video::keep_flower)
    pub flower: Vec<bool>,
    pub order: usize,
    pub coefs: Coefs,
    /// algebraic numbers per 100 px² (of the video) that auto aims for
    pub target: f64,
    pub height: usize,
    pub shape: Shape,
}

/// The max coefficient at each waypoint that has auto on: the one whose predicted density
/// per video pixel is closest to the target, however much work it needs (unlike the
/// explorer's auto: deep views share the search across tiles, so large max coefficients
/// are often quick, and a video is rendered once). Along a segment that zooms in it never
/// falls (and along one that zooms out it never rises), even where the prediction would
/// allow it. A waypoint at the end of a segment that keeps the flower size gets the max
/// coefficient that does that, whether auto is on or not. Returns all the max
/// coefficients (the others kept from `maxes`) and auto's notes.
pub fn auto_maxes(a: &AutoInputs, maxes: &[i32], p: &mut Predictor) -> (Vec<i32>, Vec<String>) {
    let mut out = maxes.to_vec();
    let mut notes = vec![String::new(); out.len()];
    for i in 0..out.len() {
        if i > 0 && a.flower.get(i - 1) == Some(&true) {
            out[i] = super::keep_flower(out[i - 1], a.stops[i - 1], a.stops[i]);
            if let Err(e) = Plan::new(a.order, &Coefs { max: out[i], ..a.coefs }, false) {
                notes[i] = format!("max coefficient {} can't be searched: {e}", out[i]);
            }
            continue;
        }
        if !a.auto[i] {
            continue;
        }
        let region = frame_region(a.stops[i], a.height, a.shape.ratio(), 1);
        let guess = Coefs { max: out[i].clamp(1, MAX_COEF), ..a.coefs };
        let (_, c, note) = choose_auto(p, a.order, guess, &region, a.target, false, f64::INFINITY);
        let mut m = c.max;
        if i > 0 {
            let (prev, h0, h1) = (out[i - 1], a.stops[i - 1].hw, a.stops[i].hw);
            if h1 < h0 {
                m = m.max(prev);
            } else if h1 > h0 {
                m = m.min(prev);
            }
        }
        (out[i], notes[i]) = (m, note);
    }
    (out, notes)
}

/// A fractional max coefficient, e.g. "6.4", or "7" when it's whole.
pub fn max_text(m: f64) -> String {
    if m.fract() == 0.0 { format!("{m:.0}") } else { format!("{m:.1}") }
}

/// "45 s", "12 min", "3 h 20 min"
pub fn duration_text(secs: f64) -> String {
    if secs < 90.0 {
        format!("{secs:.0} s")
    } else if secs < 5400.0 {
        format!("{:.0} min", secs / 60.0)
    } else {
        format!("{:.0} h {:.0} min", (secs / 3600.0).floor(), (secs % 3600.0) / 60.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{render, CoefSet};

    /// Keep flower size sets the next waypoint's max coefficient in proportion to the
    /// zoom, overriding auto there, and auto carries on from it.
    #[test]
    fn flower_sets_the_next_max() {
        let beta = View { cr: 1.0 / 3.0, ci: 2f64.sqrt() / 3.0, hw: 0.02 };
        let stops = vec![beta, View { hw: 0.002, ..beta }, View { hw: 0.0002, ..beta }];
        let inputs = AutoInputs {
            stops: stops.clone(),
            auto: vec![false, true, true],
            manual: vec![40, 0, 0],
            flower: vec![true, false],
            order: 3,
            coefs: Coefs { max: 1, ..Coefs::ints(1) },
            target: 4.0,
            height: 360,
            shape: Shape::Square,
        };
        let (maxes, _) = auto_maxes(&inputs, &[40, 1, 1], &mut Predictor::new());
        assert_eq!(maxes[..2], [40, 400]);
        assert!(maxes[2] >= 400, "auto never falls on a zoom in: {maxes:?}");
    }

    /// The length/height scale: each frame's own, the waypoints' range, or set by hand;
    /// and steady brightness takes the residual levels from the waypoints.
    #[test]
    fn colour_scales() {
        let path = ZoomPath::new(vec![View::HOME, View { hw: 0.05, ..View::HOME }], 1.2);
        let st = |m: i32, v: View| {
            let plan = Plan::new(3, &Coefs { set: CoefSet::Ints, ..Coefs::ints(m) }, false).unwrap();
            Stats::new(&render(&plan, &frame_region(v, 24, 1.0, 1), 2, &Default::default(), &Default::default()).unwrap())
        };
        let (a, b) = (st(3, path.stops[0]), st(12, path.stops[1]));
        let levels = vec![vec![(a.levels(), 1.0)], vec![(b.levels(), 1.0)]];
        let by_len = Style { by: ColourBy::Length, ..Style::default() };
        let mid = path.frames() / 2;
        let own = a.scale(&by_len);
        let each = Colouring { style: by_len, steady: false, lh: LhScale::Each };
        assert_eq!(each.scale(&path, &levels, mid, &a), own);
        assert!(!each.needs_levels());
        let wp = Colouring { lh: LhScale::Waypoints, ..each };
        assert!(wp.needs_levels());
        let s = wp.scale(&path, &levels, mid, &a);
        assert_eq!(s.length, (own.length.0.min(b.scale(&by_len).length.0), own.length.1.max(b.scale(&by_len).length.1)));
        assert_eq!(s.ln_hi, own.ln_hi, "the brightness stays the frame's own");
        let fixed = Colouring { lh: LhScale::Fixed(2.0, 50.0), ..each };
        assert_eq!(fixed.scale(&path, &[], mid, &a).length, (2.0, 50.0));
        // steady brightness: the residual levels from the waypoints, at the ends exactly
        let steady = Colouring { style: Style::default(), steady: true, lh: LhScale::Each };
        assert!(steady.needs_levels());
        let last = path.frames() - 1;
        assert_eq!(steady.scale(&path, &levels, last, &a).ln_hi, b.scale(&Style::default()).ln_hi);
        // without levels (not measured yet), each frame's own
        assert_eq!(steady.scale(&path, &[], last, &a), a.scale(&Style::default()));
    }

    #[test]
    fn durations() {
        assert_eq!(duration_text(45.0), "45 s");
        assert_eq!(duration_text(720.0), "12 min");
        assert_eq!(duration_text(3.0 * 3600.0 + 1200.0), "3 h 20 min");
        assert_eq!((max_text(7.0), max_text(6.42)), ("7".to_string(), "6.4".to_string()));
    }
}
