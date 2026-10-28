//! Rendering a whole video: every frame searched, coloured and written to an .mp4 file,
//! a folder of PNG frames, or both. The app and the command line both use `make_video`,
//! which reports each frame through a callback.

use super::frames::{duration_text, Colouring, Frames, WaypointLevels, QUALITY};
use super::project::{Format, VideoProject};
use super::{reuse, Mp4Out};
use crate::common::unused_stem;
use crate::engine::{render_axes, Plan, Render};
use crate::png::write_png_compressed;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

/// What a video is made from.
#[derive(Clone)]
pub struct VideoSettings {
    pub frames: Frames,
    pub colouring: Colouring,
    /// the waypoints' levels already measured, where they are (the rest are rendered
    /// first, if the colouring needs them)
    pub levels: Vec<Option<WaypointLevels>>,
    pub fps: u32,
    pub qp: u8,
    pub threads: usize,
}

impl VideoSettings {
    pub fn of(p: &VideoProject, threads: usize) -> VideoSettings {
        VideoSettings {
            frames: p.frames.clone(),
            colouring: p.colouring,
            levels: Vec::new(),
            fps: p.output.fps,
            qp: QUALITY[p.output.quality].1,
            threads,
        }
    }
}

/// A finished frame, as reported while the video renders.
pub struct FrameDone {
    pub k: usize,
    /// its pixels (packed RGB) and size
    pub rgb: Vec<u8>,
    pub w: usize,
    pub h: usize,
    pub max: f64,
    pub secs: f64,
    /// bytes written so far
    pub bytes: usize,
}

/// Where a video goes: the .mp4 file and/or the folder of PNG frames, named after `file`
/// in `folder`, with _2, _3, ... added so that nothing is overwritten.
pub fn output_paths(folder: &str, file: &str, format: Format) -> (Option<PathBuf>, Option<PathBuf>) {
    let dir = PathBuf::from(folder.trim());
    let name = file.trim().trim_end_matches(".mp4");
    let name = if name.is_empty() { "zoom" } else { name };
    let stem = unused_stem(name, |s| dir.join(format!("{s}.mp4")).exists() || dir.join(format!("{s}_frames")).exists());
    (format.mp4().then(|| dir.join(format!("{stem}.mp4"))), format.png().then(|| dir.join(format!("{stem}_frames"))))
}

/// The bytes of the files in `dir`.
fn folder_bytes(dir: &Path) -> usize {
    std::fs::read_dir(dir).map_or(0, |d| d.filter_map(|e| e.ok()?.metadata().ok()).map(|m| m.len() as usize).sum())
}

/// Renders every frame, writes it into `mp4` and/or as a PNG into `pngs`, and calls
/// `on_frame` with it. `rows` and `rows_total` follow the render in progress. Stopping
/// early (`cancel`) still finishes the file, with the frames done so far. Returns the
/// message to show.
pub fn make_video(
    v: &VideoSettings,
    mp4: Option<&Path>,
    pngs: Option<&Path>,
    cancel: &AtomicBool,
    (rows, rows_total): (&AtomicUsize, &AtomicUsize),
    on_frame: &mut dyn FnMut(FrameDone),
) -> Result<String, String> {
    let f = &v.frames;
    let n = f.count();
    let (w, h) = (f.width(), f.height);
    for dir in mp4.and_then(|p| p.parent()).into_iter().chain(pngs) {
        std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    let mut enc = match mp4 {
        Some(out) => Some(Mp4Out::new(out, w, h, v.fps, v.qp)?),
        None => None,
    };
    let started = Instant::now();
    let ladder = f.ladder();
    let pp = f.reuse.then(|| f.patch_plan(&ladder));
    // with reuse: the rendered patches still needed by this or later frames
    let mut patches: HashMap<usize, Render> = HashMap::new();
    let run = |re: Vec<f64>, im: Vec<f64>, max: i32| -> Result<Option<Render>, String> {
        let plan = Plan::new(f.order, &f.coefs_for(max), false)?;
        rows.store(0, Ordering::Relaxed);
        rows_total.store(im.len(), Ordering::Relaxed);
        Ok(render_axes(&plan, re, im, v.threads, cancel, rows))
    };
    // the waypoints' levels, if the colouring needs them, measuring those not known yet
    let mut levels: Vec<WaypointLevels> = Vec::new();
    if v.colouring.needs_levels() {
        for i in 0..f.path.stops.len() {
            if let Some(Some(l)) = v.levels.get(i) {
                levels.push(l.clone());
                continue;
            }
            let mut l = Vec::new();
            for (max, re, im, weight) in f.levels_key(&ladder, i).2 {
                let Some(r) = run(re, im, max)? else { break };
                l.push((f.stats(&r).levels(), weight));
            }
            levels.push(l);
        }
    }
    let (mut done, mut png_bytes) = (0, 0);
    'frames: for k in 0..n {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let t = Instant::now();
        let mut renders = Vec::new();
        match &pp {
            Some(pp) => {
                let g = f.geom(k);
                for &(id, weight) in &pp.uses[k] {
                    let p = &pp.patches[id];
                    if !patches.contains_key(&id) {
                        let (re, im) = p.axes(p.all());
                        let Some(r) = run(re, im, p.max)? else { break 'frames };
                        patches.insert(id, r);
                    }
                    let r = reuse::crop(&patches[&id], p.window(&g));
                    let st = f.stats(&r);
                    renders.push((r, st, weight));
                }
                patches.retain(|id, _| pp.patches[*id].last > k);
            }
            None => {
                for (max, re, im, weight) in f.jobs(&ladder, None, k) {
                    let Some(r) = run(re, im, max)? else { break 'frames };
                    let st = f.stats(&r);
                    renders.push((r, st, weight));
                }
            }
        }
        let c = &v.colouring;
        let rgb = f.finish(k, &renders, &c.style, &|st| c.scale(&f.path, &levels, k, st));
        if let Some(enc) = &mut enc {
            enc.push(&rgb)?;
        }
        if let Some(dir) = pngs {
            let p = dir.join(format!("frame_{k:05}.png"));
            write_png_compressed(&p, w, h, &rgb).map_err(|e| format!("could not write {}: {e}", p.display()))?;
            png_bytes += std::fs::metadata(&p).map_or(0, |m| m.len() as usize);
        }
        done = k + 1;
        let bytes = enc.as_ref().map_or(0, |e| e.bytes) + png_bytes;
        on_frame(FrameDone { k, rgb, w, h, max: f.max(k), secs: t.elapsed().as_secs_f64(), bytes });
        if cancel.load(Ordering::Relaxed) {
            break;
        }
    }
    let bytes = enc.as_ref().map_or(0, |e| e.bytes) + pngs.map_or(0, folder_bytes);
    if let Some(enc) = enc {
        enc.finish()?;
    }
    let took = duration_text(started.elapsed().as_secs_f64());
    let size = format!("{:.0} MB", bytes as f64 / 1e6);
    let what = mp4.or(pngs).map_or(String::new(), |p| p.display().to_string());
    Ok(if done == 0 {
        if let Some(out) = mp4 {
            let _ = std::fs::remove_file(out);
        }
        if let Some(dir) = pngs {
            let _ = std::fs::remove_dir(dir); // only if empty
        }
        "Stopped before the first frame was finished".into()
    } else if done < n {
        format!("Stopped after {done} of {n} frames ({took}); saved the video so far ({size}): {what}")
    } else {
        format!("Done in {took} ({size}): {what}")
    })
}

#[cfg(test)]
mod tests {
    use super::super::frames::{auto_maxes, AutoInputs, LhScale, Shape};
    use super::super::{encoder_config, ZoomPath};
    use super::*;
    use crate::colour::{ColourBy, Style};
    use crate::common::View;
    use crate::density::Predictor;
    use crate::engine::Coefs;

    /// Decodes an .mp4 file's frames from frame `from` (counting from 1) on, starting at
    /// the keyframe before it, and calls `f` with each one's number, RGB, width and height
    /// until it returns false. Returns the frame size.
    fn decode_mp4(path: &Path, from: u32, mut f: impl FnMut(u32, &[u8], usize, usize) -> bool) -> (usize, usize) {
        let file = std::fs::File::open(path).unwrap();
        let size = file.metadata().unwrap().len();
        let mut r = mp4::Mp4Reader::read_header(std::io::BufReader::new(file), size).unwrap();
        let track = r.tracks().get(&1).unwrap();
        let (n, w, h) = (track.sample_count(), track.width() as usize, track.height() as usize);
        let mut head = Vec::new();
        for ps in [track.sequence_parameter_set().unwrap(), track.picture_parameter_set().unwrap()] {
            head.extend_from_slice(&[0, 0, 0, 1]);
            head.extend_from_slice(ps);
        }
        let start = (1..=from.min(n)).rev().find(|&k| r.read_sample(1, k).unwrap().unwrap().is_sync).unwrap_or(1);
        let mut dec = openh264::decoder::Decoder::new().unwrap();
        let mut rgb = vec![0u8; w * h * 3];
        for k in start..=n {
            let sample = r.read_sample(1, k).unwrap().unwrap();
            let mut annexb = if k == start { head.clone() } else { Vec::new() };
            let mut b = &sample.bytes[..];
            while b.len() >= 4 {
                let len = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize;
                annexb.extend_from_slice(&[0, 0, 0, 1]);
                annexb.extend_from_slice(&b[4..4 + len]);
                b = &b[4 + len..];
            }
            let Some(yuv) = dec.decode(&annexb).unwrap() else { continue };
            yuv.write_rgb8(&mut rgb);
            if k >= from && !f(k, &rgb, w, h) {
                break;
            }
        }
        (w, h)
    }

    fn luma(rgb: &[u8]) -> Vec<f32> {
        rgb.chunks(3).map(|c| 0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32).collect()
    }

    /// The worst 16 x 16 block's mean |luma error| when `frames` (w x h RGB) are encoded
    /// with `config` and decoded again.
    fn worst_block(frames: &[Vec<u8>], w: usize, h: usize, config: openh264::encoder::EncoderConfig, name: &str) -> f32 {
        let out = std::env::temp_dir().join(format!("algfrac_blocks_{name}.mp4"));
        let mut enc = Mp4Out::with_config(&out, w, h, 30, config).unwrap();
        for f in frames {
            enc.push(f).unwrap();
        }
        enc.finish().unwrap();
        let (mut worst, mut k) = (0f32, 0);
        decode_mp4(&out, 1, |_, rgb, _, _| {
            let (a, b) = (luma(&frames[k]), luma(rgb));
            for by in 0..h / 16 {
                for bx in 0..w / 16 {
                    let e: f32 = (by * 16..by * 16 + 16).flat_map(|y| (bx * 16..bx * 16 + 16).map(move |x| y * w + x)).map(|i| (a[i] - b[i]).abs()).sum();
                    worst = worst.max(e / 256.0);
                }
            }
            k += 1;
            k < frames.len()
        });
        let _ = std::fs::remove_file(out);
        assert_eq!(k, frames.len());
        worst
    }

    /// Faint, regular rows of stars zoomed into come through the encoder in their places.
    /// (The encoder's background detection used to leave blocks of them behind.)
    #[test]
    fn faint_rows_of_stars_survive_encoding() {
        let (w, h, n) = (320, 192, 40);
        let frames: Vec<Vec<u8>> = (0..n)
            .map(|k| {
                let s = 1.01f64.powi(k);
                let mut rgb = vec![0u8; w * h * 3];
                for y in 0..h {
                    for x in 0..w {
                        // rows of stars 3 apart, the rows 11 apart, gently curved
                        let (u, v) = ((x as f64 - w as f64 / 2.0) / s, (y as f64 - h as f64 / 2.0) / s);
                        let v = v + 0.002 * u * u;
                        let (du, dv) = (u - (u / 3.0).round() * 3.0, v - (v / 11.0).round() * 11.0);
                        let b = 60.0 * (-(du * du + dv * dv) * s * s / 0.7).exp();
                        rgb[3 * (y * w + x)..3 * (y * w + x) + 3].copy_from_slice(&[b as u8, (b * 0.3) as u8, (b * 0.9) as u8]);
                    }
                }
                rgb
            })
            .collect();
        let now = worst_block(&frames, w, h, encoder_config(30, 20), "now");
        assert!(now < 3.0, "{now}");
    }

    #[test]
    fn short_zoom_video_reusing() {
        short_zoom_video(true);
    }

    #[test]
    fn short_zoom_video_frame_by_frame() {
        short_zoom_video(false);
    }

    /// A short zoom through a waypoint renders with a rising max coefficient (chosen by
    /// auto at the waypoints, fading between whole values), writes the MP4 and the PNG
    /// frames, and its first frame decodes back to (nearly) what was rendered.
    fn short_zoom_video(reuse: bool) {
        let mid = View { cr: 0.3, ci: 0.45, hw: 0.6 };
        let end = View { cr: 1.0 / 3.0, ci: 2f64.sqrt() / 3.0, hw: 0.2 };
        let mut frames = Frames {
            path: ZoomPath { pan: vec![0.2, 0.2], ..ZoomPath::new(vec![View::HOME, mid, end], 1.5) },
            maxes: vec![4; 3],
            order: 3,
            coefs: Coefs { zero_root: true, ..Coefs::ints(4) },
            fade: true,
            reuse,
            step: 1,
            height: 144,
            shape: Shape::Wide,
            aa: 2,
        };
        let inputs = AutoInputs {
            stops: frames.path.stops.clone(),
            auto: vec![true; 3],
            manual: vec![0; 3],
            flower: vec![false; 2],
            order: 3,
            coefs: frames.coefs,
            target: 4.0,
            height: 144,
            shape: Shape::Wide,
        };
        (frames.maxes, _) = auto_maxes(&inputs, &frames.maxes, &mut Predictor::new());
        let m = frames.maxes.clone();
        assert!(m[0] <= m[1] && m[1] <= m[2] && m[0] < m[2], "{m:?}");
        let n = frames.count();
        assert_eq!(n, frames.path.steps(0) + frames.path.steps(1) + 1);
        // steady brightness and the waypoints' length range in one of the two runs
        let colouring = if reuse {
            Colouring { style: Style { by: ColourBy::Length, ..Style::default() }, steady: true, lh: LhScale::Waypoints }
        } else {
            Colouring::default()
        };
        let dir = std::env::temp_dir().join(format!("algfrac_video_e2e_{reuse}"));
        let _ = std::fs::remove_dir_all(&dir);
        let (out, pngs) = (dir.join("z.mp4"), dir.join("frames"));
        let v = VideoSettings { frames, colouring, levels: Vec::new(), fps: 30, qp: 20, threads: 4 };
        let rows = (&AtomicUsize::new(0), &AtomicUsize::new(0));
        let mut rendered: Vec<(Vec<u8>, f64)> = Vec::new();
        let msg = make_video(&v, Some(&out), Some(&pngs), &AtomicBool::new(false), rows, &mut |f| rendered.push((f.rgb, f.max))).unwrap();
        assert!(msg.starts_with("Done"), "{msg}");
        let maxes: Vec<f64> = rendered.iter().map(|f| f.1).collect();
        assert_eq!(maxes.len(), n);
        assert!(maxes.windows(2).all(|w| w[1] >= w[0]), "{maxes:?}");
        assert_eq!((maxes[0], maxes[n - 1]), (m[0] as f64, m[2] as f64));
        assert_eq!(std::fs::read_dir(&pngs).unwrap().count(), n);

        // the first decoded frame is (nearly) what was rendered
        let mut first = None;
        let (w, h) = decode_mp4(&out, 1, |_, rgb, _, _| {
            first = Some(rgb.to_vec());
            false
        });
        assert_eq!((w, h), (256, 144));
        let (a, b) = (luma(&rendered[0].0), luma(&first.unwrap()));
        let mean = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32;
        assert!(mean < 3.0, "{mean}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Stopping before the first frame leaves no file behind.
    #[test]
    fn stopped_before_the_first_frame() {
        let frames = Frames {
            path: ZoomPath::new(vec![View::HOME, View { hw: 0.5, ..View::HOME }], 1.5),
            maxes: vec![4, 4],
            order: 3,
            coefs: Coefs::ints(4),
            fade: false,
            reuse: false,
            step: 1,
            height: 144,
            shape: Shape::Square,
            aa: 1,
        };
        let dir = std::env::temp_dir().join("algfrac_video_stopped");
        let _ = std::fs::remove_dir_all(&dir);
        let (mp4, pngs) = output_paths(&dir.display().to_string(), "z", Format::Both);
        let v = VideoSettings { frames, colouring: Colouring::default(), levels: Vec::new(), fps: 30, qp: 20, threads: 2 };
        let rows = (&AtomicUsize::new(0), &AtomicUsize::new(0));
        let msg = make_video(&v, mp4.as_deref(), pngs.as_deref(), &AtomicBool::new(true), rows, &mut |_| {}).unwrap();
        assert_eq!(msg, "Stopped before the first frame was finished");
        assert!(!mp4.unwrap().exists() && !pngs.unwrap().exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
