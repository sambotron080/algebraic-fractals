//! The search engine: for every pixel X of a region of the complex plane, find
//!
//! ```text
//!     residual(X) = min |c0 + c1 X + c2 X^2 + ... + cn X^n|
//! ```
//!
//! over integer coefficients |ci| <= maxcoef (or only 0 and +-members of a
//! sparser set: primes, Fibonacci numbers, squares, ...) with c0 != 0,
//! together with the coefficients that achieve it.
//!
//! Evaluating all (2M+1)^(n+1) polynomials at once is slow and needs a lot
//! of memory. Here the polynomial is split in two
//! ("meet in the middle"):
//!
//! ```text
//!     p(X) = A + B,   A = c0 + ... + c(k-1) X^(k-1)    (the table)
//!                     B = ck X^k + ... + cn X^n        (the queries)
//! ```
//!
//! |p(X)| = |A - (-B)| is the distance between the points A and -B, so the
//! smallest residual is the closest pair between the two sets. The A points
//! are bucketed into a 2-D grid, and each -B only checks the grid cells
//! within the best distance found so far. Since p and -p give the same
//! residual, only half of the B vectors need checking.
//!
//! The queries are walked as a tree, fixing the highest coefficient first.
//! Every subtree's queries lie in a known disc, so whole subtrees are
//! skipped when that disc can't come within the best distance of the table.
//! When the disc is small compared with the best distance (e.g. near X = 0,
//! where high powers vanish), the nearby table entries are collected once
//! and shared by the whole subtree.
//!
//! Work per pixel drops from (2M+1)^(n+1) to about (2M+1)^k + (2M+1)^(n+1-k)/2,
//! and memory to the table alone (a few MB rather than many GB).
//! E.g. order 4, M = 32: 1.16e9 polynomials (18.6 GB) -> ~4e3 + ~1.4e5 steps.
//!
//! In deep views one search serves a whole tile of pixels (see "tiles" below).

use std::ops::{Add, Mul};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;

// ---------------------------------------------------------------- complex numbers

#[derive(Clone, Copy, Debug, Default)]
struct C64 {
    re: f64,
    im: f64,
}

impl C64 {
    const ZERO: C64 = C64 { re: 0.0, im: 0.0 };
    const ONE: C64 = C64 { re: 1.0, im: 0.0 };

    fn new(re: f64, im: f64) -> C64 {
        C64 { re, im }
    }
    fn conj(self) -> C64 {
        C64::new(self.re, -self.im)
    }
    fn scale(self, k: f64) -> C64 {
        C64::new(self.re * k, self.im * k)
    }
    fn norm2(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

impl Add for C64 {
    type Output = C64;
    fn add(self, o: C64) -> C64 {
        C64::new(self.re + o.re, self.im + o.im)
    }
}

impl Mul for C64 {
    type Output = C64;
    fn mul(self, o: C64) -> C64 {
        C64::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
}

// ---------------------------------------------------------------- search plan

/// Which sizes of coefficient are allowed (each with either sign). Every set
/// contains 1; the sparser ones space the coefficients out more and more.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CoefSet {
    /// every integer: 1, 2, 3, 4, ...
    Ints,
    /// 1 and the primes: 1, 2, 3, 5, 7, 11, ...
    Primes,
    /// Fibonacci numbers: 1, 2, 3, 5, 8, 13, ...
    Fibonacci,
    /// odd numbers: 1, 3, 5, 7, ...
    Odd,
    /// triangular numbers k(k+1)/2: 1, 3, 6, 10, 15, ...
    Triangular,
    /// squares: 1, 4, 9, 16, ...
    Squares,
    /// cubes: 1, 8, 27, 64, ...
    Cubes,
    /// powers of 2: 1, 2, 4, 8, 16, ...
    Powers2,
}

impl CoefSet {
    pub const ALL: [CoefSet; 8] = [
        CoefSet::Ints,
        CoefSet::Primes,
        CoefSet::Fibonacci,
        CoefSet::Odd,
        CoefSet::Triangular,
        CoefSet::Squares,
        CoefSet::Cubes,
        CoefSet::Powers2,
    ];

    /// Identifier used on the command line and in saved places.
    pub fn name(self) -> &'static str {
        match self {
            CoefSet::Ints => "ints",
            CoefSet::Primes => "primes",
            CoefSet::Fibonacci => "fibonacci",
            CoefSet::Odd => "odd",
            CoefSet::Triangular => "triangular",
            CoefSet::Squares => "squares",
            CoefSet::Cubes => "cubes",
            CoefSet::Powers2 => "pow2",
        }
    }

    /// For people, e.g. "1 and primes".
    pub fn label(self) -> &'static str {
        match self {
            CoefSet::Ints => "integers",
            CoefSet::Primes => "1 and primes",
            CoefSet::Fibonacci => "Fibonacci numbers",
            CoefSet::Odd => "odd numbers",
            CoefSet::Triangular => "triangular numbers",
            CoefSet::Squares => "squares",
            CoefSet::Cubes => "cubes",
            CoefSet::Powers2 => "powers of 2",
        }
    }

    /// Short tag for file names ("" for integers).
    pub fn tag(self) -> &'static str {
        match self {
            CoefSet::Ints => "",
            CoefSet::Primes => "p",
            CoefSet::Fibonacci => "f",
            CoefSet::Odd => "o",
            CoefSet::Triangular => "t",
            CoefSet::Squares => "s",
            CoefSet::Cubes => "c",
            CoefSet::Powers2 => "b",
        }
    }

    /// The members from 1 up to `max`, ascending.
    pub fn upto(self, max: i32) -> Vec<i32> {
        // the k-th member (k = 1, 2, ...); all are increasing in k
        let nth = |k: i64| -> i64 {
            match self {
                CoefSet::Odd => 2 * k - 1,
                CoefSet::Triangular => k * (k + 1) / 2,
                CoefSet::Squares => k * k,
                CoefSet::Cubes => k * k * k,
                CoefSet::Powers2 => 1 << (k - 1).min(62),
                _ => k,
            }
        };
        match self {
            CoefSet::Primes => std::iter::once(1).chain(primes_upto(max)).collect(),
            CoefSet::Fibonacci => {
                let (mut v, mut a, mut b) = (Vec::new(), 1i64, 2i64);
                while a <= max as i64 {
                    v.push(a as i32);
                    (a, b) = (b, a + b);
                }
                v
            }
            _ => (1..).map(nth).take_while(|&v| v <= max as i64).map(|v| v as i32).collect(),
        }
    }
}

/// Which coefficients are allowed: the members of the set up to `max`, with
/// either sign, plus 0 if `zero`. The constant term c0 may only be 0 if `zero_root`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Coefs {
    pub set: CoefSet,
    pub max: i32,
    pub zero: bool,
    /// allow c0 = 0, i.e. polynomials x*q(x), so that X = 0 counts as a root
    pub zero_root: bool,
}

impl Coefs {
    /// Every integer in [-max, max], with c0 != 0.
    pub fn ints(max: i32) -> Coefs {
        Coefs { set: CoefSet::Ints, max, zero: true, zero_root: false }
    }

    /// The allowed values, ascending.
    pub fn values(&self) -> Vec<i32> {
        let mags = self.set.upto(self.max);
        let mut out: Vec<i32> = mags.iter().rev().map(|&m| -m).collect();
        if self.zero {
            out.push(0);
        }
        out.extend(&mags);
        out
    }

    /// e.g. "squares |c| <= 100, and 0"
    pub fn describe(&self) -> String {
        format!(
            "{} |c| <= {}{}{}",
            self.set.label(),
            self.max,
            if self.zero { ", and 0" } else { ", no 0" },
            if self.zero_root { "; 0 is a root" } else { "" }
        )
    }
}

/// The most entries a search table may have. Each thread builds its own, at about 52
/// bytes an entry (the table twice, sorted and unsorted, plus a cell index), so this is
/// about 3.5 GB per thread. At order 3 the usual table holds (2M+1)² entries, which
/// fits up to max coefficient 4095; above that the direct search (used near the real
/// axis) builds it in chunks, running its queries once per chunk: 6 chunks (about 2.7
/// times the work) at max coefficient 10 000. (Table codes are u32, so this must stay
/// below 2³².)
const TABLE_MAX: f64 = (1u64 << 26) as f64;

/// How the lattice search (see LatWalk) splits the powers: `lat` and `lat + 1`
/// are solved for directly, the rest are enumerated as table and queries.
struct LatSplit {
    lat: usize,
    table: Vec<usize>,
    query: Vec<usize>,
    /// table size + number of queries
    cost: f64,
}

/// What is searched, and how it is split between table and queries.
/// The same for every pixel.
pub struct Plan {
    pub nterms: usize,
    /// powers 0..split form the table (A), split..nterms the queries (B)
    pub split: usize,
    /// how many of power split-1's coefficients each chunk of the table has (all of
    /// them unless the whole table would be too big: see TABLE_MAX)
    chunk: usize,
    /// 0..split
    table_powers: Vec<usize>,
    /// allowed coefficients for each power, ascending
    lists: Vec<Vec<i32>>,
    /// index of the first coefficient >= 0 in each list
    first_nonneg: Vec<usize>,
    /// largest |coefficient| in each list
    max_abs: Vec<f64>,
    legacy: bool,
    /// lattice search with the lattice on powers (0, 1) for |X| <= 1 and on
    /// (n-1, n) for |X| > 1; None where it isn't worthwhile
    lattice: Option<[LatSplit; 2]>,
}

fn primes_upto(n: i32) -> Vec<i32> {
    let n = n.max(0) as usize;
    let mut sieve = vec![true; n + 1];
    let mut out = Vec::new();
    for p in 2..=n {
        if sieve[p] {
            out.push(p as i32);
            for q in (p * p..=n).step_by(p) {
                sieve[q] = false;
            }
        }
    }
    out
}

impl Plan {
    /// `legacy` reproduces QuadResFast/QuadraliseFast: order 2, the linear term
    /// uses conj(X), and c1 = 0 is excluded instead of c0 = 0.
    pub fn new(order: usize, coefs: &Coefs, legacy: bool) -> Result<Plan, String> {
        Plan::with_table_limit(order, coefs, legacy, TABLE_MAX)
    }

    /// Like `new`, with tables of at most `limit` entries.
    fn with_table_limit(order: usize, coefs: &Coefs, legacy: bool, limit: f64) -> Result<Plan, String> {
        if order < 1 {
            return Err("order must be at least 1".into());
        }
        if coefs.max < 1 {
            return Err("max coefficient must be at least 1".into());
        }
        if legacy && order != 2 {
            return Err("legacy mode only applies to order 2".into());
        }
        let base = coefs.values(); // never empty: every set contains +-1
        let nterms = order + 1;

        // c0 = 0 is excluded unless zero_root: then p(x) = x*q(x), which only adds the
        // root 0 or a lower-order polynomial scaled by |X|. The legacy quadratic
        // excluded c1 = 0 instead.
        let excluded = if legacy { 1 } else { 0 };
        let lists: Vec<Vec<i32>> = (0..nterms)
            .map(|i| {
                if i == excluded {
                    let mut l: Vec<i32> = base.iter().copied().filter(|&c| c != 0).collect();
                    if coefs.zero_root && !legacy {
                        l.insert(l.partition_point(|&c| c < 0), 0);
                    }
                    l
                } else {
                    base.clone()
                }
            })
            .collect();
        let first_nonneg = lists.iter().map(|l| l.partition_point(|&c| c < 0)).collect();
        let max_abs = lists
            .iter()
            .map(|l| l.iter().map(|c| c.unsigned_abs()).max().unwrap_or(0) as f64)
            .collect();

        // Choose the split that minimises (table size + number of queries). Queries are
        // halved by the p / -p symmetry. If the table would have more than `limit`
        // entries, it's built in chunks instead: the top table power's coefficients a
        // chunk at a time, with all the queries run against each chunk. That's only
        // slower than one big table by the repeated queries, and every split is
        // costed that way, so the balanced split stays unless chunks make another cheaper.
        let count = |r: std::ops::Range<usize>| r.map(|i| lists[i].len() as f64).product::<f64>();
        // (values of the top table power per chunk, number of chunks)
        let chunking = |k: usize| {
            let len = lists[k - 1].len();
            let per = ((limit / count(0..k - 1)).floor() as usize).clamp(1, len);
            (per, len.div_ceil(per))
        };
        let cost = |k: usize| count(0..k) + chunking(k).1 as f64 * count(k..nterms) / 2.0;
        if count(0..1) > limit {
            return Err(format!("search too large: a table of {:.3e} entries; reduce the max coefficient", count(0..1)));
        }
        // (a chunk has at least one coefficient of the top table power, so the powers below it must fit)
        let split = (1..nterms)
            .filter(|&k| count(0..k - 1) <= limit)
            .min_by(|&a, &b| cost(a).partial_cmp(&cost(b)).unwrap())
            .unwrap();
        let chunk = chunking(split).0;

        // The lattice search, on the lowest two powers and on the highest two.
        let lat_split = |lat: usize| -> LatSplit {
            let rest: Vec<usize> = (0..nterms).filter(|&p| p != lat && p != lat + 1).collect();
            let count = |ps: &[usize]| ps.iter().map(|&p| lists[p].len() as f64).product::<f64>();
            let cost = |k: usize| count(&rest[..k]) + count(&rest[k..]) / 2.0;
            // (k = 0 always fits: a table of just the zero sum)
            let k = (0..=rest.len())
                .filter(|&k| count(&rest[..k]) <= limit)
                .min_by(|&a, &b| cost(a).partial_cmp(&cost(b)).unwrap())
                .unwrap();
            LatSplit { lat, table: rest[..k].to_vec(), query: rest[k..].to_vec(), cost: cost(k) }
        };
        let lattice = if legacy || order < 2 {
            None
        } else {
            let ls = [lat_split(0), lat_split(order - 1)];
            let fits = ls.iter().all(|l| l.table.iter().map(|&p| lists[p].len() as f64).product::<f64>() <= limit);
            // only worth it if it saves a good part of the work
            (fits && ls.iter().all(|l| l.cost * 2.0 < cost(split))).then_some(ls)
        };

        Ok(Plan { nterms, split, chunk, table_powers: (0..split).collect(), lists, first_nonneg, max_abs, legacy, lattice })
    }

    /// Number of entries in the direct search's table (in one chunk of it, if it's
    /// built in chunks): what it needs in memory.
    pub fn table_len(&self) -> usize {
        self.lists[..self.split - 1].iter().map(|l| l.len()).product::<usize>() * self.chunk
    }

    /// How many chunks the direct search's table is built in (usually 1). The queries
    /// run once per chunk.
    pub fn chunks(&self) -> usize {
        self.lists[self.split - 1].len().div_ceil(self.chunk)
    }

    /// The direct search's work per pixel: all its table entries, and its queries once per chunk.
    fn direct_steps(&self) -> f64 {
        let entries: f64 = self.lists[..self.split].iter().map(|l| l.len() as f64).product();
        entries + self.chunks() as f64 * self.query_len()
    }

    /// Typical work per pixel (table entries + queries), using the lattice search
    /// where it applies.
    pub fn steps(&self) -> f64 {
        let direct = self.direct_steps();
        match &self.lattice {
            Some(ls) => ls.iter().map(|l| l.cost).fold(direct, f64::min),
            None => direct,
        }
    }

    /// Entries in the largest lattice-search table, if the lattice search applies.
    /// The direct search's table (table_len) is then only built where the lattice
    /// search gives up, mainly near the real axis.
    pub fn lattice_table_len(&self) -> Option<usize> {
        let lens = self.lattice.iter().flatten().map(|l| l.table.iter().map(|&p| self.lists[p].len()).product::<usize>());
        lens.max()
    }

    /// How fast any allowed polynomial can change with X for |X| <= a: the sum of
    /// max|ci| i a^(i-1), so |p(X) - p(Y)| <= slope * |X - Y| for |X|, |Y| <= a.
    fn slope(&self, a: f64) -> f64 {
        (1..self.nterms).map(|i| self.max_abs[i] * i as f64 * a.powi(i as i32 - 1)).sum()
    }

    /// The tie tolerance (see Solver::set_x) for |X| <= a.
    fn tol_upto(&self, a: f64) -> f64 {
        64.0 * f64::EPSILON * (0..self.nterms).map(|i| self.max_abs[i] * a.powi(i as i32)).sum::<f64>()
    }

    /// Some allowed nonzero polynomial (every set contains 1), as a starting bound.
    fn any_poly(&self) -> Vec<i32> {
        self.lists.iter().map(|l| if l.contains(&1) { 1 } else { l[l.len() - 1] }).collect()
    }

    /// Number of queries per pixel (before pruning).
    pub fn query_len(&self) -> f64 {
        self.lists[self.split..].iter().map(|l| l.len() as f64).product::<f64>() / 2.0
    }

    /// Number of polynomials a brute-force search would evaluate per pixel.
    pub fn brute_len(&self) -> f64 {
        self.lists.iter().map(|l| l.len() as f64).product()
    }
}

// ---------------------------------------------------------------- table grid

/// One table point A = sum of the low-half terms.
#[derive(Clone, Copy, Default)]
struct Entry {
    re: f64,
    im: f64,
    /// mixed-radix index of the low-half coefficients (digit 0 fastest)
    code: u32,
    /// sum of |coefficient| of the low half
    l1: u32,
}

/// Uniform 2-D bucket grid over the bounding box of the table.
#[derive(Clone, Copy)]
struct Grid {
    gx: usize,
    gy: usize,
    min_re: f64,
    max_re: f64,
    min_im: f64,
    max_im: f64,
    ix: f64,
    iy: f64,
}

impl Grid {
    /// Sizes the grid so that each cell holds about one of the n entries.
    fn new(n: usize, min_re: f64, max_re: f64, min_im: f64, max_im: f64) -> Grid {
        let (w, h) = (max_re - min_re, max_im - min_im);
        let (gx, gy) = if n <= 1 || (w <= 0.0 && h <= 0.0) {
            (1, 1)
        } else if h <= 0.0 {
            (n, 1)
        } else if w <= 0.0 {
            (1, n)
        } else {
            let gx = ((n as f64 * w / h).sqrt().round() as usize).clamp(1, n);
            (gx, (n / gx).clamp(1, n))
        };
        let ix = if w > 0.0 { gx as f64 / w } else { 0.0 };
        let iy = if h > 0.0 { gy as f64 / h } else { 0.0 };
        Grid { gx, gy, min_re, max_re, min_im, max_im, ix, iy }
    }

    // float -> usize casts saturate, so points left of / below the box map to 0
    fn cx(&self, re: f64) -> usize {
        (((re - self.min_re) * self.ix) as usize).min(self.gx - 1)
    }
    fn cy(&self, im: f64) -> usize {
        (((im - self.min_im) * self.iy) as usize).min(self.gy - 1)
    }
    fn cell(&self, re: f64, im: f64) -> usize {
        self.cy(im) * self.gx + self.cx(re)
    }

    /// True if the disc of radius r around (re, im) misses the bounding box.
    fn misses(&self, re: f64, im: f64, r: f64) -> bool {
        re + r < self.min_re || re - r > self.max_re || im + r < self.min_im || im - r > self.max_im
    }

    /// Ranges of table indices covering the square of half-width r around (re, im),
    /// one contiguous range per grid row.
    fn rows(&self, re: f64, im: f64, r: f64) -> (usize, usize, std::ops::RangeInclusive<usize>) {
        (self.cx(re - r), self.cx(re + r), self.cy(im - r)..=self.cy(im + r))
    }
}

// ---------------------------------------------------------------- query walk

/// The search over the query half for one pixel. The query coefficients form
/// a tree: level j fixes the coefficient of power split+j, the highest first.
struct Walk<'s, 'c> {
    tl: &'s [Vec<i32>], // allowed coefficients of each table power
    t_off: usize,       // index of the top table power's first coefficient in this chunk
    hl: &'s [Vec<i32>], // allowed coefficients of each query power
    ht: &'s [Vec<C64>], // their terms c * X^power
    first_nonneg: &'s [usize],
    rad: &'s [f64], // all sums of the query terms below level j lie within rad[j] of 0
    table: &'s [Entry],
    start: &'s [u32], // table[start[c]..start[c+1]] lie in grid cell c
    grid: Grid,
    tol: f64,           // residuals closer than this are treated as equal
    d: &'s mut [usize], // current query digits
    // the winner so far
    win_d: f64,
    win_l1: u32,
    win_code: u32,
    win_hi: &'s mut [usize],
    found: bool,
    reach: f64, // win_d + tol: anything further than this from a query cannot win
    reach2: f64,
    collect: Collect<'c>,
}

/// Collect mode (see Solver::collect): every polynomial within the starting reach
/// is appended to `hits` as [c0..cn], and the reach never shrinks.
struct Collect<'s> {
    hits: Option<&'s mut Vec<i32>>,
    /// stop once there are more than this many polynomials
    cap: usize,
    over: bool,
}

impl<'s> Collect<'s> {
    fn off() -> Collect<'s> {
        Collect { hits: None, cap: 0, over: false }
    }
}

impl<'s, 'c> Walk<'s, 'c> {
    /// Checks one (table entry, query) pair. Clearly smaller residuals win; within
    /// `tol` the simplest polynomial (smallest sum |ci|) wins.
    #[inline(always)]
    fn test(&mut self, e: &Entry, tr: f64, ti: f64, l1q: u32, i0: usize) {
        let (dx, dy) = (e.re - tr, e.im - ti);
        let d2 = dx * dx + dy * dy;
        if d2 <= self.reach2 {
            let dist = d2.sqrt();
            let l1 = e.l1 + l1q;
            if l1 == 0 {
                return; // the zero polynomial (possible when c0 = 0 is allowed)
            }
            if let Some(hits) = &mut self.collect.hits {
                let mut code = e.code as usize;
                for (i, l) in self.tl.iter().enumerate() {
                    hits.push(l[code % l.len() + if i + 1 == self.tl.len() { self.t_off } else { 0 }]);
                    code /= l.len();
                }
                for (j, l) in self.hl.iter().enumerate() {
                    hits.push(l[if j == 0 { i0 } else { self.d[j] }]);
                }
                if hits.len() > self.collect.cap * (self.tl.len() + self.hl.len()) {
                    self.collect.over = true;
                    self.reach2 = -1.0;
                }
                return;
            }
            if dist + self.tol < self.win_d
                || l1 < self.win_l1
                || (l1 == self.win_l1 && dist < self.win_d)
            {
                self.win_d = dist;
                self.reach = dist + self.tol;
                self.reach2 = self.reach * self.reach;
                self.win_l1 = l1;
                self.win_code = e.code;
                self.win_hi.copy_from_slice(&*self.d);
                self.win_hi[0] = i0;
                self.found = true;
            }
        }
    }

    /// Visits every query whose digits at levels >= j are fixed, with those
    /// terms summing to `s`. `sgn` is the sign of the highest nonzero fixed
    /// coefficient and `l1` the sum of their |c|. `cand`, if present, already
    /// holds every table entry that can matter for this subtree. `bufs[j]` is
    /// scratch space for level j's candidate list.
    fn walk(&mut self, j: usize, s: C64, sgn: i32, l1: u32, cand: Option<&[Entry]>, bufs: &mut [Vec<Entry>]) {
        if self.collect.over {
            return;
        }
        // every target -B in this subtree lies within rad[j] of -s
        let (cr, ci) = (-s.re, -s.im);
        let r = self.rad[j] + self.reach;
        if self.grid.misses(cr, ci, r) {
            return;
        }

        let (lower, here) = bufs.split_at_mut(j);
        let mut cand = cand;
        // If the subtree is tight compared with the best distance, collect the table
        // entries that can matter once (or narrow down the inherited list).
        if self.reach.is_finite() && self.rad[j] <= self.reach && cand.map_or(true, |c| c.len() > 8) {
            let buf = &mut here[0];
            buf.clear();
            let r2 = r * r;
            let near = |e: &Entry| (e.re - cr) * (e.re - cr) + (e.im - ci) * (e.im - ci) <= r2;
            match cand {
                Some(c) => buf.extend(c.iter().filter(|e| near(*e))),
                None => {
                    let (x0, x1, ys) = self.grid.rows(cr, ci, r);
                    for y in ys {
                        let row = y * self.grid.gx;
                        let cells = &self.table[self.start[row + x0] as usize..self.start[row + x1 + 1] as usize];
                        buf.extend(cells.iter().filter(|e| near(*e)));
                    }
                }
            }
            if buf.is_empty() {
                return;
            }
            cand = Some(&buf[..]);
        }

        // p and -p have the same residual, so only queries whose highest nonzero
        // coefficient is positive are visited (plus the all-zero one): while no
        // nonzero coefficient is fixed yet, skip the negative choices.
        if j == 1 {
            // the innermost level: each choice of digit 0 is one query
            let s0 = if sgn > 0 { 0 } else { self.first_nonneg[0] };
            let (hl, ht) = (self.hl, self.ht); // copy the references out so self.test can borrow self
            let (hl0, ht0) = (hl[0].as_slice(), ht[0].as_slice());
            for i0 in s0..hl0.len() {
                let b = ht0[i0] + s;
                let (tr, ti) = (-b.re, -b.im); // looking for A close to -B
                let l1q = l1 + hl0[i0].unsigned_abs();
                match cand {
                    Some(c) => {
                        for e in c {
                            self.test(e, tr, ti, l1q, i0);
                        }
                    }
                    None => {
                        if self.grid.misses(tr, ti, self.reach) {
                            continue;
                        }
                        let (x0, x1, ys) = self.grid.rows(tr, ti, self.reach);
                        let (table, start, gx) = (self.table, self.start, self.grid.gx);
                        for y in ys {
                            let row = y * gx;
                            for e in &table[start[row + x0] as usize..start[row + x1 + 1] as usize] {
                                self.test(e, tr, ti, l1q, i0);
                            }
                        }
                    }
                }
            }
            return;
        }

        let lvl = j - 1;
        let s0 = if sgn > 0 { 0 } else { self.first_nonneg[lvl] };
        for idx in s0..self.hl[lvl].len() {
            let c = self.hl[lvl][idx];
            self.d[lvl] = idx;
            let s2 = self.ht[lvl][idx] + s;
            let sgn2 = if sgn != 0 { sgn } else { c.signum() };
            self.walk(lvl, s2, sgn2, l1 + c.unsigned_abs(), cand, lower);
        }
    }
}

// ---------------------------------------------------------------- lattice walk

/// The lattice search for one pixel. Two consecutive powers j and j+1 are not
/// enumerated at all: whatever the other coefficients sum to (call it S),
/// c_j X^j + c_(j+1) X^(j+1) ranges over the lattice spanned by e1 = X^j and
/// e2 = X^(j+1), and the best (c_j, c_(j+1)) are the lattice points nearest -S.
/// Finding those takes O(1). So only the other n-1 coefficients are searched,
/// again by meet in the middle, but now for pairs (A, B) where A + B is close
/// to *some lattice point*: a closest-pair search on the torus C / lattice.
/// Table points are bucketed by their position inside the lattice cell, and
/// each query looks only in the cells near its own position.
///
/// This removes a factor of about (2M+1) from the work. It is used for
/// |X| <= 1 with the lattice on powers 0, 1 and for |X| > 1 on n-1, n, i.e.
/// always on the largest terms. It does badly where the lattice is nearly
/// flat (X close to the real axis), so it gives up after a work budget and the
/// direct search takes over.
struct LatWalk<'s> {
    lists: &'s [Vec<i32>],
    terms: &'s [Vec<C64>],
    first_nonneg: &'s [usize],
    qpow: &'s [usize], // the query powers, by level
    tpow: &'s [usize], // the table powers, digit 0 fastest in the code
    lat: usize,
    e1: C64,
    e2: C64,
    inv_e1: C64,
    det: f64, // Im(conj(e1) e2), signed area of the lattice cell
    n1: f64,  // |e1|
    n2: f64,  // |e2|
    table: &'s [Entry],
    start: &'s [u32],
    gx: usize,
    gy: usize,
    tol: f64,
    d: &'s mut [usize],
    win_d: f64,
    win_l1: u32,
    win_code: u32,
    win_hi: &'s mut [usize],
    win_lat: (i32, i32),
    found: bool,
    reach: f64,
    reach2: f64,
    work: usize,
    budget: usize,
    collect: Collect<'s>,
}

impl<'s> LatWalk<'s> {
    /// Lattice coordinates (a, b) with p = a e1 + b e2.
    #[inline(always)]
    fn coords(&self, p: C64) -> (f64, f64) {
        ((p.re * self.e2.im - self.e2.re * p.im) / self.det, (self.e1.re * p.im - self.e1.im * p.re) / self.det)
    }

    /// Visits the canonical-sign queries (see Walk::walk) below level j.
    fn walk(&mut self, j: usize, s: C64, sgn: i32, l1: u32) {
        if self.work > self.budget || self.collect.over {
            return;
        }
        if j == 0 {
            self.leaf(s, l1);
            return;
        }
        let lvl = j - 1;
        let p = self.qpow[lvl];
        let (list, terms) = (&self.lists[p], &self.terms[p]);
        let s0 = if sgn > 0 { 0 } else { self.first_nonneg[p] };
        for idx in s0..list.len() {
            let c = list[idx];
            self.d[lvl] = idx;
            let sgn2 = if sgn != 0 { sgn } else { c.signum() };
            self.walk(lvl, terms[idx] + s, sgn2, l1 + c.unsigned_abs());
        }
    }

    /// One query B: checks the table entries whose torus position is within
    /// `reach` of that of -B.
    fn leaf(&mut self, b: C64, l1q: u32) {
        self.work += 1;
        let (a, bb) = self.coords(C64::new(-b.re, -b.im));
        let area = self.det.abs();
        // a disc of radius r spans r|e2|/area in a and r|e1|/area in b
        let (da, db) = (self.reach * self.n2 / area, self.reach * self.n1 / area);
        let span = |f: f64, r: f64, g: usize| -> (i64, i64) {
            let f = f - f.floor();
            let (lo, hi) = (((f - r) * g as f64).floor(), ((f + r) * g as f64).floor());
            if !(hi - lo + 1.0 < g as f64) {
                (0, g as i64 - 1)
            } else {
                (lo as i64, hi as i64)
            }
        };
        let (x0, x1) = span(a, da, self.gx);
        let (y0, y1) = span(bb, db, self.gy);
        let (table, start, gx, gy) = (self.table, self.start, self.gx as i64, self.gy as i64);
        for y in y0..=y1 {
            let row = y.rem_euclid(gy) * gx;
            for x in x0..=x1 {
                let c = (row + x.rem_euclid(gx)) as usize;
                for e in &table[start[c] as usize..start[c + 1] as usize] {
                    self.check(e, b, l1q);
                }
            }
        }
    }

    /// Tries every allowed lattice point within reach of -(A + B).
    #[inline(always)]
    fn check(&mut self, e: &Entry, b: C64, l1q: u32) {
        self.work += 1;
        let s = C64::new(-(e.re + b.re), -(e.im + b.im)); // want c e1 + q e2 close to s
        let (_, sb) = self.coords(s);
        let db = self.reach * self.n1 / self.det.abs();
        let (lc, lq) = (&self.lists[self.lat], &self.lists[self.lat + 1]);
        // step through the allowed values only: with a minimum |c| most integers
        // in range are gaps, and testing each one dominated the search
        // (usually the range is empty, so check that before searching the list)
        let qlo = ((sb - db).ceil() as i64).max(lq[0] as i64);
        let qhi = ((sb + db).floor() as i64).min(lq[lq.len() - 1] as i64);
        if qlo > qhi {
            return;
        }
        let q0 = lq.partition_point(|&v| (v as i64) < qlo);
        for &q in lq[q0..].iter().take_while(|&&v| v as i64 <= qhi) {
            let r0 = s + self.e2.scale(-(q as f64));
            let xx = r0 * self.inv_e1; // r0 / e1
            let lim = self.reach / self.n1;
            let h2 = lim * lim - xx.im * xx.im;
            if !(h2 >= 0.0) {
                continue;
            }
            let h = h2.sqrt();
            let clo = ((xx.re - h).ceil() as i64).max(lc[0] as i64);
            let chi = ((xx.re + h).floor() as i64).min(lc[lc.len() - 1] as i64);
            if clo > chi {
                continue;
            }
            let c0 = lc.partition_point(|&v| (v as i64) < clo);
            for &c in lc[c0..].iter().take_while(|&&v| v as i64 <= chi) {
                let d2 = (r0 + self.e1.scale(-(c as f64))).norm2();
                if d2 <= self.reach2 {
                    let l1 = e.l1 + l1q + c.unsigned_abs() + q.unsigned_abs();
                    if l1 == 0 {
                        continue;
                    }
                    if let Some(hits) = &mut self.collect.hits {
                        let base = hits.len();
                        hits.resize(base + self.lists.len(), 0);
                        let mut code = e.code as usize;
                        for &p in self.tpow {
                            let len = self.lists[p].len();
                            hits[base + p] = self.lists[p][code % len];
                            code /= len;
                        }
                        for (l, &p) in self.qpow.iter().enumerate() {
                            hits[base + p] = self.lists[p][self.d[l]];
                        }
                        hits[base + self.lat] = c;
                        hits[base + self.lat + 1] = q;
                        if hits.len() > self.collect.cap * self.lists.len() {
                            self.collect.over = true;
                            self.reach2 = -1.0;
                            return;
                        }
                        continue;
                    }
                    let dist = d2.sqrt();
                    // same rule as Walk::test
                    if dist + self.tol < self.win_d || l1 < self.win_l1 || (l1 == self.win_l1 && dist < self.win_d) {
                        self.win_d = dist;
                        self.reach = dist + self.tol;
                        self.reach2 = self.reach * self.reach;
                        self.win_l1 = l1;
                        self.win_code = e.code;
                        self.win_hi.copy_from_slice(&*self.d);
                        self.win_lat = (c, q);
                        self.found = true;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------- per-pixel solver

/// Scratch buffers for one worker thread, reused for every pixel so the
/// hot loop never allocates.
struct Solver<'a> {
    plan: &'a Plan,
    base: Vec<C64>,       // X^i (per power)
    terms: Vec<Vec<C64>>, // terms[i][j] = lists[i][j] * X^i
    raw: Vec<Entry>,      // table before bucketing
    cell: Vec<u32>,       // grid cell of each raw entry
    table: Vec<Entry>,    // table sorted by grid cell
    start: Vec<u32>,
    cursor: Vec<u32>,
    digits: Vec<usize>,
    win_digits: Vec<usize>,
    rad: Vec<f64>,
    cands: Vec<Vec<Entry>>, // candidate list per query level
    /// pixels to skip the lattice search for, after it gave up
    skip: usize,
    /// the last collect found more polynomials than its cap
    over: bool,
}

impl<'a> Solver<'a> {
    fn new(plan: &'a Plan) -> Solver<'a> {
        let t = plan.nterms;
        let m = t - plan.split;
        Solver {
            plan,
            base: vec![C64::ZERO; t],
            terms: plan.lists.iter().map(|l| vec![C64::ZERO; l.len()]).collect(),
            // grown when needed: the largest table (direct search near the real axis)
            // can be hundreds of MB, and most threads never build it
            raw: Vec::new(),
            cell: Vec::new(),
            table: Vec::new(),
            start: Vec::new(),
            cursor: Vec::new(),
            digits: vec![0; t],
            win_digits: vec![0; t],
            rad: vec![0.0; m + 1],
            cands: vec![Vec::new(); m + 1],
            skip: 0,
            over: false,
        }
    }

    /// Every sum of one term from each of `powers` into self.raw, with the
    /// mixed-radix code of the coefficient indices (first power fastest). The last
    /// power takes only the coefficients at indices `last` (a chunk of the table), and
    /// its digit in the code counts from last.start.
    fn build_raw(&mut self, powers: &[usize], last: std::ops::Range<usize>) {
        let lists = &self.plan.lists;
        self.raw.clear();
        let k = powers.len();
        let d = &mut self.digits[..k];
        d.fill(0);
        let first = |j: usize| if j + 1 == k { last.start } else { 0 };
        let end = |j: usize| if j + 1 == k { last.end } else { lists[powers[j]].len() };
        if k > 0 {
            d[k - 1] = last.start;
        }
        let mut code = 0u32;
        loop {
            let mut v = C64::ZERO;
            let mut l1 = 0u32;
            for (i, &p) in powers.iter().enumerate() {
                v = v + self.terms[p][d[i]];
                l1 += lists[p][d[i]].unsigned_abs();
            }
            self.raw.push(Entry { re: v.re, im: v.im, code, l1 });
            code += 1;

            let mut j = 0;
            while j < k {
                d[j] += 1;
                if d[j] < end(j) {
                    break;
                }
                d[j] = first(j);
                j += 1;
            }
            if j == k {
                break;
            }
        }
    }

    /// Sorts self.raw into self.table by self.cell (counting sort, O(n)).
    fn bucket(&mut self, cells: usize) {
        self.start.clear();
        self.start.resize(cells + 1, 0);
        for &c in &self.cell {
            self.start[c as usize + 1] += 1;
        }
        for c in 0..cells {
            self.start[c + 1] += self.start[c];
        }
        if self.table.len() < self.raw.len() {
            self.table.resize(self.raw.len(), Entry::default());
        }
        self.cursor.clear();
        self.cursor.extend_from_slice(&self.start[..cells]);
        for (e, &c) in self.raw.iter().zip(&self.cell) {
            let c = c as usize;
            self.table[self.cursor[c] as usize] = *e;
            self.cursor[c] += 1;
        }
    }

    /// Computes every term c * X^i (the legacy quadratic used conj(X) for the linear
    /// term). Returns the tie tolerance and the largest |term| of each power.
    fn set_x(&mut self, x: C64) -> (f64, Vec<f64>) {
        let plan = self.plan;
        let mut pw = C64::ONE;
        for i in 0..plan.nterms {
            let b = if plan.legacy && i == 1 { x.conj() } else { pw };
            self.base[i] = b;
            for (dst, &c) in self.terms[i].iter_mut().zip(&plan.lists[i]) {
                *dst = b.scale(c as f64);
            }
            pw = pw * x;
        }

        // tie tolerance: residuals closer than the rounding error of evaluating p
        // are treated as equal, and the simplest polynomial (smallest sum |ci|) wins.
        // E.g. at X = -0.7-0.9i both 4+4x+3x^2 and 9+10x+7x^2 give exactly 0.3, and
        // at an exact algebraic X this picks the minimal polynomial, not a multiple.
        let mags: Vec<f64> = (0..plan.nterms).map(|i| plan.max_abs[i] * self.base[i].norm2().sqrt()).collect();
        (64.0 * f64::EPSILON * mags.iter().sum::<f64>(), mags)
    }

    /// Appends every allowed polynomial with |p(X)| <= r to `hits`, as [c0..cn]
    /// each (some possibly twice, as p and -p). Returns false if there are more
    /// than `cap` of them, and then `hits` is incomplete.
    fn collect(&mut self, x: C64, r: f64, cap: usize, hits: &mut Vec<i32>) -> bool {
        let plan = self.plan;
        if x.re == 0.0 && x.im == 0.0 {
            return false; // p(0) = c0 needs no search, and tiles avoid it
        }
        let (tol, mags) = self.set_x(x);
        let mut out = vec![0; plan.nterms];
        let start = hits.len();
        if let Some(ls) = &plan.lattice {
            let ls = if x.norm2() <= 1.0 { &ls[0] } else { &ls[1] };
            let col = Collect { hits: Some(&mut *hits), cap, over: false };
            if self.solve_lattice(ls, tol, r, 0, &mut out, col).is_some() {
                return !self.over;
            }
            hits.truncate(start); // it gave up part way
        }
        self.solve_direct(tol, &mags, r, 0, &mut out, Collect { hits: Some(hits), cap, over: false });
        !self.over
    }

    /// Finds the best polynomial for X. Writes its coefficients [c0..cn] to `out`
    /// and returns the residual |p(X)|. `warm` is a polynomial known to be good
    /// nearby (the previous pixel's). It gives an initial bound so that the
    /// search is tightly pruned from the start.
    fn solve(&mut self, x: C64, warm: Option<&[i32]>, out: &mut [i32]) -> f64 {
        let plan = self.plan;
        let lists = &plan.lists;

        // X = 0: p(0) = c0, so take the smallest allowed coefficient for every power
        // (with c0 = 0 allowed that is the zero polynomial, so use p = c x instead).
        if x.re == 0.0 && x.im == 0.0 {
            for (o, l) in out.iter_mut().zip(lists) {
                *o = *l.iter().min_by_key(|c| (c.unsigned_abs(), c.is_negative())).unwrap();
            }
            if out.iter().all(|&c| c == 0) {
                out[1] = *lists[1].iter().filter(|&&c| c > 0).min().unwrap();
            }
            return out[0].unsigned_abs() as f64;
        }
        let (tol, mags) = self.set_x(x);

        // ---- warm start
        let mut win_d = f64::INFINITY;
        let mut win_l1 = u32::MAX;
        if let Some(w) = warm {
            let v = w
                .iter()
                .zip(&self.base)
                .fold(C64::ZERO, |acc, (&c, &b)| acc + b.scale(c as f64));
            win_d = v.norm2().sqrt();
            win_l1 = w.iter().map(|c| c.unsigned_abs()).sum();
            out.copy_from_slice(w);
        }

        // ---- the lattice search, when it applies (it needs a finite starting bound)
        if let (Some(ls), true) = (&plan.lattice, win_d.is_finite()) {
            if self.skip > 0 {
                self.skip -= 1;
            } else {
                let ls = if x.norm2() <= 1.0 { &ls[0] } else { &ls[1] };
                if let Some(d) = self.solve_lattice(ls, tol, win_d, win_l1, out, Collect::off()) {
                    return d;
                }
                // it gave up (X near the real axis); don't retry for a while
                self.skip = 16;
                if let Some(w) = warm {
                    out.copy_from_slice(w);
                }
            }
        }
        self.solve_direct(tol, &mags, win_d, win_l1, out, Collect::off())
    }

    /// The direct meet-in-the-middle search (see Walk). Expects self.terms and
    /// the warm start in `out`.
    fn solve_direct(&mut self, tol: f64, mags: &[f64], win_d: f64, win_l1: u32, out: &mut [i32], collect: Collect) -> f64 {
        let plan = self.plan;
        let lists = &plan.lists;
        let (t, k) = (plan.nterms, plan.split);
        let m = t - k;

        // radius of the sums of the query terms below each level
        self.rad[0] = 0.0;
        for j in 0..m {
            self.rad[j + 1] = self.rad[j] + mags[k + j] * (1.0 + 1e-12);
        }

        // The table a chunk at a time (usually all at once), each chunk searched by all
        // the queries, with the best so far carried from one chunk to the next.
        let (mut win_d, mut win_l1, mut collect) = (win_d, win_l1, collect);
        let (mut found, mut win_code, mut win_off) = (false, 0u32, 0usize);
        let top = lists[k - 1].len();
        for off in (0..top).step_by(plan.chunk) {
            // ---- table: every low-half sum A, bucketed into a 2-D grid over its bounding box
            self.build_raw(&plan.table_powers, off..(off + plan.chunk).min(top));
            let (mut min_re, mut max_re) = (f64::INFINITY, f64::NEG_INFINITY);
            let (mut min_im, mut max_im) = (f64::INFINITY, f64::NEG_INFINITY);
            for e in &self.raw {
                min_re = min_re.min(e.re);
                max_re = max_re.max(e.re);
                min_im = min_im.min(e.im);
                max_im = max_im.max(e.im);
            }
            let n = self.raw.len();
            let grid = Grid::new(n, min_re, max_re, min_im, max_im);
            self.cell.clear();
            self.cell.extend(self.raw.iter().map(|e| grid.cell(e.re, e.im) as u32));
            self.bucket(grid.gx * grid.gy);

            // ---- search the query half
            let reach = win_d + tol;
            let mut walk = Walk {
                tl: &lists[..k],
                t_off: off,
                hl: &lists[k..],
                ht: &self.terms[k..],
                first_nonneg: &plan.first_nonneg[k..],
                rad: &self.rad,
                table: &self.table[..n],
                start: &self.start,
                grid,
                tol,
                d: &mut self.digits[..m],
                win_d,
                win_l1,
                win_code: 0,
                win_hi: &mut self.win_digits[..m],
                found: false,
                reach,
                reach2: reach * reach,
                collect,
            };
            walk.walk(m, C64::ZERO, 0, 0, None, &mut self.cands);
            if walk.found {
                (found, win_code, win_off) = (true, walk.win_code, off);
            }
            (win_d, win_l1) = (walk.win_d, walk.win_l1);
            collect = walk.collect;
            if collect.over {
                break;
            }
        }
        self.over = collect.over;

        // ---- decode the winner (otherwise the warm-start polynomial stands)
        if found {
            let mut c = win_code as usize;
            for i in 0..k {
                let len = lists[i].len();
                out[i] = lists[i][c % len + if i + 1 == k { win_off } else { 0 }];
                c /= len;
            }
            for j in 0..m {
                out[k + j] = lists[k + j][self.win_digits[j]];
            }
        }
        normalise_sign(out);
        win_d
    }

    /// The lattice search (see LatWalk). Returns None if it gave up.
    fn solve_lattice(&mut self, ls: &LatSplit, tol: f64, win_d: f64, win_l1: u32, out: &mut [i32], collect: Collect) -> Option<f64> {
        let plan = self.plan;
        let lists = &plan.lists;
        let (e1, e2) = (self.base[ls.lat], self.base[ls.lat + 1]);
        let det = e1.re * e2.im - e2.re * e1.im;
        let (n1, n2) = (e1.norm2().sqrt(), e2.norm2().sqrt());
        if !(det.abs() > 1e-9 * n1 * n2) {
            return None; // X (nearly) real: the lattice is flat
        }
        let coords = |p: &Entry| ((p.re * e2.im - e2.re * p.im) / det, (e1.re * p.im - e1.im * p.re) / det);

        // ---- table, bucketed by position within the lattice cell; cells roughly square
        self.build_raw(&ls.table, 0..ls.table.last().map_or(0, |&p| lists[p].len()));
        let n = self.raw.len();
        let gx = ((n as f64 * n1 / n2).sqrt().round() as usize).clamp(1, n);
        let gy = (n / gx).clamp(1, n);
        self.cell.clear();
        self.cell.extend(self.raw.iter().map(|e| {
            let (a, b) = coords(e);
            let cx = (((a - a.floor()) * gx as f64) as usize).min(gx - 1);
            let cy = (((b - b.floor()) * gy as f64) as usize).min(gy - 1);
            (cy * gx + cx) as u32
        }));
        self.bucket(gx * gy);

        // ---- search the queries
        let q = ls.query.len();
        let reach = win_d + tol;
        let inv = 1.0 / e1.norm2();
        let mut walk = LatWalk {
            lists,
            terms: &self.terms,
            first_nonneg: &plan.first_nonneg,
            qpow: &ls.query,
            tpow: &ls.table,
            lat: ls.lat,
            e1,
            e2,
            inv_e1: C64::new(e1.re * inv, -e1.im * inv),
            det,
            n1,
            n2,
            table: &self.table[..n],
            start: &self.start,
            gx,
            gy,
            tol,
            d: &mut self.digits[..q],
            win_d,
            win_l1,
            win_code: 0,
            win_hi: &mut self.win_digits[..q],
            win_lat: (0, 0),
            found: false,
            reach,
            reach2: reach * reach,
            work: 0,
            // give up once this costs as much as the direct search would
            budget: plan.direct_steps() as usize,
            collect,
        };
        walk.walk(q, C64::ZERO, 0, 0);
        self.over = walk.collect.over;
        if walk.work > walk.budget && !self.over {
            return None;
        }
        let (found, win_d, win_code, (c, cq)) = (walk.found, walk.win_d, walk.win_code, walk.win_lat);

        if found {
            let mut code = win_code as usize;
            for &p in &ls.table {
                let len = lists[p].len();
                out[p] = lists[p][code % len];
                code /= len;
            }
            for (l, &p) in ls.query.iter().enumerate() {
                out[p] = lists[p][self.win_digits[l]];
            }
            out[ls.lat] = c;
            out[ls.lat + 1] = cq;
        }
        normalise_sign(out);
        Some(win_d)
    }
}

/// Sign convention: the highest-power nonzero coefficient is positive.
fn normalise_sign(out: &mut [i32]) {
    if let Some(&lead) = out.iter().rev().find(|&&c| c != 0) {
        if lead < 0 {
            out.iter_mut().for_each(|c| *c = -*c);
        }
    }
}

// ---------------------------------------------------------------- rendering a region

/// A rectangle of the complex plane sampled on a pixel grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub re: (f64, f64),
    pub im: (f64, f64),
    /// pixels along the real axis; the height follows from the aspect ratio
    pub width: usize,
}

impl Region {
    /// The square centred on cr + ci*i with half-width hw.
    pub fn square(cr: f64, ci: f64, hw: f64, width: usize) -> Region {
        Region { re: (cr - hw, cr + hw), im: (ci - hw, ci + hw), width }
    }

    pub fn height(&self) -> usize {
        ((self.width as f64) * (self.im.1 - self.im.0) / (self.re.1 - self.re.0)).round().max(1.0) as usize
    }

    /// Real axis (left to right) and imaginary axis (TOP row first, so images
    /// come out the right way up).
    pub fn axes(&self) -> (Vec<f64>, Vec<f64>) {
        let (w, h) = (self.width.max(2), self.height());
        let re = (0..w).map(|c| self.re.0 + (self.re.1 - self.re.0) * c as f64 / (w - 1) as f64).collect();
        let im = if h == 1 {
            vec![(self.im.0 + self.im.1) / 2.0]
        } else {
            (0..h).map(|r| self.im.1 - (self.im.1 - self.im.0) * r as f64 / (h - 1) as f64).collect()
        };
        (re, im)
    }
}

/// The result of rendering a region.
#[derive(Clone)]
pub struct Render {
    pub width: usize,
    pub height: usize,
    /// coefficients per pixel (order + 1)
    pub nterms: usize,
    pub re_axis: Vec<f64>,
    pub im_axis: Vec<f64>,
    /// min |p(X)| per pixel, row-major, top row first
    pub residual: Vec<f64>,
    /// [c0..cn] of the best polynomial per pixel, row-major
    pub coeffs: Vec<i32>,
}

impl Render {
    /// Coefficients [c0..cn] of the best polynomial at (row, col).
    pub fn coeffs_at(&self, row: usize, col: usize) -> &[i32] {
        let i = (row * self.width + col) * self.nterms;
        &self.coeffs[i..i + self.nterms]
    }
}

/// Renders `region` on `threads` threads. `rows_done` counts finished rows for
/// progress reporting. Returns None if `cancel` was set before it finished.
pub fn render(
    plan: &Plan,
    region: &Region,
    threads: usize,
    cancel: &AtomicBool,
    rows_done: &AtomicUsize,
) -> Option<Render> {
    let (re_axis, im_axis) = region.axes();
    render_axes(plan, re_axis, im_axis, threads, cancel, rows_done)
}

/// Like `render`, at the grid of points (re, im) for every re in `re_axis` (left to
/// right) and im in `im_axis` (top row first).
pub fn render_axes(
    plan: &Plan,
    re_axis: Vec<f64>,
    im_axis: Vec<f64>,
    threads: usize,
    cancel: &AtomicBool,
    rows_done: &AtomicUsize,
) -> Option<Render> {
    let method = choose_method(plan, &re_axis, &im_axis);
    render_with(plan, re_axis, im_axis, threads, cancel, rows_done, method)
}

// ---------------------------------------------------------------- tiles
//
// In a deep view neighbouring pixels share their best polynomials, so one search
// can serve a whole tile. Let X0 be the tile's centre, h the distance to its
// furthest pixel and L the slope bound (Plan::slope), so that any polynomial
// changes by at most L h across the tile. If the best value at X0 is r0, every
// pixel X has a polynomial (X0's winner) with |p(X)| <= r0 + L h, so X's winner w
// has |w(X0)| <= |w(X)| + L h <= r0 + 2 L h. One collect search at X0 with that
// radius therefore finds every polynomial that can win anywhere in the tile, and
// each pixel only has to evaluate those: the same answers as searching each pixel.

/// Views whose tiles would be narrower than this are searched pixel by pixel.
const MIN_TILE: usize = 8;
/// Larger tiles hardly save more, and fewer of them spread less well over threads.
const MAX_TILE: usize = 64;
/// Tiles split down to this many pixels or fewer search each pixel instead.
const SPLIT_MIN: usize = 16;
/// Tiles are sized for about this many polynomials each.
const TILE_POLYS: f64 = 256.0;
/// A tile with more than this many is split in four.
const TILE_CAP: usize = 4096;

/// How render_with covers the pixels: row by row, searching each pixel, or in
/// square tiles of the given side with one search each.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Method {
    Rows,
    Tiles(usize),
}

/// A rectangle of pixels: columns c0..c1, rows r0..r1.
#[derive(Clone, Copy, Debug)]
struct Rect {
    c0: usize,
    c1: usize,
    r0: usize,
    r1: usize,
}

impl Rect {
    fn width(&self) -> usize {
        self.c1 - self.c0
    }
    fn pixels(&self) -> usize {
        self.width() * (self.r1 - self.r0)
    }
}

/// Tiles when the view is deep enough for tiles of at least MIN_TILE pixels to
/// hold about TILE_POLYS polynomials.
fn choose_method(plan: &Plan, re: &[f64], im: &[f64]) -> Method {
    let (w, h, t) = (re.len(), im.len(), plan.nterms);
    if plan.legacy || w < 2 * MIN_TILE || h < 2 * MIN_TILE {
        return Method::Rows;
    }
    let px = ((re[w - 1] - re[0]).abs() / (w - 1) as f64).max((im[0] - im[h - 1]).abs() / (h - 1) as f64);

    // the typical best value, from a few points across the view
    let mut solver = Solver::new(plan);
    let (mut warm, mut out) = (plan.any_poly(), vec![0; t]);
    let (mut sum, mut n) = (0.0, 0);
    for i in 0..3 {
        for j in 0..3 {
            let x = C64::new(re[(2 * j + 1) * w / 6], im[(2 * i + 1) * h / 6]);
            sum += solver.solve(x, Some(&warm), &mut out);
            warm.copy_from_slice(&out);
            n += 1;
        }
    }
    let typical = sum / n as f64;
    if !(typical > 0.0) {
        return Method::Rows;
    }
    // The values p(X) near 0 are scattered like random points. With density rho the
    // nearest is about 1 / (2 sqrt(rho)) away and about rho pi R^2 lie within R.
    let rho = 1.0 / (4.0 * typical * typical);
    let reach = (TILE_POLYS / (std::f64::consts::PI * rho)).sqrt();
    let a = [re[0], re[w - 1]]
        .iter()
        .flat_map(|&x| [im[0], im[h - 1]].map(|y| x.hypot(y)))
        .fold(0.0, f64::max);
    let half_diag = (reach - typical) / (2.0 * plan.slope(a));
    let side = (half_diag * std::f64::consts::SQRT_2 / px) as usize;
    if side < MIN_TILE {
        Method::Rows
    } else {
        Method::Tiles(side.min(MAX_TILE))
    }
}

/// Rough search work per pixel to render the grid (re, im), in the units of
/// Plan::steps: less than steps() where tiles share the search. Runs a few searches.
pub fn work_per_pixel(plan: &Plan, re: &[f64], im: &[f64]) -> f64 {
    // a tile costs about two searches (one of them a collect), and each pixel
    // evaluating the shortlist about this much (measured, order 3)
    const TILE_SEARCHES: f64 = 2.0;
    const SHORTLIST_WORK: f64 = 10.0;
    match choose_method(plan, re, im) {
        Method::Rows => plan.steps(),
        Method::Tiles(s) => (plan.steps() * TILE_SEARCHES / (s * s) as f64 + SHORTLIST_WORK).min(plan.steps()),
    }
}

/// One thread's renderer.
struct Tiler<'a> {
    solver: Solver<'a>,
    /// the starting polynomial for each row and tile: always the same one, since
    /// it can decide near-ties and every render of a view must come out the same
    warm: Vec<i32>,
    out: Vec<i32>,
    hits: Vec<i32>,
}

impl<'a> Tiler<'a> {
    fn new(plan: &'a Plan) -> Tiler<'a> {
        Tiler { solver: Solver::new(plan), warm: plan.any_poly(), out: vec![0; plan.nterms], hits: Vec::new() }
    }

    /// Searches each pixel of `rect`, writing to `res` and `co`, which hold the
    /// pixels of `outer` (which contains rect).
    fn pixels(&mut self, re: &[f64], im: &[f64], rect: Rect, outer: Rect, res: &mut [f64], co: &mut [i32]) {
        let t = self.solver.plan.nterms;
        for r in rect.r0..rect.r1 {
            let row = (r - outer.r0) * outer.width() + rect.c0 - outer.c0;
            for c in rect.c0..rect.c1 {
                // each pixel starts from its left neighbour's polynomial
                let i = row + c - rect.c0;
                let (done, rest) = co.split_at_mut(i * t);
                let warm = if c > rect.c0 { &done[(i - 1) * t..] } else { &self.warm[..] };
                res[i] = self.solver.solve(C64::new(re[c], im[r]), Some(warm), &mut rest[..t]);
            }
        }
    }

    /// Like `pixels`, with one search for the whole of `rect` (see the notes above).
    fn tile(&mut self, re: &[f64], im: &[f64], rect: Rect, outer: Rect, res: &mut [f64], co: &mut [i32]) {
        let plan = self.solver.plan;
        let t = plan.nterms;
        if rect.pixels() <= SPLIT_MIN {
            return self.pixels(re, im, rect, outer, res, co);
        }
        let x0 = C64::new((re[rect.c0] + re[rect.c1 - 1]) / 2.0, (im[rect.r0] + im[rect.r1 - 1]) / 2.0);
        let corners = [(rect.c0, rect.r0), (rect.c1 - 1, rect.r0), (rect.c0, rect.r1 - 1), (rect.c1 - 1, rect.r1 - 1)];
        let h = corners.iter().map(|&(c, r)| (re[c] - x0.re).hypot(im[r] - x0.im)).fold(0.0, f64::max);
        let r0 = self.solver.solve(x0, Some(&self.warm), &mut self.out);
        let a = x0.norm2().sqrt() + h;
        let reach = (r0 + 2.0 * plan.slope(a) * h) * (1.0 + 1e-9) + 2.0 * plan.tol_upto(a);
        self.hits.clear();
        if !self.solver.collect(x0, reach, TILE_CAP, &mut self.hits) || self.hits.is_empty() {
            // too many polynomials (or X0 = 0): quarter the tile
            let (cm, rm) = ((rect.c0 + rect.c1) / 2, (rect.r0 + rect.r1) / 2);
            for (c0, c1, r0, r1) in [(rect.c0, cm, rect.r0, rm), (cm, rect.c1, rect.r0, rm), (rect.c0, cm, rm, rect.r1), (cm, rect.c1, rm, rect.r1)] {
                if c0 < c1 && r0 < r1 {
                    self.tile(re, im, Rect { c0, c1, r0, r1 }, outer, res, co);
                }
            }
            return;
        }

        // each pixel: the best of the shortlist, chosen as in Walk::test
        let cf: Vec<f64> = self.hits.iter().map(|&c| c as f64).collect();
        let l1s: Vec<u32> = self.hits.chunks(t).map(|p| p.iter().map(|c| c.unsigned_abs()).sum()).collect();
        let mut pw = vec![C64::ZERO; t];
        for r in rect.r0..rect.r1 {
            for c in rect.c0..rect.c1 {
                let x = C64::new(re[c], im[r]);
                if x.re == 0.0 && x.im == 0.0 {
                    let one = Rect { c0: c, c1: c + 1, r0: r, r1: r + 1 };
                    self.pixels(re, im, one, outer, res, co);
                    continue;
                }
                pw[0] = C64::ONE;
                for i in 1..t {
                    pw[i] = pw[i - 1] * x;
                }
                let tol = plan.tol_upto(x.norm2().sqrt());
                let (mut win_d, mut win_l1, mut win) = (f64::INFINITY, u32::MAX, 0);
                for (k, p) in cf.chunks(t).enumerate() {
                    let v = p.iter().zip(&pw).fold(C64::ZERO, |acc, (&c, &b)| acc + b.scale(c));
                    let (d, l1) = (v.norm2().sqrt(), l1s[k]);
                    if d + tol < win_d || (d <= win_d + tol && (l1 < win_l1 || (l1 == win_l1 && d < win_d))) {
                        (win_d, win_l1, win) = (d, l1, k);
                    }
                }
                let i = (r - outer.r0) * outer.width() + c - outer.c0;
                res[i] = win_d;
                co[i * t..(i + 1) * t].copy_from_slice(&self.hits[win * t..(win + 1) * t]);
                normalise_sign(&mut co[i * t..(i + 1) * t]);
            }
        }
    }
}

/// render_axes with the given method.
fn render_with(
    plan: &Plan,
    re_axis: Vec<f64>,
    im_axis: Vec<f64>,
    threads: usize,
    cancel: &AtomicBool,
    rows_done: &AtomicUsize,
    method: Method,
) -> Option<Render> {
    let (w, h, t) = (re_axis.len(), im_axis.len(), plan.nterms);
    let mut residual = vec![0.0f64; w * h];
    let mut coeffs = vec![0i32; w * h * t];
    // Real coefficients: p(conj X) = conj(p(X)), so a view symmetric about the real
    // axis only needs its top half computed (the legacy conj(X) term breaks this).
    let mirror = method == Method::Rows
        && !plan.legacy
        && h > 1
        && (im_axis[0] + im_axis[h - 1]).abs() <= 1e-12 * (im_axis[0] - im_axis[h - 1]).abs();
    let todo = if mirror { (h + 1) / 2 } else { h };
    // the pieces of work: whole rows, or tiles
    let rects: Vec<Rect> = match method {
        Method::Rows => (0..todo).map(|r| Rect { c0: 0, c1: w, r0: r, r1: r + 1 }).collect(),
        Method::Tiles(s) => (0..h)
            .step_by(s)
            .flat_map(|r0| (0..w).step_by(s).map(move |c0| Rect { c0, c1: (c0 + s).min(w), r0, r1: (r0 + s).min(h) }))
            .collect(),
    };
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel::<(Rect, Vec<f64>, Vec<i32>)>();

    thread::scope(|s| {
        for _ in 0..threads.max(1) {
            let tx = tx.clone();
            let (next, rects, re_axis, im_axis) = (&next, &rects, &re_axis, &im_axis);
            s.spawn(move || {
                let mut tiler = Tiler::new(plan);
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    if k >= rects.len() || cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let rect = rects[k];
                    let mut res = vec![0.0; rect.pixels()];
                    let mut co = vec![0i32; rect.pixels() * t];
                    match method {
                        Method::Rows => tiler.pixels(re_axis, im_axis, rect, rect, &mut res, &mut co),
                        Method::Tiles(_) => tiler.tile(re_axis, im_axis, rect, rect, &mut res, &mut co),
                    }
                    if tx.send((rect, res, co)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);
        let mut pixels_done = 0;
        for (rect, res, co) in rx {
            // progress in whole rows' worth of pixels
            let rows_before = pixels_done / w;
            let rw = rect.width();
            for r in rect.r0..rect.r1 {
                let src = (r - rect.r0) * rw;
                let rows: &[usize] = if mirror && h - 1 - r != r { &[r, h - 1 - r] } else { &[r] };
                for &r in rows {
                    let dst = r * w + rect.c0;
                    residual[dst..dst + rw].copy_from_slice(&res[src..src + rw]);
                    coeffs[dst * t..(dst + rw) * t].copy_from_slice(&co[src * t..(src + rw) * t]);
                }
                pixels_done += rows.len() * rw;
            }
            rows_done.fetch_add(pixels_done / w - rows_before, Ordering::Relaxed);
        }
    });

    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    Some(Render { width: w, height: h, nterms: t, re_axis, im_axis, residual, coeffs })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn co(set: CoefSet, max: i32, zero: bool, zero_root: bool) -> Coefs {
        Coefs { set, max, zero, zero_root }
    }

    #[test]
    fn coefficient_sets() {
        assert_eq!(co(CoefSet::Fibonacci, 10, true, false).values(), [-8, -5, -3, -2, -1, 0, 1, 2, 3, 5, 8]);
        assert_eq!(co(CoefSet::Primes, 12, false, false).values(), [-11, -7, -5, -3, -2, -1, 1, 2, 3, 5, 7, 11]);
        assert_eq!(co(CoefSet::Ints, 2, true, false).values(), [-2, -1, 0, 1, 2]);
        let up = |s: CoefSet, m| s.upto(m);
        assert_eq!(up(CoefSet::Odd, 10), [1, 3, 5, 7, 9]);
        assert_eq!(up(CoefSet::Triangular, 21), [1, 3, 6, 10, 15, 21]);
        assert_eq!(up(CoefSet::Squares, 50), [1, 4, 9, 16, 25, 36, 49]);
        assert_eq!(up(CoefSet::Cubes, 125), [1, 8, 27, 64, 125]);
        assert_eq!(up(CoefSet::Powers2, 100), [1, 2, 4, 8, 16, 32, 64]);
        assert_eq!(up(CoefSet::Powers2, i32::MAX).len(), 31);
        for s in CoefSet::ALL {
            assert_eq!(up(s, 1), [1], "{s:?}");
        }
        let plan = Plan::new(2, &co(CoefSet::Squares, 4, false, true), false).unwrap();
        assert_eq!(plan.lists[0], [-4, -1, 0, 1, 4]);
        assert_eq!(plan.lists[1], [-4, -1, 1, 4]);
    }

    /// min |p(X)| by trying every polynomial (except p = 0).
    fn brute(plan: &Plan, x: C64) -> f64 {
        let t = plan.nterms;
        let mut d = vec![0usize; t];
        let mut best = f64::INFINITY;
        loop {
            let (mut v, mut pw, mut nz) = (C64::ZERO, C64::ONE, false);
            for i in 0..t {
                let c = plan.lists[i][d[i]];
                nz |= c != 0;
                v = v + pw.scale(c as f64);
                pw = pw * x;
            }
            if nz {
                best = best.min(v.norm2().sqrt());
            }
            let mut j = 0;
            while j < t {
                d[j] += 1;
                if d[j] < plan.lists[j].len() {
                    break;
                }
                d[j] = 0;
                j += 1;
            }
            if j == t {
                return best;
            }
        }
    }

    /// The fast searches (direct and lattice) find the same minimum as trying every polynomial.
    #[test]
    fn matches_brute_force() {
        let cases = [
            (3, co(CoefSet::Fibonacci, 13, false, false)),
            (3, co(CoefSet::Primes, 11, false, false)),
            (3, co(CoefSet::Ints, 7, true, true)),
            (3, co(CoefSet::Odd, 11, true, false)),
            (3, co(CoefSet::Triangular, 21, false, true)),
            (3, co(CoefSet::Squares, 36, true, true)),
            (3, co(CoefSet::Cubes, 125, true, false)),
            (4, co(CoefSet::Powers2, 32, true, false)),
            (4, co(CoefSet::Ints, 5, true, false)),
            (5, co(CoefSet::Ints, 3, true, true)),
            (6, co(CoefSet::Ints, 2, true, false)),
            (2, co(CoefSet::Ints, 20, true, false)),
        ];
        let mut lattice_used = 0;
        for (order, coefs) in cases {
            let plan = Plan::new(order, &coefs, false).unwrap();
            let mut solver = Solver::new(&plan);
            let mut out = vec![0; order + 1];
            let mut warm: Option<Vec<i32>> = None;
            // a path through |X| < 1 and |X| > 1, crossing the real axis and passing 0
            for k in 0..80 {
                let x = C64::new(-1.7 + 0.0425 * k as f64, 0.9 - 0.025 * k as f64);
                let x = if k == 40 { C64::ZERO } else { x };
                let skip = solver.skip;
                let r = solver.solve(x, warm.as_deref(), &mut out);
                lattice_used += (plan.lattice.is_some() && warm.is_some() && skip == 0 && solver.skip == 0) as usize;
                let best = brute(&plan, x);
                assert!((r - best).abs() <= 1e-12 * (1.0 + best), "order {order} {coefs:?} at {x:?}: {r} vs {best}");
                let v = out.iter().rev().fold(C64::ZERO, |acc, &c| acc * x + C64::new(c as f64, 0.0));
                assert!((v.norm2().sqrt() - r).abs() <= 1e-12 * (1.0 + r), "coefficients don't give the residual");
                assert!(out.iter().any(|&c| c != 0));
                for (i, &c) in out.iter().enumerate() {
                    assert!(plan.lists[i].contains(&c), "{coefs:?}: coefficient {c} not allowed");
                }
                warm = Some(out.clone());
            }
        }
        assert!(lattice_used > 200, "lattice search used for only {lattice_used} pixels");
    }

    /// Every allowed nonzero polynomial with |p(X)| <= r, sign-normalised and sorted.
    fn brute_within(plan: &Plan, x: C64, r: f64) -> Vec<Vec<i32>> {
        let t = plan.nterms;
        let mut d = vec![0usize; t];
        let mut found = Vec::new();
        loop {
            let p: Vec<i32> = (0..t).map(|i| plan.lists[i][d[i]]).collect();
            let v = p.iter().rev().fold(C64::ZERO, |acc, &c| acc * x + C64::new(c as f64, 0.0));
            if p.iter().any(|&c| c != 0) && v.norm2().sqrt() <= r {
                found.push(p);
            }
            let mut j = 0;
            while j < t {
                d[j] += 1;
                if d[j] < plan.lists[j].len() {
                    break;
                }
                d[j] = 0;
                j += 1;
            }
            if j == t {
                break;
            }
        }
        tidy(found)
    }

    fn tidy(mut ps: Vec<Vec<i32>>) -> Vec<Vec<i32>> {
        ps.iter_mut().for_each(|p| normalise_sign(p));
        ps.sort();
        ps.dedup();
        ps
    }

    /// Collect finds exactly the polynomials within the radius, by the lattice
    /// search and by the direct one.
    #[test]
    fn collect_matches_brute() {
        let cases = [
            (3, co(CoefSet::Ints, 7, true, true)),
            (3, co(CoefSet::Primes, 11, false, false)),
            (3, co(CoefSet::Squares, 36, true, true)),
            (4, co(CoefSet::Ints, 5, true, false)),
            (5, co(CoefSet::Ints, 3, true, true)),
            (2, co(CoefSet::Ints, 20, true, false)),
        ];
        let points = [(-0.3, 0.8), (0.4, 1e-4), (1.3, 0.5), (-0.62, 0.71), (0.05, -0.9), (-1.1, 0.2)];
        for (order, coefs) in cases {
            let plan = Plan::new(order, &coefs, false).unwrap();
            let t = plan.nterms;
            let mut solver = Solver::new(&plan);
            for &(re, im) in &points {
                let x = C64::new(re, im);
                // a radius holding a few polynomials
                let r = 4.0 * brute(&plan, x) + 1e-3;
                let want = brute_within(&plan, x, r);
                let mut hits = Vec::new();
                assert!(solver.collect(x, r, 1 << 20, &mut hits));
                let got = tidy(hits.chunks(t).map(|p| p.to_vec()).collect());
                assert_eq!(got, want, "order {order} {coefs:?} at {x:?}, r {r}");

                // the direct search on its own
                let (tol, mags) = solver.set_x(x);
                let mut hits = Vec::new();
                let mut out = vec![0; t];
                solver.solve_direct(tol, &mags, r, 0, &mut out, Collect { hits: Some(&mut hits), cap: 1 << 20, over: false });
                let got = tidy(hits.chunks(t).map(|p| p.to_vec()).collect());
                assert_eq!(got, want, "direct: order {order} {coefs:?} at {x:?}");

                // the cap
                if want.len() > 2 {
                    let mut hits = Vec::new();
                    assert!(!solver.collect(x, r, 1, &mut hits));
                }
            }
        }
    }

    /// With the table limited (as for very large max coefficients), the direct search
    /// takes a smaller table or builds it in chunks, and still finds the same polynomials.
    #[test]
    fn small_tables_same_result() {
        let cases = [(3, co(CoefSet::Ints, 7, true, true)), (4, co(CoefSet::Ints, 4, true, false)), (3, co(CoefSet::Primes, 11, false, false))];
        let mut chunked = 0;
        for (order, coefs) in cases {
            let full = Plan::new(order, &coefs, false).unwrap();
            assert_eq!(full.chunks(), 1);
            for limit in [20.0, 60.0] {
                let small = Plan::with_table_limit(order, &coefs, false, limit).unwrap();
                assert!(small.table_len() as f64 <= limit && small.table_len() < full.table_len(), "order {order}, limit {limit}");
                chunked += (small.chunks() > 1) as usize;
                let t = small.nterms;
                let (mut a, mut b) = (Solver::new(&full), Solver::new(&small));
                let (mut oa, mut ob) = (vec![0; t], vec![0; t]);
                // through |X| < 1 and |X| > 1 and right along the real axis; with no warm
                // start, solve uses the direct search
                for k in 0..60 {
                    let x = C64::new(-1.6 + 0.055 * k as f64, if k % 3 == 0 { 0.0 } else { 0.8 - 0.027 * k as f64 });
                    let (ra, rb) = (a.solve(x, None, &mut oa), b.solve(x, None, &mut ob));
                    let best = brute(&full, x);
                    assert!((rb - best).abs() <= 1e-12 * (1.0 + best), "order {order}, limit {limit} at {x:?}: {rb} vs {best}");
                    assert!((ra - rb).abs() <= 1e-12 * (1.0 + best));
                    let v = ob.iter().rev().fold(C64::ZERO, |acc, &c| acc * x + C64::new(c as f64, 0.0));
                    assert!((v.norm2().sqrt() - rb).abs() <= 1e-12 * (1.0 + rb), "the coefficients don't give the residual");
                    // collecting with the direct search
                    let r = 3.0 * best + 1e-3;
                    let (tol, mags) = b.set_x(x);
                    let mut hits = Vec::new();
                    b.solve_direct(tol, &mags, r, 0, &mut ob, Collect { hits: Some(&mut hits), cap: 1 << 20, over: false });
                    assert_eq!(tidy(hits.chunks(t).map(|p| p.to_vec()).collect()), brute_within(&full, x, r), "collect, order {order}, limit {limit} at {x:?}");
                }
            }
        }
        assert!(chunked >= 2, "only {chunked} chunked plans tried");
        // a max coefficient far above the old 32 767 limit is accepted, with the table in chunks
        let big = Plan::new(3, &Coefs::ints(1_000_000), false).unwrap();
        assert!(big.chunks() > 1 && (big.table_len() as f64) <= TABLE_MAX && big.lattice.is_some());
        let usual = Plan::new(3, &Coefs::ints(4095), false).unwrap();
        assert!(usual.split == 2 && usual.chunks() == 1, "the usual table up to 4095");
        let next = Plan::new(3, &Coefs::ints(10_000), false).unwrap();
        assert_eq!((next.split, next.chunks()), (2, 6));
    }

    /// Tiles give the same picture as searching every pixel.
    #[test]
    fn tiled_matches_per_pixel() {
        // (order, coefficients, centre, view width)
        let views = [
            (3, Coefs::ints(300), (-0.62, 0.71), 1e-7),
            (3, Coefs::ints(60), (-0.3, 0.8), 1e-3),
            (3, Coefs::ints(100), (0.5, 0.0), 1e-4),
            (3, Coefs::ints(100), (1.2, 0.4), 1e-5),
            (2, Coefs::ints(500), (0.31, 0.55), 1e-6),
            (4, Coefs::ints(20), (-0.45, 0.62), 1e-4),
            (3, co(CoefSet::Primes, 300, true, true), (0.1, 0.95), 1e-7),
        ];
        let (w, h) = (48, 40);
        for (order, coefs, (cr, ci), width) in views {
            let plan = Plan::new(order, &coefs, false).unwrap();
            let t = plan.nterms;
            let dx = width / (w - 1) as f64;
            let re: Vec<f64> = (0..w).map(|c| cr + dx * (c as f64 - w as f64 / 2.0)).collect();
            let im: Vec<f64> = (0..h).map(|r| ci - dx * (r as f64 - h as f64 / 2.0)).collect();
            let go = |m| render_with(&plan, re.clone(), im.clone(), 4, &AtomicBool::new(false), &AtomicUsize::new(0), m).unwrap();
            let (rows, tiles) = (go(Method::Rows), go(Method::Tiles(16)));
            let mut differ = 0;
            for r in 0..h {
                for c in 0..w {
                    let tol = plan.tol_upto(re[c].hypot(im[r]));
                    let (a, b) = (rows.residual[r * w + c], tiles.residual[r * w + c]);
                    assert!((a - b).abs() <= 4.0 * tol, "order {order} {coefs:?} at ({},{}): {a} vs {b}", re[c], im[r]);
                    differ += (rows.coeffs_at(r, c) != tiles.coeffs_at(r, c)) as usize;
                }
            }
            // only genuine ties may differ
            assert!(differ * 100 <= w * h, "order {order} {coefs:?}: {differ} pixels with other polynomials");
            assert_eq!(tiles.coeffs.len(), w * h * t);
        }
    }

    /// Times 720p frames with max coefficients above the usual table limit (TABLE_MAX),
    /// at views as deep as auto would choose for each (about 4 stars per 100 px²: the
    /// width shrinks like 1/M²), off the real axis and close to it. Each frame stops
    /// after 30 s and its time is estimated from how far it got.
    /// cargo test --release --lib bench_huge -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bench_huge() {
        let threads = thread::available_parallelism().map_or(4, |n| n.get());
        for max in [3000, 10_000, 35_000, 100_000] {
            let plan = Plan::new(3, &Coefs::ints(max), false).unwrap();
            let width = 1e-7 * (35_000.0 / max as f64).powi(2);
            for (name, (cr, ci)) in [("off the axis", (-0.62, 0.71)), ("near the axis", (0.53, 0.05))] {
                let dx = width / 1279.0;
                let re: Vec<f64> = (0..1280).map(|c| cr + dx * (c as f64 - 640.0)).collect();
                let im: Vec<f64> = (0..720).map(|r| ci - dx * (r as f64 - 360.0)).collect();
                let m = choose_method(&plan, &re, &im);
                let (rows, cancel, done) = (AtomicUsize::new(0), AtomicBool::new(false), AtomicBool::new(false));
                let t = std::time::Instant::now();
                thread::scope(|sc| {
                    sc.spawn(|| {
                        while !done.load(Ordering::Relaxed) && t.elapsed().as_secs_f64() < 30.0 {
                            thread::sleep(std::time::Duration::from_millis(100));
                        }
                        cancel.store(true, Ordering::Relaxed);
                    });
                    render_with(&plan, re.clone(), im.clone(), threads, &cancel, &rows, m);
                    done.store(true, Ordering::Relaxed);
                });
                let (secs, got) = (t.elapsed().as_secs_f64(), rows.load(Ordering::Relaxed));
                let est = if got >= 720 { format!("{secs:.2} s") } else { format!("~{:.0} s (estimated from {got} of 720 rows)", secs * 720.0 / got.max(1) as f64) };
                println!("max {max:6} (split {}), width {width:.1e}, {name}, {m:?}: 720p frame {est}", plan.split);
            }
        }
    }

    /// Times deep views with a large max coefficient: a strip of a 1280-wide view of
    /// width 1e-7, scaled up to a 1280x720 frame.
    /// cargo test --release --lib bench_deep -- --ignored --nocapture
    /// (ALGFRAC_DEEP_MAX sets the max coefficient, default 3000)
    #[test]
    #[ignore]
    fn bench_deep() {
        let max = std::env::var("ALGFRAC_DEEP_MAX").ok().and_then(|s| s.parse().ok()).unwrap_or(3000);
        let threads = thread::available_parallelism().map_or(4, |n| n.get());
        let plan = Plan::new(3, &Coefs::ints(max), false).unwrap();
        let width = 1e-7;
        let dx = width / 1279.0;
        // (name, centre, strip size)
        let views = [
            ("off the real axis", (-0.62, 0.71), (384, 96)),
            ("off the real axis, whole frame", (-0.62, 0.71), (1280, 720)),
            ("near the real axis", (0.53, 3e-6), (96, 48)),
        ];
        for (name, (cr, ci), (w, h)) in views {
            let re: Vec<f64> = (0..w).map(|c| cr + dx * (c as f64 - w as f64 / 2.0)).collect();
            let im: Vec<f64> = (0..h).map(|r| ci - dx * (r as f64 - h as f64 / 2.0)).collect();
            let mut renders = Vec::new();
            let methods: &[Option<Method>] = match w * h > 100_000 {
                true => &[Some(Method::Rows), None],
                false => &[Some(Method::Rows), None, Some(Method::Tiles(16)), Some(Method::Tiles(48))],
            };
            for &method in methods {
                let t = std::time::Instant::now();
                let method = method.unwrap_or_else(|| choose_method(&plan, &re, &im));
                let r = render_with(&plan, re.clone(), im.clone(), threads, &AtomicBool::new(false), &AtomicUsize::new(0), method).unwrap();
                let secs = t.elapsed().as_secs_f64();
                let per = secs / (w * h) as f64;
                let mean = r.residual.iter().sum::<f64>() / r.residual.len() as f64;
                println!(
                    "order 3, max {max}, width {width:e}, {name}, {method:?}: {w}x{h} px in {secs:.2} s = {:.1} us/px -> 720p frame ~{:.1} s (mean residual {mean:.2e})",
                    per * 1e6,
                    per * 1280.0 * 720.0
                );
                renders.push(r);
            }
            for r in &renders[1..] {
                let same = (0..w * h).filter(|&i| renders[0].coeffs[i * 4..i * 4 + 4] == r.coeffs[i * 4..i * 4 + 4]).count();
                println!("  same polynomial at {same} of {} pixels", w * h);
            }
        }

        // the automatic choice against searching every pixel, over depths and max coefficients
        let (w, h) = (384, 96);
        for max in [155, 1000, 3000] {
            let plan = Plan::new(3, &Coefs::ints(max), false).unwrap();
            for width in [1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-7, 1e-9] {
                let dx = width / 1279.0;
                let (cr, ci) = (-0.62, 0.71);
                let re: Vec<f64> = (0..w).map(|c| cr + dx * (c as f64 - w as f64 / 2.0)).collect();
                let im: Vec<f64> = (0..h).map(|r| ci - dx * (r as f64 - h as f64 / 2.0)).collect();
                let time = |m: Option<Method>| {
                    let t = std::time::Instant::now();
                    let m = m.unwrap_or_else(|| choose_method(&plan, &re, &im));
                    render_with(&plan, re.clone(), im.clone(), threads, &AtomicBool::new(false), &AtomicUsize::new(0), m).unwrap();
                    (m, t.elapsed().as_secs_f64())
                };
                let ((_, rows), (m, auto)) = (time(Some(Method::Rows)), time(None));
                let forced: Vec<String> = [16, 32, 64].iter().map(|&s| format!("{s}: {:.3}", time(Some(Method::Tiles(s))).1)).collect();
                println!("max {max:5}, width {width:e}: rows {rows:.3} s, {m:?} {auto:.3} s ({:.1}x); tiles {}", rows / auto, forced.join(", "));
            }
        }
    }

    /// |p(X)| in double-double arithmetic (about 32 digits).
    fn residual_dd(co: &[i32], x: (f64, f64)) -> f64 {
        type Dd = (f64, f64);
        let two_sum = |a: f64, b: f64| -> Dd {
            let s = a + b;
            let bb = s - a;
            (s, (a - (s - bb)) + (b - bb))
        };
        let add = |a: Dd, b: Dd| -> Dd {
            let (s, e) = two_sum(a.0, b.0);
            let e = e + a.1 + b.1;
            two_sum(s, e)
        };
        let mul = |a: Dd, b: Dd| -> Dd {
            let p = a.0 * b.0;
            let e = a.0.mul_add(b.0, -p) + (a.0 * b.1 + a.1 * b.0);
            two_sum(p, e)
        };
        let neg = |a: Dd| (-a.0, -a.1);
        let (xr, xi) = ((x.0, 0.0), (x.1, 0.0));
        let (mut pr, mut pi) = ((1.0, 0.0), (0.0, 0.0));
        let (mut sr, mut si) = ((0.0, 0.0), (0.0, 0.0));
        for &c in co {
            let c = (c as f64, 0.0);
            sr = add(sr, mul(c, pr));
            si = add(si, mul(c, pi));
            (pr, pi) = (add(mul(pr, xr), neg(mul(pi, xi))), add(mul(pr, xi), mul(pi, xr)));
        }
        (sr.0 + sr.1).hypot(si.0 + si.1)
    }

    /// A deep view with large coefficients next to a root of a small polynomial, where a
    /// zoom to 1e-8 showed lines of roots broken into pieces: renders it pixel by pixel
    /// and in tiles, and compares both with the residuals of their polynomials in
    /// double-double. Writes -ln(residual) maps to $ALGFRAC_DIAG_DIR.
    /// cargo test --release --lib diag_deep_lines -- --ignored --nocapture
    #[test]
    #[ignore]
    fn diag_deep_lines() {
        let dir = std::path::PathBuf::from(std::env::var("ALGFRAC_DIAG_DIR").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        let max = std::env::var("ALGFRAC_DEEP_MAX").ok().and_then(|s| s.parse().ok()).unwrap_or(37912);
        let plan = Plan::new(3, &Coefs { zero_root: true, ..Coefs::ints(max) }, false).unwrap();
        let (cr, ci, hw) = (0.4838713675049316, 0.8658613229157475, 0.000000023661758913329213);
        let px = 2.0 * hw / 1079.0;
        // frame pixels 780..1100 across, 120..360 down, of 1920 x 1080
        let (w, h) = (320, 240);
        let re: Vec<f64> = (0..w).map(|c| cr + px * ((780 + c) as f64 - 959.5)).collect();
        let im: Vec<f64> = (0..h).map(|r| ci - px * ((120 + r) as f64 - 539.5)).collect();
        let threads = thread::available_parallelism().map_or(4, |n| n.get());
        let auto = choose_method(&plan, &re, &im);
        println!("max {max}, pixel {px:e}, automatic method {auto:?}");
        let mut renders = Vec::new();
        for m in [Method::Rows, auto] {
            let r = render_with(&plan, re.clone(), im.clone(), threads, &AtomicBool::new(false), &AtomicUsize::new(0), m).unwrap();
            let mut errs: Vec<f64> = (0..w * h)
                .map(|i| {
                    let exact = residual_dd(&r.coeffs[i * 4..i * 4 + 4], (re[i % w], im[i / w]));
                    (r.residual[i] - exact).abs()
                })
                .collect();
            errs.sort_by(f64::total_cmp);
            let mut res = r.residual.clone();
            res.sort_by(f64::total_cmp);
            println!(
                "{m:?}: residual median {:.2e}, 1% {:.2e}; |f64 - exact| median {:.2e}, 99% {:.2e}, max {:.2e}; tol {:.2e}",
                res[res.len() / 2],
                res[res.len() / 100],
                errs[errs.len() / 2],
                errs[errs.len() * 99 / 100],
                errs[errs.len() - 1],
                plan.tol_upto(1.0)
            );
            let (lo, hi) = (-res[res.len() * 99 / 100].ln(), -res[0].max(1e-300).ln());
            let grey: Vec<u8> = r.residual.iter().flat_map(|&v| { let g = ((-v.ln() - lo) / (hi - lo) * 255.0).clamp(0.0, 255.0) as u8; [g, g, g] }).collect();
            crate::png::write_png(dir.join(format!("deep_{m:?}.png").replace(['(', ')'], "")), w, h, &grey).unwrap();
            renders.push(r);
        }
        let same = (0..w * h).filter(|&i| renders[0].coeffs[i * 4..i * 4 + 4] == renders[1].coeffs[i * 4..i * 4 + 4]).count();
        println!("same polynomial at {same} of {} pixels", w * h);
    }
}
