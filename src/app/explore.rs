//! The explorer: the main window. A toolbar, a strip with the search settings that stay
//! on screen, the image, and a status bar; everything else is in popups (popups.rs).

use super::widgets::{ORANGE, YELLOW};
use super::Shared;
use algfrac::colour::{colourise, colourise_into, ColourBy, Stats, Style};
use algfrac::common::*;
use algfrac::engine::{render_axes, Coefs, Plan, Region, Render};
use algfrac::png::write_png_compressed;
use eframe::egui::{self, pos2, vec2, Color32, ColorImage, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Square image sizes.
pub const RESOLUTIONS: [usize; 9] = [128, 256, 384, 512, 768, 1024, 1536, 2048, 4096];
/// Widescreen (16:9) image heights.
pub const WIDE_HEIGHTS: [usize; 6] = [360, 540, 720, 1080, 1440, 2160];
const HISTORY_LEN: usize = 30;

/// The image's shape.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageShape {
    Square,
    /// 16:9
    Wide,
    /// whatever fits the window, at its own pixels
    Fill,
}

impl ImageShape {
    const ALL: [ImageShape; 3] = [ImageShape::Square, ImageShape::Wide, ImageShape::Fill];

    fn name(self) -> &'static str {
        match self {
            ImageShape::Square => "square",
            ImageShape::Wide => "widescreen 16:9",
            ImageShape::Fill => "fill the window",
        }
    }

    fn key(self) -> &'static str {
        match self {
            ImageShape::Square => "square",
            ImageShape::Wide => "wide",
            ImageShape::Fill => "fill",
        }
    }
}

/// What auto order / max coefficient was worked out from.
#[derive(Clone, Copy, PartialEq)]
struct AutoKey {
    view: View,
    size: (usize, usize),
    target: f64,
    auto_order: bool,
    order: usize,
    coefs: Coefs,
}

/// What to search for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Search {
    pub order: usize,
    pub coefs: Coefs,
    /// the image's size in pixels
    pub width: usize,
    pub height: usize,
}

impl Search {
    /// The parts that trigger an automatic re-render when changed.
    fn key(&self) -> (usize, Coefs) {
        (self.order, self.coefs)
    }
}

/// The region a `w` x `h` image of `view` shows: `hw` is half its height, as in a video
/// (for a square image, also half its width).
pub fn image_region(view: View, w: usize, h: usize) -> Region {
    let hx = view.hw * w as f64 / h.max(1) as f64;
    Region { re: (view.cr - hx, view.cr + hx), im: (view.ci - view.hw, view.ci + view.hw), width: w.max(2) }
}

/// The search that auto order / max coefficient would choose for `view` (see
/// choose_auto), with no limit on the work, as for videos.
pub fn auto_search(p: &mut algfrac::density::Predictor, s: Search, view: View, target: f64, auto_order: bool) -> (Search, String) {
    let region = image_region(view, s.width, s.height);
    let (order, coefs, note) = choose_auto(p, s.order, s.coefs, &region, target, auto_order, f64::INFINITY);
    (Search { order, coefs, ..s }, note)
}

/// The simplest quadratic number within `r` of (x, y), the centre of the largest flower
/// there, and its polynomial; or why there's none.
pub fn flower_near(x: f64, y: f64, r: f64) -> Result<(f64, f64, String), String> {
    match algfrac::video::simplest_quadratic(x, y, r, 1_000_000) {
        Some((abc, re, im)) => Ok((re, im, quadratic_text(abc))),
        None => Err("No flower near there (too close to the real axis?)".into()),
    }
}

/// A finished render and what it shows.
pub struct Shown {
    pub render: Arc<Render>,
    pub stats: Stats,
    pub view: View,
    pub search: Search,
}

/// A render running on a background thread.
struct Job {
    search: Search,
    view: View,
    cancel: Arc<AtomicBool>,
    rows: Arc<AtomicUsize>,
    height: usize,
    started: Instant,
    rx: mpsc::Receiver<Result<Render, String>>,
}

/// The toolbar's popups.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Popup {
    View,
    Polynomials,
    Colours,
    SaveLocation,
    Locations,
    Folders,
}

impl Popup {
    const ALL: [Popup; 6] = [Popup::View, Popup::Polynomials, Popup::Colours, Popup::SaveLocation, Popup::Locations, Popup::Folders];

    fn key(self) -> &'static str {
        match self {
            Popup::View => "view",
            Popup::Polynomials => "polynomials",
            Popup::Colours => "colours",
            Popup::SaveLocation => "save",
            Popup::Locations => "locations",
            Popup::Folders => "folders",
        }
    }
}

pub enum Action {
    Run,
    Refresh,
    Back,
    Home,
    ZoomOut,
    Cancel,
    SaveImage,
    SaveImageAs,
    /// render the view typed into the centre / half-width boxes
    GoTo,
    Go(View),
    SaveLocation,
    Toggle(Popup),
}

/// What the explorer asks of the video window.
pub enum ToVideo {
    Show,
    New,
    FromGroup(usize),
    Open(String),
    OpenOld(String),
    StopRender,
}

/// What the explorer shows of the video window: whether it's open, and a render's progress.
#[derive(Default)]
pub struct VideoInfo {
    pub open: bool,
    /// (project name, frame done, frames, time left)
    pub rendering: Option<(String, usize, usize, String)>,
}

pub struct Explorer {
    pub search: Search,
    pub zoom: f64,
    /// clicked point to zoom into; None = the current centre
    target: Option<(f64, f64)>,
    pub show_box: bool,
    pub style: Style,

    pub shown: Option<Shown>,
    shown_gen: usize,
    history: Vec<View>,
    job: Option<Job>,
    /// view of the most recently requested render
    pub requested: View,
    /// search key of the most recently requested render
    applied: (usize, Coefs),
    /// pending auto re-render after the order/coefficients changed
    changed: Option<((usize, Coefs), Instant)>,

    /// choose the order / max coefficient automatically to give `target_density`
    /// algebraic numbers per 100 square pixels (at most one of the two)
    pub auto_order: bool,
    pub auto_max: bool,
    pub target_density: f64,
    auto_key: Option<AutoKey>,
    /// why the auto setting couldn't reach the target, if it couldn't
    pub auto_note: String,

    /// the open popups, the latest opened last
    pub open: Vec<Popup>,
    /// the centre / half-width text boxes, and the view they were last filled from
    pub goto_centre: String,
    pub goto_hw: String,
    pub goto_from: Option<View>,
    /// the Save location popup's name, group, and the new group being typed
    pub save_name: String,
    pub save_group: String,
    pub new_group: Option<String>,
    pub loc: super::popups::LocationsUi,

    texture: Option<TextureHandle>,
    drawn: Option<(usize, Style)>,
    pub ramp: Option<(Style, TextureHandle)>,
    hover: String,
    pub status: String,
    /// when the mouse wheel last changed the zoom factor
    zoom_note: Option<Instant>,

    /// the image's shape and size: the square size, the widescreen height, and the
    /// window's image area in pixels (for Fill), once known
    pub shape: ImageShape,
    pub square: usize,
    pub wide_height: usize,
    fill_size: Option<(usize, usize)>,
    /// a new fill size waiting to be rendered, and since when
    fill_changed: Option<((usize, usize), Instant)>,
    /// clicks move the zoom box onto the nearest flower
    pub snap: bool,
}

impl Explorer {
    pub fn new(settings: &algfrac::settings::Settings) -> Explorer {
        let s = settings;
        let coefs = Coefs {
            set: s.str("explore.set").and_then(|t| algfrac::engine::CoefSet::ALL.into_iter().find(|c| c.name() == t)).unwrap_or(algfrac::engine::CoefSet::Ints),
            max: s.get::<i32>("explore.max").unwrap_or(12).clamp(1, MAX_COEF),
            zero: true, // 0 is always an allowed coefficient
            zero_root: s.flag("explore.zero_root").unwrap_or(true),
        };
        let square = s.get::<usize>("explore.width").filter(|w| RESOLUTIONS.contains(w)).unwrap_or(512);
        let search = Search { order: s.get::<usize>("explore.order").unwrap_or(3).clamp(1, 16), coefs, width: square, height: square };
        let (auto_order, auto_max) = (s.flag("explore.auto_order").unwrap_or(false), s.flag("explore.auto_max").unwrap_or(true));
        Explorer {
            search,
            zoom: s.get::<f64>("explore.zoom").unwrap_or(4.0).clamp(1.0, 1000.0),
            target: None,
            show_box: s.flag("explore.show_box").unwrap_or(true),
            style: s.style("explore.style."),
            shown: None,
            shown_gen: 0,
            history: Vec::new(),
            job: None,
            requested: s.view("explore.view").unwrap_or(super::HOME),
            applied: search.key(),
            changed: None,
            auto_order,
            auto_max: auto_max && !auto_order,
            target_density: s.get::<f64>("explore.target").unwrap_or(4.0).clamp(0.01, 1000.0),
            auto_key: None,
            auto_note: String::new(),
            open: s.str("explore.popups").map_or(Vec::new(), |t| t.split(',').filter_map(|k| Popup::ALL.into_iter().find(|p| p.key() == k.trim())).collect()),
            goto_centre: String::new(),
            goto_hw: String::new(),
            goto_from: None,
            save_name: String::new(),
            save_group: s.str("last_group").unwrap_or(UNSORTED).to_string(),
            new_group: None,
            loc: Default::default(),
            texture: None,
            drawn: None,
            ramp: None,
            hover: String::new(),
            status: String::new(),
            zoom_note: None,
            shape: s.str("explore.shape").and_then(|t| ImageShape::ALL.into_iter().find(|x| x.key() == t)).unwrap_or(ImageShape::Square),
            square,
            wide_height: s.get::<usize>("explore.wide_height").filter(|h| WIDE_HEIGHTS.contains(h)).unwrap_or(720),
            fill_size: None,
            fill_changed: None,
            snap: s.flag("explore.snap").unwrap_or(false),
        }
    }

    /// The image size the settings ask for (None for Fill until the window is drawn).
    fn wanted_size(&self) -> Option<(usize, usize)> {
        match self.shape {
            ImageShape::Square => Some((self.square, self.square)),
            ImageShape::Wide => Some((algfrac::video::frame_width(self.wide_height, 16.0 / 9.0), self.wide_height)),
            ImageShape::Fill => self.fill_size,
        }
    }

    pub fn remember(&self, s: &mut algfrac::settings::Settings) {
        let c = self.search.coefs;
        s.set("explore.order", self.search.order);
        s.set("explore.set", c.set.name());
        s.set("explore.max", c.max);
        s.set_flag("explore.zero_root", c.zero_root);
        s.set("explore.width", self.square);
        s.set("explore.wide_height", self.wide_height);
        s.set("explore.shape", self.shape.key());
        s.set_flag("explore.snap", self.snap);
        s.set_flag("explore.auto_order", self.auto_order);
        s.set_flag("explore.auto_max", self.auto_max);
        s.set("explore.target", self.target_density);
        s.set("explore.zoom", self.zoom);
        s.set_flag("explore.show_box", self.show_box);
        s.set_style("explore.style.", &self.style);
        s.set_view("explore.view", self.current_view());
        let open: Vec<&str> = self.open.iter().filter(|&&p| p != Popup::SaveLocation).map(|p| p.key()).collect();
        s.set("explore.popups", open.join(","));
    }

    pub fn current_view(&self) -> View {
        self.shown.as_ref().map_or(self.requested, |s| s.view)
    }

    pub fn is_open(&self, p: Popup) -> bool {
        self.open.contains(&p)
    }

    pub fn toggle(&mut self, p: Popup) {
        match self.open.iter().position(|&q| q == p) {
            Some(i) => {
                self.open.remove(i);
            }
            None => {
                self.open.push(p);
                self.loc.focus_name |= p == Popup::SaveLocation;
            }
        }
    }

    /// The point the next zoom goes to: the clicked one, or the centre.
    pub fn target_point(&self) -> (f64, f64) {
        let v = self.current_view();
        self.target.unwrap_or((v.cr, v.ci))
    }

    /// If an auto setting is on, sets the order or max coefficient whose predicted density
    /// of algebraic numbers in `view` is closest to the target (see density.rs).
    fn apply_auto(&mut self, shared: &mut Shared, view: View) {
        if !self.auto_order && !self.auto_max {
            self.auto_note.clear();
            return;
        }
        let s = self.search;
        let key = AutoKey {
            view,
            size: (s.width, s.height),
            target: self.target_density,
            auto_order: self.auto_order,
            // the inputs, leaving out the setting being chosen
            order: if self.auto_order { 0 } else { s.order },
            coefs: if self.auto_max { Coefs { max: 0, ..s.coefs } } else { s.coefs },
        };
        if self.auto_key == Some(key) {
            return;
        }
        self.auto_key = Some(key);
        (self.search, self.auto_note) = auto_search(&mut shared.predictor, s, view, self.target_density, self.auto_order);
    }

    /// Starts rendering `view` with the current search settings (cancelling any running
    /// render).
    pub fn start(&mut self, ctx: &egui::Context, shared: &mut Shared, view: View) {
        if let Some(job) = self.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        if let Some((w, h)) = self.wanted_size() {
            (self.search.width, self.search.height) = (w, h);
        }
        self.apply_auto(shared, view);
        let search = self.search;
        self.requested = view;
        self.applied = search.key();
        self.changed = None;
        let (re, im) = image_region(view, search.width, search.height).axes();
        let cancel = Arc::new(AtomicBool::new(false));
        let rows = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel();
        let (c2, r2, ctx2, threads) = (cancel.clone(), rows.clone(), ctx.clone(), shared.threads);
        let height = im.len();
        std::thread::spawn(move || {
            let res = match Plan::new(search.order, &search.coefs, false) {
                Err(e) => Err(e),
                Ok(plan) => match render_axes(&plan, re, im, threads, &c2, &r2) {
                    Some(r) => Ok(r),
                    None => return, // cancelled
                },
            };
            let _ = tx.send(res);
            super::repaint_now(&ctx2);
        });
        self.job = Some(Job { search, view, cancel, rows, height, started: Instant::now(), rx });
    }

    fn show(&mut self, render: Arc<Render>, view: View, search: Search) {
        let stats = Stats::new(&render);
        self.shown = Some(Shown { render, stats, view, search });
        self.shown_gen += 1;
    }

    /// Moves to a new view, remembering the current one for Back.
    pub fn navigate(&mut self, ctx: &egui::Context, shared: &mut Shared, view: View) {
        if self.shown.is_some() {
            self.history.push(self.current_view());
            if self.history.len() > HISTORY_LEN {
                self.history.remove(0);
            }
        }
        self.target = None;
        self.start(ctx, shared, view);
    }

    /// Picks up a finished render.
    fn poll(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.job else { return };
        match job.rx.try_recv() {
            Ok(Ok(render)) => {
                let job = self.job.take().unwrap();
                let render = Arc::new(render);
                self.status = format!("Rendered {}x{} in {:.2}s", render.width, render.height, job.started.elapsed().as_secs_f64());
                self.show(render, job.view, job.search);
            }
            Ok(Err(e)) => {
                self.status = format!("Error: {e}");
                self.job = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => self.job = None,
            Err(mpsc::TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
        }
    }

    /// Re-colours the texture if the render or the style changed.
    fn refresh_texture(&mut self, ctx: &egui::Context) {
        let Some(sh) = &self.shown else { return };
        if self.drawn == Some((self.shown_gen, self.style)) {
            return;
        }
        // straight into egui's pixel format, so there's no extra conversion pass
        let mut pixels = vec![Color32::BLACK; sh.render.width * sh.render.height];
        let scale = sh.stats.scale(&self.style);
        colourise_into(&sh.render, &sh.stats, &self.style, &scale, &mut pixels, |[r, g, b]| Color32::from_rgb(r, g, b));
        let img = ColorImage { size: [sh.render.width, sh.render.height], pixels };
        match &mut self.texture {
            Some(t) => t.set(img, TextureOptions::NEAREST),
            None => self.texture = Some(ctx.load_texture("fractal", img, TextureOptions::NEAREST)),
        }
        self.drawn = Some((self.shown_gen, self.style));
    }

    /// The image's file name: every setting that decides it.
    fn image_name(&self) -> Option<String> {
        let sh = self.shown.as_ref()?;
        let (v, s, st) = (sh.view, sh.search, self.style);
        let c = s.coefs;
        Some(format!(
            "alg_o{}_m{}{}{}{}_{:+.8}{:+.8}i_hw{:.3e}_{}px_{}_{}{}",
            s.order,
            c.max,
            c.set.tag(),
            if c.zero { "" } else { "_nz" },
            if c.zero_root { "_r0" } else { "" },
            v.cr,
            v.ci,
            v.hw,
            if sh.render.width == sh.render.height { sh.render.width.to_string() } else { format!("{}x{}", sh.render.width, sh.render.height) },
            st.by.short(),
            st.cmap.name(),
            if st.by == ColourBy::Power { format!("_n{}", st.n) } else { String::new() },
        ))
    }

    /// Saves the shown image as a PNG at `path`, or in the images folder.
    fn save_image(&mut self, shared: &Shared, path: Option<PathBuf>) {
        let (Some(sh), Some(name)) = (&self.shown, self.image_name()) else { return };
        let path = path.unwrap_or_else(|| {
            let dir = PathBuf::from(shared.images_dir.trim());
            let stem = unused_stem(&name, |s| dir.join(format!("{s}.png")).exists());
            dir.join(format!("{stem}.png"))
        });
        let rgb = colourise(&sh.render, &sh.stats, &self.style);
        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        self.status = match std::fs::create_dir_all(&dir).and_then(|_| write_png_compressed(&path, sh.render.width, sh.render.height, &rgb)) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(e) => format!("Could not save {}: {e}", path.display()),
        };
    }

    /// Adds the shown view to the Save location popup's group, or replaces the location
    /// of the same name there.
    fn save_location(&mut self, shared: &mut Shared) {
        let Some(view) = self.shown.as_ref().map(|s| s.view) else { return };
        let name = match self.save_name.trim() {
            "" => fmt_complex(view.cr, view.ci),
            n => n.to_string(),
        };
        let group = match self.new_group.take() {
            Some(g) if !g.trim().is_empty() => g.trim().to_string(),
            _ => self.save_group.clone(),
        };
        let gi = match shared.groups.iter().position(|g| g.name == group) {
            Some(i) => i,
            None => {
                shared.groups.push(Group { name: group.clone(), locations: Vec::new() });
                shared.groups.len() - 1
            }
        };
        let g = &mut shared.groups[gi];
        let at = match g.locations.iter().position(|l| l.name == name) {
            Some(i) => {
                g.locations[i].view = view;
                i
            }
            None => {
                g.locations.push(Location { name: name.clone(), view });
                g.locations.len() - 1
            }
        };
        let total = g.locations.len();
        self.save_group = group.clone();
        self.save_name.clear();
        self.status = match shared.store_groups() {
            Ok(()) => format!("Saved '{name}' in {group} (location {} of {total})", at + 1),
            Err(e) => format!("Could not save the locations: {e}"),
        };
    }

    /// The toolbar, the settings strip, the status bar and the image. Returns what's asked
    /// of the video window.
    pub fn update(&mut self, ctx: &egui::Context, shared: &mut Shared, video: &VideoInfo) -> Option<ToVideo> {
        self.poll(ctx);
        if self.shown.is_none() && self.job.is_none() && self.status.is_empty() {
            if self.wanted_size().is_some() {
                let v = self.requested;
                self.start(ctx, shared, v); // the first render
            } else {
                ctx.request_repaint(); // (Fill: once the window's size is known)
            }
        }
        // filling the window: a new size (the window resized, or Fill just chosen)
        // re-renders once it has stayed the same for a moment
        if let (ImageShape::Fill, Some(size), Some(sh), None) = (self.shape, self.fill_size, &self.shown, &self.job) {
            if (sh.render.width, sh.render.height) != size {
                match self.fill_changed {
                    Some((s, t)) if s == size => {
                        if t.elapsed() >= Duration::from_millis(400) {
                            self.fill_changed = None;
                            let v = self.current_view();
                            self.start(ctx, shared, v);
                        } else {
                            ctx.request_repaint_after(Duration::from_millis(50));
                        }
                    }
                    _ => {
                        self.fill_changed = Some((size, Instant::now()));
                        ctx.request_repaint_after(Duration::from_millis(50));
                    }
                }
            }
        }

        // order / coefficient changes re-render the current region (after a short pause,
        // so dragging a value doesn't start a render for every intermediate step);
        // with an auto setting on, so do changes to the target density or resolution
        let v = self.requested;
        self.apply_auto(shared, v);
        let key = self.search.key();
        if key != self.applied {
            match self.changed {
                Some((k, t)) if k == key => {
                    if t.elapsed() >= Duration::from_millis(400) {
                        self.start(ctx, shared, v);
                    } else {
                        ctx.request_repaint_after(Duration::from_millis(50));
                    }
                }
                _ => {
                    self.changed = Some((key, Instant::now()));
                    ctx.request_repaint_after(Duration::from_millis(50));
                }
            }
        }

        let mut action = None;
        let mut to_video = None;
        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Enter) {
                    action = Some(Action::Run);
                } else if i.key_pressed(egui::Key::Backspace) {
                    action = Some(Action::Back);
                } else if i.key_pressed(egui::Key::Home) {
                    action = Some(Action::Home);
                } else if i.modifiers.command && i.key_pressed(egui::Key::S) {
                    action = Some(Action::SaveImage);
                } else if i.modifiers.command && i.key_pressed(egui::Key::L) {
                    action = Some(Action::Toggle(Popup::SaveLocation));
                }
            });
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.open.pop();
        }

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(2.0);
            self.toolbar(ui, shared, video, &mut action, &mut to_video);
            ui.add_space(2.0);
        });
        egui::TopBottomPanel::top("strip").show(ctx, |ui| {
            ui.add_space(2.0);
            self.settings_strip(ui, shared, &mut action);
            ui.add_space(2.0);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let help = "Click: next centre    Double-click: zoom in there    Wheel: zoom factor    Right-click: more    \
                            Enter: Run    Backspace: Back";
                let zooming = self.zoom_note.is_some_and(|t| t.elapsed() < Duration::from_secs(2));
                if zooming {
                    ui.label(format!("zoom x {:.1}", self.zoom));
                    ctx.request_repaint_after(Duration::from_millis(500));
                } else if self.hover.is_empty() {
                    ui.weak(help);
                } else {
                    ui.label(&self.hover);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let v = self.current_view();
                    if v.hw < 1e-12 * v.cr.abs().max(v.ci.abs()) {
                        ui.colored_label(ORANGE, "beyond 64-bit precision: the image will look blocky");
                    } else {
                        ui.add(egui::Label::new(egui::RichText::new(&self.status).weak()).truncate());
                    }
                });
            });
        });
        self.refresh_texture(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(Color32::from_gray(16)))
            .show(ctx, |ui| self.image(ui, &mut action));
        self.popups(ctx, shared, &mut action, &mut to_video);

        match action {
            Some(Action::Run) => {
                let v = self.current_view();
                let (cr, ci) = self.target.unwrap_or((v.cr, v.ci));
                self.navigate(ctx, shared, View { cr, ci, hw: v.hw / self.zoom });
            }
            Some(Action::Refresh) => {
                let v = self.current_view();
                self.start(ctx, shared, v);
            }
            Some(Action::Back) => {
                if let Some(prev) = self.history.pop() {
                    self.target = None;
                    self.start(ctx, shared, prev);
                }
            }
            Some(Action::Home) => self.navigate(ctx, shared, super::HOME),
            Some(Action::ZoomOut) => {
                let v = self.current_view();
                self.navigate(ctx, shared, View { hw: v.hw * self.zoom.max(2.0), ..v });
            }
            Some(Action::Cancel) => {
                if let Some(job) = self.job.take() {
                    job.cancel.store(true, Ordering::Relaxed);
                    self.status = "Cancelled".into();
                }
                self.applied = self.search.key();
                if let Some(sh) = &self.shown {
                    self.requested = sh.view;
                }
            }
            Some(Action::SaveImage) => self.save_image(shared, None),
            Some(Action::SaveImageAs) => {
                if let Some(name) = self.image_name() {
                    let dialog = rfd::FileDialog::new().set_file_name(format!("{name}.png")).add_filter("PNG image", &["png"]);
                    let dir = PathBuf::from(shared.images_dir.trim());
                    let dialog = if dir.is_dir() { dialog.set_directory(dir) } else { dialog };
                    if let Some(p) = dialog.save_file() {
                        self.save_image(shared, Some(p));
                    }
                }
            }
            Some(Action::GoTo) => {
                if let (Some((cr, ci)), Ok(hw)) = (parse_complex(&self.goto_centre), self.goto_hw.trim().parse::<f64>()) {
                    self.navigate(ctx, shared, View { cr, ci, hw });
                }
            }
            Some(Action::Go(v)) => self.navigate(ctx, shared, v),
            Some(Action::SaveLocation) => {
                self.save_location(shared);
                self.open.retain(|&p| p != Popup::SaveLocation);
            }
            Some(Action::Toggle(p)) => self.toggle(p),
            None => {}
        }
        if shared.reset {
            self.reset(ctx, shared);
        }
        to_video
    }

    /// Every setting back to its default (the view stays).
    fn reset(&mut self, ctx: &egui::Context, shared: &mut Shared) {
        let fresh = Explorer::new(&algfrac::settings::Settings::default());
        (self.search, self.zoom, self.show_box, self.style) = (fresh.search, fresh.zoom, fresh.show_box, fresh.style);
        (self.auto_order, self.auto_max, self.target_density) = (fresh.auto_order, fresh.auto_max, fresh.target_density);
        self.auto_key = None;
        self.save_group = UNSORTED.into();
        self.status = "All settings are back to their defaults".into();
        let v = self.current_view();
        self.start(ctx, shared, v);
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, shared: &mut Shared, video: &VideoInfo, action: &mut Option<Action>, to_video: &mut Option<ToVideo>) {
        let has_image = self.shown.is_some();
        ui.horizontal_wrapped(|ui| {
            let popup = |ui: &mut egui::Ui, open: bool, text: &str, tip: &str| ui.add(egui::Button::new(text).selected(open)).on_hover_text(tip).clicked();
            if ui.add_enabled(has_image, egui::Button::new("▶ Run")).on_hover_text("Zoom into the yellow box (Enter, or double-click on the image)").clicked() {
                *action = Some(Action::Run);
            }
            if ui.add_enabled(!self.history.is_empty(), egui::Button::new("⏴ Back")).on_hover_text("The previous view (Backspace)").clicked() {
                *action = Some(Action::Back);
            }
            if ui.button("🏠 Home").on_hover_text("Back to the home view: centred on i, half-width 0.75 (Home key)").clicked() {
                *action = Some(Action::Home);
            }
            if ui.button("➖ Zoom out").on_hover_text("Zoom out by the zoom factor (at least x2), keeping the centre").clicked() {
                *action = Some(Action::ZoomOut);
            }
            if ui.button("🔄 Refresh").on_hover_text("Render this view again with the current settings\n(needed after changing the resolution)").clicked() {
                *action = Some(Action::Refresh);
            }
            if ui
                .add(egui::Button::new("🌸 Snap to flowers").selected(self.snap))
                .on_hover_text(
                    "When on, a click puts the zoom box on the centre of the largest flower near it: the simplest\n\
                     quadratic number within the box. Right-click on the image to centre the view on a flower.",
                )
                .clicked()
            {
                self.snap = !self.snap;
            }
            ui.separator();
            if popup(ui, self.is_open(Popup::View), "View...", "The view's exact centre and half-width, and the zoom factor") {
                *action = Some(Action::Toggle(Popup::View));
            }
            ui.separator();
            if ui
                .add_enabled(has_image, egui::Button::new("⭐ Save location").selected(self.is_open(Popup::SaveLocation)))
                .on_hover_text("Save this view's centre and half-width in a group (Ctrl+L)")
                .clicked()
            {
                *action = Some(Action::Toggle(Popup::SaveLocation));
            }
            if popup(ui, self.is_open(Popup::Locations), "📂 Locations...", "Your saved locations, in groups: go to them, sort them, make videos from them") {
                *action = Some(Action::Toggle(Popup::Locations));
            }
            ui.separator();
            let save = ui
                .add_enabled(has_image, egui::Button::new("💾 Save image"))
                .on_hover_text("Save the image as a PNG in the images folder (Ctrl+S).\nRight-click to choose the name and folder.");
            if save.clicked() {
                *action = Some(Action::SaveImage);
            }
            save.context_menu(|ui| {
                if ui.button("Save image as...").clicked() {
                    *action = Some(Action::SaveImageAs);
                    ui.close_menu();
                }
            });
            ui.separator();
            if popup(ui, self.is_open(Popup::Polynomials), "Polynomials...", "Which coefficients the polynomials may have") {
                *action = Some(Action::Toggle(Popup::Polynomials));
            }
            if popup(ui, self.is_open(Popup::Colours), "🎨 Colours...", "How the image is coloured") {
                *action = Some(Action::Toggle(Popup::Colours));
            }
            ui.separator();
            ui.menu_button("🎬 Video", |ui| self.video_menu(ui, shared, video, to_video));
            ui.menu_button("☰", |ui| {
                if ui.button("Folders...").clicked() {
                    *action = Some(Action::Toggle(Popup::Folders));
                    ui.close_menu();
                }
                ui.checkbox(&mut self.show_box, "Show zoom box");
                ui.separator();
                if ui.button("Reset all settings...").clicked() {
                    self.loc.confirm_reset = true;
                    ui.close_menu();
                }
                ui.separator();
                ui.hyperlink_to("User guide", format!("{}/blob/main/docs/USER-GUIDE.md", super::REPO));
                ui.label(format!("Algebraic Fractals {}", env!("CARGO_PKG_VERSION")));
                ui.weak("MIT or Apache-2.0 licence");
            });
            if let Some((name, done, n, left)) = &video.rendering {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let text = format!("video '{name}': frame {} of {n}{left}", (done + 1).min(*n));
                    if ui.link(text).on_hover_text("Show the video window").clicked() {
                        *to_video = Some(ToVideo::Show);
                    }
                });
            }
        });
    }

    fn video_menu(&mut self, ui: &mut egui::Ui, shared: &Shared, video: &VideoInfo, to_video: &mut Option<ToVideo>) {
        let mut pick = |ui: &mut egui::Ui, t: ToVideo| {
            *to_video = Some(t);
            ui.close_menu();
        };
        if ui.add(egui::Button::new("Video window").selected(video.open)).clicked() {
            pick(ui, ToVideo::Show);
        }
        if ui.button("New video").on_hover_text("From the whole picture to the view you're looking at").clicked() {
            pick(ui, ToVideo::New);
        }
        ui.menu_button("New video from group", |ui| {
            let mut any = false;
            for (i, g) in shared.groups.iter().enumerate().filter(|(_, g)| g.locations.len() >= 2) {
                any = true;
                if ui.button(format!("{} ({})", g.name, g.locations.len())).clicked() {
                    pick(ui, ToVideo::FromGroup(i));
                }
            }
            if !any {
                ui.weak("No group has 2 locations yet");
            }
        });
        ui.menu_button("Open video project", |ui| {
            let names = algfrac::video::project::saved_projects();
            if names.is_empty() {
                ui.weak("No saved projects yet");
            }
            for name in names {
                ui.horizontal(|ui| {
                    if ui.small_button("✖").on_hover_text("Delete this project").clicked() {
                        self.loc.confirm_project_delete = Some(name.clone());
                        ui.close_menu();
                    }
                    if ui.button(&name).clicked() {
                        pick(ui, ToVideo::Open(name.clone()));
                    }
                });
            }
            let old = algfrac::video::project::old_paths();
            if !old.is_empty() {
                ui.separator();
                ui.menu_button("Old paths", |ui| {
                    for name in old {
                        if ui.button(&name).on_hover_text("A path saved by the old video maker, brought in as a new project").clicked() {
                            pick(ui, ToVideo::OpenOld(name.clone()));
                        }
                    }
                });
            }
        });
        if let Some((name, done, n, _)) = &video.rendering {
            ui.separator();
            ui.add_enabled(false, egui::Button::new(format!("Rendering '{name}': frame {} of {n}", (done + 1).min(*n))));
            if ui.button("Stop the render").on_hover_text("Stop after the current frame and save what's done").clicked() {
                pick(ui, ToVideo::StopRender);
            }
        }
    }

    fn settings_strip(&mut self, ui: &mut egui::Ui, shared: &mut Shared, action: &mut Option<Action>) {
        let auto_tip = "Choose this automatically, so that the view has the target root density\n\
                        (predicted before rendering). Only one can be automatic.";
        ui.horizontal_wrapped(|ui| {
            ui.label("Order");
            ui.add_enabled(!self.auto_order, egui::DragValue::new(&mut self.search.order).range(1..=16).speed(0.05));
            if ui.checkbox(&mut self.auto_order, "auto").on_hover_text(auto_tip).changed() && self.auto_order {
                self.auto_max = false;
            }
            ui.add_space(12.0);
            ui.label("Max coefficient");
            super::widgets::max_coef_box(ui, !self.auto_max, &mut self.search.coefs.max);
            if ui.checkbox(&mut self.auto_max, "auto").on_hover_text(auto_tip).changed() && self.auto_max {
                self.auto_order = false;
            }
            ui.add_space(12.0);
            let auto = self.auto_order || self.auto_max;
            ui.add_enabled_ui(auto, |ui| {
                ui.label("Root density");
                ui.add(egui::DragValue::new(&mut self.target_density).range(0.01..=1000.0).speed(0.05).max_decimals(2))
                    .on_hover_text("The density of algebraic numbers that auto aims for, per 100 square pixels");
                ui.label("per 100 px²");
            })
            .response
            .on_disabled_hover_text("The target for auto: tick auto order or auto max coefficient to use it");
            let view = self.requested;
            let region = image_region(view, self.search.width, self.search.height);
            let dens = shared.predictor.per_pixel(self.search.order, &self.search.coefs, &region);
            ui.weak(format!("(predicted ~{})", sig(dens * 100.0))).on_hover_text(
                "Algebraic numbers per 100 px² in this view, estimated before rendering from the number\n\
                 of allowed polynomials and where the roots of random polynomials lie. Usually within about 20%.",
            );
            if !self.auto_note.is_empty() {
                ui.colored_label(ORANGE, "⚠").on_hover_text(format!("Auto: {}", self.auto_note));
            }
            ui.add_space(12.0);
            ui.label("Image");
            egui::ComboBox::from_id_salt("shape")
                .selected_text(self.shape.name())
                .show_ui(ui, |ui| {
                    for s in ImageShape::ALL {
                        ui.selectable_value(&mut self.shape, s, s.name());
                    }
                })
                .response
                .on_hover_text("Square, widescreen, or the size of the window's image area (re-rendered when the window changes size)");
            let tip = "Pixels. Changing it doesn't re-render: press Refresh or zoom in to use it.";
            match self.shape {
                ImageShape::Square => {
                    egui::ComboBox::from_id_salt("res")
                        .selected_text(format!("{0} x {0}", self.square))
                        .show_ui(ui, |ui| {
                            for r in RESOLUTIONS {
                                ui.selectable_value(&mut self.square, r, format!("{r} x {r}"));
                            }
                        })
                        .response
                        .on_hover_text(tip);
                }
                ImageShape::Wide => {
                    let w = |h: usize| algfrac::video::frame_width(h, 16.0 / 9.0);
                    egui::ComboBox::from_id_salt("wide res")
                        .selected_text(format!("{} x {}", w(self.wide_height), self.wide_height))
                        .show_ui(ui, |ui| {
                            for h in WIDE_HEIGHTS {
                                ui.selectable_value(&mut self.wide_height, h, format!("{} x {h}", w(h)));
                            }
                        })
                        .response
                        .on_hover_text(tip);
                }
                ImageShape::Fill => {
                    if let Some((w, h)) = self.fill_size {
                        ui.weak(format!("{w} x {h}"));
                    }
                }
            }
            if let Some(job) = &self.job {
                ui.add_space(12.0);
                let done = job.rows.load(Ordering::Relaxed);
                let frac = done as f32 / job.height.max(1) as f32;
                let el = job.started.elapsed().as_secs_f64();
                let eta = if done > 0 { format!("~{:.0}s left", el * (job.height - done.min(job.height)) as f64 / done as f64) } else { String::new() };
                ui.add(egui::ProgressBar::new(frac).desired_width(180.0).text(format!("{:.0}%  {eta}", frac * 100.0)));
                if ui.button("Cancel").clicked() {
                    *action = Some(Action::Cancel);
                }
            }
        });
    }

    /// Draws the fractal and handles mouse input on it.
    fn image(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let full = ui.available_rect_before_wrap();
        // the image area in pixels, for Fill
        let ppp = ui.ctx().pixels_per_point();
        let size = ((full.width() * ppp).round().max(16.0) as usize, (full.height() * ppp).round().max(16.0) as usize);
        self.fill_size = Some(size);
        let (Some(sh), Some(tex)) = (self.shown.take(), self.texture.as_ref().map(|t| t.id())) else {
            ui.centered_and_justified(|ui| ui.label("Rendering..."));
            return;
        };
        self.image_of(ui, &sh, tex, full, action);
        self.shown = Some(sh);
    }

    fn image_of(&mut self, ui: &mut egui::Ui, sh: &Shown, tex: egui::TextureId, full: Rect, action: &mut Option<Action>) {
        let (w, h) = (sh.render.width as f32, sh.render.height as f32);
        let scale = (full.width() / w).min(full.height() / h);
        let rect = Rect::from_center_size(full.center(), vec2(w * scale, h * scale));
        let resp = ui.allocate_rect(rect, Sense::click());
        let painter = ui.painter_at(full);
        painter.image(tex, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);

        let v = sh.view;
        // half the width and half the height of the view
        let (hx, hy) = (v.hw * sh.render.width as f64 / sh.render.height as f64, v.hw);
        let to_complex = move |p: Pos2| -> (f64, f64) {
            let u = ((p.x - rect.min.x) / rect.width()) as f64;
            let t = ((p.y - rect.min.y) / rect.height()) as f64;
            (v.cr - hx + 2.0 * hx * u, v.ci + hy - 2.0 * hy * t)
        };
        let to_screen = |re: f64, im: f64| -> Pos2 {
            pos2(
                rect.min.x + ((re - (v.cr - hx)) / (2.0 * hx)) as f32 * rect.width(),
                rect.min.y + (((v.ci + hy) - im) / (2.0 * hy)) as f32 * rect.height(),
            )
        };

        // mouse: a click chooses the next zoom's centre (on the nearest flower, when snapping)
        if let Some(p) = resp.interact_pointer_pos() {
            if resp.clicked() || resp.double_clicked() {
                let (x, y) = to_complex(p);
                self.target = Some((x, y));
                if self.snap {
                    match flower_near(x, y, v.hw / self.zoom) {
                        Ok((re, im, poly)) => {
                            self.target = Some((re, im));
                            self.status = format!("Snapped to the flower at {}, a root of {poly}", complex_text(re, im));
                        }
                        Err(e) => self.status = e,
                    }
                }
            }
            if resp.double_clicked() {
                *action = Some(Action::Run);
            }
        }
        if resp.hovered() {
            let dy = ui.input(|i| i.raw_scroll_delta.y);
            if dy != 0.0 {
                self.zoom = (self.zoom * 1.15f64.powf(dy as f64 / 40.0)).clamp(1.0, 1000.0);
                self.zoom_note = Some(Instant::now());
            }
        }
        if resp.secondary_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.loc.right_click = Some(to_complex(p));
            }
        }
        let zoom = self.zoom;
        let mut menu_action = None;
        let mut flower = None; // (the half-width to go to, around which point)
        resp.context_menu(|ui| {
            let Some((re, im)) = self.loc.right_click else { return };
            if ui.button("Zoom in here").clicked() {
                menu_action = Some(Action::Go(View { cr: re, ci: im, hw: v.hw / zoom }));
                ui.close_menu();
            }
            if ui.button("Centre here").clicked() {
                menu_action = Some(Action::Go(View { cr: re, ci: im, hw: v.hw }));
                ui.close_menu();
            }
            ui.separator();
            let tip = "The centre of the largest flower near here: the simplest quadratic number within the zoom box";
            if ui.button("🌸 Zoom into the flower here").on_hover_text(tip).clicked() {
                flower = Some((v.hw / zoom, re, im));
                ui.close_menu();
            }
            if ui.button("🌸 Centre on the flower here").on_hover_text(tip).clicked() {
                flower = Some((v.hw, re, im));
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Copy this point").clicked() {
                ui.ctx().copy_text(complex_text(re, im));
                ui.close_menu();
            }
        });
        if let Some((hw, x, y)) = flower {
            match flower_near(x, y, v.hw / zoom) {
                Ok((re, im, poly)) => {
                    menu_action = Some(Action::Go(View { cr: re, ci: im, hw }));
                    self.status = format!("Centred on the flower at {}, a root of {poly}", complex_text(re, im));
                }
                Err(e) => self.status = e,
            }
        }
        if menu_action.is_some() {
            *action = menu_action;
        }
        self.hover.clear();
        if let Some(p) = resp.hover_pos() {
            let (re, im) = to_complex(p);
            let r = &sh.render;
            let col = (((p.x - rect.min.x) / rect.width()) * (r.width - 1) as f32).round().clamp(0.0, (r.width - 1) as f32) as usize;
            let row = (((p.y - rect.min.y) / rect.height()) * (r.height - 1) as f32).round().clamp(0.0, (r.height - 1) as f32) as usize;
            let i = row * r.width + col;
            self.hover = format!(
                "X = {}    residual {:.3e}    p(x) = {}    length {}  height {}",
                fmt_complex(re, im),
                r.residual[i],
                poly_string(r.coeffs_at(row, col)),
                sh.stats.length(i),
                sh.stats.height(i),
            );
        }

        // zoom box
        if self.show_box {
            let (tr, ti) = self.target.unwrap_or((v.cr, v.ci));
            let (nx, ny) = (hx / self.zoom, hy / self.zoom);
            let stroke = Stroke::new(1.5_f32, YELLOW);
            painter.rect_stroke(Rect::from_two_pos(to_screen(tr - nx, ti + ny), to_screen(tr + nx, ti - ny)), 0.0, stroke, egui::StrokeKind::Middle);
            let c = to_screen(tr, ti);
            painter.line_segment([c - vec2(6.0, 0.0), c + vec2(6.0, 0.0)], stroke);
            painter.line_segment([c - vec2(0.0, 6.0), c + vec2(0.0, 6.0)], stroke);
        }
    }
}
