//! Predicting how many algebraic numbers (distinct roots of the allowed polynomials)
//! fall inside a view, without rendering it.
//!
//!   expected roots in the view = sum over d of  N_d * integral over the view of rho_d(z)
//!
//! N_d, the number of polynomials of exact degree d, is a product of how many values
//! each coefficient can take (see `poly_counts`).
//!
//! rho_d, the expected root density of one such polynomial, is built in two parts:
//! - The shape comes from the Kac-Rice formula for a random polynomial with
//!   independent Gaussian real coefficients (as in Shepp & Vanderbei 1995). It captures
//!   the fine structure: roots crowding the unit circle more tightly as d grows,
//!   avoiding the real axis, and a separate density of roots exactly on it. It doesn't
//!   depend on the coefficients, but it assumes they can be arbitrarily small or large.
//! - A radial correction from sampling: a few hundred random allowed polynomials are
//!   solved, and their roots' distances from 0 are compared with the formula's.
//!   Integer coefficients confine the roots (with |c| <= 1 every root has
//!   1/2 < |z| < 2), which the Gaussian model misses by large factors far from the
//!   unit circle. The correction is a function of ln|z|, interpolated between bins.
//!
//! Checked against exact counts by `examples/density_check.rs`: typically within 20%
//! (most of the excess is reducible polynomials, whose roots repeat lower degrees).

use crate::engine::{Coefs, Region};
use std::collections::HashMap;
use std::f64::consts::PI;

/// Samples per side of the grid used to integrate the density over a view.
const GRID: usize = 48;
/// Radial bins: ln|z| from -LN_R to LN_R in steps of BIN.
const LN_R: f64 = 10.0;
const BIN: f64 = 0.1;
const NBINS: usize = (2.0 * LN_R / BIN) as usize;
/// Roughly how many sampled roots per degree for the correction.
const SAMPLE_ROOTS: usize = 6000;

/// Expected number of complex (non-real) roots per unit area at z = (x, y), for a
/// random polynomial of degree d with i.i.d. real Gaussian coefficients.
///
/// With f = u + iv, Kac-Rice gives rho = E[|f'|^2 | f = 0] * p_(u,v)(0); for a Gaussian
/// vector that is the trace of the conditional covariance of (u', v') given (u, v),
/// divided by 2 pi sqrt(det cov(u, v)).
pub fn complex_density(d: usize, x: f64, y: f64) -> f64 {
    // Reversing the coefficients maps the roots to 1/z and leaves their distribution
    // unchanged, so rho(z) = rho(1/z) / |z|^4. Evaluating only inside the unit disc avoids
    // overflow and the cancellation in the formula below for large |z|.
    let m = x * x + y * y;
    if m > 1.0 {
        return complex_density(d, x / m, -y / m) / (m * m);
    }
    // u, v, u', v' as vectors of their coefficient multipliers: Re/Im of z^k and k z^(k-1)
    let mut g = [[0.0f64; 4]; 4];
    let (mut pr, mut pi) = (1.0, 0.0); // z^k
    let (mut qr, mut qi) = (0.0, 0.0); // k z^(k-1)
    for k in 0..=d {
        let v = [pr, pi, qr, qi];
        for a in 0..4 {
            for b in a..4 {
                g[a][b] += v[a] * v[b];
            }
        }
        qr = (k + 1) as f64 * pr;
        qi = (k + 1) as f64 * pi;
        (pr, pi) = (pr * x - pi * y, pr * y + pi * x);
    }
    let det = g[0][0] * g[1][1] - g[0][1] * g[0][1];
    if !(det > 0.0) {
        return 0.0;
    }
    // Schur complement: trace(C - B^T A^-1 B), with A = cov(u, v), B = cov((u, v), (u', v'))
    let inv = [[g[1][1] / det, -g[0][1] / det], [-g[0][1] / det, g[0][0] / det]];
    let b = [[g[0][2], g[0][3]], [g[1][2], g[1][3]]];
    let mut trace = g[2][2] + g[3][3];
    for j in 0..2 {
        for a in 0..2 {
            for c in 0..2 {
                trace -= b[a][j] * inv[a][c] * b[c][j];
            }
        }
    }
    (trace / (2.0 * PI * det.sqrt())).max(0.0)
}

/// Expected number of real roots per unit length at x (Kac's formula).
pub fn real_density(d: usize, x: f64) -> f64 {
    // as for complex_density: rho(x) = rho(1/x) / x^2
    if x.abs() > 1.0 {
        return real_density(d, 1.0 / x) / (x * x);
    }
    let (mut a, mut b, mut c) = (0.0, 0.0, 0.0);
    let (mut p, mut q) = (1.0, 0.0); // x^k, k x^(k-1)
    for k in 0..=d {
        a += p * p;
        b += p * q;
        c += q * q;
        q = (k + 1) as f64 * p;
        p *= x;
    }
    if a <= 0.0 {
        return 0.0;
    }
    (a * c - b * b).max(0.0).sqrt() / (PI * a)
}

/// Number of polynomials of each exact degree 0..=order (index d), counting p and -p
/// once and leaving out c0 = 0 (which only adds the root 0 and repeats lower degrees).
pub fn poly_counts(order: usize, coefs: &Coefs) -> Vec<f64> {
    let vals = coefs.values();
    let nz = vals.iter().filter(|&&c| c != 0).count() as f64;
    let all = vals.len() as f64;
    (0..=order)
        .map(|d| match d {
            0 => 0.0,
            // without 0 every coefficient is nonzero, so only the full order exists
            _ if !coefs.zero && d < order => 0.0,
            // leading coefficient and c0 nonzero, the d - 1 in between anything
            _ => nz * nz * all.powi(d as i32 - 1) / 2.0,
        })
        .collect()
}

fn bin_of(ln_r: f64) -> Option<usize> {
    let b = ((ln_r + LN_R) / BIN).floor();
    (b >= 0.0 && b < NBINS as f64).then_some(b as usize)
}

/// Smooths a histogram with a 3-bin moving sum (both the sampled and the model
/// histograms get the same treatment, so their ratio stays unbiased).
fn smooth(h: &[f64]) -> Vec<f64> {
    (0..h.len()).map(|i| h[i.saturating_sub(1)..(i + 2).min(h.len())].iter().sum()).collect()
}

fn gcd(a: i32, b: i32) -> i32 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

/// Correction factors per radial bin (complex roots, real roots), sampled / model.
struct Correction {
    complex: Vec<f64>,
    real: Vec<f64>,
}

impl Correction {
    fn at(v: &[f64], r: f64) -> f64 {
        // linear interpolation between bin centres
        let t = ((r.max(1e-300).ln() + LN_R) / BIN - 0.5).clamp(0.0, (NBINS - 1) as f64);
        let i = (t.floor() as usize).min(NBINS - 2);
        let f = t - i as f64;
        v[i] * (1.0 - f) + v[i + 1] * f
    }
}

/// Predicts algebraic numbers per view; caches the expensive parts.
#[derive(Default)]
pub struct Predictor {
    /// per degree: the Gaussian model's expected (complex, real) roots per radial bin
    model: HashMap<usize, (Vec<f64>, Vec<f64>)>,
    /// per (degree, coefficients): the sampled correction
    corr: HashMap<(usize, Coefs), Correction>,
    /// per (degree, coefficients, region): expected roots of one polynomial in the region
    shapes: HashMap<(usize, Coefs, [u64; 4]), f64>,
}

impl Predictor {
    pub fn new() -> Predictor {
        Predictor::default()
    }

    /// The model's radial histograms for degree d (integrated over each annulus).
    fn model(&mut self, d: usize) -> &(Vec<f64>, Vec<f64>) {
        self.model.entry(d).or_insert_with(|| {
            let (mut c, mut r) = (vec![0.0; NBINS], vec![0.0; NBINS]);
            const RADII: usize = 4;
            const ANGLES: usize = 64;
            for b in 0..NBINS {
                let lo = -LN_R + b as f64 * BIN;
                for i in 0..RADII {
                    let rad = (lo + (i as f64 + 0.5) * BIN / RADII as f64).exp();
                    let dr = rad * BIN / RADII as f64; // d(ln r) = dr / r
                    // upper half plane, doubled (the density is symmetric under conjugation)
                    let mut s = 0.0;
                    for j in 0..ANGLES {
                        let t = PI * (j as f64 + 0.5) / ANGLES as f64;
                        s += complex_density(d, rad * t.cos(), rad * t.sin());
                    }
                    c[b] += 2.0 * s * (PI / ANGLES as f64) * rad * dr;
                    r[b] += (real_density(d, rad) + real_density(d, -rad)) * dr;
                }
            }
            (smooth(&c), smooth(&r))
        })
    }

    /// Samples random allowed polynomials of degree d and compares their roots with the model.
    fn correction(&mut self, d: usize, coefs: &Coefs) -> &Correction {
        if !self.corr.contains_key(&(d, *coefs)) {
            let (mc, mr) = self.model(d).clone();
            let vals = coefs.values();
            let nz: Vec<i32> = vals.iter().copied().filter(|&c| c != 0).collect();
            if nz.is_empty() {
                // no polynomials at all; poly_counts is 0, so the correction doesn't matter
                let ones = Correction { complex: vec![1.0; NBINS], real: vec![1.0; NBINS] };
                return self.corr.entry((d, *coefs)).or_insert(ones);
            }
            let samples = SAMPLE_ROOTS.div_ceil(d).max(200);
            let (mut hc, mut hr) = (vec![0.0; NBINS], vec![0.0; NBINS]);
            // fixed seed: the same settings always give the same prediction
            let mut seed = 0x9E3779B97F4A7C15u64 ^ (d as u64) << 32;
            let mut pick = |n: usize| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (seed % n as u64) as usize
            };
            let mut c = vec![0i32; d + 1];
            for _ in 0..samples {
                c[0] = nz[pick(nz.len())];
                c[d] = nz[pick(nz.len())];
                for k in 1..d {
                    c[k] = vals[pick(vals.len())];
                }
                // k * q(x) has the same roots as q(x): if q is also allowed, count those
                // roots once, with q. (In sparse sets q may not be allowed: with
                // triangular numbers, 6x + 3 is allowed but 2x + 1 isn't, so 6x + 3
                // is the only source of the root -1/2.)
                let g = c.iter().fold(0, |g, &v| gcd(g, v));
                if g > 1 && c.iter().all(|&v| vals.binary_search(&(v / g)).is_ok()) {
                    continue;
                }
                let cf: Vec<f64> = c.iter().map(|&v| v as f64).collect();
                for (re, im) in roots(&cf) {
                    let r = re.hypot(im);
                    if let Some(b) = bin_of(r.ln()) {
                        if im.abs() <= 1e-7 * r.max(1.0) {
                            hr[b] += 1.0;
                        } else {
                            hc[b] += 1.0;
                        }
                    }
                }
            }
            let ratio = |h: &[f64], m: &[f64]| -> Vec<f64> {
                smooth(h)
                    .iter()
                    .zip(m)
                    .map(|(h, m)| {
                        let h = h / samples as f64;
                        // where the model expects almost nothing, trust it as it is
                        if *m < 1e-9 { if h > 0.0 { 1.0 } else { 0.0 } } else { h / m }
                    })
                    .collect()
            };
            let corr = Correction { complex: ratio(&hc, &mc), real: ratio(&hr, &mr) };
            self.corr.insert((d, *coefs), corr);
        }
        &self.corr[&(d, *coefs)]
    }

    /// Expected number of roots of one random allowed polynomial of degree d in the region.
    pub fn roots_in(&mut self, d: usize, coefs: &Coefs, r: &Region) -> f64 {
        let key = (d, *coefs, [r.re.0.to_bits(), r.re.1.to_bits(), r.im.0.to_bits(), r.im.1.to_bits()]);
        if let Some(&v) = self.shapes.get(&key) {
            return v;
        }
        if self.shapes.len() > 4096 {
            self.shapes.clear();
        }
        let corr = self.correction(d, coefs);
        let (w, h) = (r.re.1 - r.re.0, r.im.1 - r.im.0);
        let (dx, dy) = (w / GRID as f64, h / GRID as f64);
        let mut sum = 0.0;
        for i in 0..GRID {
            let y = r.im.0 + (i as f64 + 0.5) * dy;
            for j in 0..GRID {
                let x = r.re.0 + (j as f64 + 0.5) * dx;
                sum += complex_density(d, x, y) * Correction::at(&corr.complex, x.hypot(y));
            }
        }
        let mut total = sum * dx * dy;
        if r.im.0 <= 0.0 && r.im.1 >= 0.0 {
            let line: f64 = (0..GRID)
                .map(|j| {
                    let x = r.re.0 + (j as f64 + 0.5) * dx;
                    real_density(d, x) * Correction::at(&corr.real, x.abs())
                })
                .sum();
            total += line * dx;
        }
        self.shapes.insert(key, total);
        total
    }

    /// Expected roots of one polynomial in the region, per degree 0..=order (0 for the
    /// degrees that can't occur). The radial correction is sampled with `coefs`.
    pub fn shape(&mut self, order: usize, coefs: &Coefs, r: &Region) -> Vec<f64> {
        (0..=order)
            .map(|d| if d == 0 || (!coefs.zero && d < order) { 0.0 } else { self.roots_in(d, coefs, r) })
            .collect()
    }

    /// Predicted number of algebraic numbers in the region.
    pub fn roots(&mut self, order: usize, coefs: &Coefs, r: &Region) -> f64 {
        let shape = self.shape(order, coefs, r);
        dot(&shape, &poly_counts(order, coefs))
    }

    /// Predicted algebraic numbers per square pixel.
    pub fn per_pixel(&mut self, order: usize, coefs: &Coefs, r: &Region) -> f64 {
        self.roots(order, coefs, r) / pixels(r)
    }
}

pub fn pixels(r: &Region) -> f64 {
    (r.width * r.height()) as f64
}

pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

/// All roots (re, im) of c[0] + c[1] x + ... + c[d] x^d, c[d] != 0, by Aberth's method.
pub fn roots(c: &[f64]) -> Vec<(f64, f64)> {
    type C = (f64, f64);
    let mul = |a: C, b: C| (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0);
    let div = |a: C, b: C| {
        let n = b.0 * b.0 + b.1 * b.1;
        ((a.0 * b.0 + a.1 * b.1) / n, (a.1 * b.0 - a.0 * b.1) / n)
    };
    let d = c.len() - 1;
    if d == 0 {
        return Vec::new();
    }
    let eval = |z: C| {
        let (mut p, mut dp) = ((0.0, 0.0), (0.0, 0.0));
        for &a in c.iter().rev() {
            let t = mul(dp, z);
            dp = (t.0 + p.0, t.1 + p.1);
            let t = mul(p, z);
            p = (t.0 + a, t.1);
        }
        (p, dp)
    };
    // start on a circle whose radius is the geometric mean of the roots' sizes
    let r = (c[0].abs() / c[d].abs()).powf(1.0 / d as f64).max(1e-3);
    let mut z: Vec<C> = (0..d)
        .map(|k| {
            let t = 2.0 * PI * k as f64 / d as f64 + 0.4;
            (r * t.cos(), r * t.sin())
        })
        .collect();
    for _ in 0..200 {
        let mut moved = 0.0f64;
        for i in 0..d {
            let (p, dp) = eval(z[i]);
            if p == (0.0, 0.0) {
                continue;
            }
            let ratio = div(p, dp);
            let mut s = (0.0, 0.0);
            for j in 0..d {
                if j != i {
                    let t = div((1.0, 0.0), (z[i].0 - z[j].0, z[i].1 - z[j].1));
                    s = (s.0 + t.0, s.1 + t.1);
                }
            }
            let rs = mul(ratio, s);
            let w = div(ratio, (1.0 - rs.0, -rs.1));
            if !(w.0.is_finite() && w.1.is_finite()) {
                continue;
            }
            z[i] = (z[i].0 - w.0, z[i].1 - w.1);
            moved = moved.max(w.0.hypot(w.1) / z[i].0.hypot(z[i].1).max(1e-300));
        }
        if moved < 1e-12 {
            break;
        }
    }
    z
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A degree-d polynomial has d roots: the model's densities must integrate to d.
    #[test]
    fn model_counts_all_roots() {
        let mut p = Predictor::new();
        for d in [1, 2, 3, 5, 8, 16] {
            let (c, r) = p.model(d).clone();
            // undo the smoothing: every bin is counted 3 times (the ends hold almost nothing)
            let total = (c.iter().sum::<f64>() + r.iter().sum::<f64>()) / 3.0;
            assert!((total - d as f64).abs() < 0.005 * d as f64, "degree {d}: {total}");
        }
    }

    #[test]
    fn roots_are_found() {
        // (x - 2)(x + 1)(x^2 + 1) = x^4 - x^3 - x^2 - x - 2
        let mut r = roots(&[-2.0, -1.0, -1.0, -1.0, 1.0]);
        r.sort_by(|a, b| (a.0.round(), a.1).partial_cmp(&(b.0.round(), b.1)).unwrap());
        let want = [(-1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (2.0, 0.0)];
        for (a, b) in r.iter().zip(want) {
            assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9, "{r:?}");
        }
    }
}
