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

/// Every window of the project, as it was built: how high its type is and how high its own sill stands, in one
/// order whatever order they were made in.
fn windows_of(s: &Session) -> Vec<(f64, f64)> {
    let m = &s.model;
    let mut all: Vec<(f64, f64)> = m
        .elements
        .values()
        .filter_map(|e| match e {
            Element::Window(o) => Some((m.types.window.get(&o.type_name)?.height, o.sill)),
            _ => None,
        })
        .collect();
    all.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    all
}

#[test]
fn the_openings_a_plan_built_are_given_the_heights_the_elevation_draws() {
    let mut s = a_survey_in_parts();
    // The plan's walls and its openings, built as a plan gives them: a plan says nothing of a height, so every
    // window comes in 1500 high on a 900 sill, which is the size the project's own starter type is.
    let built = run(&mut s, "trace_walls", json!({"part": "PIANTA PIANO TERRA", "openings": true})).expect("it builds");
    assert_eq!(built["windows"], json!(3), "{built}");
    assert_eq!(windows_of(&s), vec![(1500.0, 900.0); 3], "{built}");
    // A reading changes nothing of it, however much it says.
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800});
    run(&mut s, "cad_openings", ask.clone()).expect("it reads");
    assert_eq!(windows_of(&s), vec![(1500.0, 900.0); 3], "a reading builds nothing and changes nothing");
    // Asked for, it gives the two the elevation draws what it draws of them: 1200 high each, their sills 900 and
    // 1000 above the floor. The window in the north wall, which this elevation draws nothing of, is left as it was.
    let mut told = ask.clone();
    told["apply"] = json!(true);
    let out = run(&mut s, "cad_openings", told.clone()).expect("it gives them");
    assert_eq!((out["applied"].as_u64(), out["left"].as_u64()), (Some(2), Some(0)), "{out}");
    assert_eq!(windows_of(&s), vec![(1200.0, 900.0), (1200.0, 1000.0), (1500.0, 900.0)], "{out}");
    // Each in a type of its own width and the height that was read, made because the project has no such type.
    assert_eq!(out["made"], json!(["Fixed 1500 x 1200", "Fixed 1200 x 1200"]), "{out}");
    assert_eq!(out["openings"][0]["type"], json!("Fixed 1500 x 1200"), "{out}");
    assert!(out["openings"][0]["applied"].as_u64().is_some(), "{out}");
    // Given again, the same, and nothing made twice: those types are the project's own now.
    let out = run(&mut s, "cad_openings", told).expect("again");
    assert_eq!((out["applied"].as_u64(), out["made"].clone()), (Some(2), Value::Null), "{out}");
    assert_eq!(windows_of(&s), vec![(1200.0, 900.0), (1200.0, 1000.0), (1500.0, 900.0)]);
    // Giving them it is one undo step, and a reading is none: one step back is the openings as the plan built
    // them, and the second giving, which had nothing left to give, was no step at all.
    run(&mut s, "undo", json!({})).expect("it goes back");
    assert_eq!(windows_of(&s), vec![(1500.0, 900.0); 3], "one step takes back the lot");
    // And nothing is given without the floor, which is what a sill stands above.
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "apply": true});
    let e = run(&mut s, "cad_openings", ask).expect_err("it needs the floor");
    assert!(e.contains("`floor`"), "{e}");
}

/// The same survey, with the elevation drawing what a facade draws across its whole width: the ground, a string
/// course 1200 up, the eaves at 2800, the ridge at 4500, and the slope of the roof, which runs a part of it only.
/// The windows of `a_survey` are drawn as they are there, so the walls built have openings in them.
fn a_storey() -> Session {
    let (s, _) = a_survey();
    let path = s
        .model
        .elements
        .values()
        .find_map(|e| match e {
            Element::Underlay(u) => Some(u.path.clone()),
            _ => None,
        })
        .expect("the drawing");
    // The elevation of a_survey draws the ground and a line at 3000; this one draws a facade's own courses.
    let more = "0\nLINE\n8\nPROSPETTO\n10\n20000\n20\n1200\n11\n30000\n21\n1200\n\
                0\nLINE\n8\nPROSPETTO\n10\n20000\n20\n2800\n11\n30000\n21\n2800\n\
                0\nLINE\n8\nPROSPETTO\n10\n20000\n20\n4500\n11\n30000\n21\n4500\n\
                0\nLINE\n8\nPROSPETTO\n10\n21000\n20\n3500\n11\n24000\n21\n3500\n";
    // Its own line at 3000 goes, so that what this elevation draws across the facade is the four lines above.
    let eaves = "0\nLINE\n8\nPROSPETTO\n10\n20000\n20\n3000\n11\n30000\n21\n3000\n";
    let text = std::fs::read_to_string(&path)
        .expect("the file")
        .replace(eaves, "")
        .replace("0\nENDSEC\n0\nEOF\n", &format!("{more}0\nENDSEC\n0\nEOF\n"));
    std::fs::write(&path, text).expect("it is written");
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path})).expect("it comes in");
    let parts = json!([
        {"name": "PIANTA PIANO TERRA", "kind": "plan", "region": [[-2000, -2000], [12_000, 8000]]},
        {"name": "PROSPETTO SUD", "kind": "elevation", "region": [[19_000, -2000], [31_000, 8000]]},
    ]);
    run(&mut s, "cad_parts", json!({"parts": parts})).expect("it takes them");
    let told = json!({"part": "PROSPETTO SUD", "plan": [[0, 0], [10_000, 0]], "drawn": [[20_000, 0], [30_000, 0]]});
    run(&mut s, "cad_anchor", told).expect("it stands");
    run(&mut s, "trace_walls", json!({"part": "PIANTA PIANO TERRA", "openings": true})).expect("it builds");
    s
}

/// Every wall of the project: how high it is made and whether it still goes up to the level above.
fn walls_of(s: &Session) -> Vec<(f64, bool)> {
    let mut all: Vec<(f64, bool)> = s
        .model
        .elements
        .values()
        .filter_map(|e| match e {
            Element::Wall(w) => Some((w.height, w.top_level.is_some())),
            _ => None,
        })
        .collect();
    all.sort_by(|a, b| a.0.total_cmp(&b.0));
    all
}

#[test]
fn the_storey_an_elevation_draws_is_how_high_the_walls_of_it_are_made() {
    let mut s = a_storey();
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800});
    let out = run(&mut s, "cad_storey", ask.clone()).expect("it reads");
    // The facade is the 10 m the elevation stands against its plan by, and the lines that run it are the ground,
    // the course, the eaves and the ridge: the roof's slope runs 3 m of it and is none of them.
    assert_eq!((out["across"].clone(), out["across_from"].clone()), (json!(10_000.0), json!("anchor")), "{out}");
    let drawn: Vec<Value> = out["courses"].as_array().expect("the courses").iter().map(|c| c["above"].clone()).collect();
    assert_eq!(drawn, vec![json!(0.0), json!(1200.0), json!(2800.0), json!(4500.0)], "{out}");
    // The top is the highest of them within a storey of the floor: the ridge at 4500 is of the roof, not of this.
    assert_eq!((out["top"].clone(), out["high"].clone()), (json!(72_600.0), json!(2800.0)), "{out}");
    assert_eq!(out["walls"], json!(4), "{out}");
    // It read and changed nothing; the walls still go up to the level above, where the project put it.
    assert_eq!(walls_of(&s), vec![(3000.0, true); 4], "{out}");
    assert_eq!(out["level_above"], json!({"name": "Level 2", "high": 3000.0, "agrees": false}), "{out}");
    // Asked for, every wall of the storey is made that high and stands unattached at it.
    let mut told = ask.clone();
    told["apply"] = json!(true);
    let out = run(&mut s, "cad_storey", told.clone()).expect("it gives them");
    assert_eq!(out["applied"], json!(4), "{out}");
    assert_eq!(walls_of(&s), vec![(2800.0, false); 4], "{out}");
    // One undo step takes the storey back, and giving it again changes nothing at all.
    run(&mut s, "undo", json!({})).expect("it goes back");
    assert_eq!(walls_of(&s), vec![(3000.0, true); 4]);
    run(&mut s, "cad_storey", told.clone()).expect("again");
    run(&mut s, "cad_storey", told).expect("and again");
    run(&mut s, "undo", json!({})).expect("one step");
    assert_eq!(walls_of(&s), vec![(3000.0, true); 4], "the second giving was no step of its own");
}

#[test]
fn a_wall_an_opening_would_stand_through_is_left_as_high_as_it_was() {
    let mut s = a_storey();
    // The string course 1200 above the floor, said outright: the walls with the windows in them reach 2400, so
    // they are left as high as they were and the two blind ones are made to it.
    let told = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800, "top": 71_000, "apply": true});
    let out = run(&mut s, "cad_storey", told).expect("it gives them");
    assert_eq!((out["high"].clone(), out["applied"].clone(), out["too_short"].clone()), (json!(1200.0), json!(2), json!(2)), "{out}");
    assert_eq!(walls_of(&s), vec![(1200.0, false), (1200.0, false), (3000.0, true), (3000.0, true)], "{out}");
    // A top under the floor is no top at all, and an elevation that draws no course within the storey says so
    // instead of making the walls up out of nothing.
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800, "top": 69_000, "apply": true});
    let e = run(&mut s, "cad_storey", ask).expect_err("under the floor");
    assert!(e.contains("not above its floor"), "{e}");
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 60_000, "apply": true});
    let e = run(&mut s, "cad_storey", ask).expect_err("nothing within a storey of that floor");
    assert!(e.contains("cad_heights"), "{e}");
    // And a plan draws no heights, whatever is asked of it.
    let ask = json!({"part": "PIANTA PIANO TERRA", "plan": "PIANTA PIANO TERRA"});
    let e = run(&mut s, "cad_storey", ask).expect_err("a plan");
    assert!(e.contains("is a plan"), "{e}");
}

/// The same survey, with the elevation drawing a gable end: the eaves across the whole facade at 2800, and the two
/// rakes of the roof rising from 500 mm outside each end of it to the apex over the middle of it, at 5550. They are
/// drawn one in two, 26.57 degrees, which over the 11 m the roof spans with its overhang is that apex exactly.
fn a_roof() -> Session {
    let (s, _) = a_survey();
    let path = s
        .model
        .elements
        .values()
        .find_map(|e| match e {
            Element::Underlay(u) => Some(u.path.clone()),
            _ => None,
        })
        .expect("the drawing");
    let more = "0\nLINE\n8\nPROSPETTO\n10\n20000\n20\n2800\n11\n30000\n21\n2800\n\
                0\nLINE\n8\nPROSPETTO\n10\n19500\n20\n2800\n11\n25000\n21\n5550\n\
                0\nLINE\n8\nPROSPETTO\n10\n30500\n20\n2800\n11\n25000\n21\n5550\n";
    // Its own line at 3000 goes: the top of this facade is the eaves the rakes stand on.
    let eaves = "0\nLINE\n8\nPROSPETTO\n10\n20000\n20\n3000\n11\n30000\n21\n3000\n";
    let text = std::fs::read_to_string(&path)
        .expect("the file")
        .replace(eaves, "")
        .replace("0\nENDSEC\n0\nEOF\n", &format!("{more}0\nENDSEC\n0\nEOF\n"));
    std::fs::write(&path, text).expect("it is written");
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path})).expect("it comes in");
    let parts = json!([
        {"name": "PIANTA PIANO TERRA", "kind": "plan", "region": [[-2000, -2000], [12_000, 8000]]},
        {"name": "PROSPETTO SUD", "kind": "elevation", "region": [[19_000, -2000], [31_000, 8000]]},
    ]);
    run(&mut s, "cad_parts", json!({"parts": parts})).expect("it takes them");
    let told = json!({"part": "PROSPETTO SUD", "plan": [[0, 0], [10_000, 0]], "drawn": [[20_000, 0], [30_000, 0]]});
    run(&mut s, "cad_anchor", told).expect("it stands");
    run(&mut s, "trace_walls", json!({"part": "PIANTA PIANO TERRA", "openings": true})).expect("it builds");
    // The walls as high as that elevation draws them, which is what the roof then stands on.
    let storey = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800, "apply": true});
    run(&mut s, "cad_storey", storey).expect("it gives them the storey");
    s
}

/// Every roof of the project: the pitch it is built at, how far its base stands above its level, how far it reaches
/// past the walls, and which of its edges are left upright.
fn roofs_of(s: &Session) -> Vec<(f64, f64, f64, Vec<usize>)> {
    s.model
        .elements
        .values()
        .filter_map(|e| match e {
            Element::Roof(r) => Some((r.slope, r.offset, r.overhang, r.gables.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn the_roof_an_elevation_draws_is_built_at_the_pitch_it_draws() {
    let mut s = a_roof();
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800});
    let out = run(&mut s, "cad_roof", ask.clone()).expect("it reads");
    // The two rakes and nothing else: the eaves and the courses are drawn level, and the jambs of the windows
    // upright, so neither is a slope of anything.
    let rises: Vec<Value> = out["slopes"].as_array().expect("the slopes").iter().map(|s| s["rise"].clone()).collect();
    assert_eq!(rises, vec![json!(26.57), json!(26.57)], "{out}");
    assert_eq!(out["slope"], json!(26.57), "{out}");
    // The eaves is the foot of them, the ridge the head, and both are said as they stand above the floor.
    assert_eq!((out["eaves"].clone(), out["ridge"].clone()), (json!(2800.0), json!(5550.0)), "{out}");
    // Their feet stand 500 mm outside the ends of the facade, which is how far the roof reaches past its walls.
    assert_eq!((out["beyond"].clone(), out["overhang"].clone()), (json!(500.0), json!(500.0)), "{out}");
    assert_eq!(out["overhang_from"], json!("slopes"), "{out}");
    // The slopes meet over the middle of the facade, so the facade is a gable end: the edges of the roof that run
    // with it are left upright, and its ridge comes out where the drawing draws the apex.
    assert_eq!((out["apex"].clone(), out["gables"].clone()), (json!(true), json!([0, 2])), "{out}");
    assert_eq!(out["gables_from"], json!("ridge"), "{out}");
    assert_eq!((out["made_ridge"].clone(), out["ridge_agrees"].clone()), (json!(5550.0), json!(true)), "{out}");
    assert!(roofs_of(&s).is_empty(), "it read and changed nothing: {out}");
    // Asked for, one roof stands on the level above, its base 200 mm under that level, where the eaves was read.
    let mut told = ask.clone();
    told["apply"] = json!(true);
    let out = run(&mut s, "cad_roof", told.clone()).expect("it builds");
    assert_eq!(out["level"], json!("Level 2"), "{out}");
    // Its base stands over the faces of the walls, where its underside crosses them: 250 mm above the eaves,
    // which is how far the overhang falls below it on the way out.
    assert_eq!((out["base"].clone(), out["offset"].clone()), (json!(3050.0), json!(50.0)), "{out}");
    assert_eq!(roofs_of(&s), vec![(26.57, 50.0, 500.0, vec![0, 2])], "{out}");
    // One undo takes it away, and building it again builds no second roof over the first.
    run(&mut s, "undo", json!({})).expect("it goes back");
    assert!(roofs_of(&s).is_empty());
    run(&mut s, "cad_roof", told.clone()).expect("again");
    let out = run(&mut s, "cad_roof", told).expect("and again");
    assert_eq!(roofs_of(&s).len(), 1, "{out}");
    run(&mut s, "undo", json!({})).expect("one step");
    assert!(roofs_of(&s).is_empty(), "the second building was no step of its own");
}

#[test]
fn hips_are_built_where_the_drawing_draws_no_apex_and_the_ridge_is_held_against_it() {
    let mut s = a_roof();
    // Told to leave no edge upright, the slopes rise from all four and meet 1 m lower than the apex the drawing
    // draws: what it says is reported against what was built rather than quietly taken for it.
    let told = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800, "gables": [], "apply": true});
    let out = run(&mut s, "cad_roof", told).expect("it builds");
    assert_eq!(out["gables"], json!([]), "{out}");
    assert_eq!((out["made_ridge"].clone(), out["ridge_agrees"].clone()), (json!(4550.0), json!(false)), "{out}");
    assert_eq!(roofs_of(&s), vec![(26.57, 50.0, 500.0, vec![])], "{out}");
}

#[test]
fn an_elevation_that_draws_no_slope_above_its_walls_builds_no_roof_and_says_so() {
    // The same survey's elevation draws its courses and its ridge level, and the one line it draws across a part
    // of the facade is level too: there is no pitch in it to build a roof at.
    let mut s = a_storey();
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800, "apply": true});
    let e = run(&mut s, "cad_roof", ask).expect_err("nothing slopes");
    assert!(e.contains("`slope`"), "{e}");
    // Told the pitch outright, the eaves is the top of the walls that are built, and the roof stands on them.
    let told = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800, "slope": 30, "apply": true});
    let out = run(&mut s, "cad_roof", told).expect("it builds");
    assert_eq!((out["slope"].clone(), out["eaves_from"].clone()), (json!(30.0), json!("walls")), "{out}");
    assert_eq!(roofs_of(&s), vec![(30.0, 0.0, 500.0, vec![])], "{out}");
    // A plan draws no roof either, and apply still needs the floor the eaves stands above.
    let ask = json!({"part": "PIANTA PIANO TERRA", "plan": "PIANTA PIANO TERRA"});
    let e = run(&mut s, "cad_roof", ask).expect_err("a plan");
    assert!(e.contains("is a plan"), "{e}");
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "slope": 30, "apply": true});
    let e = run(&mut s, "cad_roof", ask).expect_err("it needs the floor");
    assert!(e.contains("`floor`"), "{e}");
}

/// The same survey, with the elevation drawing the two storeys of a facade: the ground at 0, a string course 1200
/// up, the top of the ground floor's walls at 2800, the floor of the storey over them at 3080, and the eaves of
/// that storey at 5800. The 280 mm between the third line and the fourth is the slab of the upper storey, and no
/// plan draws any of it.
fn a_slab() -> Session {
    let (s, _) = a_survey();
    let path = s
        .model
        .elements
        .values()
        .find_map(|e| match e {
            Element::Underlay(u) => Some(u.path.clone()),
            _ => None,
        })
        .expect("the drawing");
    let more = "0\nLINE\n8\nPROSPETTO\n10\n20000\n20\n1200\n11\n30000\n21\n1200\n\
                0\nLINE\n8\nPROSPETTO\n10\n20000\n20\n2800\n11\n30000\n21\n2800\n\
                0\nLINE\n8\nPROSPETTO\n10\n20000\n20\n3080\n11\n30000\n21\n3080\n\
                0\nLINE\n8\nPROSPETTO\n10\n20000\n20\n5800\n11\n30000\n21\n5800\n";
    // Its own line at 3000 goes, so that what stands over the top of the walls is the floor of the storey above.
    let eaves = "0\nLINE\n8\nPROSPETTO\n10\n20000\n20\n3000\n11\n30000\n21\n3000\n";
    let text = std::fs::read_to_string(&path)
        .expect("the file")
        .replace(eaves, "")
        .replace("0\nENDSEC\n0\nEOF\n", &format!("{more}0\nENDSEC\n0\nEOF\n"));
    std::fs::write(&path, text).expect("it is written");
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path})).expect("it comes in");
    let parts = json!([
        {"name": "PIANTA PIANO TERRA", "kind": "plan", "region": [[-2000, -2000], [12_000, 8000]]},
        {"name": "PROSPETTO SUD", "kind": "elevation", "region": [[19_000, -2000], [31_000, 8000]]},
    ]);
    run(&mut s, "cad_parts", json!({"parts": parts})).expect("it takes them");
    let told = json!({"part": "PROSPETTO SUD", "plan": [[0, 0], [10_000, 0]], "drawn": [[20_000, 0], [30_000, 0]]});
    run(&mut s, "cad_anchor", told).expect("it stands");
    run(&mut s, "trace_walls", json!({"part": "PIANTA PIANO TERRA", "openings": true})).expect("it builds");
    // The walls as high as that elevation draws them, which is what the slab over them then rests on.
    let storey = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800, "apply": true});
    run(&mut s, "cad_storey", storey).expect("it gives them the storey");
    s
}

/// Every slab of the project: the level it is the floor of, how thick its type is, and the square metres it
/// covers.
fn slabs_of(s: &Session) -> Vec<(String, f64, f64)> {
    s.model
        .elements
        .values()
        .filter_map(|e| match e {
            Element::Floor(f) => Some((
                s.model.level(f.level).map(|l| l.name.clone()).unwrap_or_default(),
                s.model.types.floor.get(&f.type_name).map_or(0.0, |t| t.thickness),
                crate::commands::m2(crate::geom::area(&f.boundary)),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn the_slab_an_elevation_draws_is_as_thick_as_the_two_lines_it_stands_between() {
    let mut s = a_slab();
    // The slab of the storey over the one traced: its own floor is drawn at 72.88, and what the elevation draws
    // 280 mm under that is the line the walls of the storey below stop at.
    let ask = json!({"part": "PROSPETTO SUD", "level": "Level 2", "floor": 72_880});
    let out = run(&mut s, "cad_floor", ask.clone()).expect("it reads");
    // The lines it draws the width of the facade, said as they stand against that floor: the one under it that a
    // slab could be thick enough to reach, and the rest of the facade, which no slab of this floor is.
    let drawn: Vec<(Value, Value)> =
        out["courses"].as_array().expect("the courses").iter().map(|c| (c["above"].clone(), c["could"].clone())).collect();
    let said = vec![
        (json!(-3080.0), json!(false)),
        (json!(-1880.0), json!(false)),
        (json!(-280.0), json!(true)),
        (json!(0.0), json!(false)),
        (json!(2720.0), json!(false)),
    ];
    assert_eq!(drawn, said, "{out}");
    assert_eq!((out["under"].clone(), out["under_from"].clone()), (json!(-280.0), json!("courses")), "{out}");
    assert_eq!((out["thickness"].clone(), out["thickness_from"].clone()), (json!(280.0), json!("courses")), "{out}");
    // Nothing is traced on that level yet, so it spans the walls under it, which are what bear it: the 10 by 6 m
    // the plan draws, out to their faces, in a type of its own thickness.
    assert_eq!((out["spans_from"].clone(), out["area_m2"].clone()), (json!("storey below"), json!(60.0)), "{out}");
    assert_eq!((out["type"].clone(), out["made"].clone()), (json!("Generic - 280mm"), json!(true)), "{out}");
    // Those walls stop 80 mm above where this underside comes out, because the project puts Level 2 lower than
    // the drawing draws its floor: said, and not quietly taken for agreement.
    assert_eq!(out["below"], json!({"name": "Level 1", "gap": 80.0, "agrees": false}), "{out}");
    assert!(slabs_of(&s).is_empty(), "it read and changed nothing: {out}");
    // Asked for, one slab is the floor of that level, as thick as the drawing draws it.
    let mut told = ask.clone();
    told["apply"] = json!(true);
    let out = run(&mut s, "cad_floor", told.clone()).expect("it builds");
    assert!(out["slab"].as_u64().is_some(), "{out}");
    assert_eq!(slabs_of(&s), vec![("Level 2".to_string(), 280.0, 60.0)], "{out}");
    // One undo step takes it back, and given again from the same reading it builds no second slab.
    run(&mut s, "undo", json!({})).expect("it goes back");
    assert!(slabs_of(&s).is_empty());
    run(&mut s, "cad_floor", told.clone()).expect("again");
    run(&mut s, "cad_floor", told).expect("and again");
    assert_eq!(slabs_of(&s).len(), 1, "the second giving built no slab of its own");
}

#[test]
fn an_elevation_that_draws_no_line_under_the_floor_builds_no_slab_and_says_so() {
    // The ground floor of the same survey: its elevation draws the floor itself and then the ground, 1583 mm
    // under it, which is the fall of the terrain and no slab's underside.
    let mut s = a_slab();
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800, "apply": true});
    let e = run(&mut s, "cad_floor", ask).expect_err("nothing under the floor");
    assert!(e.contains("`thickness`"), "{e}");
    // Told how thick it is, its underside stands that far under the floor and the slab is built: this storey's
    // own walls stand on it, so they are what it spans.
    let told = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800, "thickness": 300, "apply": true});
    let out = run(&mut s, "cad_floor", told).expect("it builds");
    assert_eq!((out["under"].clone(), out["under_from"].clone()), (json!(-300.0), json!("told")), "{out}");
    assert_eq!((out["spans_from"].clone(), out["below"].clone()), (json!("storey"), json!(null)), "{out}");
    assert_eq!(slabs_of(&s), vec![("Level 1".to_string(), 300.0, 60.0)], "{out}");
    // An underside said above the floor is no underside, a plan draws no thickness, and apply still needs the
    // floor the slab hangs under.
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800, "under": 300});
    let e = run(&mut s, "cad_floor", ask).expect_err("above the floor");
    assert!(e.contains("negative number"), "{e}");
    let ask = json!({"part": "PIANTA PIANO TERRA", "plan": "PIANTA PIANO TERRA"});
    let e = run(&mut s, "cad_floor", ask).expect_err("a plan");
    assert!(e.contains("is a plan"), "{e}");
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "thickness": 300, "apply": true});
    let e = run(&mut s, "cad_floor", ask).expect_err("it needs the floor");
    assert!(e.contains("`floor`"), "{e}");
}

#[test]
fn a_door_is_made_as_high_as_its_head_stands_above_the_floor_it_opens_onto() {
    // A plan with a doorway 1 m wide in its south wall, drawn as a gap in both faces of it, and 20 m east the
    // elevation of that side: the ground, the eaves, and the door with its head 2300 above the floor and its
    // threshold laid a step under it.
    let mut lines: Vec<(&str, [f64; 4])> = vec![
        ("MURI", [0.0, 0.0, 4000.0, 0.0]),
        ("MURI", [5000.0, 0.0, 10_000.0, 0.0]),
        ("MURI", [0.0, 300.0, 4000.0, 300.0]),
        ("MURI", [5000.0, 300.0, 10_000.0, 300.0]),
        ("MURI", [0.0, 6000.0, 10_000.0, 6000.0]),
        ("MURI", [0.0, 5700.0, 10_000.0, 5700.0]),
        ("MURI", [0.0, 0.0, 0.0, 6000.0]),
        ("MURI", [300.0, 0.0, 300.0, 6000.0]),
        ("MURI", [10_000.0, 0.0, 10_000.0, 6000.0]),
        ("MURI", [9700.0, 0.0, 9700.0, 6000.0]),
        ("PROSPETTO", [20_000.0, 0.0, 30_000.0, 0.0]),
        ("PROSPETTO", [20_000.0, 3000.0, 30_000.0, 3000.0]),
    ];
    let [x0, x1, sill, head] = [24_000.0, 25_000.0, -50.0, 2300.0];
    lines.push(("PROSPETTO", [x0, sill, x1, sill]));
    lines.push(("PROSPETTO", [x0, head, x1, head]));
    lines.push(("PROSPETTO", [x0, sill, x0, head]));
    lines.push(("PROSPETTO", [x1, sill, x1, head]));
    let words = [
        ("TESTO", 1000.0, -1200.0, 300.0, "PIANTA PIANO TERRA"),
        ("TESTO", 21_000.0, -1200.0, 300.0, "PROSPETTO SUD"),
        ("QUOTE", 20_500.0, 0.0, 250.0, "69.80"),
        ("QUOTE", 20_500.0, 3000.0, 250.0, "72.80"),
    ];
    let (mut s, _) = sheet(&lines, &words);
    let parts = json!([
        {"name": "PIANTA PIANO TERRA", "kind": "plan", "region": [[-2000, -2000], [12_000, 8000]]},
        {"name": "PROSPETTO SUD", "kind": "elevation", "region": [[19_000, -2000], [31_000, 8000]]},
    ]);
    run(&mut s, "cad_parts", json!({"parts": parts})).expect("it takes them");
    let told = json!({"part": "PROSPETTO SUD", "plan": [[0, 0], [10_000, 0]], "drawn": [[20_000, 0], [30_000, 0]]});
    run(&mut s, "cad_anchor", told).expect("it stands");
    let built = run(&mut s, "trace_walls", json!({"part": "PIANTA PIANO TERRA", "openings": true})).expect("it builds");
    assert_eq!((built["doors"].as_u64(), built["windows"].as_u64()), (Some(1), Some(0)), "{built}");
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800, "apply": true});
    let out = run(&mut s, "cad_openings", ask).expect("it reads");
    let door = &out["openings"][0];
    // The elevation draws 2350 between the two lines it draws across the door, the lower of them being the
    // threshold; what is made is 2300 high, from the floor the door opens onto to the head the elevation draws.
    assert_eq!((door["kind"].clone(), door["high"].clone()), (json!("door"), json!(2350.0)), "{out}");
    assert_eq!((door["made"].clone(), door["made_sill"].clone()), (json!(2300.0), json!(0.0)), "{out}");
    assert_eq!(out["made"], json!(["Single 900 x 2300"]), "{out}");
    let doors: Vec<(f64, f64)> = s
        .model
        .elements
        .values()
        .filter_map(|e| match e {
            Element::Door(o) => Some((s.model.types.door.get(&o.type_name)?.height, o.sill)),
            _ => None,
        })
        .collect();
    assert_eq!(doors, vec![(2300.0, 0.0)], "the door stands on its floor and reaches the head that was drawn");
}

#[test]
fn an_opening_the_elevation_says_nothing_of_is_left_as_it_was_and_says_why() {
    let mut s = a_survey_in_parts();
    run(&mut s, "trace_walls", json!({"part": "PIANTA PIANO TERRA", "openings": true})).expect("it builds");
    // Read against the storey above, where this elevation draws none of these openings: every one of them keeps
    // what it was built at, and each says what was in the way of giving it anything else.
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 72_800, "apply": true});
    let out = run(&mut s, "cad_openings", ask).expect("it reads");
    assert_eq!((out["applied"].as_u64(), out["found"].as_u64()), (Some(0), Some(0)), "{out}");
    assert_eq!(out["openings"][0]["left"], json!("the elevation draws nothing across it"), "{out}");
    assert_eq!(windows_of(&s), vec![(1500.0, 900.0); 3], "nothing of the project is changed over a reading like that");
    // And where the walls were never built, there is nothing to give it to: it is said, not invented.
    let mut bare = a_survey_in_parts();
    let ask = json!({"part": "PROSPETTO SUD", "plan": "PIANTA PIANO TERRA", "floor": 69_800, "apply": true});
    let out = run(&mut bare, "cad_openings", ask).expect("it reads");
    assert_eq!((out["applied"].as_u64(), out["left"].as_u64()), (Some(0), Some(2)), "{out}");
    assert_eq!(out["openings"][0]["left"], json!("no opening is built there"), "{out}");
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
