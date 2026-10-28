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
