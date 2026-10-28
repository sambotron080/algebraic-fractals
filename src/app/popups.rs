//! The explorer's popups: View, Polynomials, Colours, Save location, Locations and
//! Folders, and the questions asked before deleting things.

use super::explore::{Action, Explorer, Popup, ToVideo};
use super::widgets::{coef_set_rows, colour_controls, folder_box, ORANGE};
use super::Shared;
use algfrac::common::*;
use algfrac::engine::{Plan, Region};
use eframe::egui::{self, Color32};

/// The Locations popup's state: what's being renamed or asked about.
#[derive(Default)]
pub struct LocationsUi {
    /// (group, location, the name being typed)
    renaming: Option<(usize, usize, String)>,
    /// (group, the name being typed)
    renaming_group: Option<(usize, String)>,
    /// a location (group, Some(location)) or a whole group (group, None) to delete
    confirm_delete: Option<(usize, Option<usize>)>,
    /// the name of a new group being typed
    new_group: Option<String>,
    /// the groups shown closed (Old places, until it's opened)
    closed_groups: Option<Vec<String>>,
    pub confirm_reset: bool,
    /// give the Save location popup's name box the focus (when it opens)
    pub focus_name: bool,
    pub confirm_project_delete: Option<String>,
    /// where the image was right-clicked
    pub right_click: Option<(f64, f64)>,
}

/// A location being dragged: (group, location).
#[derive(Clone, Copy)]
struct Dragged(usize, usize);

impl Explorer {
    pub(super) fn popups(&mut self, ctx: &egui::Context, shared: &mut Shared, action: &mut Option<Action>, to_video: &mut Option<ToVideo>) {
        let top = ctx.available_rect().min + egui::vec2(8.0, 8.0);
        for p in self.open.clone() {
            let mut open = true;
            let title = match p {
                Popup::View => "View",
                Popup::Polynomials => "Polynomials",
                Popup::Colours => "Colours",
                Popup::SaveLocation => "Save location",
                Popup::Locations => "Locations",
                Popup::Folders => "Folders",
            };
            // each its own place at first, so they don't cover each other
            let right = ctx.screen_rect().right();
            let pos = match p {
                Popup::View | Popup::SaveLocation => top,
                Popup::Polynomials => top + egui::vec2(0.0, 230.0),
                Popup::Folders => top + egui::vec2(0.0, 420.0),
                Popup::Locations => egui::pos2(top.x + 420.0, top.y),
                Popup::Colours => egui::pos2(right - 350.0, top.y),
            };
            let mut window = egui::Window::new(title).open(&mut open).default_pos(pos).resizable(p == Popup::Locations).collapsible(false);
            if p == Popup::Locations {
                window = window.default_height(500.0);
            }
            window.show(ctx, |ui| match p {
                Popup::View => self.view_popup(ui, action),
                Popup::Polynomials => self.polynomials_popup(ui, shared),
                Popup::Colours => colour_controls(ui, &mut self.style, &mut self.ramp, "explore"),
                Popup::SaveLocation => self.save_location_popup(ui, shared, action),
                Popup::Locations => self.locations_popup(ui, shared, action, to_video),
                Popup::Folders => {
                    egui::Grid::new("folders").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label("Images");
                        folder_box(ui, &mut shared.images_dir, "images");
                        ui.end_row();
                        ui.label("Videos");
                        folder_box(ui, &mut shared.videos_dir, "videos");
                        ui.end_row();
                    });
                    ui.weak("New video projects write to the videos folder; each project keeps its own.");
                }
            });
            if !open {
                self.open.retain(|&q| q != p);
            }
        }
        self.questions(ctx, shared);
    }

    fn view_popup(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let view = self.current_view();
        if self.goto_from != Some(view) {
            // the view changed: show it in the boxes
            self.goto_centre = complex_text(view.cr, view.ci);
            self.goto_hw = format!("{}", view.hw);
            self.goto_from = Some(view);
        }
        let mut enter = false;
        egui::Grid::new("goto").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Centre");
            let r = ui
                .add_sized([300.0, 20.0], egui::TextEdit::singleline(&mut self.goto_centre))
                .on_hover_text("Type a complex number, e.g. 0.5+0.8i, -1.2-3e-4i, 2i or 0.5, 0.8, then press Enter or Go");
            enter |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.end_row();
            ui.label("Half-width");
            let r = ui.add_sized([300.0, 20.0], egui::TextEdit::singleline(&mut self.goto_hw)).on_hover_text("Distance from the centre to the edge of the image");
            enter |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.end_row();
        });
        let parsed = parse_complex(&self.goto_centre).zip(self.goto_hw.trim().parse::<f64>().ok().filter(|h| h.is_finite() && *h > 0.0));
        ui.horizontal(|ui| {
            if ui.add_enabled(parsed.is_some(), egui::Button::new("Go")).on_hover_text("Render the centre and half-width typed above").clicked() || (enter && parsed.is_some()) {
                *action = Some(Action::GoTo);
            }
            if ui.button("Copy").on_hover_text("Copy the centre and half-width as text").clicked() {
                ui.ctx().copy_text(format!("{}, {}", complex_text(view.cr, view.ci), view.hw));
            }
            if parsed.is_none() {
                ui.colored_label(Color32::LIGHT_RED, "can't read that number");
            } else if self.goto_centre != complex_text(view.cr, view.ci) || self.goto_hw != format!("{}", view.hw) {
                ui.weak("(edited, press Go)");
            }
        });
        ui.separator();
        ui.add(egui::Slider::new(&mut self.zoom, 1.0..=1000.0).logarithmic(true).text("zoom x"))
            .on_hover_text("How far Run (Enter, or a double-click) zooms in. The mouse wheel changes it too.");
        let (tr, ti) = self.target_point();
        ui.label(format!("Next: {}", fmt_complex(tr, ti)));
        ui.label(format!("      half-width {:.4e}", view.hw / self.zoom));
        ui.checkbox(&mut self.show_box, "Show zoom box");
        if view.hw < 1e-12 * view.cr.abs().max(view.ci.abs()) {
            ui.colored_label(ORANGE, "That's beyond 64-bit precision: the image will look blocky");
        }
    }

    fn polynomials_popup(&mut self, ui: &mut egui::Ui, shared: &mut Shared) {
        egui::Grid::new("search").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            coef_set_rows(ui, &mut self.search.coefs, "explore");
        });
        ui.weak(format!("Allowed: {}", allowed_text(&self.search.coefs)));
        match Plan::new(self.search.order, &self.search.coefs, false) {
            Ok(p) => ui.weak(format!("{:.2e} polynomials per pixel, searched in ~{:.2e} steps", p.brute_len(), p.steps())),
            Err(e) => ui.colored_label(Color32::LIGHT_RED, e),
        };
        let view = self.requested;
        let region = Region::square(view.cr, view.ci, view.hw, self.search.width);
        let dens = shared.predictor.per_pixel(self.search.order, &self.search.coefs, &region);
        ui.weak(format!("Predicted: ~{} algebraic numbers per 100 px² in this view", sig(dens * 100.0))).on_hover_text(
            "Estimated before rendering, from the number of allowed polynomials and where\n\
             the roots of random polynomials lie. Usually within about 20%.",
        );
        if !self.auto_note.is_empty() {
            ui.colored_label(ORANGE, format!("Auto: {}", self.auto_note));
        }
        ui.weak("Changes here, and to the order and max coefficient, re-render the view by themselves.");
    }

    fn save_location_popup(&mut self, ui: &mut egui::Ui, shared: &mut Shared, action: &mut Option<Action>) {
        let Some(view) = self.shown.as_ref().map(|s| s.view) else {
            ui.weak("Nothing to save until the image is shown");
            return;
        };
        if !shared.groups.iter().any(|g| g.name == self.save_group) {
            self.save_group = UNSORTED.into();
        }
        let mut enter = false;
        egui::Grid::new("save loc").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Name");
            let r = ui.add_sized([230.0, 20.0], egui::TextEdit::singleline(&mut self.save_name).hint_text("name this view"));
            if std::mem::take(&mut self.loc.focus_name) {
                r.request_focus();
            }
            enter |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.end_row();
            ui.label("Group");
            match &mut self.new_group {
                Some(name) => {
                    let mut cancel = false;
                    ui.horizontal(|ui| {
                        let r = ui.add_sized([190.0, 20.0], egui::TextEdit::singleline(name).hint_text("the new group's name"));
                        enter |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        cancel = ui.small_button("✖").on_hover_text("Choose an existing group instead").clicked();
                    });
                    if cancel {
                        self.new_group = None;
                    }
                }
                None => {
                    egui::ComboBox::from_id_salt("save group").selected_text(&self.save_group).width(220.0).show_ui(ui, |ui| {
                        for g in &shared.groups {
                            ui.selectable_value(&mut self.save_group, g.name.clone(), format!("{} ({})", g.name, g.locations.len()));
                        }
                        if ui.selectable_label(false, "New group...").clicked() {
                            self.new_group = Some(String::new());
                        }
                    });
                }
            }
            ui.end_row();
        });
        ui.weak(format!("centre {}, half-width {:.4e}", fmt_complex(view.cr, view.ci), view.hw));
        let group = self.new_group.clone().unwrap_or_else(|| self.save_group.clone());
        let name = match self.save_name.trim() {
            "" => fmt_complex(view.cr, view.ci),
            n => n.to_string(),
        };
        let replaces = shared.groups.iter().any(|g| g.name == group.trim() && g.locations.iter().any(|l| l.name == name));
        let save = ui
            .button(if replaces { "Replace" } else { "Save" })
            .on_hover_text("Remember this view's centre and half-width. Going to it later keeps\nthe search settings and colours you have then.");
        if save.clicked() || enter {
            *action = Some(Action::SaveLocation);
        }
    }

    fn locations_popup(&mut self, ui: &mut egui::Ui, shared: &mut Shared, action: &mut Option<Action>, to_video: &mut Option<ToVideo>) {
        if shared.groups.iter().all(|g| g.locations.is_empty()) && shared.groups.len() <= 1 {
            ui.weak("No saved locations yet. Find a view you like and press Save location.");
        }
        let here = self.current_view();
        let mut changed = false;
        let mut moved: Option<(usize, usize, usize, Option<usize>)> = None; // from (g, l) to (g, before l)
        egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
            let n_groups = shared.groups.len();
            let closed = self.loc.closed_groups.get_or_insert_with(|| vec![OLD_PLACES_GROUP.to_string()]);
            let open_now: Vec<bool> = shared.groups.iter().map(|g| !closed.contains(&g.name)).collect();
            for gi in 0..n_groups {
                let gname = shared.groups[gi].name.clone();
                let count = shared.groups[gi].locations.len();
                let is_open = open_now[gi];
                // the header: open/close, name, Make video, and the group's menu
                let header = ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(if is_open { "⏷" } else { "⏵" }).frame(false)).clicked() {
                        let closed = self.loc.closed_groups.get_or_insert_with(Vec::new);
                        if is_open {
                            closed.push(gname.clone());
                        } else {
                            closed.retain(|g| *g != gname);
                        }
                    }
                    match &mut self.loc.renaming_group {
                        Some((i, text)) if *i == gi => {
                            let r = ui.text_edit_singleline(text);
                            r.request_focus();
                            if r.lost_focus() {
                                let new = text.trim().to_string();
                                let ok = !new.is_empty() && !shared.groups.iter().any(|g| g.name == new) && ui.input(|i| !i.key_pressed(egui::Key::Escape));
                                if ok {
                                    shared.groups[gi].name = new.clone();
                                    if self.save_group == gname {
                                        self.save_group = new;
                                    }
                                    changed = true;
                                }
                                self.loc.renaming_group = None;
                            }
                        }
                        _ => {
                            ui.strong(format!("{gname} ({count})"));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button("⚙", |ui| {
                            if ui.button("Rename").clicked() {
                                self.loc.renaming_group = Some((gi, gname.clone()));
                                ui.close_menu();
                            }
                            if ui.button("Sort by depth").on_hover_text("Widest first, so a video zooms in").clicked() {
                                shared.groups[gi].locations.sort_by(|a, b| b.view.hw.total_cmp(&a.view.hw));
                                changed = true;
                                ui.close_menu();
                            }
                            if ui.button("Reverse order").clicked() {
                                shared.groups[gi].locations.reverse();
                                changed = true;
                                ui.close_menu();
                            }
                            let unsorted = gname == UNSORTED;
                            let tip = if unsorted { "Unsorted can't be deleted: this deletes its locations" } else { "Delete the group and its locations" };
                            if ui.button(if unsorted { "Empty the group..." } else { "Delete group..." }).on_hover_text(tip).clicked() {
                                self.loc.confirm_delete = Some((gi, None));
                                ui.close_menu();
                            }
                        });
                        let mk = ui
                            .add_enabled(count >= 2, egui::Button::new("🎬 Make video"))
                            .on_hover_text("Open the video window with these locations as the start, waypoints and finish")
                            .on_disabled_hover_text("A video needs at least 2 locations: a start and a finish");
                        if mk.clicked() {
                            *to_video = Some(ToVideo::FromGroup(gi));
                        }
                    });
                });
                // dropping a location on a header moves it to the end of that group
                if let Some(d) = header.response.dnd_release_payload::<Dragged>() {
                    moved = Some((d.0, d.1, gi, None));
                }
                if !is_open {
                    continue;
                }
                for li in 0..count {
                    let loc = shared.groups[gi].locations[li].clone();
                    let row = ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.dnd_drag_source(egui::Id::new(("loc", gi, li)), Dragged(gi, li), |ui| {
                            ui.weak("☰");
                        })
                        .response
                        .on_hover_text("Drag to move it");
                        ui.weak(format!("{:>2}", li + 1));
                        match &mut self.loc.renaming {
                            Some((g, l, text)) if (*g, *l) == (gi, li) => {
                                let r = ui.text_edit_singleline(text);
                                r.request_focus();
                                if r.lost_focus() {
                                    if ui.input(|i| !i.key_pressed(egui::Key::Escape)) && !text.trim().is_empty() {
                                        shared.groups[gi].locations[li].name = text.trim().to_string();
                                        changed = true;
                                    }
                                    self.loc.renaming = None;
                                }
                            }
                            _ => {
                                let current = loc.view == here;
                                let mut tip = view_info(&loc.view);
                                if li > 0 {
                                    let prev = &shared.groups[gi].locations[li - 1];
                                    let f = prev.view.hw / loc.view.hw;
                                    tip += &if f >= 1.0 { format!("\nx {} deeper than '{}'", sig(f), prev.name) } else { format!("\nx {} wider than '{}'", sig(1.0 / f), prev.name) };
                                }
                                let b = ui.add(egui::Button::new(&loc.name).selected(current).truncate()).on_hover_text(tip);
                                if b.clicked() {
                                    *action = Some(Action::Go(loc.view));
                                }
                                b.context_menu(|ui| {
                                    if ui.button("Set to this view").on_hover_text("Replace its centre and half-width with the view you're looking at").clicked() {
                                        shared.groups[gi].locations[li].view = here;
                                        changed = true;
                                        ui.close_menu();
                                    }
                                });
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✖").on_hover_text("Delete").clicked() {
                                self.loc.confirm_delete = Some((gi, Some(li)));
                            }
                            ui.menu_button("Move", |ui| {
                                ui.weak("Move to the group");
                                for (gj, g) in shared.groups.iter().enumerate().filter(|(gj, _)| *gj != gi) {
                                    if ui.button(&g.name).clicked() {
                                        moved = Some((gi, li, gj, None));
                                        ui.close_menu();
                                    }
                                }
                                if ui.button("New group...").clicked() {
                                    self.loc.new_group = Some(String::new());
                                    ui.close_menu();
                                }
                            })
                            .response
                            .on_hover_text("Move to another group");
                            if ui.small_button("✏").on_hover_text("Rename").clicked() {
                                self.loc.renaming = Some((gi, li, loc.name.clone()));
                            }
                            if ui.add_enabled(li + 1 < count, egui::Button::new("⏷").small()).on_hover_text("Move down").clicked() {
                                shared.groups[gi].locations.swap(li, li + 1);
                                changed = true;
                            }
                            if ui.add_enabled(li > 0, egui::Button::new("⏶").small()).on_hover_text("Move up").clicked() {
                                shared.groups[gi].locations.swap(li, li - 1);
                                changed = true;
                            }
                        });
                    });
                    // dropping a location on a row puts it before that row
                    if let Some(d) = row.response.dnd_release_payload::<Dragged>() {
                        moved = Some((d.0, d.1, gi, Some(li)));
                    }
                }
                ui.add_space(4.0);
            }
            ui.separator();
            match &mut self.loc.new_group {
                Some(name) => {
                    // (the new group's name, once typed; empty if cancelled)
                    let mut made = None;
                    ui.horizontal(|ui| {
                        let r = ui.add(egui::TextEdit::singleline(name).hint_text("the new group's name"));
                        r.request_focus();
                        if r.lost_focus() {
                            let n = name.trim().to_string();
                            let ok = !n.is_empty() && ui.input(|i| !i.key_pressed(egui::Key::Escape)) && !shared.groups.iter().any(|g| g.name == n);
                            made = Some(if ok { n } else { String::new() });
                        }
                    });
                    if let Some(n) = made {
                        if !n.is_empty() {
                            let at = shared.groups.iter().position(|g| g.name == OLD_PLACES_GROUP).unwrap_or(shared.groups.len());
                            shared.groups.insert(at, Group { name: n, locations: Vec::new() });
                            changed = true;
                        }
                        self.loc.new_group = None;
                    }
                }
                None => {
                    if ui.button("➕ New group").clicked() {
                        self.loc.new_group = Some(String::new());
                    }
                }
            }
        });
        if let Some((g0, l0, g1, before)) = moved {
            if g0 < shared.groups.len() && l0 < shared.groups[g0].locations.len() {
                let loc = shared.groups[g0].locations.remove(l0);
                let mut at = before.unwrap_or(shared.groups[g1].locations.len());
                if g0 == g1 && before.is_some_and(|b| b > l0) {
                    at -= 1; // the row it's dropped on moved up by one
                }
                let at = at.min(shared.groups[g1].locations.len());
                shared.groups[g1].locations.insert(at, loc);
                changed = true;
            }
        }
        if changed {
            if let Err(e) = shared.store_groups() {
                self.status = format!("Could not save the locations: {e}");
            }
        }
    }

    /// The questions asked before deleting or resetting.
    fn questions(&mut self, ctx: &egui::Context, shared: &mut Shared) {
        let ask = |ctx: &egui::Context, title: &str, text: &str, yes: &str| -> Option<bool> {
            let mut answer = None;
            egui::Window::new(title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.label(text);
                ui.horizontal(|ui| {
                    if ui.button(yes).clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Keep").clicked() {
                        answer = Some(false);
                    }
                });
            });
            answer
        };
        if let Some((gi, li)) = self.loc.confirm_delete {
            let Some(g) = shared.groups.get(gi) else {
                self.loc.confirm_delete = None;
                return;
            };
            let text = match li {
                Some(li) => format!("Delete '{}' from {}?", g.locations.get(li).map_or("", |l| l.name.as_str()), g.name),
                None if g.name == UNSORTED => format!("Delete the {} locations in {}?", g.locations.len(), g.name),
                None => format!("Delete the group {} and its {} locations?", g.name, g.locations.len()),
            };
            match ask(ctx, "Delete", &text, "Delete") {
                Some(true) => {
                    match li {
                        Some(li) if li < shared.groups[gi].locations.len() => {
                            shared.groups[gi].locations.remove(li);
                        }
                        Some(_) => {}
                        None if shared.groups[gi].name == UNSORTED => shared.groups[gi].locations.clear(),
                        None => {
                            shared.groups.remove(gi);
                        }
                    }
                    if let Err(e) = shared.store_groups() {
                        self.status = format!("Could not save the locations: {e}");
                    }
                    self.loc.confirm_delete = None;
                }
                Some(false) => self.loc.confirm_delete = None,
                None => {}
            }
        }
        if let Some(name) = self.loc.confirm_project_delete.clone() {
            match ask(ctx, "Delete project", &format!("Delete the video project '{name}'?"), "Delete") {
                Some(true) => {
                    let file = algfrac::video::project::project_file(&name);
                    self.status = match std::fs::remove_file(&file) {
                        Ok(()) => format!("Deleted the project '{name}'"),
                        Err(e) => format!("Could not delete {}: {e}", file.display()),
                    };
                    self.loc.confirm_project_delete = None;
                }
                Some(false) => self.loc.confirm_project_delete = None,
                None => {}
            }
        }
        if self.loc.confirm_reset {
            match ask(ctx, "Reset all settings", "Put every setting back to its default? Your locations and video projects stay.", "Reset") {
                Some(true) => {
                    shared.reset = true;
                    self.loc.confirm_reset = false;
                }
                Some(false) => self.loc.confirm_reset = false,
                None => {}
            }
        }
    }

}
