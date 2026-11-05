//! Link CAD through its command: what comes under a plan, what the project's file keeps of it, and what it refuses.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::geom::P2;
use crate::model::Element;

/// A DXF of two lines and a circle on two layers, in a folder of this call's own, and its path.
fn drawing(name: &str) -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bimcraft-underlay-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let text = "0\nSECTION\n2\nENTITIES\n\
                0\nLINE\n8\nMURI\n10\n0\n20\n0\n11\n5000\n21\n0\n\
                0\nLINE\n8\nMURI\n10\n5000\n20\n0\n11\n5000\n21\n4000\n\
                0\nCIRCLE\n8\nARREDI\n10\n1000\n20\n1000\n40\n250\n\
                0\nENDSEC\n0\nEOF\n";
    std::fs::write(&path, text).unwrap();
    path.to_string_lossy().into_owned()
}

fn run(s: &mut Session, id: &str, p: Value) -> Result<Value, String> {
    s.execute(id, &p)
}

/// How many lines a plan of the lowest level draws.
fn drawn(s: &Session) -> usize {
    let level = s.model.levels().first().map(|(id, _)| *id).expect("a level");
    crate::render::plan(&s.model, level, 800, 600).prims.len()
}

#[test]
fn a_drawing_comes_under_a_plan_and_the_project_builds_nothing_of_it() {
    let path = drawing("rilievo.dxf");
    let mut s = Session::new();
    let bare = drawn(&s);
    let out = run(&mut s, "underlay", json!({"path": path, "name": "Rilievo"})).expect("it comes in");
    assert_eq!((out["shapes"].as_u64(), out["name"].as_str()), (Some(3), Some("Rilievo")));
    assert_eq!(out["layers"], json!([{"name": "MURI", "shapes": 2}, {"name": "ARREDI", "shapes": 1}]));
    // It is an element of its own, which builds nothing: no wall, floor or room came of it.
    let underlays: Vec<&Element> = s.model.elements.values().filter(|e| matches!(e, Element::Underlay(_))).collect();
    assert_eq!(underlays.len(), 1);
    assert_eq!(s.model.elements.values().filter(|e| matches!(e, Element::Wall(_))).count(), 0);
    // The plan draws it: more lines than the empty plan had.
    assert!(drawn(&s) > bare, "the drawing is under the plan");
}

#[test]
fn a_part_of_a_drawing_is_named_by_what_the_survey_writes_under_it() {
    // A sheet set: two plans side by side, each captioned under it in lettering taller than the room names inside.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bimcraft-caption-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tavole.dxf");
    let words =
        |layer: &str, x: f64, y: f64, height: f64, text: &str| format!("0\nTEXT\n8\n{layer}\n10\n{x}\n20\n{y}\n40\n{height}\n1\n{text}\n");
    let text = format!(
        "0\nSECTION\n2\nENTITIES\n{}{}{}{}0\nENDSEC\n0\nEOF\n",
        words("TESTO", 1000.0, -800.0, 300.0, "PIANTA PIANO TERRA"),
        words("TESTO", 1500.0, 2000.0, 150.0, "SOGGIORNO"),
        words("TESTO", 1200.0, 1000.0, 150.0, "CAMERA"),
        words("TESTO", 21000.0, -800.0, 300.0, "PROSPETTO SUD"),
    );
    std::fs::write(&path, text).unwrap();
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path.to_string_lossy()})).expect("it comes in");
    let u = s
        .model
        .elements
        .values()
        .find_map(|e| match e {
            Element::Underlay(u) => Some(u.clone()),
            _ => None,
        })
        .expect("the drawing");
    let caption = |region| crate::underlay::caption(&s.model, &u, region);
    assert_eq!(caption([P2(-2000.0, -2000.0), P2(10000.0, 5000.0)]).as_deref(), Some("PIANTA PIANO TERRA"));
    assert_eq!(caption([P2(18000.0, -2000.0), P2(30000.0, 5000.0)]).as_deref(), Some("PROSPETTO SUD"));
    // A part with nothing written in it is named by nothing, and the room names alone are not a caption.
    assert_eq!(caption([P2(50000.0, 0.0), P2(60000.0, 1000.0)]), None);
    assert_eq!(caption([P2(1100.0, 500.0), P2(5000.0, 3000.0)]).as_deref(), Some("CAMERA"), "the lowest of two as large");
}

/// A DXF of the lines given, each on the layer it names, in a folder of this call's own; its path.
fn drawing_lines(lines: &[(String, [f64; 4])]) -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bimcraft-courses-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sezione.dxf");
    let mut text = String::from("0\nSECTION\n2\nENTITIES\n");
    for (layer, [x0, y0, x1, y1]) in lines {
        text.push_str(&format!("0\nLINE\n8\n{layer}\n10\n{x0}\n20\n{y0}\n11\n{x1}\n21\n{y1}\n"));
    }
    text.push_str("0\nENDSEC\n0\nEOF\n");
    std::fs::write(&path, text).unwrap();
    path.to_string_lossy().into_owned()
}

/// A drawing of `words`, each {layer, x, y, height, text}, under a plan; the session and the drawing it holds.
fn written(words: &[(&str, f64, f64, f64, &str)]) -> (Session, crate::model::Underlay) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bimcraft-written-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prospetto.dxf");
    let mut text = String::from("0\nSECTION\n2\nENTITIES\n");
    for (layer, x, y, height, said) in words {
        text.push_str(&format!("0\nTEXT\n8\n{layer}\n10\n{x}\n20\n{y}\n40\n{height}\n1\n{said}\n"));
    }
    text.push_str("0\nENDSEC\n0\nEOF\n");
    std::fs::write(&path, text).unwrap();
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path.to_string_lossy()})).expect("it comes in");
    let u = s
        .model
        .elements
        .values()
        .find_map(|e| match e {
            Element::Underlay(u) => Some(u.clone()),
            _ => None,
        })
        .expect("the drawing");
    (s, u)
}

#[test]
fn the_heights_an_elevation_writes_are_read_and_held_against_one_another() {
    // An elevation marking three heights in metres against the lines it measures, 150 and 400 mm apart as the
    // heights it says are: the ground at 69.70, a floor at 69.85, another at 70.10.
    let words = [
        ("QUOTE", 0.0, 0.0, 250.0, "69.70"),
        ("QUOTE", 2000.0, 150.0, 250.0, "69,85"),
        ("QUOTE", 4000.0, 400.0, 250.0, "70.10"),
        // What is no height: the section's name, a door's number, an area with its unit.
        ("TESTO", 1000.0, -800.0, 300.0, "SEZIONE A-A"),
        ("TESTO", 1500.0, 90.0, 150.0, "4"),
        ("TESTO", 2500.0, 90.0, 150.0, "20.68 M2"),
    ];
    let (s, u) = written(&words);
    let whole = [P2(-10_000.0, -10_000.0), P2(10_000.0, 10_000.0)];
    let read = crate::underlay::heights(&s.model, &u, whole);
    assert_eq!(read.marks.iter().map(|mark| mark.says).collect::<Vec<_>>(), vec![69.7, 69.85, 70.1]);
    assert_eq!(read.datum, Some(69_700.0), "the one number that turns a y into a height");
    assert!(read.worst < 1e-6, "they agree to the millimetre: {}", read.worst);
    // A mark 300 mm out of where the others put it is said to be, and by how much, rather than averaged away.
    let mut out = words;
    out[2] = ("QUOTE", 4000.0, 700.0, 250.0, "70.10");
    let (s, u) = written(&out);
    let read = crate::underlay::heights(&s.model, &u, whole);
    assert!((read.worst - 300.0).abs() < 1e-6, "{}", read.worst);
    assert!(read.marks.iter().filter(|mark| mark.out > crate::underlay::HEIGHTS_AGREE).count() == 1);
    // A drawing with nothing written in it says nothing about how high anything is.
    let (s, u) = written(&[("TESTO", 0.0, 0.0, 250.0, "PROSPETTO SUD")]);
    assert_eq!(crate::underlay::heights(&s.model, &u, whole), crate::underlay::Heights::default());
}

#[test]
fn the_heights_a_section_draws_lines_across_at_are_read_with_how_much_is_drawn_at_each() {
    // A section: the ground, a floor 2.7 m up and the one above it, each drawn right across; a window sill a metre
    // of line long; a tick too short to be a height of its own; and a roof's pitch, which is level nowhere.
    let mut lines: Vec<(String, [f64; 4])> = vec![
        ("SEZIONE".into(), [0.0, 0.0, 9000.0, 0.0]),
        ("SEZIONE".into(), [0.0, 2700.0, 9000.0, 2700.0]),
        // The floor above, drawn in two pieces as a drawing breaks a line, and 4 mm off level: one height, not two.
        ("SEZIONE".into(), [0.0, 5400.0, 4000.0, 5400.0]),
        ("SEZIONE".into(), [4000.0, 5404.0, 9000.0, 5404.0]),
        ("SEZIONE".into(), [2000.0, 1500.0, 3200.0, 1500.0]),
        ("SEZIONE".into(), [1000.0, 900.0, 1200.0, 900.0]),
        ("SEZIONE".into(), [0.0, 5400.0, 4500.0, 7000.0]),
    ];
    lines.push(("QUOTE".into(), [0.0, 0.0, 0.0, 0.0]));
    lines.pop();
    let path = drawing_lines(&lines);
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path})).expect("it comes in");
    let u = s
        .model
        .elements
        .values()
        .find_map(|e| match e {
            Element::Underlay(u) => Some(u.clone()),
            _ => None,
        })
        .expect("the drawing");
    let whole = [P2(-1000.0, -1000.0), P2(10_000.0, 10_000.0)];
    let found = crate::underlay::courses(&s.model, &u, whole, Some(69_700.0), 16);
    let at: Vec<(f64, f64)> = found.iter().map(|c| ((c.y / 10.0).round() * 10.0, (c.span / 100.0).round() * 100.0)).collect();
    assert_eq!(at, vec![(0.0, 9000.0), (1500.0, 1200.0), (2700.0, 9000.0), (5400.0, 9000.0)], "{found:?}");
    assert_eq!(found[2].height, Some(72_400.0), "the floor 2.7 m up, as the drawing writes heights");
    // The most drawn first, where only so many are wanted: the floors before the window sill.
    let few = crate::underlay::courses(&s.model, &u, whole, None, 3);
    assert_eq!(few.iter().map(|c| (c.y / 10.0).round() * 10.0).collect::<Vec<_>>(), vec![0.0, 2700.0, 5400.0]);
    assert!(few.iter().all(|c| c.height.is_none()), "no marks, no heights");
}

#[test]
fn an_elevation_is_stood_against_its_plan_by_two_points_whose_distances_must_agree() {
    let (mut s, _) = written(&[("TESTO", 0.0, 0.0, 250.0, "PROSPETTO SUD")]);
    let parts = json!([{"name": "PROSPETTO SUD", "kind": "elevation", "region": [[-50_000, -50_000], [50_000, 50_000]]}]);
    run(&mut s, "cad_parts", json!({"parts": parts})).expect("it takes it");
    // The face runs 8 m east in the plan, and the elevation draws those same 8 m starting at x 20000.
    let told = json!({"part": "PROSPETTO SUD", "plan": [[0, 0], [8000, 0]], "drawn": [[20_000, 0], [28_000, 0]]});
    let out = run(&mut s, "cad_anchor", told).expect("it stands");
    assert_eq!(out["anchor"]["along"], json!(0.0), "{out}");
    assert_eq!(out["anchor"]["out"], json!(0.0));
    // A point of the plan is drawn where its distance along the face puts it, and one off the face falls onto it.
    let ask = json!({"part": "PROSPETTO SUD", "at": [[4000, 0], [4000, 3000], [0, 0], [8000, 0]]});
    let out = run(&mut s, "cad_anchor", ask).expect("it says");
    assert_eq!(out["drawn_at"], json!([24_000.0, 24_000.0, 20_000.0, 28_000.0]), "{out}");
    // Two points that are not the same two points are refused, and both distances are said.
    let bad = json!({"part": "PROSPETTO SUD", "plan": [[0, 0], [8000, 0]], "drawn": [[20_000, 0], [25_000, 0]]});
    let e = run(&mut s, "cad_anchor", bad).expect_err("5 m is not 8 m");
    assert!(e.contains("8000") && e.contains("5000"), "{e}");
    // What it was stays what it was: a refused reading leaves the project alone.
    let out = run(&mut s, "cad_anchor", json!({"part": "PROSPETTO SUD"})).expect("it reads");
    assert_eq!(out["anchor"]["drawn"], json!([[20_000.0, 0.0], [28_000.0, 0.0]]));
    // A face running north, drawn right to left: the far end of the plan is the near end of the drawing.
    let told = json!({"part": "PROSPETTO SUD", "plan": [[0, 0], [0, 8000]], "drawn": [[28_000, 0], [20_000, 0]]});
    run(&mut s, "cad_anchor", told).expect("it stands");
    let out = run(&mut s, "cad_anchor", json!({"part": "PROSPETTO SUD", "at": [[0, 2000]]})).expect("it says");
    assert_eq!(out["anchor"]["along"], json!(90.0), "{out}");
    assert_eq!(out["drawn_at"], json!([26_000.0]));
}

#[test]
fn an_elevation_part_reports_its_heights_through_the_command() {
    let words = [
        ("QUOTE", 0.0, 0.0, 250.0, "69.70"),
        ("QUOTE", 2000.0, 150.0, 250.0, "69,85"),
        ("QUOTE", 4000.0, 400.0, 250.0, "70.10"),
        ("TESTO", 1000.0, -800.0, 300.0, "PROSPETTO SUD"),
    ];
    let (mut s, _) = written(&words);
    assert!(run(&mut s, "cad_heights", json!({})).is_err(), "it reads a part, and one has to be named");
    let whole = json!([[-10_000, -10_000], [10_000, 10_000]]);
    let parts = json!([{"name": "PROSPETTO SUD", "kind": "elevation", "region": whole}]);
    run(&mut s, "cad_parts", json!({"parts": parts})).expect("it takes it");
    let out = run(&mut s, "cad_heights", json!({"part": "PROSPETTO SUD"})).expect("it reads");
    assert_eq!(out["datum"], json!(69_700.0), "{out}");
    assert_eq!(out["agree"], json!(true));
    assert_eq!(out["kind"], json!("elevation"));
    assert_eq!(out["marks"].as_array().map(Vec::len), Some(3), "{out}");
    assert_eq!(out["marks"][0]["says"], json!(69.7));
    assert_eq!(out["marks"][2]["above"], json!(400.0), "how far the top floor stands above the lowest mark");
    let e = run(&mut s, "cad_heights", json!({"part": "SOFFITTA"})).expect_err("no such part");
    assert!(e.contains("PROSPETTO SUD"), "{e}");
}

/// A DXF of `lines` ({layer, [x0, y0, x1, y1]}) and `words` ({layer, x, y, height, text}) in a folder of this call's
/// own, under a plan; the session and the drawing it holds. A survey of a whole set of sheets is both at once: the
/// plans and the elevations are drawn, and the heights are written.
fn sheet(lines: &[(&str, [f64; 4])], words: &[(&str, f64, f64, f64, &str)]) -> (Session, crate::model::Underlay) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bimcraft-sheet-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tavole.dxf");
    let mut text = String::from("0\nSECTION\n2\nENTITIES\n");
    for (layer, [x0, y0, x1, y1]) in lines {
        text.push_str(&format!("0\nLINE\n8\n{layer}\n10\n{x0}\n20\n{y0}\n11\n{x1}\n21\n{y1}\n"));
    }
    for (layer, x, y, height, said) in words {
        text.push_str(&format!("0\nTEXT\n8\n{layer}\n10\n{x}\n20\n{y}\n40\n{height}\n1\n{said}\n"));
    }
    text.push_str("0\nENDSEC\n0\nEOF\n");
    std::fs::write(&path, text).unwrap();
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path.to_string_lossy()})).expect("it comes in");
    let u = s
        .model
        .elements
        .values()
        .find_map(|e| match e {
            Element::Underlay(u) => Some(u.clone()),
            _ => None,
        })
        .expect("the drawing");
    (s, u)
}

/// A survey of one storey: the plan of a house 10 x 6 m with walls 300 thick, two windows in its south wall and one
/// in its north, and beside it the elevation of the south side, which draws the ground, the eaves and a rectangle
/// around each of the two windows, with two heights written on it.
fn a_survey() -> (Session, crate::model::Underlay) {
    let mut lines: Vec<(&str, [f64; 4])> = vec![
        // The plan: the four walls, each drawn as its two faces.
        ("MURI", [0.0, 0.0, 10_000.0, 0.0]),
        ("MURI", [0.0, 300.0, 10_000.0, 300.0]),
        ("MURI", [0.0, 6000.0, 10_000.0, 6000.0]),
        ("MURI", [0.0, 5700.0, 10_000.0, 5700.0]),
        ("MURI", [0.0, 0.0, 0.0, 6000.0]),
        ("MURI", [300.0, 0.0, 300.0, 6000.0]),
        ("MURI", [10_000.0, 0.0, 10_000.0, 6000.0]),
        ("MURI", [9700.0, 0.0, 9700.0, 6000.0]),
        // Two windows in the south wall, each marked by its jambs with the glass between them.
        ("MURI", [3000.0, 0.0, 3000.0, 300.0]),
        ("MURI", [4500.0, 0.0, 4500.0, 300.0]),
        ("MURI", [3000.0, 150.0, 4500.0, 150.0]),
        ("MURI", [6000.0, 0.0, 6000.0, 300.0]),
        ("MURI", [7200.0, 0.0, 7200.0, 300.0]),
        ("MURI", [6000.0, 150.0, 7200.0, 150.0]),
        // And one in the north wall, which the south elevation shows nothing of.
        ("MURI", [2000.0, 5700.0, 2000.0, 6000.0]),
        ("MURI", [3200.0, 5700.0, 3200.0, 6000.0]),
        ("MURI", [2000.0, 5850.0, 3200.0, 5850.0]),
        // The elevation of the south side, 20 m east of the plan: the ground and the eaves run the width of it.
        ("PROSPETTO", [20_000.0, 0.0, 30_000.0, 0.0]),
        ("PROSPETTO", [20_000.0, 3000.0, 30_000.0, 3000.0]),
    ];
    // A rectangle around each window: its sill, its head and its two jambs.
    for [x0, x1, sill, head] in [[23_000.0, 24_500.0, 900.0, 2100.0], [26_000.0, 27_200.0, 1000.0, 2200.0]] {
        lines.push(("PROSPETTO", [x0, sill, x1, sill]));
        lines.push(("PROSPETTO", [x0, head, x1, head]));
        lines.push(("PROSPETTO", [x0, sill, x0, head]));
        lines.push(("PROSPETTO", [x1, sill, x1, head]));
    }
    let words = [
        ("TESTO", 1000.0, -1200.0, 300.0, "PIANTA PIANO TERRA"),
        ("TESTO", 21_000.0, -1200.0, 300.0, "PROSPETTO SUD"),
        // The heights it is drawn to: the ground at 69.80 and the eaves 3 m above it.
        ("QUOTE", 20_500.0, 0.0, 250.0, "69.80"),
        ("QUOTE", 20_500.0, 3000.0, 250.0, "72.80"),
    ];
    sheet(&lines, &words)
}

/// That survey with its two parts said and the elevation stood against the plan's south face.
fn a_survey_in_parts() -> Session {
    let (mut s, _) = a_survey();
    let parts = json!([
        {"name": "PIANTA PIANO TERRA", "kind": "plan", "region": [[-2000, -2000], [12_000, 8000]]},
        {"name": "PROSPETTO SUD", "kind": "elevation", "region": [[19_000, -2000], [31_000, 8000]]},
    ]);
    run(&mut s, "cad_parts", json!({"parts": parts})).expect("it takes them");
    // The south face runs from corner to corner of the plan, and the elevation draws those same 10 m from x 20000.
    let told = json!({"part": "PROSPETTO SUD", "plan": [[0, 0], [10_000, 0]], "drawn": [[20_000, 0], [30_000, 0]]});
    run(&mut s, "cad_anchor", told).expect("it stands");
    s
}

#[test]
fn the_lines_an_elevation_draws_of_an_opening_are_its_sill_and_its_head() {
    let (s, u) = a_survey();
    let region = [P2(19_000.0, -2000.0), P2(31_000.0, 8000.0)];
    let drawn = crate::underlay::drawn_lines(&s.model, &u, region, Some(69_800.0));
    let of = |from, to, slack, between| crate::underlay::opening(&drawn, from, to, slack, between);
    // The first window, 1500 wide: its sill and its head, and not the ground or the eaves, which are drawn the
    // width of the facade, nor the other window's, which is drawn 1200 wide.
    let found = of(23_000.0, 24_500.0, 500.0, None);
    let at: Vec<(f64, Option<f64>)> = found.iter().map(|c| (c.y, c.height)).collect();
    assert_eq!(at, vec![(900.0, Some(70_700.0)), (2100.0, Some(71_900.0))], "{drawn:?}");
    assert!(found.iter().all(|c| (c.span - 1500.0).abs() < 1.0), "each is drawn the width of the window: {found:?}");
    // A stretch of the facade where nothing of that width is drawn at all: nothing, and nothing invented.
    assert!(of(28_000.0, 29_500.0, 500.0, None).is_empty());
    // Where the plan puts the opening 400 mm off what the elevation draws, which is two anchor points picked a
    // little off the face: the same two lines. Farther off than the slack allows: none of them.
    assert_eq!(of(23_400.0, 24_900.0, 500.0, None).len(), 2, "within the slack it is the same opening");
    assert!(of(23_800.0, 25_300.0, 500.0, None).is_empty(), "800 mm off, and the slack is 500");
    // And read between two heights, an opening of another storey is none of this one's.
    assert!(of(23_000.0, 24_500.0, 500.0, Some((72_000.0, 75_000.0))).is_empty());
    assert_eq!(of(23_000.0, 24_500.0, 500.0, Some((69_800.0, 72_800.0))).len(), 2);
    // The facade's own lines are each one line the width of it, however many pieces they are drawn in.
    let facade: Vec<f64> = drawn.iter().filter(|c| c.width() > 5000.0).map(|c| c.width()).collect();
    assert_eq!(facade, vec![10_000.0, 10_000.0], "the ground and the eaves");
}

#[test]
fn the_openings_a_plan_finds_take_their_sill_and_head_from_the_elevation() {
    let mut s = a_survey_in_parts();
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA"});
    let out = run(&mut s, "cad_openings", ask).expect("it reads");
    assert_eq!(out["datum"], json!(69_800.0), "{out}");
    assert_eq!((out["found"].as_u64(), out["missed"].as_u64()), (Some(2), Some(0)), "{out}");
    // The window in the north wall is on no part of the face this elevation draws, so nothing of it is said.
    assert_eq!(out["elsewhere"], json!(1), "{out}");
    let holes = out["openings"].as_array().expect("the openings").clone();
    assert_eq!(holes.len(), 2, "{out}");
    // In the order the facade draws them, left to right, each with where the elevation draws it.
    assert_eq!(holes[0]["band"], json!([23_000.0, 24_500.0]), "{out}");
    assert_eq!(holes[0]["width"], json!(1500.0));
    assert_eq!(holes[0]["kind"], json!("window"));
    assert_eq!((holes[0]["sill"].as_f64(), holes[0]["head"].as_f64()), (Some(70_700.0), Some(71_900.0)));
    assert_eq!(holes[0]["high"], json!(1200.0));
    assert_eq!(holes[0]["sill_from"], json!("line"));
    // The elevation stands where the plan says it does, so its lines fall where the plan puts them, and the two
    // windows are drawn in the one plane of the one wall.
    assert_eq!((out["shift"].as_f64(), out["plane"].as_f64(), out["off_plane"].as_u64()), (Some(0.0), Some(150.0), Some(0)), "{out}");
    assert_eq!(holes[1]["band"], json!([26_000.0, 27_200.0]), "{out}");
    assert_eq!((holes[1]["sill"].as_f64(), holes[1]["head"].as_f64()), (Some(70_800.0), Some(72_000.0)));
    // How high the sill stands above the storey's floor, where the floor is said: the ground, here.
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800});
    let out = run(&mut s, "cad_openings", ask).expect("it reads");
    assert_eq!(out["openings"][0]["above"], json!(900.0), "{out}");
    assert_eq!(out["openings"][1]["above"], json!(1000.0));
}

#[test]
fn an_elevation_that_does_not_stand_against_its_plan_reads_no_openings_and_says_so() {
    let (mut s, _) = a_survey();
    let parts = json!([
        {"name": "PIANTA PIANO TERRA", "kind": "plan", "region": [[-2000, -2000], [12_000, 8000]]},
        {"name": "PROSPETTO SUD", "kind": "elevation", "region": [[19_000, -2000], [31_000, 8000]]},
    ]);
    run(&mut s, "cad_parts", json!({"parts": parts})).expect("it takes them");
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA"});
    let e = run(&mut s, "cad_openings", ask).expect_err("it does not stand against its plan yet");
    assert!(e.contains("cad_anchor"), "{e}");
    // A part it hasn't, and a part that is no plan: both say what the drawing is made of instead.
    let mut s = a_survey_in_parts();
    let ask = json!({"part": "PROSPETTO NORD", "plan": "PIANTA PIANO TERRA"});
    let e = run(&mut s, "cad_openings", ask).expect_err("no such part");
    assert!(e.contains("PROSPETTO SUD"), "{e}");
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PROSPETTO SUD"});
    let e = run(&mut s, "cad_openings", ask).expect_err("an elevation is no plan");
    assert!(e.contains("plan"), "{e}");
}

#[test]
fn the_project_keeps_the_path_and_never_the_drawing() {
    let path = drawing("rilievo.dxf");
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path.clone()})).expect("it comes in");
    let file = s.model.to_json().expect("it saves");
    assert!(file.contains("rilievo.dxf"), "the path is in the file: {file}");
    assert!(!file.contains("ARREDI"), "what was read of it is not: a drawing of any size costs a project nothing");
    // Read back, it draws nothing until its file is read again.
    let (back, _) = crate::model::Model::read(&file).expect("it opens");
    assert!(back.drawings.is_empty());
    let Some(Element::Underlay(u)) = back.elements.values().find(|e| matches!(e, Element::Underlay(_))) else { panic!() };
    assert!(crate::underlay::lines(&back, u).is_empty(), "a drawing that is not loaded draws nothing");
}

#[test]
fn it_stands_where_it_is_put_and_turns_with_the_project() {
    let path = drawing("rilievo.dxf");
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path, "at": [1000, 2000], "angle": 90})).expect("it comes in");
    let Some(Element::Underlay(u)) = s.model.elements.values().find(|e| matches!(e, Element::Underlay(_))) else { panic!() };
    let [(first, _), ..] = &crate::underlay::lines(&s.model, u)[..] else { panic!() };
    assert_eq!(first[0], P2(1000.0, 2000.0), "the drawing's origin stands where it was put");
    assert!((first[1].0 - 1000.0).abs() < 1e-9 && (first[1].1 - 7000.0).abs() < 1e-9, "turned a quarter: {:?}", first[1]);
}

#[test]
fn a_layer_it_is_told_to_leave_out_is_not_drawn() {
    let path = drawing("rilievo.dxf");
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path.clone(), "hidden": ["ARREDI"]})).expect("it comes in");
    let Some(Element::Underlay(u)) = s.model.elements.values().find(|e| matches!(e, Element::Underlay(_))) else { panic!() };
    assert_eq!(crate::underlay::lines(&s.model, u).len(), 2, "the two walls, not the circle");
}

#[test]
fn a_file_that_is_not_there_is_refused_and_nothing_is_left_behind() {
    let mut s = Session::new();
    let err = run(&mut s, "underlay", json!({"path": "C:/nothing/here/rilievo.dxf"})).expect_err("refused");
    assert!(err.contains("cannot read"), "{err}");
    assert!(s.model.elements.values().all(|e| !matches!(e, Element::Underlay(_))));
    assert!(s.model.drawings.is_empty());
}

#[test]
fn a_view_that_fits_the_model_fits_the_drawing_under_it() {
    let path = drawing("rilievo.dxf");
    let mut s = Session::new();
    let empty = s.model.model_extent();
    run(&mut s, "underlay", json!({"path": path, "at": [20000, 0]})).expect("it comes in");
    let with = s.model.model_extent();
    assert!(with[1].0 > empty[1].0 + 10000.0, "a survey off to one side is still in view: {empty:?} -> {with:?}");
}

/// Where a point picked at `at` lands on the lowest level's plan, with every snap on.
fn snapped(s: &Session, at: P2) -> crate::snap::Found {
    let level = s.model.levels().first().map(|(id, _)| *id).expect("a level");
    let reach = crate::snap::Reach { point: 300.0, line: 300.0 };
    crate::snap::snap(&s.model, level, at, reach, &[], false, &crate::snap::Snaps::default())
}

/// A project with the drawing of `drawing` under its plan, where it was read.
fn with_drawing() -> Session {
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": drawing("rilievo.dxf")})).expect("it comes in");
    s
}

#[test]
fn a_point_snaps_to_the_ends_of_the_drawing_and_to_the_middle_of_a_piece() {
    let s = with_drawing();
    // The drawing is two lines, (0,0)-(5000,0) and (5000,0)-(5000,4000), and a circle at (1000,1000).
    let found = snapped(&s, P2(4900.0, 60.0));
    assert_eq!((found.at, found.kind), (P2(5000.0, 0.0), crate::snap::Kind::Endpoint), "the corner they share");
    let found = snapped(&s, P2(2500.0, 90.0));
    assert_eq!((found.at, found.kind), (P2(2500.0, 0.0), crate::snap::Kind::Midpoint), "the middle of the long one");
    let found = snapped(&s, P2(1050.0, 1050.0));
    assert_eq!((found.at, found.kind), (P2(1000.0, 1000.0), crate::snap::Kind::Centre), "the circle's centre");
}

#[test]
fn a_point_snaps_to_where_two_of_its_lines_cross() {
    let path = drawing("incrocio.dxf");
    // Two lines that cross at (1000, 1000) without ending there.
    std::fs::write(
        &path,
        "0\nSECTION\n2\nENTITIES\n         0\nLINE\n8\nMURI\n10\n0\n20\n1000\n11\n3000\n21\n1000\n         0\nLINE\n8\nMURI\n10\n1000\n20\n0\n11\n1000\n21\n3000\n         0\nENDSEC\n0\nEOF\n",
    )
    .unwrap();
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path})).expect("it comes in");
    let found = snapped(&s, P2(1080.0, 1080.0));
    assert_eq!((found.at, found.kind), (P2(1000.0, 1000.0), crate::snap::Kind::Intersection));
}

#[test]
fn nothing_snaps_to_a_layer_left_out_or_to_a_drawing_on_another_level() {
    let path = drawing("rilievo.dxf");
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path.clone(), "hidden": ["ARREDI"]})).expect("it comes in");
    // The circle's centre is on the layer left out: the pick finds nothing there and rounds to 10 mm.
    let found = snapped(&s, P2(1050.0, 1050.0));
    assert_eq!(found.kind, crate::snap::Kind::Free, "{found:?}");

    // The same drawing on the level above is not under this plan.
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path, "level": "Level 2"})).expect("it comes in");
    assert_eq!(snapped(&s, P2(4900.0, 60.0)).kind, crate::snap::Kind::Free);
}

#[test]
fn what_a_survey_writes_is_drawn_under_the_plan_and_snapped_to_by_nothing() {
    let path = drawing("scritte.dxf");
    let text = std::fs::read_to_string(&path).expect("the drawing");
    let with = text.replace("0\nENDSEC\n0\nEOF\n", "0\nTEXT\n8\nTESTI\n1\nSoggiorno\n10\n1000\n20\n2000\n40\n250\n0\nENDSEC\n0\nEOF\n");
    std::fs::write(&path, with).expect("the drawing again");
    let mut s = Session::new();
    let bare = drawn(&s);
    run(&mut s, "underlay", json!({"path": path, "name": "Rilievo"})).expect("it comes in");
    assert!(drawn(&s) > bare + 3, "the word is drawn with the drawing: {} over {bare}", drawn(&s));
    // It is lettering and no line of the building: nothing snaps to it, and the walls are not traced from it.
    assert_eq!(snapped(&s, P2(1010.0, 2010.0)).kind, crate::snap::Kind::Free);
    let out = run(&mut s, "trace_walls", json!({"preview": true})).expect("it traces");
    assert_eq!(out["faces"], json!(2), "the drawing's own two lines, and nothing of the word: {out}");
}

#[test]
fn it_snaps_where_the_drawing_stands_not_where_the_file_drew_it() {
    let path = drawing("rilievo.dxf");
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path, "at": [10000, 0]})).expect("it comes in");
    assert_eq!(snapped(&s, P2(14980.0, 40.0)).at, P2(15000.0, 0.0), "the corner, moved with the drawing");
    assert_eq!(snapped(&s, P2(4980.0, 40.0)).kind, crate::snap::Kind::Free, "and nothing where it used to be");
}

/// What a snap costs on a survey of 50,000 lines, which is a real one: it runs at every move of the mouse, so it is
/// measured rather than assumed. Ignored, as the other measurements are:
/// `cargo test -p bimcraft-core underlay_snap_speed --release -- --ignored --nocapture`.
#[test]
#[ignore]
fn underlay_snap_speed() {
    let mut text = String::from("0\nSECTION\n2\nENTITIES\n");
    for i in 0..50_000 {
        let (x, y) = ((i % 250) * 400, (i / 250) * 400);
        text.push_str(&format!("0\nLINE\n8\nMURI\n10\n{x}\n20\n{y}\n11\n{}\n21\n{y}\n", x + 300));
    }
    text.push_str("0\nENDSEC\n0\nEOF\n");
    let path = drawing("grande.dxf");
    std::fs::write(&path, text).unwrap();
    let mut s = Session::new();
    let made = std::time::Instant::now();
    let out = run(&mut s, "underlay", json!({"path": path})).expect("it comes in");
    let read = made.elapsed();
    assert_eq!(out["shapes"], 50_000);

    let started = std::time::Instant::now();
    let rounds = 200;
    for i in 0..rounds {
        let (x, y) = ((i % 250) as f64 * 400.0 + 20.0, (i / 250) as f64 * 400.0 + 20.0);
        let found = snapped(&s, P2(x, y));
        assert_eq!(found.kind, crate::snap::Kind::Endpoint);
    }
    let each = started.elapsed() / rounds;
    eprintln!("50.000 lines: read in {read:?}, {each:?} a snap");
    assert!(each < std::time::Duration::from_millis(16), "a snap takes {each:?}, which a moving cursor would feel");
}

/// A drawing of two layers, one of what is there and one of what is to be built.
fn phased_drawing(name: &str) -> String {
    let path = drawing(name);
    std::fs::write(
        &path,
        "0\nSECTION\n2\nENTITIES\n\
         0\nLINE\n8\nStato di fatto\n10\n0\n20\n0\n11\n5000\n21\n0\n\
         0\nLINE\n8\nProgetto\n10\n0\n20\n2000\n11\n5000\n21\n2000\n\
         0\nLINE\n8\nDemolizioni\n10\n0\n20\n4000\n11\n5000\n21\n4000\n\
         0\nLINE\n8\nQuote\n10\n0\n20\n6000\n11\n5000\n21\n6000\n\
         0\nENDSEC\n0\nEOF\n",
    )
    .unwrap();
    path
}

#[test]
fn a_layers_own_name_says_what_it_is_of_the_projects_history() {
    use crate::phasing::Status;
    for (name, says) in [
        ("Stato di fatto", Some(Status::Existing)),
        ("STATO DI FATTO", Some(Status::Existing)),
        ("rilievo esistente", Some(Status::Existing)),
        ("A-EXISTING-WALL", Some(Status::Existing)),
        ("Demolizioni", Some(Status::Demolished)),
        ("muri da demolire", Some(Status::Demolished)),
        // The colours an Italian drawing is read with: yellow comes down, red goes up.
        ("giallo", Some(Status::Demolished)),
        ("rosso", Some(Status::New)),
        ("Progetto", Some(Status::New)),
        ("nuove opere", Some(Status::New)),
        // What a demolition layer says first is that it is a demolition, whatever else it names.
        ("Stato di fatto - da demolire", Some(Status::Demolished)),
        // And what says nothing is nothing: a layer of dimensions, a layer of furniture, a surveyor's own.
        ("Quote", None),
        // `red` lies inside `arredi`, and `new` inside `newel`: a short word has to be a whole word.
        ("ARREDI", None),
        ("newel post", None),
        ("Credito", None),
        ("Ciano", None),
        ("LINEEGPS-DA-CODICI", None),
        ("0", None),
    ] {
        assert_eq!(crate::underlay::says(name), says, "{name}");
    }
}

#[test]
fn a_drawing_comes_in_with_the_phases_its_layers_name_and_the_caller_may_say_otherwise() {
    let path = phased_drawing("fasi.dxf");
    let mut s = Session::new();
    let out = run(&mut s, "underlay", json!({"path": path.clone()})).expect("it comes in");
    assert_eq!(
        out["phases"],
        json!([
            {"layer": "Stato di fatto", "created": "Existing"},
            {"layer": "Progetto", "created": "New Construction"},
            {"layer": "Demolizioni", "created": "Existing", "demolished": "New Construction"},
        ]),
        "the layer of dimensions says nothing, so nothing is said of it"
    );
    // What was read is the caller's to change: a layer named after a colour, meant otherwise.
    let mut s = Session::new();
    let said = json!([{"layer": "Quote", "created": "New Construction"}]);
    let out = run(&mut s, "underlay", json!({"path": path, "phases": said.clone()})).expect("it comes in");
    assert_eq!(out["phases"], said, "what the caller says stands, and nothing is read");
}

#[test]
fn a_plan_draws_a_layer_only_in_the_phases_it_stands_in() {
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": phased_drawing("fasi.dxf")})).expect("it comes in");
    let lines = |s: &Session| drawn(s);
    run(&mut s, "graphics", json!({"view": "Level 1", "phase": "New Construction"})).expect("the phase is set");
    let building = lines(&s);
    run(&mut s, "graphics", json!({"view": "Level 1", "phase": "Existing"})).expect("the phase is set");
    let existing = lines(&s);
    // What is to be built is not there yet, so the plan of what is there draws one line fewer.
    assert_eq!(existing + 1, building, "existing {existing}, building {building}");
}

#[test]
fn a_phase_the_project_hasnt_is_refused_and_nothing_is_left_behind() {
    let mut s = Session::new();
    let said = json!([{"layer": "Progetto", "created": "Seconda fase"}]);
    let err = run(&mut s, "underlay", json!({"path": phased_drawing("fasi.dxf"), "phases": said})).expect_err("refused");
    assert!(err.contains("Seconda fase"), "{err}");
    assert!(s.model.elements.values().all(|e| !matches!(e, Element::Underlay(_))));
}

/// The one drawing under a plan.
fn under(s: &Session) -> &crate::model::Underlay {
    let found = s.model.elements.values().find_map(|e| match e {
        Element::Underlay(u) => Some(u),
        _ => None,
    });
    found.expect("a drawing under a plan")
}

fn phase(layer: &str, created: Option<&str>, demolished: Option<&str>) -> crate::model::LayerPhase {
    crate::model::LayerPhase { layer: layer.into(), created: created.map(str::to_string), demolished: demolished.map(str::to_string) }
}

#[test]
fn what_a_layer_is_of_the_project_is_corrected_and_what_it_is_not_told_is_left_as_it_was() {
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": phased_drawing("fasi.dxf")})).expect("it comes in");
    // The layer named after dimensions said nothing of itself and is a layer of what is there; `Progetto` is built
    // and comes down again in the same phase, which is said without saying what it is built in twice.
    let out = run(
        &mut s,
        "cad_layers",
        json!({"layers": [{"layer": "Quote", "created": "Existing"}, {"layer": "Progetto", "demolished": "New Construction"}]}),
    )
    .expect("it is told");
    assert_eq!(out["changed"], json!(["Quote", "Progetto"]));
    assert_eq!(
        under(&s).phases,
        vec![
            phase("Stato di fatto", Some("Existing"), None),
            phase("Progetto", Some("New Construction"), Some("New Construction")),
            phase("Demolizioni", Some("Existing"), Some("New Construction")),
            phase("Quote", Some("Existing"), None),
        ],
        "each layer keeps its place, and a phase it was not told about is left alone"
    );
    // Every layer the drawing has comes back, with how much is drawn on it: what a dialog shows.
    assert_eq!(out["layers"][3], json!({"layer": "Quote", "shapes": 1, "drawn": true, "created": "Existing", "demolished": null}));
    // Told nothing of either phase, a layer is left out again: it stands in every phase, as one never named does,
    // and leaves the other face of a wall to say which phase the wall is in.
    run(&mut s, "cad_layers", json!({"layers": [{"layer": "Quote", "created": null, "demolished": null}]})).expect("unsaid");
    assert!(!under(&s).phases.iter().any(|l| l.layer == "Quote"), "{:?}", under(&s).phases);
    // Told again what is so already, it leaves the project alone and so leaves no undo step behind: the one undo
    // below takes back the command above it.
    run(&mut s, "cad_layers", json!({"layers": [{"layer": "Quote", "created": null, "demolished": null}]})).expect("again");
    assert_eq!(under(&s).phases.len(), 3);
    // One command, one undo step.
    run(&mut s, "undo", json!({})).expect("it goes back");
    assert_eq!(under(&s).phases.len(), 4);
}

#[test]
fn a_layer_turned_off_is_not_drawn_and_turned_on_again_it_is() {
    let path = drawing("rilievo.dxf");
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path})).expect("it comes in");
    let lines = |s: &Session| crate::underlay::lines(&s.model, under(s)).len();
    assert_eq!(lines(&s), 3, "two walls and a circle");
    let out = run(&mut s, "cad_layers", json!({"layers": [{"layer": "ARREDI", "drawn": false}]})).expect("off it goes");
    assert_eq!(out["layers"][1], json!({"layer": "ARREDI", "shapes": 1, "drawn": false, "created": null, "demolished": null}));
    assert_eq!(lines(&s), 2, "the furniture is not drawn, so it is not traced either");
    run(&mut s, "cad_layers", json!({"layers": [{"layer": "ARREDI", "drawn": true}]})).expect("and back on");
    assert_eq!(lines(&s), 3);
    assert!(under(&s).hidden.is_empty());
}

#[test]
fn it_refuses_a_layer_the_drawing_hasnt_a_phase_the_project_hasnt_and_a_field_it_doesnt_know() {
    let mut s = Session::new();
    let err = run(&mut s, "cad_layers", json!({"layers": []})).expect_err("no drawing");
    assert!(err.contains("Link CAD"), "{err}");
    run(&mut s, "underlay", json!({"path": phased_drawing("fasi.dxf")})).expect("it comes in");
    for (params, said) in [
        (json!({"layers": [{"layer": "Muri", "drawn": false}]}), "Muri"),
        (json!({"layers": [{"layer": "Quote", "created": "Terza fase"}]}), "Terza fase"),
        (json!({"layers": [{"layer": "Quote", "colour": "red"}]}), "colour"),
        (json!({"layers": [{"layer": "Quote", "drawn": "no"}]}), "true or false"),
        (json!({"layers": "Quote"}), "layers"),
        (json!({}), "layers"),
    ] {
        let err = run(&mut s, "cad_layers", params.clone()).expect_err("refused");
        assert!(err.contains(said), "{params}: {err}");
    }
    // Nothing of what was refused stuck.
    assert_eq!(
        under(&s).phases,
        vec![
            phase("Stato di fatto", Some("Existing"), None),
            phase("Progetto", Some("New Construction"), None),
            phase("Demolizioni", Some("Existing"), Some("New Construction")),
        ]
    );
    assert!(under(&s).hidden.is_empty());
}
