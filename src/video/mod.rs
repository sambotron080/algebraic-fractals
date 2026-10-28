//! The zoom path through a list of views, the frame regions along it, the smooth
//! interpolation of the waypoints' max coefficients, the colouring of each frame (one
//! style for the whole video), and writing the frames to an H.264 .mp4 file (OpenH264,
//! compiled in, plus the mp4 crate's muxer).

pub mod frames;
pub mod make;
pub mod project;
pub mod reuse;

use crate::common::{View, MAX_COEF};
use crate::colour::{colourise_with, Levels, Scale, Stats, Style};
use crate::engine::{CoefSet, Region, Render};
use openh264::encoder::{
    BitRate, Complexity, Encoder, EncoderConfig, FrameRate, IntraFramePeriod, QpRange, RateControlMode, UsageType,
    VuiConfig,
};
use openh264::formats::YUVBuffer;
use openh264::OpenH264API;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/// Pan speed of a new segment: 1% of the frame's height per frame.
pub const DEFAULT_PAN: f64 = 0.01;

/// A zoom through a list of views (start, any waypoints, target), zooming by at most
/// `rate` per frame and panning by at most `pan[i]` frame heights per frame in segment i.
///
/// Within each segment the half-width changes geometrically, so the zoom speed looks
/// constant, and the centre moves in step with the half-width, so that the segment's
/// end point stays at the same place on screen relative to the frame as it closes in
/// (or recedes). That also makes the centre cross the screen at a constant speed. A
/// segment between views of the same size pans at a constant speed.
#[derive(Clone, PartialEq, Debug)]
pub struct ZoomPath {
    pub stops: Vec<View>,
    pub rate: f64,
    /// pan speed of each segment (stops.len() - 1 of them), in frame heights per frame
    pub pan: Vec<f64>,
    /// the segments where the max coefficient grows in proportion to the zoom, which
    /// keeps the dark flower around a quadratic number the same size (see keep_flower)
    pub flower: Vec<bool>,
}

impl ZoomPath {
    pub fn new(stops: Vec<View>, rate: f64) -> ZoomPath {
        let n = stops.len().saturating_sub(1);
        ZoomPath { stops, rate, pan: vec![DEFAULT_PAN; n], flower: vec![false; n] }
    }

    pub fn pan_speed(&self, i: usize) -> f64 {
        self.pan.get(i).copied().unwrap_or(DEFAULT_PAN).max(1e-6)
    }

    /// Whether segment i keeps the flower size.
    pub fn keeps_flower(&self, i: usize) -> bool {
        self.flower.get(i).copied().unwrap_or(false)
    }

    /// Frames from stop i to stop i + 1 (at least 1): enough that neither the zoom
    /// (ln(h0/h1) in all) goes faster than `rate`, nor the pan faster than its speed.
    pub fn steps(&self, i: usize) -> usize {
        let (a, b) = (self.stops[i], self.stops[i + 1]);
        let zoom = (a.hw / b.hw).ln().abs();
        // The centre moves |Δc| in all, while the frame is 2 hw high: in frame heights
        // that's |Δc| times the mean of 1/(2 hw) along the way, which (with the centre
        // tied to the half-width, see `view`) is ln(h0/h1) / (2 |h0 - h1|).
        let inv_hw = if zooms(a, b) { zoom / (a.hw - b.hw).abs() } else { 1.0 / a.hw };
        let pan = (b.cr - a.cr).hypot(b.ci - a.ci) * inv_hw / 2.0;
        let n = (zoom / self.rate.ln()).max(pan / self.pan_speed(i));
        (n.ceil() as usize).max(1)
    }

    /// Number of frames, both ends included.
    pub fn frames(&self) -> usize {
        (0..self.stops.len() - 1).map(|i| self.steps(i)).sum::<usize>() + 1
    }

    /// The frame showing stop `i` exactly.
    pub fn stop_frame(&self, i: usize) -> usize {
        (0..i.min(self.stops.len() - 1)).map(|j| self.steps(j)).sum()
    }

    /// The segment frame `k` is in and how far along it (0..1).
    pub fn locate(&self, k: usize) -> (usize, f64) {
        let mut start = 0;
        for i in 0..self.stops.len() - 1 {
            let n = self.steps(i);
            if k < start + n || i == self.stops.len() - 2 {
                return (i, ((k - start.min(k)) as f64 / n as f64).min(1.0));
            }
            start += n;
        }
        (0, 0.0) // a single stop
    }

    /// The view shown in frame `k`.
    pub fn view(&self, k: usize) -> View {
        if self.stops.len() < 2 {
            return self.stops[0];
        }
        let (i, t) = self.locate(k);
        let (s, e) = (self.stops[i], self.stops[i + 1]);
        let hw = s.hw * (e.hw / s.hw).powf(t);
        // how far along the move the centre is: 1 at the start, 0 at the end
        let a = if zooms(s, e) { (hw - e.hw) / (s.hw - e.hw) } else { 1.0 - t };
        View { cr: e.cr + (s.cr - e.cr) * a, ci: e.ci + (s.ci - e.ci) * a, hw }
    }

    /// The frame number of every stop, as f64 (for interpolating along the video).
    pub fn stop_frames(&self) -> Vec<f64> {
        (0..self.stops.len()).map(|i| self.stop_frame(i) as f64).collect()
    }
}

/// Whether the half-width changes between two views (otherwise it's a pure pan).
fn zooms(a: View, b: View) -> bool {
    (a.hw - b.hw).abs() > 1e-6 * a.hw.max(b.hw)
}

/// Monotone cubic interpolation (PCHIP, Fritsch-Carlson) through the points (xs, ys),
/// xs increasing, evaluated at x. The curve is smooth (no kink at the points), never
/// overshoots, and rises or falls steadily wherever the points do; it's flat at a
/// point where they turn round, and straight through points on a line.
pub fn smooth(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let n = xs.len();
    if n == 1 {
        return ys[0];
    }
    let x = x.clamp(xs[0], xs[n - 1]);
    let i = xs.partition_point(|&v| v <= x).saturating_sub(1).min(n - 2);
    let h = |j: usize| xs[j + 1] - xs[j];
    let d = |j: usize| (ys[j + 1] - ys[j]) / h(j);
    // the slope at an end, from the two nearest segments, kept from overshooting
    let end = |h0: f64, h1: f64, d0: f64, d1: f64| {
        let s = ((2.0 * h0 + h1) * d0 - h0 * d1) / (h0 + h1);
        if d0 == 0.0 || s.signum() != d0.signum() {
            0.0
        } else if d0.signum() != d1.signum() && s.abs() > 3.0 * d0.abs() {
            3.0 * d0
        } else {
            s
        }
    };
    let slope = |j: usize| {
        if n == 2 {
            d(0)
        } else if j == 0 {
            end(h(0), h(1), d(0), d(1))
        } else if j == n - 1 {
            end(h(n - 2), h(n - 3), d(n - 2), d(n - 3))
        } else if d(j - 1) * d(j) <= 0.0 {
            0.0
        } else {
            let (w1, w2) = (2.0 * h(j) + h(j - 1), h(j) + 2.0 * h(j - 1));
            (w1 + w2) / (w1 / d(j - 1) + w2 / d(j))
        }
    };
    let t = (x - xs[i]) / h(i);
    let (t2, t3) = (t * t, t * t * t);
    ys[i] * (2.0 * t3 - 3.0 * t2 + 1.0)
        + h(i) * slope(i) * (t3 - 2.0 * t2 + t)
        + ys[i + 1] * (3.0 * t2 - 2.0 * t3)
        + h(i) * slope(i + 1) * (t3 - t2)
}

/// The max coefficient in frame `k`: a smooth curve through the waypoints' values, on a
/// log scale. Keeping the density of algebraic numbers steady while zooming needs the max
/// coefficient to grow like a power of the magnification, which is a straight line on
/// this scale (the zoom is geometric in the frame number), so between two waypoints it
/// rises (or falls) by the same factor every frame. On a segment that keeps the flower
/// size it's exactly that straight line, so it grows in step with the zoom.
pub fn max_at(path: &ZoomPath, maxes: &[i32], k: usize) -> f64 {
    let ys: Vec<f64> = maxes.iter().map(|&m| (m.max(1) as f64).ln()).collect();
    let (i, t) = path.locate(k);
    let ln_m = if path.stops.len() > 1 && path.keeps_flower(i) {
        ys[i] + (ys[i + 1] - ys[i]) * t
    } else {
        smooth(&path.stop_frames(), &ys, k as f64)
    };
    let m = ln_m.exp();
    if (m - m.round()).abs() < 1e-6 { m.round() } else { m }
}

/// The max coefficient at the end of a segment that keeps the flower size: the start's,
/// times the segment's magnification.
///
/// Near a quadratic number β the stars are at about β - p(β)/p'(β). p(β) lies on a fixed
/// lattice, so it can't come closer to 0 than that allows, while |p'(β)| grows with the
/// max coefficient M. So the dark flower around β has a radius of about constant / M:
/// zooming in by the factor M grows by keeps it the same size on screen. (The stars
/// around it get denser, about 4x for each doubling.)
pub fn keep_flower(start_max: i32, from: View, to: View) -> i32 {
    (start_max.max(1) as f64 * from.hw / to.hw).round().clamp(1.0, MAX_COEF as f64) as i32
}

/// The simplest non-real quadratic number within `r` of (x, y): a root of
/// a z² + b z + c with a > 0, gcd(a, b, c) = 1 and b² < 4ac, of the smallest height
/// max(a, |b|, |c|) up to `max_height`, and of those the nearest. It's the centre of
/// the largest flower there. Returns (a, b, c) and the root (on the side of the
/// real axis that y is on).
pub fn simplest_quadratic(x: f64, y: f64, r: f64, max_height: i64) -> Option<([i64; 3], f64, f64)> {
    let gcd = |mut a: i64, mut b: i64| {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a.abs()
    };
    let sign = if y < 0.0 { -1.0 } else { 1.0 };
    let (ya, r) = (y.abs(), r.abs());
    // (height, distance, coefficients, root)
    let mut best: Option<(i64, f64, [i64; 3], f64, f64)> = None;
    for a in 1..=max_height {
        if best.is_some_and(|b| b.0 < a) {
            break; // every later root has a greater height
        }
        let af = a as f64;
        // re = -b / 2a within r of x, and im = sqrt(4ac - b²) / 2a within r of |y|
        let b_lo = (-2.0 * af * (x + r)).ceil() as i64;
        let b_hi = (-2.0 * af * (x - r)).floor() as i64;
        for b in b_lo..=b_hi {
            let bb = (b as f64) * (b as f64);
            let (im_lo, im_hi) = ((ya - r).max(0.0), ya + r);
            let c_lo = ((bb + (2.0 * af * im_lo).powi(2)) / (4.0 * af)).floor() as i64;
            let c_hi = ((bb + (2.0 * af * im_hi).powi(2)) / (4.0 * af)).ceil() as i64;
            for c in c_lo.max(1)..=c_hi {
                let disc = 4 * a * c - b * b;
                if disc <= 0 || gcd(gcd(a, b), c) != 1 {
                    continue;
                }
                let (re, im) = (-(b as f64) / (2.0 * af), sign * (disc as f64).sqrt() / (2.0 * af));
                let (d, h) = ((re - x).hypot(im - y), a.max(b.abs()).max(c));
                if d <= r && best.is_none_or(|bst| (h, d) < (bst.0, bst.1)) {
                    best = Some((h, d, [a, b, c], re, im));
                }
            }
        }
    }
    best.map(|b| (b.2, b.3, b.4))
}

/// For a fractional max coefficient `m`: the max coefficients just below and above it
/// that change the allowed set (`set`'s members up to floor(m), and up to its next
/// member above that), and how far `m` is from the lower to the upper (0..1, on a log
/// scale). Fading from one to the other lets the new algebraic numbers appear gradually.
pub fn fade_between(set: CoefSet, m: f64) -> (i32, i32, f64) {
    let m = m.clamp(1.0, MAX_COEF as f64);
    let lo = m.floor() as i32;
    let below = set.upto(lo).last().copied().unwrap_or(1).max(1);
    let mut n = lo.saturating_mul(2).max(4);
    let up = loop {
        let members = set.upto(n);
        let i = members.partition_point(|&v| v <= lo);
        if let Some(&u) = members.get(i) {
            break u;
        }
        if n >= MAX_COEF {
            return (lo, lo, 0.0);
        }
        n = n.saturating_mul(2).min(MAX_COEF);
    };
    let t = ((m / below as f64).ln() / (up as f64 / below as f64).ln()).clamp(0.0, 1.0);
    (lo, up, t)
}

/// Lookup tables between 8-bit values and linear light: 256 levels to linear, and back
/// from `BACK` + 1 linear levels.
pub const BACK: usize = 1 << 16;
pub fn light_tables() -> &'static (Vec<f32>, Vec<u8>) {
    static TABLES: std::sync::OnceLock<(Vec<f32>, Vec<u8>)> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| {
        let lin = (0..256).map(|v| (v as f32 / 255.0).powf(2.2)).collect();
        let back = (0..=BACK).map(|i| ((i as f32 / BACK as f32).powf(1.0 / 2.2) * 255.0).round() as u8).collect();
        (lin, back)
    })
}

/// Colours a frame's render with `style`, on `scale` or else on its own. With two
/// renders (at the max coefficients just below and above the frame's), cross-fades from
/// the first to the second by the second one's weight (see `cross_fade`).
pub fn colour_frame(renders: &[(Render, Stats, f64)], style: &Style, scale: Option<&Scale>) -> Vec<u8> {
    let images: Vec<(Vec<u8>, f64)> =
        renders.iter().map(|(r, st, w)| (colourise_with(r, st, style, &scale.copied().unwrap_or_else(|| st.scale(style))), *w)).collect();
    cross_fade(images)
}

/// The colour scale of frame `k` when it's held, from every waypoint's levels (`levels`,
/// one list per waypoint). The residual brightness is the scale at each end of the
/// frame's segment, mixed by how far along the segment it is. The lengths and heights
/// are on one range for the whole video: from the lowest at any waypoint (usually the
/// start) to the highest (usually the deepest), so a value has the same colour in every
/// frame.
pub fn held_scale(path: &ZoomPath, style: &Style, levels: &[Vec<(Levels, f64)>], k: usize) -> Scale {
    let (i, t) = path.locate(k);
    let at = |levels: &[(Levels, f64)]| Scale::mix(&levels.iter().map(|(l, w)| (l.scale(style), *w)).collect::<Vec<_>>());
    let j = (i + 1).min(levels.len() - 1);
    let (a, b) = (at(&levels[i]), at(&levels[j]));
    let mut s = Scale::mix(&[(a, 1.0 - t), (b, t)]);
    let all: Vec<Scale> = levels.iter().flatten().map(|(l, _)| l.scale(style)).collect();
    let range = |f: fn(&Scale) -> (f64, f64)| {
        (all.iter().map(|x| f(x).0).fold(f64::INFINITY, f64::min), all.iter().map(|x| f(x).1).fold(f64::NEG_INFINITY, f64::max))
    };
    if !all.is_empty() {
        (s.length, s.height) = (range(|x| x.length), range(|x| x.height));
    }
    s
}

/// Mixes images (8-bit RGB) by their weights, which sum to 1, in display brightness
/// (the 8-bit values), as video dissolves are. A star fading in then brightens on
/// screen at a steady rate, and one fade hands over to the next without a pulse. (Mixed
/// in linear light, it would brighten like t^(1/2.2): fast at first, then slowly.)
pub fn cross_fade(mut images: Vec<(Vec<u8>, f64)>) -> Vec<u8> {
    images.retain(|i| i.1 > 0.0);
    match images.len() {
        0 => Vec::new(),
        1 => images.pop().unwrap().0,
        _ => {
            let mut acc = vec![0f32; images[0].0.len()];
            for (img, w) in &images {
                let w = *w as f32;
                acc.iter_mut().zip(img).for_each(|(a, &v)| *a += v as f32 * w);
            }
            acc.iter().map(|&v| v.round().clamp(0.0, 255.0) as u8).collect()
        }
    }
}

/// Pixel width of a frame `height` pixels high with the given aspect ratio (always even,
/// as H.264's colour subsampling needs).
pub fn frame_width(height: usize, aspect: f64) -> usize {
    ((height as f64 * aspect / 2.0).round() as usize).max(1) * 2
}

/// The region a frame shows, sampled `aa` times more finely in each direction than the
/// frame's pixels: `v.hw` is half the frame's height, and wider frames show more at the sides.
pub fn frame_region(v: View, height: usize, aspect: f64, aa: usize) -> Region {
    let width = frame_width(height, aspect);
    let half_w = v.hw * width as f64 / height as f64;
    Region { re: (v.cr - half_w, v.cr + half_w), im: (v.ci - v.hw, v.ci + v.hw), width: width * aa }
}

/// Averages each `aa` x `aa` block of packed RGB into one pixel, in linear light (so
/// that small bright stars keep their brightness instead of turning grey).
pub fn downsample(rgb: &[u8], w: usize, h: usize, aa: usize) -> Vec<u8> {
    if aa == 1 {
        return rgb.to_vec();
    }
    let (lin, back) = light_tables();
    let (sw, scale) = (w * aa, BACK as f32 / (aa * aa) as f32);
    let mut out = vec![0u8; w * h * 3];
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let rows_per = h.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (chunk, part) in out.chunks_mut(rows_per * w * 3).enumerate() {
            let (lin, back) = (&lin, &back);
            s.spawn(move || {
                for (r, row_out) in part.chunks_mut(w * 3).enumerate() {
                    let y = chunk * rows_per + r;
                    for x in 0..w {
                        let mut acc = [0f32; 3];
                        for dy in 0..aa {
                            let row = (y * aa + dy) * sw;
                            for dx in 0..aa {
                                let i = 3 * (row + x * aa + dx);
                                for c in 0..3 {
                                    acc[c] += lin[rgb[i + c] as usize];
                                }
                            }
                        }
                        for c in 0..3 {
                            row_out[3 * x + c] = back[((acc[c] * scale + 0.5) as usize).min(BACK)];
                        }
                    }
                }
            });
        }
    });
    out
}

/// Converts packed RGB to I420 (BT.709, limited range), the encoder's input format.
pub fn rgb_to_i420(rgb: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 3 / 2];
    let (y_plane, uv) = out.split_at_mut(w * h);
    let (u_plane, v_plane) = uv.split_at_mut(w * h / 4);
    let px = |x: usize, y: usize| {
        let i = 3 * (y * w + x);
        (rgb[i] as f32, rgb[i + 1] as f32, rgb[i + 2] as f32)
    };
    // BT.709: Y = 0.2126 R + 0.7152 G + 0.0722 B, scaled into 16..235 (chroma 16..240)
    for y in 0..h {
        for x in 0..w {
            let (r, g, b) = px(x, y);
            y_plane[y * w + x] = (16.0 + (0.2126 * r + 0.7152 * g + 0.0722 * b) * (219.0 / 255.0)).round() as u8;
        }
    }
    for y in 0..h / 2 {
        for x in 0..w / 2 {
            let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (pr, pg, pb) = px(2 * x + dx, 2 * y + dy);
                (r, g, b) = (r + pr / 4.0, g + pg / 4.0, b + pb / 4.0);
            }
            let cb = -0.1146 * r - 0.3854 * g + 0.5 * b;
            let cr = 0.5 * r - 0.4542 * g - 0.0458 * b;
            u_plane[y * (w / 2) + x] = (128.0 + cb * (224.0 / 255.0)).round().clamp(0.0, 255.0) as u8;
            v_plane[y * (w / 2) + x] = (128.0 + cr * (224.0 / 255.0)).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// The NAL unit without its Annex B start code (00 00 01 or 00 00 00 01).
fn strip_start_code(nal: &[u8]) -> &[u8] {
    let zeros = nal.iter().take_while(|&&b| b == 0).count();
    if zeros >= 2 && nal.get(zeros) == Some(&1) {
        &nal[zeros + 1..]
    } else {
        nal
    }
}

/// Writes frames to an .mp4 file. The file is created at the first frame, once the
/// encoder has produced the stream's parameter sets (SPS/PPS), which the file's
/// header needs. Call `finish` at the end, or the file won't play.
pub struct Mp4Out {
    encoder: Encoder,
    file: Option<mp4::Mp4Writer<BufWriter<File>>>,
    path: std::path::PathBuf,
    width: usize,
    height: usize,
    /// track ticks per frame (track timescale 90 000)
    tick: u32,
    time: u64,
    /// ticks owed by frames the encoder skipped, added to the next frame
    owed: u32,
    pub frames: usize,
    pub bytes: usize,
}

const TIMESCALE: u32 = 90_000;

/// The encoder's settings (see Mp4Out::new).
pub fn encoder_config(fps: u32, qp: u8) -> EncoderConfig {
    EncoderConfig::new()
        .rate_control_mode(RateControlMode::Quality)
        .qp(QpRange::new(qp, qp))
        // only a ceiling here: the fixed quantiser decides the size
        .bitrate(BitRate::from_bps(100_000_000))
        .max_frame_rate(FrameRate::from_hz(fps as f32))
        .skip_frames(false)
        .usage_type(UsageType::CameraVideoRealTime) // OpenH264 rejects the non-real-time types
        .complexity(Complexity::High)
        // Both are for camera video. Background detection copies blocks it takes for
        // static background from the previous frame: in a zoom into faint, regular rows
        // of stars it left 16 x 16 blocks of stars in the wrong place until the next
        // keyframe. Adaptive quantisation would vary the quality within a frame.
        .background_detection(false)
        .adaptive_quantization(false)
        .intra_frame_period(IntraFramePeriod::from_num_frames(2 * fps)) // a keyframe every 2 s
        .vui(VuiConfig::bt709())
}

impl Mp4Out {
    /// Encodes at a constant quantiser `qp` (0 = best, 51 = worst): every frame gets the
    /// same quality, and the file is as big as that needs. Star fields are full of tiny
    /// details, so a fixed bit rate either blurs them badly or wastes space on easy frames.
    pub fn new(path: &Path, width: usize, height: usize, fps: u32, qp: u8) -> Result<Mp4Out, String> {
        Mp4Out::with_config(path, width, height, fps, encoder_config(fps, qp))
    }

    pub fn with_config(path: &Path, width: usize, height: usize, fps: u32, config: EncoderConfig) -> Result<Mp4Out, String> {
        let encoder = Encoder::with_api_config(OpenH264API::from_source(), config)
            .map_err(|e| format!("could not start the video encoder: {e}"))?;
        Ok(Mp4Out {
            encoder,
            file: None,
            path: path.to_path_buf(),
            width,
            height,
            tick: TIMESCALE / fps,
            time: 0,
            owed: 0,
            frames: 0,
            bytes: 0,
        })
    }

    /// Encodes one frame of packed RGB (width * height * 3 bytes).
    pub fn push(&mut self, rgb: &[u8]) -> Result<(), String> {
        let (w, h) = (self.width, self.height);
        assert_eq!(rgb.len(), w * h * 3);
        let yuv = YUVBuffer::from_vec(rgb_to_i420(rgb, w, h), w, h);
        let stream = self.encoder.encode(&yuv).map_err(|e| format!("encoding failed: {e}"))?;

        // .mp4 stores NAL units with 4-byte lengths instead of start codes, and keeps
        // the parameter sets in the header rather than in the stream
        let (mut sample, mut sps, mut pps, mut key) = (Vec::new(), None, None, false);
        for l in 0..stream.num_layers() {
            let layer = stream.layer(l).unwrap();
            for n in 0..layer.nal_count() {
                let nal = strip_start_code(layer.nal_unit(n).unwrap());
                match nal.first().map(|b| b & 0x1f) {
                    None => {}
                    Some(7) => sps = Some(nal.to_vec()),
                    Some(8) => pps = Some(nal.to_vec()),
                    Some(t) => {
                        key |= t == 5; // IDR slice
                        sample.extend_from_slice(&(nal.len() as u32).to_be_bytes());
                        sample.extend_from_slice(nal);
                    }
                }
            }
        }

        if self.file.is_none() {
            let (Some(sps), Some(pps)) = (sps, pps) else {
                return Err("the encoder's first frame had no SPS/PPS".into());
            };
            let file = File::create(&self.path).map_err(|e| format!("could not create {}: {e}", self.path.display()))?;
            let brand = |s: &str| s.parse().unwrap();
            let config = mp4::Mp4Config {
                major_brand: brand("isom"),
                minor_version: 512,
                compatible_brands: vec![brand("isom"), brand("iso2"), brand("avc1"), brand("mp41")],
                timescale: 1000,
            };
            let mut writer = mp4::Mp4Writer::write_start(BufWriter::new(file), &config).map_err(|e| e.to_string())?;
            writer
                .add_track(&mp4::TrackConfig {
                    track_type: mp4::TrackType::Video,
                    timescale: TIMESCALE,
                    language: "und".into(),
                    media_conf: mp4::MediaConfig::AvcConfig(mp4::AvcConfig {
                        width: w as u16,
                        height: h as u16,
                        seq_param_set: sps,
                        pic_param_set: pps,
                    }),
                })
                .map_err(|e| e.to_string())?;
            self.file = Some(writer);
        }

        if sample.is_empty() {
            self.owed += self.tick; // skipped frame: the previous picture stays up longer
        } else {
            let duration = self.tick + std::mem::take(&mut self.owed);
            self.bytes += sample.len();
            let s = mp4::Mp4Sample { start_time: self.time, duration, rendering_offset: 0, is_sync: key, bytes: mp4::Bytes::from(sample) };
            self.file.as_mut().unwrap().write_sample(1, &s).map_err(|e| format!("writing the video failed: {e}"))?;
            self.time += duration as u64;
        }
        self.frames += 1;
        Ok(())
    }

    /// Writes the file's index. Without it the file won't play.
    pub fn finish(mut self) -> Result<(), String> {
        if let Some(mut w) = self.file.take() {
            w.write_end().map_err(|e| format!("finishing the video failed: {e}"))?;
            use std::io::Write;
            w.into_writer().flush().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colour::colourise;

    const HOME: View = View { cr: 0.0, ci: 0.0, hw: 2.0 };

    #[test]
    fn path_ends_and_speed() {
        let end = View { cr: 0.3, ci: 0.4, hw: 0.002 };
        let p = ZoomPath::new(vec![HOME, end], 1.05);
        let n = p.frames();
        assert_eq!(n, ((1000f64).ln() / 1.05f64.ln()).ceil() as usize + 1);
        assert_eq!(p.view(0), HOME);
        let e = p.view(n - 1);
        assert!((e.cr - 0.3).abs() < 1e-15 && (e.ci - 0.4).abs() < 1e-15 && (e.hw - 0.002).abs() < 1e-15);
        // constant zoom rate (slightly below 1.05, so that a whole number of frames lands exactly on the end)
        let r = p.view(10).hw / p.view(11).hw;
        assert!(r > 1.04 && r <= 1.05, "{r}");
        assert!((p.view(100).hw / p.view(101).hw - r).abs() < 1e-12);
        // the target stays inside every frame
        for k in 0..n {
            let v = p.view(k);
            assert!((v.cr - 0.3).abs() <= v.hw && (v.ci - 0.4).abs() <= v.hw, "frame {k}");
        }
        // zooming out runs the same path backwards
        let back = ZoomPath::new(vec![end, HOME], 1.05);
        assert_eq!(back.frames(), n);
        let (a, b) = (back.view(5), p.view(n - 1 - 5));
        assert!((a.hw / b.hw - 1.0).abs() < 1e-12 && (a.cr - b.cr).abs() < 1e-12);
    }

    #[test]
    fn waypoints() {
        let mid = View { cr: 0.5, ci: 0.5, hw: 0.1 };
        let pan = View { cr: 0.7, ci: 0.5, hw: 0.1 };
        let end = View { cr: 0.71, ci: 0.52, hw: 0.001 };
        let p = ZoomPath::new(vec![HOME, mid, pan, end], 1.02);
        // every waypoint is shown exactly, at its frame
        for (i, s) in p.stops.iter().enumerate() {
            let v = p.view(p.stop_frame(i));
            assert!((v.cr - s.cr).abs() < 1e-12 && (v.ci - s.ci).abs() < 1e-12 && (v.hw / s.hw - 1.0).abs() < 1e-12, "stop {i}");
        }
        assert_eq!(p.stop_frame(3), p.frames() - 1);
        // the pan moves the centre steadily: one frame height at 1% of it per frame
        assert!(p.steps(1).abs_diff(100) <= 1, "{}", p.steps(1));
        let k = p.stop_frame(1);
        let d1 = p.view(k + 1).cr - p.view(k).cr;
        let d2 = p.view(k + 50).cr - p.view(k + 49).cr;
        assert!((d1 - d2).abs() < 1e-12 && p.view(k + 1).hw == 0.1, "{d1} {d2}");
        // frames are numbered continuously across segments
        let (i, t) = p.locate(p.stop_frame(2));
        assert_eq!((i, t), (2, 0.0));
        assert_eq!(p.locate(p.frames() - 1), (2, 1.0));
    }

    #[test]
    fn frame_sizes() {
        assert_eq!(frame_width(1080, 16.0 / 9.0), 1920);
        assert_eq!(frame_width(720, 1.0), 720);
        let r = frame_region(View { cr: 1.0, ci: 0.5, hw: 0.25 }, 1080, 16.0 / 9.0, 1);
        assert_eq!((r.width, r.height()), (1920, 1080));
        assert!((r.im.1 - r.im.0 - 0.5).abs() < 1e-15);
        let r = frame_region(View { cr: 1.0, ci: 0.5, hw: 0.25 }, 360, 16.0 / 9.0, 3);
        assert_eq!((r.width, r.height()), (1920, 1080));
    }

    /// A segment that zooms and pans crosses the screen at its pan speed, steadily.
    #[test]
    fn pan_speed() {
        let a = View { cr: 0.0, ci: 0.0, hw: 0.1 };
        let b = View { cr: 1.0, ci: 0.0, hw: 0.05 };
        for speed in [0.01, 0.03] {
            let mut p = ZoomPath::new(vec![a, b], 1.02);
            p.pan[0] = speed;
            let n = p.frames();
            let screen = |k: usize| (p.view(k + 1).cr - p.view(k).cr) / (2.0 * p.view(k).hw);
            for k in [0, n / 2, n - 2] {
                // (a whole number of frames makes it a touch slower; the step is measured
                // in the earlier frame's height, so the zoom shrinks it slightly too)
                assert!(screen(k) <= speed * 1.001 && screen(k) > speed * 0.97, "{speed} frame {k}: {}", screen(k));
            }
        }
        // a slow pan sets the length; zooming fast with no pan doesn't depend on it
        let mut p = ZoomPath::new(vec![a, View { hw: 0.001, ..a }], 1.05);
        let n = p.frames();
        p.pan[0] = 1e-4;
        assert_eq!(p.frames(), n);
    }

    #[test]
    fn smooth_curves() {
        let xs = [0.0, 10.0, 30.0, 35.0];
        let ys = [1.0, 2.0, 2.0, 5.0];
        // through the points, and never outside each pair's range
        for (x, y) in xs.iter().zip(&ys) {
            assert!((smooth(&xs, &ys, *x) - y).abs() < 1e-12);
        }
        let mut last = 0.0;
        for i in 0..=350 {
            let v = smooth(&xs, &ys, i as f64 / 10.0);
            assert!(v >= last - 1e-12 && v <= 5.0, "{i} {v}"); // rises steadily (these points never fall)
            if (10..=30).contains(&(i / 10)) && i <= 300 {
                assert!((v - 2.0).abs() < 1e-12); // flat between equal values
            }
            last = v;
        }
        // straight through points on a line, and no kink at a point
        let line = [3.0, 5.0, 9.0, 10.0];
        assert!((smooth(&xs, &line, 20.0) - 7.0).abs() < 1e-12);
        let ys = [0.0, 1.0, 0.0, 2.0];
        let slope = |x: f64| (smooth(&xs, &ys, x + 1e-6) - smooth(&xs, &ys, x - 1e-6)) / 2e-6;
        assert!((slope(30.0 - 1e-3) - slope(30.0 + 1e-3)).abs() < 1e-2);
        assert_eq!(smooth(&[0.0, 1.0], &[2.0, 4.0], 0.25), 2.5);
        assert_eq!(smooth(&[0.0], &[2.0], 7.0), 2.0);
    }

    #[test]
    fn max_coefficients() {
        let p = ZoomPath::new(vec![HOME, View { hw: 0.02, ..HOME }, View { hw: 0.0002, ..HOME }], 1.05);
        let maxes = [4, 16, 64];
        let ms: Vec<f64> = (0..p.frames()).map(|k| max_at(&p, &maxes, k)).collect();
        assert_eq!((ms[0], ms[p.stop_frame(1)], ms[p.frames() - 1]), (4.0, 16.0, 64.0));
        // the same factor every frame (the points lie on a line on the log scale)
        let r = ms[1] / ms[0];
        assert!(ms.windows(2).all(|w| (w[1] / w[0] - r).abs() < 1e-9), "{ms:?}");

        assert_eq!(fade_between(CoefSet::Ints, 6.0), (6, 7, 0.0));
        let (lo, up, t) = fade_between(CoefSet::Ints, 6.5);
        assert!((lo, up) == (6, 7) && (t - (6.5f64 / 6.0).ln() / (7.0f64 / 6.0).ln()).abs() < 1e-12);
        // primes: 9 lies between the members 7 and 11
        let (lo, up, t) = fade_between(CoefSet::Primes, 9.0);
        assert!((lo, up) == (9, 11) && (t - (9.0f64 / 7.0).ln() / (11.0f64 / 7.0).ln()).abs() < 1e-12);
        assert_eq!(fade_between(CoefSet::Powers2, 1000.0).1, 1024);
        assert_eq!(fade_between(CoefSet::Primes, 1.0).1, 2);
    }

    /// On a segment keeping the flower size, the max coefficient grows exactly in step
    /// with the zoom, even next to an ordinary segment.
    #[test]
    fn flower_keeps_its_size() {
        let beta = View { cr: 1.0 / 3.0, ci: 2f64.sqrt() / 3.0, hw: 0.02 };
        let mut p = ZoomPath::new(vec![HOME, beta, View { hw: 0.0002, ..beta }], 1.05);
        p.flower[1] = true;
        let maxes = [4, 40, keep_flower(40, p.stops[1], p.stops[2])];
        assert_eq!(maxes[2], 4000);
        for k in p.stop_frame(1)..p.frames() {
            // max coefficient x half-width: the flower's size on screen
            let size = max_at(&p, &maxes, k) * p.view(k).hw;
            assert!((size / (40.0 * 0.02) - 1.0).abs() < 1e-9, "frame {k}: {size}");
        }
    }

    #[test]
    fn finds_quadratic_numbers() {
        // (1 + i√2)/3, a root of 3z² - 2z + 1, from a point near it
        let (abc, re, im) = simplest_quadratic(0.335, 0.47, 0.01, 100_000).unwrap();
        assert_eq!(abc, [3, -2, 1]);
        assert!((re - 1.0 / 3.0).abs() < 1e-15 && (im - 2f64.sqrt() / 3.0).abs() < 1e-15);
        // i below the real axis gives -i; a wide search finds the simplest, not the nearest
        assert_eq!(simplest_quadratic(0.01, -0.98, 0.05, 1000).unwrap().0, [1, 0, 1]);
        assert_eq!(simplest_quadratic(0.45, 0.8, 0.1, 1000).unwrap().0, [1, -1, 1]);
        // (1 + i)/2 (height 2) is nearer to (0.3, 0.4) than (1 + i√7)/4 (also height 2)
        assert_eq!(simplest_quadratic(0.3, 0.4, 0.5, 1000).unwrap().0, [2, -2, 1]);
        // a deep view finds a complicated one, exactly inside it
        let (abc, re, im) = simplest_quadratic(0.123456, 0.654321, 1e-6, 1_000_000).unwrap();
        assert!((re - 0.123456).hypot(im - 0.654321) <= 1e-6 && abc[0] > 100, "{abc:?}");
        assert!((abc[0] as f64 * re * re - abc[0] as f64 * im * im + abc[1] as f64 * re + abc[2] as f64).abs() < 1e-6);
        assert_eq!(simplest_quadratic(0.5, 0.3, 0.01, 1), None);
    }

    #[test]
    fn cross_fade() {
        use crate::engine::{render, Coefs, Plan};
        let region = frame_region(View { cr: 0.3, ci: 0.4, hw: 0.2 }, 32, 1.0, 1);
        let r = |m: i32, t: f64| {
            let plan = Plan::new(3, &Coefs::ints(m), false).unwrap();
            let r = render(&plan, &region, 2, &Default::default(), &Default::default()).unwrap();
            let st = Stats::new(&r);
            (r, st, t)
        };
        let s = Style::default();
        let both = |t: f64| [r(3, 1.0 - t), r(4, t)];
        let [a, b] = both(0.0);
        let (ca, cb) = (colourise(&a.0, &a.1, &s), colourise(&b.0, &b.1, &s));
        assert_eq!(colour_frame(&both(0.0), &s, None), ca);
        assert_eq!(colour_frame(&both(1.0), &s, None), cb);
        let mid = colour_frame(&both(0.5), &s, None);
        assert!(mid.iter().zip(ca.iter().zip(&cb)).all(|(&m, (&x, &y))| m >= x.min(y) && m <= x.max(y)));
        // on a's scale, a's frame is unchanged
        assert_eq!(colour_frame(&both(0.0), &s, Some(&a.1.scale(&s))), ca);
    }

    #[test]
    fn held_scale_moves_between_the_ends() {
        use crate::engine::{render, Coefs, Plan};
        let p = ZoomPath::new(vec![HOME, View { hw: 0.02, ..HOME }], 1.05);
        let levels = |m: i32, v: View| {
            let plan = Plan::new(3, &Coefs::ints(m), false).unwrap();
            let r = render(&plan, &frame_region(v, 32, 1.0, 1), 2, &Default::default(), &Default::default()).unwrap();
            vec![(Stats::new(&r).levels(), 1.0)]
        };
        let all = [levels(3, p.stops[0]), levels(12, p.stops[1])];
        let style = Style::default();
        let last = p.frames() - 1;
        let (s0, want) = (all[0][0].0.scale(&style), all[1][0].0.scale(&style));
        // the residual brightness moves from one end's scale to the other's
        assert_eq!(held_scale(&p, &style, &all, 0).ln_hi, s0.ln_hi);
        assert!((held_scale(&p, &style, &all, last).ln_hi - want.ln_hi).abs() < 1e-9);
        let mid = held_scale(&p, &style, &all, last / 2);
        assert!(mid.ln_hi > s0.ln_hi.min(want.ln_hi) && mid.ln_hi < s0.ln_hi.max(want.ln_hi));
        // the lengths and heights are on one range throughout: the start's lowest to the deepest's highest
        let range = (s0.height.0.min(want.height.0), s0.height.1.max(want.height.1));
        assert_eq!(range.1, 12.0);
        for k in [0, last / 2, last] {
            let s = held_scale(&p, &style, &all, k);
            assert_eq!(s.height, range);
            assert_eq!(s.length, (s0.length.0.min(want.length.0), s0.length.1.max(want.length.1)));
        }
    }

    #[test]
    fn downsampling_keeps_brightness() {
        // one white pixel in a 2x2 block: a quarter of the light, not a quarter of the value
        let rgb = [255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let d = downsample(&rgb, 1, 1, 2);
        assert!((d[0] as i32 - ((0.25f32).powf(1.0 / 2.2) * 255.0).round() as i32).abs() <= 1, "{}", d[0]);
        // and a bigger image, split across threads, averages each block separately
        let (w, h) = (37, 23);
        let big: Vec<u8> = (0..w * 3 * h * 3).flat_map(|i| { let v = ((i / (w * 3)) / 3 * 11 % 256) as u8; [v, v, v] }).collect();
        let d = downsample(&big, w, h, 3);
        for y in 0..h {
            assert_eq!(d[3 * y * w], (y * 11 % 256) as u8, "row {y}");
        }
        assert_eq!(downsample(&[9, 8, 7], 1, 1, 1), [9, 8, 7]);
    }

    /// Encodes a few frames and reads the file back.
    #[test]
    fn writes_playable_mp4() {
        for (w, h) in [(320, 180), (1920, 1080), (3840, 2160)] {
            let n = 12;
            let path = std::env::temp_dir().join(format!("algfrac_video_test_{w}.mp4"));
            let mut out = Mp4Out::new(&path, w, h, 30, 20).unwrap();
            for f in 0..n {
                let rgb: Vec<u8> = (0..w * h).flat_map(|i| [((i % w + f * 7) % 256) as u8, (i / w % 256) as u8, 128]).collect();
                out.push(&rgb).unwrap();
            }
            out.finish().unwrap();
            let file = File::open(&path).unwrap();
            let size = file.metadata().unwrap().len();
            let mut r = mp4::Mp4Reader::read_header(std::io::BufReader::new(file), size).unwrap();
            let track = r.tracks().get(&1).unwrap();
            assert_eq!((track.width(), track.height()), (w as u16, h as u16));
            assert_eq!(track.sample_count(), n as u32);
            assert!((r.duration().as_secs_f64() - n as f64 / 30.0).abs() < 0.01, "{:?}", r.duration());
            assert!(r.read_sample(1, 1).unwrap().unwrap().is_sync);
            let _ = std::fs::remove_file(&path);
        }
    }
}
