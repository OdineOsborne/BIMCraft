//! Trace Walls, driven as a user drives it: the drawing's layers put right in the window, what Find found said and
//! drawn over the plan, and walls built only on Build.

use super::*;

use egui_kittest::kittest::{By, NodeT};

/// A DXF of the lines given, each on the layer it names, in a folder of this call's own; its path.
fn drawing(lines: &[(String, [f64; 4])]) -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bimcraft-ui-trace-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rilievo.dxf");
    let mut text = String::from("0\nSECTION\n2\nENTITIES\n");
    for (layer, [x0, y0, x1, y1]) in lines {
        text.push_str(&format!("0\nLINE\n8\n{layer}\n10\n{x0}\n20\n{y0}\n11\n{x1}\n21\n{y1}\n"));
    }
    text.push_str("0\nENDSEC\n0\nEOF\n");
    std::fs::write(&path, text).unwrap();
    path.to_string_lossy().into_owned()
}

/// The four sides of a rectangle, as a draughtsman draws them.
fn rect(layer: &str, [x0, y0, x1, y1]: [f64; 4]) -> Vec<(String, [f64; 4])> {
    [[x0, y0, x1, y0], [x1, y0, x1, y1], [x1, y1, x0, y1], [x0, y1, x0, y0]].into_iter().map(|l| (layer.to_string(), l)).collect()
}

/// A room drawn as a survey draws one: its walls' two faces, `t` apart, both on `layer`.
fn room(layer: &str, [x0, y0, x1, y1]: [f64; 4], t: f64) -> Vec<(String, [f64; 4])> {
    let mut out = rect(layer, [x0, y0, x1, y1]);
    out.extend(rect(layer, [x0 + t, y0 + t, x1 - t, y1 - t]));
    out
}

/// The app on a plan with `lines` under it, as Link CAD brings them in; `scale` is millimetres to a unit of the file.
fn linked(lines: &[(String, [f64; 4])], scale: Option<f64>) -> Harness<'static, BimApp> {
    let mut h = app("");
    let mut p = json!({"path": drawing(lines)});
    if let Some(k) = scale {
        p["scale"] = json!(k);
    }
    h.state_mut().session.execute("underlay", &p).expect("it comes in");
    h.run();
    h
}

/// Whether anything on screen says `part`.
fn says(h: &Harness<BimApp>, part: &str) -> bool {
    h.query_all(By::new()).any(|n| {
        let node = n.accesskit_node();
        node.label().or_else(|| node.value()).is_some_and(|s| s.contains(part))
    })
}

/// A room of four 300 walls, 8 m by 6 m.
fn one_room(layer: &str) -> Vec<(String, [f64; 4])> {
    room(layer, [0.0, 0.0, 8000.0, 6000.0], 300.0)
}

#[test]
fn tw_opens_trace_walls_on_the_drawing_under_the_plan_and_ks_lists_it() {
    let mut h = linked(&one_room("MURI"), None);
    shortcut(&mut h, "TW");
    assert!(h.state().trace_dialog.is_some(), "{}", h.state().status.text);
    // It says which drawing it reads, and offers Find.
    assert!(says(&h, "rilievo"), "the drawing is named");
    assert!(h.query_by_label("Find").is_some());
    assert!(crate::shortcuts().iter().any(|(letters, name)| *letters == "TW" && *name == "Trace Walls"));
}

#[test]
fn it_floats_so_the_plan_behind_it_can_still_be_worked_and_its_title_bar_closes_it() {
    let mut h = linked(&one_room("MURI"), None);
    shortcut(&mut h, "TW");
    press(&mut h, "Find");
    // A shortcut still reaches the plan: the window takes no keys, because what it found is drawn behind it and is
    // there to be looked at.
    let thin = h.state().thin;
    shortcut(&mut h, "TL");
    assert_ne!(h.state().thin, thin, "Thin Lines went through");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(4), "and the window is as it was");
    // Its title bar closes it, as its own Close button does.
    press(&mut h, "Close window");
    assert!(h.state().trace_dialog.is_none());
}

#[test]
fn with_no_drawing_under_the_plan_it_says_so_and_opens_nothing() {
    let mut h = app(BOX);
    shortcut(&mut h, "TW");
    assert!(h.state().trace_dialog.is_none());
    assert!(h.state().status.error && h.state().status.text.contains("Link CAD"), "{}", h.state().status.text);
}

#[test]
fn find_says_what_it_found_and_draws_it_and_nothing_is_built_until_build() {
    let mut h = linked(&one_room("MURI"), None);
    shortcut(&mut h, "TW");
    press(&mut h, "Find");
    // Four walls, their axes stretched to the corners: 2 × 7700 and 2 × 5700.
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(4));
    assert!(says(&h, "4 walls, 26.80 m, of 8 lines"), "what it found is said");
    assert!(says(&h, "300 mm: 4 \u{d7} "), "the thickness it measured, and the type the walls come in as");
    assert!(ids(&h, "wall").is_empty(), "Find builds nothing");
    // The walls it found are drawn over the plan, as thick as they measured: a band and an axis each.
    let level = h.state().plan_level().expect("a plan");
    let model = h.state_mut().view_model();
    let view = bimcraft_core::render::plan(&model, level, 800, 600).view;
    assert_eq!(h.state().traced_shapes(&view, egui::Pos2::ZERO, level).len(), 8);
    press(&mut h, "Build 4");
    assert_eq!(ids(&h, "wall").len(), 4);
    assert!(h.state().trace_dialog.is_none(), "the window closes once the walls are built");
    assert!(h.state().status.text.contains("4 walls built"), "{}", h.state().status.text);
    // Built, they are no longer a preview: nothing is drawn over the plan.
    assert!(h.state().traced_shapes(&view, egui::Pos2::ZERO, level).is_empty());
}

/// One wall 300 thick and 6 m long with a 900 doorway in the middle of it.
fn doorway() -> Vec<(String, [f64; 4])> {
    vec![
        ("MURI".to_string(), [0.0, 0.0, 2550.0, 0.0]),
        ("MURI".to_string(), [3450.0, 0.0, 6000.0, 0.0]),
        ("MURI".to_string(), [0.0, 300.0, 2550.0, 300.0]),
        ("MURI".to_string(), [3450.0, 300.0, 6000.0, 300.0]),
    ]
}

#[test]
fn the_openings_it_found_are_drawn_over_the_plan_and_come_in_as_doors_and_windows() {
    let mut h = linked(&doorway(), None);
    shortcut(&mut h, "TW");
    press(&mut h, "Find");
    assert!(says(&h, "1 Door and 0 Windows."), "what it found is said");
    assert!(says(&h, "900 mm Door: 1 \u{d7} Single 900 x 2100"), "and the type it would come in as");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.holes.len()), Some(1));
    // The wall is drawn over the plan as a band with its axis, and the opening over it.
    let level = h.state().plan_level().expect("a plan");
    let model = h.state_mut().view_model();
    let view = bimcraft_core::render::plan(&model, level, 800, 600).view;
    assert_eq!(h.state().traced_shapes(&view, egui::Pos2::ZERO, level).len(), 3);
    press(&mut h, "Build 1");
    assert_eq!((ids(&h, "wall").len(), ids(&h, "door").len()), (1, 1), "one wall with a door in it");
}

#[test]
fn with_the_openings_unticked_a_doorway_is_left_as_the_drawing_has_it() {
    let mut h = linked(&doorway(), None);
    shortcut(&mut h, "TW");
    pick(&mut h, Role::CheckBox, "Find the openings too");
    press(&mut h, "Find");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| (d.found.len(), d.holes.len())), Some((2, 0)));
    press(&mut h, "Build 2");
    assert_eq!((ids(&h, "wall").len(), ids(&h, "door").len()), (2, 0));
}

#[test]
fn the_lines_that_stand_alone_are_left_out_and_said_so_and_taken_when_they_are_asked_for() {
    // A survey draws the land around the building too: here the room, and beyond it the boundary of the plot, a pair
    // of lines as far apart as a wall is thick that meets nothing of the building.
    let mut lines = one_room("MURI");
    let boundary = [[-9000.0, -5000.0, 14000.0, -5000.0], [-9000.0, -4800.0, 14000.0, -4800.0]];
    lines.extend(boundary.into_iter().map(|l| ("CONFINE".to_string(), l)));
    let mut h = linked(&lines, None);
    shortcut(&mut h, "TW");
    press(&mut h, "Find");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(4), "the room's walls, not the boundary");
    assert!(says(&h, "1 lines met no wall of the building and were left out."), "and it says so");
    // Asked for them, it gives them: what it found is a reading, and the user's word is the last one.
    pick(&mut h, Role::CheckBox, "Take the lines that stand alone too");
    press(&mut h, "Find");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(5));
    press(&mut h, "Build 5");
    assert_eq!(ids(&h, "wall").len(), 5);
}

#[test]
fn a_layers_phase_is_put_right_in_the_window_and_the_walls_come_in_it() {
    // A layer whose name says nothing of the project's history: the walls on it would come in the first phase.
    let mut h = linked(&one_room("Quote"), None);
    shortcut(&mut h, "TW");
    press(&mut h, "Layers");
    assert_eq!(combo_value(&h, "Built in: Quote"), "<None>", "nothing was read of it");
    choose(&mut h, "Built in: Quote", "New Construction");
    assert_eq!(combo_value(&h, "Built in: Quote"), "New Construction");
    press_near(&mut h, "Walls", "Read the Layer Names Again");
    press(&mut h, "Find");
    assert!(says(&h, "New Construction: 4 walls"), "the phase the layer now names");
    press(&mut h, "Build 4");
    let built = ids(&h, "wall");
    assert_eq!(built.len(), 4);
    let m = &h.state().session.model;
    let phase = |id| m.elements.get(id).and_then(|e| bimcraft_core::phasing::value(m, e, bimcraft_core::phasing::CREATED));
    assert!(built.iter().all(|id| phase(*id) == Some("New Construction")), "every wall is in the phase the layer says");
}

#[test]
fn a_layer_turned_off_in_the_window_is_not_traced() {
    let mut lines = one_room("MURI");
    lines.extend(room("ARREDI", [12000.0, 0.0, 20000.0, 6000.0], 300.0));
    let mut h = linked(&lines, None);
    shortcut(&mut h, "TW");
    press(&mut h, "Find");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(8), "both rooms");
    press(&mut h, "Layers");
    pick(&mut h, Role::CheckBox, "ARREDI");
    press_near(&mut h, "Walls", "Read the Layer Names Again");
    // What was found was found with the layers as they were, so it is dropped and looked for again.
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(0));
    press(&mut h, "Find");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(4), "the furniture is not traced");
}

#[test]
fn a_drawing_in_metres_that_says_millimetres_finds_nothing_and_is_told_what_to_do() {
    // The same room, drawn in metres: every length a thousandth of what it should be.
    let small: Vec<(String, [f64; 4])> =
        one_room("MURI").into_iter().map(|(l, [a, b, c, d])| (l, [a / 1000.0, b / 1000.0, c / 1000.0, d / 1000.0])).collect();
    let mut h = linked(&small, None);
    shortcut(&mut h, "TW");
    press(&mut h, "Find");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(0));
    assert!(says(&h, "The longest line in it is 8.00 mm"), "the longest line says the drawing is in metres");
    // Linked again with the scale it is drawn at, the same drawing is the same room.
    let mut h = linked(&small, Some(1000.0));
    shortcut(&mut h, "TW");
    press(&mut h, "Find");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(4));
}

#[test]
fn a_refusal_is_said_inside_the_window_beside_the_button_that_was_pressed() {
    let mut h = linked(&one_room("MURI"), None);
    shortcut(&mut h, "TW");
    // The thinnest wall asked for is thicker than the thickest, so no wall can be between the two.
    h.state_mut().trace_dialog.as_mut().expect("the window").how.thin = 900.0;
    h.run();
    press(&mut h, "Find");
    let said = h.state().trace_dialog.as_ref().map(|d| d.error.clone()).unwrap_or_default();
    assert!(said.contains("trace_walls"), "the core's refusal, inside the window: {said:?}");
    assert!(says(&h, "trace_walls"), "and on screen");
    assert!(h.state().status.text.is_empty(), "not in the far corner of the view");
    assert!(ids(&h, "wall").is_empty());
}
