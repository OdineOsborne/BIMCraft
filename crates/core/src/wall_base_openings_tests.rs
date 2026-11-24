//! A wall's base moved leaves its doors and windows where they stand, as Revit keeps them at their height over the level:
//! a door's or a window's sill is measured from its wall's base, so the sill changes by as much as the base did.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::model::{Element, Id};

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

/// Where a door's or a window's bottom stands: its wall's base and its sill.
fn bottom(s: &Session, id: Id) -> f64 {
    let m = &s.model;
    let Some(Element::Door(o) | Element::Window(o)) = m.elements.get(id) else { panic!("{id} is no door or window") };
    let Some(Element::Wall(w)) = m.elements.get(o.host) else { panic!("{id} has no wall") };
    m.own_base(w) + o.sill
}

fn sill(s: &Session, id: Id) -> f64 {
    match s.model.elements.get(id) {
        Some(Element::Door(o) | Element::Window(o)) => o.sill,
        _ => panic!("{id} is no door or window"),
    }
}

/// A wall 8 m along x, 3 m tall, with a window at a sill of 900 and a door: the session and the three ids.
fn facade() -> (Session, Id, Id, Id) {
    let mut s = Session::new();
    let wall = run(&mut s, "wall", json!({"points": [[0, 0], [8000, 0]], "height": 3000}))["ids"][0].as_u64().unwrap();
    let window = run(&mut s, "window", json!({"wall": wall, "offset": 2000, "sill": 900}))["id"].as_u64().unwrap();
    let door = run(&mut s, "door", json!({"wall": wall, "offset": 5000}))["id"].as_u64().unwrap();
    (s, wall, window, door)
}

#[test]
fn a_wall_lowered_to_the_pavement_leaves_its_doors_and_windows_where_they_stand() {
    let (mut s, wall, window, door) = facade();
    let (w0, d0) = (bottom(&s, window), bottom(&s, door));
    // The facade taken down to the pavement outside, 264 mm under the floor, as the permesso di costruire's porch is.
    run(&mut s, "modify", json!({"id": wall, "base_offset": -264}));
    assert_eq!((bottom(&s, window), bottom(&s, door)), (w0, d0), "nothing of the openings moved");
    assert_eq!((sill(&s, window), sill(&s, door)), (1164.0, 264.0), "their sills are over the new base");
    // And undone, all of it goes back in one step.
    run(&mut s, "undo", json!({}));
    assert_eq!((sill(&s, window), sill(&s, door)), (900.0, 0.0));
}

#[test]
fn a_door_its_walls_base_is_raised_past_stands_on_it() {
    let (mut s, wall, window, door) = facade();
    run(&mut s, "modify", json!({"id": wall, "base_offset": 300}));
    assert_eq!(sill(&s, window), 600.0, "the window keeps its height");
    assert_eq!(sill(&s, door), 0.0, "nothing of a door hangs under its wall");
    assert_eq!(bottom(&s, door), 300.0);
}

#[test]
fn a_wall_moved_along_its_base_alone_leaves_the_sills_alone() {
    let (mut s, wall, window, door) = facade();
    run(&mut s, "modify", json!({"id": wall, "height": 3500}));
    assert_eq!((sill(&s, window), sill(&s, door)), (900.0, 0.0));
}
