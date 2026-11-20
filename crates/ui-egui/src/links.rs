//! Linked models: Manage > Link Model asks for a project or an IFC, then how to position it, and links it; Reload Links reads
//! the files again. Manage > Link CAD asks for a drawing to put under the plan on screen, and where the drawing measures
//! on itself and says it is not in the units its file claims, asks at which scale to bring it in before anything comes in
//! at all (Insert > Link CAD, which brings a drawing in as an import, is `cad.rs`).

use std::path::{Path, PathBuf};

use egui::{Align, Context, Id as UiId, Layout, RichText, TextEdit};
use serde_json::{Value, json};

use crate::BimApp;
use crate::lang::{t, tf};
use crate::popup::Modal;
use crate::style::pal;

/// A drawing picked for Link CAD whose own measurements say it is not drawn in the units its file declares: the
/// question of which scale to bring it in at, asked before it is brought in.
pub(crate) struct CadScale {
    /// The path as it will be kept, relative to the project's folder where it lies under it.
    pub(crate) path: String,
    /// The level it comes under.
    pub(crate) level: Option<Value>,
    /// What the file itself says a unit of it is, in millimetres.
    pub(crate) declares: f64,
    /// What its own measurements say that unit is.
    pub(crate) says: f64,
}

/// The ways a link can be positioned, as Revit's Link dialog names them, and what each does.
const POSITIONS: [(&str, &str, &str); 3] = [
    (
        "manual",
        bimcraft_i18n::msg!("Auto - Origin to Origin"),
        bimcraft_i18n::msg!("Its internal origin on this project's: where it was modelled, as it was modelled"),
    ),
    (
        "base",
        bimcraft_i18n::msg!("Auto - Project Base Point to Project Base Point"),
        bimcraft_i18n::msg!("Its Project Base Point on this project's, turned as the two base points are"),
    ),
    (
        "shared",
        bimcraft_i18n::msg!("Auto - By Shared Coordinates"),
        bimcraft_i18n::msg!("Where its shared coordinates put it in the world: the two projects agree on the site"),
    ),
];

/// Link Model's box: the file picked, how it is positioned, and which of its locations shared coordinates use.
pub(crate) struct LinkBox {
    path: PathBuf,
    by: &'static str,
    location: String,
    error: Option<String>,
}

impl LinkBox {
    #[cfg(test)]
    pub(crate) fn for_test(path: PathBuf) -> LinkBox {
        LinkBox { path, by: "manual", location: String::new(), error: None }
    }
}

/// What Link Model offers: a project of any of its names, or an IFC file.
pub(crate) fn link_extensions() -> Vec<&'static str> {
    let mut extensions = bimcraft_core::files::PROJECTS.to_vec();
    extensions.push("ifc");
    extensions
}

impl BimApp {
    /// Link Model (LK): the file the user picks, then its box.
    pub(crate) fn link_model_dialog(&mut self, ctx: &Context) {
        let dialog = rfd::AsyncFileDialog::new().add_filter(t("BIMCraft project or IFC"), &link_extensions());
        self.ask_file(ctx, dialog, false, |app, path| {
            app.link_box = Some(LinkBox { path, by: "manual", location: String::new(), error: None })
        });
    }

    /// Link CAD (LC): the DXF the user picks, under the plan on screen. A window with no plan open says so, since a
    /// drawing comes under one.
    pub(crate) fn link_cad_dialog(&mut self, ctx: &Context) {
        if self.plan_level().is_none() {
            self.status = crate::Status { text: t("Link CAD: open a plan first").to_string(), error: true };
            return;
        }
        // A DWG comes in as a DXF does, where this build has what turns one into the other.
        let kinds: &[&str] = match self.session.dwg.is_some() {
            true => &["dxf", "dwg"],
            false => &["dxf"],
        };
        let dialog = rfd::AsyncFileDialog::new().add_filter(t("CAD drawing"), kinds);
        self.ask_file(ctx, dialog, false, |app, path| {
            let folder = app.session.path.as_deref().and_then(Path::parent);
            let kept = folder.and_then(|dir| path.strip_prefix(dir).ok()).unwrap_or(&path).to_string_lossy().into_owned();
            let level = app.plan_level().map(Value::from);
            // Read before it comes in: a drawing whose own measurements say it is not in the units its file claims
            // would come in a hand's width across, and the question is asked while nothing has changed yet.
            // Reading is no change to the project, and a file that cannot be read says so when it comes in.
            let read = app.session.execute("dxf", &json!({"path": kept})).ok();
            let says = read
                .as_ref()
                .and_then(|r| r["measures"]["scale_says"].as_array()?.first()?.as_f64())
                .filter(|says| says.is_finite() && *says > 0.0);
            match says {
                Some(says) => {
                    let declares = read.as_ref().and_then(|r| r["scale"].as_f64()).unwrap_or(1.0);
                    app.cad_scale = Some(CadScale { path: kept, level, declares, says });
                }
                None => app.link_cad(&kept, level, None),
            }
        });
    }

    /// The drawing at `path` under `level`, brought in at `scale` where one is given, and what came in said.
    pub(crate) fn link_cad(&mut self, path: &str, level: Option<Value>, scale: Option<f64>) {
        let mut p = json!({"path": path, "level": level});
        if let Some(scale) = scale {
            p["scale"] = json!(scale);
        }
        let Some(r) = self.run("underlay", p) else { return };
        let (shapes, name) = (r["shapes"].as_u64().unwrap_or(0).to_string(), r["name"].as_str().unwrap_or_default().to_string());
        let mut text = tf("{} shapes came in from {}.", &[&shapes, &name]);
        if let Some(left) = r["left_out"].as_array().filter(|l| !l.is_empty()) {
            let what: Vec<&str> = left.iter().filter_map(Value::as_str).collect();
            text.push(' ');
            text.push_str(&tf("Left out: {}.", &[&what.join(", ")]));
        }
        self.status = crate::Status { text, error: false };
    }

    /// The question Link CAD asks of a drawing that measures on itself and says it is not in the units its file
    /// claims: at which scale to bring it in. Nothing has come in until it is answered, and either answer brings the
    /// drawing in with one command, so one undo step takes it back out.
    pub(crate) fn cad_scale_window(&mut self, ctx: &Context) {
        let Some(asked) = self.cad_scale.take() else { return };
        let (mut picked, mut close) = (None, false);
        let mut open = true;
        egui::Window::new(t("Link CAD"))
            .id(egui::Id::new("cad-scale"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width(520.0);
                let (declares, says) = (format!("{:.0}", asked.declares), format!("{:.0}", asked.says));
                let said = tf(
                    "This drawing's file says a unit of it is {} mm, and the measurements written on it say it is drawn at a scale of {}.",
                    &[&declares, &says],
                );
                ui.label(said);
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(t("Brought in as the file says, a survey drawn in metres is a hand's width across and nothing in it is as thick as a wall."))
                        .color(pal().muted),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(tf("Bring it in at {}", &[&says])).clicked() {
                        picked = Some(Some(asked.says));
                    }
                    if ui.button(tf("Bring it in at {}, as the file says", &[&declares])).clicked() {
                        picked = Some(None);
                    }
                    if ui.button(t("Cancel")).clicked() {
                        close = true;
                    }
                });
            });
        match picked {
            Some(scale) => self.link_cad(&asked.path, asked.level.clone(), scale),
            None if open && !close => self.cad_scale = Some(asked),
            None => {}
        }
    }

    /// Links the project at `path` origin to origin (what tests hand a file to, as the system's dialog would), kept relative to this project's folder where it lies under it, so the two move together.
    #[cfg(test)]
    pub(crate) fn link_path(&mut self, path: &Path) {
        self.link_path_by(path, "manual", "");
    }

    /// The project at `path` linked, positioned `by` (manual, base or shared), with the link's `location` for shared
    /// coordinates (its active one when blank). Whether it was linked.
    pub(crate) fn link_path_by(&mut self, path: &Path, by: &str, location: &str) -> bool {
        let folder = self.session.path.as_deref().and_then(Path::parent);
        let kept = folder.and_then(|dir| path.strip_prefix(dir).ok()).unwrap_or(path);
        let mut p = json!({"path": kept.to_string_lossy()});
        if by != "manual" {
            p["by"] = json!(by);
        }
        if by == "shared" && !location.trim().is_empty() {
            p["location"] = json!(location.trim());
        }
        self.run("link", p).is_some()
    }

    /// Link Model's box, while it's open: the file, its positioning, OK or Cancel.
    pub(crate) fn link_window(&mut self, ctx: &Context) {
        let Some(mut b) = self.link_box.take() else { return };
        let (mut ok, mut close) = (false, false);
        let r = Modal::new(UiId::new("link-model"), t("Link Model")).width(460.0).show(ctx, |ui| {
            crate::popup::body(ui, |ui| {
                ui.add_space(6.0);
                ui.label(RichText::new(b.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).strong());
                ui.add_space(8.0);
                ui.label(t("Positioning"));
                for (by, label, says) in POSITIONS {
                    ui.radio_value(&mut b.by, by, t(label)).on_hover_text(t(says));
                }
                ui.add_space(4.0);
                ui.add_enabled_ui(b.by == "shared", |ui| {
                    ui.horizontal(|ui| {
                        let l = ui.label(t("Its location"));
                        ui.add(TextEdit::singleline(&mut b.location).hint_text(t("its active one")).desired_width(200.0)).labelled_by(l.id);
                    });
                });
                if let Some(e) = &b.error {
                    ui.add_space(6.0);
                    ui.label(RichText::new(e).color(pal().error));
                }
            });
            crate::popup::footer(ui, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    close = ui.button(t("Cancel")).clicked();
                    ok = ui.button(t("OK")).clicked();
                });
            });
        });
        if ok {
            if self.link_path_by(&b.path, b.by, &b.location) {
                return;
            }
            b.error = Some(self.status.text.clone());
        } else if close || r.should_close() {
            return;
        }
        self.link_box = Some(b);
    }
}
