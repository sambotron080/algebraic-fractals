//! Controls used in several places: the coefficient-set rows, the colour controls with
//! their colormap strip, and small helpers.

use algfrac::colour::{ColourBy, Colormap, Style};
use algfrac::common::MAX_COEF;
use algfrac::engine::{CoefSet, Coefs};
use eframe::egui::{self, vec2, Color32, ColorImage, TextureHandle, TextureOptions};

/// Notes that something fell short (auto's notes, precision warnings).
pub const ORANGE: Color32 = Color32::from_rgb(230, 180, 60);
/// The zoom box, the selected waypoint.
pub const YELLOW: Color32 = Color32::from_rgb(255, 200, 40);

/// The coefficient-set rows of a two-column settings grid: which set, and 0 can be a
/// root. (0 is always an allowed coefficient.) `salt` keeps the ids apart where it
/// appears twice.
pub fn coef_set_rows(ui: &mut egui::Ui, c: &mut Coefs, salt: &str) {
    c.zero = true;
    ui.label("Coefficients");
    egui::ComboBox::from_id_salt(("set", salt)).selected_text(c.set.label()).show_ui(ui, |ui| {
        for s in CoefSet::ALL {
            let eg: Vec<String> = s.upto(1000).iter().take(5).map(|v| v.to_string()).collect();
            ui.selectable_value(&mut c.set, s, s.label()).on_hover_text(format!("0, ±{}, ...", eg.join(", ±")));
        }
    });
    ui.end_row();
    ui.label("");
    ui.checkbox(&mut c.zero_root, "0 can be a root")
        .on_hover_text("Allow c0 = 0, i.e. polynomials x*q(x), so that X = 0 is a root.\nUntick so that c0 is never 0.");
    ui.end_row();
}

/// A max coefficient box: dragging changes large values proportionally faster.
pub fn max_coef_box(ui: &mut egui::Ui, enabled: bool, m: &mut i32) -> egui::Response {
    let speed = (*m as f64 * 0.01).max(0.2);
    ui.add_enabled(enabled, egui::DragValue::new(m).range(1..=MAX_COEF).speed(speed))
}

/// The colour controls, in two collapsible groups (what and which colours; adjust).
/// `ramp` caches the colormap strip's texture; `salt` keeps the ids apart.
pub fn colour_controls(ui: &mut egui::Ui, style: &mut Style, ramp: &mut Option<(Style, TextureHandle)>, salt: &str) {
    let by_poly = matches!(style.by, ColourBy::Length | ColourBy::Height);
    let uses_res = !by_poly || style.shade;
    egui::CollapsingHeader::new("What and which colours").id_salt(("what", salt)).default_open(true).show(ui, |ui| {
        egui::Grid::new(("display", salt)).num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Colour by");
            egui::ComboBox::from_id_salt(("by", salt)).selected_text(style.by.name()).show_ui(ui, |ui| {
                for b in ColourBy::ALL {
                    ui.selectable_value(&mut style.by, b, b.name());
                }
            });
            ui.end_row();
            ui.label("Colormap");
            egui::ComboBox::from_id_salt(("cmap", salt)).selected_text(style.cmap.name()).show_ui(ui, |ui| {
                for c in Colormap::ALL {
                    ui.selectable_value(&mut style.cmap, c, c.name());
                }
            });
            ui.end_row();
        });
        ramp_strip(ui, style, ramp, salt);
        ui.checkbox(&mut style.invert, "Invert colormap");
        ui.add_enabled(by_poly, egui::Checkbox::new(&mut style.shade, "Shade by 1/res^n"))
            .on_hover_text("Darken the length/height colours away from the algebraic numbers");
        ui.add_enabled(by_poly, egui::Checkbox::new(&mut style.log_poly, "Log scale"))
            .on_hover_text("Spread the lengths/heights over the colormap on a log scale,\nso the small values get as much of it as the large ones");
    });
    egui::CollapsingHeader::new("Adjust").id_salt(("adjust", salt)).default_open(true).show(ui, |ui| {
        let s = &mut *style;
        ui.add_enabled(uses_res, egui::Slider::new(&mut s.n, 0.05..=5.0).logarithmic(true).text("n  (1/res^n)"))
            .on_hover_text("Larger n makes the points near algebraic numbers sharper");
        ui.add_enabled(uses_res, egui::Slider::new(&mut s.high, 90.0..=100.0).text("max %"))
            .on_hover_text("Brightness percentile that becomes full brightness: 99.5 saturates the brightest 0.5% of pixels.\nLower = brighter image");
        ui.add_enabled(uses_res, egui::Slider::new(&mut s.low, 0.0..=100.0).text("min %"))
            .on_hover_text("Brightness percentile that becomes black: 50 makes the darker half of the pixels black.\nRaise it to hide the background haze");
        if s.low > s.high - 0.1 {
            s.low = (s.high - 0.1).max(0.0);
        }
        ui.add_enabled(uses_res, egui::Checkbox::new(&mut s.smooth, "Smooth stars")).on_hover_text(
            "Average each pixel's brightness over the whole pixel instead of taking it at its centre.\n\
             Points smaller than a pixel then keep a steady brightness as a video moves them,\n\
             instead of twinkling, and rows of close points don't shimmer",
        );
        ui.add(egui::Slider::new(&mut s.brightness, -0.5..=0.5).text("brightness"));
        ui.add(egui::Slider::new(&mut s.contrast, 0.0..=4.0).text("contrast"));
        ui.add(egui::Slider::new(&mut s.gamma, 0.1..=10.0).logarithmic(true).text("gamma"))
            .on_hover_text("Above 1 brightens the midtones (faint points), below 1 darkens them,\nwithout changing pure black or full brightness");
        ui.add(egui::Slider::new(&mut s.saturation, 0.0..=3.0).text("saturation")).on_hover_text("0 = greyscale, above 1 = more vivid colours");
        ui.add(egui::Slider::new(&mut s.cycles, 1.0..=50.0).logarithmic(true).text("colour cycles"))
            .on_hover_text("Run through the colormap several times, giving bands of colour.\nTry it with hsv, sinebow, neon or acid for psychedelic contours");
        ui.add(egui::Slider::new(&mut s.shift, 0.0..=1.0).text("colour shift")).on_hover_text("Slide the colours along the colormap");
    });
    if ui.button("Reset colours").on_hover_text("Every colour setting back to its default").clicked() {
        *style = Style::default();
    }
}

/// A strip showing the colormap with the current adjustments, from black end to bright end.
fn ramp_strip(ui: &mut egui::Ui, style: &Style, ramp: &mut Option<(Style, TextureHandle)>, salt: &str) {
    if ramp.as_ref().map(|r| r.0) != Some(*style) {
        const N: usize = 256;
        let img = ColorImage::from_rgb([N, 1], &style.ramp(N));
        match ramp {
            Some((s, t)) => {
                t.set(img, TextureOptions::LINEAR);
                *s = *style;
            }
            None => *ramp = Some((*style, ui.ctx().load_texture(format!("ramp {salt}"), img, TextureOptions::LINEAR))),
        }
    }
    if let Some((_, t)) = ramp {
        let w = ui.available_width().min(320.0);
        ui.add(egui::Image::new((t.id(), vec2(w, 14.0)))).on_hover_text("The colours in use: left = darkest pixels, right = brightest");
    }
}

/// An RGB image as a texture, made or updated in place.
pub fn set_texture(ctx: &egui::Context, tex: &mut Option<TextureHandle>, name: &str, w: usize, h: usize, rgb: &[u8], options: TextureOptions) {
    let img = ColorImage::from_rgb([w, h], rgb);
    match tex {
        Some(t) => t.set(img, options),
        None => *tex = Some(ctx.load_texture(name, img, options)),
    }
}

/// Opens a file or folder with the system's default program.
pub fn open_with_system(path: &std::path::Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

/// Shows a file in its folder, selected where the system can.
pub fn show_in_folder(path: &std::path::Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(dir) = path.parent() {
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}

/// A text box for a folder, with a Browse... button that opens a native folder picker.
pub fn folder_box(ui: &mut egui::Ui, folder: &mut String, salt: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= ui.add_sized([300.0, 20.0], egui::TextEdit::singleline(folder).id_salt(("folder", salt))).changed();
        if ui.button("Browse...").clicked() {
            let start = std::path::PathBuf::from(folder.trim());
            let mut dialog = rfd::FileDialog::new();
            if start.is_dir() {
                dialog = dialog.set_directory(&start);
            }
            if let Some(d) = dialog.pick_folder() {
                *folder = d.display().to_string();
                changed = true;
            }
        }
    });
    changed
}
