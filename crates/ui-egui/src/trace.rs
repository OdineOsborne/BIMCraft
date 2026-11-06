//! Trace Walls (TW): the drawing under the plan on screen read for its walls, with what was found shown before
//! anything is built. The Layers tab is every layer of the drawing — whether it is drawn, and what it is of the
//! project's history as Link CAD read it from the layers' own names — put right a row at a time (`cad_layers`); the
//! Walls tab is what to look for, Find (`trace_walls` with `preview`, which draws what it found over the plan) and
//! Build (`trace_walls`). Nothing is built until Build is pressed, and a layer that is not drawn is not traced. The
//! Heights tab is what an elevation that stands against its plan says of the storey: how high its walls stand
//! (`cad_storey`) and the sill and the head of every opening in them (`cad_openings`), read first and given to what
//! was built on a second press.
//!
//! It floats and is dragged by its title, and it does not take the keys, because what it shows is on the plan behind
//! it: the walls it found are no use under a modal that stands in the middle of them and cannot be moved. So the plan
//! pans, zooms and fits (`ZF`) while it is open, and the only typing it takes is into numbers.

use std::ops::RangeInclusive;
use std::sync::Arc;

use bimcraft_core::geom::P2;
use bimcraft_core::ids::Guid;
use bimcraft_core::model::{Element, Id, Model, Part, PartKind, Underlay};
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

/// What of the drawing Trace Walls reads: the whole of it, what the plan's crop holds, or one of its parts, which
/// carries its own level and phase as well as its area.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum What {
    Whole,
    Crop,
    Part(String),
}

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
    /// Whether to place a room in every area the walls shut in, once they are built.
    pub(crate) rooms: bool,
    /// What of the drawing to read: a drawing that holds a whole set of sheets side by side is traced one sheet at
    /// a time.
    pub(crate) what: What,
    /// The crop region of the plan on screen as it stands this frame, or none where it is not cropped.
    pub(crate) crop: Option<[P2; 2]>,
    /// The level whose plan is on screen: what a part added here is the plan of, until it is said otherwise.
    pub(crate) level: Option<Guid>,
    pub(crate) kind: Option<String>,
    /// The Openings tab: which elevation the sills and the heads are read off, which plan the openings are found
    /// in, how high the storey's floor stands as the drawing writes heights (0 is not said), and the last reading.
    pub(crate) elevation: Option<String>,
    pub(crate) plan: Option<String>,
    pub(crate) floor: f64,
    pub(crate) openings: Option<Value>,
    /// The last reading of how high the storey stands, and the top picked out of what it draws (none: the highest
    /// the reading found, which is what the command picks for itself).
    pub(crate) storey: Option<Value>,
    pub(crate) top: Option<f64>,
    /// And the last reading of the roof over them, with the shape picked out of what it found (none: the one
    /// whose ridge the drawing agrees with, which is what the command picks for itself).
    pub(crate) roof: Option<Value>,
    pub(crate) gables: Option<Vec<usize>>,
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
            "alone": self.how.alone,
        });
        if let Some(kind) = &self.kind {
            p["type"] = json!(kind);
        }
        match &self.what {
            What::Whole => {}
            What::Crop => {
                if let Some([sw, ne]) = self.crop {
                    p["region"] = json!([[sw.0, sw.1], [ne.0, ne.1]]);
                }
            }
            What::Part(name) => p["part"] = json!(name),
        }
        match preview {
            // A Find builds nothing, so it places no rooms either: the rooms are of the walls, not of the drawing.
            true => p["preview"] = json!(true),
            false => p["rooms"] = json!(self.rooms),
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

/// The Parts tab: what each part of the drawing is of the building. A change is its own `cad_parts`, carrying the
/// whole list, as the Layers tab's clicks are their own `cad_layers`.
fn parts_tab(ui: &mut Ui, d: &TraceDialog, m: &Model, run: &mut Option<Value>) {
    let Some(u) = underlay_of(m, d.underlay) else { return };
    let said = t(
        "What each part of the drawing is of the building. A survey is often saved as a whole set of sheets side by side in one file, and this says which rectangle of it is which, so that each is read as what it is.",
    );
    ui.label(RichText::new(said).color(pal().muted));
    ui.add_space(6.0);
    let mut parts = u.parts.clone();
    let mut changed = false;
    let cropped = d.crop.is_some();
    let add = ui.add_enabled(cropped, Button::new(t("Add the plan's crop as a part")));
    let clicked = add.clicked();
    if !cropped {
        add.on_hover_text(t("Crop the plan to the part of the drawing to trace, and only that part is read."));
    }
    if clicked && let Some(region) = d.crop {
        // The survey's own caption for it, where it writes one: a sheet is captioned under it, larger than anything
        // written inside it.
        let taken = |name: &String| parts.iter().any(|part: &Part| part.name == *name);
        let name = bimcraft_core::underlay::caption(m, u, region)
            .filter(|said| !taken(said))
            .unwrap_or_else(|| tf("Part {}", &[&(parts.len() + 1).to_string()]));
        parts.push(Part { name, kind: PartKind::Plan, region, level: d.level, phase: None, anchor: None });
        changed = true;
    }
    ui.add_space(6.0);
    if parts.is_empty() {
        ui.label(RichText::new(t("It is made of no parts yet, so Trace Walls reads the whole of it.")).color(pal().muted));
        return;
    }
    let none = t("<None>").to_string();
    let kinds: Vec<String> =
        [PartKind::Plan, PartKind::Elevation, PartKind::Section].iter().map(|k| t(kind_word(*k)).to_string()).collect();
    let levels: Vec<(String, Id)> = m.levels().iter().map(|(id, l)| (l.name.clone(), *id)).collect();
    let level_options: Vec<String> = std::iter::once(none.clone()).chain(levels.iter().map(|(name, _)| name.clone())).collect();
    let phase_options: Vec<String> = std::iter::once(none.clone()).chain(m.phases.iter().cloned()).collect();
    let mut drop: Option<usize> = None;
    ScrollArea::vertical().max_height(240.0).auto_shrink([false, true]).show(ui, |ui| {
        Grid::new("trace-parts").num_columns(6).spacing([10.0, 4.0]).striped(true).show(ui, |ui| {
            for label in ["Name", "What it is", "Level", "Phase", "Heights"] {
                ui.label(RichText::new(t(label)).color(pal().muted));
            }
            ui.label("");
            ui.end_row();
            for (i, part) in parts.iter_mut().enumerate() {
                let salt = format!("{i} {}", part.name);
                changed |= ui.add_sized([170.0, 20.0], egui::TextEdit::singleline(&mut part.name)).changed();
                let row = part.name.clone();
                let named = |column: &str| format!("{}: {row}", t(column));
                let now = t(kind_word(part.kind)).to_string();
                if let Some(picked) = phase_combo(ui, (d.underlay, &format!("kind {salt}")), named("What it is"), &now, &kinds) {
                    part.kind = match kinds.iter().position(|k| *k == picked) {
                        Some(1) => PartKind::Elevation,
                        Some(2) => PartKind::Section,
                        _ => PartKind::Plan,
                    };
                    changed = true;
                }
                let on = part.level.and_then(|g| levels.iter().find(|(_, id)| m.guid(*id) == g)).map(|(name, _)| name.clone());
                let now = on.unwrap_or_else(|| none.clone());
                if let Some(picked) = phase_combo(ui, (d.underlay, &format!("level {salt}")), named("Level"), &now, &level_options) {
                    part.level = levels.iter().find(|(name, _)| *name == picked).map(|(_, id)| m.guid(*id));
                    changed = true;
                }
                let now = part.phase.clone().unwrap_or_else(|| none.clone());
                if let Some(picked) = phase_combo(ui, (d.underlay, &format!("phase {salt}")), named("Phase"), &now, &phase_options) {
                    part.phase = (picked != none).then_some(picked);
                    changed = true;
                }
                // What an elevation writes about how high things are, and whether its marks bear one another out:
                // an area that holds two views of the building holds two datums, and the marks say so.
                match part.kind {
                    PartKind::Plan => {
                        ui.label("");
                    }
                    _ => {
                        let read = bimcraft_core::underlay::heights(m, u, part.region);
                        let said = match (read.marks.len(), read.worst <= bimcraft_core::underlay::HEIGHTS_AGREE) {
                            (0, _) => t("None written").to_string(),
                            (n, true) => tf("{} heights, within {} mm", &[&n.to_string(), &format!("{:.0}", read.worst)]),
                            (n, false) => tf("{} heights, {} mm apart", &[&n.to_string(), &format!("{:.0}", read.worst)]),
                        };
                        let colour = match read.worst <= bimcraft_core::underlay::HEIGHTS_AGREE {
                            true => pal().muted,
                            false => pal().error,
                        };
                        ui.label(RichText::new(said).color(colour));
                    }
                }
                if ui.button(t("Remove")).clicked() {
                    drop = Some(i);
                }
                ui.end_row();
            }
        });
    });
    if let Some(i) = drop {
        parts.remove(i);
        changed = true;
    }
    if changed {
        let rows: Vec<Value> = parts.iter().map(|part| part_json(m, part)).collect();
        *run = Some(json!({"underlay": d.underlay, "parts": rows}));
    }
}

/// The word a kind of part is shown by.
fn kind_word(kind: PartKind) -> &'static str {
    match kind {
        PartKind::Plan => "Plan",
        PartKind::Elevation => "Elevation",
        PartKind::Section => "Section",
    }
}

/// A part as `cad_parts` takes it. Everything it holds goes back, the anchor included: the list is the whole of
/// what the drawing is made of, so what this leaves out is thrown away.
fn part_json(m: &Model, part: &Part) -> Value {
    let [sw, ne] = part.region;
    let point = |p: bimcraft_core::geom::P2| json!([p.0, p.1]);
    json!({
        "name": part.name,
        "kind": part.kind.word(),
        "region": [[sw.0, sw.1], [ne.0, ne.1]],
        "level": part.level.map(|g| m.elements.id(g)),
        "phase": part.phase,
        "anchor": part.anchor.as_ref().map(|a| {
            json!({"plan": [point(a.plan[0]), point(a.plan[1])], "drawn": [point(a.drawn[0]), point(a.drawn[1])]})
        }),
    })
}

/// The Openings tab: the sill and the head of every opening, read off an elevation that stands against its plan
/// (`cad_openings`). Reading builds nothing and changes nothing: what it says is what the drawing draws. Apply then
/// gives that to the openings the Walls tab built, which until then stand at the size a starter type is.
fn heights_tab(
    ui: &mut Ui,
    d: &mut TraceDialog,
    m: &Model,
    read: &mut Option<Value>,
    storey: &mut Option<Value>,
    roof: &mut Option<Value>,
) {
    let said = t(
        "What an elevation says of the storey it draws: how high its walls stand, and the sill and the head of every opening in them. The elevation has to stand against its plan first.",
    );
    ui.label(RichText::new(said).color(pal().muted));
    ui.add_space(6.0);
    let Some(u) = underlay_of(m, d.underlay) else { return };
    let named = |plans: bool| -> Vec<String> {
        u.parts.iter().filter(|part| (part.kind == PartKind::Plan) == plans).map(|part| part.name.clone()).collect()
    };
    let (views, plans) = (named(false), named(true));
    if views.is_empty() || plans.is_empty() {
        let said = t("It needs a plan and an elevation among its parts, and the Parts tab is where they are said.");
        ui.label(RichText::new(said).color(pal().muted));
        return;
    }
    // A part done away with, or made a plan, while the window stood open leaves nothing of it to read.
    let held = |now: &Option<String>, of: &[String]| now.clone().filter(|name| of.contains(name)).or_else(|| of.first().cloned());
    d.elevation = held(&d.elevation, &views);
    d.plan = held(&d.plan, &plans);
    let (Some(elevation), Some(plan)) = (d.elevation.clone(), d.plan.clone()) else { return };
    let mut changed = false;
    ui.horizontal(|ui| {
        let label = ui.label(t("The elevation"));
        if let Some(picked) = combo(ui, (d.underlay, "openings-elevation"), label.id, &elevation, &views) {
            d.elevation = Some(picked);
            changed = true;
        }
    });
    ui.horizontal(|ui| {
        let label = ui.label(t("The plan"));
        if let Some(picked) = combo(ui, (d.underlay, "openings-plan"), label.id, &plan, &plans) {
            d.plan = Some(picked);
            changed = true;
        }
    });
    ui.add_space(6.0);
    // A door's sill is the floor itself and the elevation draws no line of its own for it, so the floor is what
    // gives a door its height: the height the storey's floor stands at, as the elevation writes heights.
    ui.horizontal(|ui| changed |= measure(ui, "The storey's floor", &mut d.floor, 0.0..=10_000_000.0, " mm"));
    let said = t("A door's sill is the floor itself, and no line is drawn across it, so the floor is what gives a door its height.");
    ui.label(RichText::new(said).color(pal().muted));
    if changed {
        (d.openings, d.storey, d.top, d.roof, d.gables) = (None, None, None, None, None);
    }
    let (underlay, floor) = (d.underlay, d.floor);
    let params = |apply: bool, top: Option<f64>| {
        let mut p = json!({"underlay": underlay, "part": elevation.clone(), "plan": plan.clone()});
        if floor > 0.0 {
            p["floor"] = json!(floor);
        }
        if apply {
            p["apply"] = json!(true);
        }
        if let Some(top) = top {
            p["top"] = json!(top);
        }
        p
    };
    ui.add_space(8.0);
    ui.separator();
    // The walls: a plan draws no height at all, so what was built goes up to the level above, wherever the
    // project put it, until the elevation says how high the storey really stands.
    ui.label(RichText::new(t("The walls of this storey, as high as the elevation draws them:")).color(pal().muted));
    let tops: Vec<(f64, String)> = d
        .storey
        .iter()
        .flat_map(|report| report["courses"].as_array().cloned().unwrap_or_default())
        .filter(|course| course["within"].as_bool() == Some(true))
        .filter_map(|course| {
            let (height, above) = (course["height"].as_f64()?, course["above"].as_f64()?);
            Some((height, format!("{:.2} m ({above:.0} mm)", height / 1000.0)))
        })
        .collect();
    ui.horizontal(|ui| {
        if ui.button(t("Read the Storey's Top")).clicked() {
            *storey = Some(params(false, None));
        }
        // Which of the lines it draws the width of the facade is the top of these walls: the highest of them
        // within a storey of the floor, until another is picked.
        if !tops.is_empty() {
            let now = d.top.and_then(|top| tops.iter().find(|(height, _)| (height - top).abs() < 1.0)).or_else(|| tops.last());
            let now = now.map(|(_, said)| said.clone()).unwrap_or_default();
            let options: Vec<String> = tops.iter().map(|(_, said)| said.clone()).collect();
            let label = ui.label(t("Its top"));
            if let Some(picked) = combo(ui, (d.underlay, "storey-top"), label.id, &now, &options) {
                d.top = tops.iter().find(|(_, said)| *said == picked).map(|(height, _)| *height);
            }
        }
    });
    if ui.add_enabled(d.storey.is_some() && floor > 0.0, Button::new(t("Give the Walls This Height"))).clicked() {
        *storey = Some(params(true, d.top));
    }
    if let Some(report) = &d.storey {
        storey_shown(ui, report);
    }
    ui.add_space(8.0);
    ui.separator();
    // The roof: the one thing of a building whose pitch no plan draws, and which the elevation that draws its
    // gable or its hip draws at the angle it really rises at.
    ui.label(RichText::new(t("The roof over them, as the elevation draws its pitch:")).color(pal().muted));
    // Which shape it is built in: the one whose ridge comes out where the drawing draws it, until another is
    // picked. Each is said by the ridge it would reach and how many edges of the roof stand upright in it.
    let shapes: Vec<(Vec<usize>, String)> = d
        .roof
        .iter()
        .flat_map(|report| report["shapes"].as_array().cloned().unwrap_or_default())
        .filter_map(|shape| {
            let gables: Vec<usize> = shape["gables"].as_array()?.iter().filter_map(|i| i.as_u64()).map(|i| i as usize).collect();
            let ridge = shape["ridge"].as_f64()?;
            Some((gables.clone(), format!("{:.2} m ({})", ridge / 1000.0, gables.len())))
        })
        .collect();
    let picked = d.gables.clone().or_else(|| {
        d.roof
            .iter()
            .flat_map(|report| report["shapes"].as_array().cloned().unwrap_or_default())
            .find(|shape| shape["picked"].as_bool() == Some(true))
            .and_then(|shape| Some(shape["gables"].as_array()?.iter().filter_map(|i| i.as_u64()).map(|i| i as usize).collect()))
    });
    let roof_params = |apply: bool| {
        let mut p = params(apply, None);
        if let Some(gables) = &picked {
            p["gables"] = json!(gables);
        }
        p
    };
    let read_roof = d.roof.is_some();
    ui.horizontal(|ui| {
        if ui.button(t("Read the Roof")).clicked() {
            *roof = Some(params(false, None));
        }
        if !shapes.is_empty() {
            let now = picked
                .as_ref()
                .and_then(|gables| shapes.iter().find(|(one, _)| one == gables))
                .map(|(_, said)| said.clone())
                .unwrap_or_default();
            let options: Vec<String> = shapes.iter().map(|(_, said)| said.clone()).collect();
            let label = ui.label(t("Its shape"));
            if let Some(got) = combo(ui, (d.underlay, "roof-shape"), label.id, &now, &options) {
                d.gables = shapes.iter().find(|(_, said)| *said == got).map(|(gables, _)| gables.clone());
            }
        }
    });
    if !shapes.is_empty() {
        let said = t("The ridge each shape would reach, and how many edges of the roof stand upright in it.");
        ui.label(RichText::new(said).color(pal().muted));
    }
    if ui.add_enabled(read_roof && floor > 0.0, Button::new(t("Build This Roof"))).clicked() {
        *roof = Some(roof_params(true));
    }
    if let Some(report) = &d.roof {
        roof_shown(ui, report);
    }
    ui.add_space(8.0);
    ui.separator();
    // The openings: what was read given to the ones already built, which stand at the size the project's own
    // starter type is until an elevation says otherwise.
    ui.label(RichText::new(t("The openings in them, as the elevation draws their sills and their heads:")).color(pal().muted));
    let had = d.openings.is_some();
    ui.horizontal(|ui| {
        if ui.button(t("Read the Sills and Heads")).clicked() {
            *read = Some(params(false, None));
        }
        if ui.add_enabled(had && floor > 0.0, Button::new(t("Apply These Heights"))).clicked() {
            *read = Some(params(true, None));
        }
    });
    if (had || d.storey.is_some()) && floor <= 0.0 {
        let said = t("Giving these heights needs the storey's floor said, since a sill and a wall both stand so far above it.");
        ui.label(RichText::new(said).color(pal().muted));
    }
    ui.add_space(6.0);
    let Some(report) = &d.openings else { return };
    openings_shown(ui, report);
}

/// What the last reading of the storey said: how high it stands, what was given to its walls, and what was left
/// as it was. The level above is said too and never moved: what else stands on it is no business of a drawing's.
fn storey_shown(ui: &mut Ui, report: &Value) {
    match (report["high"].as_f64(), report["top"].as_f64()) {
        (Some(high), Some(top)) => {
            let (high, top) = (format!("{high:.0}"), format!("{:.2}", top / 1000.0));
            ui.label(RichText::new(tf("This storey stands {} mm high, its top drawn at {} m.", &[&high, &top])).strong());
        }
        _ => {
            let said = t(
                "No line of this elevation runs the width of its facade within a storey of that floor, so nothing says how high these walls stand.",
            );
            ui.label(RichText::new(said).color(pal().error));
        }
    }
    let number = |key: &str| report[key].as_u64().unwrap_or(0);
    match report["applied"].as_u64() {
        Some(walls) => {
            let said = tf("{} walls of this storey were made that high.", &[&walls.to_string()]);
            ui.label(RichText::new(said).strong());
        }
        None => {
            let said = tf("{} walls stand on this storey.", &[&number("walls").to_string()]);
            ui.label(RichText::new(said).color(pal().muted));
        }
    }
    if number("too_short") > 0 {
        let said = tf(
            "{} of them have an opening that would stand through that, and were left as high as they were.",
            &[&number("too_short").to_string()],
        );
        ui.label(RichText::new(said).color(pal().error));
    }
    if number("attached") > 0 {
        let said = tf(
            "{} of them are attached to a roof or a floor, which is their top whatever height they are given.",
            &[&number("attached").to_string()],
        );
        ui.label(RichText::new(said).color(pal().muted));
    }
    // The level above is where the project put it and not where this drawing says: it is said, and left alone.
    if report["level_above"]["agrees"].as_bool() == Some(false)
        && let (Some(name), Some(up)) = (report["level_above"]["name"].as_str(), report["level_above"]["high"].as_f64())
    {
        let said = tf("{} stands {} mm above this floor, and is left where the project put it.", &[name, &format!("{up:.0}")]);
        ui.label(RichText::new(said).color(pal().muted));
    }
}

/// What the last reading of the roof said: the pitch the elevation draws it at, where its eaves and its ridge
/// stand, and whether the ridge of the shape that was built comes out where the drawing draws it. A gable end and
/// a hipped end are drawn alike, so that last one is the whole check on which was taken.
fn roof_shown(ui: &mut Ui, report: &Value) {
    match (report["slope"].as_f64(), report["eaves"].as_f64()) {
        (Some(slope), Some(eaves)) => {
            let (slope, eaves) = (format!("{slope:.2}"), format!("{eaves:.0}"));
            ui.label(RichText::new(tf("This roof rises {} degrees, its eaves {} mm above the floor.", &[&slope, &eaves])).strong());
        }
        _ => {
            let said = t(
                "This elevation draws nothing above the floor at a pitch a roof is built at, so nothing on it says how steeply this roof rises.",
            );
            ui.label(RichText::new(said).color(pal().error));
        }
    }
    // Which shape was built is what the ridge the drawing draws settled, so the two are said together.
    if let Some(ridge) = report["ridge"].as_f64() {
        let ridge = format!("{ridge:.0}");
        let said = match report["gables"].as_array().is_some_and(|gables| !gables.is_empty()) {
            true => tf("Its ridge is drawn {} mm above the floor, and the roof is built with gable ends.", &[&ridge]),
            false => tf("Its ridge is drawn {} mm above the floor, and the roof is built hipped all round.", &[&ridge]),
        };
        ui.label(RichText::new(said).color(pal().muted));
    }
    if report["ridge_agrees"].as_bool() == Some(false)
        && let Some(made) = report["made_ridge"].as_f64()
    {
        let said = tf(
            "The roof this builds puts its ridge {} mm above the floor, which is not where the drawing draws it.",
            &[&format!("{made:.0}")],
        );
        ui.label(RichText::new(said).color(pal().error));
    }
    if report["overhang_from"].as_str() == Some("slopes")
        && let Some(over) = report["overhang"].as_f64()
    {
        let said =
            tf("It reaches {} mm past its walls, which is how far outside this facade its slopes are drawn.", &[&format!("{over:.0}")]);
        ui.label(RichText::new(said).color(pal().muted));
    }
    if let (Some(roof), Some(level)) = (report["roof"].as_u64(), report["level"].as_str()) {
        let said = tf("Roof {} was built, standing on {}.", &[&roof.to_string(), level]);
        ui.label(RichText::new(said).strong());
    }
}

/// What the last reading of the sills and the heads said: how many openings the elevation draws, each with where it
/// draws it, and what it left unsaid rather than inventing.
fn openings_shown(ui: &mut Ui, report: &Value) {
    let number = |key: &str| report[key].as_u64().unwrap_or(0);
    let rows = report["openings"].as_array().cloned().unwrap_or_default();
    let found = number("found").to_string();
    ui.label(
        RichText::new(tf("{} openings on this face, {} of them with a sill and a head.", &[&rows.len().to_string(), &found])).strong(),
    );
    // What was given to the openings already built, where that was asked for: what each of them is made to now,
    // and what was left as it was rather than made to a reading that says nothing.
    if let Some(applied) = report["applied"].as_u64() {
        let said = tf("{} openings were given the heights the elevation draws.", &[&applied.to_string()]);
        ui.label(RichText::new(said).strong().color(pal().text));
        let made: Vec<String> = report["made"].as_array().into_iter().flatten().filter_map(|n| n.as_str().map(str::to_string)).collect();
        if !made.is_empty() {
            ui.label(
                RichText::new(tf("{} types were made for them: {}.", &[&made.len().to_string(), &made.join(", ")])).color(pal().muted),
            );
        }
        if let Some(left) = report["left"].as_u64().filter(|left| *left > 0) {
            let said = tf("{} of them were left as they were, each saying why.", &[&left.to_string()]);
            ui.label(RichText::new(said).color(pal().error));
        }
    }
    if number("missed") > 0 {
        let said =
            tf("{} of them have no two lines drawn across them, so nothing is said of their heights.", &[&number("missed").to_string()]);
        ui.label(RichText::new(said).color(pal().error));
    }
    if number("elsewhere") > 0 {
        let said = tf("{} openings are on another side of the building.", &[&number("elsewhere").to_string()]);
        ui.label(RichText::new(said).color(pal().muted));
    }
    // A wall's openings are drawn in its one face, so one that stands off the plane the others agree on is another
    // wall's, and the lines read of it may be those of whatever the elevation draws in front of it.
    if number("off_plane") > 0 {
        let said = tf(
            "{} of them stand off the plane the others agree on, so the lines read of them may be another opening's.",
            &[&number("off_plane").to_string()],
        );
        ui.label(RichText::new(said).color(pal().error));
    }
    // One shift for all of them is two anchor points picked that much off the face, which is worth putting right:
    // the openings are drawn where the facade is, not where the picking was.
    if let Some(shift) = report["shift"].as_f64().filter(|shift| shift.abs() > 100.0) {
        let said = tf(
            "Their lines fall {} mm from where the plan puts them, so the two points this elevation stands by are that much along the face.",
            &[&format!("{shift:.0}")],
        );
        ui.label(RichText::new(said).color(pal().muted));
    }
    // The heights an elevation writes should all put its datum in the same place; where they do not, what it says
    // of a sill stands on the same shaky ground.
    if report["agree"].as_bool() == Some(false) {
        let said = t("The heights written on this elevation do not bear one another out, so what it says of a sill may be out too.");
        ui.label(RichText::new(said).color(pal().error));
    }
    ScrollArea::vertical().max_height(180.0).auto_shrink([false, true]).show(ui, |ui| {
        for row in &rows {
            let width = format!("{:.0}", row["width"].as_f64().unwrap_or_default());
            let what = category(if row["kind"] == json!("window") { "window" } else { "door" }, false);
            // One that stands off the plane the rest agree on is said in the colour of what is wrong with it: the
            // lines read of it may be those of whatever the elevation draws in front of it.
            let colour = match (row["on_plane"].as_bool(), row["high"].as_f64()) {
                (Some(false), _) => pal().error,
                (_, None) => pal().muted,
                _ => pal().text,
            };
            // What it is made to where it was given these heights, and what the elevation draws of it where it was
            // not: a door is made from the floor it opens onto, and the elevation may draw its threshold a step
            // under that, so the two are not always the same number.
            let (how_high, how_far) = (row["made"].as_f64().or(row["high"].as_f64()), row["made_sill"].as_f64().or(row["above"].as_f64()));
            let said = match (row["sill"].as_f64(), row["head"].as_f64(), how_high, how_far) {
                // Where the storey's floor is known, how high the sill stands above it and how tall the opening
                // is: the two numbers an opening is made to, rather than the heights the drawing writes.
                (Some(_), Some(_), Some(high), Some(above)) => {
                    tf("{} mm {}: sill {} mm above the floor, {} mm high", &[&width, &what, &format!("{above:.0}"), &format!("{high:.0}")])
                }
                (Some(sill), Some(head), Some(high), None) => {
                    let (sill, head) = (format!("{:.2}", sill / 1000.0), format!("{:.2}", head / 1000.0));
                    tf("{} mm {}: sill {} m, head {} m, {} mm high", &[&width, &what, &sill, &head, &format!("{high:.0}")])
                }
                _ => tf("{} mm {}: nothing is drawn across it.", &[&width, &what]),
            };
            // And what stood in the way of giving it what was read, in the core's own words: one that was given it
            // says nothing more, since the sentence above is what it is made to.
            let said = match row["left"].as_str() {
                Some(why) => format!("{said} \u{2014} {why}"),
                None => said,
            };
            ui.label(RichText::new(said).color(colour));
        }
    });
}

/// The Walls tab: what to look for, Find, Build, and what the last Find said.
fn walls_tab(ui: &mut Ui, d: &mut TraceDialog, m: &Model, find: &mut bool, build: &mut bool) {
    let said = t("A survey draws a wall as its two faces, so a pair of lines a wall's thickness apart is a wall. What to look for:");
    ui.label(RichText::new(said).color(pal().muted));
    ui.add_space(6.0);
    let mut changed = false;
    // A drawing that holds a whole set of sheets side by side is read one sheet at a time: one of its parts by name,
    // which carries its level and its phase too, or what the plan's crop holds, which is an area and nothing more.
    // An elevation's lines pair as a wall's faces do, so a file of sheets read whole gives walls that are no walls.
    let (whole, crop) = (t("The whole drawing").to_string(), t("Inside the plan's crop").to_string());
    let named: Vec<String> = underlay_of(m, d.underlay).map(|u| u.parts.iter().map(|part| part.name.clone()).collect()).unwrap_or_default();
    let mut options = vec![whole.clone()];
    if d.crop.is_some() {
        options.push(crop.clone());
    }
    options.extend(named);
    let now = match &d.what {
        What::Whole => whole.clone(),
        What::Crop => crop.clone(),
        What::Part(name) => name.clone(),
    };
    ui.horizontal(|ui| {
        let label = ui.label(t("What to read"));
        if let Some(picked) = combo(ui, (d.underlay, "trace-what"), label.id, &now, &options) {
            d.what = match picked {
                p if p == whole => What::Whole,
                p if p == crop => What::Crop,
                p => What::Part(p),
            };
            changed = true;
        }
    });
    ui.add_space(6.0);
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
    // A survey draws the land around the building as well, and its lines pair as a wall's faces do: the boundary of
    // the plot, the kerb of the road. What tells them apart is that they meet no wall of the building.
    changed |= ui.checkbox(&mut d.how.alone, t("Take the lines that stand alone too")).changed();
    // The rooms are of the walls and not of the drawing, so asking for them changes nothing a Find would find.
    ui.checkbox(&mut d.rooms, t("Place the rooms too"));
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
    found_shown(ui, report, d.how.alone);
}

/// What the last Find found: how much, of what thickness and type, in which phase, what it left out, and the layers
/// that disagreed. `alone` is whether the lines that stand alone were asked for, because then none was left out.
fn found_shown(ui: &mut Ui, report: &Value, alone: bool) {
    let number = |key: &str| report[key].as_u64().unwrap_or(0).to_string();
    let metres = format!("{:.2}", report["length"].as_f64().unwrap_or(0.0) / 1000.0);
    ui.label(
        RichText::new(tf("{} walls, {} m, of {} lines long enough to be a wall's face.", &[&number("walls"), &metres, &number("faces")]))
            .strong(),
    );
    // A drawing in metres that says millimetres has nothing in it as thick as a wall. Where it measures on itself,
    // its own figures say what scale to link it with; where it does not, the longest line in it is the sign of it.
    if report["walls"].as_u64() == Some(0) {
        let said = match report["scale_says"].as_array().and_then(|said| said.first()).and_then(|scale| scale.as_f64()) {
            Some(scale) => tf(
                "Nothing in it is as thick as a wall, and the measurements written on it say it is drawn at a scale of {}: link it again with that.",
                &[&format!("{scale:.0}")],
            ),
            None => {
                let longest = format!("{:.2}", report["longest"].as_f64().unwrap_or(0.0));
                tf(
                    "The longest line in it is {} mm. A drawing in metres that says millimetres reads like this: link it again with a scale of 1000.",
                    &[&longest],
                )
            }
        };
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
    match report["alone"].as_u64().unwrap_or(0) {
        0 => {}
        left if alone => {
            let said = tf("{} of them meet no wall of the building.", &[&left.to_string()]);
            ui.label(RichText::new(said).color(pal().muted));
        }
        left => {
            let said = tf("{} lines met no wall of the building and were left out.", &[&left.to_string()]);
            ui.label(RichText::new(said).color(pal().muted));
        }
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
        // A plan cropped before the window was opened was cropped to what is to be traced.
        let crop = self.plan_key().and_then(|key| bimcraft_core::render::crop_region(&self.session.model, key));
        let what = match crop.is_some() {
            true => What::Crop,
            false => What::Whole,
        };
        let dialog = TraceDialog {
            underlay,
            tab: 2,
            how,
            rooms: false,
            what,
            crop,
            level: Some(here),
            kind: None,
            elevation: None,
            plan: None,
            floor: 0.0,
            openings: None,
            storey: None,
            roof: None,
            gables: None,
            top: None,
            found,
            holes,
            report: None,
            error: String::new(),
        };
        self.trace_dialog = Some(dialog);
    }

    pub(crate) fn trace_window(&mut self, ctx: &Context) {
        let Some(mut d) = self.trace_dialog.take() else { return };
        let model = Arc::clone(&self.session.model);
        // The plan may have been cropped, or uncropped, since the last frame; with no crop there is nothing to look
        // inside, so the window stops offering it.
        d.crop = self.plan_key().and_then(|key| bimcraft_core::render::crop_region(&model, key));
        d.level = self.plan_level().map(|level| model.guid(level));
        // A crop taken off, or a part done away with, while the window stands open leaves nothing to read inside it.
        let named: Vec<String> =
            underlay_of(&model, d.underlay).map(|u| u.parts.iter().map(|part| part.name.clone()).collect()).unwrap_or_default();
        match &d.what {
            What::Crop if d.crop.is_none() => d.what = What::Whole,
            What::Part(name) if !named.contains(name) => d.what = What::Whole,
            _ => {}
        }
        // The drawing may have been taken out of the project while the window stood open.
        let Some(name) = underlay_of(&model, d.underlay).map(|u| u.name.clone()) else { return };
        let (mut close, mut layers, mut find, mut build) = (false, None::<Value>, false, false);
        let (mut parts, mut read) = (None::<Value>, None::<Value>);
        let (mut storey, mut roofing) = (None::<Value>, None::<Value>);
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
                let tabs = [t("Layers").to_string(), t("Parts").to_string(), category("wall", true), t("Heights").to_string()];
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
                    1 => parts_tab(ui, &d, &model, &mut parts),
                    2 => walls_tab(ui, &mut d, &model, &mut find, &mut build),
                    _ => heights_tab(ui, &mut d, &model, &mut read, &mut storey, &mut roofing),
                }
                if !d.error.is_empty() {
                    ui.add_space(6.0);
                    ui.label(RichText::new(&d.error).color(pal().error));
                }
                ui.add_space(8.0);
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| close = ui.button(t("Close")).clicked());
            });
        let (reading, storeying, roofed) = (read.is_some(), storey.is_some(), roofing.is_some());
        let asked = match (layers, parts, read, storey, roofing, find || build) {
            (Some(p), ..) => Some(("cad_layers", p)),
            (_, Some(p), ..) => Some(("cad_parts", p)),
            (_, _, Some(p), ..) => Some(("cad_openings", p)),
            (_, _, _, Some(p), ..) => Some(("cad_storey", p)),
            (_, _, _, _, Some(p), _) => Some(("cad_roof", p)),
            (None, None, None, None, None, true) => Some(("trace_walls", d.params(find))),
            (None, None, None, None, None, false) => None,
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
                // What the drawing says of the sills and the heads: read off the elevation, and nothing built.
                Some(reply) if reading => d.openings = Some(reply),
                // And what it says of how high the storey stands, which is the walls' own height.
                Some(reply) if storeying => d.storey = Some(reply),
                // And what it says of the roof over them: the pitch no plan draws.
                Some(reply) if roofed => d.roof = Some(reply),
                Some(reply) => {
                    if build {
                        let walls = reply["walls"].as_u64().unwrap_or(0).to_string();
                        let text = match reply["rooms"].as_u64() {
                            Some(rooms) => tf("{} walls and {} rooms built from {}.", &[&walls, &rooms.to_string(), &name]),
                            None => tf("{} walls built from {}.", &[&walls, &name]),
                        };
                        self.status = crate::Status { text, error: false };
                        close = true;
                    }
                    d.stale();
                }
                None => {
                    d.error = std::mem::take(&mut self.status.text);
                    self.status = crate::Status::default();
                    (d.openings, d.storey, d.roof) = (None, None, None);
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
