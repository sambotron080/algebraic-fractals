# Algebraic Fractals: specification for the combined program

This document describes the new program completely: what it does, how it looks, how it behaves, how it's built and how it's shipped on GitHub. It merges the old explorer (`algfrac-gui.exe`) and the old video maker (`algfrac-video.exe`) into one app.

**How to read and edit it**
- Edit anything. Delete what you don't want, add what's missing, and change any default.
- **(New)** marks a feature the old programs didn't have. Everything else is a feature the old programs have, and the new one must keep.
- **Decision** marks a choice I made that you may want to change. They're also collected in [section 13](#13-decisions-for-you).
- The old programs, in `..\algfrac`, are the reference implementation. Where this spec says nothing about a detail, the new program behaves exactly like the old one. Where they disagree, this spec wins.
- Section 3 lists the code that has already been copied over. Everything else will be written new.

---

## Contents

1. [What the program is](#1-what-the-program-is)
2. [What gets shipped](#2-what-gets-shipped)
3. [Code already carried over](#3-code-already-carried-over)
4. [How the app is organised](#4-how-the-app-is-organised)
5. [The window](#5-the-window)
6. [Shared by both modes](#6-shared-by-both-modes)
7. [Explore mode](#7-explore-mode)
8. [Video mode](#8-video-mode)
9. [Moving between the modes (New)](#9-moving-between-the-modes-new)
10. [The command-line tool](#10-the-command-line-tool)
11. [Files on disk](#11-files-on-disk)
12. [The engine: what must stay true](#12-the-engine-what-must-stay-true)
13. [Decisions for you](#13-decisions-for-you)
14. [Tests](#14-tests)
15. [Building and releasing](#15-building-and-releasing)
16. [Documentation](#16-documentation)
17. [Order of work](#17-order-of-work)
18. [Appendix: constants](#18-appendix-constants)

---

## 1. What the program is

**Algebraic Fractals** draws the complex plane, showing how close each point is to an **algebraic number**: a root of a polynomial with whole-number coefficients. For each pixel `X` it finds

```
residual(X) = min |c0 + c1·X + c2·X² + … + cn·Xⁿ|
```

over every polynomial up to a chosen **order** `n`, with coefficients from a chosen set up to a chosen **max coefficient** `M`. It also records which polynomial gives that minimum. Small residuals light up as stars. The result is star fields, lace, flowers and spirals that continue at every zoom level.

The program has two **modes** in one window:
- **Explore:** click and zoom through the plane, change the search settings, recolour instantly, save PNGs, and bookmark places with their renders.
- **Video:** set a path of waypoints through the plane and render a smooth zoom video (`.mp4`), with the star density and the colouring changing smoothly along the way.

A separate **command-line tool**, `algfrac.exe`, renders single images for scripts and very large renders.

**Platforms.** 64-bit Windows 10/11 is the main target. macOS 11+ (Apple silicon and Intel) is the second target. Linux should build from source, but no release builds are made for it.

---

## 2. What gets shipped

### 2.1 The GitHub repository

The repository is this folder. Its layout when finished:

| path | contents |
|---|---|
| `README.md` | the GitHub front page: example pictures, what it is, download links, quick start, why the patterns look the way they do, further reading |
| `LICENSE` (or `LICENSE-MIT` and `LICENSE-APACHE`) | the licence: **Decision**, see [13](#13-decisions-for-you) |
| `docs/USER-GUIDE.md` | the user guide for both modes, merged from the old `README-GUI.md` and `README-VIDEO.md` |
| `docs/DEVELOPER.md` | building, the source layout, how the search works, the library, file formats and tests, from the old developer `README.md` |
| `docs/MAC.md` | first-run instructions for macOS (Gatekeeper), from the old `README-MAC.md` |
| `docs/images/` | a few example pictures for the README, compressed (the program writes uncompressed PNGs of about 3 MB each) |
| `SPEC.md` | this document |
| `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml` | the Rust package |
| `src/` | the source (see [4.1](#41-source-layout)) |
| `examples/density_check.rs` | checks density predictions against exact counts |
| `build.ps1`, `build-mac.sh` | release builds for Windows and macOS |
| `.github/workflows/` | **(New, Decision)** automatic builds on GitHub (see [15.3](#153-releases-on-github)) |
| `.gitignore` | `target/`, `dist/`, `dist-mac/`, `*.algf` |

### 2.2 Release downloads

For each release, a GitHub **Release** has:
- **Windows:** `AlgebraicFractals-<version>-windows-x64.zip`, containing `Algebraic Fractals.exe` (the app), `algfrac.exe` (the command line) and `USER-GUIDE.md`. Each `.exe` is a single, self-contained file that uses only DLLs that ship with Windows.
- **macOS:** `AlgebraicFractals-<version>-macos.zip`, containing `Algebraic Fractals.app` (universal: Apple silicon and Intel), `algfrac` and the guides.

**Decision:** the old programs were named `algfrac-gui.exe` and `algfrac-video.exe`. The new app is `Algebraic Fractals.exe` (Cargo binary name `algebraic-fractals`, renamed when packaged), and the window title is "Algebraic Fractals".

---

## 3. Code already carried over

These files were copied from the old programs, because they would have been rewritten word for word. They're the parts with the hardest-won details: the search algorithms, the tie rules, the colouring maths, the encoder settings and the patch planning. They build and pass their tests in this folder.

| new file | copied from | what it is | edits made |
|---|---|---|---|
| `src/engine.rs` | `src/engine.rs` | the search: coefficient sets, `Plan`, the direct and lattice searches, tiles for deep views, `render`, `render_axes`, `Render` | none |
| `src/colour.rs` | `src/colour.rs` | turning a render into colours: `Style`, `ColourBy`, colormaps, `Stats`, percentiles, Smooth stars, `Scale` and `Levels` | none |
| `src/density.rs` | `src/density.rs` | predicting the density of algebraic numbers before rendering (`Predictor`) | none |
| `src/png.rs` | `src/png.rs` | a dependency-free PNG writer (uncompressed) | none |
| `src/algf.rs` | `src/algf.rs` | reading and writing `.algf` render files | none |
| `src/video/mod.rs` | `src/bin/algfrac-video/video.rs` | the zoom path (`ZoomPath`), smooth curves through the waypoints, fading, frame colouring, the held colour scale, downsampling, `simplest_quadratic`, and the `.mp4` writer (`Mp4Out`) | `algfrac::` became `crate::`; declares `pub mod reuse` |
| `src/video/reuse.rs` | `src/bin/algfrac-video/reuse.rs` | reusing renders between frames: the ladder, patch planning, cropping and averaging down | `algfrac::` became `crate::` |
| `src/common.rs` | `src/bin/common/mod.rs` | `View`, saved places (`Place`, `places.tsv`, where renders go), number parsing and formatting, `choose_auto`, `allowed_text` | the egui widgets were left out (they'll be rewritten in the app); the parsing test moved here |
| `src/bin/algfrac.rs` | `src/main.rs` | the command-line tool | none |
| `examples/density_check.rs` | `examples/density_check.rs` | density predictions against exact counts | none |
| `.cargo/config.toml` | `.cargo/config.toml` | portable CPU target and the static C++ runtime for Windows | none |

Not copied, to be rewritten: the two windows (`algfrac-gui.rs`, `algfrac-video/main.rs`) and the egui parts of `common/mod.rs`. The video maker's non-window logic (`Frames`, `auto_maxes`, `RenderKey`, `RenderMemory`, `SavedPath`, `make_video`, `frame_secs`) will be moved into the library as it's ported, mostly unchanged, so it can be tested without a window ([4.1](#41-source-layout)).

---

## 4. How the app is organised

### 4.1 Source layout

| file | contents |
|---|---|
| `src/lib.rs` | the `algfrac` library: `engine`, `colour`, `density`, `png`, `algf`, `common`, `video` |
| `src/video/mod.rs`, `src/video/reuse.rs` | carried over (section 3) |
| `src/video/frames.rs` | **to write:** `Frames` (everything that decides what the frames show), `auto_maxes`, `frame_secs` |
| `src/video/paths.rs` | **to write:** `SavedPath` and the `.path` files |
| `src/video/make.rs` | **to write:** `make_video`, the render loop, with progress reported through a callback instead of egui |
| `src/memory.rs` | **to write:** `RenderKey` and `RenderMemory`, shared by both modes ([6.6](#66-renders-kept-in-memory)) |
| `src/main.rs` | **to write:** the app: creates the window, owns the shared state, switches modes |
| `src/app/explore.rs` | **to write:** Explore mode |
| `src/app/video.rs` | **to write:** Video mode |
| `src/app/widgets.rs` | **to write:** controls used by both modes: the coefficient-set rows, the Display section, the colormap strip, places menus |
| `src/bin/algfrac.rs` | the command-line tool (carried over) |

The library stays free of GUI code, so the command line and the tests use it without a window. Only `src/main.rs` and `src/app/` use `eframe`/`egui`.

### 4.2 Threads

- The **UI thread** draws the window and never runs a search. A slow step, such as estimating a video's render time, runs on a background thread.
- **Searches** run on worker threads, one per core (`std::thread::available_parallelism`), through `engine::render_axes`. Each job has a `cancel` flag and a `rows_done` counter for progress.
- **Explore** has at most one render running. Starting a new one cancels the old one.
- **Video** has at most one preview render, and at most one video render. A preview is cancelled when a new preview starts, or when Render video starts.
- **(New) Explore and Video can be busy at the same time.** For example, a video renders while you explore. They share the cores, so each runs slower while the other is busy. While the video renders, its progress also shows in the top bar ([5.1](#51-layout)).
- Background threads ask for a repaint when they finish. While a job runs, the UI repaints every 100 ms for the progress bars (200 ms while a video renders).

### 4.3 Shared state

Owned by the app and used by both modes:
- the saved places list (one list in memory, written to `places.tsv` whenever it changes, so the modes always agree);
- the renders kept in memory ([6.6](#66-renders-kept-in-memory));
- the density predictor (`Predictor`, which caches its sampled root densities);
- the thread count;
- (New, Decision) the remembered settings ([6.8](#68-remembered-settings-new)).

Each mode keeps its own state when you switch to the other one, and anything running carries on.

---

## 5. The window

### 5.1 Layout

```
┌───────────────────────────────────────────────────────────────────────┐
│ [ Explore ] [ Video ]                     video: frame 120 of 900, ~3 min left │  top bar
├──────────────────┬────────────────────────────────────────────────────┤
│ the mode's       │                                                    │
│ controls         │              the mode's image                      │
│ (scrolls)        │                                                    │
│                  │                                                    │
├──────────────────┴────────────────────────────────────────────────────┤
│ status: the point under the mouse, its polynomial; messages           │  status bar
└───────────────────────────────────────────────────────────────────────┘
```

- **Top bar (New):** two tabs, **Explore** and **Video**. The mode that's showing is highlighted. On the right, while a video renders, a short progress note: "video: frame k of n, ~T left". Clicking it switches to Video.
- **Controls:** a panel on the left, 340 px wide, not resizable, scrolling vertically. (The old explorer's was 300 px, the video maker's 350 px.)
- **Image:** the rest of the window, background grey 16. The image is scaled to fit, keeping its aspect ratio, and centred.
- **Status bar:** at the bottom. Each mode sets its own text ([7.10](#710-status-bar), [8.10](#810-status-bar)).
- **Window:** 1400 × 900 at start, title "Algebraic Fractals". It opens in **Explore** mode. **Decision:** or reopen in the mode used last ([6.8](#68-remembered-settings-new)).
- A window icon, and an icon for the `.exe` and the Mac app. **(New, Decision)** The old programs had none.

### 5.2 Keyboard

Keys act only while no text box has the keyboard focus.

| key | mode | effect |
|---|---|---|
| Enter | Explore | Run: zoom into the yellow box |
| Backspace | Explore | Back |
| Esc | Video | cancel picking on the image |
| Ctrl+1 / Ctrl+2 | both | switch to Explore / Video **(New)** |

Enter in a text box does that box's own action (Go, Save, and so on), as described with each box.

---

## 6. Shared by both modes

### 6.1 Views and coordinates

- A **view** is a centre `cr + ci·i` and a **half-width** `hw`.
- In Explore, the image is square: it spans `cr ± hw` and `ci ± hw`.
- In Video, `hw` is half the frame's **height**. A widescreen frame also spans `cr ± hw·16/9` horizontally.
- Row 0 of a render is the top: the largest imaginary part.
- The **home** view is centre 0, half-width 2.
- **Typing a centre:** these forms are accepted: `0.5+0.8i`, `-1.2 - 3e-4i`, `2i`, `-i`, `0.3`, `0.5, 0.8`, `1.5E+2-2j`. Spaces are ignored, `j` counts as `i`, and `*i` as `i` (`common::parse_complex`). A half-width must be a finite number above 0.
- **Showing a centre:**
  - in text boxes, the shortest exact form, such as `0.5-0.25i` (`complex_text`), so copying it out and back loses nothing;
  - on labels, a fixed form with 12 decimals, such as `+0.500000000000 + 0.250000000000i` (`fmt_complex`);
  - in the waypoint list, 4 decimals.
- **Precision:** coordinates are 64-bit floats. Below a half-width of about 10⁻¹² × |centre|, neighbouring pixels can't be told apart and the image turns blocky. Video mode warns when a waypoint goes that deep (Explore's guide mentions it under troubleshooting).

### 6.2 Search settings

| setting | values | meaning |
|---|---|---|
| **Order** | 1–16 | the highest power of `x` |
| **Max coefficient** | 1–1 000 000 | the largest allowed `|c|` |
| **Coefficients** | integers; 1 and primes; Fibonacci numbers; odd numbers; triangular numbers; squares; cubes; powers of 2 | which sizes are allowed, each with either sign. Every set contains 1. In the dropdown, hovering a set shows its first members, e.g. `±1, ±2, ±3, ±5, ±7, ...` |
| **allow 0** | on/off (default on) | whether a coefficient may be 0. Off means every power from 1 to the order appears. |
| **0 can be a root** | on/off (default on) | whether `c0` may be 0 (polynomials `x·q(x)`), so that 0 itself lights up. Off: `c0` is never 0 and 0 sits in a dark hole. |

The **Allowed** line under the settings lists the coefficients in use, e.g. `0, ±1, ±2, ±3, ±5, ±8`. Lists of more than 10 are shortened to `±1, ±2, ±3, ±4, ±5, ±6, ... ±1000 (1000 values)`. With allow 0 on and 0 can be a root off, it adds "(c0 is never 0)" (`allowed_text`).

The "Max coefficient" box can be dragged. The drag speed is 1% of the value (at least 0.2 per pixel), so large values change proportionally. Order drags at 0.05 per pixel.

### 6.3 Density prediction and auto

How a star field looks depends mostly on how many algebraic numbers land per pixel. `density::Predictor` estimates this before rendering, in **algebraic numbers per 100 square pixels**, usually to within about 20%.

`common::choose_auto` picks either the **order** or the **max coefficient** whose predicted density is closest to a **target density** (in log ratio). It keeps all the other settings, and it never needs more than `max_steps` of work per pixel.
- **Explore** uses `max_steps` = 2·10⁴ (`AUTO_MAX_STEPS`), about 7 s at 256² or 2 minutes at 1024² on 12 cores. If the target needs more, an orange "Auto: …" note explains why it fell short.
- **Video** uses no work limit, so it can go up to max coefficient 1 000 000. Its note then says "has too many polynomials to search".
- The first prediction for a new coefficient set takes up to about 70 ms. After that, each view takes about 3 ms.

### 6.4 Colouring (the Display section)

A **style** (`colour::Style`) decides how a render is coloured. Changing it recolours instantly, without searching again. The Display section looks the same in both modes ([8.8](#88-display-colouring-per-waypoint) says which parts are per waypoint in Video).

| control | range (default) | what it does |
|---|---|---|
| **Colour by** | 1 / residual^n (default); −log(residual); length (sum \|ci\|); height (max \|ci\|) | what's plotted. Power: brightness `(res/res_high)^(−n)`, clamped, stretched so `res_low` is black, computed in log space. Log: `−ln res` between the percentiles. Length/height: the polynomial's Σ\|ci\| or max\|ci\|, from the image's lowest to highest. |
| **Colormap** | 23 maps (bone default) | classic: bone, gray, hot, copper, pink, spring, summer, autumn, winter, cool, jet, parula; modern: inferno, magma, plasma, viridis, turbo; rainbow/psychedelic: hsv, sinebow, neon, acid, candy, cosmic. hsv, sinebow, neon and acid are cyclic (their ends match). |
| colormap strip | | a 14 px strip under the colormap, 256 samples with every setting below applied: left = darkest pixels, right = brightest. Redrawn only when the style changes. |
| **n** | 0.05–20, log slider (1) | the sharpness exponent, for 1/residual^n and for shading |
| **max %** | 50–100 (99.5) | the brightness percentile that becomes full brightness |
| **min %** | 0–100 (0) | the brightness percentile that becomes black. It's kept at least 0.1 below max %. |
| **Smooth stars** | on/off (on) | average each pixel's brightness over the pixel instead of taking it at the centre (see [12](#12-the-engine-what-must-stay-true)) |
| **Invert colormap** | on/off (off) | swap the ends |
| **Shade by 1/res^n** | on/off (on) | length/height only: darken away from the stars |
| **Log scale** | on/off (off) | length/height only: spread the colormap over the log of the length/height |
| **brightness** | −1 to 1 (0) | lighten or darken every colour |
| **contrast** | 0 to 4 (1) | spread colours from mid-grey, or squash them towards it |
| **gamma** | 0.1–10, log (1) | above 1 brightens the faint points and midtones; black and full brightness stay put |
| **saturation** | 0 to 3 (1) | 0 = grey, above 1 = more vivid |
| **colour cycles** | 1–50, log (1) | run through the colormap several times: bands like contour lines |
| **colour shift** | 0 to 1 (0) | slide the colours along the colormap |
| **Reset display** | button | everything back to the defaults |

**Greyed out:** n, max %, min % and Smooth stars when colouring by length/height with shading off (they only act on the residual). Shade and Log scale unless colouring by length/height.

**Hover texts** (keep them, worded as in the old programs):
- n: "Larger n makes the points near algebraic numbers sharper".
- max %: "Brightness percentile that becomes full brightness: 99.5 saturates the brightest 0.5% of pixels. Lower = brighter image".
- min %: "Brightness percentile that becomes black: 50 makes the darker half of the pixels black. Raise it to hide the background haze".
- Smooth stars, gamma, saturation, colour cycles, colour shift, shading and log scale: as in `common/mod.rs`.

**The order of operations** for each colour (`Style::colour`, carried over):
1. invert;
2. gamma;
3. cycles and shift (cyclic maps wrap; others go back and forth);
4. the colormap;
5. shading;
6. saturation (Rec. 601 grey);
7. contrast and brightness.

Recolouring uses lookup tables and all cores: 4–8 ms for 1024², about 45 ms for a 720p frame with Smooth stars.

### 6.5 Saved places

A **place** is a named view, kept in `places.tsv` ([11](#11-files-on-disk)). Both modes use the same list.

- **Saving (Explore):** saves the shown view's centre and half-width, and its **whole render**: every pixel's best polynomial and residual, as an `.algf` file in the `renders` folder. The name comes from the name box. An empty name uses the centre (`fmt_complex`). Saving under an existing name replaces that place and its render. The status line reports "Saved the place 'NAME' with its render (N MB)". If the render can't be written, the place is still saved, any half-written file is deleted, and the status says so.
- **Saving (Video, "Save as place"):** adds the selected waypoint as a place named "<path name> <waypoint number>", without a render. (The waypoint's frame is widescreen, and Explore only opens square renders.)
- **What a place line holds:** the line also holds an order, coefficients and colouring. Going to a place never uses them. They're stored because saved video paths use the same line format for their waypoints.
- **Going to a place (Explore):** see [7.6](#76-saved-places-panel). The colouring stays as it is.
- **Using a place (Video):** sets only the selected waypoint's centre and half-width.
- **Hovering a place's name:** shows its centre, its half-width, and the saved render's size, order and coefficients, if it has one. Work this out only while hovering, since it reads the file's header.
- **Deleting (Explore):** **x**, then **Delete** or **Keep**. Deleting removes the place and its render file.
- **Sizes:** a saved render takes about 24 bytes per pixel at order 3: 25 MB at 1024², 400 MB at 4096².

### 6.6 Renders kept in memory

Every finished render is kept in a shared memory, so the same search never runs twice. This was in the old video maker; now Explore uses it too **(New for Explore)**.
- **Key:** a render is filed under its order, its coefficients (with their max) and the exact bits of its sample points (`RenderKey`). A render with the same key is the same search.
- **Size:** up to **3 GB**, least recently used dropped first, counting 8 bytes per axis value and residual, and 4 per coefficient. **(New)** Also count the colouring `Stats`, if they're kept with the render.
- **Explore uses it for:**
  - **Back**, which is instant when the earlier view's render is still there with the same settings. That replaces the old 30-render history.
  - **Refresh**, and changing a setting back again (e.g. order 3 → 4 → 3), which are instant.
  - **Opening a saved place's render**, which is put into the memory too.
- **Video uses it** as before:
  - Preview renders, including the waypoints' frames rendered for the held colour scale, are kept and reused.
  - **Render video** takes the waypoints' colour levels from memory where it can.
- It's cleared when the program closes.

### 6.7 Folders and file names

- **Images** (Explore's Save PNG) and **videos** (Video's output folder) default to the default save folder.
- **Decision:** the old programs used a `Results` folder above the program if there was one, otherwise the folder they were started from. For people downloading from GitHub, the new default is:
  - Windows: `Pictures\Algebraic Fractals` for images and `Videos\Algebraic Fractals` for videos;
  - macOS: `~/Pictures/Algebraic Fractals` and `~/Movies/Algebraic Fractals`.

  A `Results` folder above the program still wins, for the developer's own setup. The folders are created when the first file is saved.
- Both folders are text boxes you can type any path into. **(New, Decision)** Add a **Browse…** button with a native folder picker (the `rfd` crate).

### 6.8 Remembered settings (New)

**Decision.** The old programs started from their defaults every time, apart from places and saved paths. The new one remembers between sessions, in `settings.tsv` next to `places.tsv` (`key<TAB>value` lines, unknown keys ignored):
- the last mode;
- Explore: the search settings, auto ticks, target density, resolution, zoom factor, Show zoom box, the style and the image folder. It reopens on the last view, re-rendered.
- Video: the whole path and its settings, in `.path` format ([11](#11-files-on-disk)), as an unnamed "last session" path, plus the output settings and folder.

**Reset all settings** (in a small menu on the top bar) goes back to the defaults in this spec.

---

## 7. Explore mode

### 7.1 On start

It shows the home view (−2 … 2), order 3, integer coefficients up to 12 with 0 allowed and 0 can be a root, resolution 256 × 256, zoom × 4, default colouring. It starts rendering straight away, which takes a fraction of a second. (With remembered settings: the last ones instead.)

### 7.2 Mouse on the image

| action | effect |
|---|---|
| click | choose the centre of the next zoom (the yellow box moves there) |
| double-click | choose the centre and Run at once |
| mouse wheel | change the zoom factor: × 1.15 per notch (`1.15^(dy/40)`), kept within 1–1000 |
| hover | the status bar shows the point, its residual, its polynomial, length and height |

- **The zoom box:** yellow (255, 200, 40), 1.5 px. It's the next view's square, with a 12 px cross at its centre. Hidden by unticking **Show zoom box**.
- **Hover text:** `X = <fmt_complex>    residual 1.234e-5    p(x) = 3x^4 - 2x^2 + 1    length 6  height 3`. The polynomial is written highest power first, with `x^k` powers, and coefficients of ±1 left out except the constant (`poly_string`). The pixel is the nearest one to the mouse.
- The texture uses nearest-neighbour sampling, so pixels stay sharp when enlarged.
- Before the first render finishes, the image area shows "Rendering…".

### 7.3 Search panel

Controls, in order:
1. **Order:** number box (1–16) and an **auto** tick-box.
2. **Max coefficient:** number box (1–1 000 000) and an **auto** tick-box. Only one of the two autos can be on: ticking one unticks the other. A box is greyed while its auto is on.
   - Hover text for auto: "Choose this automatically, so that the view has the target density of algebraic numbers (predicted before rendering). Only one can be automatic."
3. **Target density:** shown only while an auto is on. A number box (0.01–1000, default 4), then "per 100 px²".
4. **Coefficients**, **allow 0**, **0 can be a root** ([6.2](#62-search-settings)).
5. **Resolution:** 128, 256, 384, 512, 768, 1024, 1536, 2048 or 4096, all square. Default 256.

Grey lines under the controls:
- `Allowed: …`.
- `1.95e+07 polynomials per pixel, searched in ~3.10e+02 steps` (`Plan::brute_len`, `Plan::steps`), or the plan's error in red.
- `Predicted: ~4.1 algebraic numbers per 100 px² in this view`, to 2 significant figures. Hover: "Estimated before rendering, from the number of allowed polynomials and where the roots of random polynomials lie. Usually within about 20%."
- In orange, if auto fell short: `Auto: <note>`.
- `Order and coefficient changes re-render this region automatically; resolution changes need Run or Refresh.`

**When it re-renders by itself:**
- Changing the order or any coefficient setting re-renders the current view. It waits until the value has stayed the same for **400 ms**, so dragging a number doesn't start a render for every step. It keeps the view.
- Changing the resolution does **not** re-render, so you can explore at low resolution and press Refresh or Run to go high.
- **With an auto on,** the auto value is worked out again whenever its inputs change: the view, the resolution, the target, which auto is on, and the other settings. If that changes the order or coefficients, it re-renders (after the same pause).
- Auto works from the view being *requested*, so a render in progress already has the right value.

### 7.4 View panel

- **Centre** and **Half-width** text boxes. They show the current view's exact values, and refill whenever the view changes.
  - Typing a new value and pressing **Enter** or **Go** goes there. Go is greyed if the text can't be read.
  - Red "can't read that number" when the text can't be read; grey "(edited, press Go)" when it differs from the current view.
  - Hover: "Type a complex number, e.g. 0.5+0.8i, -1.2-3e-4i, 2i or 0.5, 0.8, then press Enter or Go" and "Distance from the centre to the edge of the image".
- **zoom ×:** slider 1–1000, log scale, default 4.
- `Next: <fmt_complex of the box's centre>` and `half-width <hw/zoom>` (4 significant figures, scientific).
- Buttons:
  - **Run (zoom in)** (Enter): render the yellow box. With no click, it zooms into the current centre.
  - **Refresh:** render the current view again with the current settings.
  - **Back** (Backspace; greyed when there's nothing to go back to): the previous view. The last 30 views are remembered. If that render is still in memory with the current settings, it shows instantly; otherwise it's rendered again. (Auto first works out its value for that view.)
  - **Zoom out:** half-width × max(zoom, 2), same centre.
  - **Reset:** back to the home view.
- **Show zoom box** tick-box (on).
- **While rendering:** a progress bar `NN%  ~Ts left` (from the rows finished so far) and **Cancel**. Cancel stops the render, keeps the old image, says "Cancelled", and doesn't re-render for the settings that were pending.
- **While opening a saved render:** "Opening the saved render (W x W)…" instead of the progress bar.
- **The old image stays on screen** until the new one is ready.

Run, Zoom out, Reset, Go and going to a place all add the view being left to the Back list, and clear the clicked target.

### 7.5 Render finished

The status line says `Rendered WxH in 1.23s`, or `Opened the saved render, WxH in 0.4s`. The colouring `Stats` are computed on arrival. Errors show as `Error: <message>`.

### 7.6 Saved places panel

- A name box (hint "name this view") and **Save**. Enter in the box saves. Save is greyed until an image is shown. Hover: "Remember this view's centre and half-width, and its render, so it opens again straight away without searching. Going to a place keeps the colouring you have. Saving with an existing name replaces it."
- "None yet. Name the current view and press Save." when there are no places.
- One row per place: a small **x** (delete), then the name as a button (shortened with "…" if too long). Click to go there. Hover shows the place's info ([6.5](#65-saved-places)).
- **Going to a place with a saved render** (`renders/<name>.algf` exists, with a square, non-legacy header):
  1. The order, coefficients and resolution switch to the ones in the file's header, so the controls match the picture.
  2. Auto is turned off, with " (auto order / max coefficient turned off, to keep its settings)" added to the status.
  3. The file is read on a background thread and checked to really be that view (`View::shown_by`: centre and half-width within 10⁻⁶ of the half-width).
  4. If it doesn't match, or can't be read, the status says so and suggests Refresh.
  5. The colouring is unchanged.
- **Going to a place without a saved render:** goes to its centre and half-width with the current settings (auto included), and renders.

**Decision:** adopting the saved render's settings is more than "location and half-width only". The alternative is to keep the current settings, use the saved render only when they match it, and search otherwise.

### 7.7 Display panel

[6.4](#64-colouring-the-display-section), with one style for the whole of Explore.

### 7.8 Save panel

- **Folder** text box ([6.7](#67-folders-and-file-names)).
- **Save PNG** (greyed until an image is shown): writes the shown render at its full resolution with the current colouring. The status says `Saved <path>` or `Could not save <path>: <error>`.
- **File name:** `alg_o<order>_m<max><set tag>[_nz][_r0]_<re:+.8><im:+.8>i_hw<hw:.3e>_<px>px_<colour by>_<colormap>[_n<n>].png`
  - set tags: none for integers, `p` primes, `f` Fibonacci, `o` odd, `t` triangular, `s` squares, `c` cubes, `b` powers of 2;
  - `_nz` = 0 not allowed, `_r0` = c0 = 0 allowed;
  - `_n<n>` only when colouring by 1/residual^n;
  - e.g. `alg_o4_m12_+0.50000000+0.80000000i_hw7.500e-1_1024px_pow_inferno_n3.png`.
- **(New, Decision)** Compress the PNG (zlib, via the `png` crate or `miniz_oxide`). The old writer stored it uncompressed, about 50 MB at 4096². Keep the carried-over uncompressed writer only if you'd rather not add a dependency.

### 7.9 Help text

At the bottom of the panel, in grey:

```
Click: choose the next centre
Double-click: zoom in there now
Mouse wheel: change zoom factor
Enter: Run   Backspace: Back
Hover: shows that pixel's polynomial
```

### 7.10 Status bar

- **Line 1:** the hover text, or "Hover over the image to see each pixel's polynomial".
- **Line 2,** in grey: the latest status message.

---

## 8. Video mode

### 8.1 On start

The path zooms from the home view into the flower around (1 + i√2)/3, a root of 3x² − 2x + 1:
- **Waypoints:** start: centre 0, half-width 2; target: centre 1/3 + (√2/3)i, half-width 0.002. Both have auto max coefficient on (initial value 12, replaced by auto straight away) and default colouring.
- **Path:** zoom per frame 1.01; pan speed 1% per frame.
- **Search:** order 3, integers, 0 allowed, 0 can be a root, target density 4. Reuse on, coefficient steps medium, fade on, hold the colour scale off.
- **Output:** 16:9, 1280 × 720, no smoothing, 30 fps, quality high, file name `zoom`, PNG frames off.
- **Path name:** "my zoom". Mark the waypoints on.

The start's frame is previewed straight away.

### 8.2 Waypoints panel

**The list:** one selectable row per waypoint: `1. Start: 0.0000+0.0000i, hw 2.00e0, max 6`, then `2. Waypoint: …`, and the last is `N. Target: …`. Hover: "Edit this waypoint and preview its frame". Clicking selects it, stops picking, and previews **its frame**: the one frame where the video shows it exactly.

**Buttons:**
- **Insert waypoint:** adds a waypoint halfway (in frames) along the segment after the selected one (before it, if the target is selected). It's placed where the video already passes, with that frame's colouring, rounded max coefficient and pan speed. Its auto is on only if both ends' are, and its segment keeps the flower size if the old segment did. The video doesn't change until you move it. It becomes selected.
- **Add at end:** a new target after the last: a copy of it (view, colouring, max coefficient, auto, pan speed, keep flower size). It becomes selected. Hover: "Add a new target after the last waypoint, a copy of it. Then move it with Pick on image."
- **Delete** (greyed with only 2 waypoints): removes the selected one. The segments either side merge, with the earlier one's pan speed and flower setting.
- **Reverse:** runs the whole path backwards (waypoints, pan speeds, flower settings, max coefficients, autos, colourings). The same waypoint stays selected.

**Saving and opening paths:**
- A **name** box (default "my zoom"), **Save path** (replaces a saved path of the same name; an empty name says "Give the path a name first"), and an **Open…** dropdown listing the saved paths alphabetically ("No saved paths yet" if none).
- A path saves every waypoint with everything set at it (view, max coefficient and auto, pan speed, keep flower size, colouring), plus zoom per frame, target density, fade, reuse, coefficient steps, hold the colour scale, order and coefficients.
- Opening one replaces everything those cover, shares the first waypoint's whole-video colouring choices with all the waypoints ([8.8](#88-display-colouring-per-waypoint)), and selects the start.
- Status: `Saved the path 'NAME' (N waypoints)`, `Opened the path 'NAME' (N waypoints)`, or the error.
- **(New, Decision)** A **Delete** item for saved paths. The old program couldn't delete them except by deleting the file.

**The selected waypoint:**
- **Centre** and **Half-width** text boxes. Changes apply as you type, as soon as the text can be read. Red "can't read that centre or half-width" while it can't. Hover: "Half the frame's height (the explorer's half-width). Widescreen frames show more at the sides."
- **Max coefficient:** a number box (1–1 000 000) and **auto** (default on).
  - The box is greyed while auto is on.
  - If the segment *before* this waypoint keeps the flower size, the box is greyed and "keeps the flower size from <i>" replaces the auto tick-box.
  - Hovers: "Choose the max coefficient that gives this waypoint's view the target density of algebraic numbers (see Search). Along a zoom in it never falls." and "The frames between waypoints follow a smooth curve through their max coefficients".
  - An orange `Auto: <note>` appears under it when auto fell short.
- **Pick on image** (a toggle, labelled "Picking: click or drag" while on): see [8.7](#87-the-image-and-picking).
- **Zoom in x2** / **Out x2:** halve or double the half-width.
- **Centre on flower:** moves the centre exactly onto the simplest quadratic number within half a frame height of it (`video::simplest_quadratic`, heights up to 1 000 000). That's the centre of the largest flower there. The status says `Waypoint 3 centred on <complex_text>, a root of 3x² - 2x + 1`, or "No quadratic number near waypoint 3 (it's too close to the real axis?)".
- **from a saved place…** dropdown: sets the waypoint's centre and half-width only. Its colouring and max coefficient stay. Hovering an entry shows the place's info. With no places it shows "No saved places yet: save some in Explore".
- **Save as place:** [6.5](#65-saved-places).
- **(New)** **Open in Explore:** [9](#9-moving-between-the-modes-new).

**The segment to the next waypoint** (shown unless the target is selected):
- **Pan speed to N:** 0.01–50 % of the frame height per frame (default 1%). Hover explains that zooming and panning happen together, and the segment takes whichever needs more frames.
- **Keep flower size to N:** see [8.4](#84-the-max-coefficient).
- Grey: `To waypoint N: <frames> frames (<duration>)`.

**For the whole video:**
- **Zoom per frame:** × 1.001–4 (default 1.01, 3 decimals). Hover: "How much each frame magnifies the last (for the whole video). 1.01 is slow and smooth, 1.05 is quick."
- An orange warning if any waypoint's half-width is below 10⁻¹² of its centre's size: "That's beyond 64-bit precision: the deepest frames will look blocky".

### 8.3 How the path moves

This is carried over in `video::ZoomPath`. It must stay exactly as it is, since saved paths and the tests depend on it.
- **Frames per segment:** each segment from view `(c0, h0)` to `(c1, h1)` has `steps = ceil(max(|ln(h0/h1)| / ln(rate), P / s))` frames, at least 1. `s` is the segment's pan speed, and `P` the pan in frame heights: `|c1 − c0| · |ln(h0/h1)| / 2|h0 − h1|`, or `|c1 − c0| / 2h` for a pure pan (equal half-widths, to within 10⁻⁶).
- **Within a segment:** frame `j` has half-width `h0·(h1/h0)^(j/steps)`. The centre is `c1 + (c0 − c1)·(h − h1)/(h0 − h1)`, or linear in `j` for a pure pan. So the zoom is steady, the next waypoint stays put on screen as the view closes in on it, and the centre crosses the screen at a steady speed.
- **Waypoints:** each one is shown exactly in one frame (`stop_frame(i)`). The total is `Σ steps + 1` frames.

### 8.4 The max coefficient

- **At each waypoint**, it's set by hand or by **auto**.
- **What auto does** (`auto_maxes`, to be moved into the library unchanged):
  - It runs `choose_auto` on the waypoint's frame region at the video's resolution, with no work limit.
  - Along a segment that zooms in, the result is raised to at least the previous waypoint's value. Along a zoom out, it's lowered to at most it.
  - It's worked out again whenever its inputs change: the views, which waypoints have auto, the hand-set values, the flower settings, the order, the coefficient set, the target, the resolution and the shape.
- **Keep flower size** (per segment): the waypoint at the end of such a segment gets `keep_flower(previous max, previous view, this view)`: the previous max times the zoom. That holds whether its auto is on or not. If that max can't be searched, the note says so.
- **Between waypoints** (`max_at`): a monotone cubic curve (PCHIP, Fritsch–Carlson) through the waypoints' `ln M` against frame number. Between two waypoints `M` rises by the same factor every frame, and it never overshoots.
- **Fractional values and fading:** `M` is fractional between waypoints, but only whole coefficients exist.
  - **Fade in new coefficients** on: a frame is rendered at the two values either side and cross-fades them, so new stars fade in.
  - Without reuse, those values are members of the coefficient set (`fade_between`). With reuse, they're rungs of the ladder (`rung_between`).
  - A frame exactly on a value needs one render.
  - Fading off: the nearer value is used.
- **Cross-fades** are done in display brightness (8-bit values), not linear light, so new stars brighten at a steady rate.

### 8.5 Search panel

- **Order:** 1–16 (default 3).
- **Target density:** 0.01–1000 per 100 px² of video (default 4). Hover: "The density of algebraic numbers that auto aims for at the waypoints. Auto picks whatever max coefficient that needs, however slow: check the render time."
- **Coefficients**, **allow 0**, **0 can be a root:** as in [6.2](#62-search-settings). The max is per waypoint, not here.
- **Allowed:** `allowed_text`, using the start's max coefficient.
- **Reuse renders between frames** (default on). Hover: "Render each part of the plane once, finely enough for several consecutive frames, and make those frames by averaging it down: several times faster, and anti-aliased. The max coefficient then moves in steps … Untick to render every frame separately."
- **Coefficient steps** (only with reuse): fine / **medium** / coarse.
  - Each step allows about 13% / 28% / 65% more polynomials (`g` = 0.125 / 0.25 / 0.5).
  - Each option is labelled with both percentages for the current order, e.g. "medium: 28% more polynomials (max +6.4%)".
- **Fade in new coefficients** (default on).
- **Auto everywhere:** turn auto on at every waypoint.

**Reusing renders** (carried over in `video::reuse`). The algorithm must not change:
- **The ladder:** every member of the set while they're more than `e^(g/(n+1))` apart, then values about that far apart, plus the waypoints' own values.
- **Patches:** fixed square grids at integer multiples of a spacing, each at one rung, covering a run of frames.
- **Planning:** each patch is extended while its pixels per frame fall and it stays under 6 frames of pixels (`PATCH_FRAMES`) and 30 million pixels (`MAX_RENDER_PIXELS`).
- **Making a frame:** crop the patch to the frame; take histogram percentiles (`Stats::binned`); colour; average down in linear light (box filter weighted by overlap); then cross-fade.
- **Memory:** a patch is dropped after its last frame. Up to two patches are in memory at a time.

### 8.6 Output panel

| control | values (default) | notes |
|---|---|---|
| **Shape** | 16:9 (widescreen) (default), square | |
| **Resolution** | heights 360, 480, **720**, 1080, 1440, 2160 | shown as `W x H`. The width is rounded (`frame_width`). |
| **Smoothing** | none (default), 2 × 2 (4× slower), 3 × 3 (9× slower) | renders aa × aa samples per pixel and averages them in linear light. An option is greyed if it would exceed 30 million render pixels. If the resolution changes so the current choice no longer fits, it drops to the largest that does. Hover: "Render each frame more finely and average it down. Stars no longer flicker as they cross pixel boundaries, and the video compresses better." |
| **Frame rate** | 24, 25, **30**, 50, 60 fps | |
| **Quality** | good (QP 26), **high** (QP 20), very high (QP 16), near lossless (QP 10) | each entry's hover gives its quantiser. Hover: "Every frame is encoded at the same quality; busy frames make bigger files". |
| **Folder** | the default video folder | [6.7](#67-folders-and-file-names) |
| **File name** | `zoom` | `.mp4` is added. An existing file is never overwritten: `_2`, `_3`, … is added (also when a `<name>_frames` folder exists). |
| **Also save every frame as a PNG** | off | into `<name>_frames\frame_00000.png`, … next to the video |

### 8.7 The image and picking

- **The image:** the previewed frame, or while a video renders, its latest finished frame. Linear texture sampling.
- **Mark the waypoints** (Preview panel, on by default): each waypoint inside the previewed frame gets a 12 px cross and its number. Once it's more than 3 px and less than the frame's height, its view box is drawn too. The selected waypoint is yellow (255, 200, 40), the others white at 150 alpha. Marks are hidden while picking.
- **Picking** (Pick on image on):
  - A frame-shaped box follows the cursor, sized as the previewed frame divided by the **pick zoom**, labelled `x4.000`. The **mouse wheel** changes the pick zoom by × 1.15 per notch, within 0.1–10 000 (default 4).
  - **Click:** that box becomes the selected waypoint's view.
  - **Drag:** draw a box (keeping the frame's shape); it becomes the view on release. A box under 4 px keeps the old half-width.
  - **Shift-click:** moves only the centre.
  - Hold Shift to hide the box and see only the crosshair.
  - **Esc** cancels.
  - After a pick, picking stops, the status says `Waypoint N set to <centre>, half-width <hw>`, and the waypoint's new frame is previewed.
- Screen and plane coordinates come from the previewed frame's region.

### 8.8 Display: colouring per waypoint

The heading is "Display at waypoint N". Under it, in grey: "Colour by, colormap, invert, shading and log scale apply to the whole video. The other settings are set at each waypoint, and change smoothly from one waypoint to the next."
- **For the whole video:** colour by, colormap, invert, shading, log scale and Smooth stars. Changing one at the selected waypoint copies it to every waypoint (`share_choices`).
- **Per waypoint:** n, max %, min %, brightness, contrast, gamma, saturation, colour cycles and colour shift.
- **Between waypoints** (`style_at`): each number follows the same kind of monotone curve as the max coefficient, through its values at the waypoints. n, gamma and cycles are interpolated on a log scale, the others directly.
- If the previewed frame isn't the selected waypoint's, an orange note says "The preview shows a frame between waypoints i and i+1, with the settings in between".
- **Hold the colour scale** (whole video, default off): see below.
- **Use this colouring at every waypoint:** copies the selected waypoint's colouring to all of them.

**Hold the colour scale** (`video::held_scale`, carried over):
- Each waypoint's frame is rendered on its own (never from patches), and its `Levels` are measured.
- **Length/height:** one fixed range for the whole video, from the lowest value at any waypoint to the highest.
- **Residual brightness:** the percentiles at min % and max % move steadily from one end of each segment to the other.
- The preview shows the held scale too. The waypoints' `Levels` are cached (the 64 most recent) under their order, coefficients and frame jobs.

### 8.9 Preview panel

- A **frame** slider over the whole video, with **|<** (first frame) and **>|** (last frame).
- **Mark the waypoints** tick-box ([8.7](#87-the-image-and-picking)).
- **While rendering:** a progress bar "rendering frame k…".
- **The frame's details,** in grey:

  ```
  Frame k: centre <fmt_complex>
  half-width 1.2345e-3, order 3
  max coefficient 43.6 (fading from 41 to 46: 52%)        (or "43 (rendered at 43)")
  rendered in 0.42 s
  ```

- **When it re-renders:** whenever the frame number, the path or anything else that decides what the frames show changes, after a **400 ms** pause. Colouring changes only recolour.
- **What a preview does:**
  - It renders exactly as the video will: the same patch windows with reuse (so the same sample points), the same fades and smoothing, and the held scale.
  - It takes renders from memory where it can, and keeps everything it renders.
  - The time it takes (for the frame's own uncached renders) calibrates the render-time estimate.
- An older preview from a path with a different number of waypoints keeps its colours until the new one arrives.

### 8.10 Status bar

One line:
- while picking: "Click to make the box the waypoint's view (mouse wheel: box size), or drag a box. Shift-click: move only the centre. Esc cancels.";
- otherwise, the latest status message, or "Set the waypoints, preview frames and adjust the colours, then Render video".

### 8.11 Video panel and rendering

**Before rendering:**
- `N frames of W x H, <duration> at F fps`.
- `File size: roughly N MB`, from bits per pixel per frame: good 1.5, high 2.3, very high 2.9, near lossless 3.7. Hover: "Star fields need a lot of data to stay sharp; the real size depends on how busy the frames are".
- `Rough render time: <duration>`, once worked out in the background (`frame_secs`, scaled by the latest preview's time per unit of work). Hover: "Scaled from the preview frame's render time by the work each frame needs. Views near the real axis can take longer."
- **Render video:** a large, bold button.

**Durations** read like `45 s`, `12 min` or `3 h 20 min` (`duration_text`).

**While rendering:**
- All the settings above are locked (greyed). Display stays usable, but it doesn't affect this video: the colouring was fixed when Render was pressed.
- A progress bar `frame k of N, ~T left`, and a thin bar for the current render's rows.
- Grey: `elapsed T, last frame 1.2 s, 140 MB so far, max coefficient 43.6`.
- **Stop:** finishes the current frame, then saves the video so far as a shorter video. The status says "Stopping after the current frame…".
- The image shows each frame as it finishes.

**Time left:** each frame's time is estimated in advance, from its search work (with reuse, a patch counts at the first frame that needs it, plus about 40 ns per pixel per render). The time so far is scaled by (estimated total − estimated so far) / estimated so far.

**Each frame** (`make_video`, to be moved into the library):
1. With the colour scale held, any waypoint levels not known yet are rendered first.
2. For each frame:
   - the renders it needs: with reuse, any of its patches not yet rendered; otherwise its region at each max coefficient, with smoothing;
   - colouring with `style_at` and the held scale if on;
   - averaging down and cross-fading;
   - optionally the PNG;
   - encoding.
3. **When it ends:**
   - `Done in T (N MB): <path>`;
   - stopped early: `Stopped after k of N frames (T); saved the video so far (N MB): <path>`;
   - stopped before any frame: the file is deleted and the status says "Stopped before the first frame was finished".
4. Afterwards the preview is rendered again.

**Encoding** (`video::Mp4Out`, carried over):
- H.264 via OpenH264, compiled into the program.
- BT.709 limited-range I420.
- Fixed quantiser (`QpRange(qp, qp)`, quality mode), no frame skipping, a keyframe every 2 s.
- **Background detection and adaptive quantisation off.** This is what fixed the "chunks of stars jumping" glitch; don't change it.
- Muxed with the `mp4` crate (AVCC samples, SPS/PPS in `avcC`). A stopped video is still finished properly, so it plays.

**Closing the window** while a video renders loses that video, since the file is only playable once finished. **(New, Decision)** Ask first: "A video is rendering. Stop it and save what's done, or keep rendering?" with **Stop and save**, **Keep rendering** and **Quit without saving**.

---

## 9. Moving between the modes (New)

The point of combining the programs.

- **Explore → Video:** a **Send to video** menu in Explore's View panel:
  - **as the start** / **as the target:** replaces that waypoint's view;
  - **as a new target:** adds a waypoint after the last.

  Each carries the view and Explore's current colouring. The waypoint's max coefficient is set to auto. If the video's search settings (order, coefficient set, allow 0, 0 can be a root) differ from Explore's, Video mode shows a note with **Use Explore's search settings**. It then switches to Video and selects the waypoint.
- **Video → Explore:** **Open in Explore** (Waypoints panel) opens the previewed frame in Explore, as a square view:
  - its centre and half-width (half the frame height);
  - the video's order and coefficients, with the frame's max coefficient rounded, and auto off;
  - the frame's colouring.

  Use it to look closely at a frame at high resolution, or to save it as a place with its render.
- **Places** are one list, so a place saved in one mode appears in the other at once. (The old programs only saw each other's places after a restart, apart from the video maker re-reading the file when saving one.)

**Decision:** whether these bridges carry the colouring and search settings as above, or only the view.

---

## 10. The command-line tool

`algfrac.exe` is carried over unchanged (`src/bin/algfrac.rs`):

```
algfrac.exe --order 4 --maxcoef 32 --centre 1.9,0.19 --halfwidth 0.16 --width 1024 --out spiral
```

It writes `spiral.png` and `spiral.algf`, and prints its settings, the steps per pixel, the predicted density, the memory needed, and progress every 2 s with the time left.

| option | meaning | default |
|---|---|---|
| `--centre RE,IM` `--halfwidth H` | a square region | |
| `--re MIN,MAX` `--im MIN,MAX` | explicit ranges (non-square is fine) | `-2,2` / `-2,2` |
| `--order N` | highest power | 3 |
| `--maxcoef M` | largest \|c\| | 8 |
| `--coefs SET` | `ints`, `primes`, `fib`, `odd`, `triangular`, `squares`, `cubes`, `pow2` | `ints` |
| `--nozero` | 0 not allowed | |
| `--zeroroot` | c0 = 0 allowed | |
| `--legacy` | the conjugate quadratic variant: `min \|c0 + c1·conj(X) + c2·X²\|`, order 2, c1 ≠ 0 | |
| `--width W` | pixels across | 512 |
| `--colour pow\|log\|len\|hgt` | what to plot | `log` |
| `--n N`, `--max P` (or `--clip P`), `--min P` | as in Display | 1, 99.5, 0 |
| `--colormap NAME` | any of the 23 | `bone` |
| `--invert`, `--noshade`, `--logpoly`, `--sharp` | invert; no shading; log length/height; no Smooth stars | |
| `--gamma`, `--cycles`, `--shift`, `--brightness`, `--contrast`, `--saturation` | as in Display | 1, 1, 0, 0, 1, 1 |
| `--out NAME`, `--nodata`, `--threads T` | output name; skip the `.algf`; worker threads | `algfrac_out`, all cores |

The `--legacy` variant exists only on the command line.

---

## 11. Files on disk

The new program must read everything the old programs wrote, and write in the same formats, so nothing a user has saved is lost.

**The settings folder** is `%APPDATA%\AlgebraicFractals\` on Windows, `~/Library/Application Support/AlgebraicFractals/` on macOS, and `$XDG_CONFIG_HOME/AlgebraicFractals/` (or `~/.config/…`) elsewhere. It holds:

| file | format |
|---|---|
| `places.tsv` | one place per line, tab-separated; `#` lines are comments. Columns: name, re, im, half-width, order, set name, `1` (unused), max, zero (0/1), colour by, n, max %, colormap, invert, shade, zero root, min %, gamma, cycles, shift, brightness, contrast, saturation, log scale, smooth stars. Columns were only ever added at the end, and missing ones take defaults (`Place::from_line`). Control characters in names become spaces. |
| `renders/<name>.algf` | a saved place's render. `<name>` has `\ / : * ? " < > \|` and control characters replaced by `_` (`file_safe`). |
| `paths/<name>.path` | a saved video path (format below) |
| `settings.tsv` | **(New, Decision)** remembered settings ([6.8](#68-remembered-settings-new)) |

**`.path` files:**

```
# comment line
rate<TAB>1.01
target<TAB>4
fade<TAB>1
reuse<TAB>1
step<TAB>1
hold<TAB>0
flower<TAB>0,1            one 0/1 per segment
wp<TAB><auto 0/1><TAB><pan speed><TAB><a places.tsv line, named "waypoint N">
...
```

- The order and coefficient set come from the first waypoint line, and each waypoint's max coefficient from its own line.
- **Defaults for anything missing:** rate 1.01 (clamped to 1.001–4), target 4 (clamped to 0.01–1000), fade and reuse on, step 1, hold off, flowers off, pan 0.01.
- Unknown keys are ignored. Older files have `work`, which is no longer used.
- A path needs at least two waypoints.

**`.algf` render files** (`src/algf.rs`), little-endian:

```
"ALGF"  u32 version = 2
u32 width, height, order; i32 maxcoef; i32 reserved = 1
u32 coefset (0 ints, 1 primes, 2 fib, 3 odd, 4 triangular, 5 squares, 6 cubes, 7 pow2)
u32 flags (bit 0 conjugate variant, bit 1 zero not allowed, bit 2 c0 = 0 allowed)
f64 re_axis[width], f64 im_axis[height] (top first)
f64 residual[height·width] (row-major, top row first)
i32 coeffs[height·width·(order+1)]
```

Reading checks the size and refuses a truncated file.

---

## 12. The engine: what must stay true

The engine is carried over unchanged. These properties are tested and must stay true. `docs/DEVELOPER.md` will explain them in full, from the old README.

- **Exact results:**
  - the residuals match brute force to rounding error;
  - **ties** within `64·ε·Σ maxcoef·|X|^i` go to the polynomial with the smallest Σ|ci|;
  - the highest nonzero coefficient is made positive;
  - `X = 0` is special-cased.
- **The searches:**
  - meet in the middle, with a bucketed table and pruned queries;
  - the lattice search on two neighbouring powers, which falls back to the direct search near the real axis on a work budget;
  - a warm start from the left neighbour;
  - rows start from 1 + x + … + xⁿ, so a view always renders the same way whatever the threads do.
- **Tables** have at most 2²⁶ entries, built in chunks above that (max coefficient 4096+ at order 3). They're allocated only when needed.
- **Mirror symmetry:** a view centred on the real axis computes the top half and mirrors it.
- **Tiles for deep views:** one search per tile, listing every polynomial within `r0 + 2Lh` of the tile's centre. Each pixel picks from that list by the same rule. The picture is identical to searching every pixel. This is chosen automatically (`choose_method`).
- **Smooth stars:** pixels whose best polynomial's linear zero is within 4 pixels are averaged over the cell (8 × 8 near the zero, 4 × 4 further out). A star's total brightness then stays within about 5% as it crosses a pixel, against 86% from the centres.
- **Percentiles:** `Stats::new` sorts every pixel. `Stats::binned` uses a 65 536-bin histogram, exact to about 10⁻⁴ in ln(residual).
- **Performance targets** (12-core PC; the new program must be no slower):
  - order 4, M = 32, 256²: 0.7 s;
  - order 3, M = 3000, 1280 × 720 at width 10⁻⁷: 0.03 s;
  - recolour 1024²: under 10 ms.

---

## 13. Decisions for you

| # | question | my default |
|---|---|---|
| 1 | Names: the app `Algebraic Fractals.exe` (Cargo `algebraic-fractals`), the command line `algfrac.exe`, the repo `algebraic-fractals` | as stated |
| 2 | Licence | **MIT OR Apache-2.0** (the usual Rust choice). The H.264 encoder (OpenH264) is BSD-licensed, but H.264 is patented. The README will say that the compiled-in encoder isn't covered by Cisco's patent licence, which only covers Cisco's own binaries. |
| 3 | Going to a place with a saved render adopts its order, coefficients and resolution, and turns auto off ([7.6](#76-saved-places-panel)) | adopt them |
| 4 | Default folders for images and videos ([6.7](#67-folders-and-file-names)) | Pictures / Videos (Movies on Mac) `\Algebraic Fractals`, unless there's a `Results` folder above the program |
| 5 | Browse… buttons for folders (adds the `rfd` crate) | yes |
| 6 | Remember settings between sessions ([6.8](#68-remembered-settings-new)) | yes |
| 7 | Compressed PNGs ([7.8](#78-save-panel)) | yes |
| 8 | Ask before closing while a video renders ([8.11](#811-video-panel-and-rendering)) | yes |
| 9 | Delete saved paths from the Open… menu | yes |
| 10 | The bridges between modes carry colouring and search settings ([9](#9-moving-between-the-modes-new)) | yes |
| 11 | Explore keeps renders in the shared 3 GB memory, so Back and changing settings back are instant ([6.6](#66-renders-kept-in-memory)) | yes |
| 12 | An app icon | yes (a small star-field image, drawn once from a render) |
| 13 | Automatic GitHub builds for Windows and macOS on each release tag ([15.3](#153-releases-on-github)) | yes, so no Mac is needed for Mac builds |
| 14 | Open in Explore mode, or in the mode used last | the mode used last |
| 15 | Keep the command-line tool in the release downloads | yes |

---

## 14. Tests

Run with `cargo test --release --lib --bins`. Plain `cargo test` also runs the doc comments' formulas as code.

**Carried over with the code** (already passing here):
- **engine:** both searches against brute force (orders 2–6, every coefficient set, with and without c0 = 0); the tile search's list against brute force; tiled renders equal to per-pixel renders (depths 10⁻³ to 10⁻⁷, across the real axis, outside the unit circle); table chunking.
  - Ignored: `bench_deep`, `diag_deep_lines`.
- **colour:** lookup tables equal the per-pixel pipeline; Smooth stars keep their brightness; binned percentiles equal sorting.
  - Ignored: `recolour_speed`.
- **density:** densities integrate to d roots; the root finder.
- **algf:** round trip; a truncated file is refused.
- **video:** path ends and speeds, waypoints, frame sizes, pan speed, smooth curves, style blending, max coefficients, flower size, finding quadratic numbers, cross-fade, held scale, downsampling keeps brightness, writes a playable mp4 (up to 3840 × 2160).
- **reuse:** ladders, plans cover the frames, same grid gives the same picture, windows and averaging.
- **common:** parsing complex numbers; a place's line round trip.

**To port from the old video maker** (with the logic moved into the library):
- `render_memory`;
- `paths_save_and_load`;
- `flower_sets_the_next_max`;
- `short_zoom_video_reusing` and `short_zoom_video_frame_by_frame`. These are end-to-end: auto never falls, maxes never fall, and the first decoded frame matches what was rendered;
- `faint_rows_of_stars_survive_encoding`: the worst 16 × 16 block's error stays below 3;
- ignored: `bench_reuse`, `flower_video`, `diag_wobble`, `diag_decode`, `diag_encoder`.

**To port from the old explorer:**
- `auto_density`: auto lands near the target and never falls as you zoom in;
- `saved_place_renders`.

**New:**
- the shared render memory gives Explore an instant Back;
- `settings.tsv` round trip;
- old `places.tsv` and `.path` files from the old programs load unchanged. Check in a sample of each as test data.

**Equivalence with the old programs:**
- The command line with the same arguments writes byte-identical `.png` and `.algf` files.
- A short video with the same settings gives identical frames (compare the PNG frames).

**GUI checks by hand** before each release:
- explore, zoom, Back;
- save and go to a place, with and without a saved render;
- send a view to Video;
- pick a waypoint;
- preview with Hold the colour scale;
- render a short video and play it;
- stop a video early and play it.

---

## 15. Building and releasing

### 15.1 Windows

- **Toolchain:** as now. Rust's GNU toolchain (`x86_64-pc-windows-gnu`) plus WinLibs MinGW-w64, installed with `winget install BrechtSanders.WinLibs.POSIX.MSVCRT --scope user`.
- **`build.ps1`:**
  - puts `cargo` and MinGW on PATH;
  - runs `cargo build --release`;
  - puts `Algebraic Fractals.exe`, `algfrac.exe` and `USER-GUIDE.md` into `dist\`;
  - zips them as `dist\AlgebraicFractals-<version>-windows-x64.zip`.
- **`.cargo/config.toml`** (carried over): targets `x86-64-v2`, and links MinGW's C++ runtime statically. Each `.exe` is self-contained. Check with `objdump -p … | findstr "DLL Name"`.
- **Release profile:** thin LTO, one codegen unit, `panic = "abort"`.
- `#![windows_subsystem = "windows"]` on the app (no console window), but not on `algfrac.exe`.

### 15.2 macOS

`build-mac.sh` (rewritten from the old one) does the following:
- builds `aarch64-apple-darwin` and `x86_64-apple-darwin` and joins them with `lipo`;
- makes `Algebraic Fractals.app` (bundle id `io.github.<user>.algebraic-fractals`, minimum macOS 11) and signs it ad hoc;
- adds `algfrac`;
- zips them into `dist-mac/`.

Without an Apple Developer ID, Gatekeeper blocks the app until the user allows it once. `docs/MAC.md` explains how.

### 15.3 Releases on GitHub

- **Version:** in `Cargo.toml`. A release is a tag `v<version>`.
- **(New, Decision) GitHub Actions:**
  - `.github/workflows/release.yml` runs on a `v*` tag. It builds on `windows-latest` (installing MSYS2's MinGW, or using the MSVC toolchain with a static C runtime, whichever builds OpenH264 cleanly) and on `macos-latest` (both architectures). It runs the tests and attaches both zips to a draft release.
  - `ci.yml` runs the tests on each push.
- **Without Actions:** build locally and upload the zip by hand.
- **Commit author email:** commits show it publicly once pushed. If you'd rather keep your address private, use GitHub's noreply address (Settings → Emails) for this repository before the first push.

---

## 16. Documentation

- **`README.md` (GitHub front page):** three example pictures, then:
  - a one-paragraph description;
  - **Download** (links to the latest release) and **Quick start**;
  - what Explore and Video do, in a few bullets each;
  - "What is computed";
  - "Why the patterns look the way they do" (dark holes, the real axis, flowers, order 3, symmetries, star size, density), from the old root README;
  - further reading;
  - building from source (a short paragraph pointing to DEVELOPER.md);
  - the licence.
- **`docs/USER-GUIDE.md`:** one guide for the app.
  - Getting started.
  - The window and the modes.
  - Shared concepts: search settings, density and auto, colouring, places.
  - Explore.
  - Video (waypoints, how the path moves, keeping a flower the same size, the max coefficient, reuse, output, preview, colouring per waypoint, holding the colour scale, render times).
  - Moving between modes.
  - Tips.
  - Troubleshooting (both old tables, merged).

  Merge the old two guides' text, keeping the measured tables (render times, file sizes, quality).
- **`docs/DEVELOPER.md`:** the old developer README, updated for the new layout and names.
- **`docs/MAC.md`:** the old README-MAC, updated for one app.
- **Writing style:** British spelling, plain words, short sentences, numbers measured rather than guessed, as in the old guides.

---

## 17. Order of work

1. **The library:** move `Frames`, `auto_maxes`, `frame_secs`, `RenderKey`/`RenderMemory`, `SavedPath` and `make_video` into the library, with their tests. Make `make_video` report progress through a callback, without egui.
2. **The app shell:** window, top bar with the mode tabs, shared state, status bar, `widgets.rs` (coefficient rows, Display, colormap strip, places menu).
3. **Explore mode**, at parity with the old explorer, using the shared render memory.
4. **Video mode**, at parity with the old video maker.
5. **The bridges** between modes, remembered settings, folder defaults and Browse…, compressed PNGs, the close prompt, the icon.
6. **Tests:** ported and new; equivalence checks against the old programs.
7. **Build scripts, docs, licence, README pictures, GitHub workflows.** Then a release build and the manual checks in [14](#14-tests).

Each step ends with the tests passing and a commit.

---

## 18. Appendix: constants

| constant | value | where |
|---|---|---|
| `MAX_COEF` | 1 000 000 | largest max coefficient |
| `AUTO_MAX_STEPS` | 2·10⁴ | Explore's auto work limit per pixel |
| default target density | 4 per 100 px² | both modes |
| Explore resolutions | 128, 256, 384, 512, 768, 1024, 1536, 2048, 4096 | |
| Explore history | 30 views | Back |
| zoom factor | 1–1000, default 4, wheel × 1.15 per notch | Explore |
| settle delay | 400 ms | before an automatic re-render (both modes) |
| `TABLE_MAX` | 2²⁶ entries per thread | engine |
| tiles | about 256 polynomials each, at most 64 px across; split above 4096; per-pixel below 8 px or 4 × 4 | engine |
| `RENDER_MEMORY` | 3 GB | shared render memory |
| levels cache | 64 waypoint levels | Video |
| video heights | 360, 480, 720, 1080, 1440, 2160 | Video |
| frame rates | 24, 25, 30, 50, 60 | Video |
| quality | good QP 26 (1.5 bpp), high QP 20 (2.3), very high QP 16 (2.9), near lossless QP 10 (3.7) | Video |
| smoothing | 1, 2, 3 (aa × aa) | Video |
| `MAX_RENDER_PIXELS` | 30 000 000 | Video renders and patches |
| `PATCH_FRAMES` | 6 | Video reuse |
| `COEF_STEPS` | fine 0.125, medium 0.25, coarse 0.5 | Video reuse |
| `DEFAULT_PAN` | 0.01 frame heights per frame | Video |
| zoom per frame | 1.001–4, default 1.01 | Video |
| pick zoom | 0.1–10 000, default 4 | Video |
| keyframe interval | 2 s | encoder |
| time per pixel per render (estimate) | 40 ns | Video time estimates |
| precision warning | half-width < 10⁻¹² × \|centre\| | Video |
