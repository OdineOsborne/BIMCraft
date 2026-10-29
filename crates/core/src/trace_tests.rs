//! Walls traced from a drawing, through the command: what is found in a survey's lines, what is not, which type and
//! phase each wall comes in with, and what is said when the drawing is not in the units it claims.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::model::{Element, Wall};

/// A DXF of the lines given, each on the layer it names, in a folder of this call's own; its path.
fn drawing(lines: &[(String, [f64; 4])]) -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bimcraft-trace-{}-{n}", std::process::id()));
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

/// The four sides of a rectangle, as a draughtsman draws them: four lines on one layer.
fn rect(layer: &str, [x0, y0, x1, y1]: [f64; 4]) -> Vec<(String, [f64; 4])> {
    [[x0, y0, x1, y0], [x1, y0, x1, y1], [x1, y1, x0, y1], [x0, y1, x0, y0]].into_iter().map(|l| (layer.to_string(), l)).collect()
}

/// A room drawn as a survey draws one: its walls' two faces, `t` apart, both on `layer`.
fn room(layer: &str, [x0, y0, x1, y1]: [f64; 4], t: f64) -> Vec<(String, [f64; 4])> {
    let mut out = rect(layer, [x0, y0, x1, y1]);
    out.extend(rect(layer, [x0 + t, y0 + t, x1 - t, y1 - t]));
    out
}

fn run(s: &mut Session, id: &str, p: Value) -> Result<Value, String> {
    s.execute(id, &p)
}

/// The project's elements of a category, by id.
fn ids(s: &Session, category: &str) -> Vec<crate::model::Id> {
    s.model.elements.iter().filter(|(_, e)| e.category() == category).map(|(id, _)| *id).collect()
}

/// The project's walls.
fn walls(s: &Session) -> Vec<&Wall> {
    s.model
        .elements
        .values()
        .filter_map(|e| match e {
            Element::Wall(w) => Some(w),
            _ => None,
        })
        .collect()
}

/// A session with the drawing under its lowest level.
fn linked(lines: &[(String, [f64; 4])]) -> Session {
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": drawing(lines)})).expect("it comes in");
    s
}

#[test]
fn a_room_drawn_as_the_two_faces_of_its_walls_becomes_four_walls_that_meet_at_its_corners() {
    let mut s = linked(&room("MURI", [0.0, 0.0, 8000.0, 6000.0], 300.0));
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!(out["walls"], json!(4), "{out}");
    let found = walls(&s);
    assert!(found.iter().all(|w| (s.model.thickness(w) - 300.0).abs() < 1.0), "every wall is 300 thick");
    // Each axis runs down the middle of its wall and reaches the corners: the south one from the west wall's
    // middle to the east wall's, though the faces that made it stop 300 short of both.
    let south = found.iter().find(|w| w.start.1 < 200.0 && w.end.1 < 200.0).expect("the south wall");
    let (x0, x1) = (south.start.0.min(south.end.0), south.start.0.max(south.end.0));
    assert!((x0 - 150.0).abs() < 1.0 && (x1 - 7850.0).abs() < 1.0, "{:?} to {:?}", south.start, south.end);
    assert!((south.start.1 - 150.0).abs() < 1.0, "down the middle of the wall");
}

#[test]
fn a_face_takes_the_face_across_the_wall_and_not_the_one_across_the_room() {
    // A corridor 600 wide between two walls 100 thick: every distance from 100 to 800 is there to be mistaken for a
    // wall, and only the two 100s are walls.
    let lines: Vec<(String, [f64; 4])> =
        [0.0, 100.0, 700.0, 800.0].into_iter().map(|y| ("MURI".to_string(), [0.0, y, 5000.0, y])).collect();
    let mut s = linked(&lines);
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!(out["walls"], json!(2), "{out}");
    assert!(walls(&s).iter().all(|w| (s.model.thickness(w) - 100.0).abs() < 1.0), "both are 100 thick");
}

#[test]
fn what_is_too_close_or_too_far_apart_to_be_a_wall_is_not_one() {
    for apart in [20.0, 3000.0] {
        let lines = vec![("MURI".to_string(), [0.0, 0.0, 5000.0, 0.0]), ("MURI".to_string(), [0.0, apart, 5000.0, apart])];
        let mut s = linked(&lines);
        let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
        assert_eq!(out["walls"], json!(0), "{apart} apart is no wall: {out}");
    }
    // What the caller is looking for is the caller's to say: a hairline pair is a wall if they say so.
    let lines = vec![("MURI".to_string(), [0.0, 0.0, 5000.0, 0.0]), ("MURI".to_string(), [0.0, 20.0, 5000.0, 20.0])];
    let mut s = linked(&lines);
    let out = run(&mut s, "trace_walls", json!({"thin": 10})).expect("it traces");
    assert_eq!(out["walls"], json!(1), "{out}");
}

#[test]
fn lines_that_are_not_parallel_or_barely_alongside_one_another_are_no_wall() {
    // Two lines 300 apart at one end and 900 at the other: a roof's pitch, a kerb, anything but a wall.
    let splayed = vec![("MURI".to_string(), [0.0, 0.0, 5000.0, 0.0]), ("MURI".to_string(), [0.0, 300.0, 5000.0, 900.0])];
    let mut s = linked(&splayed);
    assert_eq!(run(&mut s, "trace_walls", json!({})).expect("it traces")["walls"], json!(0));
    // Two lines 300 apart that pass one another for 100 mm: not enough of a wall to be one.
    let past = vec![("MURI".to_string(), [0.0, 0.0, 5000.0, 0.0]), ("MURI".to_string(), [4900.0, 300.0, 9900.0, 300.0])];
    let mut s = linked(&past);
    assert_eq!(run(&mut s, "trace_walls", json!({})).expect("it traces")["walls"], json!(0));
}

#[test]
fn each_wall_comes_in_with_the_phase_the_layer_of_its_faces_names() {
    let mut lines = room("Stato di fatto", [0.0, 0.0, 8000.0, 6000.0], 300.0);
    lines.extend(room("Progetto", [20000.0, 0.0, 26000.0, 5000.0], 300.0));
    let mut s = linked(&lines);
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!(out["walls"], json!(8), "{out}");
    let (existing, new) = (s.model.phases.first().cloned(), s.model.phases.last().cloned());
    let created: Vec<Option<String>> =
        walls(&s).iter().map(|w| w.params.get(crate::phasing::CREATED).and_then(|v| v.as_str()).map(str::to_string)).collect();
    assert_eq!(created.iter().filter(|c| *c == &existing).count(), 4, "the survey's four walls are there already");
    assert_eq!(created.iter().filter(|c| *c == &new).count(), 4, "the project's four are built");
    // And the answer says as much, so a user sees what it made of their layers.
    assert_eq!(out["phases"], json!([{"phase": existing, "walls": 4}, {"phase": new, "walls": 4}]), "{out}");
}

#[test]
fn a_thickness_the_project_has_a_type_for_uses_it_and_one_it_hasnt_gets_a_type_of_its_own() {
    let mut lines = room("MURI", [0.0, 0.0, 8000.0, 6000.0], 310.0);
    lines.extend(room("MURI", [20000.0, 0.0, 26000.0, 5000.0], 120.0));
    let mut s = linked(&lines);
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!(
        out["thicknesses"],
        json!([
            {"thickness": 120.0, "type": "Generic - 120mm", "walls": 4, "made": true},
            {"thickness": 310.0, "type": "Exterior - 300mm", "walls": 4, "made": false},
        ]),
        "{out}"
    );
    assert!(s.model.types.wall.contains_key("Generic - 120mm"), "the type it needed, made");
    assert_eq!(s.model.types.wall.get("Generic - 120mm").map(|t| t.thickness), Some(120.0));
    // A type the caller names is the type of every wall, and nothing is made.
    let mut s = linked(&lines);
    run(&mut s, "trace_walls", json!({"type": "Interior - 100mm"})).expect("it traces");
    assert!(walls(&s).iter().all(|w| w.type_name == "Interior - 100mm"), "the type the caller named");
    assert!(!s.model.types.wall.contains_key("Generic - 120mm"), "nothing was made");
}

#[test]
fn what_it_would_build_can_be_seen_first_and_nothing_is_built_by_the_looking() {
    let mut s = linked(&room("MURI", [0.0, 0.0, 8000.0, 6000.0], 300.0));
    let out = run(&mut s, "trace_walls", json!({"preview": true})).expect("it looks");
    assert_eq!(out["walls"], json!(4), "{out}");
    assert_eq!(out["found"].as_array().map(Vec::len), Some(4), "each one, to be looked at: {out}");
    assert!(walls(&s).is_empty(), "the looking built nothing");
    assert!(out["ids"].is_null(), "and named no element");
}

#[test]
fn a_drawing_in_metres_that_says_millimetres_finds_no_walls_and_says_how_long_its_lines_are() {
    // The same room, drawn in metres; `$INSUNITS` of the file it came from said millimetres, and nothing in the
    // drawing says otherwise. Every length is a thousandth of what it should be, so there is no wall in it.
    let small = room("MURI", [0.0, 0.0, 8.0, 6.0], 0.3);
    let mut s = linked(&small);
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!(out["walls"], json!(0), "{out}");
    assert_eq!(out["faces"], json!(0), "not one line of it is as long as a wall");
    assert_eq!(out["longest"], json!(8.0), "and the longest is 8 mm, which is the drawing's units speaking");
    // Told the scale, the same drawing is the same room.
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": drawing(&small), "scale": 1000})).expect("it comes in");
    assert_eq!(run(&mut s, "trace_walls", json!({})).expect("it traces")["walls"], json!(4));
}

#[test]
fn with_no_drawing_under_the_plan_it_says_so_and_with_several_it_asks_which() {
    let mut s = Session::new();
    let err = run(&mut s, "trace_walls", json!({})).expect_err("nothing to trace");
    assert!(err.contains("Link CAD"), "{err}");
    let mut s = linked(&room("MURI", [0.0, 0.0, 8000.0, 6000.0], 300.0));
    run(&mut s, "underlay", json!({"path": drawing(&room("ALTRO", [0.0, 0.0, 8000.0, 6000.0], 300.0)), "name": "Due"}))
        .expect("a second one");
    let err = run(&mut s, "trace_walls", json!({})).expect_err("which of the two?");
    assert!(err.contains("Due") && err.contains("underlay"), "{err}");
    // Named, it traces that one.
    let id = s
        .model
        .elements
        .iter()
        .find_map(|(id, e)| matches!(e, Element::Underlay(u) if u.name == "Due").then_some(*id))
        .expect("the second");
    assert_eq!(run(&mut s, "trace_walls", json!({"underlay": id})).expect("it traces")["walls"], json!(4));
}

#[test]
fn a_face_the_drawing_broke_in_two_makes_one_wall_and_a_doorway_makes_a_door_in_it() {
    // One wall 300 thick and 6 m long, its inner face drawn in two pieces that meet.
    let joined = vec![
        ("MURI".to_string(), [0.0, 0.0, 6000.0, 0.0]),
        ("MURI".to_string(), [0.0, 300.0, 2500.0, 300.0]),
        ("MURI".to_string(), [2500.0, 300.0, 6000.0, 300.0]),
    ];
    let mut s = linked(&joined);
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!(out["walls"], json!(1), "the pieces of one wall are one wall: {out}");
    // The same wall with a 900 doorway in it: one wall with a door in it, which is what a building is.
    let mut s = linked(&doorway());
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!((out["walls"].clone(), out["doors"].clone(), out["windows"].clone()), (json!(1), json!(1), json!(0)), "{out}");
    // Asked for the walls alone, the gap is left as the drawing has it.
    let mut s = linked(&doorway());
    let out = run(&mut s, "trace_walls", json!({"openings": false})).expect("it traces");
    assert_eq!((out["walls"].clone(), out["doors"].clone()), (json!(2), json!(0)), "{out}");
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
fn a_doorway_comes_in_as_a_door_in_the_wall_it_is_in_and_a_window_with_glass_in_it() {
    let mut s = linked(&doorway());
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!(out["openings"], json!([{"kind": "door", "width": 900.0, "type": "Single 900 x 2100", "openings": 1, "made": false}]));
    // It is a door of the project's own type, in the wall that was built, centred on the doorway.
    let (walls, doors) = (ids(&s, "wall"), ids(&s, "door"));
    assert_eq!((walls.len(), doors.len()), (1, 1));
    let Some(Element::Door(o)) = s.model.elements.get(doors[0]) else { panic!() };
    assert_eq!(s.model.elements.id(o.host), Some(walls[0]), "it is in the wall");
    assert!((o.offset - 3000.0).abs() < 1.0, "in the middle of it: {}", o.offset);
    assert_eq!(out["opening_ids"], json!(doors));
    // The same doorway with glass drawn across it is a window, with a sill, and its own type made for it.
    let mut glazed = doorway();
    glazed.push(("MURI".to_string(), [2550.0, 140.0, 3450.0, 140.0]));
    let mut s = linked(&glazed);
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!(out["openings"], json!([{"kind": "window", "width": 900.0, "type": "Fixed 900 x 1500", "openings": 1, "made": true}]));
    let windows = ids(&s, "window");
    let Some(Element::Window(o)) = s.model.elements.get(windows[0]) else { panic!() };
    assert!(o.sill > 0.0, "a window stands off the floor");
    assert_eq!(s.model.types.window["Fixed 900 x 1500"].width, 900.0);
}

#[test]
fn a_preview_says_where_the_openings_are_and_builds_none_of_them() {
    let mut s = linked(&doorway());
    let out = run(&mut s, "trace_walls", json!({"preview": true})).expect("it looks");
    assert_eq!(out["doors"], json!(1));
    let [hole] = out["holes"].as_array().expect("the openings").as_slice() else { panic!("{out}") };
    assert_eq!(hole["kind"], json!("door"));
    assert_eq!(hole["width"], json!(900.0));
    assert_eq!((hole["from"].clone(), hole["to"].clone()), (json!([2550.0, 150.0]), json!([3450.0, 150.0])), "where it runs");
    assert!(ids(&s, "door").is_empty() && ids(&s, "wall").is_empty(), "nothing is built by the looking");
}

#[test]
fn the_lines_of_the_land_round_a_building_are_left_out_and_counted_and_taken_when_they_are_asked_for() {
    // The room, and beyond it the boundary of the plot: a pair of lines as far apart as a wall is thick, drawn where
    // nothing of the building reaches.
    let mut lines = room("MURI", [0.0, 0.0, 8000.0, 6000.0], 300.0);
    let boundary = [[-9000.0, -5000.0, 14000.0, -5000.0], [-9000.0, -4800.0, 14000.0, -4800.0]];
    lines.extend(boundary.into_iter().map(|l| ("CONFINE".to_string(), l)));
    let mut s = linked(&lines);
    let out = run(&mut s, "trace_walls", json!({"preview": true})).expect("it looks");
    assert_eq!((out["walls"].clone(), out["alone"].clone()), (json!(4), json!(1)), "{out}");
    // Built, it is the four walls of the room: the boundary is the land, not the building.
    run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!(walls(&s).len(), 4, "{:?}", walls(&s));
    // And asked for them it takes them, because what it finds is a reading and the user's word is the last one.
    let mut s = linked(&lines);
    let out = run(&mut s, "trace_walls", json!({"alone": true})).expect("it traces");
    assert_eq!((out["walls"].clone(), out["alone"].clone()), (json!(5), json!(1)), "{out}");
    assert_eq!(walls(&s).len(), 5);
}

#[test]
fn the_walls_it_builds_are_walls_like_any_other() {
    let mut s = linked(&room("MURI", [0.0, 0.0, 8000.0, 6000.0], 300.0));
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    let ids: Vec<u64> = out["ids"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
    assert_eq!(ids.len(), 4);
    // They schedule, as walls do.
    let sched = run(&mut s, "schedule", json!({"category": "Walls", "fields": ["Type", "Length"]})).expect("a schedule");
    assert_eq!(sched["rows"].as_array().map(Vec::len), Some(4), "{sched}");
    // And one undo takes all four away again (the first takes away the schedule the line above made).
    run(&mut s, "undo", json!({})).expect("the schedule, undone");
    run(&mut s, "undo", json!({})).expect("the walls, undone");
    assert!(walls(&s).is_empty(), "one command, one undo");
}
