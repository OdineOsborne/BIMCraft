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
