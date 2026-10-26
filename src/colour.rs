//! Turning a Render into an RGB image: what is plotted, how it is scaled, and colormaps.

use crate::engine::Render;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Colormap {
    Bone,
    Gray,
    Hot,
    Copper,
    Inferno,
    Magma,
    Plasma,
    Viridis,
    Turbo,
    Hsv,
    Spring,
    Summer,
    Autumn,
    Winter,
    Cool,
    Pink,
    Jet,
    Parula,
    Sinebow,
    Neon,
    Acid,
    Candy,
    Cosmic,
}

impl Colormap {
    pub const ALL: [Colormap; 23] = [
        Colormap::Bone,
        Colormap::Gray,
        Colormap::Hot,
        Colormap::Copper,
        Colormap::Pink,
        Colormap::Spring,
        Colormap::Summer,
        Colormap::Autumn,
        Colormap::Winter,
        Colormap::Cool,
        Colormap::Jet,
        Colormap::Parula,
        Colormap::Inferno,
        Colormap::Magma,
        Colormap::Plasma,
        Colormap::Viridis,
        Colormap::Turbo,
        Colormap::Hsv,
        Colormap::Sinebow,
        Colormap::Neon,
        Colormap::Acid,
        Colormap::Candy,
        Colormap::Cosmic,
    ];

    /// The two ends have the same colour, so colour cycling has no seams.
    pub fn cyclic(self) -> bool {
        matches!(self, Colormap::Hsv | Colormap::Sinebow | Colormap::Neon | Colormap::Acid)
    }

    pub fn name(self) -> &'static str {
        match self {
            Colormap::Bone => "bone",
            Colormap::Gray => "gray",
            Colormap::Hot => "hot",
            Colormap::Copper => "copper",
            Colormap::Inferno => "inferno",
            Colormap::Magma => "magma",
            Colormap::Plasma => "plasma",
            Colormap::Viridis => "viridis",
            Colormap::Turbo => "turbo",
            Colormap::Hsv => "hsv",
            Colormap::Spring => "spring",
            Colormap::Summer => "summer",
            Colormap::Autumn => "autumn",
            Colormap::Winter => "winter",
            Colormap::Cool => "cool",
            Colormap::Pink => "pink",
            Colormap::Jet => "jet",
            Colormap::Parula => "parula",
            Colormap::Sinebow => "sinebow",
            Colormap::Neon => "neon",
            Colormap::Acid => "acid",
            Colormap::Candy => "candy",
            Colormap::Cosmic => "cosmic",
        }
    }

    /// Colour for x in [0, 1].
    pub fn map(self, x: f64) -> [u8; 3] {
        let x = if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) };
        let f = |r: f64, g: f64, b: f64| [to8(r), to8(g), to8(b)];
        match self {
            // MATLAB's definitions
            Colormap::Bone => {
                let (hr, hg, hb) = hot(x);
                f((7.0 * x + hb) / 8.0, (7.0 * x + hg) / 8.0, (7.0 * x + hr) / 8.0)
            }
            Colormap::Gray => f(x, x, x),
            Colormap::Hot => {
                let (r, g, b) = hot(x);
                f(r, g, b)
            }
            Colormap::Copper => f((1.25 * x).min(1.0), 0.7812 * x, 0.4975 * x),
            Colormap::Hsv => {
                let h = x * 6.0;
                let c = |o: f64| (((h + o) % 6.0 - 3.0).abs() - 1.0).clamp(0.0, 1.0);
                f(c(0.0), c(4.0), c(2.0))
            }
            Colormap::Spring => f(1.0, x, 1.0 - x),
            Colormap::Summer => f(x, 0.5 + x / 2.0, 0.4),
            Colormap::Autumn => f(1.0, x, 0.0),
            Colormap::Winter => f(0.0, x, 1.0 - x / 2.0),
            Colormap::Cool => f(x, 1.0 - x, 1.0),
            Colormap::Pink => {
                let (r, g, b) = hot(x);
                let p = |h: f64| ((2.0 * x + h) / 3.0).sqrt();
                f(p(r), p(g), p(b))
            }
            Colormap::Jet => {
                let c = |o: f64| (1.5 - (4.0 * x - o).abs()).clamp(0.0, 1.0);
                f(c(3.0), c(2.0), c(1.0))
            }
            Colormap::Parula => stops(&PARULA, x),
            // matplotlib's perceptual maps, sampled at evenly spaced stops
            Colormap::Inferno => stops(&INFERNO, x),
            Colormap::Magma => stops(&MAGMA, x),
            Colormap::Plasma => stops(&PLASMA, x),
            Colormap::Viridis => stops(&VIRIDIS, x),
            Colormap::Turbo => stops(&TURBO, x),
            // psychedelic maps: rainbow-like sine and cosine waves (Inigo Quilez's palettes)
            Colormap::Sinebow => {
                let s = |o: f64| (std::f64::consts::PI * (x + o)).sin().powi(2);
                f(s(0.5), s(0.5 + 1.0 / 3.0), s(0.5 + 2.0 / 3.0))
            }
            Colormap::Neon => cosine(x, [0.5; 3], [0.5; 3], [2.0, 1.0, 0.0], [0.5, 0.2, 0.25]),
            Colormap::Acid => cosine(x, [0.8, 0.5, 0.4], [0.2, 0.4, 0.2], [2.0, 1.0, 1.0], [0.0, 0.25, 0.25]),
            Colormap::Candy => cosine(x, [0.5; 3], [0.5; 3], [1.0, 1.0, 0.5], [0.8, 0.9, 0.3]),
            Colormap::Cosmic => cosine(x, [0.5; 3], [0.5; 3], [1.0, 0.7, 0.4], [0.0, 0.15, 0.2]),
        }
    }
}

/// a + b cos(2 pi (c x + d)), per channel
fn cosine(x: f64, a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> [u8; 3] {
    std::array::from_fn(|k| to8(a[k] + b[k] * (std::f64::consts::TAU * (c[k] * x + d[k])).cos()))
}

fn to8(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn hot(x: f64) -> (f64, f64, f64) {
    (
        (x * 8.0 / 3.0).clamp(0.0, 1.0),
        (x * 8.0 / 3.0 - 1.0).clamp(0.0, 1.0),
        (x * 4.0 - 3.0).clamp(0.0, 1.0),
    )
}

fn stops(t: &[[u8; 3]], x: f64) -> [u8; 3] {
    let n = t.len() - 1;
    let p = x * n as f64;
    let i = (p.floor() as usize).min(n - 1);
    let f = p - i as f64;
    let mut out = [0u8; 3];
    for k in 0..3 {
        out[k] = (t[i][k] as f64 * (1.0 - f) + t[i + 1][k] as f64 * f).round() as u8;
    }
    out
}

const VIRIDIS: [[u8; 3]; 9] = [
    [68, 1, 84], [71, 44, 122], [59, 81, 139], [44, 113, 142], [33, 144, 141],
    [39, 173, 129], [92, 200, 99], [170, 220, 50], [253, 231, 37],
];
const INFERNO: [[u8; 3]; 9] = [
    [0, 0, 4], [31, 12, 72], [85, 15, 109], [136, 34, 106], [186, 54, 85],
    [227, 89, 51], [249, 140, 10], [249, 201, 50], [252, 255, 164],
];
const MAGMA: [[u8; 3]; 9] = [
    [0, 0, 4], [28, 16, 68], [79, 18, 123], [129, 37, 129], [181, 54, 122],
    [229, 80, 100], [251, 135, 97], [254, 194, 135], [252, 253, 191],
];
const PLASMA: [[u8; 3]; 9] = [
    [13, 8, 135], [75, 3, 161], [125, 3, 168], [168, 34, 150], [203, 70, 121],
    [229, 107, 93], [248, 148, 65], [253, 195, 40], [240, 249, 33],
];
// MATLAB's default map since R2014b, sampled at 9 stops (close, not exact)
const PARULA: [[u8; 3]; 9] = [
    [62, 38, 168], [71, 82, 244], [44, 135, 247], [21, 175, 219], [55, 200, 151],
    [146, 205, 79], [225, 186, 58], [252, 207, 49], [249, 251, 21],
];
const TURBO: [[u8; 3]; 11] = [
    [48, 18, 59], [67, 85, 197], [63, 142, 251], [34, 194, 227], [32, 232, 174], [100, 253, 106],
    [165, 251, 56], [226, 220, 56], [254, 163, 49], [231, 84, 16], [122, 4, 3],
];

/// What each pixel's colour represents.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColourBy {
    /// 1 / residual^n: larger n makes the points near algebraic numbers sharper
    Power,
    /// -log(residual): shows the most structure, but blurrier
    Log,
    /// sum |ci| of the best polynomial
    Length,
    /// max |ci| of the best polynomial
    Height,
}

impl ColourBy {
    pub const ALL: [ColourBy; 4] = [ColourBy::Power, ColourBy::Log, ColourBy::Length, ColourBy::Height];

    pub fn name(self) -> &'static str {
        match self {
            ColourBy::Power => "1 / residual^n",
            ColourBy::Log => "-log(residual)",
            ColourBy::Length => "length (sum |ci|)",
            ColourBy::Height => "height (max |ci|)",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            ColourBy::Power => "pow",
            ColourBy::Log => "log",
            ColourBy::Length => "len",
            ColourBy::Height => "hgt",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Style {
    pub by: ColourBy,
    /// exponent for ColourBy::Power (and for shading)
    pub n: f64,
    /// percentiles (0-100) of the residual brightness that map to black (low) and to
    /// full brightness (high); everything outside is clipped
    pub low: f64,
    pub high: f64,
    pub cmap: Colormap,
    pub invert: bool,
    /// for Length/Height: darken pixels by their 1/residual^n brightness
    pub shade: bool,
    /// for Length/Height: spread the colormap over ln(value) instead of the value
    pub log_poly: bool,
    /// average the residual brightness over each pixel instead of taking it at the pixel's
    /// centre, so that points smaller than a pixel don't twinkle as they move (see
    /// `Stats::cell_mean`)
    pub smooth: bool,
    /// x -> x^(1/gamma) before the colormap: above 1 brightens the midtones
    pub gamma: f64,
    /// run through the colormap this many times (with `shift` added, as a fraction of it)
    pub cycles: f64,
    pub shift: f64,
    /// applied to the final RGB, like an image editor: brightness is added (-1..1),
    /// contrast scales around mid-grey, saturation scales away from grey
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
}

impl Default for Style {
    fn default() -> Style {
        Style {
            by: ColourBy::Power,
            n: 1.0,
            low: 0.0,
            high: 99.5,
            cmap: Colormap::Bone,
            invert: false,
            shade: true,
            log_poly: false,
            smooth: true,
            gamma: 1.0,
            cycles: 1.0,
            shift: 0.0,
            brightness: 0.0,
            contrast: 1.0,
            saturation: 1.0,
        }
    }
}

impl Style {
    /// Colour of a value x in [0, 1] (after scaling), darkened by w in [0, 1].
    pub fn colour(&self, x: f64, w: f64) -> [u8; 3] {
        self.finish(self.base(x), w)
    }

    /// The colormap's colour for x, after invert, gamma and cycling.
    fn base(&self, x: f64) -> [u8; 3] {
        let mut x = if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) };
        if self.invert {
            x = 1.0 - x;
        }
        if self.gamma != 1.0 {
            x = x.powf(1.0 / self.gamma.max(1e-3));
        }
        if self.cycles != 1.0 || self.shift != 0.0 {
            let t = x * self.cycles + self.shift;
            x = if self.cmap.cyclic() {
                t.rem_euclid(1.0)
            } else {
                // there and back, so non-cyclic maps don't jump from one end to the other
                let u = t.rem_euclid(2.0);
                if u > 1.0 { 2.0 - u } else { u }
            };
        }
        self.cmap.map(x)
    }

    /// Shading by w, then saturation, contrast and brightness.
    fn finish(&self, c: [u8; 3], w: f64) -> [u8; 3] {
        if w >= 1.0 && self.brightness == 0.0 && self.contrast == 1.0 && self.saturation == 1.0 {
            return c;
        }
        let mut c = c.map(|v| v as f64 / 255.0 * w);
        if self.saturation != 1.0 {
            let grey = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
            c = c.map(|v| grey + (v - grey) * self.saturation);
        }
        c.map(|v| to8((v - 0.5) * self.contrast + 0.5 + self.brightness))
    }

    /// The colormap as it is currently applied, as `n` RGB samples (for a preview strip).
    pub fn ramp(&self, n: usize) -> Vec<u8> {
        (0..n).flat_map(|i| self.colour(i as f64 / (n - 1).max(1) as f64, 1.0)).collect()
    }
}

/// Per-render statistics, computed once so recolouring is a single pass.
pub struct Stats {
    /// ln(residual) of each pixel
    ln: Vec<f32>,
    /// the same, sorted ascending (for percentiles)
    sorted_ln: Vec<f32>,
    length: Vec<u32>,
    height: Vec<u32>,
    length_range: (u32, u32),
    height_range: (u32, u32),
    /// per pixel, the best polynomial near it as a straight line, for `cell_mean`:
    /// p(X + δ) ≈ p(X) + p'(X) δ vanishes at δ = z. Stored as [z.re / dx, z.im / dy,
    /// ln(|p'(X)| dx)], with dx and dy the pixel spacing (NaN where there's no line)
    line: Vec<[f32; 3]>,
    /// dy / dx
    aspect: f32,
}

fn ln_res(r: f64) -> f64 {
    r.max(1e-300).ln()
}

/// Pixels whose line's zero is further than this from their centre (in pixels) have
/// nearly the same brightness all over, and keep their centre's.
const NEAR: f32 = 4.0;

/// Per pixel, on all cores: ln(residual), the length and height of its polynomial, and
/// its line (see `Stats::line`).
#[allow(clippy::type_complexity)]
fn per_pixel(r: &Render) -> (Vec<f32>, Vec<u32>, Vec<u32>, Vec<[f32; 3]>, f32) {
    let (n, t, w) = (r.residual.len(), r.nterms, r.width);
    let (mut ln, mut length, mut height) = (vec![0f32; n], vec![0u32; n], vec![0u32; n]);
    let mut line = vec![[f32::NAN; 3]; n];
    // the pixel spacing (a single row or column has the other's)
    let gap = |a: &[f64]| (a.len() > 1).then(|| (a[1] - a[0]).abs()).filter(|&d| d > 0.0);
    let (dx, dy) = match (gap(&r.re_axis), gap(&r.im_axis)) {
        (Some(x), Some(y)) => (x, y),
        (Some(x), None) => (x, x),
        (None, Some(y)) => (y, y),
        (None, None) => (0.0, 0.0),
    };
    let threads = std::thread::available_parallelism().map_or(4, |c| c.get());
    let per = n.div_ceil(threads).max(1 << 14);
    std::thread::scope(|s| {
        let parts = ln.chunks_mut(per).zip(length.chunks_mut(per)).zip(height.chunks_mut(per)).zip(line.chunks_mut(per));
        for (c, (((ln, length), height), line)) in parts.enumerate() {
            s.spawn(move || {
                for j in 0..ln.len() {
                    let i = c * per + j;
                    ln[j] = ln_res(r.residual[i]) as f32;
                    let co = &r.coeffs[i * t..(i + 1) * t];
                    length[j] = co.iter().map(|v| v.unsigned_abs()).sum();
                    height[j] = co.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
                    if dx > 0.0 {
                        line[j] = line_of(co, r.re_axis[i % w], r.im_axis[i / w], dx, dy);
                    }
                }
            });
        }
    });
    (ln, length, height, line, (dy / dx) as f32)
}

/// ln(x) for positive normal x, to within about 1e-4 (several times quicker than `ln`).
fn fast_ln(x: f32) -> f32 {
    // x = m 2^e with m in [1, 2): ln x = e ln 2 + ln m, and ln m from a polynomial in
    // t = (m - 1) / (m + 1): ln m = 2 (t + t^3/3 + t^5/5 + ...), |t| <= 1/3
    let bits = x.to_bits();
    let e = ((bits >> 23) & 0xff) as i32 - 127;
    let m = f32::from_bits((bits & 0x007f_ffff) | 0x3f80_0000);
    let t = (m - 1.0) / (m + 1.0);
    let t2 = t * t;
    e as f32 * std::f32::consts::LN_2 + 2.0 * t * (1.0 + t2 * (1.0 / 3.0 + t2 * (0.2 + t2 / 7.0)))
}

/// The line of polynomial `co` ([c0..cn]) at x + iy (see `Stats::line`).
fn line_of(co: &[i32], x: f64, y: f64, dx: f64, dy: f64) -> [f32; 3] {
    let mul = |(a, b): (f64, f64), (c, d): (f64, f64)| (a * c - b * d, a * d + b * c);
    // Horner's rule for p and p' together
    let (mut p, mut d) = ((0.0, 0.0), (0.0, 0.0));
    for &c in co.iter().rev() {
        let pd = mul(d, (x, y));
        d = (pd.0 + p.0, pd.1 + p.1);
        let pp = mul(p, (x, y));
        p = (pp.0 + c as f64, pp.1);
    }
    let dd = d.0 * d.0 + d.1 * d.1;
    if !(dd > 0.0 && dd.is_finite()) {
        return [f32::NAN; 3];
    }
    // z = -p / p'
    let z = (-(p.0 * d.0 + p.1 * d.1) / dd, -(p.1 * d.0 - p.0 * d.1) / dd);
    let (u, v) = (z.0 / dx, z.1 / dy);
    if !(u.abs() < NEAR as f64 && v.abs() < NEAR as f64) {
        return [f32::NAN; 3];
    }
    [u as f32, v as f32, (0.5 * dd.ln() + dx.ln()) as f32]
}

impl Stats {
    pub fn new(r: &Render) -> Stats {
        let (ln, length, height, line, aspect) = per_pixel(r);
        let mut sorted_ln = ln.clone();
        sorted_ln.sort_unstable_by(|a, b| a.total_cmp(b));
        Stats::with(ln, sorted_ln, length, height, line, aspect)
    }

    fn with(ln: Vec<f32>, sorted_ln: Vec<f32>, length: Vec<u32>, height: Vec<u32>, line: Vec<[f32; 3]>, aspect: f32) -> Stats {
        let range = |v: &[u32]| (v.iter().copied().min().unwrap_or(0), v.iter().copied().max().unwrap_or(0));
        Stats { length_range: range(&length), height_range: range(&height), ln, sorted_ln, length, height, line, aspect }
    }

    /// The mean over pixel i's cell of `lut` (a table over ln(residual): entry k at
    /// ln_hi + k / per), from its line, if it has one; None if it doesn't, or its zero is
    /// far enough away that the cell's centre will do.
    ///
    /// A point smaller than a pixel is a sharp peak of brightness, so a pixel's centre
    /// sees more or less of it depending on exactly where it falls, and points twinkle as
    /// a zoom moves them across the pixels. Averaged over the cell, a point's brightness
    /// shifts smoothly from one pixel to the next instead. Across one pixel the best
    /// polynomial is very nearly a straight line, so the average needs no more searching:
    /// it's taken over a grid of 4 x 4 points of the cell, or 8 x 8 within a pixel of the
    /// zero, where the brightness changes fastest.
    fn cell_mean(&self, i: usize, ln_hi: f64, per: f64, lut: &[f32]) -> Option<f32> {
        let [u, v, lnd] = self.line[i];
        if u.is_nan() {
            return None;
        }
        let s = if u * u + v * v < 1.0 { 8 } else { 4 };
        let (a2, last) = (self.aspect * self.aspect, (lut.len() - 1) as f32);
        // the table index: (ln|p| - ln_hi) per, with ln|p| = lnd + ln(distance^2) / 2
        let (base, half) = ((lnd as f64 - ln_hi) as f32 * per as f32, 0.5 * per as f32);
        let step = 1.0 / s as f32;
        let mut sum = 0.0;
        for j in 0..s {
            let dv = (j as f32 + 0.5) * step - 0.5 - v;
            for k in 0..s {
                let du = (k as f32 + 0.5) * step - 0.5 - u;
                let x = base + half * fast_ln((du * du + a2 * dv * dv).max(1e-30));
                sum += lut[(x + 0.5).clamp(0.0, last) as usize];
            }
        }
        Some(sum / (s * s) as f32)
    }

    /// Like `new`, but quicker for big renders (the video maker needs one per frame).
    /// Instead of sorting every pixel, it reads a table of 16 385 evenly spaced
    /// percentiles off a fine histogram of all of them. Its percentiles are exact to
    /// about 1e-4 in ln(residual) and 1/16 384 in percentile. Unlike the percentiles of
    /// a sample of the pixels, they change smoothly from one render to the next.
    pub fn binned(r: &Render) -> Stats {
        const K: usize = 1 << 14; // the table has K + 1 entries, for percentiles i / K
        const BINS: usize = 1 << 16;
        let (ln, length, height, line, aspect) = per_pixel(r);
        let n = ln.len();
        let exact = |ln: Vec<f32>| {
            let mut sorted = ln.clone();
            sorted.sort_unstable_by(|a, b| a.total_cmp(b));
            Stats::with(ln, sorted, length.clone(), height.clone(), line.clone(), aspect)
        };
        if n <= 8 * K {
            return exact(ln);
        }
        // The histogram spans the 0.2% to 99.8% points of a sample. The few pixels
        // outside that (such as exact roots, with ln(residual) near -690) are kept and
        // sorted instead.
        const PHI: f64 = 0.618_033_988_749_894_8;
        let mut sample: Vec<f32> = (0..4096).map(|i| ln[((i as f64 * PHI).fract() * n as f64) as usize]).collect();
        sample.sort_unstable_by(|a, b| a.total_cmp(b));
        let (lo, hi) = (sample[8], sample[4087]);
        if !(hi > lo) {
            return exact(ln);
        }
        let scale = BINS as f64 / (hi - lo) as f64;
        let threads = std::thread::available_parallelism().map_or(4, |c| c.get());
        let per = n.div_ceil(threads).max(1 << 14);
        let parts: Vec<(Vec<u32>, Vec<f32>, Vec<f32>)> = std::thread::scope(|s| {
            let handles: Vec<_> = ln
                .chunks(per)
                .map(|c| {
                    s.spawn(move || {
                        let (mut hist, mut below, mut above) = (vec![0u32; BINS], Vec::new(), Vec::new());
                        for &v in c {
                            if v < lo {
                                below.push(v);
                            } else if v >= hi {
                                above.push(v);
                            } else {
                                hist[(((v - lo) as f64 * scale) as usize).min(BINS - 1)] += 1;
                            }
                        }
                        (hist, below, above)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let mut hist = vec![0u32; BINS];
        let (mut below, mut above) = (Vec::new(), Vec::new());
        for (h, b, a) in parts {
            hist.iter_mut().zip(&h).for_each(|(x, y)| *x += y);
            below.extend(b);
            above.extend(a);
        }
        below.sort_unstable_by(|a, b| a.total_cmp(b));
        above.sort_unstable_by(|a, b| a.total_cmp(b));

        // the value at each rank, interpolating within a bin by the rank's position in it
        let first_above = n - above.len();
        let (mut bin, mut before) = (0usize, below.len()); // pixels before `bin`
        let table = (0..=K)
            .map(|i| {
                let rank = ((n - 1) as f64 * i as f64 / K as f64).round() as usize;
                if rank < below.len() {
                    return below[rank];
                }
                if rank >= first_above {
                    return above[rank - first_above];
                }
                while before + hist[bin] as usize <= rank {
                    before += hist[bin] as usize;
                    bin += 1;
                }
                let frac = (rank - before) as f64 + 0.5;
                (lo as f64 + (bin as f64 + frac / hist[bin] as f64) / scale) as f32
            })
            .collect();
        Stats::with(ln, table, length, height, line, aspect)
    }

    pub fn length(&self, i: usize) -> u32 {
        self.length[i]
    }

    pub fn height(&self, i: usize) -> u32 {
        self.height[i]
    }

    /// ln(residual) at percentile q in [0, 1]
    fn pct(&self, q: f64) -> f64 {
        pct(&self.sorted_ln, q)
    }

    /// The colour scale this render gets with style `s` on its own.
    pub fn scale(&self, s: &Style) -> Scale {
        Scale::of(|q| self.pct(q), self.length_range, self.height_range, s)
    }

    /// What `scale` needs, without the per-pixel values (at most 16 385 percentiles).
    pub fn levels(&self) -> Levels {
        const K: usize = 1 << 14;
        let n = self.sorted_ln.len();
        let sorted_ln = if n <= K + 1 { self.sorted_ln.clone() } else { (0..=K).map(|i| self.pct(i as f64 / K as f64) as f32).collect() };
        Levels { sorted_ln, length_range: self.length_range, height_range: self.height_range }
    }
}

fn pct(sorted: &[f32], q: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * q.clamp(0.0, 1.0)).round() as usize] as f64
}

/// A render's percentiles of ln(residual) and its length and height ranges: enough to
/// work out its colour scale for any style, to colour other renders on it.
#[derive(Clone, Debug)]
pub struct Levels {
    sorted_ln: Vec<f32>,
    length_range: (u32, u32),
    height_range: (u32, u32),
}

impl Levels {
    pub fn scale(&self, s: &Style) -> Scale {
        Scale::of(|q| pct(&self.sorted_ln, q), self.length_range, self.height_range, s)
    }
}

/// What the ends of the colour scale are: the ln(residual) that becomes full brightness
/// and the one that becomes black (from the max % and min % percentiles), and the
/// lengths and heights at the two ends of the colormap. Normally each render is coloured
/// on its own scale; colouring several on one scale keeps their colours comparable.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Scale {
    pub ln_hi: f64,
    pub ln_lo: f64,
    pub length: (f64, f64),
    pub height: (f64, f64),
}

impl Scale {
    fn of(pct: impl Fn(f64) -> f64, length: (u32, u32), height: (u32, u32), s: &Style) -> Scale {
        let high = (s.high / 100.0).clamp(0.0, 1.0);
        let low = (s.low / 100.0).clamp(0.0, high);
        let f = |(a, b): (u32, u32)| (a as f64, b as f64);
        Scale { ln_hi: pct(1.0 - high), ln_lo: pct(1.0 - low), length: f(length), height: f(height) }
    }

    /// The weighted mean of scales (weights summing to anything positive): ln(residual)
    /// averaged, and the lengths and heights averaged on a log scale.
    pub fn mix(parts: &[(Scale, f64)]) -> Scale {
        if let [(s, _)] = parts.iter().filter(|p| p.1 > 0.0).collect::<Vec<_>>()[..] {
            return *s; // exactly
        }
        let total: f64 = parts.iter().map(|p| p.1).sum::<f64>().max(1e-300);
        let avg = |f: &dyn Fn(&Scale) -> f64| parts.iter().map(|(s, w)| f(s) * w).sum::<f64>() / total;
        let geo = |f: &dyn Fn(&Scale) -> f64| avg(&|s| f(s).max(1.0).ln()).exp();
        Scale {
            ln_hi: avg(&|s| s.ln_hi),
            ln_lo: avg(&|s| s.ln_lo),
            length: (geo(&|s| s.length.0), geo(&|s| s.length.1)),
            height: (geo(&|s| s.height.0), geo(&|s| s.height.1)),
        }
    }
}

/// Colours every pixel on the render's own scale; returns packed RGB, row-major, top row first.
pub fn colourise(r: &Render, st: &Stats, s: &Style) -> Vec<u8> {
    colourise_with(r, st, s, &st.scale(s))
}

/// Like `colourise`, on a given scale.
pub fn colourise_with(r: &Render, st: &Stats, s: &Style, scale: &Scale) -> Vec<u8> {
    let mut out = vec![[0u8; 3]; r.residual.len()];
    colourise_into(r, st, s, scale, &mut out, |c| c);
    out.into_flattened()
}

/// Size limits of the lookup tables over ln(residual)
const LUT_MIN: usize = 1 << 12;
const LUT_MAX: usize = 1 << 17;

/// Colours every pixel into `out` (row-major, top row first) on `scale` (usually
/// `st.scale(s)`), converting each with `f`.
///
/// For speed, the colour of every pixel comes from a lookup table: a pixel's colour
/// depends only on its ln(residual) (Power, Log) or its length/height, so the colour
/// pipeline runs once per table entry instead of once per pixel, and the pixels are
/// filled in parallel. Recolouring a 1024 x 1024 image takes a few milliseconds.
pub fn colourise_into<T: Copy + Send + Sync>(
    r: &Render,
    st: &Stats,
    s: &Style,
    scale: &Scale,
    out: &mut [T],
    f: impl Fn([u8; 3]) -> T + Sync,
) {
    assert_eq!(out.len(), r.residual.len(), "output must have one entry per pixel");
    let n = s.n.max(1e-6);
    // ln(residual) of the pixels at the high and low brightness percentiles
    let (ln_hi, ln_lo) = (scale.ln_hi, scale.ln_lo.max(scale.ln_hi));
    // brightness 1/res^n, scaled so that the high percentile is 1:
    //   (res / res_hi)^-n = exp(-n (ln res - ln res_hi))
    // then stretched so that the low percentile is 0
    let floor = (-n * (ln_lo - ln_hi)).exp().min(1.0 - 1e-9);
    let power = |l: f64| (((-n * (l - ln_hi)).exp().min(1.0) - floor) / (1.0 - floor)).max(0.0);
    // -log(res) from the low to the high percentile
    let span = (ln_lo - ln_hi).max(1e-9);
    let log = |l: f64| (ln_lo - l) / span;

    // Both are constant outside [ln_hi, ln_lo], so the tables only need to cover that
    // range: entry k is at ln(res) = ln_hi + k / per, and pixels outside clamp to the ends.
    // Power's relative slope is n per unit of ln(res), so steep settings get finer tables.
    let entries = ((n.max(1.0) * span * 2000.0) as usize).clamp(LUT_MIN, LUT_MAX);
    let per = (entries - 1) as f64 / span;
    let at = |k: usize| ln_hi + k as f64 / per;
    let ln = &st.ln;
    let index = |i: usize| ((ln[i] as f64 - ln_hi) * per).clamp(0.0, (entries - 1) as f64).round() as usize;

    match s.by {
        ColourBy::Power | ColourBy::Log => {
            let x = if s.by == ColourBy::Power { &power as &(dyn Fn(f64) -> f64 + Sync) } else { &log };
            let mut lut = vec![f([0; 3]); entries];
            par_fill(&mut lut, |k| f(s.colour(x(at(k)), 1.0)));
            if s.smooth {
                // the brightness averaged over the pixels near points, coloured by a table over it
                const LEVELS: usize = 4096;
                let mut xs = vec![0f32; entries];
                par_fill(&mut xs, |k| x(at(k)) as f32);
                let mut by_x = vec![f([0; 3]); LEVELS];
                par_fill(&mut by_x, |k| f(s.colour(k as f64 / (LEVELS - 1) as f64, 1.0)));
                par_fill(out, |i| match st.cell_mean(i, ln_hi, per, &xs) {
                    Some(m) => by_x[(m * (LEVELS - 1) as f32).clamp(0.0, (LEVELS - 1) as f32).round() as usize],
                    None => lut[index(i)],
                });
            } else {
                par_fill(out, |i| lut[index(i)]);
            }
        }
        ColourBy::Length | ColourBy::Height => {
            // the tables cover this render's values; the scale's ends map to the colormap's
            let (v, (a, b), ends) = match s.by {
                ColourBy::Length => (&st.length, st.length_range, scale.length),
                _ => (&st.height, st.height_range, scale.height),
            };
            let norm = |v: u32| poly_norm(v, ends, s.log_poly);
            if s.shade {
                let base: Vec<[u8; 3]> = (a..=b).map(|v| s.base(norm(v))).collect();
                let mut w = vec![0f32; entries];
                par_fill(&mut w, |k| power(at(k)) as f32);
                let shade = |i: usize| if s.smooth { st.cell_mean(i, ln_hi, per, &w) } else { None }.unwrap_or(w[index(i)]);
                par_fill(out, |i| f(s.finish(base[(v[i] - a) as usize], shade(i) as f64)));
            } else {
                let lut: Vec<T> = (a..=b).map(|v| f(s.colour(norm(v), 1.0))).collect();
                par_fill(out, |i| lut[(v[i] - a) as usize]);
            }
        }
    }
}

/// Where a length or height `v` is between the scale's ends `lo` and `hi` (0..1), on a
/// log scale if `log`.
fn poly_norm(v: u32, (lo, hi): (f64, f64), log: bool) -> f64 {
    let f = |x: f64| if log { x.max(1.0).ln() } else { x };
    let (a, b) = (f(lo), f(hi));
    if b > a { ((f(v as f64) - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 }
}

/// out[i] = g(i), split across all cores.
fn par_fill<T: Send>(out: &mut [T], g: impl Fn(usize) -> T + Sync) {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = out.len().div_ceil(threads).max(1 << 12);
    let g = &g;
    std::thread::scope(|sc| {
        for (c, part) in out.chunks_mut(chunk).enumerate() {
            sc.spawn(move || {
                for (j, o) in part.iter_mut().enumerate() {
                    *o = g(c * chunk + j);
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A synthetic render: residuals spread over many orders of magnitude, plus exact zeros.
    fn fake(w: usize) -> Render {
        let px = w * w;
        let mut seed = 12345u64;
        let mut rnd = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let residual = (0..px).map(|i| if i % 997 == 0 { 0.0 } else { (-20.0 * rnd()).exp() }).collect();
        let coeffs = (0..px * 4).map(|_| (rnd() * 17.0) as i32 - 8).collect();
        Render { width: w, height: w, nterms: 4, re_axis: vec![0.0; w], im_axis: vec![0.0; w], residual, coeffs }
    }

    /// The lookup tables give the colours that the pipeline gives at each pixel's
    /// ln(residual), to within one table step (where steep settings can make
    /// neighbouring table entries differ by a few levels).
    /// The histogram's percentiles match sorting every pixel, outliers included.
    #[test]
    fn binned_percentiles_match() {
        let r = fake(600);
        let (exact, binned) = (Stats::new(&r), Stats::binned(&r));
        assert_eq!(binned.sorted_ln.len(), (1 << 14) + 1);
        assert_eq!(binned.ln, exact.ln);
        for q in [0.0, 0.001, 0.002, 0.005, 0.05, 0.3, 0.5, 0.9, 0.995, 0.999, 1.0] {
            // the table's percentiles are 1/16384 apart: allow a rank either side
            let n = exact.sorted_ln.len() as f64;
            let span = |d: f64| exact.sorted_ln[((n - 1.0) * (q + d).clamp(0.0, 1.0)).round() as usize];
            let (a, b) = (span(-1.0 / 16384.0), span(1.0 / 16384.0));
            let got = binned.pct(q) as f32;
            assert!(got >= a.min(b) - 1e-3 && got <= a.max(b) + 1e-3, "q {q}: {got} not in {a}..{b}");
        }
    }

    #[test]
    fn lookup_matches_direct() {
        let r = fake(256);
        let st = Stats::new(&r);
        for by in ColourBy::ALL {
            for (n, low, high, gamma, cycles, log_poly) in
                [(1.0, 0.0, 99.5, 1.0, 1.0, false), (20.0, 30.0, 99.9, 2.0, 7.0, true), (0.3, 60.0, 90.0, 0.5, 1.0, false)]
            {
                let s = Style { by, n, low, high, gamma, cycles, log_poly, cmap: Colormap::Acid, contrast: 1.3, saturation: 1.5, brightness: -0.1, ..Style::default() };
                let got = colourise(&r, &st, &s);
                // direct, per pixel
                let (hp, lp) = ((high / 100.0f64).clamp(0.0, 1.0), (low / 100.0f64).clamp(0.0, high / 100.0));
                let (ln_hi, ln_lo) = (st.pct(1.0 - hp), st.pct(1.0 - lp));
                let floor = (-n * (ln_lo - ln_hi)).exp().min(1.0 - 1e-9);
                let power = |l: f64| (((-n * (l - ln_hi)).exp().min(1.0) - floor) / (1.0 - floor)).max(0.0);
                let norm = |v: u32, (a, b): (u32, u32)| {
                    let f = |x: u32| if log_poly { (x.max(1) as f64).ln() } else { x as f64 };
                    if b > a { ((f(v) - f(a)) / (f(b) - f(a))).clamp(0.0, 1.0) } else { 0.0 }
                };
                let span = (ln_lo - ln_hi).max(1e-9);
                let step = span / (((n.max(1.0) * span * 2000.0) as usize).clamp(LUT_MIN, LUT_MAX) - 1) as f64;
                let mut worst = 0;
                for i in 0..r.residual.len() {
                    // the range of colours within half a table step of the pixel's ln(residual)
                    let (mut lo_c, mut hi_c) = ([255i32; 3], [0i32; 3]);
                    for t in -4..=4 {
                        let l = st.ln[i] as f64 + t as f64 / 8.0 * step;
                        let x = match by {
                            ColourBy::Power => power(l),
                            ColourBy::Log => (ln_lo - l) / span,
                            ColourBy::Length => norm(st.length[i], st.length_range),
                            ColourBy::Height => norm(st.height[i], st.height_range),
                        };
                        let w = if s.shade && matches!(by, ColourBy::Length | ColourBy::Height) { power(l) } else { 1.0 };
                        let c = s.colour(x, w);
                        for k in 0..3 {
                            lo_c[k] = lo_c[k].min(c[k] as i32);
                            hi_c[k] = hi_c[k].max(c[k] as i32);
                        }
                    }
                    for k in 0..3 {
                        let g = got[3 * i + k] as i32;
                        worst = worst.max(lo_c[k] - g).max(g - hi_c[k]);
                    }
                }
                // slack for the f32 ln(residual) and for samples between the 9 tried here
                assert!(worst <= 2, "{by:?} n={n}: off by {worst}");
            }
        }
    }

    /// Two renders coloured on one scale give equal values equal colours; their levels
    /// give the same scale as their stats.
    #[test]
    fn shared_scale() {
        let (a, b) = (fake(300), fake(170));
        let (sa, sb) = (Stats::binned(&a), Stats::new(&b));
        for by in ColourBy::ALL {
            for log_poly in [false, true] {
                let s = Style { by, log_poly, shade: false, ..Style::default() };
                assert_eq!(sa.levels().scale(&s), sa.scale(&s));
                let sc = sb.levels().scale(&s);
                // (a rank or two apart: ln(residual) is spread over 20 here)
                assert!((sc.ln_hi - sb.scale(&s).ln_hi).abs() < 0.01 && sc.length == sb.scale(&s).length);
                let scale = Scale::mix(&[(sa.scale(&s), 0.3), (sb.scale(&s), 0.7)]);
                let (ca, cb) = (colourise_with(&a, &sa, &s, &scale), colourise_with(&b, &sb, &s, &scale));
                let key = |st: &Stats, i: usize| match by {
                    ColourBy::Length => st.length(i) as i64,
                    ColourBy::Height => st.height(i) as i64,
                    _ => (st.ln[i] * 1024.0).round() as i64, // about equal
                };
                let mut seen = HashMap::new();
                for i in 0..a.residual.len() {
                    seen.insert(key(&sa, i), [ca[3 * i], ca[3 * i + 1], ca[3 * i + 2]]);
                }
                for i in 0..b.residual.len() {
                    if let Some(c) = seen.get(&key(&sb, i)) {
                        let d = (0..3).map(|k| (c[k] as i32 - cb[3 * i + k] as i32).abs()).max().unwrap();
                        assert!(d <= 3, "{by:?} log {log_poly}: off by {d}");
                    }
                }
            }
        }
    }

    #[test]
    fn fast_ln_is_close() {
        let worst = (0..100_000).map(|i| 1e-30f32 * 1.0007f32.powi(i)).take_while(|x| x.is_finite()).map(|x| (fast_ln(x) - x.ln()).abs()).fold(0f32, f32::max);
        assert!(worst < 1e-4, "{worst}");
    }

    /// With smooth stars, a point's total brightness stays steady as it moves across the
    /// pixels; at the pixels' centres alone, it depends on where it falls.
    #[test]
    fn smooth_points_keep_their_brightness() {
        // p(X) = 2X - 1, with its root 0.5 moving across the middle pixel in tenths
        let (w, d) = (9usize, 0.1);
        let scale = Scale { ln_hi: (0.02f64).ln(), ln_lo: (2.0f64).ln(), length: (3.0, 3.0), height: (2.0, 2.0) };
        // (the 8-bit levels of the few lit pixels alone make the totals vary by a few %)
        for (smooth, most, least) in [(false, 0.3, f64::INFINITY), (true, 0.0, 0.08)] {
            let totals: Vec<f64> = (0..10)
                .map(|step| {
                    let x0 = 0.5 - 4.0 * d + d * step as f64 / 10.0;
                    let re_axis: Vec<f64> = (0..w).map(|i| x0 + i as f64 * d).collect();
                    let im_axis: Vec<f64> = (0..w).map(|j| (4.0 - j as f64) * d).collect();
                    let residual = (0..w * w).map(|i| ((2.0 * re_axis[i % w] - 1.0).powi(2) + (2.0 * im_axis[i / w]).powi(2)).sqrt()).collect();
                    let r = Render { width: w, height: w, nterms: 2, re_axis, im_axis, residual, coeffs: [-1, 2].repeat(w * w) };
                    let st = Stats::new(&r);
                    let s = Style { n: 2.0, cmap: Colormap::Gray, smooth, ..Style::default() };
                    colourise_with(&r, &st, &s, &scale).iter().step_by(3).map(|&v| v as f64).sum()
                })
                .collect();
            let (lo, hi) = totals.iter().fold((f64::MAX, 0f64), |(a, b), &t| (a.min(t), b.max(t)));
            let spread = (hi - lo) / hi;
            eprintln!("smooth {smooth}: totals {lo:.0}..{hi:.0}, spread {spread:.3}");
            assert!(spread >= most && spread <= least, "smooth {smooth}: spread {spread}");
        }
    }

    /// Times colouring a real 720p render with and without smooth stars.
    /// cargo test --release --lib smooth_speed -- --ignored --nocapture
    #[test]
    #[ignore]
    fn smooth_speed() {
        use crate::engine::{render, Coefs, Plan, Region};
        let region = Region { re: (1.0 / 3.0 - 0.0356, 1.0 / 3.0 + 0.0356), im: (2f64.sqrt() / 3.0 - 0.02, 2f64.sqrt() / 3.0 + 0.02), width: 1280 };
        let plan = Plan::new(3, &Coefs { zero_root: true, ..Coefs::ints(40) }, false).unwrap();
        let r = render(&plan, &region, 12, &Default::default(), &Default::default()).unwrap();
        let st = Stats::new(&r);
        let near = st.line.iter().filter(|l| !l[0].is_nan()).count() as f64 / st.line.len() as f64;
        for smooth in [false, true] {
            let s = Style { smooth, ..Style::default() };
            let mut out = vec![[0u8; 3]; r.residual.len()];
            colourise_into(&r, &st, &s, &st.scale(&s), &mut out, |c| c);
            let t = std::time::Instant::now();
            for _ in 0..10 {
                colourise_into(&r, &st, &s, &st.scale(&s), &mut out, |c| c);
            }
            eprintln!("{}x{}, smooth {smooth} ({:.0}% of pixels near a point): {:.1} ms", r.width, r.height, near * 100.0, t.elapsed().as_secs_f64() * 100.0);
        }
    }

    #[test]
    fn recolour_speed() {
        let r = fake(1024);
        let st = Stats::new(&r);
        for (n, smooth) in [(1.0, false), (1.0, true), (20.0, false), (20.0, true)] {
            let s = Style { n, smooth, cmap: Colormap::Sinebow, gamma: 1.5, cycles: 5.0, contrast: 1.2, ..Style::default() };
            let mut out = vec![[0u8; 3]; r.residual.len()];
            colourise_into(&r, &st, &s, &st.scale(&s), &mut out, |c| c); // warm up
            let t = std::time::Instant::now();
            for _ in 0..10 {
                colourise_into(&r, &st, &s, &st.scale(&s), &mut out, |c| c);
            }
            eprintln!("recolour 1024x1024, n = {n}, smooth {smooth}: {:.2} ms", t.elapsed().as_secs_f64() * 100.0);
        }
    }
}
