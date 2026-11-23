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

/// A session with a drawing of `lines` under its lowest level, with `blocks` defined in it and `puts` putting them.
fn linked_with_blocks(lines: &[(String, [f64; 4])], blocks: &str, puts: &str) -> Session {
    let path = drawing(lines);
    let text = std::fs::read_to_string(&path).expect("the drawing");
    let with =
        text.replace("0\nSECTION\n2\nENTITIES\n", &format!("0\nSECTION\n2\nBLOCKS\n{blocks}0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{puts}"));
    std::fs::write(&path, with).expect("the drawing again");
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path})).expect("it comes in");
    s
}

/// The project's levels, lowest first.
fn levels(s: &Session) -> Vec<crate::model::Id> {
    let mut found: Vec<crate::model::Id> = ids(s, "level");
    found.sort();
    found
}

#[test]
fn what_each_part_of_a_drawing_is_is_told_to_it_and_kept() {
    let mut lines = room("MURI", [0.0, 0.0, 8000.0, 6000.0], 300.0);
    lines.extend(room("MURI", [100_000.0, 0.0, 108_000.0, 6000.0], 300.0));
    let mut s = linked(&lines);
    let out = run(&mut s, "cad_parts", json!({})).expect("it reads");
    assert_eq!(out["parts"], json!([]), "a drawing is made of nothing until someone says so: {out}");
    let (ground, phase) = (levels(&s)[0], s.model.phases.first().cloned().expect("a phase"));
    let parts = json!([
        {"name": "Ground floor", "kind": "plan", "region": [[-1000, -1000], [9000, 7000]], "level": ground, "phase": phase},
        {"name": "South elevation", "kind": "elevation", "region": [[99_000, -1000], [109_000, 7000]]},
    ]);
    let out = run(&mut s, "cad_parts", json!({"parts": parts})).expect("it takes them");
    assert_eq!(out["parts"].as_array().map(Vec::len), Some(2), "{out}");
    assert_eq!(out["parts"][0]["kind"], json!("plan"));
    assert_eq!(out["parts"][1]["kind"], json!("elevation"));
    assert_eq!(out["parts"][0]["phase"], json!(phase));
    // They are the drawing's own, and the project carries them as it carries the layers' phases.
    let held = s
        .model
        .elements
        .values()
        .find_map(|e| match e {
            Element::Underlay(u) => Some(u.parts.clone()),
            _ => None,
        })
        .expect("the drawing");
    assert_eq!(held.len(), 2);
    assert_eq!(held[0].name, "Ground floor");
    // What is no part of a drawing is refused, rather than kept and traced as nothing.
    let bad = [
        json!([{"name": "A", "kind": "plan", "region": [[9000, 7000], [-1000, -1000]]}]),
        json!([{"name": "A", "kind": "floor plan", "region": [[0, 0], [1000, 1000]]}]),
        json!([{"name": "", "kind": "plan", "region": [[0, 0], [1000, 1000]]}]),
        json!([{"name": "A", "kind": "plan", "region": [[0, 0], [1000, 1000]], "phase": "Someday"}]),
        json!([{"name": "A", "kind": "plan", "region": [[0, 0]]}]),
        json!([{"name": "A", "kind": "plan"}]),
    ];
    for parts in bad {
        assert!(run(&mut s, "cad_parts", json!({"parts": parts})).is_err(), "{parts}");
    }
    // Refused, nothing of what was there is lost.
    assert_eq!(run(&mut s, "cad_parts", json!({})).expect("it reads")["parts"].as_array().map(Vec::len), Some(2));
}

#[test]
fn a_plan_part_is_traced_into_its_own_level_in_its_own_phase_and_nowhere_else() {
    let mut lines = room("MURI", [0.0, 0.0, 8000.0, 6000.0], 300.0);
    lines.extend(room("MURI", [100_000.0, 0.0, 108_000.0, 6000.0], 300.0));
    let mut s = linked(&lines);
    let upper = levels(&s)[1];
    let phase = s.model.phases.last().cloned().expect("a phase");
    let parts = json!([
        {"name": "Upstairs", "kind": "plan", "region": [[-1000, -1000], [9000, 7000]], "level": upper, "phase": phase},
        {"name": "Next door", "kind": "plan", "region": [[99_000, -1000], [109_000, 7000]]},
    ]);
    run(&mut s, "cad_parts", json!({"parts": parts})).expect("it takes them");
    let out = run(&mut s, "trace_walls", json!({"part": "Upstairs"})).expect("it traces");
    assert_eq!(out["walls"], json!(4), "the part and no other: {out}");
    let here = s.model.guid(upper);
    assert!(walls(&s).iter().all(|w| w.level == here), "every wall is on the part's own level");
    let created = |w: &crate::model::Wall| w.params.get("Phase Created").and_then(|v| v.as_str().map(str::to_string));
    assert!(
        walls(&s).iter().all(|w| created(w).as_deref() == Some(phase.as_str())),
        "and in its phase: {:?}",
        walls(&s).iter().map(|w| created(w)).collect::<Vec<_>>()
    );
    assert_eq!(out["phases"], json!([{"phase": phase, "walls": 4}]), "{out}");
    // A part the drawing hasn't is refused, and names the ones it has.
    let e = run(&mut s, "trace_walls", json!({"part": "Attic"})).expect_err("no such part");
    assert!(e.contains("Upstairs") && e.contains("Next door"), "{e}");
}

#[test]
fn the_rooms_of_a_part_take_their_names_from_the_drawing_wherever_the_drawing_stands() {
    // Two plans in one file, the second a floor of its own, each with a room name written inside it. The drawing
    // comes under the lowest level; the second plan is traced onto a level the drawing does not stand on.
    let mut lines = room("MURI", [0.0, 0.0, 8000.0, 6000.0], 300.0);
    lines.extend(room("MURI", [100_000.0, 0.0, 108_000.0, 6000.0], 300.0));
    let path = drawing(&lines);
    let said = |x: f64, y: f64, text: &str| format!("0\nTEXT\n8\nTESTO\n10\n{x}\n20\n{y}\n40\n200\n1\n{text}\n");
    let text = std::fs::read_to_string(&path).expect("the drawing");
    let with = text.replace(
        "0\nENDSEC\n0\nEOF\n",
        &format!("{}{}0\nENDSEC\n0\nEOF\n", said(4000.0, 3000.0, "CUCINA"), said(104_000.0, 3000.0, "CANTINA")),
    );
    std::fs::write(&path, with).expect("the drawing again");
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": path})).expect("it comes in");
    let upper = levels(&s)[1];
    let parts = json!([
        {"name": "Ground", "kind": "plan", "region": [[-1000, -1000], [9000, 7000]]},
        {"name": "Cellar", "kind": "plan", "region": [[99_000, -1000], [109_000, 7000]], "level": upper},
    ]);
    run(&mut s, "cad_parts", json!({"parts": parts})).expect("it takes them");
    run(&mut s, "trace_walls", json!({"part": "Ground", "rooms": true})).expect("it traces");
    run(&mut s, "trace_walls", json!({"part": "Cellar", "rooms": true})).expect("it traces");
    let mut named: Vec<String> = s
        .model
        .elements
        .values()
        .filter_map(|e| match e {
            Element::Room(r) => Some(r.name.clone()),
            _ => None,
        })
        .collect();
    named.sort();
    assert_eq!(named, vec!["CANTINA".to_string(), "CUCINA".to_string()], "the survey names both, on whichever level");
}

#[test]
fn a_drawing_of_many_sheets_is_traced_inside_the_area_asked_for_and_nowhere_else() {
    // A survey saved as a set of sheets in one file: the plan, and a hundred metres away the elevation, whose lines
    // pair as a wall's faces do.
    let mut lines = room("MURI", [0.0, 0.0, 8000.0, 6000.0], 300.0);
    lines.extend(room("PROSPETTO", [100_000.0, 0.0, 108_000.0, 6000.0], 300.0));
    let mut s = linked(&lines);
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!(out["walls"], json!(8), "the whole file, with no area asked for: {out}");
    let mut s = linked(&lines);
    let out = run(&mut s, "trace_walls", json!({"region": [[-1000.0, -1000.0], [9000.0, 7000.0]]})).expect("it traces");
    assert_eq!(out["walls"], json!(4), "the plan and not the elevation: {out}");
    assert!(walls(&s).iter().all(|w| w.start.0 < 9000.0 && w.end.0 < 9000.0), "every wall is in the area");
    // An area is two corners with room between them, and one that is neither is refused rather than finding nothing.
    for bad in [json!([[9000.0, 7000.0], [-1000.0, -1000.0]]), json!([[0.0, 0.0]]), json!("the plan")] {
        let mut s = linked(&lines);
        assert!(run(&mut s, "trace_walls", json!({"region": bad})).is_err(), "{bad}");
    }
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
fn the_rooms_the_walls_shut_in_are_placed_with_them_when_they_are_asked_for() {
    let mut lines = room("MURI", [0.0, 0.0, 8000.0, 6000.0], 300.0);
    // A partition across it, 100 thick, from the one wall to the other.
    lines.push(("MURI".to_string(), [300.0, 2950.0, 7700.0, 2950.0]));
    lines.push(("MURI".to_string(), [300.0, 3050.0, 7700.0, 3050.0]));
    let mut s = linked(&lines);
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!(out["walls"], json!(5), "{out}");
    assert!(out["rooms"].is_null(), "no rooms unless they are asked for: {out}");
    assert!(ids(&s, "room").is_empty());
    // Asked for, each area the walls shut in holds a room, and they are the walls' own areas.
    let mut s = linked(&lines);
    let out = run(&mut s, "trace_walls", json!({"rooms": true})).expect("it traces");
    assert_eq!((out["walls"].as_u64(), out["rooms"].as_u64()), (Some(5), Some(2)), "{out}");
    let mut areas: Vec<f64> = s
        .model
        .elements
        .values()
        .filter_map(|e| match e {
            Element::Room(r) => Some(s.model.room_region(r)?.area()),
            _ => None,
        })
        .collect();
    areas.sort_by(f64::total_cmp);
    // The room is 7700 by 5700 at the wall centre lines, and the partition down the middle halves it.
    assert_eq!(areas, vec![21.945e6, 21.945e6], "{areas:?}");
    assert_eq!(out["room_ids"].as_array().map(Vec::len), Some(2), "{out}");
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
fn a_window_a_survey_draws_as_a_block_is_read_like_any_other() {
    // The wall's two faces run through and the window is a block of its two jambs and its glass, put in the wall
    // where it stands. The block is opened on the wall's own layer, so it reads as a window drawn line by line.
    let wall = vec![("MURI".to_string(), [0.0, 0.0, 6000.0, 0.0]), ("MURI".to_string(), [0.0, 300.0, 6000.0, 300.0])];
    let block = "0\nBLOCK\n2\nFINESTRA\n10\n0\n20\n0\n\
                 0\nLINE\n8\n0\n10\n0\n20\n0\n11\n0\n21\n300\n\
                 0\nLINE\n8\n0\n10\n900\n20\n0\n11\n900\n21\n300\n\
                 0\nLINE\n8\n0\n10\n0\n20\n150\n11\n900\n21\n150\n\
                 0\nENDBLK\n";
    let mut s = linked_with_blocks(&wall, block, "0\nINSERT\n8\nMURI\n2\nFINESTRA\n10\n2550\n20\n0\n");
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!((out["walls"].clone(), out["windows"].clone()), (json!(1), json!(1)), "{out}");
    let (walls, windows) = (ids(&s, "wall"), ids(&s, "window"));
    let Some(Element::Window(o)) = s.model.elements.get(windows[0]) else { panic!("a window") };
    assert_eq!(s.model.elements.id(o.host), Some(walls[0]), "it is in the wall");
    assert!((o.offset - 3000.0).abs() < 1.0, "where the block was put: {}", o.offset);
}

#[test]
fn a_room_takes_the_name_the_survey_writes_in_it() {
    // A surveyor writes a room's name in the room, larger than what he writes under it.
    let walls = room("MURI", [0.0, 0.0, 6000.0, 5000.0], 300.0);
    let words = "0\nTEXT\n8\nTESTI\n1\nSoggiorno\n10\n2000\n20\n2600\n40\n250\n\
                 0\nTEXT\n8\nTESTI\n1\nmq 24,20\n10\n2000\n20\n2000\n40\n120\n";
    let mut s = linked_with_blocks(&walls, "", words);
    let out = run(&mut s, "trace_walls", json!({"rooms": true})).expect("it traces");
    assert_eq!(out["rooms"], json!(1), "{out}");
    let rooms = ids(&s, "room");
    let Some(Element::Room(r)) = s.model.elements.get(rooms[0]) else { panic!("a room") };
    assert_eq!(r.name, "Soggiorno", "the biggest of what is written in it, and not what it measures");
    // A name the caller says is the name, whatever the drawing writes.
    let mut s = linked_with_blocks(&walls, "", words);
    run(&mut s, "trace_walls", json!({})).expect("it traces");
    run(&mut s, "place_rooms", json!({"name": "Ufficio"})).expect("it places");
    let rooms = ids(&s, "room");
    let Some(Element::Room(r)) = s.model.elements.get(rooms[0]) else { panic!("a room") };
    assert_eq!(r.name, "Ufficio");
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

/// The ground floor the inner walls of a permesso di costruire taught this: a room of 300 walls with, inside it, a
/// wall that is 300 thick for most of its length and 130 at one end, sharing its lower face, a 100 wall above it, a
/// partition between the two that is a door from jamb to jamb, and a wall that carries on past the corner of another
/// to a door and the jamb beyond it. Every line on layer `0`.
fn inner_walls() -> Vec<(String, [f64; 4])> {
    let l = |x0: f64, y0: f64, x1: f64, y1: f64| ("0".to_string(), [x0, y0, x1, y1]);
    let mut lines = room("0", [0.0, 0.0, 8000.0, 6000.0], 300.0);
    lines.extend([
        // One lower face under two walls: 300 thick from 3000 on, 130 thick before it.
        l(300.0, 2000.0, 7700.0, 2000.0),
        l(3000.0, 2300.0, 7700.0, 2300.0),
        l(300.0, 2130.0, 3000.0, 2130.0),
        l(3000.0, 2130.0, 3000.0, 2300.0),
        // A 100 wall over it.
        l(300.0, 3300.0, 7700.0, 3300.0),
        l(300.0, 3400.0, 7700.0, 3400.0),
        // Between the two, a partition that is all door: a 100 jamb at each end, closed on the door's side.
        l(4000.0, 2300.0, 4000.0, 2400.0),
        l(4100.0, 2300.0, 4100.0, 2400.0),
        l(4000.0, 2400.0, 4100.0, 2400.0),
        l(4000.0, 3200.0, 4000.0, 3300.0),
        l(4100.0, 3200.0, 4100.0, 3300.0),
        l(4000.0, 3200.0, 4100.0, 3200.0),
        // Above, a wall down from the north wall, met on its east side by a 100 wall at 4500..4600, carrying on past
        // that corner 200 more on its east face, then a door 800 wide and a 100 jamb on the wall below.
        l(6000.0, 5700.0, 6000.0, 4300.0),
        l(6100.0, 5700.0, 6100.0, 4600.0),
        l(6100.0, 4600.0, 7700.0, 4600.0),
        l(6100.0, 4500.0, 7700.0, 4500.0),
        l(6100.0, 4500.0, 6100.0, 4300.0),
        l(6000.0, 4300.0, 6100.0, 4300.0),
        l(6000.0, 3400.0, 6000.0, 3500.0),
        l(6100.0, 3400.0, 6100.0, 3500.0),
        l(6000.0, 3500.0, 6100.0, 3500.0),
    ]);
    lines
}

/// A wall found: its axis's two ends and its thickness.
type Axis = ([f64; 2], [f64; 2], f64);

/// What Find found: each wall as its axis's ends and its thickness, and each opening's width.
fn found(s: &mut Session) -> (Vec<Axis>, Vec<f64>) {
    let out = run(s, "trace_walls", json!({"preview": true})).expect("it reads");
    let point = |v: &Value| [v[0].as_f64().unwrap_or(f64::NAN), v[1].as_f64().unwrap_or(f64::NAN)];
    let walls = out["found"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|w| (point(&w["start"]), point(&w["end"]), w["thickness"].as_f64().unwrap_or(0.0)));
    let holes = out["holes"].as_array().into_iter().flatten().map(|h| h["width"].as_f64().unwrap_or(0.0));
    (walls.collect(), holes.collect())
}

/// The walls found running along `x` (upright) or `y` (across), as their stretch of the other coordinate and their
/// thickness.
fn along(walls: &[Axis], upright: bool, at: f64, near: f64) -> Vec<(f64, f64, f64)> {
    let (k, other) = if upright { (0, 1) } else { (1, 0) };
    walls
        .iter()
        .filter(|(a, b, _)| (a[k] - at).abs() <= near && (b[k] - at).abs() <= near)
        .map(|(a, b, t)| (a[other].min(b[other]), a[other].max(b[other]), *t))
        .collect()
}

#[test]
fn one_face_shared_by_two_walls_of_different_thickness_makes_both() {
    let mut s = linked(&inner_walls());
    let (walls, _) = found(&mut s);
    // The 300 stretch and the 130 one, both on the same lower face: the nearer pair at one end takes nothing from
    // the farther one at the other.
    let thick = along(&walls, false, 2150.0, 20.0);
    assert!(thick.iter().any(|(lo, hi, t)| (*t - 300.0).abs() < 1.0 && *lo <= 3050.0 && *hi >= 7650.0), "{walls:?}");
    let thin = along(&walls, false, 2065.0, 20.0);
    assert!(thin.iter().any(|(lo, hi, t)| (*t - 130.0).abs() < 1.0 && *lo <= 350.0 && *hi >= 2950.0), "{walls:?}");
}

#[test]
fn a_partition_that_is_a_door_from_jamb_to_jamb_is_a_wall_with_a_door_in_it() {
    let mut s = linked(&inner_walls());
    let (walls, holes) = found(&mut s);
    // From the wall under it to the wall over it, 100 thick, with the door between the jambs.
    let partition = along(&walls, true, 4050.0, 20.0);
    assert!(partition.iter().any(|(lo, hi, t)| (*t - 100.0).abs() < 1.0 && *lo <= 2310.0 && *hi >= 3290.0), "{walls:?}");
    assert!(holes.iter().any(|w| (*w - 800.0).abs() <= 120.0), "the door: {holes:?}");
}

#[test]
fn a_wall_carries_on_past_a_corner_through_its_door_to_the_jamb_beyond() {
    let mut s = linked(&inner_walls());
    let (walls, holes) = found(&mut s);
    // Down from the north wall past the corner at 4500, through the door, to the wall under it at 3400.
    let wall = along(&walls, true, 6050.0, 20.0);
    assert!(wall.iter().any(|(lo, hi, t)| (*t - 100.0).abs() < 1.0 && *lo <= 3410.0 && *hi >= 5690.0), "{walls:?}");
    assert!(holes.iter().filter(|w| (**w - 800.0).abs() <= 120.0).count() >= 2, "both doors: {holes:?}");
}

#[test]
fn short_pairs_that_meet_nothing_and_close_no_door_build_nothing() {
    // Two short parallel lines and their end, alone in a room: a mark, and no wall.
    let mut lines = room("0", [0.0, 0.0, 8000.0, 6000.0], 300.0);
    let l = |x0: f64, y0: f64, x1: f64, y1: f64| ("0".to_string(), [x0, y0, x1, y1]);
    lines.extend([l(3000.0, 3000.0, 3000.0, 3200.0), l(3100.0, 3000.0, 3100.0, 3200.0), l(3000.0, 3200.0, 3100.0, 3200.0)]);
    // A porch's piers in a row in front of it, each a box 200 square, 2600 apart: closed at both ends, and no jamb.
    for k in 0..4 {
        let x = 300.0 + 2600.0 * f64::from(k);
        lines.extend(rect("0", [x, -1700.0, x + 200.0, -1500.0]));
    }
    // A door's cross, 600 across, a wall's thickness from the wall's face: it stands on nothing.
    lines.extend([l(5000.0, 5100.0, 5600.0, 5100.0), l(5300.0, 4800.0, 5300.0, 5400.0)]);
    let mut s = linked(&lines);
    let (walls, holes) = found(&mut s);
    assert_eq!(walls.len(), 4, "the room's four walls and nothing else: {walls:?}");
    assert!(holes.is_empty(), "{holes:?}");
}

/// What a preview found standing on its own: each column's middle, its width and its depth.
fn columns_found(s: &mut Session) -> Vec<([f64; 2], f64, f64)> {
    let out = run(s, "trace_walls", json!({"preview": true})).expect("it reads");
    let num = |v: &Value| v.as_f64().unwrap_or(f64::NAN);
    out["found_columns"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| ([num(&c["at"][0]), num(&c["at"][1])], num(&c["width"]), num(&c["depth"])))
        .collect()
}

/// The corner of a living room on a permesso di costruire with a flue in it: an east wall 400 thick (its inner face, its
/// outer face and the line of its render 100 inside that), a cross wall 300 thick running into it, the flue a block
/// 450 square in the corner under the cross wall, and the flue's stack a box 300 by 496 standing out of the east
/// wall's outer face. The faces stop where the flue stands, as the drawing has it, and turn round it. The room is
/// closed by a wall 300 thick to the west and walls 400 thick north and south; the line of the render stops 100 short
/// of the outer corners, as that drawing has it. Every line on layer `0`.
fn flue_corner() -> Vec<(String, [f64; 4])> {
    let l = |x0: f64, y0: f64, x1: f64, y1: f64| ("0".to_string(), [x0, y0, x1, y1]);
    vec![
        // The north wall and the south wall.
        l(6654.0, -7600.0, 10898.0, -7600.0),
        l(6954.0, -8000.0, 10498.0, -8000.0),
        l(6654.0, -14400.0, 10908.0, -14400.0),
        l(6954.0, -14000.0, 10508.0, -14000.0),
        // The west wall, broken where the cross wall meets it.
        l(6654.0, -7600.0, 6654.0, -14400.0),
        l(6954.0, -8000.0, 6954.0, -10685.0),
        l(6954.0, -10985.0, 6954.0, -14000.0),
        // The cross wall: its north face runs to the east wall, its south face stops at the flue and turns down it.
        l(6954.0, -10685.0, 10498.0, -10685.0),
        l(6954.0, -10985.0, 10058.0, -10985.0),
        l(10058.0, -10985.0, 10058.0, -11435.0),
        l(10058.0, -11435.0, 10508.0, -11435.0),
        // The east wall north of the flue, and south of it.
        l(10498.0, -8000.0, 10498.0, -10685.0),
        l(10898.0, -7600.0, 10898.0, -10814.0),
        l(10798.0, -7700.0, 10798.0, -10814.0),
        l(10508.0, -11435.0, 10508.0, -14000.0),
        l(10908.0, -11310.0, 10908.0, -14400.0),
        l(10808.0, -11310.0, 10808.0, -14300.0),
        // The stack, out of the outer face.
        l(10798.0, -10814.0, 11208.0, -10814.0),
        l(11208.0, -10814.0, 11208.0, -11310.0),
        l(11208.0, -11310.0, 10808.0, -11310.0),
    ]
}

#[test]
fn a_flue_in_a_corner_is_a_solid_and_makes_no_door_in_the_walls_it_stands_in() {
    let mut s = linked(&flue_corner());
    let (walls, holes) = found(&mut s);
    assert!(holes.is_empty(), "a flue is no doorway: {holes:?}");
    // The east wall runs whole past the flue, and the cross wall runs on to it.
    let east = along(&walls, true, 10700.0, 30.0);
    assert!(east.iter().any(|(lo, hi, t)| (*t - 400.0).abs() < 15.0 && *lo <= -13900.0 && *hi >= -8100.0), "{walls:?}");
    let cross = along(&walls, false, -10835.0, 20.0);
    assert!(cross.iter().any(|(_, hi, t)| (*t - 300.0).abs() < 1.0 && *hi >= 10490.0), "{walls:?}");
    // The flue and its stack are what stands there: two columns of their sizes.
    let columns = columns_found(&mut s);
    let near = |x: f64, y: f64, a: f64, b: f64| {
        columns.iter().any(|([cx, cy], w, d)| {
            let size = ((w - a).abs() <= 30.0 && (d - b).abs() <= 30.0) || ((w - b).abs() <= 30.0 && (d - a).abs() <= 30.0);
            (cx - x).abs() <= 30.0 && (cy - y).abs() <= 30.0 && size
        })
    };
    assert!(near(10283.0, -11210.0, 450.0, 450.0), "the flue: {columns:?}");
    assert!(near(11055.0, -11062.0, 305.0, 496.0), "its stack: {columns:?}");
    assert_eq!(columns.len(), 2, "{columns:?}");
}

#[test]
fn a_small_closed_square_standing_free_is_a_column_and_no_wall() {
    // A cellar of 300 walls with a pillar 400 square in it, under the corner of the floor above, whose outline the
    // drawing lays over the pillar's two sides in red and carries on across the room.
    let mut lines = room("0", [0.0, 0.0, 6000.0, 5000.0], 300.0);
    lines.extend(rect("0", [2000.0, 2000.0, 2400.0, 2400.0]));
    lines.push(("rosso".to_string(), [2000.0, 2400.0, 5700.0, 2400.0]));
    lines.push(("rosso".to_string(), [2000.0, 2400.0, 2000.0, 300.0]));
    // And the mark of the entrance outside it, a box 300 by 72: a symbol, slenderer than anything that bears a floor.
    lines.extend(rect("0", [3000.0, -600.0, 3300.0, -528.0]));
    let mut s = linked(&lines);
    let (found_walls, holes) = found(&mut s);
    assert_eq!(found_walls.len(), 4, "the cellar's four walls and no wall to the pillar: {found_walls:?}");
    assert!(holes.is_empty(), "{holes:?}");
    assert_eq!(columns_found(&mut s), vec![([2200.0, 2200.0], 400.0, 400.0)]);
    // Built, it is a column of a type its size, standing on the level the walls stand on.
    let out = run(&mut s, "trace_walls", json!({})).expect("it traces");
    assert_eq!(out["columns"], json!(1), "{out}");
    let columns = ids(&s, "column");
    assert_eq!(out["column_ids"], json!(columns));
    let Some(Element::Column(c)) = s.model.elements.get(columns[0]) else { panic!("a column") };
    let kind = &s.model.types.column[&c.type_name];
    assert_eq!((kind.width, kind.depth, c.at), (400.0, 400.0, crate::geom::P2(2200.0, 2200.0)));
    assert_eq!(Some(c.level), walls(&s).first().map(|w| w.level));
}

#[test]
fn a_pier_between_two_openings_is_wall_and_no_column() {
    // A wall 300 thick with a door 1000 wide and a window 1200 wide in it, 500 apart: the pier between them is drawn as a
    // box of its two faces and its two jambs, and it is the wall, not a pillar standing in it.
    let l = |x0: f64, y0: f64, x1: f64, y1: f64| ("0".to_string(), [x0, y0, x1, y1]);
    let lines = vec![
        l(0.0, 0.0, 1000.0, 0.0),
        l(0.0, 300.0, 1000.0, 300.0),
        l(1000.0, 0.0, 1000.0, 300.0),
        l(2000.0, 0.0, 2000.0, 300.0),
        l(2000.0, 0.0, 2500.0, 0.0),
        l(2000.0, 300.0, 2500.0, 300.0),
        l(2500.0, 0.0, 2500.0, 300.0),
        l(3700.0, 0.0, 3700.0, 300.0),
        l(2500.0, 150.0, 3700.0, 150.0),
        l(3700.0, 0.0, 6000.0, 0.0),
        l(3700.0, 300.0, 6000.0, 300.0),
    ];
    let mut s = linked(&lines);
    let (walls, holes) = found(&mut s);
    assert!(columns_found(&mut s).is_empty());
    assert_eq!(walls.len(), 1, "{walls:?}");
    assert_eq!(holes.len(), 2, "{holes:?}");
}

#[test]
fn a_wall_over_a_stair_whose_first_tread_is_its_inner_face_still_meets_the_wall_at_its_end() {
    // A north wall 350 thick over a room and a stair beside it, its inner face broken where the stair's partition meets
    // it; over the stair that face is also where the flight starts, its treads 280 apart under it. The east wall is 300.
    let l = |x0: f64, y0: f64, x1: f64, y1: f64| ("0".to_string(), [x0, y0, x1, y1]);
    let mut lines = vec![
        l(3198.0, -2344.0, 10798.0, -2344.0),
        l(3548.0, -2694.0, 9398.0, -2694.0),
        l(9498.0, -2694.0, 10498.0, -2694.0),
        l(3198.0, -2344.0, 3198.0, -6000.0),
        l(3548.0, -2694.0, 3548.0, -5650.0),
        l(10798.0, -2344.0, 10798.0, -6000.0),
        l(10498.0, -2694.0, 10498.0, -5650.0),
        l(3198.0, -6000.0, 10798.0, -6000.0),
        l(3548.0, -5650.0, 10498.0, -5650.0),
        l(9398.0, -2694.0, 9398.0, -5650.0),
        l(9498.0, -2694.0, 9498.0, -5650.0),
    ];
    for k in 0..10 {
        let y = -2974.0 - 280.0 * f64::from(k);
        lines.push(("Ciano".to_string(), [9498.0, y, 10498.0, y]));
    }
    let mut s = linked(&lines);
    let (walls, _) = found(&mut s);
    let north = along(&walls, false, -2519.0, 20.0);
    assert!(north.iter().any(|(lo, hi, t)| (*t - 350.0).abs() < 1.0 && *lo <= 3400.0 && *hi >= 10640.0), "{walls:?}");
    let east = along(&walls, true, 10648.0, 20.0);
    assert!(east.iter().any(|(_, hi, _)| *hi >= -2530.0), "the east wall stands on the north one: {walls:?}");
}

#[test]
fn a_wall_that_jogs_aside_by_its_thickness_is_carried_round_the_jog() {
    // A partition 100 thick down a cellar that steps 200 to the east halfway, the step drawn as its two faces turning
    // across: the room beside it is closed, so the partition goes round the step.
    let l = |x0: f64, y0: f64, x1: f64, y1: f64| ("0".to_string(), [x0, y0, x1, y1]);
    let mut lines = room("0", [8000.0, -5000.0, 11000.0, 1000.0], 300.0);
    lines.extend([
        l(9298.0, -2414.0, 9298.0, 700.0),
        l(9198.0, -2514.0, 9198.0, 700.0),
        l(9498.0, -2414.0, 9298.0, -2414.0),
        l(9398.0, -2514.0, 9198.0, -2514.0),
        l(9498.0, -4700.0, 9498.0, -2414.0),
        l(9398.0, -4700.0, 9398.0, -2514.0),
    ]);
    let mut s = linked(&lines);
    let (walls, _) = found(&mut s);
    let upper = along(&walls, true, 9248.0, 5.0);
    let lower = along(&walls, true, 9448.0, 5.0);
    let step = along(&walls, false, -2464.0, 5.0);
    assert!(upper.iter().any(|(lo, _, _)| (*lo + 2464.0).abs() <= 1.0), "{walls:?}");
    assert!(lower.iter().any(|(_, hi, _)| (*hi + 2464.0).abs() <= 1.0), "{walls:?}");
    let [(lo, hi, t)] = step[..] else { panic!("one wall across the step: {walls:?}") };
    assert_eq!((lo, hi, t), (9248.0, 9448.0, 100.0));
}
