//! Trace Walls (TW): the drawing under the plan on screen read for its walls, with what was found shown before
//! anything is built. The Layers tab is every layer of the drawing — whether it is drawn, and what it is of the
//! project's history as Link CAD read it from the layers' own names — put right a row at a time (`cad_layers`); the
//! Walls tab is what to look for, Find (`trace_walls` with `preview`, which draws what it found over the plan) and
//! Build (`trace_walls`). Nothing is built until Build is pressed, and a layer that is not drawn is not traced.
//!
//! It floats and is dragged by its title, and it does not take the keys, because what it shows is on the plan behind
//! it: the walls it found are no use under a modal that stands in the middle of them and cannot be moved. So the plan
//! pans, zooms and fits (`ZF`) while it is open, and the only typing it takes is into numbers.

use std::ops::RangeInclusive;
use std::sync::Arc;

use bimcraft_core::geom::P2;
use bimcraft_core::model::{Element, Id, Model, Underlay};
use bimcraft_core::render::View;
use bimcraft_core::trace::How;
use egui::{
    Align, Button, ComboBox, Context, DragValue, Grid, Id as UiId, Layout, Pos2, RichText, ScrollArea, Shape, Stroke, Ui, WidgetInfo,
    WidgetType, pos2, vec2,
};
use serde_json::{Value, json};

use crate::BimApp;
use crate::lang::{category, t, tf};
use crate::properties::combo;
use crate::style::pal;

/// A wall found in the drawing and not yet built: where its axis runs and how thick, in the project's millimetres.
pub(crate) struct Traced {
    pub(crate) start: P2,
    pub(crate) end: P2,
    pub(crate) thickness: f64,
}

/// An opening found in one of those walls: where it runs along the wall's axis, how thick the wall is there, and
/// whether it is a window.
pub(crate) struct Hole {
    pub(crate) from: P2,
    pub(crate) to: P2,
    pub(crate) thickness: f64,
    pub(crate) window: bool,
}

/// The Trace Walls window: which drawing it reads, which tab is open, what it looks for, the wall type every wall
/// comes in as (none is one by thickness), what the last Find found and what the command said of it, and why a
/// command was refused — said inside the window, where it is read beside the button that was pressed and not in the
/// far corner of the view.
pub(crate) struct TraceDialog {
    pub(crate) underlay: Id,
    pub(crate) tab: usize,
    pub(crate) how: How,
    pub(crate) kind: Option<String>,
    pub(crate) found: Vec<Traced>,
    pub(crate) holes: Vec<Hole>,
    pub(crate) report: Option<Value>,
    pub(crate) error: String,
}

impl TraceDialog {
    /// What `trace_walls` is given: what to look for, and whether to build it.
    fn params(&self, preview: bool) -> Value {
        let mut p = json!({
            "underlay": self.underlay,
            "thin": self.how.thin,
            "thick": self.how.thick,
            "shortest": self.how.shortest,
            "skew": self.how.skew,
            "openings": self.how.openings,
        });
        if let Some(kind) = &self.kind {
            p["type"] = json!(kind);
        }
        if preview {
            p["preview"] = json!(true);
        }
        p
    }

    /// What was found was found with the measurements, the type and the layers as they were: it is no longer what
    /// the drawing would give.
    fn stale(&mut self) {
        self.found.clear();
        self.holes.clear();
        self.report = None;
    }
}

/// The drawing `id` is, if it is one.
fn underlay_of(m: &Model, id: Id) -> Option<&Underlay> {
    match m.elements.get(id) {
        Some(Element::Underlay(u)) => Some(u),
        _ => None,
    }
}

/// A point of a `preview`'s answer.
fn point(v: &Value, key: &str) -> Option<P2> {
    let p = v[key].as_array()?;
    Some(P2(p.first()?.as_f64()?, p.get(1)?.as_f64()?))
}

/// One wall of a `preview`'s `found`.
fn traced(v: &Value) -> Option<Traced> {
    Some(Traced { start: point(v, "start")?, end: point(v, "end")?, thickness: v["thickness"].as_f64()? })
}

/// One opening of a `preview`'s `holes`.
fn hole(v: &Value) -> Option<Hole> {
    let thickness = v["thickness"].as_f64()?;
    Some(Hole { from: point(v, "from")?, to: point(v, "to")?, thickness, window: v["kind"] == json!("window") })
}

/// A layer's phase, picked from the project's. Named for the screen reader after the column and the layer, because a
/// row holds two of these and a `Grid`'s cell has no label to be labelled by.
fn phase_combo(ui: &mut Ui, salt: (Id, &str), name: String, current: &str, options: &[String]) -> Option<String> {
    let mut picked = None;
    let c = ComboBox::from_id_salt(salt).selected_text(current).width(ui.available_width()).show_ui(ui, |ui| {
        for o in options {
            if ui.selectable_label(o == current, o).clicked() && o != current {
                picked = Some(o.clone());
            }
        }
    });
    let now = current.to_string();
    c.response.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::ComboBox);
        info.label = Some(name.clone());
        info.current_text_value = Some(now.clone());
        info
    });
    picked
}

/// One of the measurements, with its label: a field in a `Grid` comes out narrow, and a bare one has no name for a
/// screen reader. Whether it was changed.
fn measure(ui: &mut Ui, label: &str, value: &mut f64, range: RangeInclusive<f64>, suffix: &str) -> bool {
    let name = t(label).to_string();
    ui.label(&name);
    let r = ui.add_sized([110.0, 20.0], DragValue::new(value).range(range).speed(1.0).suffix(suffix));
    r.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, true, name.clone()));
    r.changed()
}

/// The Layers tab: a row for each layer the drawing has. Each click is its own `cad_layers`, as the Phasing dialog's
/// are their own commands.
fn layers_tab(ui: &mut Ui, d: &TraceDialog, m: &Model, run: &mut Option<Value>) {
    let Some(u) = underlay_of(m, d.underlay) else { return };
    let Some(drawing) = m.drawings.get(&u.path) else {
        ui.label(RichText::new(t("Its file is not loaded.")).color(pal().muted));
        return;
    };
    let said = t(
        "What each layer of the drawing is of the project's history, as its own name says. A layer that is not drawn is not traced either.",
    );
    ui.label(RichText::new(said).color(pal().muted));
    ui.add_space(6.0);
    if ui.button(t("Read the Layer Names Again")).clicked() {
        let read = bimcraft_core::underlay::read_phases(m, u);
        let rows: Vec<Value> = drawing
            .layers
            .iter()
            .map(|l| {
                let p = read.iter().find(|p| p.layer == l.name);
                json!({"layer": l.name, "created": p.and_then(|p| p.created.clone()), "demolished": p.and_then(|p| p.demolished.clone())})
            })
            .collect();
        *run = Some(json!({"underlay": d.underlay, "layers": rows}));
    }
    ui.add_space(6.0);
    let none = t("<None>").to_string();
    let options: Vec<String> = std::iter::once(none.clone()).chain(m.phases.iter().cloned()).collect();
    ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(ui, |ui| {
        Grid::new("trace-layers").num_columns(4).spacing([10.0, 4.0]).striped(true).show(ui, |ui| {
            for label in ["Layer", "Shapes", "Built in", "Taken down in"] {
                ui.label(RichText::new(t(label)).color(pal().muted));
            }
            ui.end_row();
            for l in &drawing.layers {
                let mut drawn = !u.hidden.contains(&l.name);
                if ui.checkbox(&mut drawn, &l.name).changed() {
                    *run = Some(json!({"underlay": d.underlay, "layers": [{"layer": l.name, "drawn": drawn}]}));
                }
                ui.label(RichText::new(l.shapes.len().to_string()).color(pal().muted));
                let p = u.phases.iter().find(|p| p.layer == l.name);
                let created = ("created", "Built in", p.and_then(|p| p.created.clone()));
                let demolished = ("demolished", "Taken down in", p.and_then(|p| p.demolished.clone()));
                for (key, column, now) in [created, demolished] {
                    let now = now.unwrap_or_else(|| none.clone());
                    let salt = format!("{key} {}", l.name);
                    let name = format!("{}: {}", t(column), l.name);
                    if let Some(picked) = phase_combo(ui, (d.underlay, &salt), name, &now, &options) {
                        let said = if picked == none { Value::Null } else { json!(picked) };
                        *run = Some(json!({"underlay": d.underlay, "layers": [{"layer": l.name, key: said}]}));
                    }
                }
                ui.end_row();
            }
        });
    });
}

/// The Walls tab: what to look for, Find, Build, and what the last Find said.
fn walls_tab(ui: &mut Ui, d: &mut TraceDialog, m: &Model, find: &mut bool, build: &mut bool) {
    let said = t("A survey draws a wall as its two faces, so a pair of lines a wall's thickness apart is a wall. What to look for:");
    ui.label(RichText::new(said).color(pal().muted));
    ui.add_space(6.0);
    let mut changed = false;
    Grid::new("trace-how").num_columns(4).spacing([10.0, 6.0]).show(ui, |ui| {
        changed |= measure(ui, "Thinnest wall", &mut d.how.thin, 1.0..=10_000.0, " mm");
        changed |= measure(ui, "Thickest wall", &mut d.how.thick, 1.0..=10_000.0, " mm");
        ui.end_row();
        changed |= measure(ui, "Shortest wall", &mut d.how.shortest, 1.0..=100_000.0, " mm");
        changed |= measure(ui, "Out of parallel", &mut d.how.skew, 0.0..=45.0, "\u{b0}");
        ui.end_row();
    });
    ui.add_space(6.0);
    changed |= ui.checkbox(&mut d.how.openings, t("Find the openings too")).changed();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let by = t("By thickness").to_string();
        let label = ui.label(t("Wall type"));
        let now = d.kind.clone().unwrap_or_else(|| by.clone());
        let options: Vec<String> = std::iter::once(by.clone()).chain(m.types.wall.keys().cloned()).collect();
        if let Some(picked) = combo(ui, (d.underlay, "trace-type"), label.id, &now, &options) {
            d.kind = (picked != by).then_some(picked);
            changed = true;
        }
    });
    if changed {
        d.stale();
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        *find = ui.button(t("Find")).clicked();
        let count = d.found.len();
        *build = ui.add_enabled(count > 0, Button::new(tf("Build {}", &[&count.to_string()]))).clicked();
    });
    ui.add_space(6.0);
    let Some(report) = &d.report else {
        let said = t("Find reads the drawing and draws the walls it found over the plan; nothing is built until Build.");
        ui.label(RichText::new(said).color(pal().muted));
        return;
    };
    found_shown(ui, report);
}

/// What the last Find found: how much, of what thickness and type, in which phase, and the layers that disagreed.
fn found_shown(ui: &mut Ui, report: &Value) {
    let number = |key: &str| report[key].as_u64().unwrap_or(0).to_string();
    let metres = format!("{:.2}", report["length"].as_f64().unwrap_or(0.0) / 1000.0);
    ui.label(
        RichText::new(tf("{} walls, {} m, of {} lines long enough to be a wall's face.", &[&number("walls"), &metres, &number("faces")]))
            .strong(),
    );
    // A drawing in metres that says millimetres has nothing in it as thick as a wall: its longest line says so.
    if report["walls"].as_u64() == Some(0) {
        let longest = format!("{:.2}", report["longest"].as_f64().unwrap_or(0.0));
        let said = tf(
            "The longest line in it is {} mm. A drawing in metres that says millimetres reads like this: link it again with a scale of 1000.",
            &[&longest],
        );
        ui.label(RichText::new(said).color(pal().muted));
    }
    let (doors, windows) = (number("doors"), number("windows"));
    if doors != "0" || windows != "0" {
        let what = [category("door", doors != "1"), category("window", windows != "1")];
        ui.label(tf("{} {} and {} {}.", &[&doors, &what[0], &windows, &what[1]]));
    }
    if let Some(left) = report["too_high"].as_u64() {
        let said = tf("{} openings stood higher than the wall they are in and were left out.", &[&left.to_string()]);
        ui.label(RichText::new(said).color(pal().error));
    }
    ScrollArea::vertical().max_height(130.0).auto_shrink([false, true]).show(ui, |ui| {
        for row in report["thicknesses"].as_array().into_iter().flatten() {
            let thickness = format!("{:.0}", row["thickness"].as_f64().unwrap_or(0.0));
            let walls = row["walls"].as_u64().unwrap_or(0).to_string();
            let kind = row["type"].as_str().unwrap_or_default();
            let line = tf("{} mm: {} \u{d7} {}", &[&thickness, &walls, kind]);
            match row["made"].as_bool() == Some(true) {
                true => ui.label(tf("{} (a type made for it)", &[&line])),
                false => ui.label(line),
            };
        }
        for row in report["openings"].as_array().into_iter().flatten() {
            let width = format!("{:.0}", row["width"].as_f64().unwrap_or(0.0));
            let count = row["openings"].as_u64().unwrap_or(0);
            let kind = category(if row["kind"] == json!("window") { "window" } else { "door" }, count != 1);
            let line = tf("{} mm {}: {} \u{d7} {}", &[&width, &kind, &count.to_string(), row["type"].as_str().unwrap_or_default()]);
            match row["made"].as_bool() == Some(true) {
                true => ui.label(tf("{} (a type made for it)", &[&line])),
                false => ui.label(line),
            };
        }
        for row in report["phases"].as_array().into_iter().flatten() {
            let walls = row["walls"].as_u64().unwrap_or(0).to_string();
            ui.label(tf("{}: {} walls", &[row["phase"].as_str().unwrap_or_default(), &walls]));
        }
    });
    let pairs: Vec<String> = report["disagreed"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let layers = p["layers"].as_array()?;
            Some(format!("{} / {}", layers.first()?.as_str()?, layers.get(1)?.as_str()?))
        })
        .collect();
    if !pairs.is_empty() {
        ui.add_space(4.0);
        let said = tf("These layers name different phases, so the walls between them come in the first: {}", &[&pairs.join(", ")]);
        ui.label(RichText::new(said).color(pal().error));
    }
}

impl BimApp {
    /// Trace Walls (TW): the drawing under the plan on screen. A window with no plan open, or a plan with no drawing
    /// under it, says so, as Link CAD does.
    pub(crate) fn open_trace(&mut self) {
        let Some(level) = self.plan_level() else {
            self.status = crate::Status { text: t("Trace Walls: open a plan first").to_string(), error: true };
            return;
        };
        let m = &self.session.model;
        let here = m.guid(level);
        let found = m.elements.iter().find_map(|(id, e)| match e {
            Element::Underlay(u) if u.level == here => Some(*id),
            _ => None,
        });
        let Some(underlay) = found else {
            let text = t("Trace Walls: bring a drawing under this plan with Link CAD first").to_string();
            self.status = crate::Status { text, error: true };
            return;
        };
        let how = How::default();
        let (found, holes) = (Vec::new(), Vec::new());
        self.trace_dialog = Some(TraceDialog { underlay, tab: 1, how, kind: None, found, holes, report: None, error: String::new() });
    }

    pub(crate) fn trace_window(&mut self, ctx: &Context) {
        let Some(mut d) = self.trace_dialog.take() else { return };
        let model = Arc::clone(&self.session.model);
        // The drawing may have been taken out of the project while the window stood open.
        let Some(name) = underlay_of(&model, d.underlay).map(|u| u.name.clone()) else { return };
        let (mut close, mut layers, mut find, mut build) = (false, None::<Value>, false, false);
        let mut open = true;
        egui::Window::new(t("Trace Walls"))
            .id(UiId::new("trace-walls"))
            .open(&mut open)
            // Rolled up to its title bar, it is out of the way of the walls it drew on the plan without being closed.
            .collapsible(true)
            .resizable(false)
            // Where it stands the first time: under the ribbon and against the palettes down the left, so the plan it
            // reads is beside it. After that it stands where it was dragged.
            .default_pos(pos2(320.0, 190.0))
            .show(ctx, |ui| {
                // As large on either tab, so the window doesn't move about as the tabs change.
                ui.set_width(600.0);
                ui.set_min_height(360.0);
                ui.label(RichText::new(tf("From the drawing {}, under this plan.", &[&name])).color(pal().muted));
                ui.add_space(6.0);
                let tabs = [t("Layers").to_string(), category("wall", true)];
                ui.horizontal(|ui| {
                    for (i, label) in tabs.iter().enumerate() {
                        if ui.selectable_label(d.tab == i, label).clicked() {
                            d.tab = i;
                        }
                    }
                });
                ui.separator();
                match d.tab {
                    0 => layers_tab(ui, &d, &model, &mut layers),
                    _ => walls_tab(ui, &mut d, &model, &mut find, &mut build),
                }
                if !d.error.is_empty() {
                    ui.add_space(6.0);
                    ui.label(RichText::new(&d.error).color(pal().error));
                }
                ui.add_space(8.0);
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| close = ui.button(t("Close")).clicked());
            });
        let asked = match (layers, find || build) {
            (Some(p), _) => Some(("cad_layers", p)),
            (None, true) => Some(("trace_walls", d.params(find))),
            (None, false) => None,
        };
        if let Some((id, params)) = asked {
            d.error.clear();
            match self.run(id, params) {
                // Find is the one answer worth keeping: the walls to draw over the plan, and what to show of them.
                Some(reply) if find => {
                    d.found = reply["found"].as_array().into_iter().flatten().filter_map(traced).collect();
                    d.holes = reply["holes"].as_array().into_iter().flatten().filter_map(hole).collect();
                    d.report = Some(reply);
                }
                Some(reply) => {
                    if build {
                        let walls = reply["walls"].as_u64().unwrap_or(0).to_string();
                        self.status = crate::Status { text: tf("{} walls built from {}.", &[&walls, &name]), error: false };
                        close = true;
                    }
                    d.stale();
                }
                None => {
                    d.error = std::mem::take(&mut self.status.text);
                    self.status = crate::Status::default();
                    d.stale();
                }
            }
        }
        if open && !close {
            self.trace_dialog = Some(d);
        }
    }

    /// The walls the last Find found, drawn over the plan of the drawing's own level: a band as thick as each
    /// measured, with its axis down it. Nothing where no Find has been made, or on any other plan.
    pub(crate) fn traced_shapes(&self, view: &View, origin: Pos2, level: Id) -> Vec<Shape> {
        let Some(d) = &self.trace_dialog else { return Vec::new() };
        let m = &self.session.model;
        if underlay_of(m, d.underlay).map(|u| u.level) != Some(m.guid(level)) {
            return Vec::new();
        }
        let at = |p: P2| {
            let (x, y) = view.px(p);
            origin + vec2(x, y)
        };
        let mut out = Vec::new();
        for w in &d.found {
            let ends = [at(w.start), at(w.end)];
            let width = ((w.thickness * view.scale) as f32).max(1.0);
            out.push(Shape::line_segment(ends, Stroke::new(width, pal().select_fill.gamma_multiply(0.7))));
            out.push(Shape::line_segment(ends, Stroke::new(1.0, pal().accent)));
        }
        // An opening over the wall it is in, lighter than the wall: a door is left open and a window is glazed, so a
        // window keeps a line down the middle of it and a door does not.
        for h in &d.holes {
            let ends = [at(h.from), at(h.to)];
            let width = ((h.thickness * view.scale) as f32).max(1.0);
            out.push(Shape::line_segment(ends, Stroke::new(width, pal().hover_fill)));
            if h.window {
                out.push(Shape::line_segment(ends, Stroke::new(1.0, pal().hover)));
            }
        }
        out
    }
}
