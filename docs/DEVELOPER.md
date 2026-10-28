# Algebraic Fractals: developer guide

How the program is built, how its source is laid out, how the search works, and how videos are made. The user guide is [USER-GUIDE.md](USER-GUIDE.md); the full specification is [SPEC.md](../SPEC.md).

For every pixel `X` of a region of the complex plane the engine finds

```
residual(X) = min |c0 + c1·X + c2·X² + … + cn·Xⁿ|
```

over integer coefficients `|ci| ≤ M` with `c0 ≠ 0`, together with the coefficients that achieve it. Small residuals mark points close to an algebraic number of degree ≤ n. Several other coefficient sets are available:
- sparser sets, with 0 and ± the members up to `M`: 1 and the primes, Fibonacci numbers, odd numbers, triangular numbers, squares, cubes, or powers of 2;
- no zero coefficients;
- 0 counted as a root (`zero_root`). Polynomials with `c0 = 0`, `x·q(x)`, are never searched: near `q`'s roots inside the unit circle `|X·q(X)| < |q(X)|`, so they'd beat the simpler `q` and pixels would show a spare factor of x. Instead the polynomial `x` (residual `|X|`) is tried at every pixel after the search, and wins ties (`add_root_zero` in engine.rs). Other reducible polynomials can still win, e.g. `(x + 1)·q` near `q`'s roots within 1 of −1.

For why the images look the way they do, see [the README](../README.md#why-the-patterns-look-the-way-they-do).

The package builds two programs from one library (`algfrac`):

| program | what it is |
|---|---|
| `Algebraic Fractals.exe` (Cargo binary `algebraic-fractals`) | the app: the explorer, and the video window |
| `algfrac.exe` | command line: render one region to a PNG (and a raw data file), or render a saved video project ([below](#command-line)) |

---

## Building

From the repository's folder, in PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -File build.ps1
```

This builds both programs into `target\release\`, copies them into **`dist\`** (the app renamed to `Algebraic Fractals.exe`) with `USER-GUIDE.md`, and zips them as `dist\AlgebraicFractals-<version>-windows-x64.zip`. The first build downloads the GUI library (`eframe`) and compiles the OpenH264 video encoder from its C++ source, which takes a minute or two. Later builds take seconds.

**Requirements:**

| tool | why | install |
|---|---|---|
| Rust, GNU toolchain | compiler | `rustup-init.exe -y --default-host x86_64-pc-windows-gnu --profile minimal` from [rustup.rs](https://rustup.rs) |
| MinGW-w64 (WinLibs) | the GUI's Windows dependencies need MinGW's `dlltool` and assembler, and OpenH264 needs its C++ compiler; the minimal Rust GNU toolchain has neither | `winget install BrechtSanders.WinLibs.POSIX.MSVCRT --scope user` |

`build.ps1` adds both to PATH for the build, so they don't need to be on PATH permanently. It looks for Rust in `%USERPROFILE%\.cargo\bin` and for WinLibs in winget's package folder. If you installed them elsewhere, put them on PATH yourself or edit the top of `build.ps1`. To work with `cargo` directly (e.g. `cargo check`), set PATH the same way.

**Portability.** [.cargo/config.toml](../.cargo/config.toml) sets two things, for the Windows GNU target only:
- **CPU target:** it targets `x86-64-v2`, which covers any x86-64 CPU from about 2009 onwards. In testing this was as fast as `target-cpu=native`, and unlike `native` the `.exe`s run on other PCs.
- **C++ runtime:** OpenH264 is C++, and MinGW would normally link it against `libstdc++-6.dll`, which other PCs don't have. `CXXSTDLIB_x86_64_pc_windows_gnu = ""` stops the `cc` crate from linking that DLL, and the link arguments add the static `libstdc++`, `libwinpthread` and `libgcc` instead.

So both programs link only against DLLs that ship with Windows, and each `.exe` is a single self-contained file. (Check with `objdump -p "dist\Algebraic Fractals.exe" | findstr "DLL Name"`.) The release profile uses thin LTO, one codegen unit and `panic = "abort"`.

### macOS

macOS programs have to be built on a Mac: Apple's SDK and linker are needed, and Apple's licence only allows them on Apple hardware. On a Mac with Rust ([rustup.rs](https://rustup.rs)) and Apple's command line tools (`xcode-select --install`), run:

```sh
bash build-mac.sh
```

The script builds for both Apple silicon and Intel (`aarch64-apple-darwin` and `x86_64-apple-darwin`) and joins each program's two builds into one universal binary with `lipo`. It wraps the app as `Algebraic Fractals.app`, signs everything ad hoc with `codesign --sign -`, and zips it all in **`dist-mac/`** with the user guide and [MAC.md](MAC.md). The minimum macOS version is 11. `MACOSX_DEPLOYMENT_TARGET` also applies to the C++ encoder. The Mac build uses the system `libc++`, which every Mac has, so nothing needs linking statically.

Without an Apple Developer ID ($99 a year) the app can't be notarised, so on other Macs Gatekeeper blocks it until the user allows it once. MAC.md explains how.

### Releases on GitHub

A version tag (`v0.3.0`, …) runs [.github/workflows/release.yml](../.github/workflows/release.yml): it runs the tests and builds the Windows zip (on GitHub's Windows machines, with the MSVC toolchain and a statically linked C runtime) and the macOS zip (with `build-mac.sh`), then attaches both to a **draft** release to check and publish by hand. So Mac builds don't need a Mac. [ci.yml](../.github/workflows/ci.yml) runs the tests on Windows, macOS and Linux on every push.

**Tests.** Run `cargo test --release --lib --bins`. (Plain `cargo test` also tries to run the formulas in the doc comments as code.) See [Accuracy](#accuracy) for what they check.

---

## Source layout

| file | contents |
|---|---|
| [src/engine.rs](../src/engine.rs) | the search: `Coefs` (which coefficients), `Plan`, the direct (`Walk`) and lattice (`LatWalk`) searches, the tiled renderer for deep views (`Tiler`), `Region`, `render()`, `render_axes()`, `Render`, tests. No dependencies. |
| [src/colour.rs](../src/colour.rs) | turning a `Render` into RGB: what to plot (`ColourBy`), scaling (`Style`), colormaps, per-render `Stats`, `Levels` and `Scale` |
| [src/density.rs](../src/density.rs) | predicting how many algebraic numbers a view will contain, before rendering it (`Predictor`; see below) |
| [src/common.rs](../src/common.rs) | `View`, saved locations and groups (`locations.tsv`), bringing in the old programs' places (`OldPlace`), number parsing and formatting, the auto order / max coefficient (`choose_auto`), default folders |
| [src/icon.rs](../src/icon.rs) | the icon: the lotus flower around (−1)^(1/3), rendered at any size (`examples/make_icon.rs` writes `assets/icon.ico` and `icon.png`; `build.rs` puts the `.ico` into the `.exe`) |
| [src/settings.rs](../src/settings.rs) | the remembered settings (`settings.tsv`) |
| [src/png.rs](../src/png.rs) | minimal PNG writers: uncompressed with no dependencies (the command line's), and compressed with `miniz_oxide` (the app's) |
| [src/algf.rs](../src/algf.rs) | reading and writing `.algf` render files (see below), written by the command line |
| [src/video/mod.rs](../src/video/mod.rs) | the zoom path through the waypoints (`ZoomPath`), frame regions, the smooth curve through the waypoints' max coefficients (`smooth`, `max_at`), fading, the held colour scale (`held_scale`), smoothing (`downsample`), and the `.mp4` writer (`Mp4Out`) |
| [src/video/frames.rs](../src/video/frames.rs) | `Frames` (everything that decides what the frames show), `Colouring` (one style for the whole video and its colour scale), `auto_maxes`, `Frames::frame_secs` |
| [src/video/reuse.rs](../src/video/reuse.rs) | reusing renders between frames: the ladder of max coefficients, planning the patches, and making frames from them |
| [src/video/project.rs](../src/video/project.rs) | `VideoProject`: every setting of a video, its `.afvideo` file, the waypoint edits, and bringing in the old video maker's `.path` files |
| [src/video/make.rs](../src/video/make.rs) | `make_video`: the render loop, shared by the app and the command line, reporting each frame through a callback |
| [src/lib.rs](../src/lib.rs) | exposes the modules above as the `algfrac` library |
| [src/main.rs](../src/main.rs) | the app: the window, the video window's viewport, quitting, remembered settings |
| [src/app/explore.rs](../src/app/explore.rs) | the explorer: toolbar, settings strip, image, status bar |
| [src/app/popups.rs](../src/app/popups.rs) | the explorer's popups: View, Polynomials, Colours, Save location, Locations, Folders |
| [src/app/video.rs](../src/app/video.rs) | the video window: project bar, tabs, waypoint strip, frame preview, rendering |
| [src/app/widgets.rs](../src/app/widgets.rs) | controls used in several places: coefficient rows, colour controls, folder boxes |
| [src/bin/algfrac.rs](../src/bin/algfrac.rs) | `algfrac.exe`: argument parsing, progress output, `.png` / `.algf` writing, and `algfrac video` |
| [examples/density_check.rs](../examples/density_check.rs) | compares density predictions with exact root counts |
| [tests/data/](../tests/data) | files in the old programs' formats, to check they still load |

The library has no GUI code, so the command line and the tests use it without a window. Only `src/main.rs` and `src/app/` use `eframe`/`egui`.

**Windows and threads.** The video window is a second native window (an egui *immediate viewport*), drawn inside the main window's update. So every repaint is asked of the main window (`app::repaint_now`), including from background threads. Searches run on worker threads, one per core; the explorer, the waypoint strip, the frame preview and a video render each run their own background job, and the explorer keeps working while a video renders.

---

## How the search works

### What is searched

[`Coefs`](../src/engine.rs) describes the allowed coefficients: a set of sizes (`CoefSet`: `Ints`, `Primes` = 1 and the primes, `Fibonacci`, `Odd`, `Triangular`, `Squares`, `Cubes`, `Powers2`; every set contains 1), a `max` on `|c|`, whether 0 is allowed (`zero`), and whether `c0` may be 0 (`zero_root`). `Plan::new` turns that into one sorted list of allowed values per power. `c0 = 0` is excluded unless `zero_root`, because then `p(x) = x·q(x)`, which only adds the root 0 (and lower-order polynomials scaled by `|X|`). With `zero_root`, the zero polynomial itself is skipped explicitly.

### The problem with brute force

Trying every polynomial means `(2M+1)^(n+1)` evaluations per pixel. For order 4, `M = 32` that is 1.16×10⁹, and a 1024 × 1024 image needs 10¹⁵ of them.

### Meet in the middle

Split the polynomial at a power `k`:

```
p(X) = A + B,   A = c0 + c1·X + … + c(k-1)·X^(k-1)      the "table"
                B = ck·X^k + … + cn·Xⁿ                   the "queries"
```

`|p(X)| = |A − (−B)|` is the distance between the points `A` and `−B` in the complex plane. The minimum residual is therefore the **closest pair** between the set of all `A`s and the set of all `−B`s, and closest-pair search doesn't need to try every combination. The direct search, per pixel ([`Solver::solve_direct`](../src/engine.rs)):

1. **Table.** Evaluate every `A` (order 4, `M = 32`: 64 × 65 = 4160 points). Bucket them into a uniform 2-D grid sized so each cell holds about one point (a counting sort, O(n)).
2. **Queries.** For each `B`, scan only the grid cells within the current best distance of `−B`. Once a good bound exists, that is usually a single cell, often an empty one. Queries whose disc misses the table's bounding box are skipped in O(1).
3. **Sign symmetry.** `p` and `−p` have the same residual, so only `B`s whose highest nonzero coefficient is positive are visited. That halves the queries.
4. **Tree pruning.** The queries are enumerated as a tree, fixing the highest coefficient first. All queries in a subtree lie inside a disc of known radius `rad[j]` (the sum of the maximum magnitudes of the unfixed terms). A subtree whose disc can't come within the best distance of the table is skipped whole. When the disc is small compared with the best distance, the table entries within reach are collected once into a candidate list that the whole subtree shares, and the list is narrowed as the walk descends. This matters near `X = 0`, where high powers vanish, all queries bunch together and the best residual stays large. Without it such pixels took 10 s each; with it they take milliseconds.
5. **Warm start.** Each pixel first evaluates its left neighbour's best polynomial. That gives a tight bound before the search begins.
6. **Ties.** Residuals within `64·ε·Σ maxcoef·|X|^i` (the rounding error of evaluating `p`) count as equal, and the polynomial with the smallest `Σ|ci|` wins. Many grids with round-number coordinates contain exact mathematical ties (e.g. at `X = −0.7−0.9i`, `4+4x+3x²` and `9+10x+7x²` both give exactly 0.3). With this rule the choice doesn't depend on rounding noise, and at an exact algebraic number it returns the minimal polynomial rather than a multiple of it.
7. **Sign convention.** The returned coefficients are flipped if necessary so that the highest-power nonzero coefficient is positive.

`X = 0` is special-cased (`p(0) = c0`, so the answer is the smallest allowed `|c0|`, or 0 with `p = x` when `c0 = 0` is allowed).

The split `k` is chosen in [`Plan::new`](../src/engine.rs) to minimise *table size + queries/2*. Work per pixel is therefore about

```
(2M+1)^k + (2M+1)^(n+1−k) / 2      instead of      (2M+1)^(n+1)
```

and memory is just the table (≈ 52 bytes per entry per thread).

A table holds at most `TABLE_MAX` = 2²⁶ entries (about 3.5 GB per thread). At order 3 the balanced split (a table of c₀, c₁) fits up to `M` = 4095. Above that, the table is built in **chunks**: c₀ with a slice of the c₁ values at a time, and all the queries run against each chunk, carrying the best polynomial (or the collected shortlist) from one chunk to the next. The work becomes *(2M+1)² + chunks × (2M+1)²/2*:

| max coefficient | chunks | direct-search work vs one big table |
|---|---|---|
| ≤ 4095 | 1 | the same |
| 4096 | 2 | 1.3× |
| 10 000 | 6 | 2.7× |
| 32 767 | 64 | 22× |

The split and chunk size are chosen together to minimise that cost. Only the direct search uses this table, mainly near the real axis (see below). The lattice search's table is chosen the same way. So any max coefficient up to 1 000 000 is accepted. Measured exactly on the real axis at order 3, a pixel takes about 9 s at max 4095 or 4096 and about 4 minutes at 10 000.

### The lattice search

Two neighbouring terms don't need enumerating at all. Fix every coefficient except `cj` and `c(j+1)`, and let `S` be the sum of the fixed terms. Then

```
cj·X^j + c(j+1)·X^(j+1)  =  cj·e1 + c(j+1)·e2,      e1 = X^j,  e2 = X^(j+1)
```

ranges over a **lattice** in the complex plane (unless `X` is real, `e1` and `e2` point in different directions). The best `(cj, c(j+1))` for that `S` are the lattice points nearest `−S`, and they can be found in O(1) by solving `−S = a·e1 + b·e2` for real `a`, `b` and rounding. The box `|c| ≤ M` and the allowed set are checked on the few lattice points within reach.

So only the other `n − 1` coefficients are searched, again by meet in the middle: table `A` plus query `B`. But now the question for each pair is whether `A + B` is close to *any* lattice point, i.e. whether `A` and `−B` are close **on the torus** `ℂ / lattice`. Table points are bucketed by their position inside the lattice cell (the fractional parts of `a` and `b`), and each query only scans the cells near its own position, wrapping around the edges. Each table entry found there is then checked against the actual lattice points in the box ([`LatWalk`](../src/engine.rs)).

That removes a factor of about `2M+1` from the work:

```
(2M+1)^k + (2M+1)^(n−1−k) / 2      instead of      (2M+1)^k + (2M+1)^(n+1−k) / 2
```

The lattice goes on the two largest terms: powers 0 and 1 for `|X| ≤ 1`, and `n−1` and `n` for `|X| > 1`.

**Where it struggles: near the real axis.** There the lattice is almost flat (its cell area is `|X|^(2j)·|Im X|`), so many table entries look close on the torus but have no lattice point inside the coefficient box. Each search therefore has a work budget equal to the direct search's cost. If it runs out, the pixel is redone with the direct search, and the next 16 pixels skip straight to it. The lattice search also needs a finite starting bound. Each pixel starts from its left neighbour's polynomial, and the first pixel of each row starts from 1 + x + … + xⁿ. That is always the same polynomial, so a view always renders the same way, whichever thread gets which row. `Plan::steps()` reports the typical work per pixel.

### Mirror symmetry

Real coefficients mean `p(conj X) = conj p(X)`, so the residual image is symmetric about the real axis. When a view is exactly symmetric (centre on the real axis), [`render()`](../src/engine.rs) computes only the top half and mirrors it.

### Deep views: one search per tile

In a deep view, neighbouring pixels share their best polynomials. So [`render()`](../src/engine.rs) splits a deep view into square tiles, does one search per tile, and lets every pixel choose from that tile's shortlist.

**Why the shortlist is complete.** Take a tile with centre `X0`, where `h` is the distance to its furthest pixel. Any polynomial changes by at most `L·h` across the tile, where `L = Σ max|ci| · i · (|X0| + h)^(i−1)` (`Plan::slope`). If the best value at `X0` is `r0`:
- every pixel `X` has a polynomial (X0's winner) with `|p(X)| ≤ r0 + L·h`;
- so X's winner `w` has `|w(X0)| ≤ |w(X)| + L·h ≤ r0 + 2·L·h`.

**The tile search.** `Solver::collect` is the ordinary search (lattice or direct) run with a fixed reach: it lists every polynomial with `|p(X0)| ≤ r0 + 2·L·h` (plus the tie tolerance), instead of shrinking the reach to the best one found. Each pixel then evaluates just those polynomials and picks one by the same tie rule. The answers are the same as searching every pixel, apart from exact ties.

**Choosing the tile size.**
- `choose_method` samples the best value at 9 points of the view. It treats the values `p(X)` near 0 as random points, with a density estimated from that typical distance.
- It sizes tiles to hold about 256 polynomials each, capped at 64 pixels across.
- A tile that finds more than 4096 is split in four, down to 4 × 4 pixels, which are then searched pixel by pixel.
- Views whose tiles would be under 8 pixels across are searched pixel by pixel. That covers shallow views, where neighbouring pixels have different winners.

**Where tiles don't help.** Near the real axis, values bunch up along a line, so a tile's shortlist would be huge. `choose_method` sees this and searches pixel by pixel there.

### Parallelism

[`render()`](../src/engine.rs) hands out rows (or tiles) to one worker thread per core through an atomic counter. Each worker keeps its own `Solver` (reusable scratch buffers, so the hot loop never allocates) and sends finished rows back over a channel. A shared `rows_done` counter reports progress, and a `cancel` flag stops the workers at the next row. The GUI uses this flag when you start a new render before the old one finishes.

### Conjugate quadratic variant

`legacy = true` (`--legacy`) computes a different, non-holomorphic quantity: order 2 only, with the linear term using `conj(X)`, i.e. `min |c0 + c1·conj(X) + c2·X²|`, and with `c1 = 0` excluded instead of `c0 = 0`. It agrees with the ordinary residual only on the real axis, and gives its own distinctive images. It always uses the direct search.

---

## Using the library

```rust
use algfrac::colour::{colourise, ColourBy, Colormap, Stats, Style};
use algfrac::engine::{render, CoefSet, Coefs, Plan, Region};
use std::sync::atomic::{AtomicBool, AtomicUsize};

let plan = Plan::new(4, &Coefs::ints(32), false)?;           // order, coefficients, legacy
// or e.g. Coefs { set: CoefSet::Squares, max: 100, zero: false, zero_root: true }
let region = Region::square(1.9, 0.19, 0.16, 1024);           // centre re, im, half-width, width px
let (cancel, rows) = (AtomicBool::new(false), AtomicUsize::new(0));
let r = render(&plan, &region, 12, &cancel, &rows).unwrap();  // None only if cancelled

r.residual[row * r.width + col];  // min |p(X)|, row 0 = top (largest imaginary part)
r.coeffs_at(row, col);            // [c0, c1, …, cn]

let style = Style { by: ColourBy::Power, n: 3.0, cmap: Colormap::Inferno, ..Style::default() };
let rgb = colourise(&r, &Stats::new(&r), &style);             // packed RGB, row-major
algfrac::png::write_png("out.png", r.width, r.height, &rgb)?;
```

`Stats::new` stores each pixel's `ln(residual)` (as `f32`), sorts a copy once for the percentiles, and computes each pixel's length and height.

`colourise` (or `colourise_into`, which writes any pixel type, such as egui's `Color32`) then uses lookup tables, because a pixel's colour depends only on its `ln(residual)` or its length/height:
- The colour pipeline runs once per table entry, not once per pixel.
- The `ln(residual)` table covers only the range between the min and max percentiles, since colours are constant outside it. It has 4 096–131 072 entries, more for large `n`.
- Each pixel is then a single lookup, and the pixels are filled on all cores.

Recolouring 1024 × 1024 takes 4–8 ms on 12 cores, which keeps the GUI's display sliders smooth.

### Colouring (`colour.rs`)

| `ColourBy` | pixel value `x` before the colormap |
|---|---|
| `Power` | `b = (res / res_high)^(−n)`, clamped to 1, then stretched so that `b(res_low)` is 0. `res_high` and `res_low` are the residuals at the `high` and `low` percentiles of brightness. Computed in log space, so large `n` can't overflow. |
| `Log` | `−ln res`, scaled from the `low` to the `high` percentile |
| `Length` | `Σ|ci|`, scaled from the image's min to max (of their logs with `log_poly`) |
| `Height` | `max|ci|`, scaled from the image's min to max (of their logs with `log_poly`) |

The ends of these ranges form a `Scale` (`Stats::scale(style)`): the `ln(residual)` at the `high` and `low` percentiles, and the length and height ranges. `colourise` uses the render's own; `colourise_with` takes any. `Stats::levels()` keeps only what a scale needs (up to 16 385 percentiles and the ranges), and `Scale::mix` averages scales (`ln(residual)` linearly, lengths and heights geometrically). A video's **Steady brightness** and its length/height scale fixed from the waypoints use these (`video::held_scale`, see [Zoom videos](#zoom-videos)).

**Smooth stars** (`Style::smooth`, on by default; `--sharp` turns it off) averages the residual brightness (`Power`, `Log`, and the shading) over each pixel's cell instead of taking it at the centre. A star is a peak of `res^(−n)` much narrower than a pixel, so centre samples make it twinkle as it moves, and rows of close stars shimmer in moiré patterns.
- **The line.** `Stats` keeps, per pixel, the best polynomial as a straight line, `p(X + δ) ≈ p(X) + p′(X) δ`. It stores where that line is zero, in pixel units, and `ln(|p′| dx)`.
- **Which pixels.** Only pixels whose line's zero is within 4 pixels get averaged (about half of them in a typical view). The rest keep their centre value.
- **The average.** `Stats::cell_mean` averages the brightness table over 8 × 8 points of the cell within a pixel of the zero, and 4 × 4 further out, using a polynomial `fast_ln`.
- **The colour.** The mean brightness is then coloured through a 4096-entry table.
- **Cost.** About 45 ms to recolour a 720p frame on 12 cores, against 14 ms without.
- **Result.** A star's total brightness stays within about 5% as it crosses a pixel, against 86% from the centres (`smooth_points_keep_their_brightness`).
- **Limitation.** A star whose polynomial isn't the best at a cell's centre isn't counted in that cell.

`Style::colour(x, w)` then applies, in order:
1. `invert` (`1 − x`);
2. `gamma` (`x^(1/gamma)`);
3. colour cycling: `t = x·cycles + shift`. Cyclic maps take `t mod 1`; other maps go back and forth (a triangle wave), so there are no seams;
4. the colormap;
5. for `Length`/`Height` with `shade`: multiplying by `w`, the `Power` brightness;
6. `saturation` (scaling away from the Rec. 601 grey);
7. `(c − ½)·contrast + ½ + brightness`.

Steps 5–7 are skipped when they would change nothing. `Style::ramp(n)` samples steps 1–7 for the GUI's preview strip.

Colormaps:
- `bone`, `gray`, `hot`, `copper`, `pink`, `spring`, `summer`, `autumn`, `winter`, `cool`, `jet` and `hsv` are the classic formula-based maps.
- `parula`, `inferno`, `magma`, `plasma`, `viridis` and `turbo` are interpolated from 9–11 sampled stops of the well-known originals, so they are close but not exact.
- The psychedelic maps are `sinebow` (`sin²` waves a third of a turn apart) and four of Inigo Quilez's cosine palettes, `a + b·cos(2π(c·x + d))`: `neon`, `acid`, `candy` and `cosmic`.
- `hsv`, `sinebow`, `neon` and `acid` are cyclic (`Colormap::cyclic`).

### Predicting the density (`density.rs`)

How the star field looks depends mostly on how many algebraic numbers land per pixel. `Predictor` estimates that before rendering:

```
expected algebraic numbers in the view = Σ_d  N_d · ∫_view ρ_d(z) dA
```

- **N_d** (`poly_counts`) is the number of allowed polynomials of exact degree d. It's a product of how many values each coefficient can take, with c0 ≠ 0 and p, −p counted once. c0 = 0 only adds the root 0 and repeats lower degrees.
- **ρ_d** is the expected root density of one such polynomial. It's built in two parts.
  - **Shape:** the Kac–Rice formula for a random polynomial with independent Gaussian real coefficients (`complex_density`, `real_density`). This captures the roots crowding the unit circle more tightly as d grows, avoiding the real axis, and the separate density of roots exactly on it. It's evaluated inside the unit disc only, using ρ(z) = ρ(1/z)/|z|⁴ (reversing the coefficients inverts the roots), because the formula cancels badly for large |z|.
  - **Radial correction:** the Gaussian model assumes coefficients can be arbitrarily small or large. Integer coefficients confine the roots (with |c| ≤ 1, every root has ½ < |z| < 2), and the model misses that by large factors away from the unit circle. So random allowed polynomials are solved (about 6000 roots' worth per degree, using Aberth's method) (Aberth's method), and their roots' histogram over ln|z| is compared with the model's. The ratio, interpolated between bins of 0.1, multiplies ρ_d. Polynomials with a common factor are skipped, since k·q has the same roots as q.
- **Integral:** a 48 × 48 grid over the view, plus the real-root density along the real axis when the view contains it.

`cargo run --release --example density_check` compares the predictions with exact counts, found by solving every allowed polynomial and removing duplicate roots. Wherever a view holds more than a few dozen algebraic numbers, the prediction is within about 20%. It's 25–35% high for max coefficient 1–2, where many polynomials factor and repeat roots. Inside sharp-edged holes it can be several times too high, but the true count there is near 0 anyway.

The first prediction for a new set of coefficients takes up to about 70 ms (sampling every degree up to 16). After that, a new view takes about 3 ms for all 16 orders. The app uses this for its auto order / max coefficient: `choose_auto` in [common.rs](../src/common.rs) picks the value whose predicted density (the *root density*) is closest to the target (in log ratio). The app uses no work limit (`max_steps` = ∞; the old explorer had 2·10⁴ steps per pixel), so it can go up to the program's largest max coefficient, 1 000 000.

---

## Zoom videos

The video window (user guide: [Making a video](USER-GUIDE.md#making-a-video)) renders a zoom through a list of waypoints, one frame at a time. Everything except the window is in the library ([src/video/](../src/video)), so the command line renders the same videos.

**The path** ([`ZoomPath`](../src/video/mod.rs)) is a list of views (`stops`), each pair joined by a segment with its own pan speed `s` (`pan[i]`, frame heights per frame). Frame `j` of a segment from half-width `h0` to `h1`, with centres `c0` and `c1`, has half-width `h0·(h1/h0)^(j/steps)`, so the zoom speed is constant. Its centre is

```
c = c1 + (c0 − c1) · (h − h1) / (h0 − h1)       (or linear in j when h0 ≈ h1: a pure pan)
```

The centre moves in proportion to the half-width, so the end point's position on screen, `(c1 − c) / h`, moves steadily to the middle: an end point visible at the start of a segment stays visible throughout it. It also means the centre crosses the screen at a constant speed, `|dc| / 2h = |c1 − c0| · |d ln h| / 2|h0 − h1|` frame heights per frame. Summed over the segment, the pan covers `P = |c1 − c0| · |ln(h0/h1)| / 2|h0 − h1|` frame heights (`|c1 − c0| / 2h` for a pure pan). So the segment takes

```
steps = ceil( max( |ln(h0/h1)| / ln(rate),  P / s ) )
```

frames, which keeps both the zoom and the pan within their speeds.
- **Waypoints:** each one is shown exactly, at frame `stop_frame(i)`. `locate(k)` gives the segment and fraction for any frame.
- **Frame size:** the half-width is half the frame's *height*. Widescreen frames widen the region to the frame's aspect ratio ([`frame_region`](../src/video/mod.rs)).

**The max coefficient between waypoints.** It's set at the waypoints, and every frame between takes a value from a curve through them, with the frame number as x. The curve is monotone cubic Hermite interpolation ([`smooth`](../src/video/mod.rs): PCHIP, with Fritsch–Carlson slopes). It has no kinks at the waypoints, never overshoots, rises or falls steadily wherever the waypoint values do, levels off where they turn, and is a straight line through collinear points.
- **The curve** ([`max_at`](../src/video/mod.rs)) is through `ln M`. Holding the density steady needs `M` to grow roughly like a power of the magnification, and `ln h` is linear in the frame number, so `ln M` is too: between two waypoints `M` rises by the same factor every frame. Measured against `choose_auto` run on every frame of three zooms from the whole view (to 2·10⁻³, 10⁻³ and 10⁻⁴), this curve runs at most about 10% above it mid-zoom and matches it at the ends. Per-frame auto was dropped because its choices fluctuate: it sometimes fell during a zoom in, or jumped up.
- **Auto at a waypoint** ([`auto_maxes`](../src/video/frames.rs)) runs `choose_auto` on the waypoint's region at the video's resolution, with no work limit. Along a segment that zooms in, the result is raised to at least the previous waypoint's value, and along a zoom out lowered to at most it.
- **Keeping a flower the same size** ([`keep_flower`](../src/video/mod.rs)): near a quadratic number the dark flower's radius is about constant / M, so a segment that keeps the flower size multiplies `M` by its zoom, and `max_at` follows that straight line on the log scale.

**One colouring for the whole video** ([`Colouring`](../src/video/frames.rs)): a single `Style`, plus how its scale is chosen.
- **Each frame on its own** (the default for the residual brightness): every render's own percentiles and length/height range.
- **Steady brightness** (`steady`): each waypoint's frame is rendered on its own (never from patches) and its `Levels` measured. Every frame then gets the brightness levels of its segment's two ends, mixed by how far along the segment it is ([`held_scale`](../src/video/mod.rs)).
- **The length/height scale** (`LhScale`): each frame's own range; one range for the whole video from the lowest at any waypoint to the highest (`Waypoints`, the default, which also needs the waypoints' levels); or a range set by hand (`Fixed`). Values outside the range take the colormap's ends.
- `Colouring::needs_levels` says whether the waypoints' levels must be measured; `Colouring::scale` gives the scale for a render of a frame.

**Fading between max coefficients.** `M` is fractional between waypoints, but the allowed coefficients only change when it passes a member of the set. [`fade_between`](../src/video/mod.rs) finds the max coefficients either side, `lo = floor(M)` and `up` (the next member above it), and the fraction `t = ln(M / L) / ln(up / L)`, where `L` is the largest member up to `lo`. With fading on, the frame is rendered with both, each coloured with its own percentiles, and cross-faded by `t`. So the stars of each new coefficient fade in over the frames until `M` reaches it, instead of all appearing at once. That costs up to twice the render time. At a whole `M` (`t = 0`), one render is enough. Without fading, the nearer of `lo` and `up` is used.

**Reusing renders between frames** ([reuse.rs](../src/video/reuse.rs), on by default). Consecutive frames overlap almost completely, so the plane is rendered in **patches**. Each patch is a render on a fixed square grid, at one max coefficient, covering a run of consecutive frames, with spacing at most the finest frame's pixel spacing (divided by the smoothing factor). Each frame is cut out of its patch and averaged down to its pixels.
- **The ladder.** At large `M`, the frame's max coefficient changes every frame, which would leave nothing to share. So with reuse, `M` fades between the rungs of a ladder ([`ladder`](../src/video/reuse.rs)) instead of between neighbouring members of the set.
  - **Rungs:** the ladder has every member while they're more than `q = e^(g/(n+1))` apart, then members about `q` apart, plus the waypoints' own values. Each rung then allows `e^g` times as many polynomials as the one below.
  - **Step sizes:** `g` is the **Coefficient steps** setting, `COEF_STEPS`: fine (`g` = 0.125, 13% more polynomials, `M` +3% at order 3), medium (the default, 0.25: 28%, +6%) and coarse (0.5: 65%, +13%).
  - **Zoom per rung:** since `M ∝ h^(−2/(n+1))` roughly (see above), each rung serves a zoom of about `e^g` on either side. At `g = 0.5`, that's the cheapest span for a patch.
- **The grid.** A patch's samples are the multiples of its spacing `d`, stored as integer offsets (`left`, `top`). So patches with the same spacing, such as consecutive patches of a pan, sample identical points where they overlap, and changing patch can't make the stars jump.
- **Planning** ([`plan`](../src/video/reuse.rs)). The whole path is known in advance, so the patches are planned before rendering. Going through the frames in order, each rung's current patch is extended to the next frame that needs it: it grows to the bounding box of the frames' pixel areas, and gets finer if the new frame needs that. It's extended as long as its pixels per frame fall and it stays under `PATCH_FRAMES = 6` frames of pixels (and `MAX_RENDER_PIXELS`); otherwise a new patch starts.
  - In a zoom by `Z`, a patch costs `Z²` frames of pixels for `ln Z / ln(rate)` frames. That's cheapest at `Z = √e ≈ 1.65`, which matches a rung's span. At 1.02 per frame, that's about 25 frames for 2.7 frames of pixels.
  - In a pan, the cost per frame keeps falling, so the patch grows to the cap.
- **Making a frame** ([`add_frame`](../src/video/reuse.rs)):
  1. Crop the patch to the samples whose cells overlap the frame.
  2. Compute the percentiles with [`Stats::binned`](../src/colour.rs) and colour the crop. It doesn't sort every pixel. Instead it reads a table of 16 385 percentiles off a 65 536-bin histogram spanning the 0.2–99.8% points of a sample, and sorts the few pixels outside that range. That's exact to about 10⁻⁴ in ln(residual), and parallel. An earlier version took the percentiles from a 65 536-pixel sample instead. The sample differed every frame, which made the exposure jitter by a few percent.
  3. Average it down with a separable box filter: each frame pixel's cell takes the source cells it overlaps, weighted by the overlap, in linear light.
  4. Cross-fade the frame's renders ([`cross_fade`](../src/video/mod.rs)) in display brightness (the 8-bit values), as video dissolves are. Mixed in linear light, a star fading in brightens on screen like `t^(1/2.2)`: quickly at first, then slowly. That makes a pulse at every rung, which got worse with finer steps.
- **Same samples in the frame preview.** The video window's frame preview renders only the window of each patch that covers its frame, at the patch's own sample points ([`render_axes`](../src/engine.rs) takes explicit axes). So it matches the video exactly, at about 1–2.7 times the cost of rendering the frame directly.
- **Measured** at 1280 × 720 on 12 cores, order 3, auto max coefficients, with fading (`bench_reuse` in the old video maker). It also measured flicker in two ways (0–255 scale, lower is steadier):
  - **Exposure jitter:** the mean |second difference| of the frames' mean brightness.
  - **Frame-to-frame change:** each frame is mapped onto the next by the path's exact motion and compared, as mean |luma difference|. This includes stars genuinely fading in.

  | | time | exposure jitter | frame-to-frame change |
  |---|---|---|---|
  | **zoom** ×10 into the flower around (1 + i√2)/3, `M` 49 → 155, 118 frames | | | |
  | every frame separately | 163 s | 0.38 | 4.42 |
  | every frame separately, 2 × 2 smoothing | 621 s | 0.25 | 4.38 |
  | reuse, coarse steps | 25 s (6.6×) | 0.12 | 3.46 |
  | reuse, medium steps | 25 s (6.4×) | 0.17 | 3.48 |
  | reuse, fine steps | 37 s (4.5×) | 0.18 | 3.63 |
  | **pan** of half a frame height at `M` = 62, 52 frames | | | |
  | every frame separately | 25 s | 0.12 | 0.92 |
  | every frame separately, 2 × 2 smoothing | 95 s | 0.05 | 0.95 |
  | reuse (any steps: one patch) | 1.8 s (14×) | 0.07 | 0.62 |

  - **Per-frame overhead:** after the patches are rendered, each render a frame uses costs about 40 ns per pixel (crop, percentiles, colouring, averaging down), plus encoding. The time estimates include this.
  - **Quality:** on frames at a waypoint, where both methods use the same max coefficient, reuse and separate rendering are equally far from a 3 × 3 smoothed reference (about 25 dB PSNR each). The low PSNR is due to the single-pixel stars, not to either method, and the images look the same.

**The waypoint strip** (in [app/video.rs](../src/app/video.rs)). Each waypoint's frame is rendered at thumbnail size (90, 135 or 180 pixels high), on its own: at the waypoint's own max coefficient, without reuse or smoothing. A thumbnail's renders are kept with the `LevelsKey` they were made for (order, coefficients and the frame's jobs), so a change re-renders only the thumbnails whose search changed, 400 ms after the last change, the selected one first. Colour changes only recolour. With a colour scale measured at the waypoints, the strip measures it from the thumbnails themselves, which is close to but not exactly the video's.

**No render memory.** The old video maker kept up to 3 GB of renders to avoid repeating searches; to keep the memory use down, the app keeps only what's on screen: the explorer's image, the thumbnails' renders and the frame preview's. Only the waypoint `Levels` measured by the frame preview are cached (64 of them, since they're small), and rendering a video uses them instead of measuring those waypoints again.

**Time left.** [`Frames::frame_secs`](../src/video/frames.rs) estimates each frame's time: its search work (pixels × steps per pixel), using the time per unit of work measured by the latest frame preview (or the strip), plus the overhead. With reuse, a patch's work counts at the first frame that needs it. The progress display scales the time so far by (estimated total − estimated so far) / estimated so far. So later frames, with larger max coefficients and more work, count for more, and any error in the time per unit of work cancels out. That's also why the command line, which measures nothing beforehand, gets the same estimates.

**Each frame** ([`make_video`](../src/video/make.rs)):
1. If the colouring needs them, the waypoints' levels not known yet are measured first, each from its frame rendered on its own.
2. `Frames::needs` gives the max coefficient(s) and their weights in the fade: rungs of the ladder with reuse, members of the set without.
3. With reuse, any of the frame's patches not yet rendered are rendered (`render_axes` on all cores), and a patch is dropped after its last frame. Without reuse, the frame's region is rendered once per max coefficient, with smoothing sampling `aa` × `aa` times per video pixel.
4. `Frames::finish` colours the renders with the video's style, each on the scale `Colouring::scale` gives it. With reuse, `add_frame` averages each render down and `cross_fade` mixes them. Without reuse, the renders are cross-faded and [`downsample`](../src/video/mod.rs) averages each `aa` × `aa` block. Averaging is in linear light (γ 2.2, via lookup tables, across all cores), so small bright stars keep their brightness rather than turning grey. Cross-fades are in display brightness (see above).
5. The frame goes into the `.mp4`, and/or is written as a compressed PNG (`frame_00000.png`, …), as the project's format says.
6. The callback gets the frame (the app shows it and updates the progress; the command line prints progress every 2 s).

**Encoding** ([`Mp4Out`](../src/video/mod.rs)):
- **Colour conversion:** RGB is converted to I420 with BT.709 limited-range coefficients, matching the VUI flags written into the stream.
- **Encoder:** the [`openh264`](https://crates.io/crates/openh264) crate compiles Cisco's OpenH264 encoder from source into the exe.
- **Rate control:** the encoder uses OpenH264's quality mode with a fixed quantiser (`QpRange::new(qp, qp)`), so every frame gets the same quality and the file is as big as the content needs. Frame skipping is off, and there's a keyframe every 2 s. The bit-rate setting is only a ceiling. Star fields are close to the worst case for video compression: thousands of single-pixel points, all moving. A fixed bit rate either blurs them badly or wastes space on easy frames. The quality presets are QP 26 / 20 / 16 / 10. On a 720p zoom (decoding every frame and comparing with the rendered frames) they measured:

  | QP | bits per pixel per frame | PSNR(Y) |
  |---|---|---|
  | 26 | 1.45 | 35.3 dB |
  | 20 | 2.26 | 40.8 dB |
  | 16 | 2.92 | 44.5 dB |
  | 10 | 3.69 | 48.0 dB |

- **Background detection and adaptive quantisation are off.** Both are on by default in OpenH264 and are meant for camera video. Background detection copies blocks it judges to be static background from the previous frame. In a zoom to 2.4e-8 into faint, regular rows of stars, it left 16 × 16 blocks of stars displaced or behind until the next keyframe. On that zoom's last 40 frames, the worst block's mean |luma error| at QP 20 fell from 5.5 to 2.5, with a slightly smaller file. Adaptive quantisation made no measurable difference there.
- **Muxing:** the [`mp4`](https://crates.io/crates/mp4) crate writes the container. Its samples are AVCC: NAL units with 4-byte length prefixes instead of Annex B start codes. The SPS and PPS go into the `avcC` header and are left out of the samples, so the file is created when the first frame arrives, which is when they're known.
- **Stopping early:** `write_end` still runs when the render is stopped, so a stopped video plays. Stopped before the first frame, the file is deleted.

**Video projects** ([project.rs](../src/video/project.rs)) are `key<tab>value` text files, `projects/<name>.afvideo` in the settings folder, with one `wp` line per waypoint (name, centre, half-width, max coefficient, auto, pan speed and keep-flower to the next). Unknown keys are ignored and missing ones take their defaults, so files move freely between versions. The old video maker's `.path` files open as new projects, with their first waypoint's colouring for the whole video.

**Licensing.** OpenH264's source is BSD-licensed. H.264 itself is covered by patents. Cisco pays the licence fees only for its own prebuilt OpenH264 binaries, not for builds compiled from source like this one. For personal, non-commercial use this is rarely a concern. For commercial distribution, check the rules that apply to you.

---

## Command line

```powershell
dist\algfrac.exe --order 4 --maxcoef 32 --centre 1.9,0.19 --halfwidth 0.16 --width 1024 --out spiral
```

This writes `spiral.png` and `spiral.algf`, and prints progress with a time estimate:

```
algfrac: order 4, coefficients integers |c| <= 32
  region re [1.74, 2.06]  im [0.03, 0.35]  ->  1024 x 1024 px on 12 threads
  per pixel: ~2176 steps (direct search: table 4160 + ~137312 queries), vs 1.142e9 polynomials brute force
  predicted: ~16.207 algebraic numbers per 100 px^2
  memory: ~32 MB working set
  ...                                    (progress every 2 s: % done, time left)
  done in 15.53s (67500 px/s)
```

| option | meaning | default |
|---|---|---|
| `--centre RE,IM` `--halfwidth H` | square region around a centre | |
| `--re MIN,MAX` `--im MIN,MAX` | explicit ranges (non-square is fine) | `-2,2` / `-2,2` |
| `--order N` | highest power | 3 |
| `--maxcoef M` | coefficients in `[−M, M]` | 8 |
| `--coefs SET` | which coefficient sizes are allowed (with either sign, and 0): `ints` (1, 2, 3, …), `primes` (1, 2, 3, 5, 7, …), `fib` (1, 2, 3, 5, 8, …), `odd` (1, 3, 5, …), `triangular` (1, 3, 6, 10, …), `squares` (1, 4, 9, …), `cubes` (1, 8, 27, …) or `pow2` (1, 2, 4, 8, …) | `ints` |
| `--nozero` | don't allow 0 as a coefficient, so every power appears | |
| `--zeroroot` | allow `c0 = 0` (polynomials `x·q(x)`), so `X = 0` counts as a root | |
| `--legacy` | the conjugate quadratic variant (see above) | |
| `--width W` | pixels along the real axis. The height follows the aspect ratio. | 512 |
| `--colour pow\|log\|len\|hgt` | what to plot (see Colouring) | `log` |
| `--n N` | exponent for `pow` and for shading | 1 |
| `--max P` (or `--clip P`) | brightness percentile mapped to full brightness | 99.5 |
| `--min P` | brightness percentile mapped to black | 0 |
| `--colormap NAME` | `bone gray hot copper pink spring summer autumn winter cool jet parula inferno magma plasma viridis turbo hsv sinebow neon acid candy cosmic` | `bone` |
| `--invert` / `--noshade` | reverse the colormap / don't shade `len`/`hgt` | |
| `--logpoly` | spread `len`/`hgt` over the colormap on a log scale | |
| `--sharp` | brightness at each pixel's centre instead of averaged over the pixel (smooth stars) | |
| `--gamma G` | `x → x^(1/G)` before the colormap; above 1 brightens midtones | 1 |
| `--cycles C` / `--shift S` | run through the colormap `C` times / shift along it by `S` (a fraction of it) | 1 / 0 |
| `--brightness B` / `--contrast K` / `--saturation S` | final-colour adjustments (see Colouring) | 0 / 1 / 1 |
| `--out NAME` | output name (`NAME.png`, `NAME.algf`) | `algfrac_out` |
| `--nodata` | skip the `.algf` file | |
| `--threads T` | worker threads | all cores |

The command line's PNGs are written uncompressed (byte for byte as the old `algfrac.exe` wrote them), so they are about 3 bytes per pixel. Re-save them in any image editor to shrink them. The app's are compressed.

### Rendering a video project

```powershell
dist\algfrac.exe video "Spiral dive"          # a project saved in the app, by name
dist\algfrac.exe video D:\shared\dive.afvideo # or a project file
dist\algfrac.exe video --list                 # the saved projects
```

It renders the project exactly as the app's Render now would (the same `make_video`), after choosing the automatic max coefficients again. `--out FOLDER` writes it somewhere other than the project's folder, and `--threads T` limits the worker threads. It prints the frame, the size so far and the time left every 2 s. Ctrl+C stops after the current frame and keeps the video so far; a second Ctrl+C stops at once. Rendering long videos this way (overnight, say) leaves the app free.

### `.algf` data format

The `.algf` file holds the raw results, for analysis or custom colouring in other tools ([src/algf.rs](../src/algf.rs) reads and writes them). The old explorer also kept one with each saved place; the app doesn't use them. It is little-endian:

```
"ALGF"                                  4 bytes
u32 version (= 2)
u32 width, u32 height, u32 order, i32 maxcoef, i32 reserved (always 1)
u32 coefset (0 = ints, 1 = primes, 2 = fib, 3 = odd, 4 = triangular, 5 = squares, 6 = cubes, 7 = pow2)
u32 flags (bit 0 = conjugate variant, bit 1 = 0 not allowed, bit 2 = c0 = 0 allowed)
f64 re_axis[width]                      left to right
f64 im_axis[height]                     top row first
f64 residual[height*width]              row-major, top row first
i32 coeffs[height*width*(order+1)]      [c0 … cn] per pixel
```

For example, in Python with NumPy:

```python
import numpy as np
b = open("spiral.algf", "rb").read()
w, h, order = map(int, np.frombuffer(b, "<u4", 3, 8))
o = 36
re  = np.frombuffer(b, "<f8", w, o);        o += 8 * w
im  = np.frombuffer(b, "<f8", h, o);        o += 8 * h
res = np.frombuffer(b, "<f8", w * h, o).reshape(h, w);  o += 8 * w * h
coeffs = np.frombuffer(b, "<i4", w * h * (order + 1), o).reshape(h, w, order + 1)
```

---

## Performance

12-core PC, 256×256 unless stated. "Brute force" is the number of polynomials per pixel that trying every one would need.

| settings | brute force | algfrac |
|---|---|---|
| order 3, M 32 | 1.8×10⁷ | 0.1 s |
| order 4, M 16 | 3.9×10⁷ | 0.2 s |
| order 4, M 32 | 1.2×10⁹ | 0.7 s, 5 MB |
| order 4, M 32, 1024×1024 | 1.2×10⁹ | 16 s |
| order 5, M 12 | 2.4×10⁸ | 0.55 s |
| order 6, M 8 | 4.1×10⁸ | 1.1 s |
| order 8, M 4 | 3.9×10⁸ | 1.35 s |
| order 8, M 10, centre 0.5+0.5i, half-width 1.5 | 7.6×10¹¹ | 2 min (≈ 550 px/s) |
| order 8, M 10, 128×128 off the real axis (0.3+0.7i, half-width 0.2) | 7.6×10¹¹ | 7 s (direct search alone: 95 s) |

Before the lattice search was added, the order 4–8 rows took 2.3–6.7 s (3–6× longer). Time per pixel grows with the steps printed at start-up (and shown live in the GUI), but a lattice step costs more than a direct one, so compare steps only between similar settings. Views that cross or hug the real axis are slower, because the lattice search falls back to the direct search there.

**Deep views** ([tiles](#deep-views-one-search-per-tile)). Order 3, a 1280 × 720 frame of width 10⁻⁷, 12-core PC:

| max coefficient 3000 | before | pixel by pixel now | tiles |
|---|---|---|---|
| centre −0.62 + 0.71i | ~23 min | 18 s | **0.03 s**, the same polynomial at all 921 600 pixels |
| centre 0.53 + 3×10⁻⁶ i (near the real axis) | ~3.3 h | ~2 min | (not used: pixel by pixel) |

"Before" was dominated by the first pixel of each row, which used to start without a bound and so ran the direct search, building a 36-million-entry table. The table is now also allocated only when needed, instead of up front for every thread (860 MB each at max coefficient 3000).

Speed-up from tiles over pixel by pixel, on a 384 × 96 strip of a 1280-wide view (whole frames gain more):

| view width | 10⁻² | 10⁻³ | 10⁻⁴ | 10⁻⁵ | 10⁻⁶ | 10⁻⁷ | 10⁻⁹ |
|---|---|---|---|---|---|---|---|
| max coefficient 155 | 1× (pixel by pixel) | 10× | 31× | 30× | 32× | 36× | 37× |
| max coefficient 1000 | 1× (pixel by pixel) | 1× (pixel by pixel) | 16× | 54× | 96× | 102× | 91× |
| max coefficient 3000 | 1× (pixel by pixel) | 1× (pixel by pixel) | 1× (pixel by pixel) | 21× | 122× | 154× | 150× |

Run it with `cargo test --release --lib bench_deep -- --ignored --nocapture` (set `ALGFRAC_DEEP_MAX` for another max coefficient).

## Accuracy

The results were checked pixel by pixel against an independent brute-force implementation on seven test grids (orders 1–4, `M` from 4 to 300, grids through exact algebraic points and through `X = 0`):
- residuals agreed to within 5×10⁻¹⁵ (rounding error), and to 6×10⁻¹⁴ relative for the conjugate quadratic variant;
- the coefficients found were never more complex than brute force's, and at tied pixels (about 3% of the test pixels) they were simpler, because of the tie rule above.

`cargo test --release --lib --bins` checks:
- both searches (direct and lattice) against brute force on orders 2–6 with every kind of coefficient set, including `c0 = 0`;
- that the tile search (`collect`) lists exactly the polynomials within its radius, against brute force, and that tiled renders match pixel-by-pixel renders at depths from 10⁻³ to 10⁻⁷, across the real axis and outside the unit circle;
- that the colour lookup tables give the colours the per-pixel pipeline would, to within one table step;
- that the density model's root densities integrate to exactly `d` roots for each degree;
- the root finder;
- that the auto settings land near the target root density, and never decrease as you zoom in;
- the zoom path, the max coefficient curve, the colour scales, `.mp4` and PNG-frame writing, short end-to-end videos with and without reuse, and stopping before the first frame (see [Zoom videos](#zoom-videos));
- saved locations and groups, video projects and remembered settings reading back exactly, and files from the old programs (`tests/data/`) loading;
- the compressed PNG writer.

`recolour_speed` prints the time to recolour 1024 × 1024. On real renders, the lattice search returns the same polynomial as the direct search on every pixel except exact ties (e.g. two polynomials with equal `Σ|ci|` giving exactly 0.0016). The density predictions are checked against exact counts by `cargo run --release --example density_check` (see [Predicting the density](#predicting-the-density-densityrs)).

## Limitations

- **Deep zoom**: coordinates are `f64`, so below a half-width of about `10⁻¹² × |centre|` neighbouring pixels become indistinguishable and the image turns blocky.
- **Table size**: tables are kept to 2²⁶ entries per thread (see [Meet in the middle](#meet-in-the-middle)). Above max coefficient 4095 at order 3, the direct search builds its table in chunks, which adds work.
- **Exactly on the real axis** with large max coefficients, every pixel needs the direct search: about 4 minutes per pixel at max coefficient 10 000. So a view whose rows pass within about 10⁻⁹ of the axis gets very slow at large max coefficients.
- **Near the real axis** the lattice search gives up and the slower direct search does the work (see [The lattice search](#the-lattice-search)). Exactly on the axis, `X` is real and only the direct search applies. Tiles don't help there either, so deep views hugging the axis with large max coefficients stay slow (about 2 minutes for a 720p frame at max coefficient 3000).
- **Density predictions** are averages over the view. They don't see the holes around individual simple numbers, so a deep zoom into a hole holds fewer stars than predicted. Views exactly on the real axis at extreme zoom are least reliable, because the complex-root formula loses precision very close to the axis.
- **Coefficient sets** must be symmetric about 0 (true for every `CoefSet`, with or without 0), because the sign-symmetry trick relies on it.
- **Builds**: `build.ps1` is for 64-bit Windows and `build-mac.sh` for macOS 11+. Linux builds from source with `cargo build --release`, but no release builds are made for it.
