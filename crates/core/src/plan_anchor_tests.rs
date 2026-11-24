//! A plan part stood under the plan of another floor drawn beside it on the same sheet: two points both plans draw, and
//! the walls traced from it come in under the floor above, not where the sheet draws them.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::geom::P2;
use crate::model::Element;

fn run(s: &mut Session, id: &str, p: Value) -> Result<Value, String> {
    s.execute(id, &p)
}

/// A DXF of `lines`, in a folder of this call's own, and its path.
fn drawing(lines: &[[f64; 4]]) -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bimcraft-plan-anchor-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("piante.dxf");
    let mut text = String::from("0\nSECTION\n2\nENTITIES\n");
    for [x0, y0, x1, y1] in lines {
        text.push_str(&format!("0\nLINE\n8\nMURI\n10\n{x0}\n20\n{y0}\n11\n{x1}\n21\n{y1}\n"));
    }
    text.push_str("0\nENDSEC\n0\nEOF\n");
    std::fs::write(&path, text).unwrap();
    path.to_string_lossy().into_owned()
}

/// The faces of a box of walls 300 thick, 10 m by 6 m outside, its south-west corner at the origin, each point put
/// where `at` says.
fn box_of(at: impl Fn(P2) -> P2) -> Vec<[f64; 4]> {
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64| {
        let c = [P2(x0, y0), P2(x1, y0), P2(x1, y1), P2(x0, y1)].map(&at);
        (0..4).map(|i| [c[i].0, c[i].1, c[(i + 1) % 4].0, c[(i + 1) % 4].1]).collect::<Vec<_>>()
    };
    let mut lines = rect(0.0, 0.0, 10_000.0, 6000.0);
    lines.extend(rect(300.0, 300.0, 9700.0, 5700.0));
    lines
}

/// A sheet with the ground floor's box at the origin and the basement's drawn beside it as `drawn` puts it, the two
/// marked as parts: the session.
fn sheet(drawn: impl Fn(P2) -> P2, basement: [[f64; 2]; 2]) -> Session {
    let mut lines = box_of(|p| p);
    lines.extend(box_of(drawn));
    let mut s = Session::new();
    run(&mut s, "underlay", json!({"path": drawing(&lines)})).expect("it comes in");
    let parts = json!([
        {"name": "PIANTA PIANO TERRA", "kind": "plan", "region": [[-2000, -2000], [12_000, 8000]]},
        {"name": "PIANTA PIANO SEMINTERRATO", "kind": "plan", "region": basement},
    ]);
    run(&mut s, "cad_parts", json!({"parts": parts})).expect("it takes them");
    s
}

/// Whether every wall of the project runs along the ground floor box's centre lines, and how many there are.
fn under_the_ground_floor(s: &Session) -> (bool, usize) {
    let on = |p: P2| [150.0, 9850.0].iter().any(|x| (p.0 - x).abs() < 1.0) || [150.0, 5850.0].iter().any(|y| (p.1 - y).abs() < 1.0);
    let walls: Vec<(P2, P2)> = s
        .model
        .elements
        .values()
        .filter_map(|e| match e {
            Element::Wall(w) => Some((w.start, w.end)),
            _ => None,
        })
        .collect();
    (walls.iter().all(|(a, b)| on(*a) && on(*b) && a.0 > -1.0 && a.0 < 10_001.0 && b.0 > -1.0 && b.0 < 10_001.0), walls.len())
}

#[test]
fn a_basement_drawn_beside_the_ground_floor_is_traced_under_it() {
    // The basement drawn 20 m east of the ground floor, as a sheet lays its floors side by side.
    let mut s = sheet(|p| P2(p.0 + 20_000.0, p.1), [[18_000.0, -2000.0], [32_000.0, 8000.0]]);
    // Stood by the two south corners of the box, which both floors draw.
    let told = json!({"part": "PIANTA PIANO SEMINTERRATO", "plan": [[0, 0], [10_000, 0]], "drawn": [[20_000, 0], [30_000, 0]]});
    run(&mut s, "cad_anchor", told).expect("it stands");
    run(&mut s, "trace_walls", json!({"part": "PIANTA PIANO SEMINTERRATO"})).expect("it builds");
    assert_eq!(under_the_ground_floor(&s), (true, 4), "the basement's walls are under the ground floor's");
}

#[test]
fn a_basement_drawn_turned_is_turned_back_under_the_ground_floor() {
    // Drawn a quarter turn round and east of it: the box's south side runs north there, along x = 20 m.
    let mut s = sheet(|p| P2(20_000.0 - p.1, p.0), [[13_000.0, -1000.0], [21_000.0, 11_000.0]]);
    let told = json!({"part": "PIANTA PIANO SEMINTERRATO", "plan": [[0, 0], [10_000, 0]], "drawn": [[20_000, 0], [20_000, 10_000]]});
    run(&mut s, "cad_anchor", told).expect("it stands");
    run(&mut s, "trace_walls", json!({"part": "PIANTA PIANO SEMINTERRATO"})).expect("it builds");
    assert_eq!(under_the_ground_floor(&s), (true, 4));
}

#[test]
fn a_plan_stood_by_two_points_that_are_not_the_same_two_is_refused() {
    let mut s = sheet(|p| P2(p.0 + 20_000.0, p.1), [[18_000.0, -2000.0], [32_000.0, 8000.0]]);
    let told = json!({"part": "PIANTA PIANO SEMINTERRATO", "plan": [[0, 0], [10_000, 0]], "drawn": [[20_000, 0], [26_000, 0]]});
    let e = run(&mut s, "cad_anchor", told).expect_err("10 m is not 6 m");
    assert!(e.contains("10000 mm apart") && e.contains("6000 mm apart"), "{e}");
    // And where it is not stood, a plan is traced where the sheet draws it, as before.
    run(&mut s, "trace_walls", json!({"part": "PIANTA PIANO SEMINTERRATO"})).expect("it builds");
    assert!(!under_the_ground_floor(&s).0);
}
