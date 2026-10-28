//! The program's icon: the lotus flower around (−1)^(1/3) = (1 + i√3)/2, a root of
//! x² − x + 1, at order 3 with coefficients up to 60. Rendered at whatever size is
//! needed, for the window (at start) and for assets/icon.ico (examples/make_icon.rs).

use crate::colour::{colourise, ColourBy, Stats, Style};
use crate::engine::{render, Coefs, Plan, Region};

/// The icon, `size` pixels square, as packed RGB.
pub fn lotus(size: usize) -> Vec<u8> {
    let plan = Plan::new(3, &Coefs { zero_root: true, ..Coefs::ints(60) }, false).expect("a small plan");
    let region = Region::square(0.5, 3f64.sqrt() / 2.0, 0.013, size.max(2));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let r = render(&plan, &region, threads, &Default::default(), &Default::default()).expect("not cancelled");
    let style = Style { by: ColourBy::Log, gamma: 1.6, contrast: 1.3, ..Style::default() };
    colourise(&r, &Stats::new(&r), &style)
}
