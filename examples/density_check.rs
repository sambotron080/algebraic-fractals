//! Compares density::predicted_roots with the true number of distinct algebraic numbers
//! in a few views, found by solving every allowed polynomial.
//!   cargo run --release --example density_check

use algfrac::density::Predictor;
use algfrac::engine::{CoefSet, Coefs, Region};
use std::collections::HashSet;

#[derive(Clone, Copy)]
struct C(f64, f64);
impl C {
    fn add(self, o: C) -> C { C(self.0 + o.0, self.1 + o.1) }
    fn sub(self, o: C) -> C { C(self.0 - o.0, self.1 - o.1) }
    fn mul(self, o: C) -> C { C(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0) }
    fn div(self, o: C) -> C {
        let d = o.0 * o.0 + o.1 * o.1;
        C((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d)
    }
    fn abs(self) -> f64 { self.0.hypot(self.1) }
}

/// All roots of c[0] + c[1] x + ... + c[d] x^d (c[d] != 0), by Aberth's method.
fn roots(c: &[f64]) -> Vec<C> {
    let d = c.len() - 1;
    let eval = |z: C| {
        let (mut p, mut dp) = (C(0.0, 0.0), C(0.0, 0.0));
        for &a in c.iter().rev() {
            dp = dp.mul(z).add(p);
            p = p.mul(z).add(C(a, 0.0));
        }
        (p, dp)
    };
    // start on a circle of the Cauchy-bound-ish radius
    let r = (c[0].abs() / c[d].abs()).powf(1.0 / d as f64).max(0.1);
    let mut z: Vec<C> = (0..d).map(|k| {
        let t = 2.0 * std::f64::consts::PI * k as f64 / d as f64 + 0.4;
        C(r * t.cos(), r * t.sin())
    }).collect();
    for _ in 0..500 {
        let mut moved = 0.0f64;
        for i in 0..d {
            let (p, dp) = eval(z[i]);
            if p.abs() == 0.0 { continue; }
            let ratio = p.div(dp);
            let mut s = C(0.0, 0.0);
            for j in 0..d {
                if j != i { s = s.add(C(1.0, 0.0).div(z[i].sub(z[j]))); }
            }
            let w = ratio.div(C(1.0, 0.0).sub(ratio.mul(s)));
            z[i] = z[i].sub(w);
            moved = moved.max(w.abs() / z[i].abs().max(1e-300));
        }
        if moved < 1e-14 { break; }
    }
    z
}

fn main() {
    let views = [
        ("whole -2..2", Region::square(0.0, 0.0, 2.0, 256)),
        ("off-axis 0.5+0.8i hw .25", Region::square(0.5, 0.8, 0.25, 256)),
        ("spiral 1.9+.19i hw .16", Region::square(1.9, 0.19, 0.16, 256)),
        ("axis 0.6 hw .1", Region::square(0.6, 0.0, 0.1, 256)),
        ("inside 0.2+0.3i hw .1", Region::square(0.2, 0.3, 0.1, 256)),
        ("far 3+3i hw 1", Region::square(3.0, 3.0, 1.0, 256)),
    ];
    let cases = [
        (2, Coefs::ints(30)),
        (3, Coefs::ints(12)),
        (4, Coefs::ints(6)),
        (5, Coefs::ints(3)),
        (6, Coefs::ints(2)),
        (8, Coefs::ints(1)),
        (4, Coefs { set: CoefSet::Primes, ..Coefs::ints(11) }),
        (4, Coefs { zero: false, ..Coefs::ints(7) }),
        (3, Coefs { set: CoefSet::Squares, ..Coefs::ints(100) }),
        (3, Coefs { set: CoefSet::Powers2, ..Coefs::ints(256) }),
        (4, Coefs { set: CoefSet::Odd, ..Coefs::ints(9) }),
    ];
    let mut p = Predictor::new();
    for (order, coefs) in cases {
        let vals = coefs.values();
        let nz: Vec<i32> = vals.iter().copied().filter(|&c| c != 0).collect();
        // distinct roots, rounded to merge copies of the same algebraic number
        let mut seen: HashSet<(i64, i64)> = HashSet::new();
        let mut all = Vec::new();
        let mut c = vec![0i32; order + 1];
        // enumerate: c0 nonzero, leading coefficient positive (p ~ -p)
        let n = vals.len();
        let total = n.pow(order as u32);
        for idx in 0..total * nz.len() {
            let mut t = idx;
            c[0] = nz[t % nz.len()];
            t /= nz.len();
            for k in 1..=order {
                c[k] = vals[t % n];
                t /= n;
            }
            let Some(d) = (1..=order).rev().find(|&k| c[k] != 0) else { continue };
            if c[d] < 0 { continue; }
            let cf: Vec<f64> = c[..=d].iter().map(|&v| v as f64).collect();
            for z in roots(&cf) {
                let key = ((z.0 * 1e7).round() as i64, (z.1 * 1e7).round() as i64);
                if seen.insert(key) {
                    all.push(z);
                }
            }
        }
        println!("order {order}, {}:", coefs.describe());
        for (name, r) in &views {
            let actual = all.iter().filter(|z| z.0 >= r.re.0 && z.0 < r.re.1 && z.1 >= r.im.0 && z.1 < r.im.1).count();
            let pred = p.roots(order, &coefs, r);
            println!("  {name:28} actual {actual:8}  predicted {pred:10.0}  ratio {:.2}", pred / actual.max(1) as f64);
        }
    }

    // timing: what the GUI's auto order does on a fresh view
    let t = std::time::Instant::now();
    let r = Region::square(0.3, 0.9, 0.01, 512);
    for order in 1..=16 {
        p.per_pixel(order, &Coefs::ints(12), &r);
    }
    println!("first prediction for orders 1..16: {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    let t = std::time::Instant::now();
    let r = Region::square(0.31, 0.9, 0.005, 512);
    for order in 1..=16 {
        p.per_pixel(order, &Coefs::ints(12), &r);
    }
    println!("same coefficients, new view: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
}
