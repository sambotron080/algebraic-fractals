//! Reusing renders between frames.
//!
//! Consecutive frames of a zoom or a pan show mostly the same part of the plane. So
//! instead of rendering every frame separately, the plane is rendered in *patches*: a
//! render on a fixed square grid that covers several consecutive frames, sampled as
//! finely as the most magnified of them needs. Each frame is cut out of its patch and
//! averaged down to the frame's pixels in linear light, which also anti-aliases it.
//!
//! A patch can only serve frames with the same max coefficient, but between waypoints
//! the max coefficient changes almost every frame once it's large. So it moves up a
//! ladder of values instead (`ladder`), each allowing a fixed proportion more
//! polynomials than the one below, and each frame cross-fades between the two rungs
//! either side of its own max coefficient.

use crate::common::MAX_COEF;
use crate::video::{light_tables, BACK};
use crate::colour::{colourise_with, Scale, Stats, Style};
use crate::engine::{CoefSet, Region, Render};
use std::collections::HashMap;

/// The max coefficients that patches are rendered at, up to (at least) `top`: every
/// member of the set while consecutive members are more than a factor q apart, then
/// members about q apart, with q = e^(growth/(order+1)). Then each rung allows about
/// q^(order+1) = e^growth times as many polynomials as the one below. As the max
/// coefficient needed grows roughly like the magnification^(2/(order+1)), each rung is
/// used for a zoom of about e^growth either side. At growth 0.5 that's √e, which is
/// also the zoom one patch covers most cheaply (see `plan`). Smaller steps bring in
/// fewer new stars at a time, and cost more patches.
pub fn ladder(set: CoefSet, order: usize, top: i32, growth: f64) -> Vec<i32> {
    let q = (growth / (order + 1) as f64).exp();
    let top = top.clamp(1, MAX_COEF);
    let members = set.upto(top.saturating_mul(2).min(MAX_COEF));
    let mut out = vec![1];
    while let Some(&cur) = out.last() {
        if cur >= top {
            break;
        }
        let target = ((cur as f64 * q).ceil() as i32).max(cur + 1);
        match members.get(members.partition_point(|&v| v < target)) {
            Some(&m) => out.push(m),
            None => break,
        }
    }
    out
}

/// The rungs either side of a (fractional) max coefficient `m`, and how far `m` is
/// from the lower to the upper (0..1, on a log scale).
pub fn rung_between(ladder: &[i32], m: f64) -> (i32, i32, f64) {
    let i = ladder.partition_point(|&r| r as f64 <= m * (1.0 + 1e-12)).max(1) - 1;
    if i + 1 >= ladder.len() {
        return (ladder[i], ladder[i], 0.0);
    }
    let (lo, up) = (ladder[i], ladder[i + 1]);
    let t = ((m / lo as f64).ln() / (up as f64 / lo as f64).ln()).clamp(0.0, 1.0);
    (lo, up, t)
}

/// Where a frame's pixels are: sample points from `re.0` to `re.1` (left to right) and
/// `im.1` to `im.0` (top to bottom), `w` x `h` of them, as `Region::axes` places them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geom {
    pub re: (f64, f64),
    pub im: (f64, f64),
    pub w: usize,
    pub h: usize,
}

impl Geom {
    pub fn of(r: &Region) -> Geom {
        Geom { re: r.re, im: r.im, w: r.width, h: r.height() }
    }

    /// Pixel spacing along the real and imaginary axes.
    pub fn dx(&self) -> f64 {
        (self.re.1 - self.re.0) / (self.w.max(2) - 1) as f64
    }
    pub fn dy(&self) -> f64 {
        (self.im.1 - self.im.0) / (self.h.max(2) - 1) as f64
    }

    /// The part of the plane the pixels cover, each pixel being the cell around its
    /// sample point: (re min, re max, im min, im max).
    fn extent(&self) -> (f64, f64, f64, f64) {
        let (hx, hy) = (self.dx() / 2.0, self.dy() / 2.0);
        (self.re.0 - hx, self.re.1 + hx, self.im.0 - hy, self.im.1 + hy)
    }
}

/// A render on a square grid with spacing `d`, `w` x `h` samples, sample (i, j) at
/// ((left + i) d, (top - j) d), at max coefficient `max`, for frames `first..=last`.
///
/// The grid is the lattice of multiples of `d`, so two patches with the same spacing
/// (such as consecutive patches of a pan) sample exactly the same points where they
/// overlap, and changing from one to the other can't make the stars jump.
#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub max: i32,
    pub left: i64,
    pub top: i64,
    pub d: f64,
    pub w: usize,
    pub h: usize,
    pub first: usize,
    pub last: usize,
}

/// Samples [i0, i1) x [j0, j1) of a patch.
pub type Window = (usize, usize, usize, usize);

impl Patch {
    /// The patch covering `ext` (as from `Geom::extent`) at spacing `d`.
    fn covering(max: i32, ext: (f64, f64, f64, f64), d: f64, first: usize, last: usize) -> Patch {
        let (left, right) = ((ext.0 / d).floor() as i64, (ext.1 / d).ceil() as i64);
        let (bottom, top) = ((ext.2 / d).floor() as i64, (ext.3 / d).ceil() as i64);
        let (w, h) = ((right - left + 1) as usize, (top - bottom + 1) as usize);
        Patch { max, left, top, d, w, h, first, last }
    }

    fn x0(&self) -> f64 {
        self.left as f64 * self.d
    }

    fn y0(&self) -> f64 {
        self.top as f64 * self.d
    }

    pub fn pixels(&self) -> usize {
        self.w * self.h
    }

    pub fn all(&self) -> Window {
        (0, self.w, 0, self.h)
    }

    /// The samples whose cells overlap a frame's pixels.
    pub fn window(&self, g: &Geom) -> Window {
        let (x0, x1, y0, y1) = g.extent();
        let lo = |v: f64| (v - 0.5).floor().max(0.0) as usize;
        let hi = |v: f64, n: usize| ((v + 0.5).ceil().max(0.0) as usize + 1).min(n);
        let (i0, i1) = (lo((x0 - self.x0()) / self.d), hi((x1 - self.x0()) / self.d, self.w));
        let (j0, j1) = (lo((self.y0() - y1) / self.d), hi((self.y0() - y0) / self.d, self.h));
        (i0, i1.max(i0 + 1), j0, j1.max(j0 + 1))
    }

    /// The sample points of a window: (real axis, imaginary axis, top first).
    pub fn axes(&self, (i0, i1, j0, j1): Window) -> (Vec<f64>, Vec<f64>) {
        let re = (i0..i1).map(|i| (self.left + i as i64) as f64 * self.d).collect();
        let im = (j0..j1).map(|j| (self.top - j as i64) as f64 * self.d).collect();
        (re, im)
    }
}

/// The patches a video needs, and for each frame, the patches it's made from (with
/// their weights in the cross-fade).
#[derive(Clone, Debug)]
pub struct PatchPlan {
    pub patches: Vec<Patch>,
    pub uses: Vec<Vec<(usize, f64)>>,
}

/// Plans the patches for `n` frames. `need(k)` gives frame k's pixels, the sample
/// spacing it needs (at most its pixel spacing) and the max coefficients it blends,
/// with their weights.
///
/// Each patch serves consecutive frames at one max coefficient. It's extended to the
/// next frame (growing to cover it, and getting finer if that needs it) as long as that
/// lowers its pixels per frame and keeps it within `cap` pixels. In a zoom by a factor Z,
/// a patch needs ZÂ² times a frame's pixels, for frames âˆ ln Z, which is cheapest per
/// frame at Z = âˆše â‰ˆ 1.65; in a pan it keeps growing until the cap.
pub fn plan(n: usize, need: impl Fn(usize) -> (Geom, f64, Vec<(i32, f64)>), cap: usize) -> PatchPlan {
    let mut patches: Vec<Patch> = Vec::new();
    let mut uses = Vec::with_capacity(n);
    // the latest patch of each max coefficient, and the extent it was made to cover
    let mut open: HashMap<i32, (usize, (f64, f64, f64, f64))> = HashMap::new();
    for k in 0..n {
        let (g, d, rungs) = need(k);
        let e = g.extent();
        let mut used = Vec::with_capacity(rungs.len());
        for (m, weight) in rungs {
            let extended = open.get(&m).copied().and_then(|(id, ext)| {
                let p = &patches[id];
                if p.last + 1 != k {
                    return None;
                }
                let ext2 = (ext.0.min(e.0), ext.1.max(e.1), ext.2.min(e.2), ext.3.max(e.3));
                let bigger = Patch::covering(m, ext2, p.d.min(d), p.first, k);
                let frames = (p.last - p.first + 1) as f64;
                let cheaper = (bigger.pixels() as f64 / (frames + 1.0)) <= p.pixels() as f64 / frames;
                (bigger.pixels() <= cap && cheaper).then_some((id, bigger, ext2))
            });
            let id = match extended {
                Some((id, p, ext)) => {
                    patches[id] = p;
                    open.insert(m, (id, ext));
                    id
                }
                None => {
                    patches.push(Patch::covering(m, e, d, k, k));
                    open.insert(m, (patches.len() - 1, e));
                    patches.len() - 1
                }
            };
            used.push((id, weight));
        }
        uses.push(used);
    }
    PatchPlan { patches, uses }
}

/// A window of a render (made from the patch's full grid).
pub fn crop(r: &Render, (i0, i1, j0, j1): Window) -> Render {
    let (w, t) = (i1 - i0, r.nterms);
    let mut residual = Vec::with_capacity(w * (j1 - j0));
    let mut coeffs = Vec::with_capacity(w * (j1 - j0) * t);
    for j in j0..j1 {
        let row = j * r.width;
        residual.extend_from_slice(&r.residual[row + i0..row + i1]);
        coeffs.extend_from_slice(&r.coeffs[(row + i0) * t..(row + i1) * t]);
    }
    Render {
        width: w,
        height: j1 - j0,
        nterms: t,
        re_axis: r.re_axis[i0..i1].to_vec(),
        im_axis: r.im_axis[j0..j1].to_vec(),
        residual,
        coeffs,
    }
}

/// For each of `n` output cells (centred at start + o * step, `step` wide), the source
/// samples (at increasing positions `src`, each the centre of a cell `d` wide) that
/// overlap it, weighted by the overlap and normalised.
fn box_weights(src: &[f64], d: f64, start: f64, step: f64, n: usize) -> Vec<Vec<(usize, f32)>> {
    let s0 = src[0];
    (0..n)
        .map(|o| {
            let c = start + o as f64 * step;
            let (lo, hi) = (c - step / 2.0, c + step / 2.0);
            let i0 = ((lo - s0) / d - 1.0).floor().max(0.0) as usize;
            let i1 = (((hi - s0) / d + 1.0).ceil().max(0.0) as usize).min(src.len() - 1);
            let mut ws: Vec<(usize, f64)> = (i0.min(i1)..=i1)
                .filter_map(|i| {
                    let overlap = (src[i] + d / 2.0).min(hi) - (src[i] - d / 2.0).max(lo);
                    (overlap > 0.0).then_some((i, overlap))
                })
                .collect();
            if ws.is_empty() {
                // (only from rounding at the very edge) the nearest sample
                let i = ((c - s0) / d).round().clamp(0.0, (src.len() - 1) as f64) as usize;
                ws.push((i, 1.0));
            }
            let total: f64 = ws.iter().map(|w| w.1).sum();
            ws.into_iter().map(|(i, w)| (i, (w / total) as f32)).collect()
        })
        .collect()
}

/// Colours `r` (a window of a patch covering frame `g`) with `style` on `scale`,
/// averages it down to the frame's pixels in linear light, and adds it times `weight`
/// to `out` (linear RGB, g.w * g.h * 3).
pub fn add_frame(r: &Render, st: &Stats, g: &Geom, style: &Style, scale: &Scale, weight: f32, out: &mut [f32]) {
    let rgb = colourise_with(r, st, style, scale);
    let (lin, _) = light_tables();
    // the sample spacing (a patch's grid is square, but a frame's own isn't quite)
    let dsx = if r.width > 1 { r.re_axis[1] - r.re_axis[0] } else { g.dx() };
    let dsy = if r.height > 1 { r.im_axis[0] - r.im_axis[1] } else { dsx };
    let dsx = if r.width > 1 { dsx } else { dsy };
    let wx = box_weights(&r.re_axis, dsx, g.re.0, g.dx(), g.w);
    let neg_im: Vec<f64> = r.im_axis.iter().map(|v| -v).collect();
    let wy = box_weights(&neg_im, dsy, -g.im.1, g.dy(), g.h);
    let (sw, w) = (r.width, g.w);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());

    // across: each source row -> w pixels
    let mut across = vec![0f32; r.height * w * 3];
    let rows_per = r.height.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (chunk, part) in across.chunks_mut(rows_per * w * 3).enumerate() {
            let (rgb, lin, wx) = (&rgb, &lin, &wx);
            s.spawn(move || {
                for (jr, row) in part.chunks_mut(w * 3).enumerate() {
                    let src = (chunk * rows_per + jr) * sw * 3;
                    for (x, taps) in wx.iter().enumerate() {
                        let mut acc = [0f32; 3];
                        for &(i, wt) in taps {
                            for c in 0..3 {
                                acc[c] += lin[rgb[src + 3 * i + c] as usize] * wt;
                            }
                        }
                        row[3 * x..3 * x + 3].copy_from_slice(&acc);
                    }
                }
            });
        }
    });

    // down: rows of `across` -> the frame's rows
    let rows_per = g.h.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (chunk, part) in out.chunks_mut(rows_per * w * 3).enumerate() {
            let (across, wy) = (&across, &wy);
            s.spawn(move || {
                for (yr, row) in part.chunks_mut(w * 3).enumerate() {
                    for &(j, wt) in &wy[chunk * rows_per + yr] {
                        let src = &across[j * w * 3..(j + 1) * w * 3];
                        let wt = wt * weight;
                        for (o, v) in row.iter_mut().zip(src) {
                            *o += v * wt;
                        }
                    }
                }
            });
        }
    });
}

/// Linear-light RGB back to 8-bit RGB (on all cores).
pub fn to_rgb(lin: &[f32]) -> Vec<u8> {
    let (_, back) = light_tables();
    let mut out = vec![0u8; lin.len()];
    let per = lin.len().div_ceil(std::thread::available_parallelism().map_or(4, |n| n.get())).max(1 << 14);
    std::thread::scope(|s| {
        for (o, l) in out.chunks_mut(per).zip(lin.chunks(per)) {
            let back = &back;
            s.spawn(move || {
                for (o, &v) in o.iter_mut().zip(l) {
                    *o = back[((v.clamp(0.0, 1.0) * BACK as f32) + 0.5) as usize];
                }
            });
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colour::colourise;
    use crate::engine::{render, render_axes, Coefs, Plan};

    #[test]
    fn ladders() {
        let l = ladder(CoefSet::Ints, 3, 200, 0.5);
        assert_eq!(&l[..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(*l.last().unwrap() >= 200);
        // about 13% apart once the integers are closer than that
        for w in l.windows(2).filter(|w| w[0] > 20) {
            let r = w[1] as f64 / w[0] as f64;
            assert!(r > 1.1 && r < 1.2, "{w:?}");
        }
        // finer steps: about 3% apart (every whole number up to 32)
        let fine = ladder(CoefSet::Ints, 3, 1000, 0.125);
        assert_eq!(&fine[..32], &(1..=32).collect::<Vec<i32>>()[..]);
        for w in fine.windows(2).filter(|w| w[0] > 100) {
            let r = w[1] as f64 / w[0] as f64;
            assert!(r > 1.025 && r < 1.045, "{w:?}");
        }
        assert_eq!(ladder(CoefSet::Powers2, 3, 100, 0.5), [1, 2, 4, 8, 16, 32, 64, 128]);
        assert!(ladder(CoefSet::Primes, 3, 50, 0.5).iter().skip(1).all(|p| CoefSet::Primes.upto(60).contains(p)));
        let (lo, up, t) = rung_between(&l, 6.0);
        assert_eq!((lo, up, t), (6, 7, 0.0));
        let (lo, up, t) = rung_between(&l, 1e9);
        assert_eq!((lo, up, t), (*l.last().unwrap(), *l.last().unwrap(), 0.0));
        let (lo, up, t) = rung_between(&l, 100.0);
        assert!(lo as f64 <= 100.0 && up as f64 > 100.0 && t > 0.0 && t < 1.0);
    }

    fn geom(cr: f64, ci: f64, hw: f64, w: usize, h: usize) -> Geom {
        let half_w = hw * w as f64 / h as f64;
        Geom::of(&Region { re: (cr - half_w, cr + half_w), im: (ci - hw, ci + hw), width: w })
    }

    /// Every frame's patches are fine enough for it and cover it; zooms and pans reuse them.
    #[test]
    fn plans_cover_frames() {
        // a zoom by 1.02 per frame at one max coefficient, then a pan
        let n = 120;
        let g = |k: usize| {
            if k < 60 {
                geom(0.3, 0.4, 0.1 / 1.02f64.powi(k as i32), 64, 36)
            } else {
                geom(0.3 + (k - 60) as f64 * 0.01 * 0.1 / 1.02f64.powi(59), 0.4, 0.1 / 1.02f64.powi(59), 64, 36)
            }
        };
        let need = |k: usize| (g(k), g(k).dx().min(g(k).dy()), vec![(10, 1.0)]);
        let p = plan(n, need, 64 * 36 * 8);
        let frame_px = (64 * 36) as f64;
        let total: usize = p.patches.iter().map(|p| p.pixels()).sum();
        eprintln!("{} patches, {:.2} frames of pixels per frame", p.patches.len(), total as f64 / frame_px / n as f64);
        assert!((total as f64) < 0.4 * frame_px * n as f64, "{total}");
        for k in 0..n {
            assert_eq!(p.uses[k].len(), 1);
            let patch = &p.patches[p.uses[k][0].0];
            assert!(patch.first <= k && k <= patch.last && patch.d <= g(k).dx().min(g(k).dy()) * (1.0 + 1e-12));
            let (i0, i1, j0, j1) = patch.window(&g(k));
            let (re, im) = patch.axes((i0, i1, j0, j1));
            let (x0, x1, y0, y1) = g(k).extent();
            // the window's sample cells cover the frame's pixel cells
            assert!(re[0] - patch.d / 2.0 <= x0 && re[re.len() - 1] + patch.d / 2.0 >= x1, "frame {k}");
            assert!(im[im.len() - 1] - patch.d / 2.0 <= y0 && im[0] + patch.d / 2.0 >= y1, "frame {k}");
        }
    }

    /// With the patch's grid exactly the frame's, a frame is the direct render's colours.
    #[test]
    fn same_grid_same_picture() {
        let region = Region { re: (0.1, 0.5), im: (0.2, 0.425), width: 64 };
        let g = Geom::of(&region);
        let plan_ = Plan::new(3, &Coefs::ints(6), false).unwrap();
        let direct = render(&plan_, &region, 2, &Default::default(), &Default::default()).unwrap();
        let (re, im) = region.axes();
        let r = render_axes(&plan_, re, im, 2, &Default::default(), &Default::default()).unwrap();
        assert_eq!(r.residual, direct.residual);
        let st = Stats::binned(&r);
        let style = Style::default();
        let mut lin = vec![0f32; g.w * g.h * 3];
        add_frame(&r, &st, &g, &style, &st.scale(&style), 1.0, &mut lin);
        let a = to_rgb(&lin);
        let b = colourise(&direct, &st, &style);
        let worst = a.iter().zip(&b).map(|(&x, &y)| (x as i32 - y as i32).abs()).max().unwrap();
        assert!(worst <= 1, "{worst}");
    }

    /// Averaging down keeps the mean brightness, and a window of a full patch render
    /// is the same as rendering just the window.
    #[test]
    fn windows_and_averaging() {
        let g = geom(0.3, 0.4, 0.05, 48, 27);
        let p = plan(1, |_| (g, g.dx() / 2.7, vec![(5, 1.0)]), usize::MAX).patches.remove(0);
        let plan_ = Plan::new(3, &Coefs::ints(5), false).unwrap();
        let (re, im) = p.axes(p.all());
        let full = render_axes(&plan_, re, im, 2, &Default::default(), &Default::default()).unwrap();
        let win = p.window(&g);
        let (re, im) = p.axes(win);
        let part = render_axes(&plan_, re, im, 2, &Default::default(), &Default::default()).unwrap();
        let cropped = crop(&full, win);
        assert_eq!(cropped.residual, part.residual);
        assert_eq!(cropped.coeffs, part.coeffs);

        let st = Stats::binned(&part);
        let style = Style::default();
        let mut lin = vec![0f32; g.w * g.h * 3];
        add_frame(&part, &st, &g, &style, &st.scale(&style), 1.0, &mut lin);
        let (l, _) = light_tables();
        let src: f64 = colourise(&part, &st, &style).iter().map(|&v| l[v as usize] as f64).sum::<f64>() / (part.width * part.height) as f64;
        let out: f64 = lin.iter().map(|&v| v as f64).sum::<f64>() / (g.w * g.h) as f64;
        assert!((src - out).abs() < 0.03 * src.max(1e-3), "{src} {out}");
    }
}
