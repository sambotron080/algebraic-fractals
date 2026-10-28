//! The app's windows: the explorer (the main window, with its popups) and the video window.

pub mod explore;
pub mod popups;
pub mod video;
pub mod widgets;

use algfrac::common::{Group, View};
use algfrac::density::Predictor;
use algfrac::memory::RenderMemory;
use algfrac::video::project::VideoProject;
use eframe::egui;
use explore::Explorer;
use std::time::Duration;

/// Where the program lives on GitHub (the ☰ menu's User guide).
pub const REPO: &str = "https://github.com/sambotron080/algebraic-fractals";

/// What the explorer and the video window share.
pub struct Shared {
    /// the root viewport's context, for background threads to ask for a repaint
    pub ctx: egui::Context,
    pub groups: Vec<Group>,
    pub memory: RenderMemory,
    pub predictor: Predictor,
    pub threads: usize,
    pub images_dir: String,
    pub videos_dir: String,
    /// bring the main window to the front (after Show in the explorer)
    pub focus_main: bool,
    /// Reset all settings was confirmed
    pub reset: bool,
}

impl Shared {
    pub fn store_groups(&self) -> std::io::Result<()> {
        algfrac::common::store_groups(&self.groups)
    }
}

/// Asks for the windows to be drawn again (from any thread). The video window is drawn
/// inside the main window's update, so it's the main one that repaints.
pub fn repaint_now(ctx: &egui::Context) {
    ctx.request_repaint_of(egui::ViewportId::ROOT);
}

pub fn repaint_after(ctx: &egui::Context, ms: u64) {
    ctx.request_repaint_after_for(Duration::from_millis(ms), egui::ViewportId::ROOT);
}

/// The flower around (1 + i√2)/3, a root of 3x² - 2x + 1.
pub const FLOWER: View = View { cr: 1.0 / 3.0, ci: 0.4714045207910317, hw: 0.002 };

/// A new video project with the explorer's settings and colours: through the locations
/// of group `group`, or else from the whole picture to the explorer's view (or the
/// flower, if the explorer is showing the whole picture).
pub fn new_project(shared: &Shared, explorer: &Explorer, group: Option<usize>) -> VideoProject {
    let (name, stops) = match group.and_then(|i| shared.groups.get(i)) {
        Some(g) => (g.name.clone(), g.locations.iter().map(|l| (l.name.clone(), l.view)).collect()),
        None => {
            let here = explorer.current_view();
            let finish = if here == View::HOME { FLOWER } else { here };
            ("my zoom".to_string(), vec![("start".to_string(), View::HOME), ("finish".to_string(), finish)])
        }
    };
    let s = &explorer.search;
    VideoProject::new(&name, stops, s.order, s.coefs, explorer.target_density, explorer.style, &shared.videos_dir)
}
