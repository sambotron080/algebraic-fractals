//! A video project: every setting of one video (the path, the max coefficients, the
//! colouring and the output), its file (`projects/<name>.afvideo` in the settings
//! folder), the edits made to its waypoints, and bringing in the old video maker's paths.

use super::frames::{auto_maxes, AutoInputs, Colouring, Frames, LhScale, Shape, COEF_STEPS, FPS, HEIGHTS, QUALITY};
use super::{simplest_quadratic, ZoomPath, DEFAULT_PAN};
use crate::colour::{ColourBy, Colormap, Style};
use crate::common::{complex_text, file_safe, quadratic_text, settings_dir, OldPlace, View, MAX_COEF};
use crate::density::Predictor;
use crate::engine::{CoefSet, Coefs};
use std::collections::HashMap;
use std::path::PathBuf;

pub const EXTENSION: &str = "afvideo";
/// The project kept when the program closes with unsaved changes.
pub const LAST_SESSION: &str = "(last session)";

/// What a video is written as.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Mp4,
    Png,
    Both,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Mp4, Format::Png, Format::Both];

    pub fn name(self) -> &'static str {
        match self {
            Format::Mp4 => "MP4 video",
            Format::Png => "PNG frames",
            Format::Both => "MP4 and PNG frames",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::Png => "png",
            Format::Both => "both",
        }
    }

    pub fn mp4(self) -> bool {
        self != Format::Png
    }

    pub fn png(self) -> bool {
        self != Format::Mp4
    }
}

/// Where and how the video is written (its size and shape are in `Frames`).
#[derive(Clone, PartialEq, Debug)]
pub struct Output {
    pub format: Format,
    pub fps: u32,
    /// index into QUALITY
    pub quality: usize,
    pub folder: String,
    /// the file name, without .mp4
    pub file: String,
}

/// Every setting of one video.
#[derive(Clone, PartialEq, Debug)]
pub struct VideoProject {
    pub name: String,
    pub frames: Frames,
    /// each waypoint's label (parallel to frames.path.stops)
    pub names: Vec<String>,
    /// whether each waypoint's max coefficient is chosen automatically
    pub auto: Vec<bool>,
    /// algebraic numbers per 100 px² (of the video) that auto aims for
    pub target: f64,
    pub colouring: Colouring,
    pub output: Output,
}

impl VideoProject {
    /// A new project through `stops` (at least two), every max coefficient on auto, with
    /// the default path, rendering and output settings.
    pub fn new(name: &str, stops: Vec<(String, View)>, order: usize, coefs: Coefs, target: f64, style: Style, folder: &str) -> VideoProject {
        let n = stops.len();
        let coefs = Coefs { zero: true, ..coefs }; // 0 is always an allowed coefficient
        let (names, views): (Vec<String>, Vec<View>) = stops.into_iter().unzip();
        VideoProject {
            name: name.to_string(),
            frames: Frames {
                path: ZoomPath::new(views, 1.01),
                maxes: vec![coefs.max.clamp(1, MAX_COEF); n],
                order,
                coefs,
                fade: true,
                reuse: true,
                step: 1,
                height: 720,
                shape: Shape::Wide,
                aa: 1,
            },
            names,
            auto: vec![true; n],
            target,
            colouring: Colouring { style, ..Colouring::default() },
            output: Output { format: Format::Mp4, fps: 30, quality: 1, folder: folder.to_string(), file: name.to_string() },
        }
    }

    pub fn len(&self) -> usize {
        self.frames.path.stops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// "Start", "Finish", or the waypoint's own name.
    pub fn label(&self, i: usize) -> String {
        match i {
            0 => "Start".into(),
            _ if i + 1 == self.len() => "Finish".into(),
            _ => self.names[i].clone(),
        }
    }

    /// What the automatic max coefficients depend on.
    pub fn auto_inputs(&self) -> AutoInputs {
        let f = &self.frames;
        AutoInputs {
            stops: f.path.stops.clone(),
            auto: self.auto.clone(),
            manual: f.maxes.iter().zip(&self.auto).map(|(&m, &a)| if a { 0 } else { m }).collect(),
            flower: (0..self.len().saturating_sub(1)).map(|i| f.path.keeps_flower(i)).collect(),
            order: f.order,
            coefs: Coefs { max: 1, ..f.coefs },
            target: self.target,
            height: f.height,
            shape: f.shape,
        }
    }

    /// Sets the max coefficients of the waypoints with auto on (and those kept by the
    /// flower size); returns auto's notes.
    pub fn refresh_auto(&mut self, p: &mut Predictor) -> Vec<String> {
        let notes;
        (self.frames.maxes, notes) = auto_maxes(&self.auto_inputs(), &self.frames.maxes, p);
        notes
    }

    /// Makes the per-segment lists one shorter than the waypoints.
    fn fix_segments(&mut self) {
        let n = self.len().saturating_sub(1);
        let p = &mut self.frames.path;
        p.pan.resize(n, DEFAULT_PAN);
        p.flower.resize(n, false);
    }

    /// Inserts a waypoint at `i` (0 < i), with its own settings, and the segment from it
    /// to the next at `pan` and `flower`.
    fn insert(&mut self, i: usize, name: String, view: View, max: i32, auto: bool, pan: f64, flower: bool) {
        self.fix_segments();
        let f = &mut self.frames;
        f.path.stops.insert(i, view);
        // segment i - 1 keeps its speed (now to the new waypoint); the new one goes on to the next
        if i < f.path.stops.len() - 1 {
            f.path.pan.insert(i, pan);
            f.path.flower.insert(i, flower);
        } else {
            f.path.pan.push(pan);
            f.path.flower.push(flower);
        }
        f.maxes.insert(i, max);
        self.names.insert(i, name);
        self.auto.insert(i, auto);
    }

    /// Adds a waypoint with `view` after waypoint `i`, with an automatic max coefficient
    /// and the default pan speed. Returns its index.
    pub fn add_after(&mut self, i: usize, name: &str, view: View) -> usize {
        let i = i.min(self.len() - 1);
        let max = self.frames.maxes[i];
        self.insert(i + 1, name.to_string(), view, max, true, DEFAULT_PAN, false);
        i + 1
    }

    /// Adds a waypoint in the middle of the segment after waypoint `i` (or before it, for
    /// the finish), where the video already passes: the video doesn't change until it's
    /// moved. Returns its index.
    pub fn insert_halfway(&mut self, i: usize) -> usize {
        self.fix_segments();
        let i = i.min(self.len() - 2); // segment i .. i + 1
        let p = &self.frames.path;
        let k = p.stop_frame(i) + p.steps(i) / 2;
        let (view, pan, flower) = (p.view(k), p.pan_speed(i), p.keeps_flower(i));
        let max = self.frames.max(k).round() as i32;
        let auto = self.auto[i] && self.auto[i + 1];
        self.insert(i + 1, format!("waypoint {}", i + 2), view, max, auto, pan, flower);
        i + 1
    }

    /// Removes waypoint `i` (keeping at least two). The segments either side merge, with
    /// the earlier one's pan speed and flower setting.
    pub fn delete(&mut self, i: usize) {
        let n = self.len();
        if n <= 2 || i >= n {
            return;
        }
        self.fix_segments();
        let f = &mut self.frames;
        f.path.stops.remove(i);
        f.path.pan.remove(i.min(n - 2));
        f.path.flower.remove(i.min(n - 2));
        f.maxes.remove(i);
        self.names.remove(i);
        self.auto.remove(i);
    }

    /// Swaps waypoints `i` and `i + 1` (their views, names and max coefficients; the
    /// segments stay where they are).
    pub fn swap(&mut self, i: usize) {
        if i + 1 >= self.len() {
            return;
        }
        let f = &mut self.frames;
        f.path.stops.swap(i, i + 1);
        f.maxes.swap(i, i + 1);
        self.names.swap(i, i + 1);
        self.auto.swap(i, i + 1);
    }

    /// Runs the whole path backwards.
    pub fn reverse(&mut self) {
        self.fix_segments();
        let f = &mut self.frames;
        f.path.stops.reverse();
        f.path.pan.reverse();
        f.path.flower.reverse();
        f.maxes.reverse();
        self.names.reverse();
        self.auto.reverse();
    }

    /// Replaces the waypoints with `stops` (at least two), every max coefficient on auto.
    pub fn replace_waypoints(&mut self, stops: Vec<(String, View)>) {
        let n = stops.len();
        let (names, views): (Vec<String>, Vec<View>) = stops.into_iter().unzip();
        let rate = self.frames.path.rate;
        self.frames.path = ZoomPath::new(views, rate);
        self.frames.maxes = vec![self.frames.maxes[0]; n];
        self.names = names;
        self.auto = vec![true; n];
    }

    /// Moves waypoint `i`'s centre exactly onto the simplest quadratic number within half
    /// its frame height: the centre of the largest flower there. Returns the status message.
    pub fn centre_on_flower(&mut self, i: usize) -> String {
        let v = self.frames.path.stops[i];
        match simplest_quadratic(v.cr, v.ci, v.hw / 2.0, 1_000_000) {
            Some((abc, re, im)) => {
                self.frames.path.stops[i] = View { cr: re, ci: im, ..v };
                format!("Waypoint {} centred on {}, a root of {}", i + 1, complex_text(re, im), quadratic_text(abc))
            }
            None => format!("No quadratic number near waypoint {} (it's too close to the real axis?)", i + 1),
        }
    }

    /// The settings as `key<tab>value` lines, a `wp` line per waypoint.
    pub fn to_text(&self) -> String {
        let (f, s, o) = (&self.frames, &self.colouring.style, &self.output);
        let mut t = String::from("# Algebraic Fractals video project\nversion\t1\n");
        let mut kv = |k: &str, v: String| t += &format!("{k}\t{v}\n");
        kv("name", one_line(&self.name));
        kv("rate", f.path.rate.to_string());
        let mut wps = String::new();
        for i in 0..self.len() {
            let v = f.path.stops[i];
            wps += &format!(
                "wp\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                one_line(&self.names[i]),
                v.cr,
                v.ci,
                v.hw,
                f.maxes[i],
                self.auto[i] as u8,
                f.path.pan_speed(i),
                f.path.keeps_flower(i) as u8
            );
        }
        kv("order", f.order.to_string());
        kv("set", f.coefs.set.name().into());
        kv("zero", (f.coefs.zero as u8).to_string());
        kv("zero_root", (f.coefs.zero_root as u8).to_string());
        kv("target", self.target.to_string());
        kv("reuse", (f.reuse as u8).to_string());
        kv("step", f.step.to_string());
        kv("fade", (f.fade as u8).to_string());
        kv("colour_by", s.by.short().into());
        kv("colormap", s.cmap.name().into());
        for (k, v) in [
            ("n", s.n),
            ("high", s.high),
            ("low", s.low),
            ("gamma", s.gamma),
            ("cycles", s.cycles),
            ("shift", s.shift),
            ("brightness", s.brightness),
            ("contrast", s.contrast),
            ("saturation", s.saturation),
        ] {
            kv(k, v.to_string());
        }
        for (k, v) in [("invert", s.invert), ("shade", s.shade), ("log_poly", s.log_poly), ("smooth", s.smooth), ("steady", self.colouring.steady)] {
            kv(k, (v as u8).to_string());
        }
        kv(
            "lh_scale",
            match self.colouring.lh {
                LhScale::Each => "each".into(),
                LhScale::Waypoints => "waypoints".into(),
                LhScale::Fixed(lo, hi) => format!("{lo},{hi}"),
            },
        );
        kv("format", o.format.short().into());
        kv("shape", f.shape.short().into());
        kv("height", f.height.to_string());
        kv("smoothing", f.aa.to_string());
        kv("fps", o.fps.to_string());
        kv("quality", QUALITY[o.quality].0.into());
        kv("folder", one_line(&o.folder));
        kv("file", one_line(&o.file));
        t + &wps
    }

    /// Reads a project file. Unknown keys are ignored, and missing ones take the defaults
    /// (with the videos folder `folder`).
    pub fn from_text(text: &str, folder: &str) -> Result<VideoProject, String> {
        let (mut wps, mut kv) = (Vec::new(), HashMap::new());
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            match line.split_once('\t') {
                Some(("wp", rest)) => {
                    let f: Vec<&str> = rest.split('\t').collect();
                    let num = |i: usize| f.get(i).and_then(|v| v.trim().parse::<f64>().ok()).filter(|v| v.is_finite());
                    let view = match (num(1), num(2), num(3)) {
                        (Some(cr), Some(ci), Some(hw)) if hw > 0.0 => View { cr, ci, hw },
                        _ => return Err("a waypoint line couldn't be read".into()),
                    };
                    let max = num(4).map_or(12, |m| (m as i32).clamp(1, MAX_COEF));
                    let auto = num(5).is_none_or(|a| a != 0.0);
                    let pan = num(6).filter(|p| *p > 0.0).map_or(DEFAULT_PAN, |p| p.clamp(1e-4, 0.5));
                    let flower = num(7).is_some_and(|v| v != 0.0);
                    wps.push((f[0].to_string(), view, max, auto, pan, flower));
                }
                Some((key, value)) => {
                    kv.insert(key.trim().to_string(), value.trim().to_string());
                }
                None => {}
            }
        }
        if wps.len() < 2 {
            return Err("a video needs at least two waypoints".into());
        }
        let get = |k: &str| kv.get(k).map(String::as_str);
        let num = |k: &str| get(k).and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite());
        let flag = |k: &str, d: bool| num(k).map_or(d, |v| v != 0.0);
        let d = Style::default();
        let style = Style {
            by: get("colour_by").and_then(|t| ColourBy::ALL.into_iter().find(|b| b.short() == t)).unwrap_or(d.by),
            cmap: get("colormap").and_then(|t| Colormap::ALL.into_iter().find(|c| c.name() == t)).unwrap_or(d.cmap),
            n: num("n").map_or(d.n, |v| v.clamp(0.05, 5.0)),
            high: num("high").map_or(d.high, |v| v.clamp(90.0, 100.0)),
            low: num("low").map_or(d.low, |v| v.clamp(0.0, 100.0)),
            gamma: num("gamma").map_or(d.gamma, |v| v.clamp(0.1, 10.0)),
            cycles: num("cycles").map_or(d.cycles, |v| v.clamp(1.0, 50.0)),
            shift: num("shift").map_or(d.shift, |v| v.clamp(0.0, 1.0)),
            brightness: num("brightness").map_or(d.brightness, |v| v.clamp(-0.5, 0.5)),
            contrast: num("contrast").map_or(d.contrast, |v| v.clamp(0.0, 4.0)),
            saturation: num("saturation").map_or(d.saturation, |v| v.clamp(0.0, 3.0)),
            invert: flag("invert", d.invert),
            shade: flag("shade", d.shade),
            log_poly: flag("log_poly", d.log_poly),
            smooth: flag("smooth", d.smooth),
        };
        let lh = match get("lh_scale") {
            Some("each") => LhScale::Each,
            Some(v) => match v.split_once(',').map(|(a, b)| (a.trim().parse::<f64>(), b.trim().parse::<f64>())) {
                Some((Ok(lo), Ok(hi))) if lo.is_finite() && hi.is_finite() => LhScale::Fixed(lo, hi),
                _ => LhScale::Waypoints,
            },
            None => LhScale::Waypoints,
        };
        let name = get("name").unwrap_or("my zoom").to_string();
        let n = wps.len();
        let path = ZoomPath {
            stops: wps.iter().map(|w| w.1).collect(),
            rate: num("rate").map_or(1.01, |r| r.clamp(1.001, 4.0)),
            pan: wps[..n - 1].iter().map(|w| w.4).collect(),
            flower: wps[..n - 1].iter().map(|w| w.5).collect(),
        };
        let aa = num("smoothing").map_or(1, |v| (v as usize).clamp(1, 3));
        Ok(VideoProject {
            frames: Frames {
                path,
                maxes: wps.iter().map(|w| w.2).collect(),
                order: num("order").map_or(3, |o| (o as usize).clamp(1, 16)),
                coefs: Coefs {
                    set: get("set").and_then(|t| CoefSet::ALL.into_iter().find(|s| s.name() == t)).unwrap_or(CoefSet::Ints),
                    max: wps[0].2,
                    zero: true, // (always allowed now; older files may say otherwise)
                    zero_root: flag("zero_root", true),
                },
                fade: flag("fade", true),
                reuse: flag("reuse", true),
                step: num("step").map_or(1, |v| (v.max(0.0) as usize).min(COEF_STEPS.len() - 1)),
                height: num("height").map(|h| h as usize).filter(|h| HEIGHTS.contains(h)).unwrap_or(720),
                shape: get("shape").and_then(|t| Shape::ALL.into_iter().find(|s| s.short() == t)).unwrap_or(Shape::Wide),
                aa,
            },
            names: wps.iter().map(|w| w.0.clone()).collect(),
            auto: wps.iter().map(|w| w.3).collect(),
            target: num("target").map_or(4.0, |t| t.clamp(0.01, 1000.0)),
            colouring: Colouring { style, steady: flag("steady", false), lh },
            output: Output {
                format: get("format").and_then(|t| Format::ALL.into_iter().find(|f| f.short() == t)).unwrap_or(Format::Mp4),
                fps: num("fps").map(|f| f as u32).filter(|f| FPS.contains(f)).unwrap_or(30),
                quality: get("quality").and_then(|t| QUALITY.iter().position(|q| q.0 == t)).unwrap_or(1),
                folder: get("folder").filter(|f| !f.is_empty()).unwrap_or(folder).to_string(),
                file: get("file").filter(|f| !f.is_empty()).unwrap_or(&name).to_string(),
            },
            name,
        })
    }

    /// A project from the old video maker's saved path (a .path file), named `name`: the
    /// waypoints and the path's settings, the start's colouring for the whole video, and
    /// the default output.
    pub fn from_old_path(text: &str, name: &str, folder: &str) -> Result<VideoProject, String> {
        let (mut wps, mut kv) = (Vec::new(), HashMap::new());
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            match line.split_once('\t') {
                Some(("wp", rest)) => {
                    let mut parts = rest.splitn(3, '\t');
                    let (auto, pan, place) = (parts.next(), parts.next(), parts.next());
                    let place = place.and_then(OldPlace::from_line).ok_or("a waypoint line couldn't be read")?;
                    let pan = pan.and_then(|p| p.trim().parse::<f64>().ok()).filter(|p| *p > 0.0).unwrap_or(DEFAULT_PAN);
                    wps.push((auto.map(str::trim) == Some("1"), pan, place));
                }
                Some((key, value)) => {
                    kv.insert(key.trim().to_string(), value.trim().to_string());
                }
                None => {}
            }
        }
        if wps.len() < 2 {
            return Err("a path needs at least two waypoints".into());
        }
        let num = |k: &str| kv.get(k).and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite());
        let mut flower: Vec<bool> = kv.get("flower").map_or(Vec::new(), |v| v.split(',').map(|f| f.trim() == "1").collect());
        flower.resize(wps.len() - 1, false);
        let first = &wps[0].2;
        let stops = wps.iter().enumerate().map(|(i, w)| (format!("waypoint {}", i + 1), w.2.view)).collect();
        let mut p = VideoProject::new(name, stops, first.order, first.coefs, num("target").map_or(4.0, |t| t.clamp(0.01, 1000.0)), first.style, folder);
        let f = &mut p.frames;
        f.path.rate = num("rate").map_or(1.01, |r| r.clamp(1.001, 4.0));
        f.path.pan = wps[..wps.len() - 1].iter().map(|w| w.1).collect();
        f.path.flower = flower;
        f.maxes = wps.iter().map(|w| w.2.coefs.max).collect();
        f.fade = num("fade").is_none_or(|v| v != 0.0);
        f.reuse = num("reuse").is_none_or(|v| v != 0.0);
        f.step = num("step").map_or(1, |v| (v.max(0.0) as usize).min(COEF_STEPS.len() - 1));
        p.auto = wps.iter().map(|w| w.0).collect();
        p.colouring.steady = num("hold").is_some_and(|v| v != 0.0);
        Ok(p)
    }
}

fn one_line(s: &str) -> String {
    s.chars().map(|ch| if ch.is_control() { ' ' } else { ch }).collect()
}

/// Saved projects live in a "projects" folder in the settings folder.
pub fn projects_dir() -> PathBuf {
    settings_dir().join("projects")
}

pub fn project_file(name: &str) -> PathBuf {
    projects_dir().join(format!("{}.{EXTENSION}", file_safe(name)))
}

/// The old video maker's saved paths.
pub fn old_paths_dir() -> PathBuf {
    settings_dir().join("paths")
}

/// The names (file stems) of the files in `dir` with extension `ext`, alphabetically.
fn names_in(dir: PathBuf, ext: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == ext))
                .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .collect()
        })
        .unwrap_or_default();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// The saved projects, alphabetically, with the last session's first.
pub fn saved_projects() -> Vec<String> {
    let mut names = names_in(projects_dir(), EXTENSION);
    if let Some(i) = names.iter().position(|n| n == LAST_SESSION) {
        let last = names.remove(i);
        names.insert(0, last);
    }
    names
}

/// The old video maker's saved paths, alphabetically.
pub fn old_paths() -> Vec<String> {
    names_in(old_paths_dir(), "path")
}

pub fn save_project(p: &VideoProject, as_name: &str) -> Result<PathBuf, String> {
    let file = project_file(as_name);
    let named = VideoProject { name: as_name.to_string(), ..p.clone() };
    std::fs::create_dir_all(projects_dir())
        .and_then(|_| std::fs::write(&file, named.to_text()))
        .map_err(|e| format!("could not save {}: {e}", file.display()))?;
    Ok(file)
}

/// Opens a saved project by name, or a project file by its path.
pub fn open_project(name_or_file: &str, folder: &str) -> Result<VideoProject, String> {
    let file = if name_or_file.ends_with(&format!(".{EXTENSION}")) { PathBuf::from(name_or_file) } else { project_file(name_or_file) };
    let text = std::fs::read_to_string(&file).map_err(|e| format!("could not open {}: {e}", file.display()))?;
    VideoProject::from_text(&text, folder)
}

pub fn open_old_path(name: &str, folder: &str) -> Result<VideoProject, String> {
    let file = old_paths_dir().join(format!("{name}.path"));
    let text = std::fs::read_to_string(&file).map_err(|e| format!("could not open {}: {e}", file.display()))?;
    VideoProject::from_old_path(&text, name, folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> VideoProject {
        let stops = vec![
            ("start".to_string(), View::HOME),
            ("arm".to_string(), View { cr: 0.1, ci: -0.3, hw: 1e-5 }),
            ("finish".to_string(), View { cr: 1.0 / 3.0, ci: 2f64.sqrt() / 3.0, hw: 0.002 }),
        ];
        let coefs = Coefs { set: CoefSet::Primes, max: 6, zero: true, zero_root: false };
        let mut p = VideoProject::new("dive", stops, 4, coefs, 3.5, Style::default(), "C:\\videos");
        p.frames.path.rate = 1.015;
        p.frames.path.pan = vec![0.02, 0.005];
        p.frames.path.flower = vec![false, true];
        p.frames.maxes = vec![6, 40, 155];
        p.auto = vec![true, false, true];
        (p.frames.fade, p.frames.step, p.frames.height, p.frames.shape, p.frames.aa) = (false, 2, 1080, Shape::Square, 2);
        p.colouring = Colouring {
            style: Style { by: ColourBy::Height, cmap: Colormap::Magma, n: 2.5, gamma: 1.7, low: 20.0, high: 99.0, log_poly: true, smooth: false, ..Style::default() },
            steady: true,
            lh: LhScale::Fixed(3.0, 1840.0),
        };
        p.output = Output { format: Format::Both, fps: 60, quality: 3, folder: "D:\\out".into(), file: "deep dive".into() };
        p
    }

    /// A project reads back exactly, with every setting changed from its default.
    #[test]
    fn project_file_round_trip() {
        let p = sample();
        let text = p.to_text();
        assert_eq!(VideoProject::from_text(&text, "x").unwrap(), p, "{text}");
        assert!(VideoProject::from_text("rate\t1.02\n", "x").is_err());
        // missing settings take their defaults; unknown ones are ignored
        let bare = "wp\ta\t0\t0\t2\t6\t1\t0.01\t0\nwp\tb\t0.3\t0.4\t0.01\t40\t1\t0.01\t0\nsomething\tnew\n";
        let q = VideoProject::from_text(bare, "C:\\videos").unwrap();
        assert_eq!((q.frames.order, q.frames.height, q.output.fps, q.output.folder.as_str()), (3, 720, 30, "C:\\videos"));
        assert_eq!(q.colouring, Colouring::default());
    }

    /// The old video maker's paths come in with their settings and the start's colouring.
    #[test]
    fn old_paths_come_in() {
        let place = |name: &str, v: View, max: i32, by: &str| {
            format!("{name}\t{}\t{}\t{}\t4\tprimes\t1\t{max}\t0\t{by}\t2\t99\tinferno\t0\t1\t1", v.cr, v.ci, v.hw)
        };
        let (a, b) = (View::HOME, View { cr: 0.3, ci: 0.4, hw: 1e-4 });
        let text = format!(
            "# old\nrate\t1.02\ntarget\t6\nfade\t0\nreuse\t1\nstep\t2\nhold\t1\nwork\t20000\nflower\t1\nwp\t0\t0.03\t{}\nwp\t1\t0.01\t{}\n",
            place("waypoint 1", a, 8, "log"),
            place("waypoint 2", b, 90, "pow")
        );
        let p = VideoProject::from_old_path(&text, "old one", "C:\\v").unwrap();
        assert_eq!(p.frames.path.stops, vec![a, b]);
        assert_eq!((p.frames.path.rate, p.frames.path.pan.clone(), p.frames.path.flower.clone()), (1.02, vec![0.03], vec![true]));
        assert_eq!((p.frames.maxes.clone(), p.auto.clone(), p.frames.order, p.frames.coefs.set), (vec![8, 90], vec![false, true], 4, CoefSet::Primes));
        assert_eq!((p.target, p.frames.fade, p.frames.step, p.colouring.steady), (6.0, false, 2, true));
        assert_eq!((p.colouring.style.by, p.colouring.style.cmap), (ColourBy::Log, Colormap::Inferno), "the start's colouring");
        assert_eq!((p.name.as_str(), p.output.file.as_str()), ("old one", "old one"));
    }

    /// A .path file written by the old video maker opens.
    #[test]
    fn old_path_file() {
        let p = VideoProject::from_old_path(include_str!("../../tests/data/old.path"), "old", "C:\\v").unwrap();
        assert_eq!((p.len(), p.frames.maxes.clone(), p.colouring.style.cmap), (2, vec![6, 120], Colormap::Magma));
        assert_eq!(p.frames.path.stops[1].hw, 0.002);
    }

    /// Waypoint edits keep the parallel lists in step.
    #[test]
    fn waypoint_edits() {
        let mut p = sample();
        let frames = p.frames.count();
        // halfway along the first segment: the video doesn't change
        let i = p.insert_halfway(0);
        assert_eq!((i, p.len(), p.frames.path.pan.len(), p.frames.path.flower.len()), (1, 4, 3, 3));
        assert!(p.frames.count().abs_diff(frames) <= 2, "{} vs {frames}", p.frames.count());
        let v = View { cr: 0.5, ci: 0.5, hw: 0.1 };
        let j = p.add_after(3, "new finish", v);
        assert_eq!((j, p.len(), p.label(4), p.frames.path.pan.len()), (4, 5, "Finish".to_string(), 4));
        assert_eq!(p.frames.path.stops[4], v);
        p.delete(1);
        assert_eq!((p.len(), p.names.len(), p.auto.len(), p.frames.maxes.len(), p.frames.path.pan.len()), (4, 4, 4, 4, 3));
        assert_eq!(p.names, ["start", "arm", "finish", "new finish"]);
        p.swap(1);
        assert_eq!(p.names, ["start", "finish", "arm", "new finish"]);
        p.reverse();
        assert_eq!((p.frames.path.stops[0], p.names[0].as_str()), (v, "new finish"));
        p.replace_waypoints(vec![("a".into(), View::HOME), ("b".into(), v)]);
        assert_eq!((p.len(), p.auto.clone(), p.frames.path.pan.len()), (2, vec![true, true], 1));
        p.delete(0);
        assert_eq!(p.len(), 2, "never fewer than two");
    }
}
