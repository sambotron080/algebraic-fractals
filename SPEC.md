# Algebraic Fractals: specification for the combined program

This document describes the new program completely: what it does, how it looks, how it behaves, how it's built and how it's shipped on GitHub. It replaces the old explorer (`algfrac-gui.exe`) and the old video maker (`algfrac-video.exe`) with one app: an explorer with a slim toolbar, and a video window that opens from it.

**How to read and edit it**
- Edit anything. Delete what you don't want, add what's missing, and change any default.
- **(New)** marks a feature the old programs didn't have. **(Changed)** marks one that works differently from the old programs. Everything else is a feature the old programs have, and the new one must keep.
- **Decision** marks a choice I made that you may want to change. They're also collected in [section 13](#13-decisions-for-you).
- The old programs, in `..\algfrac`, are the reference implementation. Where this spec says nothing about a detail, the new program behaves exactly like the old one. Where they disagree, this spec wins.
- Section 3 lists the code that has already been copied over. Everything else will be written new.

---

## Contents

1. [What the program is](#1-what-the-program-is)
2. [What gets shipped](#2-what-gets-shipped)
3. [Code already carried over](#3-code-already-carried-over)
4. [How the app is organised](#4-how-the-app-is-organised)
5. [The main window](#5-the-main-window)
6. [Shared by the explorer and videos](#6-shared-by-the-explorer-and-videos)
7. [The explorer in detail](#7-the-explorer-in-detail)
8. [The video window](#8-the-video-window)
9. [How the explorer and videos work together](#9-how-the-explorer-and-videos-work-together)
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

The program is one window:
- **The explorer** fills the window. Click and zoom through the plane, change the order, max coefficient, root density and resolution from a slim settings strip, save the view as a PNG, and save **locations** (a centre and half-width) into **groups**. A group is usually the waypoints of one future zoom video.
- **Everything else is in popups** opened from the toolbar: the polynomial settings, the colours, the view's exact numbers, the saved locations, and the folders.
- **Videos** are made in a **video window** that opens from the toolbar's Video menu. Its tabs set the path (start, finish and waypoints), the colouring (one for the whole video), the polynomial settings and the output. A strip of small, fast renders of every waypoint stays in view and updates as settings change. A video can be rendered at once, or saved as a **video project** and rendered later.

A separate **command-line tool**, `algfrac.exe`, renders single images for scripts and very large renders, and **(New, Decision)** renders saved video projects.

**Platforms.** 64-bit Windows 10/11 is the main target. macOS 11+ (Apple silicon and Intel) is the second target. Linux should build from source, but no release builds are made for it.

### 1.1 What changes from the old programs

- **(Changed)** One window instead of two programs. The explorer is the main window; the old video maker's controls are in the video window.
- **(Changed)** The explorer's side panel is gone. Only order, max coefficient, the two auto ticks, root density and resolution stay on screen. The rest is in toolbar popups.
- **(Changed)** Saved places become **locations**: only a name, a group, a centre and a half-width. Going to one keeps your search settings and colouring. Locations don't keep a saved render (`.algf`). The old places are brought in as the group "Old places".
- **(Changed)** A video has **one colouring for the whole video**. The old per-waypoint colouring, and the smooth blending of it between waypoints, are gone.
- **(Changed)** The length and height scale can be fixed for the whole video, either from the waypoints or by hand.
- **(Changed)** Views for waypoints are chosen in the explorer (from a group, or "Add the explorer's view"), not by picking on the video's preview image. The old pick box, pick zoom and drag-to-pick are gone.
- **(Changed)** The old large frame preview is replaced by the **waypoint strip**: a small render of every waypoint, side by side, always visible.
- **(New)** Video projects save every setting, so a video can be rendered later, from the app or the command line.
- **(New)** A choice of output format: MP4, PNG frames, or both.

---

## 2. What gets shipped

### 2.1 The GitHub repository

The repository is this folder. Its layout when finished:

| path | contents |
|---|---|
| `README.md` | the GitHub front page: example pictures, what it is, download links, quick start, why the patterns look the way they do, further reading |
| `LICENSE` (or `LICENSE-MIT` and `LICENSE-APACHE`) | the licence: **Decision**, see [13](#13-decisions-for-you) |
| `docs/USER-GUIDE.md` | the user guide, rewritten for the new layout from the old `README-GUI.md` and `README-VIDEO.md` |
| `docs/DEVELOPER.md` | building, the source layout, how the search works, the library, file formats and tests, from the old developer `README.md` |
| `docs/MAC.md` | first-run instructions for macOS (Gatekeeper), from the old `README-MAC.md` |
| `docs/images/` | a few example pictures for the README, compressed |
| `SPEC.md` | this document |
| `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml` | the Rust package |
| `src/` | the source (see [4.1](#41-source-layout)) |
| `examples/density_check.rs` | checks density predictions against exact counts |
| `tests/data/` | sample files written by the old programs (`places.tsv`, a `.path`), to check they still load |
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
| `src/algf.rs` | `src/algf.rs` | reading and writing `.algf` render files (now used only by the command line) | none |
| `src/video/mod.rs` | `src/bin/algfrac-video/video.rs` | the zoom path (`ZoomPath`), smooth curves through the waypoints, fading, frame colouring, the held colour scale, downsampling, `simplest_quadratic`, and the `.mp4` writer (`Mp4Out`) | `algfrac::` became `crate::`; declares `pub mod reuse`. **For one colouring per video:** `style_at` (blending the waypoints' colourings) and its test were removed, and `held_scale` takes one `Style` instead of one per waypoint. |
| `src/video/reuse.rs` | `src/bin/algfrac-video/reuse.rs` | reusing renders between frames: the ladder, patch planning, cropping and averaging down | `algfrac::` became `crate::` |
| `src/common.rs` | `src/bin/common/mod.rs` | `View`, saved locations, number parsing and formatting, `choose_auto`, `allowed_text` | the egui widgets were left out (they'll be rewritten in the app). **For locations:** `Place` became `Location` (name, view), filed in a `Group`, in `locations.tsv`, with `load_groups`, `store_groups` and `old_places_group`. The old place-line reader stays as `OldPlace::from_line`, only for bringing in old places and old `.path` files. `place_render_path` and `Place::info` were removed (`view_info` replaces the latter); `places_path` became `settings_dir`. Tests: `location_lines` and `old_place_lines`. |
| `src/bin/algfrac.rs` | `src/main.rs` | the command-line tool | none |
| `examples/density_check.rs` | `examples/density_check.rs` | density predictions against exact counts | none |
| `.cargo/config.toml` | `.cargo/config.toml` | portable CPU target and the static C++ runtime for Windows | none |

Not copied, to be rewritten: the two windows (`algfrac-gui.rs`, `algfrac-video/main.rs`) and the egui parts of `common/mod.rs`. The video maker's non-window logic (`Frames`, `auto_maxes`, `make_video`, `frame_secs`) will be moved into the library as it's ported, mostly unchanged, so it can be tested without a window ([4.1](#41-source-layout)). Its `SavedPath` becomes `VideoProject` ([11.3](#113-video-projects)).

---

## 4. How the app is organised

### 4.1 Source layout

| file | contents |
|---|---|
| `src/lib.rs` | the `algfrac` library: `engine`, `colour`, `density`, `png`, `algf`, `common`, `video`, `settings`, `icon` |
| `src/video/mod.rs`, `src/video/reuse.rs` | carried over (section 3) |
| `src/video/frames.rs` | **to write:** `Frames` (everything that decides what the frames show), `auto_maxes`, `frame_secs` |
| `src/video/project.rs` | **to write:** `VideoProject` (every setting of a video) and its files, and bringing in old `.path` files |
| `src/video/make.rs` | **to write:** `make_video`, the render loop, with progress reported through a callback instead of egui, so the app and the command line share it |
| `src/settings.rs` | **to write:** the remembered settings, `settings.tsv` ([6.8](#68-remembered-settings-new)) |
| `src/main.rs` | **to write:** the app: creates the window, owns the shared state |
| `src/app/explore.rs` | **to write:** the main window: toolbar, settings strip, image, status bar |
| `src/app/popups.rs` | **to write:** the toolbar's popups: View, Polynomials, Colours, Save location, Locations, Folders |
| `src/app/video.rs` | **to write:** the video window: its tabs, the waypoint strip, the bottom bar |
| `src/app/widgets.rs` | **to write:** controls used in several places: the coefficient-set rows, the colour controls, the colormap strip, the location and group menus |
| `src/bin/algfrac.rs` | the command-line tool (carried over; gains `video`, [10](#10-the-command-line-tool)) |

The library stays free of GUI code, so the command line and the tests use it without a window. Only `src/main.rs` and `src/app/` use `eframe`/`egui`.

### 4.2 Windows and popups

- **The main window** is the explorer ([5](#5-the-main-window)).
- **Popups** are small floating panels (`egui::Window`) over the explorer's image. Each opens and closes with its toolbar button (the button stays highlighted while it's open), has a × to close, can be dragged, and stays where it was put for the rest of the session. Several can be open at once. **Esc** closes the one most recently opened. They don't block anything: the explorer keeps working underneath.
- **The video window** is a separate window of its own (an egui viewport) **(Decision)**, so it can be moved beside the explorer or onto a second screen. It's 1100 × 750 at first, resizable, at least 800 × 560, titled "Video: <project name>". At most one is open. The alternative is a large popup inside the main window.

### 4.3 Threads

- The **UI thread** draws the windows and never runs a search. A slow step, such as estimating a video's render time, runs on a background thread.
- **Searches** run on worker threads, one per core (`std::thread::available_parallelism`), through `engine::render_axes`. Each job has a `cancel` flag and a `rows_done` counter for progress.
- **The explorer** has at most one render running. Starting a new one cancels the old one.
- **The video window** has at most one waypoint-strip job ([8.8](#88-the-waypoint-strip)), at most one frame preview ([8.4](#84-path-tab)), and at most one video render. The strip and the preview are cancelled when their settings change again, or when a video render starts.
- **The explorer and a video render can run at the same time.** They share the cores, so each runs slower while the other is busy. While a video renders, its progress also shows in the explorer's toolbar ([5.1](#51-layout)).
- Background threads ask for a repaint when they finish. While a job runs, the UI repaints every 100 ms for the progress bars (200 ms while a video renders).

### 4.4 Shared state

Owned by the app and used by the explorer and the video window:
- the saved locations (one list in memory, written to `locations.tsv` whenever it changes);
- the density predictor (`Predictor`, which caches its sampled root densities);
- the thread count;
- the folders for images and videos ([6.7](#67-folders-and-file-names));
- (New, Decision) the remembered settings ([6.8](#68-remembered-settings-new)).

---

## 5. The main window

### 5.1 Layout

```
┌───────────────────────────────────────────────────────────────────────────────────────┐
│ ◀ Back  ⌂ Home  ⊖ Zoom out  ⟳ │ View… │ ★ Save location  Locations… │ ⬇ Save image │    │
│ Polynomials…  Colours… │ Video ▾ │ ☰                    video: frame 120 of 900, ~3 min  │  toolbar
├───────────────────────────────────────────────────────────────────────────────────────┤
│ Order [ 3 ] ☐ auto   Max coefficient [ 12 ] ☐ auto   Root density [ 4 ] per 100 px²   │
│ (predicted ~4.1)   Image [square ▾] [512 x 512 ▾]        ▓▓▓▓▓▓░░░ 62%  ~2 s left  [Cancel]     │  settings strip
├───────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                       │
│                                  the image                                            │
│                                                                                       │
├───────────────────────────────────────────────────────────────────────────────────────┤
│ X = +0.5000… residual 1.2e-5  p(x) = 3x^2 - 2x + 1  length 6  height 3 │ Rendered 256x256 in 0.4 s │ status bar
└───────────────────────────────────────────────────────────────────────────────────────┘
```

The toolbar and the settings strip are each one row when the window is wide enough, and wrap onto a second row when it isn't.

- **Toolbar** ([7.2](#72-toolbar)): navigation, the popups, saving, the Video menu and the ☰ menu. On the right, while a video renders: "video: frame k of n, ~T left". Clicking it shows the video window.
- **Settings strip** ([7.3](#73-settings-strip)): order, max coefficient, their auto ticks, root density and resolution, then the render's progress.
- **Image:** the rest of the window, background grey 16. The image is scaled to fit, keeping its aspect ratio, and centred. Popups float over it.
- **Status bar:** one line. On the left, the hover text; on the right, the latest status message ([7.12](#712-status-bar)).
- **Window:** 1200 × 900 at start, title "Algebraic Fractals". The old explorer's side panel is gone, so the image gets the whole width.
- A window icon, and an icon for the `.exe` and the Mac app **(New)**: a render of the lotus flower around (−1)^(1/3) = (1 + i√3)/2, a root of x² − x + 1 (order 3, max coefficient 60, half-width 0.013, −log(residual), gamma 1.6, contrast 1.3: `icon::lotus`). The window's icon is rendered at start; `examples/make_icon.rs` writes `assets/icon.ico` (16–256 px, put into the `.exe` by `build.rs`) and `assets/icon.png` (made into the Mac app's icon by `build-mac.sh`).

### 5.2 Keyboard

Keys act only while no text box has the keyboard focus, and only in the window that has it.

| key | where | effect |
|---|---|---|
| Enter | explorer | Run: zoom into the yellow box |
| Backspace | explorer | Back |
| Home | explorer | Home view **(New)** |
| Ctrl+S | explorer | Save image **(New)** |
| Ctrl+L | explorer | Save location **(New)** |
| Esc | explorer | close the most recently opened popup |
| Ctrl+S | video window | save the video project **(New)** |
| Esc | video window | close the video window (asks first if there are unsaved changes) |

Enter in a text box does that box's own action (Go, Save, and so on), as described with each box.

---

## 6. Shared by the explorer and videos

### 6.1 Views and coordinates

- A **view** is a centre `cr + ci·i` and a **half-width** `hw`.
- In the explorer, the image is square, widescreen (16:9) or the shape of the window (see [7.3](#73-settings-strip)). As in a video, `hw` is half its **height**: it spans `ci ± hw`, and `cr ± hw·W/H` across (just `cr ± hw` when square).
- In a video, `hw` is half the frame's **height**. A widescreen frame also spans `cr ± hw·16/9` horizontally. So a location saved in the explorer is the same height in a video, with more at the sides.
- Row 0 of a render is the top: the largest imaginary part.
- The **home** view is centre 0, half-width 2.
- **Typing a centre:** these forms are accepted: `0.5+0.8i`, `-1.2 - 3e-4i`, `2i`, `-i`, `0.3`, `0.5, 0.8`, `1.5E+2-2j`. Spaces are ignored, `j` counts as `i`, and `*i` as `i` (`common::parse_complex`). A half-width must be a finite number above 0.
- **Showing a centre:**
  - in text boxes, the shortest exact form, such as `0.5-0.25i` (`complex_text`), so copying it out and back loses nothing;
  - on labels and hovers, a fixed form with 12 decimals, such as `+0.500000000000 + 0.250000000000i` (`fmt_complex`);
  - in the waypoint table and on the waypoint strip, 4 decimals.
- **Precision:** coordinates are 64-bit floats. Below a half-width of about 10⁻¹² × |centre|, neighbouring pixels can't be told apart and the image turns blocky. The video window warns when a waypoint goes that deep, and the explorer shows the same warning in the status bar.

### 6.2 Search settings

| setting | values | meaning | where |
|---|---|---|---|
| **Order** | 1–16 | the highest power of `x` | explorer: settings strip; video: Polynomials tab |
| **Max coefficient** | 1–1 000 000 | the largest allowed `|c|` | explorer: settings strip; video: per waypoint, Polynomials tab |
| **Coefficients** | integers; 1 and primes; Fibonacci numbers; odd numbers; triangular numbers; squares; cubes; powers of 2 | which sizes are allowed, each with either sign, and 0. Every set contains 1. In the dropdown, hovering a set shows its first members, e.g. `0, ±1, ±2, ±3, ±5, ...` | explorer: Polynomials popup; video: Polynomials tab |
| **0 can be a root** | on/off (default on) | whether 0 counts as an algebraic number, so that it lights up. On, the polynomial `x` is tried at every pixel (residual \|X\|) as well; off, 0 sits in a dark hole. **(Changed)** Polynomials with `c0` = 0, `x·q(x)`, are never searched: near q's roots inside the unit circle they'd beat the simpler q, so pixels showed polynomials with a spare factor of x. | as Coefficients |

**(Changed)** 0 is always an allowed coefficient: the old "allow 0" setting is gone. Settings, projects and old paths that turned it off are read with it on. (The command line keeps `--nozero`.)

The max coefficient box can be dragged. The drag speed is 1% of the value (at least 0.2 per pixel), so large values change proportionally. Order drags at 0.05 per pixel.

### 6.3 Root density and auto

How a star field looks depends mostly on how many algebraic numbers land per pixel. `density::Predictor` estimates this before rendering, in **algebraic numbers per 100 square pixels**, usually to within about 20%. In the program this is called the **root density**.

`common::choose_auto` picks either the **order** or the **max coefficient** whose predicted density is closest to the **target root density** (in log ratio). It keeps all the other settings.
- **(Changed)** The explorer and videos both use no work limit (`max_steps` = ∞; the old explorer's `AUTO_MAX_STEPS` is gone), so auto can go up to max coefficient 1 000 000, however slow that is to render. When even that falls short, the note says "has too many polynomials to search".
- The first prediction for a new coefficient set takes up to about 70 ms. After that, each view takes about 3 ms.

### 6.4 Colour controls

A **style** (`colour::Style`) decides how a render is coloured. Changing it recolours instantly, without searching again. The same controls appear in the explorer's Colours popup ([7.7](#77-colours-popup)) and the video window's Colour tab ([8.5](#85-colour-tab)), each with its own style.

| control | range (default) | what it does |
|---|---|---|
| **Colour by** | 1 / residual^n (default); −log(residual); length (sum \|ci\|); height (max \|ci\|) | what's plotted. Power: brightness `(res/res_high)^(−n)`, clamped, stretched so `res_low` is black, computed in log space. Log: `−ln res` between the percentiles. Length/height: the polynomial's Σ\|ci\| or max\|ci\|, from the image's lowest to highest (or the video's fixed scale, [8.5](#85-colour-tab)). |
| **Colormap** | 23 maps (bone default) | classic: bone, gray, hot, copper, pink, spring, summer, autumn, winter, cool, jet, parula; modern: inferno, magma, plasma, viridis, turbo; rainbow/psychedelic: hsv, sinebow, neon, acid, candy, cosmic. hsv, sinebow, neon and acid are cyclic (their ends match). |
| colormap strip | | a 14 px strip under the colormap, 256 samples with every setting below applied: left = darkest pixels, right = brightest. Redrawn only when the style changes. |
| **n** | 0.05–5, log slider (1) | the sharpness exponent, for 1/residual^n and for shading |
| **max %** | 90–100 (99.5) | the brightness percentile that becomes full brightness |
| **min %** | 0–100 (0) | the brightness percentile that becomes black. It's kept at least 0.1 below max %. |
| **Smooth stars** | on/off (on) | average each pixel's brightness over the pixel instead of taking it at the centre (see [12](#12-the-engine-what-must-stay-true)) |
| **Invert colormap** | on/off (off) | swap the ends |
| **Shade by 1/res^n** | on/off (on) | length/height only: darken away from the stars |
| **Log scale** | on/off (off) | length/height only: spread the colormap over the log of the length/height |
| **brightness** | −0.5 to 0.5 (0) | lighten or darken every colour |
| **contrast** | 0 to 4 (1) | spread colours from mid-grey, or squash them towards it |
| **gamma** | 0.1–10, log (1) | above 1 brightens the faint points and midtones; black and full brightness stay put |
| **saturation** | 0 to 3 (1) | 0 = grey, above 1 = more vivid |
| **colour cycles** | 1–50, log (1) | run through the colormap several times: bands like contour lines |
| **colour shift** | 0 to 1 (0) | slide the colours along the colormap |
| **Reset colours** | button | everything back to the defaults |

**Greyed out:** n, max %, min % and Smooth stars when colouring by length/height with shading off (they only act on the residual). Shade and Log scale unless colouring by length/height.

**Layout:** the controls are in two groups, so the popup stays short: **What and which colours** (colour by, colormap and its strip, invert, shading, log scale) and **Adjust** (n, max %, min %, Smooth stars, brightness, contrast, gamma, saturation, cycles, shift). Each group is a collapsible header, both open at first.

**Hover texts** (keep them, worded as in the old programs):
- n: "Larger n makes the points near algebraic numbers sharper".
- max %: "Brightness percentile that becomes full brightness: 99.5 saturates the brightest 0.5% of pixels. Lower = brighter image".
- min %: "Brightness percentile that becomes black: 50 makes the darker half of the pixels black. Raise it to hide the background haze".
- Smooth stars, gamma, saturation, colour cycles, colour shift, shading and log scale: as in the old `common/mod.rs`.

**The order of operations** for each colour (`Style::colour`, carried over):
1. invert;
2. gamma;
3. cycles and shift (cyclic maps wrap; others go back and forth);
4. the colormap;
5. shading;
6. saturation (Rec. 601 grey);
7. contrast and brightness.

Recolouring uses lookup tables and all cores: 4–8 ms for 1024², about 45 ms for a 720p frame with Smooth stars.

### 6.5 Locations and groups (Changed)

A **location** is a saved view: a **name**, a **group**, a **centre** and a **half-width**. Nothing else is kept. They're stored in `locations.tsv` ([11](#11-files-on-disk)).

- **Groups** collect locations in an order. A group is usually the start, waypoints and finish of one zoom video, so the order matters: it's the order the video visits them. Every location is in exactly one group.
- **Unsorted** is the default group. It always exists, even when empty.
- **Saving** ([7.4](#74-save-location-popup)) adds the explorer's current view to a group, at the end.
- **Going to a location** goes to its centre and half-width with the explorer's current search settings (auto included) and colouring, and renders. The view left behind goes onto the Back list.
- **Hovering a location** shows its centre and half-width (`view_info`) and, if it's not the group's first, how far it zooms from the previous one in the group ("× 230 deeper than 'arm'").
- **Old places:** until there's a `locations.tsv`, the old programs' `places.tsv` is read and its places listed in the group **Old places**, keeping their names and views. Their order, coefficients and colouring are left behind. Their saved renders (`renders\*.algf`) aren't used, and aren't deleted. `places.tsv` itself is never changed.

### 6.6 No renders kept in memory

**(Changed)** Finished renders aren't kept for later, to keep the memory use down (the old video maker kept up to 3 GB, and the old explorer its last 30 renders). So:
- the explorer keeps only the render it shows; **Back** renders the earlier view again;
- the video window keeps each thumbnail's own renders (so colour changes only recolour), the frame preview's, and the waypoints' measured levels (small: 64 at most). A thumbnail moved to another place in the strip keeps its renders.

### 6.7 Folders and file names

- **Images** (Save image) and **videos** go to two folders, set in ☰ → **Folders…** ([7.10](#710-the--menu)). Each video project also keeps its own output folder, which starts as the videos folder ([8.7](#87-output-tab)).
- The old programs used a `Results` folder above the program if there was one, otherwise the folder they were started from. The new defaults are:
  - Windows: `Pictures\Algebraic Fractals` for images and `Videos\Algebraic Fractals` for videos;
  - macOS: `~/Pictures/Algebraic Fractals` and `~/Movies/Algebraic Fractals`.

  The folders are created when the first file is saved.
- Each folder is a text box you can type any path into. **(New, Decision)** Each has a **Browse…** button with a native folder picker (the `rfd` crate).

### 6.8 Remembered settings (New)

**Decision.** The old programs started from their defaults every time, apart from places and saved paths. The new one remembers between sessions, in `settings.tsv` next to `locations.tsv` (`key<TAB>value` lines, unknown keys ignored):
- the explorer: the search settings, the auto ticks, the target root density, the resolution, the zoom factor, Show zoom box, the style and the last view. It reopens there, re-rendered;
- the group last saved to;
- the two folders;
- which popups were open (they reopen in their default places);
- the video window: the project that was open, and whether the window was open. Unsaved changes are kept as a project named "(last session)" ([8.3](#83-projects-saving-and-opening)).

☰ → **Reset all settings** goes back to the defaults in this spec. It doesn't touch locations or saved video projects.

---

## 7. The explorer in detail

### 7.1 On start

It shows the home view (centred on i, half-width 0.75: clear of the bright points on the real axis), order 3, integer coefficients with auto max coefficient on (root density 4) and 0 can be a root, a square image of 512 × 512, zoom × 4, default colouring, no popups open. It starts rendering straight away, which takes a fraction of a second. (With remembered settings: the last ones instead.)

### 7.2 Toolbar

Left to right, as icon-and-word buttons, with hover text for each:

| button | effect |
|---|---|
| **▶ Run** (Enter) | zoom into the yellow box (as a double-click on the image does). Greyed until an image is shown. |
| **⏴ Back** (Backspace) | the previous view (the last 30 are remembered), rendered again (auto first works out its value for that view). Greyed when there's nothing to go back to. |
| **🏠 Home** (Home key) | back to the home view |
| **➖ Zoom out** | half-width × max(zoom, 2), same centre |
| **🔄 Refresh** | render the current view again with the current settings. Needed after changing the resolution ([7.3](#73-settings-strip)). |
| **🌸 Snap to flowers** **(New)** | a toggle (remembered). When on, a click puts the zoom box's centre on the simplest quadratic number within the zoom box's half-height of the clicked point (`video::simplest_quadratic`): the centre of the largest flower there. The status names its polynomial, or says there's none (too close to the real axis). |
| **View…** | opens the View popup ([7.5](#75-view-popup)), which also has **🌸 Centre on flower** **(New)**: the view's centre onto the simplest quadratic number within half its half-width |
| **⭐ Save location** (Ctrl+L) | opens the Save location popup ([7.4](#74-save-location-popup)). Greyed until an image is shown. |
| **📂 Locations…** | opens the Locations popup ([7.8](#78-locations-popup)) |
| **💾 Save image** (Ctrl+S) | saves the shown image as a PNG straight away ([7.13](#713-saving-images)). Greyed until an image is shown. |
| **Polynomials…** | opens the Polynomials popup ([7.6](#76-polynomials-popup)) |
| **🎨 Colours…** | opens the Colours popup ([7.7](#77-colours-popup)) |
| **🎬 Video ▾** | a menu ([7.9](#79-the-video-menu)) |
| **☰** | a menu ([7.10](#710-the--menu)) |

(The symbols are ones egui's built-in fonts have; ⌂, ⊖, ⟳ and ⋯ aren't among them.)

Back, Home, Zoom out, Run, Go and going to a location all add the view being left to the Back list, and clear the clicked target.

### 7.3 Settings strip

The only search settings on screen. Left to right:
1. **Order:** number box (1–16) and an **auto** tick-box.
2. **Max coefficient:** number box (1–1 000 000) and an **auto** tick-box. Only one of the two autos can be on: ticking one unticks the other. A box is greyed while its auto is on, and shows the value auto chose.
   - Hover text for auto: "Choose this automatically, so that the view has the target root density (predicted before rendering). Only one can be automatic."
3. **Root density:** a number box (0.01–1000, default 4), then "per 100 px²". It's the target for auto, so it's greyed while neither auto is on.
   - After it, in grey: "(predicted ~4.1)", the predicted density of the view with the current settings, to 2 significant figures. Hover: "Estimated before rendering, from the number of allowed polynomials and where the roots of random polynomials lie. Usually within about 20%."
   - When auto falls short, an orange **⚠** follows. Its hover is the note, e.g. "Auto: a max coefficient above 1000000 would get closer, but has too many polynomials to search".
4. **Image** **(Changed)**: the shape, then the size:
   - **square**: 128, 256, 384, 512, 768, 1024, 1536, 2048 or 4096 pixels (default 512);
   - **widescreen 16:9**: heights 360, 540, 720, 1080, 1440 or 2160 (640 × 360 to 3840 × 2160);
   - **fill the window**: the image area's own size in pixels (shown in grey). It re-renders by itself when the window changes size, once the size has stayed the same for 400 ms.
   Hover on the size: "Pixels. Changing it doesn't re-render: press Refresh or zoom in to use it."
5. **Progress,** while rendering: a progress bar `NN%  ~Ts left` (from the rows finished so far) and **Cancel**. Cancel stops the render, keeps the old image, says "Cancelled", and doesn't re-render for the settings that were pending. While nothing renders, the space is empty.

**When it re-renders by itself:**
- Changing the order, the max coefficient or any setting in the Polynomials popup re-renders the current view. It waits until the value has stayed the same for **400 ms**, so dragging a number doesn't start a render for every step. It keeps the view.
- Changing the square or widescreen size, or the shape, does **not** re-render (except to fill the window), so you can explore at low resolution and press Refresh or zoom in to go high. **Decision:** or re-render at once.
- **With an auto on,** the auto value is worked out again whenever its inputs change: the view, the resolution, the target, which auto is on, and the other settings. If that changes the order or coefficients, it re-renders (after the same pause).
- Auto works from the view being *requested*, so a render in progress already has the right value.
- **The old image stays on screen** until the new one is ready.

### 7.4 Save location popup

Opened by **★ Save location**. Small, near the button:
- **Name:** a text box, focused when the popup opens, with the hint "name this view". An empty name uses the centre (`fmt_complex`).
- **Group:** a dropdown of the groups, then **New group…**, which turns the dropdown into a text box for the new group's name. Default: the group last saved to (at first, Unsorted).
- The view, in grey: `centre <fmt_complex>, half-width <hw:.4e>`.
- **Save** (Enter): adds the location at the end of the group, closes the popup, and the status says `Saved 'NAME' in GROUP (location 4 of 4)`.
- **Same name in the same group:** the Save button reads **Replace** and the location keeps its place in the group, with the new view. Names may repeat across groups.
- Hover on Save: "Remember this view's centre and half-width. Going to it later keeps the search settings and colours you have then."

### 7.5 View popup

The exact numbers of the view and the zoom:
- **Centre** and **Half-width** text boxes. They show the current view's exact values, and refill whenever the view changes.
  - Typing a new value and pressing **Enter** or **Go** goes there. Go is greyed if the text can't be read.
  - Red "can't read that number" when the text can't be read; grey "(edited, press Go)" when it differs from the current view.
  - Hover: "Type a complex number, e.g. 0.5+0.8i, -1.2-3e-4i, 2i or 0.5, 0.8, then press Enter or Go" and "Distance from the centre to the edge of the image".
  - **Copy** puts `centre, half-width` on the clipboard as text.
- **Zoom ×:** slider 1–1000, log scale, default 4. The mouse wheel over the image changes it too.
- `Next: <fmt_complex of the box's centre>` and `half-width <hw/zoom>` (4 significant figures, scientific).
- **Show zoom box** tick-box (on).
- A warning, in orange, when the half-width is below 10⁻¹² × |centre|: "That's beyond 64-bit precision: the image will look blocky".

### 7.6 Polynomials popup

The search settings that aren't in the strip, and what they mean:
- **Coefficients** dropdown, **0 can be a root** ([6.2](#62-search-settings)).
- Grey lines:
  - `Allowed: …`;
  - `1.95e+07 polynomials per pixel, searched in ~3.10e+02 steps` (`Plan::brute_len`, `Plan::steps`), or the plan's error in red;
  - `Predicted: ~4.1 algebraic numbers per 100 px² in this view`;
  - in orange, if auto fell short: `Auto: <note>`.
- Changes re-render as in [7.3](#73-settings-strip).

### 7.7 Colours popup

The colour controls of [6.4](#64-colour-controls), with one style for the whole explorer. Changes recolour the shown image at once. It's taller than the other popups; it opens on the right of the image so it covers less of it.

### 7.8 Locations popup

Everything about saved locations. It's a scrolling list of groups, each a collapsible header:

```
▾ Spiral dive (4)                         [🎬 Make video]  [⚙]
  ☰  1  start           ⏶ ⏷  ✏  Move  ✖
  ☰  2  arm             ⏶ ⏷  ✏  Move  ✖
  ☰  3  inner arm       ⏶ ⏷  ✏  Move  ✖
  ☰  4  flower          ⏶ ⏷  ✏  Move  ✖
▸ Unsorted (2)
▸ Old places (17)
[ + New group ]
```

- **A location's row:** its number in the group, then its name as a button (shortened with "…" if too long). Click to go there ([6.5](#65-locations-and-groups-changed)). Hover shows its info. The row for the view the explorer is showing is highlighted.
  - **↑ ↓** move it within the group (greyed at the ends).
  - **✎** renames it in place (Enter to finish, Esc to cancel).
  - **Move** is a menu of the other groups, to move it to the end of one, and **New group…**.
  - **×** asks "Delete 'NAME'?" with **Delete** and **Keep**.
  - It can also be dragged to a new place in its group, or onto another group's header. **(Decision:** dragging as well as the buttons.)
  - **Set to this view** (in the row's right-click menu) replaces its centre and half-width with the explorer's current view, keeping its name and place.
- **A group's header:** its name and number of locations, then:
  - **🎬 Make video:** opens the video window with a new project from this group ([9](#9-how-the-explorer-and-videos-work-together)). Greyed for a group of fewer than 2 locations, with the hover "A video needs at least 2 locations: a start and a finish".
  - **⚙** menu: **Rename**, **Sort by depth** (widest first, so the video zooms in), **Reverse order**, **Delete group…** (asks, and deletes its locations too; Unsorted can't be deleted, only emptied).
  - Collapsed or open is remembered for the session.
- **+ New group:** asks for a name and adds an empty group at the end. Group names are unique.
- **Empty:** "No saved locations yet. Find a view you like and press ★ Save location."
- The groups are listed in the order they were made, with Unsorted first and Old places last.

### 7.9 The Video menu

- **Video window:** shows the video window with the project that was last open (or a new one, if none). Ticked while it's open.
- **New video:** opens the video window with a new project ([8.1](#81-a-new-project)).
- **New video from group ▸:** the groups with at least 2 locations. Same as the group's 🎬 Make video.
- **Open video project ▸:** the saved projects, alphabetically, with "(last session)" first if there is one. Each entry has a small × to delete it (asks first). Then, if there are any, **Old paths ▸**: the old video maker's saved paths, to bring in ([11.4](#114-old-files-brought-in)).
- **While a video renders:** "Rendering 'NAME': frame k of n" (greyed), and **Stop the render** (asks, then stops and saves what's done).

### 7.10 The ☰ menu

- **Folders…:** a small popup with the images folder and the videos folder: text boxes with **Browse…** ([6.7](#67-folders-and-file-names)).
- **Show zoom box** (tick).
- **Reset all settings** (asks first).
- **User guide:** opens the guide on GitHub in the browser.
- **About:** the version, the licence, and a link to the repository.

### 7.11 Mouse on the image

| action | effect |
|---|---|
| click | choose the centre of the next zoom (the yellow box moves there) |
| double-click | choose the centre and Run at once |
| mouse wheel | change the zoom factor: × 1.15 per notch (`1.15^(dy/40)`), kept within 1–1000. The status bar briefly shows "zoom × 5.3". |
| right-click | a menu **(New)**: **Zoom in here**, **Centre here** (same half-width), **🌸 Zoom into the flower here** and **🌸 Centre on the flower here** (onto the simplest quadratic number within the zoom box's half-height of the point), **Copy this point** (`complex_text` to the clipboard) |
| hover | the status bar shows the point, its residual, its polynomial, length and height |

- **The zoom box:** yellow (255, 200, 40), 1.5 px. It's the next view's square, with a 12 px cross at its centre. Hidden by unticking **Show zoom box**.
- **Hover text:** `X = <fmt_complex>    residual 1.234e-5    p(x) = 3x^4 - 2x^2 + 1    length 6  height 3`. The polynomial is written highest power first, with `x^k` powers, and coefficients of ±1 left out except the constant (`poly_string`). The pixel is the nearest one to the mouse.
- The texture uses nearest-neighbour sampling, so pixels stay sharp when enlarged.
- Before the first render finishes, the image area shows "Rendering…".

### 7.12 Status bar

- **Left:** the hover text, or "Hover over the image to see each pixel's polynomial".
- **Right,** in grey: the latest status message: `Rendered WxH in 1.23s`, `Saved <path>`, `Cancelled`, `Error: <message>`, and so on. The colouring `Stats` are computed when a render arrives.

### 7.13 Saving images

- **⬇ Save image** writes the shown render at its full resolution with the current colouring, to the images folder. The status says `Saved <path>` or `Could not save <path>: <error>`.
- **File name:** `alg_o<order>_m<max><set tag>[_nz][_r0]_<re:+.8><im:+.8>i_hw<hw:.3e>_<px>px_<colour by>_<colormap>[_n<n>].png`
  - set tags: none for integers, `p` primes, `f` Fibonacci, `o` odd, `t` triangular, `s` squares, `c` cubes, `b` powers of 2;
  - `_nz` = 0 not allowed, `_r0` = c0 = 0 allowed;
  - `_n<n>` only when colouring by 1/residual^n;
  - e.g. `alg_o4_m12_+0.50000000+0.80000000i_hw7.500e-1_1024px_pow_inferno_n3.png`.
  - An existing file is never overwritten: `_2`, `_3`, … is added.
- **(New, Decision)** The PNG is compressed (zlib, via the `png` crate or `miniz_oxide`). The old writer stored it uncompressed, about 50 MB at 4096². Keep the carried-over uncompressed writer only if you'd rather not add a dependency.
- **Save image as…** (right-click on the button): the same, but asks for the file name and folder with a native save dialog (`rfd`).

### 7.14 Help

When the mouse isn't over the image, the status bar's left side shows the short tips (on one line):

```
Click: choose the next centre    Double-click: zoom in there now
Mouse wheel: change zoom factor    Right-click: more
Enter: Run   Backspace: Back   Ctrl+L: save location   Ctrl+S: save image
```

---

## 8. The video window

### 8.1 A new project

A **video project** is every setting of one video: the path, the colouring, the polynomial settings and the output. The video window always shows one project.

A new project starts as:
- **From a group** ([9](#9-how-the-explorer-and-videos-work-together)): the group's locations, in order, as the waypoints: the first is the start, the last the finish. The project is named after the group.
- **Otherwise** ("New video"): the start is the home view (centre i, half-width 0.75), and the finish is the explorer's current view. If the explorer is at the home view, the finish is the flower around (1 + i√2)/3, a root of 3x² − 2x + 1: centre 1/3 + (√2/3)i, half-width 0.002. Named "my zoom".
- **Either way:**
  - every waypoint's max coefficient on auto, pan speed 1% per frame, keep flower size off;
  - zoom per frame 1.01;
  - the order, coefficient set, 0 can be a root and target root density copied from the explorer;
  - the colouring copied from the explorer;
  - reuse on, coefficient steps medium, fade on, steady brightness off, length/height scale fixed from the waypoints;
  - output: 16:9, 1280 × 720, no smoothing, 30 fps, quality high, MP4, file name = the project name, the videos folder.

The waypoint strip starts rendering straight away.

### 8.2 Layout

```
┌ Video: Spiral dive ──────────────────────────────────────────────────────────────────┐
│ Project [ Spiral dive      ]  [Save] [Save as…] [Open ▾] [New]        ● unsaved     │  project bar
│ [ Path ] [ Colour ] [ Polynomials ] [ Output ]                                       │  tabs
├──────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                      │
│   the tab's controls (scrolls)                                                       │
│                                                                                      │
├──────────────────────────────────────────────────────────────────────────────────────┤
│  ┌────────┐  12 s  ┌────────┐  8 s  ┌────────┐  20 s  ┌────────┐                     │
│  │        │ ─────▶ │        │ ────▶ │        │ ─────▶ │        │                     │  waypoint strip
│  └────────┘        └────────┘       └────────┘        └────────┘                     │  (always shown)
│  1 Start           2 arm            3 inner arm       4 Finish                       │
│  hw 2.0  max 6     hw 0.03  max 41  hw 4e-4 max 900   hw 2e-6 max 31 000             │
├──────────────────────────────────────────────────────────────────────────────────────┤
│ 1 340 frames · 44.7 s at 30 fps · 1280 x 720 · ~150 MB · ~25 min     [ ● Render now ] │  bottom bar
└──────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.3 Projects: saving and opening

The **project bar**, at the top of the window:
- **Project:** a text box with the project's name. It's also the file name the project saves under.
- **Save** (Ctrl+S): saves under the name, replacing a saved project of the same name. An empty name says "Give the project a name first". Status: `Saved the project 'NAME'`.
- **Save as…:** asks for a new name, then saves (asking before replacing one).
- **Open ▾:** the saved projects, alphabetically, each with a × to delete it (asks first); then **Old paths ▸** ([11.4](#114-old-files-brought-in)). Opening one replaces the window's project.
- **New ▾:** **Blank** ([8.1](#81-a-new-project)) or **From group ▸**.
- **● unsaved** shows while the project has changes since it was last saved or opened.
- **Leaving a project with unsaved changes** (Open, New, closing the window, or the explorer's 🎬 Make video): asks "Save changes to 'NAME'?" with **Save**, **Don't save** and **Cancel**. **(Decision:** or keep it silently as "(last session)".)
- On quitting the program, an unsaved project is kept as "(last session)", so nothing is lost ([6.8](#68-remembered-settings-new)).

Projects are saved in the settings folder ([11.3](#113-video-projects)), so they're listed wherever the program runs. **Export…** and **Import…** in the Open menu copy a project file to or from anywhere, to move it to another computer or share it.

### 8.4 Path tab

**The waypoint table:** one row per waypoint, in order. The selected row is highlighted, and so is its thumbnail in the strip.

| # | name | centre | half-width | to the next one: pan speed | keep flower size | frames (time) | |
|---|---|---|---|---|---|---|---|
| 1 | Start | text box | text box | 1% | ☐ | 360 (12 s) | ↑ ↓ × |
| 2 | arm | … | … | … | … | … | ↑ ↓ × |
| 3 | Finish | … | … | | | | ↑ ↓ × |

- **#:** the waypoint's number. The first is labelled Start and the last Finish, whatever their names.
- **Name:** the location's name it came from, or "waypoint N". Editable. It's only a label.
- **Centre** and **half-width:** text boxes; changes apply as you type, as soon as the text can be read. Red while they can't. Hover on half-width: "Half the frame's height (the explorer's half-width). Widescreen frames show more at the sides."
- **Pan speed** to the next waypoint: 0.01–50 % of the frame height per frame (default 1%). Hover: "Zooming and panning happen together. The segment takes whichever needs more frames: zooming at the zoom per frame, or panning at this speed."
- **Keep flower size** to the next waypoint: see [8.6](#86-polynomials-tab).
- **Frames (time):** the segment's frames and their duration at the video's frame rate.
- **↑ ↓** move the waypoint (greyed at the ends); **×** deletes it (greyed with only 2). The segments either side of a deleted waypoint merge, with the earlier one's pan speed and flower setting.
- Clicking a row selects it.

**Buttons under the table:**
- **Add the explorer's view:** a new waypoint after the selected one (or at the end, if the finish is selected), with the explorer's current view, auto max coefficient, default pan speed. It becomes selected.
- **Set to the explorer's view:** replaces the selected waypoint's view with the explorer's.
- **Show in the explorer:** the explorer goes to the selected waypoint's view (the frame's height becomes the explorer's square), with the explorer's own settings. The main window comes to the front.
- **Insert halfway:** adds a waypoint halfway (in frames) along the segment after the selected one (before it, if the finish is selected). It's placed where the video already passes, with that frame's rounded max coefficient and pan speed. Its auto is on only if both ends' are, and its segment keeps the flower size if the old segment did. The video doesn't change until you move it.
- **Centre on flower:** moves the selected waypoint's centre exactly onto the simplest quadratic number within half a frame height of it (`video::simplest_quadratic`, heights up to 1 000 000). That's the centre of the largest flower there. Status: `Waypoint 3 centred on <complex_text>, a root of 3x² - 2x + 1`, or "No quadratic number near waypoint 3 (it's too close to the real axis?)".
- **Reverse:** runs the whole path backwards (waypoints, pan speeds, flower settings, max coefficients, autos). The same waypoint stays selected.
- **Replace with a group ▸:** replaces all the waypoints with a group's locations, keeping every other setting.
- **Save as a group:** saves the waypoints as locations in a new group, named after the project (with " 2", " 3", … added if that group exists). Status: `Saved 4 locations in the group 'Spiral dive'`.

**For the whole video:**
- **Zoom per frame:** × 1.001–4 (default 1.01, 3 decimals). Hover: "How much each frame magnifies the last. 1.01 is slow and smooth, 1.05 is quick."
- An orange warning if any waypoint's half-width is below 10⁻¹² of its centre's size: "That's beyond 64-bit precision: the deepest frames will look blocky".

**Look at a frame (Decision, default on; also the Colour tab's example frame, [8.5](#85-colour-tab)):** the strip only shows the waypoints. To check the frames in between (fades, the max coefficient between waypoints), the Path tab ends with:
- a **frame** slider over the whole video, with **|<** and **>|**, and the frame's time (`0:12.4`);
- that frame, rendered exactly as the video will render it (patch windows with reuse, fades, smoothing and the colour scale), at the video's full resolution, shown up to 960 px wide (so its render time calibrates the estimate well);
- in grey: `Frame k: centre <fmt_complex>, half-width 1.2345e-3, max coefficient 43.6 (fading from 41 to 46: 52%), rendered in 0.42 s`;
- it re-renders 400 ms after the frame or anything that decides it changes; colour changes only recolour. Its render time calibrates the render-time estimate ([8.9](#89-the-bottom-bar-and-rendering)).

### 8.5 Colour tab

**One colouring for the whole video (Changed).** The colour controls of [6.4](#64-colour-controls), applied to every frame, on the left; **(New)** on the right, an **example frame**: the same frame and slider as the Path tab's "Look at a frame", recoloured as the controls change. The old per-waypoint settings and their blending between waypoints are gone. Changes recolour the waypoint strip (and the frame preview) at once, without searching again.

Buttons at the top:
- **Copy the explorer's colours:** replaces this colouring with the explorer's.
- **Use these colours in the explorer:** the other way round.

**Colour scale for the whole video.** A star field's brightness percentiles and its range of lengths and heights change from frame to frame, so a colouring that's right at one waypoint can drift. Two settings fix that:

- **Steady brightness** (on/off, **Decision:** default off): each waypoint's frame is rendered on its own, and its brightness percentiles (at max % and min %) are measured. Every frame then uses percentiles that move steadily from one end of its segment to the other, instead of its own. Hover: "Stops the brightness flickering as stars appear and leave. The percentiles are measured at the waypoints and change steadily between them." (`video::held_scale`, carried over.) Only for colouring by residual, or by length/height with shading on.
- **Length and height scale** (shown when colouring by length or height):
  - **Each frame its own:** each frame spreads the colormap from its own lowest length/height to its highest (as in the explorer).
  - **Fixed from the waypoints** (default): one range for the whole video, from the lowest value at any waypoint to the highest (`held_scale`), so a given length has the same colour in every frame. The range in use is shown in grey: `lengths 3 to 1 840, from the waypoints`.
  - **Fixed by hand:** **from** and **to** number boxes (1–10⁹, log drag) for the length (when colouring by length) or the height (by height). They start as the range from the waypoints. A **Measure** button puts the waypoints' range back in. Values outside the range take the colormap's end colours.
  - Hover: "Fixing the scale gives each length the same colour throughout the video, so colours don't shift as you zoom."

**How the waypoints' levels are measured:** for the waypoint strip, from the thumbnails themselves. For the frame preview and the video, from each waypoint's frame at the video's full resolution, rendered first when the video starts (and for the preview, when steady brightness or a fixed-from-the-waypoints scale is on). The two can differ a little, so the strip's brightness is a close guide, not exact. The levels are cached (the 64 most recent) under their order, coefficients and frame jobs.

### 8.6 Polynomials tab

**For the whole video:**
- **Order:** 1–16.
- **Coefficients**, **0 can be a root:** as in [6.2](#62-search-settings). `Allowed: …` uses the start's max coefficient.
- **Root density:** 0.01–1000 per 100 px² of video (default from the explorer, 4). Hover: "The density of algebraic numbers that auto aims for at the waypoints. Auto picks whatever max coefficient that needs, however slow: check the render time."
- **Copy the explorer's settings:** the order, coefficients and root density from the explorer.

**The max coefficient at each waypoint:** a table, one row per waypoint:

| # | name | max coefficient | auto | note |
|---|---|---|---|---|
| 1 | Start | 6 | ☑ | |
| 2 | arm | 41 | ☑ | |
| 3 | inner arm | 900 | ☐ | |
| 4 | Finish | 31 000 | ☑ | keeps the flower size from 3 |

- **Max coefficient:** a number box (1–1 000 000), greyed while auto is on (it shows auto's value).
- **Auto** (default on): "Choose the max coefficient that gives this waypoint's view the target root density. Along a zoom in it never falls."
- If the segment *before* the waypoint keeps the flower size, the box and the tick are greyed and the note says "keeps the flower size from <i>".
- **Note:** in orange, auto's note if it fell short.
- **Auto everywhere:** turns auto on at every waypoint.
- Hover on the table: "The frames between waypoints follow a smooth curve through these values."

**How the max coefficient moves** (carried over):
- **Auto** (`auto_maxes`, to be moved into the library unchanged):
  - runs `choose_auto` on the waypoint's frame region at the video's resolution, with no work limit;
  - along a segment that zooms in, the result is raised to at least the previous waypoint's value; along a zoom out, it's lowered to at most it;
  - is worked out again whenever its inputs change: the views, which waypoints have auto, the hand-set values, the flower settings, the order, the coefficient set, the target, the resolution and the shape.
- **Keep flower size** (per segment, set in the Path tab): the waypoint at the end of such a segment gets `keep_flower(previous max, previous view, this view)`: the previous max times the zoom. That holds whether its auto is on or not. If that max can't be searched, the note says so. Hover: "Grow the max coefficient with the zoom, so the dark flower around a quadratic number stays the same size on screen while the stars around it get denser."
- **Between waypoints** (`max_at`): a monotone cubic curve (PCHIP, Fritsch–Carlson) through the waypoints' `ln M` against frame number. Between two waypoints `M` rises by the same factor every frame, and it never overshoots.

**Rendering choices:**
- **Reuse renders between frames** (default on). Hover: "Render each part of the plane once, finely enough for several consecutive frames, and make those frames by averaging it down: several times faster, and anti-aliased. The max coefficient then moves in steps. Untick to render every frame separately."
- **Coefficient steps** (only with reuse): fine / **medium** / coarse.
  - Each step allows about 13% / 28% / 65% more polynomials (`g` = 0.125 / 0.25 / 0.5).
  - Each option is labelled with both percentages for the current order, e.g. "medium: 28% more polynomials (max +6.4%)".
- **Fade in new coefficients** (default on).
  - On: a frame between two allowed max coefficients is rendered at both and cross-fades them, so new stars fade in. Without reuse, those values are members of the coefficient set (`fade_between`); with reuse, they're rungs of the ladder (`rung_between`). A frame exactly on a value needs one render.
  - Off: the nearer value is used.
  - Cross-fades are done in display brightness (8-bit values), not linear light, so new stars brighten at a steady rate.
- Grey: the deepest waypoint's `polynomials per pixel, searched in ~N steps`.

**Reusing renders** (carried over in `video::reuse`). The algorithm must not change:
- **The ladder:** every member of the set while they're more than `e^(g/(n+1))` apart, then values about that far apart, plus the waypoints' own values.
- **Patches:** fixed square grids at integer multiples of a spacing, each at one rung, covering a run of frames.
- **Planning:** each patch is extended while its pixels per frame fall and it stays under 6 frames of pixels (`PATCH_FRAMES`) and 30 million pixels (`MAX_RENDER_PIXELS`).
- **Making a frame:** crop the patch to the frame; take histogram percentiles (`Stats::binned`); colour; average down in linear light (box filter weighted by overlap); then cross-fade.
- **Memory:** a patch is dropped after its last frame. Up to two patches are in memory at a time.

### 8.7 Output tab

| control | values (default) | notes |
|---|---|---|
| **File format** **(New)** | **MP4 video** (default); **PNG frames**; **MP4 and PNG frames** | MP4: H.264 in an `.mp4` file, plays everywhere. PNG frames: a folder of numbered, lossless PNGs (`frame_00000.png`, …), for editing in other software; large (a few MB each at 720p, compressed). Both: the two together. **Decision:** other formats (GIF, WebM/AV1, H.265) would each need a new encoder library; GIF suits star fields badly (256 colours). |
| **Shape** | 16:9 (widescreen) (default), square | **Decision:** add 9:16 (portrait, for phones) and 4:3? |
| **Resolution** | heights 360, 480, **720**, 1080, 1440, 2160 | shown as `W x H`. The width is rounded (`frame_width`). |
| **Smoothing** | none (default), 2 × 2 (4× slower), 3 × 3 (9× slower) | renders aa × aa samples per pixel and averages them in linear light. An option is greyed if it would exceed 30 million render pixels. If the resolution changes so the current choice no longer fits, it drops to the largest that does. Hover: "Render each frame more finely and average it down. Stars no longer flicker as they cross pixel boundaries, and the video compresses better." |
| **Frame rate** | 24, 25, **30**, 50, 60 fps | |
| **Quality** | good (QP 26), **high** (QP 20), very high (QP 16), near lossless (QP 10) | MP4 only. Each entry's hover gives its quantiser. Hover: "Every frame is encoded at the same quality; busy frames make bigger files". |
| **Folder** | the videos folder | a text box with **Browse…** |
| **File name** | the project's name | `.mp4` is added, or `_frames` for the PNG folder. An existing file or folder is never overwritten: `_2`, `_3`, … is added. |

Under the table, in grey: the output's full path, e.g. `C:\Users\…\Videos\Algebraic Fractals\Spiral dive.mp4`.

### 8.8 The waypoint strip

Always shown, under the tabs, whatever tab is open. It's the user's way to see the whole video at a glance while changing settings.

- **One thumbnail per waypoint,** left to right in order, scrolling sideways when they don't fit. Each is the waypoint's frame (the exact frame the video shows it in), in the video's shape: **160 × 90** for 16:9, 90 × 90 for square. **Decision:** a size control (small 160 / medium 240 / large 320 wide), remembered.
- **(New)** The other waypoints inside a thumbnail's frame are marked on it, as in the old preview: a cross and the waypoint's number, and its own frame's box once that's big enough to see (the selected waypoint yellow, the others white). The frame preview and the Colour tab's example frame are marked the same way.
- **Under each:** its number and name (Start, Finish, or its own), then `hw 2.0e-3  max 41` (the max coefficient, with "auto" in grey if auto chose it, or an orange ⚠ if auto fell short).
- **Between each pair:** an arrow with the segment's duration (`12 s`).
- **The selected waypoint's thumbnail** has a yellow (255, 200, 40) border. Click a thumbnail to select it (the tables in the Path and Polynomials tabs follow). Double-click: Show in the explorer. Right-click: **Show in the explorer**, **Set to the explorer's view**, **Insert halfway after**, **Delete**.
- **How they're rendered:** at the thumbnail's own size, one render each at the waypoint's own max coefficient (so no fading), with no reuse and no smoothing, then coloured with the project's colouring and colour scale. They're low-resolution on purpose: even deep, high-coefficient views take a fraction of a second at this size. A thumbnail shows the frame shrunk, so its stars look denser than the video's.
- **When they update:**
  - **Search changes** (a waypoint's view, the max coefficients, the order, the coefficients, the shape): the affected thumbnails re-render 400 ms after the last change. A thumbnail being re-rendered keeps its old picture, dimmed, with a small spinner, until the new one arrives.
  - **Colour changes:** every thumbnail recolours at once, without searching again.
  - **Order of work:** the selected waypoint first, then the rest left to right, on all cores, one at a time. A change cancels the current job and starts again with the new settings, keeping thumbnails that are still right (also when waypoints are reordered).
- **While a video renders:** the strip stays, and stops updating until the render ends.

### 8.9 The bottom bar and rendering

**Before rendering,** one line:
- `N frames · <duration> at F fps · W x H`;
- `~N MB`, from bits per pixel per frame: good 1.5, high 2.3, very high 2.9, near lossless 3.7 (MP4); for PNG frames, about 4 bytes per pixel. Hover: "Star fields need a lot of data to stay sharp; the real size depends on how busy the frames are";
- `~<render time>`, once worked out in the background (`frame_secs`, scaled by the latest measured render's time per unit of work: the frame preview's, else the strip's). Hover: "Scaled from a measured render by the work each frame needs. Views near the real axis can take longer.";
- **● Render now:** a large, bold button. It saves the project first if it has a name and unsaved changes **(Decision)**.

**Rendering later:** save the project (Ctrl+S) and close the window. To render it later, open it (Video ▾ → Open video project) and press Render now, or render it with the command line, `algfrac video "Spiral dive"` ([10](#10-the-command-line-tool)), e.g. overnight. **Decision:** a render queue in the app (add several projects, render them one after another) is left out; say if you want it.

**Durations** read like `45 s`, `12 min` or `3 h 20 min` (`duration_text`).

**While rendering:**
- The tabs' settings are all locked (greyed), and the project bar too. The window can be closed: the render carries on, and its progress shows in the explorer's toolbar ([5.1](#51-layout)) and the Video menu.
- The tab area shows the **render panel** instead of the tab:
  - the latest finished frame, as large as fits;
  - a progress bar `frame k of N, ~T left`, and a thin bar for the current render's rows;
  - grey: `elapsed T, last frame 1.2 s, 140 MB so far, max coefficient 43.6`;
  - **Stop:** finishes the current frame, then saves the video so far as a shorter video. The status says "Stopping after the current frame…".
- The explorer stays fully usable.

**Time left:** each frame's time is estimated in advance, from its search work (with reuse, a patch counts at the first frame that needs it, plus about 40 ns per pixel per render). The time so far is scaled by (estimated total − estimated so far) / estimated so far.

**Each frame** (`make_video`, to be moved into the library):
1. If the colour scale needs the waypoints' levels (steady brightness, or the length/height scale fixed from the waypoints), each waypoint's frame is rendered first at full resolution, and its levels measured.
2. For each frame:
   - the renders it needs: with reuse, any of its patches not yet rendered; otherwise its region at each max coefficient, with smoothing;
   - colouring with the project's style and colour scale;
   - averaging down and cross-fading;
   - writing it: encoding into the MP4, and/or saving the PNG.
3. **When it ends:**
   - `Done in T (N MB): <path>`, with **Show in folder** and **Play** buttons in the render panel;
   - stopped early: `Stopped after k of N frames (T); saved the video so far (N MB): <path>`;
   - stopped before any frame: the file is deleted and the status says "Stopped before the first frame was finished".
4. Afterwards the tabs come back, unlocked.

**Encoding** (`video::Mp4Out`, carried over):
- H.264 via OpenH264, compiled into the program.
- BT.709 limited-range I420.
- Fixed quantiser (`QpRange(qp, qp)`, quality mode), no frame skipping, a keyframe every 2 s.
- **Background detection and adaptive quantisation off.** This is what fixed the "chunks of stars jumping" glitch; don't change it.
- Muxed with the `mp4` crate (AVCC samples, SPS/PPS in `avcC`). A stopped video is still finished properly, so it plays.

**Quitting the program** while a video renders loses that video, since the file is only playable once finished. **(New, Decision)** Ask first: "A video is rendering. Stop it and save what's done, or keep rendering?" with **Stop and save**, **Keep rendering** and **Quit without saving**.

### 8.10 Status line

The video window has its own status line, a thin grey line just above the bottom bar, for its messages: saving, opening, Centre on flower, errors. It shows the latest one for 10 s, then clears.

---

## 9. How the explorer and videos work together

The point of combining the programs. The intended way to make a video:

1. **Explore** and find views you like.
2. **Save each as a location** into one group, in the order the video should visit them (★ Save location, then the group). Reorder them in the Locations popup, or use Sort by depth.
3. **🎬 Make video** on the group (or Video ▾ → New video from group). The video window opens with the group's locations as the start, waypoints and finish, and the explorer's settings and colours.
4. **Adjust** in the video window's tabs, watching the waypoint strip.
5. **Render now**, or **Save** and render later.

The links both ways:
- **Explorer → video:**
  - a group becomes the waypoints (Make video; or Replace with a group in the Path tab);
  - **Add the explorer's view** and **Set to the explorer's view** (Path tab, and the strip's right-click menu);
  - **Copy the explorer's colours** (Colour tab) and **Copy the explorer's settings** (Polynomials tab).
- **Video → explorer:**
  - **Show in the explorer** (Path tab, and double-click on a thumbnail): look at a waypoint at high resolution, or adjust it there and press Set to the explorer's view;
  - **Use these colours in the explorer** (Colour tab);
  - **Save as a group** (Path tab): the waypoints become locations.
- A video's waypoints are copies: changing a location later doesn't change a project made from it, and the other way round.

---

## 10. The command-line tool

`algfrac.exe` is carried over (`src/bin/algfrac.rs`):

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
| `--n N`, `--max P` (or `--clip P`), `--min P` | as in the colour controls | 1, 99.5, 0 |
| `--colormap NAME` | any of the 23 | `bone` |
| `--invert`, `--noshade`, `--logpoly`, `--sharp` | invert; no shading; log length/height; no Smooth stars | |
| `--gamma`, `--cycles`, `--shift`, `--brightness`, `--contrast`, `--saturation` | as in the colour controls | 1, 1, 0, 0, 1, 1 |
| `--out NAME`, `--nodata`, `--threads T` | output name; skip the `.algf`; worker threads | `algfrac_out`, all cores |

The `--legacy` variant exists only on the command line.

**(New, Decision) Rendering a video project:**

```
algfrac.exe video "Spiral dive"            a saved project, by name
algfrac.exe video D:\shared\dive.afvideo   or a project file
            [--threads T] [--out FOLDER]
```

It renders the project exactly as Render now would, to the project's folder (or `--out`), printing the frame, the time left and the size so far every 2 s. Ctrl+C stops after the current frame and saves the video so far. `algfrac.exe video --list` lists the saved projects.

---

## 11. Files on disk

The new program must read what the old programs wrote (places and paths are brought in, [11.4](#114-old-files-brought-in)), and never change those files.

### 11.1 The settings folder

`%APPDATA%\AlgebraicFractals\` on Windows, `~/Library/Application Support/AlgebraicFractals/` on macOS, and `$XDG_CONFIG_HOME/AlgebraicFractals/` (or `~/.config/…`) elsewhere (`common::settings_dir`). It holds:

| file | format |
|---|---|
| `locations.tsv` | **(New)** the saved locations ([11.2](#112-locations)) |
| `projects/<name>.afvideo` | **(New)** saved video projects ([11.3](#113-video-projects)). `<name>` has `\ / : * ? " < > \|` and control characters replaced by `_` (`file_safe`). |
| `settings.tsv` | **(New, Decision)** remembered settings ([6.8](#68-remembered-settings-new)) |
| `places.tsv`, `renders/`, `paths/` | the old programs' files: read, never written ([11.4](#114-old-files-brought-in)) |

### 11.2 Locations

`locations.tsv`: one location per line, in order, tab-separated; `#` lines are comments.

```
# Algebraic Fractals saved locations: group, name, re, im, half-width
Spiral dive	start	0	0	2
Spiral dive	arm	0.2731	0.5512	0.03
Unsorted	lace	-0.5	0.25	0.001
```

- Columns: group, name, centre re, centre im, half-width (`Location::to_line`). Numbers are written in Rust's shortest exact form, so they read back exactly.
- Within a group, the file's order is the group's order. The groups' own order is the order they first appear.
- Tabs and control characters in names become spaces.
- A line that can't be read (too few columns, a number that isn't one, a half-width of 0) is skipped.
- An empty group is kept as a line with only its name and no other columns.

### 11.3 Video projects

`projects/<name>.afvideo`: text, `key<TAB>value` lines; `#` lines are comments. Unknown keys are ignored and missing ones take the defaults in [8.1](#81-a-new-project), so newer files open in older versions and the other way round.

```
# Algebraic Fractals video project
version	1
name	Spiral dive
# path
rate	1.01
wp	start	0	0	2	6	1	0.01	0          name, re, im, half-width, max, auto 0/1, pan to next, keep flower to next 0/1
wp	arm	0.2731	0.5512	0.03	41	1	0.01	0
wp	finish	0.27312	0.55121	2e-6	31000	1	0.01	0
# polynomials
order	3
set	ints
zero	1
zero_root	1
target	4
reuse	1
step	1                0 fine, 1 medium, 2 coarse
fade	1
# colour
colour_by	pow
colormap	inferno
n	3
high	99.5
low	0
invert	0
shade	1
log_poly	0
smooth	1
gamma	1
cycles	1
shift	0
brightness	0
contrast	1
saturation	1
steady	0
lh_scale	waypoints       each | waypoints | <from>,<to>
# output
format	mp4               mp4 | png | both
shape	16:9              16:9 | 1:1
height	720
smoothing	1
fps	30
quality	high            good | high | very high | near lossless
folder	C:\Users\…\Videos\Algebraic Fractals
file	Spiral dive
```

- A project needs at least two `wp` lines; one with fewer won't open, and says so.
- The last waypoint's pan and flower columns are ignored.
- Numbers outside a control's range are clamped to it.
- `version` goes up only if a change can't be read by the rule above.

### 11.4 Old files brought in

- **`places.tsv`** (old saved places): listed as the group **Old places** until a `locations.tsv` exists ([6.5](#65-locations-and-groups-changed)). Columns: name, re, im, half-width, order, set name, `1` (unused), max, zero (0/1), colour by, n, max %, colormap, invert, shade, zero root, min %, gamma, cycles, shift, brightness, contrast, saturation, log scale, smooth stars. Columns were only ever added at the end, and missing ones take defaults (`OldPlace::from_line`). Only the name and view are used.
- **`renders/<name>.algf`** (old places' saved renders): not used. The command line still reads and writes `.algf`.
- **`paths/<name>.path`** (old saved video paths, **Decision:** can be opened): listed under **Old paths** in the Open menus. Opening one makes a new, unsaved project: the waypoints with their views, max coefficients, autos, pan speeds and flower settings; zoom per frame, target, fade, reuse, coefficient steps, order and coefficients; the **first waypoint's colouring** for the whole video; steady brightness from `hold`; the default output. The status says `Brought in the old path 'NAME': its colouring is the start's`. The format:

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
  - Defaults for anything missing: rate 1.01 (clamped to 1.001–4), target 4 (clamped to 0.01–1000), fade and reuse on, step 1, hold off, flowers off, pan 0.01.
  - Unknown keys are ignored. Older files have `work`, which is no longer used.
  - A path needs at least two waypoints.

### 11.5 `.algf` render files

Used by the command line only (`src/algf.rs`), little-endian:

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
  - **(Changed)** `c0 = 0` is never searched. With 0 as a root, the polynomial `x` (residual \|X\|) is tried at every pixel after the search and wins ties, so a pixel's polynomial is never `x·q(x)`: the old engine's `x·q` beat `q` near `q`'s roots inside the unit circle, since \|X·q(X)\| < \|q(X)\| there. (Other reducible polynomials, such as `(x + 1)·q` near `q`'s roots within 1 of −1, can still win; only a factor of x is removed.)
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
  - recolour 1024²: under 10 ms;
  - **(New)** the waypoint strip, 6 waypoints at 160 × 90 with auto max coefficients up to 10⁵: under 1 s in all.

---

## 13. Decisions for you

| # | question | my default |
|---|---|---|
| 1 | Names: the app `Algebraic Fractals.exe` (Cargo `algebraic-fractals`), the command line `algfrac.exe`, the repo `algebraic-fractals` | as stated |
| 2 | Licence | **MIT OR Apache-2.0** (the usual Rust choice). The H.264 encoder (OpenH264) is BSD-licensed, but H.264 is patented. The README will say that the compiled-in encoder isn't covered by Cisco's patent licence, which only covers Cisco's own binaries. |
| 3 | The video window: a separate window, or a large popup inside the main window ([4.2](#42-windows-and-popups)) | a separate window |
| 4 | A Run button in the toolbar, as well as Enter and double-click ([7.2](#72-toolbar)) | no |
| 5 | Changing the explorer's resolution re-renders at once ([7.3](#73-settings-strip)) | no (as the old explorer) |
| 6 | Default folders for images and videos ([6.7](#67-folders-and-file-names)) | Pictures / Videos (Movies on Mac) `\Algebraic Fractals` |
| 7 | Browse… buttons and a Save image as… dialog (adds the `rfd` crate) | yes |
| 8 | Remember settings between sessions ([6.8](#68-remembered-settings-new)) | yes |
| 9 | Compressed PNGs ([7.13](#713-saving-images)) | yes |
| 10 | Dragging locations in the Locations popup, as well as ⏶ ⏷ and Move ([7.8](#78-locations-popup)) | yes |
| 11 | Old places come in as the group "Old places"; their saved renders aren't used ([6.5](#65-locations-and-groups-changed)) | yes |
| 12 | Old video paths can be opened as projects, with the start's colouring ([11.4](#114-old-files-brought-in)) | yes |
| 13 | Leaving a project with unsaved changes asks first ([8.3](#83-projects-saving-and-opening)) | ask |
| 14 | The "Look at a frame" preview in the Path tab, for the frames between waypoints ([8.4](#84-path-tab)) | yes |
| 15 | Steady brightness on by default ([8.5](#85-colour-tab)) | off (as the old video maker) |
| 16 | The length/height scale's default ([8.5](#85-colour-tab)) | fixed from the waypoints |
| 17 | Output formats: MP4, PNG frames, both ([8.7](#87-output-tab)) | those three |
| 18 | More shapes: 9:16 portrait, 4:3 ([8.7](#87-output-tab)) | no |
| 19 | A size control for the waypoint strip ([8.8](#88-the-waypoint-strip)) | yes |
| 20 | Render now saves the project first ([8.9](#89-the-bottom-bar-and-rendering)) | yes |
| 21 | A render queue in the app ([8.9](#89-the-bottom-bar-and-rendering)) | no |
| 22 | The command line renders saved video projects ([10](#10-the-command-line-tool)) | yes |
| 23 | Ask before quitting while a video renders ([8.9](#89-the-bottom-bar-and-rendering)) | yes |
| 25 | An app icon | yes (a small star-field image, drawn once from a render) |
| 26 | Automatic GitHub builds for Windows and macOS on each release tag ([15.3](#153-releases-on-github)) | yes, so no Mac is needed for Mac builds |
| 27 | Keep the command-line tool in the release downloads | yes |

---

## 14. Tests

Run with `cargo test --release --lib --bins`. Plain `cargo test` also runs the doc comments' formulas as code.

**Carried over with the code** (already passing here, 33 tests):
- **engine:** both searches against brute force (orders 2–6, every coefficient set, with and without c0 = 0); the tile search's list against brute force; tiled renders equal to per-pixel renders (depths 10⁻³ to 10⁻⁷, across the real axis, outside the unit circle); table chunking.
  - Ignored: `bench_deep`, `diag_deep_lines`.
- **colour:** lookup tables equal the per-pixel pipeline; Smooth stars keep their brightness; binned percentiles equal sorting.
  - Ignored: `recolour_speed`.
- **density:** densities integrate to d roots; the root finder.
- **algf:** round trip; a truncated file is refused.
- **video:** path ends and speeds, waypoints, frame sizes, pan speed, smooth curves, max coefficients, flower size, finding quadratic numbers, cross-fade, held scale (one style), downsampling keeps brightness, writes a playable mp4 (up to 3840 × 2160).
- **reuse:** ladders, plans cover the frames, same grid gives the same picture, windows and averaging.
- **common:** parsing complex numbers; a location's line round trip; old place lines (oldest and newest column counts).

**To port from the old video maker** (with the logic moved into the library):
- `flower_sets_the_next_max`;
- `short_zoom_video_reusing` and `short_zoom_video_frame_by_frame`. These are end-to-end: auto never falls, maxes never fall, and the first decoded frame matches what was rendered;
- `faint_rows_of_stars_survive_encoding`: the worst 16 × 16 block's error stays below 3;
- ignored: `bench_reuse`, `flower_video`, `diag_wobble`, `diag_decode`, `diag_encoder`.

**To port from the old explorer:**
- `auto_density`: auto lands near the target and never falls as you zoom in.

**New:**
- locations: saving, moving within and between groups, renaming, deleting, sorting by depth, and the file round trip (including empty groups);
- `places.tsv` from the old programs comes in as Old places;
- a video project's file round trip, with every setting changed from its default;
- an old `.path` file opens as a project, with the start's colouring;
- the length/height scale: each frame, fixed from the waypoints, fixed by hand;
- PNG-frames output writes the frames, numbered, and the same pixels as the MP4's input;
- the command line renders a saved project;
- `settings.tsv` round trip.

Check in a sample `places.tsv` and `.path` from the old programs as test data (`tests/data/`).

**Equivalence with the old programs:**
- The command line with the same arguments writes byte-identical `.png` and `.algf` files, except with `--zeroroot` (see the engine's change to 0 as a root, [12](#12-the-engine-what-must-stay-true)).
- A short video with the same settings and one colouring gives identical frames (compare the PNG frames).

**GUI checks by hand** before each release:
- explore, zoom, Back, Home, the right-click menu;
- each popup opens, closes with Esc, and works over the image;
- save locations into a new group, reorder them, move one to another group;
- Make video from the group; the strip renders; change the colour, the order and the root density, and watch the strip update;
- Show in the explorer, then Set to the explorer's view;
- save the project, close the window, reopen it from the Video menu;
- render a short video (MP4 and PNG frames) and play it;
- stop a video early and play it;
- render a saved project from the command line.

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
- **Dependencies to add for the app:** `eframe`/`egui` 0.31 (as the old programs), `rfd` (Decision 7), and `png` or `miniz_oxide` (Decision 9).

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
  - what the explorer and the video window do, in a few bullets each;
  - "What is computed";
  - "Why the patterns look the way they do" (dark holes, the real axis, flowers, order 3, symmetries, star size, density), from the old root README;
  - further reading;
  - building from source (a short paragraph pointing to DEVELOPER.md);
  - the licence.
- **`docs/USER-GUIDE.md`:** one guide for the app.
  - Getting started.
  - The explorer: the toolbar, the settings strip, the popups, the mouse.
  - Search settings, root density and auto, colours.
  - Locations and groups.
  - Making a video: from a group to a finished video (the steps in [9](#9-how-the-explorer-and-videos-work-together)), then each tab (how the path moves, keeping a flower the same size, the max coefficient, reuse, the colour scale, output), the waypoint strip, render times, rendering later and from the command line.
  - Tips.
  - Troubleshooting (both old tables, merged and updated).

  Keep the old guides' measured tables (render times, file sizes, quality).
- **`docs/DEVELOPER.md`:** the old developer README, updated for the new layout and names.
- **`docs/MAC.md`:** the old README-MAC, updated for one app.
- **Writing style:** British spelling, plain words, short sentences, numbers measured rather than guessed, as in the old guides.

---

## 17. Order of work

1. **The library:** move `Frames`, `auto_maxes`, `frame_secs` and `make_video` into the library, with their tests. Make `make_video` report progress through a callback, without egui, and write PNG frames as well as MP4. Write `VideoProject` and its file, bringing in old `.path` files, and the length/height scale options. Finish locations (empty groups) and `settings.tsv`.
2. **The main window:** toolbar, settings strip, image and mouse, status bar; then the popups (View, Polynomials, Colours, Save location, Locations, Folders); saving images.
3. **The video window:** project bar, the four tabs, the waypoint strip, the bottom bar, rendering, the frame preview.
4. **The links** between the explorer and the video window ([9](#9-how-the-explorer-and-videos-work-together)), remembered settings, the quit prompt, the icon.
5. **The command line's `video`.**
6. **Tests:** ported and new; equivalence checks against the old programs.
7. **Build scripts, docs, licence, README pictures, GitHub workflows.** Then a release build and the manual checks in [14](#14-tests).

Each step ends with the tests passing and a commit.

---

## 18. Appendix: constants

| constant | value | where |
|---|---|---|
| `MAX_COEF` | 1 000 000 | largest max coefficient |
| default target root density | 4 per 100 px² | everywhere |
| explorer sizes | square 128, 256, 384, 512, 768, 1024, 1536, 2048, 4096; widescreen heights 360, 540, 720, 1080, 1440, 2160; or the window's | |
| home view | centre i, half-width 0.75 | explorer; a new video's start |
| explorer history | 30 views | Back |
| zoom factor | 1–1000, default 4, wheel × 1.15 per notch | explorer |
| settle delay | 400 ms | before an automatic re-render (explorer, waypoint strip, frame preview) |
| `TABLE_MAX` | 2²⁶ entries per thread | engine |
| tiles | about 256 polynomials each, at most 64 px across; split above 4096; per-pixel below 8 px or 4 × 4 | engine |
| levels cache | 64 waypoint levels | video |
| thumbnails | 160 × 90 (16:9), 90 × 90 (square); medium 240, large 320 wide | waypoint strip |
| video heights | 360, 480, 720, 1080, 1440, 2160 | video |
| frame rates | 24, 25, 30, 50, 60 | video |
| quality | good QP 26 (1.5 bpp), high QP 20 (2.3), very high QP 16 (2.9), near lossless QP 10 (3.7) | video |
| smoothing | 1, 2, 3 (aa × aa) | video |
| `MAX_RENDER_PIXELS` | 30 000 000 | video renders and patches |
| `PATCH_FRAMES` | 6 | video reuse |
| `COEF_STEPS` | fine 0.125, medium 0.25, coarse 0.5 | video reuse |
| `DEFAULT_PAN` | 0.01 frame heights per frame | video |
| zoom per frame | 1.001–4, default 1.01 | video |
| keyframe interval | 2 s | encoder |
| time per pixel per render (estimate) | 40 ns | video time estimates |
| precision warning | half-width < 10⁻¹² × \|centre\| | explorer and video |
| `OLD_PLACES_GROUP` | "Old places" | locations |
| default group | "Unsorted" | locations |
