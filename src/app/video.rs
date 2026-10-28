//! The video window: a project bar, four tabs (Path, Colour, Polynomials, Output), the
//! waypoint strip (a small render of every waypoint, always in view), and the bottom
//! bar with the video's size, its render time and Render now.

use super::explore::{Explorer, VideoInfo};
use super::widgets::{coef_set_rows, colour_controls, folder_box, max_coef_box, open_with_system, set_texture, show_in_folder, ORANGE, YELLOW};
use super::{repaint_after, repaint_now, Shared};
use algfrac::colour::{ColourBy, Scale, Stats, Style};
use algfrac::common::*;
use algfrac::engine::{render_axes, work_per_pixel, Coefs, Plan, Render};
use algfrac::video::frame_width;
use algfrac::video::frames::*;
use algfrac::video::make::{make_video, output_paths, FrameDone, VideoSettings};
use algfrac::video::project::{self, Format, VideoProject, LAST_SESSION};
use eframe::egui::{self, vec2, Color32, Rect, Sense, Stroke, TextureHandle, TextureOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Thumbnail heights: small, medium, large.
const THUMB_HEIGHTS: [usize; 3] = [90, 135, 180];

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Path,
    Colour,
    Polynomials,
    Output,
}

/// A waypoint's thumbnail: its renders (one, or two to fade between), what they're of,
/// and its texture.
#[derive(Default)]
struct Thumb {
    key: Option<LevelsKey>,
    renders: Vec<(Render, Stats, f64)>,
    texture: Option<TextureHandle>,
}

enum StripMsg {
    /// thumbnail i's renders, for `key`
    Thumb(usize, LevelsKey, Vec<Arc<Render>>),
    /// the search work rendered and how long it took, for the render-time estimate
    Timing(f64, f64),
}

/// The thumbnails being rendered on a background thread.
struct StripJob {
    cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<StripMsg>,
}

#[derive(Default)]
struct Strip {
    thumbs: Vec<Thumb>,
    /// index into THUMB_HEIGHTS
    size: usize,
    /// what the last job was started for
    target: Vec<LevelsKey>,
    /// pending re-render after a change, and when it was asked for
    changed: Option<(Vec<LevelsKey>, Instant)>,
    job: Option<StripJob>,
    /// goes up whenever a thumbnail gets new renders
    gen: usize,
    /// what the textures show: (gen, colouring)
    drawn: Option<(usize, Colouring)>,
    /// the waypoints' length and height ranges, measured from the thumbnails
    lh_range: Option<((f64, f64), (f64, f64))>,
}

/// A rendered preview frame: its renders, each with its percentiles and weight.
struct Preview {
    renders: Vec<(Render, Stats, f64)>,
    frames: Frames,
    k: usize,
    /// (max coefficient, weight) of each render
    needs: Vec<(i32, f64)>,
    secs: f64,
    /// with a colour scale measured at the waypoints: every waypoint's levels
    held: Option<Vec<WaypointLevels>>,
}

/// Where a preview gets a waypoint's levels from.
enum End {
    Cached(WaypointLevels),
    /// the waypoint is the previewed frame itself
    Frame,
    /// rendered with the preview: the index of its renders
    Render(usize),
}

/// A preview render running on a background thread.
struct PreviewJob {
    frames: Frames,
    k: usize,
    needs: Vec<(i32, f64)>,
    ends: Vec<(LevelsKey, End)>,
    extra_lens: Vec<usize>,
    cancel: Arc<AtomicBool>,
    rows: Arc<AtomicUsize>,
    rows_total: usize,
    rx: mpsc::Receiver<(Vec<Arc<Render>>, f64, f64)>,
}

/// Messages from the video thread.
enum Msg {
    /// the estimated time to the end of each frame (cumulative), for the time left
    Estimate(Vec<f64>),
    Frame(FrameDone),
    Done(Result<String, String>),
}

/// The video being rendered.
pub struct RenderJob {
    name: String,
    frames: usize,
    est_done: Vec<f64>,
    rows_total: Arc<AtomicUsize>,
    done: usize,
    max: f64,
    last_secs: f64,
    bytes: usize,
    started: Instant,
    cancel: Arc<AtomicBool>,
    rows: Arc<AtomicUsize>,
    rx: mpsc::Receiver<Msg>,
    /// where it's written
    out: PathBuf,
}

/// What to do once unsaved changes are dealt with.
pub enum Leave {
    /// replace the project with this one, then show this status
    Replace(Box<VideoProject>, String),
    Close,
}

pub struct VideoWindow {
    pub project: VideoProject,
    /// the project as last saved or opened, to tell whether it has changed
    saved: Option<VideoProject>,
    /// take `saved` once auto has set the max coefficients
    snapshot: bool,
    tab: Tab,
    pub sel: usize,
    auto_notes: Vec<String>,
    auto_inputs: Option<AutoInputs>,
    /// each waypoint's centre and half-width text boxes, and the view they were filled from
    edits: Vec<(String, String, View)>,
    strip: Strip,
    /// "Look at a frame"
    look: bool,
    preview_k: usize,
    preview: Option<Preview>,
    preview_job: Option<PreviewJob>,
    requested: Option<(Frames, usize)>,
    changed: Option<((Frames, usize), Instant)>,
    preview_tex: Option<TextureHandle>,
    preview_drawn: Option<(usize, Colouring)>,
    preview_gen: usize,
    /// the levels of waypoints' frames measured so far, latest last
    levels_cache: Vec<(LevelsKey, WaypointLevels)>,
    /// search work per second of rendering, once measured
    secs_per_work: Option<f64>,
    /// (frames, waypoint levels, the render-time estimate it's for)
    estimate: Option<(Frames, bool, f64)>,
    estimating: Option<(Frames, bool, mpsc::Receiver<f64>)>,
    pub render: Option<RenderJob>,
    frame_tex: Option<TextureHandle>,
    /// the last render's message and output
    finished: Option<(String, Option<PathBuf>)>,
    status: Option<(String, Instant)>,
    ramp: Option<(Style, TextureHandle)>,
    /// unsaved changes to ask about, and what to do after
    pub pending: Option<Leave>,
    /// a name being typed for Save as...
    save_as: Option<String>,
    /// close the window (set when it should go)
    pub close: bool,
}

impl VideoWindow {
    pub fn new(project: VideoProject, thumb_size: usize) -> VideoWindow {
        let n = project.len();
        VideoWindow {
            project,
            saved: None,
            snapshot: true,
            tab: Tab::Path,
            sel: 0,
            auto_notes: vec![String::new(); n],
            auto_inputs: None,
            edits: Vec::new(),
            strip: Strip { size: thumb_size.min(THUMB_HEIGHTS.len() - 1), ..Default::default() },
            look: true,
            preview_k: 0,
            preview: None,
            preview_job: None,
            requested: None,
            changed: None,
            preview_tex: None,
            preview_drawn: None,
            preview_gen: 0,
            levels_cache: Vec::new(),
            secs_per_work: None,
            estimate: None,
            estimating: None,
            render: None,
            frame_tex: None,
            finished: None,
            status: None,
            ramp: None,
            pending: None,
            save_as: None,
            close: false,
        }
    }

    pub fn thumb_size(&self) -> usize {
        self.strip.size
    }

    pub fn unsaved(&self) -> bool {
        !self.snapshot && self.saved.as_ref() != Some(&self.project)
    }

    pub fn rendering(&self) -> bool {
        self.render.is_some()
    }

    fn say(&mut self, s: impl Into<String>) {
        self.status = Some((s.into(), Instant::now()));
    }

    /// Replaces the project (unsaved changes already dealt with).
    pub fn replace(&mut self, p: VideoProject, status: String) {
        if let Some(job) = self.preview_job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.project = p;
        self.snapshot = true;
        self.sel = 0;
        self.preview_k = 0;
        self.auto_inputs = None;
        self.edits.clear();
        self.say(status);
    }

    /// Does `leave` now, or asks first if there are unsaved changes.
    pub fn leave(&mut self, leave: Leave) {
        if self.unsaved() && !self.rendering() {
            self.pending = Some(leave);
        } else {
            self.do_leave(leave);
        }
    }

    fn do_leave(&mut self, leave: Leave) {
        match leave {
            Leave::Replace(p, s) => self.replace(*p, s),
            Leave::Close => self.close = true,
        }
    }

    pub fn info(&self) -> Option<(String, usize, usize, String)> {
        let job = self.render.as_ref()?;
        Some((job.name.clone(), job.done, job.frames, time_left(job)))
    }

    /// Saves the project under `name` (and makes that its name).
    fn save(&mut self, name: &str) {
        let name = name.trim().to_string();
        if name.is_empty() {
            self.say("Give the project a name first");
            return;
        }
        match project::save_project(&self.project, &name) {
            Ok(_) => {
                self.project.name = name.clone();
                self.saved = Some(self.project.clone());
                self.say(format!("Saved the project '{name}'"));
            }
            Err(e) => self.say(e),
        }
    }

    /// Keeps unsaved changes as the "(last session)" project, when the program closes.
    pub fn keep_last_session(&mut self) -> Option<String> {
        if !self.unsaved() && project::project_file(&self.project.name).exists() {
            return Some(self.project.name.clone());
        }
        project::save_project(&self.project, LAST_SESSION).ok().map(|_| LAST_SESSION.to_string())
    }

    /// Picks the max coefficients of the waypoints with auto on, if anything they depend
    /// on has changed.
    fn refresh_auto(&mut self, shared: &mut Shared) {
        let inputs = self.project.auto_inputs();
        if self.auto_inputs.as_ref() != Some(&inputs) {
            self.auto_notes = self.project.refresh_auto(&mut shared.predictor);
            self.auto_inputs = Some(inputs);
        }
        if self.snapshot {
            self.saved = Some(self.project.clone());
            self.snapshot = false;
        }
    }

    /// Picks up finished work: thumbnails, the preview, the estimate and the video. Called
    /// every frame, even while the window is closed (a render carries on; the thumbnails
    /// and preview wait until it's open again).
    pub fn poll(&mut self, ctx: &egui::Context, shared: &mut Shared, open: bool) {
        self.poll_render(ctx);
        if self.render.is_some() || !open {
            return;
        }
        self.refresh_auto(shared);
        self.poll_strip(shared);
        self.poll_preview(shared);
    }

    // ---- the waypoint strip ----

    /// The frames the thumbnails are rendered as: small, each frame on its own.
    fn thumb_frames(&self) -> Frames {
        Frames { height: THUMB_HEIGHTS[self.strip.size], reuse: false, aa: 1, ..self.project.frames.clone() }
    }

    fn poll_strip(&mut self, shared: &mut Shared) {
        let tf = self.thumb_frames();
        let n = self.project.len();
        let wanted: Vec<LevelsKey> = (0..n).map(|i| tf.levels_key(&[], i)).collect();
        if let Some(job) = &self.strip.job {
            for msg in job.rx.try_iter().collect::<Vec<_>>() {
                match msg {
                    StripMsg::Thumb(i, key, renders) => {
                        if wanted.get(i) == Some(&key) {
                            let t = &mut self.strip.thumbs[i];
                            t.renders = renders.iter().zip(&key.2).map(|(r, j)| (Render::clone(r), Stats::new(r), j.3)).collect();
                            t.key = Some(key);
                            self.strip.gen += 1;
                        }
                    }
                    StripMsg::Timing(work, secs) => {
                        if work > 0.0 && secs > 0.05 && self.preview.is_none() {
                            self.secs_per_work = Some(secs / work);
                        }
                    }
                }
            }
        }
        if self.strip.thumbs.len() != n {
            // waypoints added, removed or moved: keep the thumbnails that are still right
            let mut old: Vec<Thumb> = std::mem::take(&mut self.strip.thumbs);
            for key in &wanted {
                let t = old.iter().position(|t| t.key.as_ref() == Some(key)).map(|j| old.remove(j)).unwrap_or_default();
                self.strip.thumbs.push(t);
            }
            self.strip.gen += 1;
        }
        if self.strip.target == wanted {
            return;
        }
        match &self.strip.changed {
            Some((w, t)) if *w == wanted => {
                if t.elapsed() < Duration::from_millis(400) {
                    return;
                }
            }
            _ => {
                self.strip.changed = Some((wanted, Instant::now()));
                return;
            }
        }
        // start rendering the thumbnails that aren't right, the selected one first
        self.strip.changed = None;
        if let Some(job) = self.strip.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        for (i, key) in wanted.iter().enumerate() {
            if self.strip.thumbs[i].key.as_ref() != Some(key) {
                if let Some(j) = (0..n).find(|&j| self.strip.thumbs[j].key.as_ref() == Some(key)) {
                    let renders = self.strip.thumbs[j].renders.iter().map(|(r, s, w)| (r.clone(), clone_stats(s, r), *w)).collect();
                    self.strip.thumbs[i].renders = renders;
                    self.strip.thumbs[i].key = Some(key.clone());
                    self.strip.gen += 1;
                }
            }
        }
        let mut order: Vec<usize> = (0..n).filter(|&i| self.strip.thumbs[i].key.as_ref() != Some(&wanted[i])).collect();
        order.sort_by_key(|&i| i != self.sel);
        let todo: Vec<(usize, LevelsKey)> = order.into_iter().map(|i| (i, wanted[i].clone())).collect();
        self.strip.target = wanted;
        if todo.is_empty() {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let (c2, threads, ctx) = (cancel.clone(), shared.threads, shared.ctx.clone());
        std::thread::spawn(move || {
            let rows = AtomicUsize::new(0);
            let (mut work, started) = (0.0, Instant::now());
            for (i, key) in todo {
                let mut out = Vec::new();
                for (m, re, im, _) in &key.2 {
                    let Ok(plan) = Plan::new(key.0, &Coefs { max: *m, ..key.1 }, false) else { return };
                    work += work_per_pixel(&plan, re, im) * (re.len() * im.len()) as f64;
                    match render_axes(&plan, re.clone(), im.clone(), threads, &c2, &rows) {
                        Some(r) => out.push(Arc::new(r)),
                        None => return, // cancelled
                    }
                }
                let _ = tx.send(StripMsg::Thumb(i, key, out));
                repaint_now(&ctx);
            }
            let _ = tx.send(StripMsg::Timing(work, started.elapsed().as_secs_f64()));
        });
        self.strip.job = Some(StripJob { cancel, rx });
    }

    /// The waypoints' levels from the thumbnails, if they're all up to date.
    fn thumb_levels(&self, wanted: &[LevelsKey]) -> Vec<WaypointLevels> {
        let fresh = self.strip.thumbs.len() == wanted.len() && self.strip.thumbs.iter().zip(wanted).all(|(t, k)| t.key.as_ref() == Some(k));
        if !fresh || !self.project.colouring.needs_levels() {
            return Vec::new();
        }
        self.strip.thumbs.iter().map(|t| t.renders.iter().map(|(_, st, w)| (st.levels(), *w)).collect()).collect()
    }

    /// Re-colours the thumbnails if they or the colouring changed.
    fn colour_strip(&mut self, ctx: &egui::Context) {
        let c = self.project.colouring;
        if self.strip.drawn == Some((self.strip.gen, c)) {
            return;
        }
        let tf = self.thumb_frames();
        let wanted: Vec<LevelsKey> = (0..self.project.len()).map(|i| tf.levels_key(&[], i)).collect();
        let levels = self.thumb_levels(&wanted);
        // the waypoints' length and height ranges (shown in the Colour tab)
        let all: Vec<Scale> = self.strip.thumbs.iter().flat_map(|t| t.renders.iter().map(|(_, st, _)| st.scale(&c.style))).collect();
        self.strip.lh_range = (!all.is_empty()).then(|| {
            let r = |f: fn(&Scale) -> (f64, f64)| (all.iter().map(|s| f(s).0).fold(f64::INFINITY, f64::min), all.iter().map(|s| f(s).1).fold(0.0, f64::max));
            (r(|s| s.length), r(|s| s.height))
        });
        for (i, t) in self.strip.thumbs.iter_mut().enumerate() {
            if t.renders.is_empty() {
                continue;
            }
            let k = tf.path.stop_frame(i.min(tf.path.stops.len() - 1));
            let rgb = tf.finish(k, &t.renders, &c.style, &|st| c.scale(&tf.path, &levels, k, st));
            set_texture(ctx, &mut t.texture, &format!("thumb {i}"), tf.width(), tf.height, &rgb, TextureOptions::LINEAR);
        }
        self.strip.drawn = Some((self.strip.gen, c));
    }

    // ---- the frame preview ("Look at a frame") ----

    /// Renders preview frame `preview_k` in the background, replacing any running preview.
    fn start_preview(&mut self, shared: &mut Shared) {
        if let Some(job) = self.preview_job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        let frames = self.project.frames.clone();
        let k = self.preview_k.min(frames.count() - 1);
        self.requested = Some((frames.clone(), self.preview_k));
        self.changed = None;
        let ladder = frames.ladder();
        let pp = frames.reuse.then(|| frames.patch_plan(&ladder));
        let jobs = frames.jobs(&ladder, pp.as_ref(), k);
        // with a colour scale measured at the waypoints, every waypoint's levels:
        // measured before, from this frame, or rendered after it
        let mut ends = Vec::new();
        let mut extra: Vec<Vec<Job>> = Vec::new();
        if self.project.colouring.needs_levels() {
            for j in 0..frames.path.stops.len() {
                let key = frames.levels_key(&ladder, j);
                let end = if key.2 == jobs {
                    End::Frame
                } else if let Some((_, l)) = self.levels_cache.iter().find(|c| c.0 == key) {
                    End::Cached(l.clone())
                } else if let Some(x) = extra.iter().position(|e| *e == key.2) {
                    End::Render(x)
                } else {
                    extra.push(key.2.clone());
                    End::Render(extra.len() - 1)
                };
                ends.push((key, end));
            }
        }
        // every render, the frame's first
        let extra_lens = extra.iter().map(|e| e.len()).collect();
        let all: Vec<Job> = jobs.iter().cloned().chain(extra.into_iter().flatten()).collect();
        let plan = |j: &Job| Plan::new(frames.order, &frames.coefs_for(j.0), false);
        let plans = match all.iter().map(plan).collect::<Result<Vec<Plan>, _>>() {
            Ok(p) => p,
            Err(e) => {
                self.say(format!("Error: {e}"));
                return;
            }
        };
        let rows_total = all.iter().map(|j| j.2.len()).sum();
        let needs: Vec<(i32, f64)> = jobs.iter().map(|j| (j.0, j.3)).collect();
        let n_frame = jobs.len();
        let cancel = Arc::new(AtomicBool::new(false));
        let rows = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel();
        let (c2, r2, ctx, threads) = (cancel.clone(), rows.clone(), shared.ctx.clone(), shared.threads);
        std::thread::spawn(move || {
            // the search work of the frame's own renders, to time against
            let work = (0..n_frame).map(|i| work_per_pixel(&plans[i], &all[i].1, &all[i].2) * (all[i].1.len() * all[i].2.len()) as f64).sum();
            let started = Instant::now();
            let mut secs = 0.0;
            let mut out = Vec::with_capacity(all.len());
            for (i, (plan, (_, re, im, _))) in plans.iter().zip(all).enumerate() {
                match render_axes(plan, re, im, threads, &c2, &r2) {
                    Some(r) => out.push(Arc::new(r)),
                    None => return, // cancelled
                }
                if i + 1 == n_frame {
                    secs = started.elapsed().as_secs_f64();
                }
            }
            let _ = tx.send((out, work, secs));
            repaint_now(&ctx);
        });
        self.preview_job = Some(PreviewJob { frames, k, needs, ends, extra_lens, cancel, rows, rows_total, rx });
    }

    fn poll_preview(&mut self, shared: &mut Shared) {
        if let Some(job) = &self.preview_job {
            match job.rx.try_recv() {
                Ok((all, work, secs)) => {
                    let job = self.preview_job.take().unwrap();
                    let mut all = all.into_iter();
                    let renders: Vec<(Render, Stats, f64)> = all
                        .by_ref()
                        .take(job.needs.len())
                        .zip(&job.needs)
                        .map(|(r, &(_, weight))| {
                            let r = Render::clone(&r);
                            let st = job.frames.stats(&r);
                            (r, st, weight)
                        })
                        .collect();
                    let extra: Vec<Vec<Arc<Render>>> = job.extra_lens.iter().map(|&n| all.by_ref().take(n).collect()).collect();
                    if work > 0.0 && secs > 0.0 {
                        self.secs_per_work = Some(secs / work);
                    }
                    let mut held = Vec::new();
                    for (key, end) in job.ends {
                        let levels: WaypointLevels = match end {
                            End::Cached(l) => l,
                            End::Frame => renders.iter().map(|(_, st, w)| (st.levels(), *w)).collect(),
                            End::Render(x) => extra[x].iter().zip(&key.2).map(|(r, j)| (job.frames.stats(r).levels(), j.3)).collect(),
                        };
                        self.levels_cache.retain(|c| c.0 != key);
                        self.levels_cache.push((key, levels.clone()));
                        held.push(levels);
                    }
                    if self.levels_cache.len() > 64 {
                        self.levels_cache.drain(..self.levels_cache.len() - 64);
                    }
                    let held = (!held.is_empty()).then_some(held);
                    self.preview = Some(Preview { renders, frames: job.frames, k: job.k, needs: job.needs, secs, held });
                    self.preview_gen += 1;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.preview_job = None,
                Err(mpsc::TryRecvError::Empty) => repaint_after(&shared.ctx, 100),
            }
        }
        // settings changes re-render the preview after a short pause
        if self.tab == Tab::Path && self.look {
            let want = (self.project.frames.clone(), self.preview_k);
            if self.requested.as_ref() != Some(&want) {
                match &self.changed {
                    Some((w, t)) if *w == want => {
                        if t.elapsed() >= Duration::from_millis(400) {
                            self.start_preview(shared);
                        } else {
                            repaint_after(&shared.ctx, 50);
                        }
                    }
                    _ => {
                        self.changed = Some((want, Instant::now()));
                        repaint_after(&shared.ctx, 50);
                    }
                }
            }
        }
        self.update_estimate(shared);
    }

    fn colour_preview(&mut self, ctx: &egui::Context) {
        let Some(pv) = &self.preview else { return };
        let c = self.project.colouring;
        if self.preview_drawn == Some((self.preview_gen, c)) {
            return;
        }
        let same_path = pv.frames.path.stops.len() == self.project.len();
        let levels = match &pv.held {
            Some(l) if same_path && c.needs_levels() => l.clone(),
            _ => Vec::new(),
        };
        let rgb = pv.frames.finish(pv.k, &pv.renders, &c.style, &|st| c.scale(&pv.frames.path, &levels, pv.k, st));
        set_texture(ctx, &mut self.preview_tex, "preview", pv.frames.width(), pv.frames.height, &rgb, TextureOptions::LINEAR);
        self.preview_drawn = Some((self.preview_gen, c));
    }

    /// Rough total render time, scaled from a measured render's time by the work the
    /// video needs. Worked out in the background, since it runs a few searches.
    fn update_estimate(&mut self, shared: &Shared) {
        let Some(spw) = self.secs_per_work else { return };
        let (f, levels) = (&self.project.frames, self.project.colouring.needs_levels());
        if self.estimate.as_ref().is_some_and(|e| e.0 == *f && e.1 == levels) {
            return;
        }
        if let Some((ef, el, rx)) = &self.estimating {
            if ef == f && *el == levels {
                if let Ok(secs) = rx.try_recv() {
                    self.estimate = Some((f.clone(), levels, secs));
                    self.estimating = None;
                }
                return;
            }
        }
        let (tx, rx) = mpsc::channel();
        let (frames, ctx) = (f.clone(), shared.ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(frames.frame_secs(spw, levels).iter().sum());
            repaint_now(&ctx);
        });
        self.estimating = Some((f.clone(), levels, rx));
    }

    // ---- rendering the video ----

    fn start_render(&mut self, shared: &mut Shared) {
        if let Some(job) = self.preview_job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        if let Some(job) = self.strip.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
            self.strip.target.clear(); // finish the thumbnails afterwards
        }
        if self.unsaved() && !self.project.name.trim().is_empty() && self.project.name != LAST_SESSION {
            let name = self.project.name.clone();
            self.save(&name);
        }
        let p = &self.project;
        let (mp4, pngs) = output_paths(&p.output.folder, &p.output.file, p.output.format);
        let mut v = VideoSettings::of(p, shared.threads);
        // the waypoints' levels already measured by the frame preview (the rest are
        // measured when the render starts)
        if p.colouring.needs_levels() {
            let f = &v.frames;
            let ladder = f.ladder();
            v.levels = (0..f.path.stops.len())
                .map(|i| {
                    let key = f.levels_key(&ladder, i);
                    self.levels_cache.iter().find(|c| c.0 == key).map(|c| c.1.clone())
                })
                .collect();
        }
        let (count, start_max) = (v.frames.count(), v.frames.max(0));
        let rows_total = Arc::new(AtomicUsize::new(v.frames.height * v.frames.aa));
        let cancel = Arc::new(AtomicBool::new(false));
        let rows = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel();
        // the time left needs each frame's estimated time, which runs a few searches
        let (f2, spw, tx2, ctx2) = (v.frames.clone(), self.secs_per_work.unwrap_or(1e-9), tx.clone(), shared.ctx.clone());
        let levels = p.colouring.needs_levels();
        std::thread::spawn(move || {
            let est_done = f2.frame_secs(spw, levels).into_iter().scan(0.0, |t, s| {
                *t += s;
                Some(*t)
            });
            let _ = tx2.send(Msg::Estimate(est_done.collect()));
            repaint_now(&ctx2);
        });
        let out = mp4.clone().or(pngs.clone()).unwrap_or_default();
        let (c2, r2, t2, ctx) = (cancel.clone(), rows.clone(), rows_total.clone(), shared.ctx.clone());
        std::thread::spawn(move || {
            let tx3 = tx.clone();
            let ctx3 = ctx.clone();
            let res = make_video(&v, mp4.as_deref(), pngs.as_deref(), &c2, (&r2, &t2), &mut |f| {
                let _ = tx3.send(Msg::Frame(f));
                repaint_now(&ctx3);
            });
            let _ = tx.send(Msg::Done(res));
            repaint_now(&ctx);
        });
        self.render = Some(RenderJob {
            name: p.name.clone(),
            frames: count,
            est_done: Vec::new(),
            rows_total,
            done: 0,
            max: start_max,
            last_secs: 0.0,
            bytes: 0,
            started: Instant::now(),
            cancel,
            rows,
            rx,
            out,
        });
        self.finished = None;
        self.say(format!("Rendering {count} frames..."));
    }

    /// Stops the render after the current frame (it saves what's done).
    pub fn stop_render(&mut self) {
        if let Some(job) = &self.render {
            job.cancel.store(true, Ordering::Relaxed);
            self.say("Stopping after the current frame...");
        }
    }

    fn poll_render(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.render else { return };
        let msgs: Vec<Msg> = job.rx.try_iter().collect();
        let mut finished = None;
        for msg in msgs {
            match msg {
                Msg::Frame(f) => {
                    (job.done, job.max, job.last_secs, job.bytes) = (f.k + 1, f.max, f.secs, f.bytes);
                    set_texture(ctx, &mut self.frame_tex, "frame", f.w, f.h, &f.rgb, TextureOptions::LINEAR);
                }
                Msg::Estimate(est) => job.est_done = est,
                Msg::Done(r) => finished = Some(r),
            }
        }
        if let Some(r) = finished {
            let out = job.out.clone();
            let msg = match r {
                Ok(s) => s,
                Err(e) => format!("Error: {e}"),
            };
            let ok = msg.starts_with("Done") || msg.starts_with("Stopped after");
            self.finished = Some((msg.clone(), ok.then_some(out)));
            self.say(msg);
            self.render = None;
        } else {
            repaint_after(ctx, 200);
        }
    }

    // ---- the window ----

    /// Draws the window's contents.
    pub fn ui(&mut self, ctx: &egui::Context, shared: &mut Shared, explorer: &mut Explorer) {
        self.colour_strip(ctx);
        self.colour_preview(ctx);
        let busy = self.rendering();
        if !busy && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            let name = self.project.name.clone();
            self.save(&name);
        }
        egui::TopBottomPanel::top("project bar").show(ctx, |ui| {
            ui.add_space(3.0);
            ui.add_enabled_ui(!busy, |ui| self.project_bar(ui, shared, explorer));
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                for (t, name) in [(Tab::Path, "Path"), (Tab::Colour, "Colour"), (Tab::Polynomials, "Polynomials"), (Tab::Output, "Output")] {
                    ui.selectable_value(&mut self.tab, t, egui::RichText::new(name).size(15.0));
                }
            });
            ui.add_space(2.0);
        });
        egui::TopBottomPanel::bottom("bottom bar").show(ctx, |ui| {
            ui.add_space(3.0);
            if let Some((s, t)) = &self.status {
                if t.elapsed() < Duration::from_secs(10) || busy {
                    ui.weak(s);
                } else {
                    self.status = None;
                }
            }
            self.bottom_bar(ui, shared);
            ui.add_space(3.0);
        });
        let strip_height = THUMB_HEIGHTS[self.strip.size] as f32 + 90.0;
        egui::TopBottomPanel::bottom("strip").resizable(false).exact_height(strip_height).show(ctx, |ui| self.strip_ui(ui, shared, explorer));
        egui::CentralPanel::default().show(ctx, |ui| {
            if busy {
                self.render_panel(ui);
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
                Tab::Path => self.path_tab(ui, shared, explorer),
                Tab::Colour => self.colour_tab(ui, explorer),
                Tab::Polynomials => self.polynomials_tab(ui, explorer),
                Tab::Output => self.output_tab(ui),
            });
        });
        self.questions(ctx);
    }

    fn project_bar(&mut self, ui: &mut egui::Ui, shared: &mut Shared, explorer: &Explorer) {
        ui.horizontal(|ui| {
            ui.label("Project");
            ui.add(egui::TextEdit::singleline(&mut self.project.name).desired_width(200.0))
                .on_hover_text("The project's name, which it's saved under");
            if ui.button("💾 Save").on_hover_text("Save every setting of this video under its name (Ctrl+S)").clicked() {
                let name = self.project.name.clone();
                self.save(&name);
            }
            if ui.button("Save as...").clicked() {
                self.save_as = Some(self.project.name.clone());
            }
            ui.menu_button("📂 Open", |ui| {
                let names = project::saved_projects();
                if names.is_empty() {
                    ui.weak("No saved projects yet");
                }
                for name in names {
                    if ui.button(&name).clicked() {
                        match project::open_project(&name, &shared.videos_dir) {
                            Ok(p) => self.leave(Leave::Replace(Box::new(p), format!("Opened the project '{name}'"))),
                            Err(e) => self.say(e),
                        }
                        ui.close_menu();
                    }
                }
                let old = project::old_paths();
                if !old.is_empty() {
                    ui.separator();
                    ui.menu_button("Old paths", |ui| {
                        for name in old {
                            if ui.button(&name).clicked() {
                                match project::open_old_path(&name, &shared.videos_dir) {
                                    Ok(p) => self.leave(Leave::Replace(Box::new(p), format!("Brought in the old path '{name}': its colouring is the start's"))),
                                    Err(e) => self.say(e),
                                }
                                ui.close_menu();
                            }
                        }
                    });
                }
                ui.separator();
                if ui.button("Import...").on_hover_text("Open a project file from anywhere").clicked() {
                    ui.close_menu();
                    if let Some(f) = rfd::FileDialog::new().add_filter("Video project", &[project::EXTENSION]).pick_file() {
                        match project::open_project(&f.display().to_string(), &shared.videos_dir) {
                            Ok(p) => self.leave(Leave::Replace(Box::new(p), format!("Imported {}", f.display()))),
                            Err(e) => self.say(e),
                        }
                    }
                }
                if ui.button("Export...").on_hover_text("Save a copy of the project file anywhere, to move or share it").clicked() {
                    ui.close_menu();
                    let file = format!("{}.{}", file_safe(&self.project.name), project::EXTENSION);
                    if let Some(f) = rfd::FileDialog::new().set_file_name(file).add_filter("Video project", &[project::EXTENSION]).save_file() {
                        let r = std::fs::write(&f, self.project.to_text());
                        self.say(match r {
                            Ok(()) => format!("Exported {}", f.display()),
                            Err(e) => format!("Could not write {}: {e}", f.display()),
                        });
                    }
                }
            });
            ui.menu_button("New", |ui| {
                if ui.button("Blank").on_hover_text("From the whole picture to the explorer's view").clicked() {
                    let p = super::new_project(shared, explorer, None);
                    self.leave(Leave::Replace(Box::new(p), "A new project".into()));
                    ui.close_menu();
                }
                ui.menu_button("From group", |ui| {
                    for (i, g) in shared.groups.iter().enumerate().filter(|(_, g)| g.locations.len() >= 2) {
                        if ui.button(&g.name).clicked() {
                            let p = super::new_project(shared, explorer, Some(i));
                            self.leave(Leave::Replace(Box::new(p), format!("A new project from the group {}", g.name)));
                            ui.close_menu();
                        }
                    }
                });
            });
            if self.unsaved() {
                ui.colored_label(ORANGE, "● unsaved").on_hover_text("Changed since it was last saved or opened");
            }
        });
    }

    fn bottom_bar(&mut self, ui: &mut egui::Ui, shared: &mut Shared) {
        let (f, o) = (&self.project.frames, &self.project.output);
        let n = f.count();
        let px = (f.width() * f.height) as f64;
        let mb = if o.format.mp4() { px * QUALITY[o.quality].2 / 8.0 * n as f64 / 1e6 } else { 0.0 } + if o.format.png() { px * 4.0 * n as f64 / 1e6 } else { 0.0 };
        let summary = format!("{n} frames · {} at {} fps · {} x {}", duration_text(n as f64 / o.fps as f64), o.fps, f.width(), f.height);
        let levels = self.project.colouring.needs_levels();
        let time = self.estimate.as_ref().filter(|(ef, el, _)| ef == f && *el == levels).map(|e| e.2);
        let (mut start, mut stop) = (false, false);
        ui.horizontal(|ui| {
            ui.label(summary);
            ui.weak(format!("· ~{mb:.0} MB")).on_hover_text("Star fields need a lot of data to stay sharp; the real size depends on how busy the frames are");
            if let Some(secs) = time {
                ui.weak(format!("· ~{} to render", duration_text(secs)))
                    .on_hover_text("Scaled from a measured render by the work each frame needs.\nViews near the real axis can take longer.");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.render.is_some() {
                    stop = ui.button("⏹ Stop").on_hover_text("Stop after the current frame; the frames so far are kept as a shorter video").clicked();
                } else {
                    let go = egui::Button::new(egui::RichText::new("⏺ Render now").strong().size(16.0)).min_size(vec2(150.0, 30.0));
                    start = ui.add(go).on_hover_text("Render every frame and write the video (the project is saved first)").clicked();
                }
            });
        });
        if start {
            self.start_render(shared);
        }
        if stop {
            self.stop_render();
        }
    }

    fn render_panel(&mut self, ui: &mut egui::Ui) {
        let Some(job) = &self.render else { return };
        let rows = job.rows.load(Ordering::Relaxed) as f32 / job.rows_total.load(Ordering::Relaxed).max(1) as f32;
        let frac = (job.done as f32 + rows.min(1.0)) / job.frames as f32;
        ui.add(egui::ProgressBar::new(frac).text(format!("frame {} of {}{}", (job.done + 1).min(job.frames), job.frames, time_left(job))));
        ui.add(egui::ProgressBar::new(rows.min(1.0)).desired_height(6.0));
        ui.weak(format!(
            "elapsed {}, last frame {:.1} s, {:.0} MB so far, max coefficient {}",
            duration_text(job.started.elapsed().as_secs_f64()),
            job.last_secs,
            job.bytes as f64 / 1e6,
            max_text(job.max)
        ));
        ui.weak("The settings are locked while the video renders. The explorer keeps working, and this window can be closed.");
        if let Some(tex) = &self.frame_tex {
            let size = tex.size_vec2();
            let avail = ui.available_size();
            let s = (avail.x / size.x).min(avail.y / size.y).min(1.0);
            ui.centered_and_justified(|ui| ui.image((tex.id(), size * s)));
        }
    }

    fn path_tab(&mut self, ui: &mut egui::Ui, shared: &mut Shared, explorer: &mut Explorer) {
        let n = self.project.len();
        if self.edits.len() != n || (0..n).any(|i| self.edits[i].2 != self.project.frames.path.stops[i]) {
            self.edits = (0..n)
                .map(|i| {
                    let v = self.project.frames.path.stops[i];
                    let keep = self.edits.get(i).filter(|e| e.2 == v);
                    keep.cloned().unwrap_or_else(|| (complex_text(v.cr, v.ci), v.hw.to_string(), v))
                })
                .collect();
        }
        let fps = self.project.output.fps as f64;
        let mut op: Option<Box<dyn FnOnce(&mut VideoProject) -> Option<usize>>> = None;
        egui::Grid::new("waypoints").num_columns(8).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
            for h in ["#", "name", "centre", "half-width", "pan to the next", "keep flower size", "to the next", ""] {
                ui.strong(h);
            }
            ui.end_row();
            for i in 0..n {
                let label = if i == 0 { "1 Start".to_string() } else if i + 1 == n { format!("{} Finish", i + 1) } else { format!("{}", i + 1) };
                if ui.selectable_label(self.sel == i, label).on_hover_text("Select this waypoint").clicked() {
                    self.select(i);
                }
                ui.add_sized([110.0, 20.0], egui::TextEdit::singleline(&mut self.project.names[i]));
                let (c, h, _) = &mut self.edits[i];
                let rc = ui.add_sized([250.0, 20.0], egui::TextEdit::singleline(c)).on_hover_text("A complex number, e.g. 0.5+0.8i, -1.2-3e-4i, 2i or 0.5, 0.8");
                let rh = ui
                    .add_sized([150.0, 20.0], egui::TextEdit::singleline(h))
                    .on_hover_text("Half the frame's height (the explorer's half-width).\nWidescreen frames show more at the sides.");
                if rc.has_focus() || rh.has_focus() {
                    self.sel = i;
                }
                let parsed = parse_complex(c).zip(h.trim().parse::<f64>().ok().filter(|h| h.is_finite() && *h > 0.0));
                match parsed {
                    Some(((cr, ci), hw)) => {
                        let v = View { cr, ci, hw };
                        if v != self.project.frames.path.stops[i] {
                            self.project.frames.path.stops[i] = v;
                            self.edits[i].2 = v; // keep the text as typed
                        }
                    }
                    None => {
                        ui.painter().rect_stroke(rc.rect.union(rh.rect), 2.0, Stroke::new(1.0_f32, Color32::LIGHT_RED), egui::StrokeKind::Outside);
                    }
                }
                if i + 1 < n {
                    let mut pct = self.project.frames.path.pan_speed(i) * 100.0;
                    let drag = egui::DragValue::new(&mut pct).range(0.01..=50.0).speed(0.01).max_decimals(2).suffix("% / frame");
                    if ui
                        .add(drag)
                        .on_hover_text(
                            "How fast the view may move across the plane on the way to the next waypoint,\n\
                             as a percentage of the frame's height per frame. Zooming and panning happen together:\n\
                             the segment takes whichever needs more frames.",
                        )
                        .changed()
                    {
                        self.project.frames.path.pan.resize(n - 1, algfrac::video::DEFAULT_PAN);
                        self.project.frames.path.pan[i] = pct / 100.0;
                    }
                    let mut keep = self.project.frames.path.keeps_flower(i);
                    if ui
                        .checkbox(&mut keep, "")
                        .on_hover_text(
                            "Grow the max coefficient with the zoom on the way to the next waypoint, so the dark\n\
                             flower around a quadratic number stays the same size on screen while the stars around it\n\
                             get denser. Centre both waypoints on the flower (Centre on flower).",
                        )
                        .changed()
                    {
                        self.project.frames.path.flower.resize(n - 1, false);
                        self.project.frames.path.flower[i] = keep;
                    }
                    let steps = self.project.frames.path.steps(i);
                    ui.weak(format!("{steps} frames ({})", duration_text(steps as f64 / fps)));
                } else {
                    ui.label("");
                    ui.label("");
                    ui.label("");
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(i > 0, egui::Button::new("⏶").small()).on_hover_text("Move earlier").clicked() {
                        op = Some(Box::new(move |p| {
                            p.swap(i - 1);
                            Some(i - 1)
                        }));
                    }
                    if ui.add_enabled(i + 1 < n, egui::Button::new("⏷").small()).on_hover_text("Move later").clicked() {
                        op = Some(Box::new(move |p| {
                            p.swap(i);
                            Some(i + 1)
                        }));
                    }
                    if ui.add_enabled(n > 2, egui::Button::new("✖").small()).on_hover_text("Delete this waypoint").clicked() {
                        op = Some(Box::new(move |p| {
                            p.delete(i);
                            Some(i.min(p.len() - 1))
                        }));
                    }
                });
                ui.end_row();
            }
        });
        if (0..n).any(|i| self.edits[i].2 != self.project.frames.path.stops[i]) {
            ui.colored_label(Color32::LIGHT_RED, "can't read a centre or half-width (highlighted)");
        }
        ui.add_space(6.0);
        let sel = self.sel.min(n - 1);
        let here = explorer.current_view();
        ui.horizontal_wrapped(|ui| {
            if ui.button("➕ Add the explorer's view").on_hover_text("A new waypoint after the selected one, with the view the explorer is showing").clicked() {
                op = Some(Box::new(move |p| Some(p.add_after(sel, "explorer view", here))));
            }
            if ui.button("Set to the explorer's view").on_hover_text("Give the selected waypoint the view the explorer is showing").clicked() {
                op = Some(Box::new(move |p| {
                    p.frames.path.stops[sel] = here;
                    Some(sel)
                }));
            }
            if ui.button("Show in the explorer").on_hover_text("Look at the selected waypoint in the explorer").clicked() {
                let (v, ctx) = (self.project.frames.path.stops[sel], shared.ctx.clone());
                explorer.navigate(&ctx, shared, v);
                shared.focus_main = true;
            }
            if ui.button("Insert halfway").on_hover_text("A waypoint halfway along the segment after the selected one, where the video already passes").clicked() {
                op = Some(Box::new(move |p| Some(p.insert_halfway(sel))));
            }
            if ui
                .button("Centre on flower")
                .on_hover_text("Move the selected waypoint's centre exactly onto the simplest quadratic number within half a\nframe height: the centre of the largest flower there.")
                .clicked()
            {
                let s = self.project.centre_on_flower(sel);
                self.say(s);
            }
            if ui.button("Reverse").on_hover_text("Run the whole path backwards").clicked() {
                op = Some(Box::new(move |p| {
                    p.reverse();
                    Some(p.len() - 1 - sel)
                }));
            }
            ui.menu_button("Replace with a group", |ui| {
                for g in shared.groups.iter().filter(|g| g.locations.len() >= 2) {
                    if ui.button(format!("{} ({})", g.name, g.locations.len())).clicked() {
                        let stops: Vec<(String, View)> = g.locations.iter().map(|l| (l.name.clone(), l.view)).collect();
                        op = Some(Box::new(move |p| {
                            p.replace_waypoints(stops);
                            Some(0)
                        }));
                        ui.close_menu();
                    }
                }
            });
            if ui.button("Save as a group").on_hover_text("Save the waypoints as locations in a new group, named after the project").clicked() {
                let base = if self.project.name.trim().is_empty() { "video".to_string() } else { self.project.name.trim().to_string() };
                let name = (1..).map(|i| if i == 1 { base.clone() } else { format!("{base} {i}") }).find(|n| !shared.groups.iter().any(|g| &g.name == n)).unwrap();
                let p = &self.project;
                let locations = (0..n).map(|i| Location { name: p.names[i].clone(), view: p.frames.path.stops[i] }).collect();
                shared.groups.push(Group { name: name.clone(), locations });
                let s = match shared.store_groups() {
                    Ok(()) => format!("Saved {n} locations in the group '{name}'"),
                    Err(e) => format!("Could not save the locations: {e}"),
                };
                self.say(s);
            }
        });
        if let Some(op) = op {
            if let Some(s) = op(&mut self.project) {
                self.select(s);
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Zoom per frame");
            ui.add(egui::DragValue::new(&mut self.project.frames.path.rate).range(1.001..=4.0).speed(0.001).max_decimals(3).prefix("x "))
                .on_hover_text("How much each frame magnifies the last. 1.01 is slow and smooth, 1.05 is quick.");
        });
        let deepest = self.project.frames.path.stops.iter().map(|v| v.hw / v.cr.abs().max(v.ci.abs()).max(1e-300)).fold(f64::INFINITY, f64::min);
        if deepest < 1e-12 {
            ui.colored_label(ORANGE, "That's beyond 64-bit precision: the deepest frames will look blocky");
        }
        ui.add_space(6.0);
        let r = egui::CollapsingHeader::new("Look at a frame").default_open(self.look).show(ui, |ui| self.look_at_frame(ui));
        self.look = r.body_returned.is_some();
    }

    fn look_at_frame(&mut self, ui: &mut egui::Ui) {
        let n = self.project.frames.count();
        self.preview_k = self.preview_k.min(n - 1);
        let fps = self.project.output.fps as f64;
        ui.horizontal(|ui| {
            if ui.button("|<").on_hover_text("First frame").clicked() {
                self.preview_k = 0;
            }
            ui.add(egui::Slider::new(&mut self.preview_k, 0..=n - 1).text("frame"));
            if ui.button(">|").on_hover_text("Last frame").clicked() {
                self.preview_k = n - 1;
            }
            let t = self.preview_k as f64 / fps;
            ui.weak(format!("{}:{:04.1}", (t / 60.0).floor(), t % 60.0));
        });
        if let Some(job) = &self.preview_job {
            let frac = job.rows.load(Ordering::Relaxed) as f32 / job.rows_total.max(1) as f32;
            ui.add(egui::ProgressBar::new(frac.min(1.0)).desired_width(300.0).text(format!("rendering frame {}...", job.k)));
        }
        if let Some(pv) = &self.preview {
            let v = pv.frames.path.view(pv.k);
            let max = match &pv.needs[..] {
                [(a, _), (b, t)] => format!("{} (fading from {a} to {b}: {:.0}%)", max_text(pv.frames.max(pv.k)), t * 100.0),
                [(a, _)] => format!("{} (rendered at {a})", max_text(pv.frames.max(pv.k))),
                _ => String::new(),
            };
            ui.weak(format!("Frame {}: centre {}, half-width {:.4e}, max coefficient {max}, rendered in {:.2} s", pv.k, fmt_complex(v.cr, v.ci), v.hw, pv.secs));
        }
        if let Some(tex) = &self.preview_tex {
            let size = tex.size_vec2();
            let s = (ui.available_width().min(960.0) / size.x).min(1.0);
            ui.image((tex.id(), size * s));
        }
    }

    fn colour_tab(&mut self, ui: &mut egui::Ui, explorer: &mut Explorer) {
        ui.weak("One colouring for the whole video.");
        ui.horizontal(|ui| {
            if ui.button("Copy the explorer's colours").clicked() {
                self.project.colouring.style = explorer.style;
            }
            if ui.button("Use these colours in the explorer").clicked() {
                explorer.style = self.project.colouring.style;
            }
        });
        ui.add_space(4.0);
        colour_controls(ui, &mut self.project.colouring.style, &mut self.ramp, "video");
        ui.add_space(8.0);
        ui.strong("Colour scale for the whole video");
        let c = &mut self.project.colouring;
        let by_poly = matches!(c.style.by, ColourBy::Length | ColourBy::Height);
        let uses_res = !by_poly || c.style.shade;
        ui.add_enabled(uses_res, egui::Checkbox::new(&mut c.steady, "Steady brightness")).on_hover_text(
            "Stops the brightness flickering as stars appear and leave. The percentiles (at max % and min %)\n\
             are measured at the waypoints and change steadily between them, instead of each frame's own.",
        );
        if by_poly {
            let what = if c.style.by == ColourBy::Length { "length" } else { "height" };
            ui.label(format!("The {what} scale"));
            let tip = "Fixing the scale gives each length or height the same colour throughout the video,\nso colours don't shift as you zoom.";
            ui.radio_value(&mut c.lh, LhScale::Each, "Each frame its own").on_hover_text("Each frame spreads the colormap from its own lowest to its highest");
            ui.radio_value(&mut c.lh, LhScale::Waypoints, "Fixed from the waypoints").on_hover_text(tip);
            let measured = self.strip.lh_range.map(|(l, h)| if c.style.by == ColourBy::Length { l } else { h });
            let fixed = matches!(c.lh, LhScale::Fixed(..));
            if ui.radio(fixed, "Fixed by hand").on_hover_text(tip).clicked() && !fixed {
                let (lo, hi) = measured.unwrap_or((1.0, 100.0));
                c.lh = LhScale::Fixed(lo, hi);
            }
            if let LhScale::Fixed(lo, hi) = &mut c.lh {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    let (s_lo, s_hi) = ((*lo * 0.01).max(0.1), (*hi * 0.01).max(0.1));
                    ui.label("from");
                    ui.add(egui::DragValue::new(lo).range(1.0..=1e9).speed(s_lo).max_decimals(0));
                    ui.label("to");
                    ui.add(egui::DragValue::new(hi).range(1.0..=1e9).speed(s_hi).max_decimals(0));
                    if ui.add_enabled(measured.is_some(), egui::Button::new("Measure")).on_hover_text("The waypoints' range").clicked() {
                        if let Some(m) = measured {
                            (*lo, *hi) = m;
                        }
                    }
                });
            }
            if let (Some((lo, hi)), LhScale::Waypoints) = (measured, c.lh) {
                ui.weak(format!("{what}s {lo:.0} to {hi:.0}, from the waypoints"));
            }
        }
        ui.add_space(4.0);
        ui.weak("The strip's brightness comes from the thumbnails themselves, so it's a close guide to the video's, not exact.");
    }

    fn polynomials_tab(&mut self, ui: &mut egui::Ui, explorer: &Explorer) {
        let p = &mut self.project;
        egui::Grid::new("video search").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Order");
            ui.add(egui::DragValue::new(&mut p.frames.order).range(1..=16).speed(0.05));
            ui.end_row();
            coef_set_rows(ui, &mut p.frames.coefs, "video");
            ui.label("Root density");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut p.target).range(0.01..=1000.0).speed(0.05).max_decimals(2));
                ui.label("per 100 px² of video");
            })
            .response
            .on_hover_text(
                "The density of algebraic numbers that auto aims for at the waypoints.\n\
                 Auto picks whatever max coefficient that needs, however slow: check the render time.",
            );
            ui.end_row();
        });
        ui.weak(format!("Allowed: {}", allowed_text(&Coefs { max: p.frames.maxes[0], ..p.frames.coefs })));
        if ui.button("Copy the explorer's settings").on_hover_text("The explorer's order, coefficients and root density").clicked() {
            p.frames.order = explorer.search.order;
            p.frames.coefs = Coefs { max: p.frames.coefs.max, ..explorer.search.coefs };
            p.target = explorer.target_density;
        }
        ui.add_space(8.0);
        ui.strong("The max coefficient at each waypoint").on_hover_text("The frames between waypoints follow a smooth curve through these values");
        let n = p.len();
        egui::Grid::new("maxes").num_columns(5).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            for h in ["#", "name", "max coefficient", "auto", ""] {
                ui.strong(h);
            }
            ui.end_row();
            for i in 0..n {
                ui.label(format!("{}", i + 1));
                ui.label(p.label(i));
                let kept = i > 0 && p.frames.path.keeps_flower(i - 1);
                max_coef_box(ui, !p.auto[i] && !kept, &mut p.frames.maxes[i]);
                if kept {
                    ui.weak(format!("keeps the flower size from {i}"));
                } else {
                    ui.checkbox(&mut p.auto[i], "").on_hover_text(
                        "Choose the max coefficient that gives this waypoint's view the target root density.\nAlong a zoom in it never falls.",
                    );
                }
                match self.auto_notes.get(i).filter(|s| !s.is_empty()) {
                    Some(note) => ui.colored_label(ORANGE, note),
                    None => ui.label(""),
                };
                ui.end_row();
            }
        });
        if ui.button("Auto everywhere").on_hover_text("Turn on auto max coefficient at every waypoint").clicked() {
            p.auto.iter_mut().for_each(|a| *a = true);
        }
        ui.add_space(8.0);
        ui.strong("Rendering");
        ui.checkbox(&mut p.frames.reuse, "Reuse renders between frames").on_hover_text(
            "Render each part of the plane once, finely enough for several consecutive frames,\n\
             and make those frames by averaging it down: several times faster, and anti-aliased.\n\
             The max coefficient then moves in steps. Untick to render every frame separately.",
        );
        ui.add_enabled_ui(p.frames.reuse, |ui| {
            ui.horizontal(|ui| {
                ui.label("Coefficient steps");
                let order = p.frames.order;
                let text = |(name, g): (&str, f64)| format!("{name}: {:.0}% more polynomials (max +{:.1}%)", (g.exp() - 1.0) * 100.0, ((g / (order + 1) as f64).exp() - 1.0) * 100.0);
                egui::ComboBox::from_id_salt("steps").selected_text(text(COEF_STEPS[p.frames.step])).show_ui(ui, |ui| {
                    for (i, s) in COEF_STEPS.iter().enumerate() {
                        ui.selectable_value(&mut p.frames.step, i, text(*s));
                    }
                });
            });
        });
        ui.checkbox(&mut p.frames.fade, "Fade in new coefficients").on_hover_text(
            "As the max coefficient rises, render each frame with the max coefficients just below and\n\
             above its own and cross-fade, so new algebraic numbers fade in instead of appearing all at once.",
        );
        let deepest = (0..n).max_by(|&a, &b| p.frames.path.stops[b].hw.total_cmp(&p.frames.path.stops[a].hw)).unwrap_or(0);
        match Plan::new(p.frames.order, &p.frames.coefs_for(p.frames.maxes[deepest]), false) {
            Ok(plan) => ui.weak(format!("At the deepest waypoint: {:.2e} polynomials per pixel, searched in ~{:.2e} steps", plan.brute_len(), plan.steps())),
            Err(e) => ui.colored_label(Color32::LIGHT_RED, e),
        };
    }

    fn output_tab(&mut self, ui: &mut egui::Ui) {
        let p = &mut self.project;
        let (f, o) = (&mut p.frames, &mut p.output);
        egui::Grid::new("output").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("File format");
            egui::ComboBox::from_id_salt("format").selected_text(o.format.name()).show_ui(ui, |ui| {
                for fm in Format::ALL {
                    ui.selectable_value(&mut o.format, fm, fm.name());
                }
            })
            .response
            .on_hover_text("MP4: H.264, plays everywhere. PNG frames: a folder of numbered, lossless images, for editing in other software.");
            ui.end_row();
            ui.label("Shape");
            egui::ComboBox::from_id_salt("shape").selected_text(f.shape.name()).show_ui(ui, |ui| {
                for s in Shape::ALL {
                    ui.selectable_value(&mut f.shape, s, s.name());
                }
            });
            ui.end_row();
            ui.label("Resolution");
            let ratio = f.shape.ratio();
            let w = move |h: usize| frame_width(h, ratio);
            egui::ComboBox::from_id_salt("res").selected_text(format!("{} x {}", w(f.height), f.height)).show_ui(ui, |ui| {
                for h in HEIGHTS {
                    ui.selectable_value(&mut f.height, h, format!("{} x {h}", w(h)));
                }
            });
            ui.end_row();
            ui.label("Smoothing");
            let fits = move |aa: usize, h: usize| w(h) * h * aa * aa <= MAX_RENDER_PIXELS;
            if !fits(f.aa, f.height) {
                f.aa = (1..=3).rev().find(|&a| fits(a, f.height)).unwrap_or(1);
            }
            let name = |aa: usize| SMOOTHING.iter().find(|s| s.0 == aa).map_or("", |s| s.1);
            egui::ComboBox::from_id_salt("aa")
                .selected_text(name(f.aa))
                .show_ui(ui, |ui| {
                    for (aa, label) in SMOOTHING {
                        let ok = fits(aa, f.height);
                        ui.add_enabled_ui(ok, |ui| ui.selectable_value(&mut f.aa, aa, label));
                    }
                })
                .response
                .on_hover_text("Render each frame more finely and average it down. Stars no longer flicker\nas they cross pixel boundaries, and the video compresses better.");
            ui.end_row();
            ui.label("Frame rate");
            egui::ComboBox::from_id_salt("fps").selected_text(format!("{} fps", o.fps)).show_ui(ui, |ui| {
                for r in FPS {
                    ui.selectable_value(&mut o.fps, r, format!("{r} fps"));
                }
            });
            ui.end_row();
            ui.label("Quality");
            ui.add_enabled_ui(o.format.mp4(), |ui| {
                egui::ComboBox::from_id_salt("quality")
                    .selected_text(QUALITY[o.quality].0)
                    .show_ui(ui, |ui| {
                        for (q, (name, qp, _)) in QUALITY.iter().enumerate() {
                            ui.selectable_value(&mut o.quality, q, *name).on_hover_text(format!("H.264 quantiser {qp} (lower = better)"));
                        }
                    })
                    .response
                    .on_hover_text("Every frame is encoded at the same quality; busy frames make bigger files");
            });
            ui.end_row();
            ui.label("Folder");
            folder_box(ui, &mut o.folder, "video out");
            ui.end_row();
            ui.label("File name");
            ui.add_sized([260.0, 20.0], egui::TextEdit::singleline(&mut o.file)).on_hover_text("An existing file is never overwritten: _2, _3, ... is added instead");
            ui.end_row();
        });
        let (mp4, pngs) = output_paths(&o.folder, &o.file, o.format);
        for p in mp4.iter().chain(pngs.iter()) {
            ui.weak(p.display().to_string());
        }
        if let Some((msg, Some(out))) = &self.finished {
            ui.add_space(8.0);
            ui.label(msg);
            ui.horizontal(|ui| {
                if ui.button("Show in folder").clicked() {
                    show_in_folder(out);
                }
                if out.extension().is_some_and(|e| e == "mp4") && ui.button("▶ Play").clicked() {
                    open_with_system(out);
                }
            });
        }
    }

    fn select(&mut self, i: usize) {
        self.sel = i;
        self.preview_k = self.project.frames.path.stop_frame(i.min(self.project.len() - 1));
    }

    fn strip_ui(&mut self, ui: &mut egui::Ui, shared: &mut Shared, explorer: &mut Explorer) {
        let n = self.project.len();
        let tf = self.thumb_frames();
        let (tw, th) = (tf.width() as f32, tf.height as f32);
        let wanted: Vec<LevelsKey> = (0..n).map(|i| tf.levels_key(&[], i)).collect();
        let fps = self.project.output.fps as f64;
        let mut select = None;
        let mut show = None;
        let mut set_here = None;
        let mut insert = None;
        let mut delete = None;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.strong("Waypoints");
            ui.weak("click to select, double-click to look at it in the explorer, right-click for more");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // (right to left: these read small, medium, large)
                for (s, name) in [(2, "large"), (1, "medium"), (0, "small")] {
                    ui.selectable_value(&mut self.strip.size, s, name);
                }
                ui.weak("size");
            });
        });
        egui::ScrollArea::horizontal().id_salt("strip").show(ui, |ui| {
            ui.horizontal(|ui| {
                for i in 0..n {
                    if i > 0 {
                        let steps = self.project.frames.path.steps(i - 1);
                        ui.vertical(|ui| {
                            ui.add_space(th / 2.0 - 10.0);
                            ui.weak(duration_text(steps as f64 / fps));
                            ui.weak("➡");
                        });
                    }
                    ui.vertical(|ui| {
                        let (rect, resp) = ui.allocate_exact_size(vec2(tw, th), Sense::click());
                        let t = self.strip.thumbs.get(i);
                        let fresh = t.is_some_and(|t| t.key.as_ref() == Some(&wanted[i]));
                        match t.and_then(|t| t.texture.as_ref()) {
                            Some(tex) => {
                                let tint = if fresh { Color32::WHITE } else { Color32::from_gray(110) };
                                ui.painter().image(tex.id(), rect, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), tint);
                            }
                            None => {
                                ui.painter().rect_filled(rect, 0.0, Color32::from_gray(24));
                            }
                        }
                        if !fresh {
                            ui.put(Rect::from_center_size(rect.center(), vec2(20.0, 20.0)), egui::Spinner::new());
                        }
                        let stroke = if i == self.sel { Stroke::new(2.0_f32, YELLOW) } else { Stroke::new(1.0_f32, Color32::from_gray(70)) };
                        ui.painter().rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Outside);
                        if resp.clicked() {
                            select = Some(i);
                        }
                        if resp.double_clicked() {
                            show = Some(i);
                        }
                        resp.context_menu(|ui| {
                            if ui.button("Show in the explorer").clicked() {
                                show = Some(i);
                                ui.close_menu();
                            }
                            if ui.button("Set to the explorer's view").clicked() {
                                set_here = Some(i);
                                ui.close_menu();
                            }
                            if ui.button("Insert halfway after").clicked() {
                                insert = Some(i);
                                ui.close_menu();
                            }
                            if ui.add_enabled(n > 2, egui::Button::new("Delete")).clicked() {
                                delete = Some(i);
                                ui.close_menu();
                            }
                        });
                        let p = &self.project;
                        ui.label(egui::RichText::new(format!("{} {}", i + 1, p.label(i))).strong());
                        let auto = if p.auto[i] { " auto" } else { "" };
                        ui.horizontal(|ui| {
                            ui.weak(format!("hw {:.1e}  max {}{auto}", p.frames.path.stops[i].hw, p.frames.maxes[i]));
                            if let Some(note) = self.auto_notes.get(i).filter(|s| !s.is_empty()) {
                                ui.colored_label(ORANGE, "⚠").on_hover_text(note);
                            }
                        });
                    });
                }
            });
        });
        ui.add_space(4.0);
        let busy = self.rendering();
        if let Some(i) = select {
            self.select(i);
        }
        if let Some(i) = show {
            let (v, ctx) = (self.project.frames.path.stops[i], shared.ctx.clone());
            explorer.navigate(&ctx, shared, v);
            shared.focus_main = true;
        }
        if busy {
            return;
        }
        if let Some(i) = set_here {
            self.project.frames.path.stops[i] = explorer.current_view();
        }
        if let Some(i) = insert {
            let j = self.project.insert_halfway(i);
            self.select(j);
        }
        if let Some(i) = delete {
            self.project.delete(i);
            self.select(i.min(self.project.len() - 1));
        }
    }

    /// Unsaved changes, and the name for Save as.
    fn questions(&mut self, ctx: &egui::Context) {
        if self.pending.is_some() {
            let mut answer = None;
            egui::Window::new("Unsaved changes").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.label(format!("Save changes to '{}'?", self.project.name));
                ui.horizontal(|ui| {
                    for (a, text) in [(0, "Save"), (1, "Don't save"), (2, "Cancel")] {
                        if ui.button(text).clicked() {
                            answer = Some(a);
                        }
                    }
                });
            });
            match answer {
                Some(0) => {
                    let name = self.project.name.clone();
                    self.save(&name);
                    if !self.unsaved() {
                        let l = self.pending.take().unwrap();
                        self.do_leave(l);
                    }
                }
                Some(1) => {
                    let l = self.pending.take().unwrap();
                    self.do_leave(l);
                }
                Some(_) => self.pending = None,
                None => {}
            }
        }
        if let Some(name) = &mut self.save_as {
            let mut done = None;
            egui::Window::new("Save as").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                let r = ui.text_edit_singleline(name);
                r.request_focus();
                let exists = project::project_file(name).exists();
                if exists {
                    ui.colored_label(ORANGE, "A project with this name exists: saving replaces it");
                }
                ui.horizontal(|ui| {
                    if ui.button(if exists { "Replace" } else { "Save" }).clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                        done = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        done = Some(false);
                    }
                });
            });
            match done {
                Some(true) => {
                    let name = name.clone();
                    self.save_as = None;
                    self.save(&name);
                }
                Some(false) => self.save_as = None,
                None => {}
            }
        }
    }
}

/// ", ~3 min left": the time so far, scaled by how much of the estimated time is left
/// (later frames, with larger max coefficients, are slower).
fn time_left(job: &RenderJob) -> String {
    let el = job.started.elapsed().as_secs_f64();
    match (job.done, job.est_done.last()) {
        (done, Some(&total)) if done > 0 && done <= job.est_done.len() => {
            let so_far = job.est_done[done - 1].max(1e-9);
            format!(", ~{} left", duration_text(el * (total - so_far).max(0.0) / so_far))
        }
        _ => String::new(),
    }
}

/// Stats for a render already measured (they're cheap to make again at thumbnail size).
fn clone_stats(_st: &Stats, r: &Render) -> Stats {
    Stats::new(r)
}

impl VideoInfo {
    pub fn of(open: bool, v: Option<&VideoWindow>) -> VideoInfo {
        VideoInfo { open, rendering: v.and_then(|v| v.info()) }
    }
}
