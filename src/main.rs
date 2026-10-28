//! Algebraic Fractals: explore algebraic-number fractals, save locations in groups, and
//! make zoom videos of them. The explorer is the main window; the video window opens
//! from its toolbar.

#![windows_subsystem = "windows"] // no console window

mod app;

use algfrac::common::{default_images_dir, default_videos_dir, load_groups, UNSORTED};
use algfrac::density::Predictor;
use algfrac::settings::Settings;
use algfrac::video::project;
use app::explore::{Explorer, ToVideo, VideoInfo};
use app::video::{Leave, VideoWindow};
use app::Shared;
use eframe::egui;

struct App {
    shared: Shared,
    explorer: Explorer,
    video: Option<VideoWindow>,
    video_open: bool,
    /// the question asked when closing while a video renders
    asking_quit: bool,
    /// close as soon as the render has stopped (and saved what's done)
    quit_after_render: bool,
    /// closing without waiting for the render
    quitting: bool,
}

fn video_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("video window")
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> App {
        let settings = Settings::load();
        let shared = Shared {
            ctx: cc.egui_ctx.clone(),
            groups: load_groups(),
            predictor: Predictor::new(),
            threads: std::thread::available_parallelism().map_or(4, |n| n.get()),
            images_dir: settings.str("images_dir").map_or_else(default_images_dir, str::to_string),
            videos_dir: settings.str("videos_dir").map_or_else(default_videos_dir, str::to_string),
            focus_main: false,
            reset: false,
        };
        let explorer = Explorer::new(&settings);
        let mut app = App { shared, explorer, video: None, video_open: false, asking_quit: false, quit_after_render: false, quitting: false };
        // the video window, as it was left
        if let Some(name) = settings.str("video.project") {
            if let Ok(p) = project::open_project(name, &app.shared.videos_dir) {
                let mut v = VideoWindow::new(p, settings.get("video.thumbs").unwrap_or(0));
                v.replace(v.project.clone(), String::new());
                app.video = Some(v);
                app.video_open = settings.flag("video.open").unwrap_or(false);
            }
        }
        app
    }

    /// Opens the video window with `p` (asking about unsaved changes to the one there).
    fn open_video(&mut self, p: algfrac::video::project::VideoProject, status: String) {
        match &mut self.video {
            Some(v) => v.leave(Leave::Replace(Box::new(p), status)),
            None => {
                let mut v = VideoWindow::new(p, 0);
                v.replace(v.project.clone(), status);
                self.video = Some(v);
            }
        }
        self.video_open = true;
        self.shared.ctx.send_viewport_cmd_to(video_id(), egui::ViewportCommand::Focus);
    }

    fn handle(&mut self, t: ToVideo) {
        let s = &self.shared;
        match t {
            ToVideo::Show => {
                if self.video.is_none() {
                    let p = app::new_project(s, &self.explorer, None);
                    self.open_video(p, "A new project".into());
                } else {
                    self.video_open = true;
                    s.ctx.send_viewport_cmd_to(video_id(), egui::ViewportCommand::Focus);
                }
            }
            ToVideo::New => {
                let p = app::new_project(s, &self.explorer, None);
                self.open_video(p, "A new project".into());
            }
            ToVideo::FromGroup(i) => {
                let p = app::new_project(s, &self.explorer, Some(i));
                let msg = format!("A new project from the group {}", p.name);
                self.open_video(p, msg);
            }
            ToVideo::Open(name) => match project::open_project(&name, &s.videos_dir) {
                Ok(p) => self.open_video(p, format!("Opened the project '{name}'")),
                Err(e) => self.explorer.status = e,
            },
            ToVideo::OpenOld(name) => match project::open_old_path(&name, &s.videos_dir) {
                Ok(p) => self.open_video(p, format!("Brought in the old path '{name}': its colouring is the start's")),
                Err(e) => self.explorer.status = e,
            },
            ToVideo::StopRender => {
                if let Some(v) = &mut self.video {
                    v.stop_render();
                }
            }
        }
    }

    /// Writes the remembered settings, and keeps an unsaved video project.
    fn remember(&mut self) {
        let mut s = Settings::default();
        self.explorer.remember(&mut s);
        s.set("images_dir", &self.shared.images_dir);
        s.set("videos_dir", &self.shared.videos_dir);
        s.set("last_group", &self.explorer.save_group);
        if let Some(v) = &mut self.video {
            if let Some(name) = v.keep_last_session() {
                s.set("video.project", name);
            }
            s.set_flag("video.open", self.video_open);
            s.set("video.thumbs", v.thumb_size());
        }
        let _ = s.store();
    }

    /// The question asked when the window is closed while a video renders.
    fn quit_question(&mut self, ctx: &egui::Context) {
        let mut answer = None;
        egui::Window::new("A video is rendering").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label("Stop it and save what's done, or keep rendering?");
            ui.horizontal(|ui| {
                for (a, text) in [(0, "Stop and save"), (1, "Keep rendering"), (2, "Quit without saving")] {
                    if ui.button(text).clicked() {
                        answer = Some(a);
                    }
                }
            });
        });
        match answer {
            Some(0) => {
                if let Some(v) = &mut self.video {
                    v.stop_render();
                }
                self.quit_after_render = true;
                self.asking_quit = false;
            }
            Some(1) => self.asking_quit = false,
            Some(_) => {
                self.quitting = true;
                self.asking_quit = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            None => {}
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // closing the main window while a video renders asks first
        let rendering = self.video.as_ref().is_some_and(|v| v.rendering());
        if ctx.input(|i| i.viewport().close_requested()) && rendering && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.asking_quit = true;
        }
        if self.quit_after_render && !rendering {
            self.quitting = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        if let Some(v) = &mut self.video {
            v.poll(ctx, &mut self.shared, self.video_open);
        }
        let info = VideoInfo::of(self.video_open, self.video.as_ref());
        if let Some(t) = self.explorer.update(ctx, &mut self.shared, &info) {
            self.handle(t);
        }
        if self.shared.reset {
            self.shared.reset = false;
            self.shared.images_dir = default_images_dir();
            self.shared.videos_dir = default_videos_dir();
            self.explorer.save_group = UNSORTED.into();
        }
        if self.asking_quit {
            self.quit_question(ctx);
        }

        if self.video_open {
            if let Some(v) = &mut self.video {
                let title = format!("Video: {}", if v.project.name.is_empty() { "untitled" } else { &v.project.name });
                let builder = egui::ViewportBuilder::default().with_title(title).with_inner_size([1100.0, 750.0]).with_min_inner_size([800.0, 560.0]);
                let (shared, explorer) = (&mut self.shared, &mut self.explorer);
                ctx.show_viewport_immediate(video_id(), builder, |ctx, _class| {
                    v.ui(ctx, shared, explorer);
                    let close = ctx.input(|i| i.viewport().close_requested()) || (ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !ctx.wants_keyboard_input());
                    if close {
                        if !v.rendering() && v.unsaved() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                        }
                        v.leave(Leave::Close);
                    }
                });
                if v.close {
                    v.close = false;
                    self.video_open = false;
                }
            }
        }
        if std::mem::take(&mut self.shared.focus_main) {
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.remember();
    }
}

/// The window's icon: the lotus flower around (−1)^(1/3), rendered at start (a few ms).
fn icon() -> egui::IconData {
    const W: usize = 64;
    let rgba = algfrac::icon::lotus(W).chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect();
    egui::IconData { rgba, width: W as u32, height: W as u32 }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 900.0]).with_title("Algebraic Fractals").with_icon(icon()),
        ..Default::default()
    };
    eframe::run_native("Algebraic Fractals", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
