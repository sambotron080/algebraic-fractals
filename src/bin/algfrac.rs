//! algfrac command-line tool: render one region to a PNG (and a raw .algf data
//! file). The search itself lives in engine.rs; see there for how it works.

use algfrac::colour::{colourise, ColourBy, Colormap, Stats, Style};
use algfrac::engine::{render, CoefSet, Coefs, Plan, Region};
use algfrac::png::write_png;
use std::env;
use std::process;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const USAGE: &str = "\
algfrac - map how close each point of the complex plane is to an algebraic number

USAGE:
  algfrac [options]

REGION (pick one):
  --centre RE,IM        centre of the view            (with --halfwidth)
  --halfwidth H         half-width of the view        (default 1)
  --re MIN,MAX          real range                    (default -2,2)
  --im MIN,MAX          imaginary range               (default -2,2)

SEARCH:
  --order N             highest power of the polynomial (default 3)
  --maxcoef M           coefficients in [-M, M]         (default 8)
  --nozero              don't allow 0 as a coefficient (every power is used)
  --zeroroot            allow c0 = 0, so that X = 0 counts as a root
  --coefs SET           which coefficient sizes are allowed, each with either sign
                        (default ints):
                          ints        1, 2, 3, 4, ...      primes  1, 2, 3, 5, 7, 11, ...
                          fib         1, 2, 3, 5, 8, ...   odd     1, 3, 5, 7, ...
                          triangular  1, 3, 6, 10, ...     squares 1, 4, 9, 16, ...
                          cubes       1, 8, 27, 64, ...    pow2    1, 2, 4, 8, 16, ...
  --legacy              conjugate quadratic variant: order 2, the linear term uses
                        conj(X), and c1 = 0 is excluded instead of c0 = 0

IMAGE:
  --width W             image width in pixels; height follows the aspect ratio (default 512)
  --colour pow|log|len|hgt
                        1/residual^n, -log(residual), polynomial length (sum |ci|)
                        or height (max |ci|)                          (default log)
  --n N                 exponent for --colour pow, and for shading     (default 1)
  --max P               brightness percentile mapped to full brightness (default 99.5)
                        (--clip P is the same)
  --min P               brightness percentile mapped to black          (default 0)
  --colormap NAME       bone gray hot copper pink spring summer autumn winter cool jet
                        parula inferno magma plasma viridis turbo hsv
                        sinebow neon acid candy cosmic                 (default bone)
  --invert              reverse the colormap
  --noshade             don't darken len/hgt colours by 1/residual^n
  --logpoly             spread len/hgt over the colormap on a log scale
  --sharp               take each pixel's brightness at its centre, instead of
                        averaged over the pixel (smooth stars, the default)
  --gamma G             x -> x^(1/G) before the colormap; >1 brightens midtones (default 1)
  --cycles C            run through the colormap C times               (default 1)
  --shift S             shift along the colormap, as a fraction of it  (default 0)
  --brightness B        added to the final colours, -1..1              (default 0)
  --contrast K          scales the final colours around mid-grey       (default 1)
  --saturation S        0 = greyscale, >1 = more vivid                 (default 1)

OUTPUT:
  --out NAME            writes NAME.png and NAME.algf (default algfrac_out)
  --nodata              skip the .algf data file (raw residuals and coefficients)
  --threads T           worker threads (default: all cores)

EXAMPLE:
  algfrac --order 4 --maxcoef 32 --centre 1.9,0.19 --halfwidth 0.16 --width 1024 --out spiral
";

struct Config {
    order: usize,
    coefs: Coefs,
    region: Region,
    threads: usize,
    out: String,
    legacy: bool,
    data: bool,
    style: Style,
}

fn num<T: FromStr>(s: &str) -> Result<T, String> {
    s.trim().parse().map_err(|_| format!("cannot parse number '{s}'"))
}

fn pair(s: &str) -> Result<(f64, f64), String> {
    let mut it = s.split(',');
    match (it.next(), it.next(), it.next()) {
        (Some(a), Some(b), None) => Ok((num(a)?, num(b)?)),
        _ => Err(format!("expected two comma-separated numbers, got '{s}'")),
    }
}

/// Parses the command line. Err("") means "print usage and exit cleanly".
fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut cfg = Config {
        order: 3,
        coefs: Coefs::ints(8),
        region: Region { re: (-2.0, 2.0), im: (-2.0, 2.0), width: 512 },
        threads: thread::available_parallelism().map_or(4, |n| n.get()),
        out: "algfrac_out".to_string(),
        legacy: false,
        data: true,
        style: Style { by: ColourBy::Log, ..Style::default() },
    };
    let mut centre = None;
    let mut half = None;

    let mut i = 0;
    while i < args.len() {
        let key = args[i].trim_start_matches('-').to_lowercase();
        // flags without a value
        let flag = match key.as_str() {
            "help" | "h" | "?" => return Err(String::new()),
            "legacy" => Some(cfg.legacy = true),
            "nopng" => Some(()), // accepted for compatibility; the PNG is the point now
            "nodata" => Some(cfg.data = false),
            "invert" => Some(cfg.style.invert = true),
            "noshade" => Some(cfg.style.shade = false),
            "logpoly" => Some(cfg.style.log_poly = true),
            "sharp" => Some(cfg.style.smooth = false),
            "nozero" => Some(cfg.coefs.zero = false),
            "zeroroot" => Some(cfg.coefs.zero_root = true),
            _ => None,
        };
        if flag.is_some() {
            i += 1;
            continue;
        }
        let val = args.get(i + 1).ok_or(format!("missing value for --{key}"))?;
        match key.as_str() {
            "order" => cfg.order = num(val)?,
            "maxcoef" => cfg.coefs.max = num(val)?,
            "coefs" => {
                let v = val.to_lowercase();
                cfg.coefs.set = match v.as_str() {
                    "fib" => CoefSet::Fibonacci,
                    "tri" => CoefSet::Triangular,
                    _ => CoefSet::ALL.into_iter().find(|s| s.name() == v).ok_or(format!(
                        "--coefs must be ints, primes, fib, odd, triangular, squares, cubes or pow2, got '{val}'"
                    ))?,
                }
            }
            "centre" | "center" => centre = Some(pair(val)?),
            "halfwidth" => half = Some(num::<f64>(val)?),
            "re" => cfg.region.re = pair(val)?,
            "im" => cfg.region.im = pair(val)?,
            "width" => cfg.region.width = num(val)?,
            "threads" => cfg.threads = num(val)?,
            "out" => cfg.out = val.clone(),
            "colour" | "color" => {
                cfg.style.by = ColourBy::ALL
                    .into_iter()
                    .find(|b| b.short() == val.to_lowercase())
                    .ok_or(format!("--colour must be pow, log, len or hgt, got '{val}'"))?
            }
            "n" => cfg.style.n = num(val)?,
            "clip" | "max" => cfg.style.high = num(val)?,
            "min" => cfg.style.low = num(val)?,
            "gamma" => cfg.style.gamma = num(val)?,
            "cycles" => cfg.style.cycles = num(val)?,
            "shift" => cfg.style.shift = num(val)?,
            "brightness" => cfg.style.brightness = num(val)?,
            "contrast" => cfg.style.contrast = num(val)?,
            "saturation" => cfg.style.saturation = num(val)?,
            "colormap" | "colourmap" => {
                cfg.style.cmap = Colormap::ALL
                    .into_iter()
                    .find(|c| c.name() == val.to_lowercase())
                    .ok_or(format!("unknown colormap '{val}'"))?
            }
            _ => return Err(format!("unknown option --{key}")),
        }
        i += 2;
    }

    match (centre, half) {
        (Some((cr, ci)), h) => {
            cfg.region = Region::square(cr, ci, h.unwrap_or(1.0), cfg.region.width);
        }
        (None, Some(_)) => return Err("--halfwidth needs --centre".into()),
        (None, None) => {}
    }

    let r = &cfg.region;
    if r.width < 2 {
        return Err("--width must be at least 2".into());
    }
    if !(r.re.1 > r.re.0 && r.im.1 > r.im.0) {
        return Err("region ranges must be increasing (MIN,MAX)".into());
    }
    if !(cfg.style.n > 0.0) {
        return Err("--n must be positive".into());
    }
    cfg.threads = cfg.threads.max(1);
    Ok(cfg)
}

fn main() {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(e) if e.is_empty() => {
            println!("{USAGE}");
            return;
        }
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            process::exit(2);
        }
    };
    let plan = Plan::new(cfg.order, &cfg.coefs, cfg.legacy).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        process::exit(2);
    });

    let reg = cfg.region;
    let (w, h) = (reg.width, reg.height());
    let set = cfg.coefs.describe();
    eprintln!(
        "algfrac: order {}, coefficients {}{}",
        cfg.order,
        set,
        if cfg.legacy { " (legacy quadratic)" } else { "" }
    );
    eprintln!(
        "  region re [{}, {}]  im [{}, {}]  ->  {} x {} px on {} threads",
        reg.re.0, reg.re.1, reg.im.0, reg.im.1, w, h, cfg.threads
    );
    let chunks = match plan.chunks() {
        1 => String::new(),
        n => format!(", in {n} chunks of {} entries", plan.table_len()),
    };
    eprintln!(
        "  per pixel: ~{:.0} steps (direct search: table {} + ~{:.0} queries{chunks}), vs {:.3e} polynomials brute force",
        plan.steps(),
        plan.table_len() * plan.chunks(),
        plan.query_len() * plan.chunks() as f64,
        plan.brute_len()
    );
    if !cfg.legacy {
        let dens = algfrac::density::Predictor::new().per_pixel(cfg.order, &cfg.coefs, &reg);
        eprintln!("  predicted: ~{:.3} algebraic numbers per 100 px^2", dens * 100.0);
    }
    // per thread: raw + table entries + grid, for the tables actually built
    let table_mb = |len: usize| len as f64 * 56.0 / 1e6 * cfg.threads as f64;
    let image_mb = (w * h * (8 + 4 * plan.nterms)) as f64 / 1e6;
    match plan.lattice_table_len() {
        Some(len) => eprintln!(
            "  memory: ~{:.0} MB working set (up to {:.0} MB near the real axis)",
            table_mb(len) + image_mb,
            table_mb(plan.table_len()) + image_mb
        ),
        None => eprintln!("  memory: ~{:.0} MB working set", table_mb(plan.table_len()) + image_mb),
    }

    // render on worker threads while this thread reports progress
    let cancel = AtomicBool::new(false);
    let rows_done = AtomicUsize::new(0);
    let t0 = Instant::now();
    let result = thread::scope(|s| {
        let job = s.spawn(|| render(&plan, &reg, cfg.threads, &cancel, &rows_done));
        let mut last = Instant::now();
        while !job.is_finished() {
            thread::sleep(Duration::from_millis(100));
            let done = rows_done.load(Ordering::Relaxed);
            if last.elapsed() >= Duration::from_secs(2) && done > 0 && done < h {
                let el = t0.elapsed().as_secs_f64();
                eprintln!(
                    "  {:5.1}%  {:.0}s elapsed, ~{:.0}s left",
                    100.0 * done as f64 / h as f64,
                    el,
                    el * (h - done) as f64 / done as f64
                );
                last = Instant::now();
            }
        }
        job.join().expect("render thread panicked")
    });
    let r = result.expect("render was not cancelled");
    let el = t0.elapsed().as_secs_f64();
    eprintln!("  done in {:.2}s ({:.0} px/s)", el, (w * h) as f64 / el);
    let png_path = format!("{}.png", cfg.out);
    let rgb = colourise(&r, &Stats::new(&r), &cfg.style);
    if let Err(e) = write_png(&png_path, r.width, r.height, &rgb) {
        eprintln!("error writing {png_path}: {e}");
        process::exit(1);
    }
    eprintln!("  wrote {png_path}");
    if cfg.data {
        let data_path = format!("{}.algf", cfg.out);
        if let Err(e) = algfrac::algf::write(&data_path, &r, cfg.order, &cfg.coefs, cfg.legacy) {
            eprintln!("error writing {data_path}: {e}");
            process::exit(1);
        }
        eprintln!("  wrote {data_path}");
    }
}
