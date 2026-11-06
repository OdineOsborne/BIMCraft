//! Trace Walls, driven as a user drives it: the drawing's layers put right in the window, what Find found said and
//! drawn over the plan, and walls built only on Build.

use super::*;

use egui_kittest::kittest::{By, NodeT};

/// A DXF of the lines given, each on the layer it names, in a folder of this call's own; its path.
fn drawing(lines: &[(String, [f64; 4])]) -> String {
    drawing_with(lines, "")
}

/// The same, with `extra` written into its ENTITIES after the lines: a dimension, say.
fn drawing_with(lines: &[(String, [f64; 4])], extra: &str) -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bimcraft-ui-trace-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rilievo.dxf");
    let mut text = String::from("0\nSECTION\n2\nENTITIES\n");
    for (layer, [x0, y0, x1, y1]) in lines {
        text.push_str(&format!("0\nLINE\n8\n{layer}\n10\n{x0}\n20\n{y0}\n11\n{x1}\n21\n{y1}\n"));
    }
    text.push_str(extra);
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
fn a_plan_cropped_to_one_sheet_traces_that_sheet_and_not_the_rest_of_the_file() {
    // What a survey saved as a set of sheets is: the plan, and a hundred metres away the elevation, whose lines
    // pair as a wall's faces do.
    let mut lines = one_room("MURI");
    lines.extend(room("PROSPETTO", [100_000.0, 0.0, 108_000.0, 6000.0], 300.0));
    let mut h = linked(&lines, None);
    shortcut(&mut h, "TW");
    // With the plan uncropped there is nothing to read inside, so it reads the lot.
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.what.clone()), Some(crate::trace::What::Whole));
    press(&mut h, "Find");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(8), "both sheets");
    press(&mut h, "Close window");
    // Cropped to the plan, it reads what is inside the crop and nothing else.
    let level = h.state().plan_level().expect("a plan");
    let region = json!({"view": level, "region": [[-1000.0, -1000.0], [9000.0, 7000.0]]});
    h.state_mut().session.execute("crop", &region).expect("it crops");
    h.run();
    shortcut(&mut h, "TW");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.what.clone()), Some(crate::trace::What::Crop), "a cropped plan is what to trace");
    press(&mut h, "Find");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(4), "the sheet in the crop and no other");
}

#[test]
fn a_part_is_added_from_the_crop_with_the_survey_own_name_for_it_and_traced_by_name() {
    // Two plans in one file, each captioned under it as a survey captions its sheets.
    let mut lines = one_room("MURI");
    lines.extend(room("MURI", [100_000.0, 0.0, 108_000.0, 6000.0], 300.0));
    let caption = "0\nTEXT\n8\nTESTO\n10\n2000\n20\n-900\n40\n300\n1\nPIANTA PIANO TERRA\n";
    let mut h = app("");
    h.state_mut().session.execute("underlay", &json!({"path": drawing_with(&lines, caption)})).expect("it comes in");
    let level = h.state().plan_level().expect("a plan");
    let region = json!({"view": level, "region": [[-2000.0, -2000.0], [9000.0, 7000.0]]});
    h.state_mut().session.execute("crop", &region).expect("it crops");
    h.run();
    shortcut(&mut h, "TW");
    press(&mut h, "Parts");
    assert!(says(&h, "It is made of no parts yet"), "a drawing is made of nothing until someone says so");
    press(&mut h, "Add the plan's crop as a part");
    // It is named by what the survey writes under it, and it is the plan of the level whose plan is on screen.
    let parts = h.state().session.model.elements.values().find_map(|e| match e {
        bimcraft_core::model::Element::Underlay(u) => Some(u.parts.clone()),
        _ => None,
    });
    let parts = parts.expect("the drawing");
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].name, "PIANTA PIANO TERRA");
    assert_eq!(parts[0].kind, bimcraft_core::model::PartKind::Plan);
    assert_eq!(parts[0].level, Some(h.state().session.model.guid(level)));
    // And the Walls tab offers it by name among what there is to read: the whole drawing, the crop, or a part.
    press(&mut h, "Walls");
    assert_eq!(combo_value(&h, "What to read"), "Inside the plan's crop", "a cropped plan is what to read");
    choose(&mut h, "What to read", "PIANTA PIANO TERRA");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.what.clone()), Some(crate::trace::What::Part("PIANTA PIANO TERRA".into())));
    press(&mut h, "Find");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(4), "the part and no other");
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
fn the_heights_tab_reads_the_sills_and_the_heads_off_the_elevation_and_builds_nothing() {
    let mut h = a_storey_with_an_elevation();
    shortcut(&mut h, "TW");
    press(&mut h, "Heights");
    // The elevation and the plan it is a view of, each the first of its kind among the parts.
    assert_eq!((combo_value(&h, "The elevation"), combo_value(&h, "The plan")), ("PROSPETTO SUD".to_string(), "PIANTA".to_string()));
    press(&mut h, "Read the Sills and Heads");
    assert!(says(&h, "2 openings on this face, 2 of them with a sill and a head."), "what it read is said");
    assert!(says(&h, "1500 mm Window: sill 70.70 m, head 71.90 m, 1200 mm high"), "the first window as the elevation draws it");
    assert!(says(&h, "1200 mm Window: sill 70.80 m, head 72.00 m, 1200 mm high"), "and the second");
    // It reads and reports: nothing is built, and the project is where it was.
    assert!(ids(&h, "wall").is_empty() && ids(&h, "window").is_empty(), "the sills and the heads build nothing");
    assert!(h.state().trace_dialog.as_ref().is_some_and(|d| d.openings.is_some()), "the window keeps what it read");
}

#[test]
fn the_heights_tab_gives_what_it_read_to_the_openings_the_walls_tab_built() {
    let mut h = a_storey_with_an_elevation();
    // The walls and their openings built from the plan, as the Walls tab builds them: a plan says nothing of a
    // height, so each window comes in at the size the project's own starter type is.
    h.state_mut().session.execute("trace_walls", &json!({"part": "PIANTA", "openings": true})).expect("it builds");
    h.run();
    assert_eq!(windows_of(&h), vec![(1500.0, 900.0); 2], "the starter size, until an elevation says otherwise");
    shortcut(&mut h, "TW");
    press(&mut h, "Heights");
    press(&mut h, "Read the Sills and Heads");
    // Nothing is given until the floor of the storey is said, which is what a sill is made so far above: the
    // ground, written 69.80 on this elevation.
    assert!(says(&h, "Giving these heights needs the storey's floor said, since a sill and a wall both stand so far above it."));
    assert_eq!(windows_of(&h), vec![(1500.0, 900.0); 2], "and read again it is still a reading");
    if let Some(d) = h.state_mut().trace_dialog.as_mut() {
        d.floor = 69_800.0;
    }
    h.run();
    press(&mut h, "Read the Sills and Heads");
    assert!(says(&h, "1500 mm Window: sill 900 mm above the floor, 1200 mm high"), "what it read, in the two numbers it is made to");
    press(&mut h, "Apply These Heights");
    assert!(says(&h, "2 openings were given the heights the elevation draws."), "what was given");
    assert!(says(&h, "2 types were made for them: Fixed 1500 x 1200, Fixed 1200 x 1200."), "at their own widths and the heights read");
    assert_eq!(windows_of(&h), vec![(1200.0, 900.0), (1200.0, 1000.0)], "each as the elevation draws it");
}

#[test]
fn the_heights_tab_gives_the_walls_of_the_storey_the_height_the_elevation_draws() {
    let mut h = a_storey_with_an_elevation();
    h.state_mut().session.execute("trace_walls", &json!({"part": "PIANTA", "openings": true})).expect("it builds");
    h.run();
    assert_eq!(walls_of(&h), vec![(3000.0, true); 4], "up to the level above, because a plan draws no height at all");
    shortcut(&mut h, "TW");
    press(&mut h, "Heights");
    if let Some(d) = h.state_mut().trace_dialog.as_mut() {
        d.floor = 69_800.0;
    }
    h.run();
    press(&mut h, "Read the Storey's Top");
    // The eaves, which is the one line this elevation draws the width of its facade above the floor.
    assert!(says(&h, "This storey stands 2800 mm high, its top drawn at 72.60 m."), "what it read");
    assert!(says(&h, "4 walls stand on this storey."), "and whose walls it is of");
    assert!(says(&h, "Level 2 stands 3000 mm above this floor, and is left where the project put it."), "said, and not moved");
    assert_eq!(walls_of(&h), vec![(3000.0, true); 4], "a reading changes nothing");
    press(&mut h, "Give the Walls This Height");
    assert!(says(&h, "4 walls of this storey were made that high."), "what was given");
    assert_eq!(walls_of(&h), vec![(2800.0, false); 4], "each unattached at the height the elevation draws");
}

#[test]
fn the_heights_tab_builds_the_roof_the_elevation_draws_over_those_walls() {
    let mut h = a_storey_with_an_elevation();
    h.state_mut().session.execute("trace_walls", &json!({"part": "PIANTA", "openings": true})).expect("it builds");
    h.run();
    shortcut(&mut h, "TW");
    press(&mut h, "Heights");
    if let Some(d) = h.state_mut().trace_dialog.as_mut() {
        d.floor = 69_800.0;
    }
    h.run();
    press(&mut h, "Read the Storey's Top");
    press(&mut h, "Give the Walls This Height");
    press(&mut h, "Read the Roof");
    // The pitch no plan draws, off the two rakes, and how far outside the facade their feet are drawn.
    assert!(says(&h, "This roof rises 26.57 degrees, its eaves 2800 mm above the floor."), "what it read");
    assert!(says(&h, "It reaches 500 mm past its walls, which is how far outside this facade its slopes are drawn."));
    // A gable end and a hipped end are drawn alike, and the apex the drawing draws is what settles which.
    assert!(says(&h, "Its ridge is drawn 5550 mm above the floor, and the roof is built with gable ends."));
    assert!(roofs_of(&h).is_empty(), "a reading builds nothing");
    press(&mut h, "Build This Roof");
    assert_eq!(roofs_of(&h), vec![(26.57, 50.0)], "at the pitch it draws, its base over the walls' faces");
}

#[test]
fn the_heights_tab_builds_the_slab_the_elevation_draws_under_this_storeys_floor() {
    let mut h = a_storey_with_an_elevation();
    h.state_mut().session.execute("trace_walls", &json!({"part": "PIANTA", "openings": true})).expect("it builds");
    h.run();
    shortcut(&mut h, "TW");
    press(&mut h, "Heights");
    if let Some(d) = h.state_mut().trace_dialog.as_mut() {
        d.floor = 69_800.0;
    }
    h.run();
    press(&mut h, "Read the Storey's Top");
    press(&mut h, "Give the Walls This Height");
    press(&mut h, "Read the Slab");
    // The one thing of a slab no plan draws: what lies between the floor of this storey and the line the
    // elevation draws under it.
    assert!(says(&h, "This slab is 260 mm thick, its top the floor of this storey."), "what it read");
    assert!(says(&h, "It would span 60.00 m², out to the faces of the walls that bear it."));
    assert!(slabs_of(&h).is_empty(), "a reading builds nothing");
    press(&mut h, "Build This Slab");
    assert_eq!(slabs_of(&h), vec![(260.0, 0.0)], "as thick as it is drawn, its top the floor it is of");
    assert!(says(&h, "A slab of 60.00 m² was built, the floor of Level 1."));
}

/// Every slab of the project: how thick the type it is of is, and how far its top stands above its level.
fn slabs_of(h: &Harness<BimApp>) -> Vec<(f64, f64)> {
    let m = &h.state().session.model;
    m.elements
        .values()
        .filter_map(|e| match e {
            bimcraft_core::model::Element::Floor(f) => Some((m.types.floor.get(&f.type_name)?.thickness, f.offset)),
            _ => None,
        })
        .collect()
}

/// Every roof of the project: the pitch it is built at and how far its base stands above its level.
fn roofs_of(h: &Harness<BimApp>) -> Vec<(f64, f64)> {
    h.state()
        .session
        .model
        .elements
        .values()
        .filter_map(|e| match e {
            bimcraft_core::model::Element::Roof(r) => Some((r.slope, r.offset)),
            _ => None,
        })
        .collect()
}

/// Every wall of the project: how high it is made, and whether it still goes up to the level above.
fn walls_of(h: &Harness<BimApp>) -> Vec<(f64, bool)> {
    let mut all: Vec<(f64, bool)> = h
        .state()
        .session
        .model
        .elements
        .values()
        .filter_map(|e| match e {
            bimcraft_core::model::Element::Wall(w) => Some((w.height, w.top_level.is_some())),
            _ => None,
        })
        .collect();
    all.sort_by(|a, b| a.0.total_cmp(&b.0));
    all
}

/// Every window of the project: how high the type it is of stands, and how high its own sill does.
fn windows_of(h: &Harness<BimApp>) -> Vec<(f64, f64)> {
    let m = &h.state().session.model;
    let mut all: Vec<(f64, f64)> = m
        .elements
        .values()
        .filter_map(|e| match e {
            bimcraft_core::model::Element::Window(o) => Some((m.types.window.get(&o.type_name)?.height, o.sill)),
            _ => None,
        })
        .collect();
    all.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    all
}

/// A survey of one storey, linked and in its parts, with the elevation stood against the plan: the plan of a room
/// with two windows in its south wall, and 20 m east of it the elevation of that side, which draws the ground, the
/// line the walls stop at, the underside of the slab this floor is, the two rakes of the roof, a rectangle around
/// each window, and the two heights it is drawn to.
fn a_storey_with_an_elevation() -> Harness<'static, BimApp> {
    let mut lines = room("MURI", [0.0, 0.0, 10_000.0, 6000.0], 300.0);
    for [x0, x1] in [[3000.0, 4500.0], [6000.0, 7200.0]] {
        lines.push(("MURI".to_string(), [x0, 0.0, x0, 300.0]));
        lines.push(("MURI".to_string(), [x1, 0.0, x1, 300.0]));
        lines.push(("MURI".to_string(), [x0, 150.0, x1, 150.0]));
    }
    lines.push(("PROSPETTO".to_string(), [20_000.0, 0.0, 30_000.0, 0.0]));
    lines.push(("PROSPETTO".to_string(), [20_000.0, 2800.0, 30_000.0, 2800.0]));
    // And the underside of the slab this floor is, 260 mm under it: the slab between the two lines.
    lines.push(("PROSPETTO".to_string(), [20_000.0, -260.0, 30_000.0, -260.0]));
    // And the two rakes of the roof over them, from 500 mm outside each end of the eaves to the apex over the
    // middle: one in two, which over the 11 m the roof spans with its overhang is that apex exactly.
    lines.push(("PROSPETTO".to_string(), [19_500.0, 2800.0, 25_000.0, 5550.0]));
    lines.push(("PROSPETTO".to_string(), [30_500.0, 2800.0, 25_000.0, 5550.0]));
    for [x0, x1, sill, head] in [[23_000.0, 24_500.0, 900.0, 2100.0], [26_000.0, 27_200.0, 1000.0, 2200.0]] {
        lines.push(("PROSPETTO".to_string(), [x0, sill, x1, sill]));
        lines.push(("PROSPETTO".to_string(), [x0, head, x1, head]));
        lines.push(("PROSPETTO".to_string(), [x0, sill, x0, head]));
        lines.push(("PROSPETTO".to_string(), [x1, sill, x1, head]));
    }
    let heights = "0\nTEXT\n8\nQUOTE\n10\n20500\n20\n0\n40\n250\n1\n69.80\n\
                   0\nTEXT\n8\nQUOTE\n10\n20500\n20\n3000\n40\n250\n1\n72.80\n";
    let mut h = app("");
    h.state_mut().session.execute("underlay", &json!({"path": drawing_with(&lines, heights)})).expect("it comes in");
    let parts = json!([
        {"name": "PIANTA", "kind": "plan", "region": [[-2000, -2000], [12_000, 8000]]},
        {"name": "PROSPETTO SUD", "kind": "elevation", "region": [[19_000, -2000], [31_000, 8000]]},
    ]);
    h.state_mut().session.execute("cad_parts", &json!({"parts": parts})).expect("it takes them");
    let anchor = json!({"part": "PROSPETTO SUD", "plan": [[0, 0], [10_000, 0]], "drawn": [[20_000, 0], [30_000, 0]]});
    h.state_mut().session.execute("cad_anchor", &anchor).expect("it stands");
    h.run();
    h
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
fn the_rooms_the_walls_shut_in_are_placed_with_them_when_the_window_asks_for_them() {
    let mut lines = one_room("MURI");
    // A partition across the room, from the one wall to the other: two rooms where there was one.
    lines.push(("MURI".to_string(), [300.0, 2950.0, 7700.0, 2950.0]));
    lines.push(("MURI".to_string(), [300.0, 3050.0, 7700.0, 3050.0]));
    let mut h = linked(&lines, None);
    shortcut(&mut h, "TW");
    pick(&mut h, Role::CheckBox, "Place the rooms too");
    press(&mut h, "Find");
    press(&mut h, "Build 5");
    assert_eq!((ids(&h, "wall").len(), ids(&h, "room").len()), (5, 2));
    assert!(says(&h, "5 walls and 2 rooms built from"), "{}", h.state().status.text);
}

#[test]
fn place_rooms_fills_the_plan_on_screen_and_leaves_the_rooms_already_in_it_alone() {
    let mut lines = one_room("MURI");
    lines.push(("MURI".to_string(), [300.0, 2950.0, 7700.0, 2950.0]));
    lines.push(("MURI".to_string(), [300.0, 3050.0, 7700.0, 3050.0]));
    let mut h = linked(&lines, None);
    shortcut(&mut h, "TW");
    press(&mut h, "Find");
    press(&mut h, "Build 5");
    assert!(ids(&h, "room").is_empty(), "the rooms were not asked for");
    press(&mut h, "Architecture");
    press(&mut h, "Place Rooms");
    assert_eq!(ids(&h, "room").len(), 2);
    assert!(says(&h, "2 rooms placed."), "{}", h.state().status.text);
    // Asked again, the areas are those rooms' and nothing is placed twice.
    press(&mut h, "Place Rooms");
    assert_eq!(ids(&h, "room").len(), 2);
    assert!(says(&h, "0 rooms placed."), "{}", h.state().status.text);
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
fn a_drawing_that_measures_on_itself_says_what_scale_to_link_it_with() {
    // The same room in metres, with the survey's own measurement written over its long side, printed in centimetres
    // as a survey prints it. Read as millimetres its figures come out a hundredth of one, which is no unit anybody
    // writes a length in, so the drawing itself says what it is drawn at and the window says it rather than the
    // longest line in it.
    let small: Vec<(String, [f64; 4])> =
        one_room("MURI").into_iter().map(|(l, [a, b, c, d])| (l, [a / 1000.0, b / 1000.0, c / 1000.0, d / 1000.0])).collect();
    let measure = "0\nDIMENSION\n8\nQUOTE\n70\n32\n10\n4\n20\n-1\n13\n0\n23\n0\n14\n8\n24\n0\n50\n0\n1\n800\n";
    let mut h = app("");
    h.state_mut().session.execute("underlay", &json!({"path": drawing_with(&small, measure)})).expect("it comes in");
    h.run();
    shortcut(&mut h, "TW");
    press(&mut h, "Find");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(0));
    assert!(says(&h, "drawn at a scale of 1000"), "its own measurements say what to link it with");
    assert!(!says(&h, "The longest line in it is"), "and the sign a drawing with no measurement goes by is not shown");
}

#[test]
fn link_cad_asks_at_which_scale_a_drawing_comes_in_before_it_comes_in_at_all() {
    // The same room in metres in a file that says millimetres, with the survey's own measurement over its long side.
    let small: Vec<(String, [f64; 4])> =
        one_room("MURI").into_iter().map(|(l, [a, b, c, d])| (l, [a / 1000.0, b / 1000.0, c / 1000.0, d / 1000.0])).collect();
    let measure = "0
DIMENSION
8
QUOTE
70
32
10
4
20
-1
13
0
23
0
14
8
24
0
50
0
1
800
";
    let path = drawing_with(&small, measure);
    let mut h = app("");
    let level = h.state().plan_level().map(serde_json::Value::from);
    // What Link CAD does once the file is picked: it reads it, and asks rather than bringing it in.
    let read = h.state_mut().session.execute("dxf", &json!({"path": path})).expect("it reads");
    let measured = read["measures"]["scale_says"][0].as_f64().expect("its measurements say a scale");
    let declares = read["scale"].as_f64().unwrap_or(1.0);
    h.state_mut().cad_scale = Some(crate::links::CadScale { path: path.clone(), level, declares, says: measured });
    h.run();
    assert!(says(&h, "drawn at a scale of 1000"), "the question says what the drawing measures");
    assert!(ids(&h, "underlay").is_empty(), "and nothing has come in while it is asked");
    // Answered, the drawing comes in at that scale, with one command, so one undo takes it back out.
    press(&mut h, "Bring it in at 1000");
    assert_eq!(ids(&h, "underlay").len(), 1, "{}", h.state().status.text);
    shortcut(&mut h, "TW");
    press(&mut h, "Find");
    assert_eq!(h.state().trace_dialog.as_ref().map(|d| d.found.len()), Some(4), "and it traces as a drawing in metres");
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
